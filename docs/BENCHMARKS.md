# Benchmarks

Ortam: WSL (Ubuntu), 8 CPU, 15 GB RAM, release profili (`strip` + `thin LTO`).
İkili boyutu: **6.0 MB** (tek dosya, statik).

## Sentetik ağaç (16.000 dosya, 63 MB, 400 proje)

| Komut | Süre | Not |
| --- | --- | --- |
| `devscan /tmp/speed` | 0.21 sn | 400 cruft dizini bulundu |
| `bigfiles --top 10` | 0.14 sn | sıralı |
| `deepscan --preview` | 0.18 sn | 8000 eşleşme |
| `dupes` | 0.68 sn | 15.998 yinelenen, SipHash doğrulamalı |

## Gerçek sistem

| Komut | Süre | Not |
| --- | --- | --- |
| `preview --all` | 4.85 sn | `/usr` taraması + 12.611 localization dahil |
| `list` | 0.42 sn | ilk çalıştırma (önbelleksiz) |
| `--help` | 0.005 sn | anlık açılış |

## Yorum

- BleachBit (Python) aynı `/usr` taramasını onlarca saniyede bitirir; paralel
  `rayon` worker'lar ve blok-tabanlı boyut hesabı farkı yaratır.
- `dupes` iki fazlı elemeyle (önce boyut, sonra özet) 16 bin dosyayı 1 sn altında işler.
- Tekrar üretmek için: `cargo build --release`, yukarıdaki sentetik ağaç
  betiğiyle (`docs/` dışındaki betik bu dosyada saklı değildir; 200 modül ×
  40 dosya + node_modules/target iskeleti kurun).
