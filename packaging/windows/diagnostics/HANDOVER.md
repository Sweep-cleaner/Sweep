# Devir teslim notu - Sweep Windows GUI acilmama sorunu

Tarih: 2026-09-12   Kapsam: Windows / MinGW / Qt 6.8.1 `win64_mingw`

## Durum

Duzeltildi ve dogrulandi. Kurulu paket:
`C:\Program Files\Sweep` (133 dosya, `verify_gui_package.py` -> PASS).

Uretilen kurulum: `packaging\dist\windows\Sweep-0.4.0-Setup.exe`
(34,38 MB, sha256 `4BD77D5D6D153F2E12E6FD8B774D0F0D14233192D26351D9C18D874C77C48A1B`).

## Iki kok neden

1. **Paketleme / CRT.** Paket `sweep-qml.exe`'yi UCRT ile bagliyordu
   (`api-ms-win-crt-*`) ama Qt 6.8.1 `win64_mingw` DLL'leri `msvcrt.dll` ile
   bagli. Iki CRT bir arada -> pencere olusmadan `0xC0000005` (ntdll icinde).
   Ayrica stage takasi, yeni stage'i eski stage'in icine koyabiliyordu; bu
   yuzden `makensis` eski ikiliyi paketliyordu.

2. **Uygulama / tepsi.** `Tray` sinifi `new QMenu()` cagiriyordu; `QMenu` bir
   QWidget'tir ve uygulama `QGuiApplication` kullaniyordu ->
   `QWidget: Cannot create a QWidget without QApplication` = `qFatal` = `abort()`.
   Gercek tepsi olan her makinede pencere cizilemeden olum.

## Ortam gereksinimleri (derleme)

- Qt 6.8.1 `win64_mingw`: `.workbuddy-ai/tools/qt/6.8.1/mingw_64`
- Qt'nin eslesen MinGW 13.1: `.workbuddy-ai/tools/qt-mingw/Tools/mingw1310_64`
- NSIS 3.09: `.workbuddy-ai/tools/nsis-3.09`
- `cmake` + `ninja` (`python -m pip install cmake ninja`)
- Rust 1.88 gnu (`RUSTUP_TOOLCHAIN=1.88-x86_64-pc-windows-gnu`)
- Python 3 (dogrulayici icin)

## Tek komut

```powershell
powershell -ExecutionPolicy Bypass -File packaging\dist\windows\build-setup.ps1
python packaging\windows\verify_gui_package.py "C:\Program Files\Sweep"
```

## Bilinen sinirlamalar ve kalan riskler

- Duzeltme MinGW + msvcrt Qt zinciri icindir. Qt'nin MSVC varyantiyla
  derlenirse bu karisiklik olusmaz; o durumda `verify_gui_package.py`'nin
  `msvcrt.dll` beklentisi guncellenmelidir.
- GPU hizlandirma varsayilan olarak KAPALI (`SWEEP_GPU=1` ile acilir): bu
  makinede donanim GL yolu `0xC0000005` ile cokuyor. Yazilim arka ucu daha
  yavas ama her makinede acilir.
- Kurulum paketi hala saglayici imzasi tasimiyor (SmartScreen uyarisi beklenir);
  bkz. `packaging/windows/sign-dev.ps1`.
- Derleme sirasinda olusan `packaging/dist/windows/stage*` ve
  `packaging/qml/build-console` dizinleri buyuyebilir; pakete girmezler
  (`sweep.nsi` yalnizca `-DSTAGE` ile verilen dizini paketler).
- Temizlik notu: duzenleme oncesi alinan yedekler depoda duruyor ve silinemedi
  (guvenlik korumasi silme komutlarini onay gerektiriyor):
  `packaging\qml\main.cpp.bak-before-grab`,
  `packaging\qml\main.cpp.bak-before-qapp`,
  `packaging\dist\windows\sweep.nsi.bak-before-crtfix`.
  Elle silinebilir.
- `stage-verify-0918` ve `packaging/windows/diagnostics/extract-0854` gecici
  dogrulama klasorleridir; elle silinebilir.

## Sorun tekrarlarsa

`packaging/windows/README.md` -> "GUI acilmiyor / pencere bos kaliyor"
bolumundeki 6 maddelik liste.
