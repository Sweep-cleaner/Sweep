# Başlangıç ve Zamanlanmış Görevler

Sweep, bilgisayar açılırken ya da oturum açılırken kendiliğinden başlayan her
şeyi **tek bir listede** toplar ve her değişikliği **önce yedekleyip** sonra
uygular. Bu belge hem `sweep startup …` / `sweep tasks …` komutlarını hem de
GUI'deki **Başlangıç ve Zamanlanmış Görevler** ekranını kapsar.

## Ne listelenir?

Tek listede iki tür girdi bir arada görünür:

- **Başlangıç girdileri** — kayıt defteri `Run` / `RunOnce`, ilke
  (`Policies\Explorer\Run`), `Wow6432Node` aynaları, `Startup` klasörleri
  (kullanıcı ve sistem), `Winlogon` ve oturum açma betikleri; Linux'ta
  `~/.config/autostart` (XDG), `systemd --user` birimleri, `cron` ve kabuk
  başlangıç dosyaları; macOS'ta launchd ajanları/daemon'ları.
- **Zamanlanmış görevler** — Windows Görev Zamanlayıcı görevleri, systemd
  zamanlayıcıları ve sistem cron'u.

Her satırda şunlar bulunur: ad, kategori rozeti, risk rozeti, kaynak
(`registry-run`, `scheduled-task`, …), kapsam (kullanıcı / sistem),
tetikleyici (oturum açma, önyükleme, zamanlama, olay, boşta), durum
(etkin / devre dışı), son çalışma zamanı ve varsa etki düzeyi.

> Not: `sweep startup list` yalnız başlangıç girdilerini **ve** oturum
> açma/başlatma tetikleyicili görevleri döner. Ekrandaki birleşik liste bunun
> yanında `sweep tasks list` çağrısını da yapar ve iki sonucu kimliğe göre
> birleştirir; böylece yalnız zamanlamayla çalışan görevler de görünür.

## Arama, filtre ve sıralama

- **Arama**: ad, komut ve konum (kaynak dâhil) üzerinde anlık, büyük/küçük
  harf duyarsız.
- **Filtreler**: tür (başlangıç / zamanlanmış), kapsam (kullanıcı / sistem),
  kategori, risk ve durum (etkin / devre dışı).
- **Sıralama**: ad, kaynak veya etki.

Filtreler CLI bayraklarına çevrilip listeleme **serileştirmeden önce**
uygulanır; kalan birleştirme/ayıklama işi tek bir geçişte yapılır. 500'den
fazla girdide liste akıcı kalır (satır geri dönüşümü açık `ListView`).

## Etkinleştirme, devre dışı bırakma, kaldırma, düzenleme

```sh
sweep startup list                       # hepsi
sweep startup list --kind task --risk critical
sweep startup list --search onedrive --disabled
sweep startup enable  "<id|ad>"          # yeniden başlat
sweep startup disable "<id|ad>"          # başlatmayı durdur (silmez)
sweep startup remove  "<id|ad>"          # girdiyi kaldır (yedeklenir)
sweep startup edit    "<id|ad>" --command "C:\\yol\\uygulama.exe --argüman"
sweep startup history --limit 100        # denetim defteri
```

Ad yerine **kimlik** kullanmak belirsizliği önler: aynı ad birden çok girdide
geçerse CLI tahmin etmez, `ambiguous` hatası verir. Kimlikler
`sweep startup list --json` çıktısındaki `id` alanındadır ve kararlıdır.

**Devre dışı bırakma hiçbir şeyi silmez.** Sweep yalnız girdinin etkin
olmadığını işaretler:

- Windows kayıt defteri: değer `…\sweep-disabled` alt anahtarına taşınır.
- Windows Görev Zamanlayıcı: görev devre dışı bırakılır (`Disable-ScheduledTask`).
- XDG `.desktop`: `Hidden=true` yazılır.
- launchd plist: `.disabled` uzantısına taşınır.

Kaldırma ve düzenleme gerçek bir değişikliktir; ikisi de önce yedeklenir.

## Geri alma (rollback) neyi geri getirir?

Her değiştirici işlem şu sırayı izler:
**politika kontrolü → yedek (fail closed) → uygula → yeniden okuyup doğrula →
deftere yaz.**

- Yedek **yazılamazsa değişiklik yapılmaz** ("fail closed").
- Geri alma, o girdi için en yeni **tüketilmemiş** yedeği okur ve **önceki
  durumu gerçekten kurar** (yalnız deftere not düşmez). Tüketilen yedek bir
  daha kullanılmaz.
- `sweep startup rollback "<id|ad>"` girdiye özel yedeği geri alır;
  `sweep startup rollback --last` en son değişikliği geri alır.
- GUI'de listedeki bir öğe için **Geri al**, denetim geçmişindeki bir satır
  için de **Geri al** düğmesi vardır. Geçmiş ekranında ayrıca
  **Son değişikliği geri al** bulunur.
- Yedek bulunamazsa geri alma "no backup found" hatası verir; sessizce
  başarılı sayılmaz.

Doğrulama tutmazsa işlem `ok` değil **`unverified`** olarak kaydedilir — yani
Sweep, "uyguladım ama emin değilim" durumunu gizlemez.

