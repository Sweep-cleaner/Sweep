# Sweep GUI entegrasyon hata senaryoları

| # | Senaryo | Belirti | Neden | Çözüm | GUI davranışı |
|---|---------|---------|-------|-------|---------------|
| 1 | Boş seçimle preview | `{"error":"nothing selected"}` | `preview` altkomutuna seçici verilmedi | Boş seçimde `--all` ekle | `failed()` ile status satırında göster, çökme yok |
| 2 | Bilinmeyen cleaner | `{"error":"unknown cleaner 'x'"}` | Yanlış `cleaner.option` anahtarı | `list` çıktısındaki anahtarları kullan | Hata metni status satırında, liste korunur |
| 3 | Binary yok | `FailedToStart`, `cannot start sweep` | `SWEEP_BIN` veya `/usr/local/bin/sweep` eksik | `SWEEP_BIN=/yol/sweep` export et | `lastError` set edilir, busy sıfırlanır |
| 4 | İş sürerken ikinci komut | `busy` hatası | Tek `QProcess` slotu dolu | Önce `cancel()` çağır, `busyChanged` bekle | İkinci komut kuyruğa alınmaz, kullanıcı bilgilendirilir |
| 5 | Temizlik yarıda iptal | `CrashExit`, kısmi stdout | `terminate()` sonrası 3sn doldu, `kill()` devreye girdi | Kısmi raporu yok say, yeniden `preview` çalıştır | `failed()` emit, progress sıfırlanır |
| 6 | JSON dışı stdout | `preview failed` / `clean failed` | Backend insan-formatında çıktı (`--json` unutuldu) | Her çağrıda `--json --quiet` kullan | Parse guard `entries` dizisini kontrol eder |
| 7 | Şema değişti | `bad list` / `bad report` | Backend yeni alan ekledi veya kaldırdı | `report.schema.json` ile validate et, guard'ları gevşek tut | Bilinmeyen alanlar yok sayılır, eksik alan varsayılanlanır |
| 8 | `/proc` yok (macOS/Windows) | CPU %0, RAM 0 | `SysMonitor` Linux yolu okuyamaz | Değerler son iyi durumda donar, QML NaN göstermez | `readCpu` toplam 0 ise erken dönülür |
| 9 | Podman DISPLAY yok | Headless shot siyah | Xvfb başlamadı | `start.sh` sırasını koru: Xvfb, bekle, app, scrot | Konteyner logunda `screenshot at /out/shot.png` görülmeli |
| 10 | Tema yüklenemez | Ekran boş | `themes/` kopyalanmadı | `Containerfile.qml` `COPY qml` satırını kontrol et | `ThemeManager` varsayılan Tokyo Night'a düşer |
| 11 | Mica görünmüyor (Win11) | Düz arka plan | Eski Win11 yapısı ya da DWM kapalı | `main.cpp` `DWMWA_SYSTEMBACKDROP_TYPE` → `DWMWA_MICA_EFFECT` geri dönüşü; ikisi de olmazsa `sys.winMica=false` kalır, düz renk çizilir | Koyu başlık çubuğu yine de uygulanır; görünüm normal tema rengidir |
| 12 | Tepsi simgesi yok | Menü yok | `QSystemTrayIcon::isSystemTrayAvailable()` false (Explorer yok, uzak oturum) | Otomatik: tepsi yoksa GUI normal pencerede kalır, kapat düğmesi uygulamayı kapatır | `minimizeToTray` yok sayılır |
| 13 | Yükseltilmiş temizlik çıktı vermedi | "elevated sweep produced no output" | `cmd /c` yönlendirmesi başarısız ya da sweep.exe PATH'te yok | `SWEEP_BIN` ile sweep.exe yolunu sabitleyin; elle `cmd /c ""sweep.exe" --quiet --json clean --yes --all > out.json"` deneyin | Kullanıcı UAC'yi reddederse "elevation cancelled" gösterilir |
| 14 | Ayarlar sıfırlanıyor | Tema/açılış değişmiyor | `QSettings` kayıt defterine yazamıyor (kısıtlı kullanıcı) | `regedit`'te `HKCU\Software\Sweep` yazılabilirliğini kontrol edin | Varsayılanlar kullanılır, uygulama çalışmaya devam eder |
