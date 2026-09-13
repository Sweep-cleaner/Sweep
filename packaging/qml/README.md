# Sweep Qt6/QML kabuk

`--json` CLI üzerine ince QML arayüz. Köprü kodu yok: `main.cpp`
(`SweepBridge`) CLI'ı `QProcess` ile çalıştırıp JSON'u QML'e verir.

```sh
podman build -t sweep -f packaging/podman/Containerfile .   # önce CLI
podman build -t sweep-qml -f packaging/qml/Containerfile.qml .
podman run --rm -v "$PWD/out:/out" sweep-qml                 # -> out/shot.png
```

Host ekranında denemek için `Main.qml` aynı imajla `DISPLAY` paylaşılarak
çalıştırılabilir (X11 soketi bağlanır).

## Ekranlar

Kenar çubuğu rotaları `Sidebar.qml`'de, ekran yükleme/baslık/alt başlık
eşlemeleri `Main.qml`'de tanımlıdır; `verify_qml.py` üçünün de eksiksiz
olmasını zorunlu kılar.

- `Dashboard.qml`, `CleanerScreen.qml`, `FilesScreen.qml`, `DiskScreen.qml`
- `HistoryScreen.qml`, `SystemScreen.qml`, `MemoryScreen.qml`
- `StartupScreen.qml` — temel başlangıç listesi (etki rozeti + aç/kapat)
- `AutostartScreen.qml` — **Başlangıç ve Zamanlanmış Görevler**: başlangıç
  girdileri ve zamanlanmış görevler tek listede; arama/filtre/sıralama,
  ayrıntı paneli, etkinleştir/devre dışı bırak/kaldır/düzenle/geri al, toplu
  işlem, kritik risk için onay uyarısı ve denetim geçmişi (JSON/CSV dışa
  aktarma dâhil). Köprü yöntemleri `main.cpp`'de (`autostartList`,
  `tasksList`, `autostartAct`, `autostartRollback`, `autostartHistory`,
  `autostartExport`).
- `SysInfoScreen.qml`, `NetworkScreen.qml`, `PrivacyScreen.qml`,
  `ScheduleScreen.qml`, `Settings.qml`

## Dosya düzeni

- `qml/` — gerçek kaynaklar (`CMakeLists.txt`'teki `QML_FILES` listesi burayı
  gösterir).
- `qml/themes/` — renk paletleri (`ThemeManager` bunları seçer).
- `build/` — üretilmiş çıktı; elle düzenlenmez.
- `preview-dashboard.html` — panelin yeni tasarımının tarayıcıda açılabilen
  maketi. QML derlemeden tasarımı gözden geçirmek için.

## Windows'ta çalıştırma

```bat
cmake -B packaging\qml\build -S packaging\qml -DCMAKE_BUILD_TYPE=Release
cmake --build packaging\qml\build --config Release
windeployqt packaging\qml\build\Release\sweep-qml.exe
```

`sweep-qml.exe`'yi `sweep.exe`'nin yanına koyun (aynı klasör, `%ProgramFiles%\sweep`
veya `PATH`). Windows'a özel davranışlar `main.cpp`'de `Q_OS_WIN` altında:

- **Tepsi simgesi:** Aç / Hızlı tara / Ayarlar / Çıkış menüsü; kapat düğmesi
  varsayılan olarak tepsiye küçültür (Ayarlar → Tray'den kapatılabilir).
  Temizlik ve bellek optimizasyonu bitince bildirim gösterilir.
- **UAC yükseltme:** Cleaner ekranında yönetici gerektiren temizlik
  (`sweep.needsElevated()`) ayrı bir `runas` süreciyle koşar; JSON sonucu
  geçici dosyadan okunup normal sinyallerle QML'ye döner.
- **Win11 Mica + koyu başlık çubuğu:** `DwmSetWindowAttribute` ile; Win11
  22H2+ `DWMWA_SYSTEMBACKDROP_TYPE` (Mica), eski Win11 yapıları
  `DWMWA_MICA_EFFECT` geri dönüşü kullanır. Mica aktifken pencere arka planı
  saydamlaşır (`sys.winMica`); Win10 ve diğer platformlarda görünüm değişmez.
- **Kalıcı ayarlar:** tema, vurgu rengi, pencere/panel saydamlığı, blur,
  animasyon hızı, dil ve tepsi tercihi `QSettings`'e yazılır; bir sonraki
  açılışta geri yüklenir.
- **Çekirdek başına CPU:** Linux'ta `/proc/stat`'ten per-core veri gelir ve
  Sistem ekranında animasyonlu barlar çizilir; Windows'ta dürüst bir tek
  global bar gösterilir (`GetSystemTimes` sınırı).

Windows GUI derlemesi CI'da `windows-gui` job'ıyla, sürüm paketlemesi de
`release.yml`'de `sweep-qml-windows.zip` olarak doğrulanır.

## Doğrulama

```sh
python3 verify_qml.py
```

`qml/` altındaki her dosyanın `CMakeLists.txt`'teki `QML_FILES` listesinde tam
olarak bir kez bulunduğunu, her bileşen referansının kayıtlı bir dosyaya
çözüldüğünü ve parantezlerin dengeli olduğunu denetler. Listede olmayan bir
dosya derlemeye hiç girmez ve yalnızca çalışma anında hata verir; bu script o
durumu CI'da yakalar.