## Denetim defteri ve yedekler nerede?

Veri dizini: `<yapılandırma>/sweep/autostart`

- Windows: `%APPDATA%\sweep\autostart`
- Linux: `~/.config/sweep/autostart`
- macOS: `~/Library/Application Support/sweep/autostart`
- Taşınabilir kip (`--portable <dizin>`): `<dizin>/autostart`

Bu dizinde:

- `audit.jsonl` — denetim defteri; her denenen işlem (listeleme dâhil) için tek
  satır JSON. **Yalnız eklenir** (append-only). Bozuk/yarım son satır sessizce
  atlanır.
- `backups/<UTC-zaman>-<güvenli-kimlik>.json` — değişiklik öncesi durum; geri
  alma bu dosyaları okur.

Defter satırları `ts`, `user`, `op`, `id`, `name`, `source`, `location`,
`before`, `after`, `result`, `backup`, `detail` alanlarını taşır. `result`
değerleri: `ok`, `failed`, `skipped`, `unverified`.

GUI'nin **Geçmiş** sekmesi defteri en yeni üstte, varsayılan olarak son 100
satır gösterir; **JSON** veya **CSV** olarak dışa aktarabilir. Dışa aktarma
göreli bir dosya adı verilirse Belgeler klasörüne yazılır.

```sh
sweep startup history --limit 0 --json                    # defteri bas
sweep startup history --to ~/audit.json --format json     # dosyaya yaz
sweep startup history --to ~/audit.csv  --format csv
```

## Yönetici hakları

Şu işlemler yönetici (Windows'ta yükseltilmiş, Linux'ta root) gerektirir; normal
kullanıcıda başarısız olur ve hata mesajı gösterilir:

- Makine geneli girdiler: `HKLM\…\Run`, sistem `Startup` klasörü, `Winlogon`,
  ilke anahtarları.
- Sistem görevleri (`\Microsoft\…` dâhil) ve sistem cron/systemd birimleri.
- Yönetici hesabı altında kayıtlı görevlerin değiştirilmesi.

Kullanıcı kapsamlı girdiler (`HKCU\…`, kullanıcı `Startup` klasörü, XDG
autostart, kullanıcı görevleri) yükseltme gerektirmez.

## Risk rozetleri ne anlama gelir?

Sınıflandırma ad + komut + konumdan yapılır; **ilk eşleşen kural kazanır**:

| Rozet | Anlamı | Örnek |
| --- | --- | --- |
| **Kritik** (`critical`) | OS oturum açma mekanizması; düzenlenmesi/kaldırılması sistemi açılışta bozabilir | `Winlogon`, ilke `Run` |
| **Yüksek** (`high`) | Güvenlik yazılımı ya da işletim sistemi bileşeni; kapatmak korumayı azaltır veya bir işlevi bozar | Defender, `\Microsoft\…` sistem görevi |
| **Orta** (`medium`) | Sürücü yardımcısı ya da tanınmayan girdi | ekran kartı/ses yardımcısı, bilinmeyen araç |
| **Düşük** (`low`) | Bilinen üçüncü taraf ya da güncelleyici | Spotify, GoogleUpdate |

Tanınmayan bir girdi **asla düşük risk sayılmaz**; en az `medium` olur ve
"review before disabling" notu taşır.

GUI'de `critical` bir öğede onay penceresi ek bir uyarı gösterir ve **Riski
anladım** kutusu işaretlenene kadar onay düğmesi kilitlidir; onay düğmesi dolu
vurgu yerine yalnız kırmızı çerçeveyle çizilir (kasten caydırıcı).

## Sweep'in bilerek dokunmadıkları

- **`Winlogon` ve ilke `Run` girdileri**: salt okunur; değiştirilemez,
  devre dışı bırakılamaz, kaldırılamaz.
- **Korumalı Microsoft OS görevleri** (`\Microsoft\…` yolundaki ya da Microsoft
  imzalı): varsayılan olarak düzenlenemez ve kaldırılamaz. Yalnız `--force`
  ile ve **her hâlükârda deftere kaydedilerek** ele alınır.
- **systemd birimleri, cron satırları ve kabuk başlangıç dosyaları**: keşfedilir
  ve listelenir, ama değiştirilmez (`reversible=false`).
- **Kimliği belirsiz hedefler**: ad birden çok girdiyle eşleşirse Sweep tahmin
  etmez; `ambiguous` hatası verir ve kimlik ister.
- Bir şeyi **silmeden önce yedek yazılamıyorsa** işlem hiç yapılmaz.

## GUI ekranı

Kenar çubuğunda **Başlangıç ve Görevler** yolundan açılır (mevcut **Başlangıç**
ekranı ayrıca durur). Ekran; birleşik listeyi, arama/filtre/sıralama
araçlarını, seçili öğe için ayrıntı panelini (tam komut, konum, tetikleyici,
yayıncı, etki gerekçesi, notlar ve geri alınabilir/düzenlenebilir/kaldırılabilir
bilgileri), toplu etkinleştirme/devre dışı bırakmayı, sistem değiştiren her
işlem öncesi onay penceresini ve denetim geçmişini içerir. Sistem çağrıları
`--quiet --json` ile yapılır ve QML ham komut satırı kurmaz.
