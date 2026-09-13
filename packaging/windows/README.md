# Windows packaging (sweep.exe)

Build on Windows:

```powershell
cargo build --release
```

The CLI binary (`sweep.exe`) keeps its console on purpose — it is a
terminal tool. The graphical interface is the Qt6/QML shell under
`packaging/qml/`.

The GUI carries the Sweep logo in three places: the sidebar and the
Settings "about" card (`packaging/qml/qml/assets/sweep-logo.png`,
shipped through `CMakeLists.txt` RESOURCES), and the window/taskbar
icon (`app.setWindowIcon` in `packaging/qml/main.cpp`, resolved next
to the QML sources). The `.exe` icon itself is embedded at link time
by `build.rs` from `packaging/windows/sweep.ico`. All three derive
from the `Sweep-Logo.jpg` master at the repo root — regenerate them
with `python packaging/windows/make-ico.py` after replacing the
master (needs Pillow).

## Windows-specific features (Rust CLI)

- `sweep startup list / disable / enable` reads and edits the `Run` registry
  keys (`HKCU` + `HKLM`). Disabling **moves** the value into a `sweep-disabled`
  subkey instead of deleting it, so it is always reversible; HKLM writes need
  an elevated shell.
- `sweep memopt` trims process working sets with `EmptyWorkingSet` — no admin
  needed for user processes.
- `cleaners/windows.toml` ships 13 options: recycle bin, prefetch, error
  reports, Windows Update / Delivery Optimization / Explorer / Store caches,
  thumbnail databases, DirectX shader cache, crash dumps, memory dumps and
  recent documents.

## Qt6 GUI build on Windows

The C++ shell is portable: `SysInfo` reads CPU/memory/disk through
`GetSystemTimes` / `GlobalMemoryStatusEx` / `GetDiskFreeSpaceExW`,
elevation is probed with `net session`, and `sweep.exe` is found next
to the GUI, under `%ProgramFiles%\sweep`, or on `PATH`. Build with
Qt 6 + CMake + Ninja on `PATH`:

```powershell
cargo build --release
cmake -B packaging/qml/build -S packaging/qml -G "Ninja" -DCMAKE_BUILD_TYPE=Release
cmake --build packaging/qml/build --config Release
windeployqt packaging/qml/build/sweep-qml.exe
```

Ship `sweep-qml.exe` next to `sweep.exe` (same directory); the GUI
finds the CLI there without any configuration. `SWEEP_BIN` overrides
the lookup, `SWEEP_QML_DIR` overrides the QML search path.

## GUI çökerse: hangi başlangıç yolu?

Görüntü gerektiren üç başlangıç yolu var ve her biri ayrı ayrı kapatılabilir
(hepsi ayarlanmadıkça etkisiz):

## Grafik: donanım yoksa yazılıma düşer

Qt'nin `win64_mingw` derlemesinde **ANGLE yok** (`libEGL`/`libGLESv2` yok), bu
yüzden Qt Quick doğrudan makinenin OpenGL sürücüsüne bağlı. Sürücü yok veya
bozuksa Qt Quick bir GL bağlamı kuramaz, `qFatal` fırlatır ve süreç `ntdll`
içinde `ACCESS_VIOLATION` ile ölür.

Buna karşı GUI **varsayılan olarak yazılım sahnesini** kullanır; donanım
hızlandırma `SWEEP_GPU=1` ile açıkça istenir.

Bu yönde karar bilinçli: bir yoklama (GL bağlamı kuruldu mu?) bağlamın
kurulduğunu söyleyip sahne grafiği yine çökertebilir. Asimetri açık —
**yanlış negatif = uygulama hiç açılmıyor, yanlış pozitif = biraz yavaş ama
çalışan bir arayüz.** Bu yüzden riskli olan taraf değil, güvenli taraf
varsayılan.

