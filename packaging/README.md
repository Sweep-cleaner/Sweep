# Paketleme

## Windows .exe'yi Linux'ta derleme (MinGW cross, rootsuz)

```sh
rustup target add x86_64-pc-windows-gnu
apt-get download gcc-mingw-w64-x86-64-posix g++-mingw-w64-x86-64-posix \
  binutils-mingw-w64-x86-64 mingw-w64-common mingw-w64-x86-64-dev gcc-mingw-w64-base
for d in *.deb; do dpkg-deb -x "$d" ~/mingw/; done
ln -sf x86_64-w64-mingw32-gcc-*-posix ~/mingw/usr/bin/x86_64-w64-mingw32-gcc
export PATH="$HOME/mingw/usr/bin:$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=$HOME/mingw/usr/bin/x86_64-w64-mingw32-gcc
cargo build --release --target x86_64-pc-windows-gnu
# -> target/x86_64-pc-windows-gnu/release/sweep.exe
```

CLI varsayılan derlemede hafiftir (`cargo build --release` yalnızca `sweep`
üretir). Grafiksel arayüz, `--json` CLI üzerine kurulu Qt6/QML kabuktur;
bkz. `packaging/qml/README.md`.

## Arch (AUR)

`packaging/aur/PKGBUILD` dosyasını AUR'a `sweep` adıyla gönderin.
`pkgver` ve `source` satırındaki `sweep-cleaner/sweep` yer tutucusunu
gerçek GitHub konumuyla değiştirin.

## Fedora / RHEL (COPR)

```sh
copr-cli build @sweep/sweep packaging/copr/sweep.spec
```

## Debian / Ubuntu (PPA)

```sh
cd packaging  # veya kökten: debuild -us -uc
```

`packaging/debian/` kuralları `sweep` ikilisini ve systemd birimlerini kurar.

## Flatpak (Flathub)

```sh
flatpak-builder --user --install build packaging/flatpak/org.sweep.Sweep.json
```

Temizlik aracı sandbox dışını görmelidir; manifest `--filesystem=host-reset`
ister. `REPLACE_WITH_RELEASE_SHA256` alanını sürüm tarball özetiyle doldurun.

## Podman / Docker

```sh
podman build -t sweep -f packaging/podman/Containerfile .
podman run --rm sweep --help
```

Detaylar `packaging/podman/README.md` dosyasında (host dizini bağlama,
SELinux `:z` notu).

## Zamanlayıcı

Sistem geneli haftalık temizlik için `packaging/systemd/` birimleri paketlere
dahildir. Kullanıcı düzeyi için kurulum gerekmez:

```sh
sweep schedule --enable --hour 3 -- system.journal_vacuum apt.deep_cache
sweep schedule --disable
```
