# Podman ile derleme ve çalıştırma

`packaging/podman/Containerfile` çok aşamalı derleme yapar: Rust 1.82'de
`sweep` CLI derlenir, `debian:bookworm-slim` üzerine tek binary konur.
Grafik arayüz için `packaging/qml/` altındaki Qt6 kabuğa bakın.

## Derle

```sh
# Proje kökünden:
podman build -t sweep -f packaging/podman/Containerfile .
```

## Çalıştır

```sh
podman run --rm sweep --help
podman run --rm sweep list
podman run --rm sweep preview --all
```

## Host dizinlerini temizleme

Konteyner kendi dosya sistemini görür; host yollarını açıkça bağla:

```sh
# Önizleme (salt okunur da olur):
podman run --rm -v "$HOME/Downloads:/data:ro" sweep bigfiles /data

# Gerçek temizlik (yazma izniyle):
podman run --rm -v "$HOME/Downloads:/data:rw" sweep clean --yes bigfiles
```

SELinux açık sistemlerde `:z` ekle: `-v "$HOME/Downloads:/data:rw,z"`.

> Dikkat: konteyner varsayılan olarak root çalışır. Bağladığın dizin
> dışında bir yere dokunmaz, ama `--yes` ile yıkıcı komutları ancak
> ne yaptığını bilerek çalıştır. Önce `preview`, sonra `clean`.

## Docker ile kullanım

Aynı dosya Docker ile de çalışır (`podman` yerine `docker` yazman yeterli).

## GUI ile çalıştırma (host ekranında)

Grafik arayüz Qt6/QML kabuktur (`packaging/qml/README.md`):

```sh
podman build -t sweep-qml -f packaging/qml/Containerfile.qml .
podman run --rm \
  -e DISPLAY="$DISPLAY" \
  -v /tmp/.X11-unix:/tmp/.X11-unix:ro \
  sweep-qml
```
