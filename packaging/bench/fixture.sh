#!/bin/bash
# Deterministic junk fixture both tools clean from the SAME real paths.
#   /tmp/sweep-bench-*.tmp      300 files x 100KB, 10 days old  (~30MB)
#   ~/.cache/thumbnails/large   100 files x 500KB               (~50MB)
#   ~/.cache/thumbnails/normal  100 files x 50KB                (~5MB)
set -u
rm -rf /tmp/sweep-bench-*.tmp "$HOME/.cache/thumbnails"
mkdir -p "$HOME/.cache/thumbnails/large" "$HOME/.cache/thumbnails/normal"
for i in $(seq 1 300); do
    dd if=/dev/zero of="/tmp/sweep-bench-$i.tmp" bs=100K count=1 status=none
    touch -d '10 days ago' "/tmp/sweep-bench-$i.tmp"
done
for i in $(seq 1 100); do
    dd if=/dev/zero of="$HOME/.cache/thumbnails/large/$i.png" bs=500K count=1 status=none
    dd if=/dev/zero of="$HOME/.cache/thumbnails/normal/$i.png" bs=50K count=1 status=none
done
# Firefox profile cache + shared mozilla cache
mkdir -p "$HOME/.mozilla/firefox/test.default/cache2" "$HOME/.cache/mozilla/firefox"
for i in $(seq 1 20); do
    dd if=/dev/zero of="$HOME/.mozilla/firefox/test.default/cache2/$i.bin" bs=100K count=1 status=none
    dd if=/dev/zero of="$HOME/.cache/mozilla/firefox/$i.bin" bs=100K count=1 status=none
done
# Evolution + Chrome caches
mkdir -p "$HOME/.cache/evolution" "$HOME/.config/google-chrome/Default/Cache"
for i in $(seq 1 20); do
    dd if=/dev/zero of="$HOME/.cache/evolution/$i.bin" bs=100K count=1 status=none
    dd if=/dev/zero of="$HOME/.config/google-chrome/Default/Cache/$i.bin" bs=100K count=1 status=none
done
echo "fixture ready: 300 tmp + $(du -sh "$HOME/.cache/thumbnails" | cut -f1) thumbnails + ff/evo/chrome caches"