| Değişken | Etki |
|---|---|
| *(hiçbiri)* | **yazılım** (güvenli varsayılan) |
| `SWEEP_GPU=1` | donanım; bağlam kurulamazsa yine yazılıma döner |
| `SWEEP_SOFTWARE=1` | yazılıma zorla |

Donanım hızlandırma makinenizde sorunsuzsa ve animasyonları daha akıcı
istiyorsanız `SWEEP_GPU=1` ile çalıştırın; sorun çıkmazsa varsayılanı
ileride tersine çevirebiliriz.

Kalıcı ve en sağlam çözüm, Qt'nin **MSVC** varyantıyla derlemektir (ANGLE
gelir, yerel OpenGL'e bağımlılık kalkar); MinGW ile dağıtımda bu geri dönüş
gerekir.

## Diğer başlangıç yolları

| Değişken | Kapatılan yol |
|---|---|
| `SWEEP_NO_TRAY=1` | sistem tepsisi (`Shell_NotifyIcon`) |
| `SWEEP_NO_MICA=1` | koyu başlık çubuğu + Mica (DWM) |

Hangisinin suçlu olduğunu **otomatik** bulmak için, ekranı olan bir makinede:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\diagnose-gui.ps1
```

Betik GUI'yi her yapılandırmayla birkaç saniye açar ve hangisinin ayakta
kaldığını yazar. (Başsız ortamda anlamsızdır — orada her şey zaten açılamaz.)

## SmartScreen: "Windows protected your PC"

This is **not** malware detection. SmartScreen (AppRep) blocks any `.exe`
that is unsigned and therefore has no reputation. A binary you just
compiled has no publisher identity at all, so Windows warns — even
though you built it yourself. Nothing in the source can change that;
the fix is a signature.

### 1. Local development (free, no certificate needed)

`sign-dev.ps1` creates a self-signed code-signing certificate, trusts it
**for your user account only**, and authenticodes the built binaries.
Windows then launches them without the dialog:

```powershell
cargo build --release
powershell -ExecutionPolicy Bypass -File packaging/windows/sign-dev.ps1
```

Run it again after every rebuild that you intend to launch. Undo with
`-Remove`, which deletes the certificate from `My`, `Root` and
`TrustedPublisher`.

> **Be aware of what this does.** Putting a certificate in Trusted Root
> lets it sign *anything* on this machine, so keep it to a dev account
> and remove it (`-Remove`) when you no longer need it. It only affects
> your own PC — nobody else trusts it.

### 2. Public releases (needs a real certificate)

`.github/workflows/release.yml` signs `sweep.exe` and `sweep-qml.exe`
automatically when these repository secrets exist:

| Secret | Value |
|---|---|
| `WINDOWS_SIGN_CERT_BASE64` | the PFX, base64-encoded |
| `WINDOWS_SIGN_CERT_PASSWORD` | the PFX password |

Without them the release still builds and publishes — it just warns, and
the job logs a notice saying so. Set them with:

```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes('cert.pfx')) | Set-Clipboard
```

An **EV** certificate gives instant SmartScreen reputation; an **OV**
one earns it over a few days of downloads. For an open-source project,
[SignPath.io](https://signpath.io) and Azure Trusted Signing both offer
free or low-cost options that integrate with the same two secrets.

### 3. One-off bypass (no signing at all)

- Click **More info → Run anyway** once per binary.
- For a *downloaded* file, remove the Mark of the Web first:
  `Unblock-File .\sweep.exe`. (Files you built locally are not
  marked, so this only matters for downloads.)

---

## GUI acilmiyor / pencere bos kaliyor (iki kanitlanmis kok neden)

**Belirti.** Kurulum tamamlaniyor, kisayol calistiriliyor; ya hicbir pencere
gorunmuyor, ya bos/beyaz bir pencere kalıyor ve surec sessizce oluyor. Hata
mesaji hicbir yere yazilmaz: ikili Windows-subsystem olarak baglandigi icin
konsolu yoktur ve Qt'nin `qFatal` ciktisi gorunmez.

Iki ayri kusur vardir; ikisi de duzeltildi.

### Kok neden 1 - paketlenen GUI UCRT ile bagliydi (paketleme hatasi)

`sweep-qml.exe` **UCRT** ile bagliydi (`api-ms-win-crt-*`), oysa yaninda
dagitilan Qt 6.8.1 `win64_mingw` DLL'leri **msvcrt.dll** ile bagli. Ayni surecte
iki C calisma zamani, pencere olusturulmadan once baslatma yolunda ntdll icinde
`0xC0000005` uretir.

Kanit:

| Kanit | Deger |
|---|---|
| `%LOCALAPPDATA%\CrashDumps\sweep-qml.exe.*.dmp` | 14 döküm, hepsi `0xC0000005`, hepsi `ntdll.dll` icinde (11.09 19:18 -> 12.09 09:17) |
| Olay Gunlugu (Application, Id 1000) | `Hatali modul adi: ntdll.dll`, `Ozel durum kodu: 0xc0000005` |
| `verify_gui_package.py "C:\Program Files\Sweep"` | `FAIL - sweep-qml.exe links the UCRT ... while Qt links msvcrt.dll` |
| Konsollu derleme (`-DSWEEP_WIN_GUI_SUBSYSTEM=OFF`) ile UCRT ikili | **hicbir** cikti uretmedi - Qt baslatmaya hic ulasmiyor |

Ikincil olarak `build-setup.sh`'in stage takasi, hedef dizin dururken `mv`
yaptigi icin yeni stage'i eski stage'in **icine** koyabiliyordu
(`stage.new.*` klasorleri); `makensis` sonra eski (UCRT) ikiliyi paketliyordu.

### Kok neden 2 - tepsi menusu QApplication olmadan olusturuluyordu (uygulama hatasi)

`Tray` sinifi baglam menusunu `new QMenu()` ile kurar; `QMenu` bir **QWidget**'tir
ve `QGuiApplication` altinda olusturulamaz:

```
QWidget: Cannot create a QWidget without QApplication
```

Bu bir `qFatal`'dir, yani `abort()`. Uygulama `QGuiApplication` kullaniyordu, bu
yuzden gercek bir masaustunde (tepsi **mevcut** oldugunda) pencere acilir
acilmaz surec oluyordu: kullanici ya bos bir pencere ya da Windows'un
"Hizli Basarisizlik" penceresini goruyordu.

Kanit:

| Kanit | Deger |
|---|---|
| Konsollu derleme, tepsi acik | stderr: `QWidget: Cannot create a QWidget without QApplication` |
| Ayni ikili, `SWEEP_NO_TRAY=1` | **tam arayuz ciziliyor** (koyu tema, kenar cubugu, kartlar) |
| Tepsi acikken ekran goruntusu | pencere bos/beyaz, ardindan `sweep-qml.exe - Hizli Basarisizlik Ozel Durumu` diyalogu |
| Neden hic yakalanmadi | offscreen/headless smoke testlerde `isSystemTrayAvailable()` **false** -> tepsi hic kurulmaz -> hata gorunmez |

**Duzeltme.** `packaging/qml/main.cpp` artik `QApplication` kullaniyor
(`Qt6::Widgets` zaten bagli ve `Qt6Widgets.dll` zaten dagitiliyordu).

### Kontrol

```powershell
python packaging\windows\verify_gui_package.py "C:\Program Files\Sweep"   # paket
python packaging\windows\verify_gui_package.py packaging\qml\build-mingw13\sweep-qml.exe   # tek ikili
```

FAIL satiri gorursen bu paket gonderilmemelidir.

### Kalici duzeltme / tek komutla yeniden derleme

```powershell
powershell -ExecutionPolicy Bypass -File packaging\dist\windows\build-setup.ps1
```

Sirasiyla: `cargo build --release` -> GUI'yi **Qt'nin kendi MinGW 13.1'i** ile
derler (derleyiciler acikca verilir; PATH'teki UCRT MinGW asla kullanilmaz) ->
taze ikiliyi denetler -> temiz bir `stage.new.<zaman>` dizinine kurar ->
`windeployqt` -> staging'i denetler -> `makensis -DSTAGE=<dogrulanan dizin>` ->
uretilen kurulumun **icindeki** `sweep-qml.exe`'yi staged ikiliyle sha256
karsilastirir. Herhangi bir adim basarisiz olursa betik **paketlemez**.

`-SkipCli` / `-SkipGuiBuild` ile yalnizca paketleme adimi da calistirilabilir.

Kuruluma eklenen korumalar:

- `sweep.nsi`, `sweep-qml.exe` + `Qt6Core.dll` + `qml\Main.qml` yoksa
  **derlenmez** (`!error`) - GUI'siz kurulum uretilemez.
- `Sweep` kisayolu artik **GUI'yi** acar; terminal araci `Sweep (CLI)` adini alir.
- Kurulum ve kaldirma, eski bozuk kurulumun `stage.new.*` kalintilarini temizler.

### Gorsel dogrulama

En saglam yol, uygulamanin kendi sahnesini yazdirmasidir (ekran goruntusu
araclarinin occlusion / kilitli oturum / RDP sorunlarindan bagimsiz):

```powershell
$env:SWEEP_GRAB="$env:TEMP\sweep.png"
& "C:\Program Files\Sweep\sweep-qml.exe"      # ~3.5 sn sonra PNG yazar, cikis kodu 0
```

Pencere acilisini ve omrunu olcmek icin:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\gui-probe.ps1 `
    -Exe "C:\Program Files\Sweep\sweep-qml.exe" `
    -LogPath packaging\windows\diagnostics\gui-launch-log.jsonl
```

### Belirti tekrarlarsa bakilacak yerler

1. `python packaging\windows\verify_gui_package.py "C:\Program Files\Sweep"` -
   CRT uyusmazligi veya kalinti varsa burada gorunur.
2. `Get-ChildItem "$env:LOCALAPPDATA\CrashDumps" -Filter "sweep-qml*"` - yeni
   döküm olustu mu, saati ne?
3. `Get-WinEvent -LogName Application -MaxEvents 50 | Where-Object { $_.Id -eq 1000 -and $_.Message -match 'sweep' }`
   - hatali modul ve uzaklik.
4. Pencere bos ise: `SWEEP_NO_TRAY=1` ve `SWEEP_NO_MICA=1` ile A/B yapin. Bos
   pencere + tepsi acikken olum = qFatal yolu; hangi bilesen oldugunu bu ayirt eder.
5. Qt mesajlarini gormek icin konsollu derleme:
   `cmake -B packaging\qml\build-console -S packaging\qml -G Ninja -DSWEEP_WIN_GUI_SUBSYSTEM=OFF ...`
   (Qt, konsolu olmayan ikilide mesajlari `OutputDebugString`'e yazar; DebugView
   da kullanilabilir.)

### Ortam gereksinimleri (derleme makinesi)

- Qt 6.8.1 `win64_mingw` (`.workbuddy-ai/tools/qt/6.8.1/mingw_64`)
- Qt'nin eslesen MinGW 13.1 (`.workbuddy-ai/tools/qt-mingw/Tools/mingw1310_64`)
- NSIS 3.09 (`.workbuddy-ai/tools/nsis-3.09`)
- `cmake` + `ninja` (`python -m pip install cmake ninja`)
- Rust 1.88 gnu: `RUSTUP_TOOLCHAIN=1.88-x86_64-pc-windows-gnu`
- Python 3 (dogrulayici icin)

**Bilinen sinirlamalar.** Duzeltme Windows + MinGW + msvcrt Qt zinciri icindir.
GPU hizlandirma varsayilan olarak KAPALIDIR (`SWEEP_GPU=1` ile acilir) cunku bu
makinede donanim GL yolu `0xC0000005` ile cokuyor; yazilim arka ucu daha
yavastir ama her makinede acilir.
