#!/usr/bin/env python3
"""Turn the Sweep artwork into the icon assets the project ships.

The source (`Sweep-Logo.jpg` at the repo root) was exported with a
*transparency checkerboard* baked into the pixels: a JPEG cannot store an
alpha channel, so the "transparent" area became a grey checkerboard with a
teal glow around the icon. Feeding that straight into an icon build produces
a grey, square-cornered blob, so this script:

  1. classifies every pixel as icon (dark navy surface / orange stroke) or
     background (neutral grey checkerboard / teal glow),
  2. crops to the icon's bounding box,
  3. replaces the background RGB with the icon's own surface colour, so an
     antialiased edge pixel fades to navy instead of to grey, and
  4. writes a high-resolution PNG for the QML GUI plus a multi-size Windows
     `.ico` for the executable and installer.

Usage:
    python3 make-ico.py                       # both assets, default paths
    python3 make-ico.py --ico out.ico --png out.png
    python3 make-ico.py --source ../../Sweep-Logo.jpg
    python3 make-ico.py --preview             # also write a check sheet

Requires Pillow:  pip install pillow
"""

from __future__ import annotations

import argparse
import os
import sys

try:
    from PIL import Image, ImageDraw, ImageFilter
except ImportError:  # pragma: no cover - environment guard
    print("Pillow gerekli / Pillow required:  pip install pillow", file=sys.stderr)
    raise SystemExit(1)

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))  # packaging/windows -> repo root

DEFAULT_SOURCE = os.path.join(REPO, "Sweep-Logo.jpg")
DEFAULT_ICO = os.path.join(HERE, "sweep.ico")
DEFAULT_PNG = os.path.join(REPO, "packaging", "qml", "qml", "assets", "sweep-logo.png")

# Sizes Windows actually asks for (Explorer tiles, taskbar, Alt-Tab, jumbo).
ICO_SIZES = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]

# Master resolution: the PNG is written at 512 and every ICO size is derived
# from one clean master, so no size is an upscale of another.
MASTER = 1024
PNG_SIZE = 512

# Corner radius as a fraction of the side. The artwork's corner is a
# superellipse ("squircle"); this circular arc is chosen to sit just inside
# it, so the mask never re-exposes the glow that surrounds the source icon.
CORNER_FRACTION = 0.18


def _is_icon(pixel: tuple[int, int, int]) -> bool:
    """Icon pixels are the flat navy plate or the orange stroke.

    Two traps make a naive test fail: the checkerboard is a *neutral* grey
    (red == blue), and where the teal glow meets a dark checkerboard square it
    produces a dark teal that also looks dark and blue-ish. The plate is
    darker and greener-suppressed than either, so the bounds below admit the
    artwork and reject both the checkerboard and the glow.
    """
    r, g, b = pixel
    navy_plate = max(r, g, b) < 60 and b > r and g < 50
    orange_stroke = r > 110 and r > b + 25
    return navy_plate or orange_stroke


def _scan(img: Image.Image) -> tuple[int, int, int, int]:
    """Find the solid square the icon sits on.

    The glow and the checkerboard sit *outside* the icon, and JPEG noise makes
    a few of those pixels look icon-like, so a plain bounding box would grow a
    halo. Instead, count icon pixels per row and per column and keep the
    contiguous band around the peak: the solid square dominates its rows and
    columns, while stray halo pixels never reach a third of the peak count.
    """
    w, h = img.size
    px = img.load()
    rows = [0] * h
    cols = [0] * w
    for y in range(h):
        for x in range(w):
            if _is_icon(px[x, y]):
                rows[y] += 1
                cols[x] += 1

    def band(counts: list[int]) -> tuple[int, int]:
        peak = max(counts)
        if peak == 0:
            raise SystemExit("no icon found in the source image")
        threshold = peak // 3
        hit = [i for i, n in enumerate(counts) if n >= threshold]
        return hit[0], hit[-1]

    top, bottom = band(rows)
    left, right = band(cols)
    return left, top, right, bottom


def _surface_colour(img: Image.Image, box: tuple[int, int, int, int]) -> tuple[int, int, int]:
    """Colour of the icon surface, sampled well inside the rounded corners."""
    left, top, right, bottom = box
    side = min(right - left + 1, bottom - top + 1)
    inset = max(2, int(side * 0.18))
    picks = [
        (left + inset, top + inset),
        (right - inset, top + inset),
        (left + inset, bottom - inset),
        (right - inset, bottom - inset),
    ]
    samples = [img.getpixel(p) for p in picks]
    samples = [s for s in samples if max(s) < 110] or samples
    samples.sort(key=lambda p: sum(p))
    return samples[len(samples) // 2]


def build_master(source: str) -> Image.Image:
    """Return a square RGBA master image with the checkerboard removed."""
    img = Image.open(source).convert("RGB")
    left, top, right, bottom = _scan(img)
    surface = _surface_colour(img, (left, top, right, bottom))

    side = max(right - left + 1, bottom - top + 1)
    w, h = img.size
    box = (
        max(0, left),
        max(0, top),
        min(w, left + side),
        min(h, top + side),
    )
    crop = img.crop(box)
    cw, ch = crop.size
    cpx = crop.load()

    # Rebuild the RGB: background pixels carry the surface colour, so an
    # antialiased edge pixel fades to navy instead of to grey checkerboard.
    flat = Image.new("RGB", crop.size, surface)
    fpx = flat.load()
    for y in range(ch):
        for x in range(cw):
            p = cpx[x, y]
            if _is_icon(p):
                fpx[x, y] = p

    flat = flat.resize((MASTER, MASTER), Image.LANCZOS)

    # The silhouette is drawn geometrically rather than traced from the
    # pixels: the source's glow would otherwise leak into the alpha. Drawing
    # at 4x and downsampling gives a clean antialiased edge.
    ss = 4
    big = Image.new("L", (MASTER * ss, MASTER * ss), 0)
    ImageDraw.Draw(big).rounded_rectangle(
        (0, 0, MASTER * ss - 1, MASTER * ss - 1),
        radius=int(MASTER * ss * CORNER_FRACTION),
        fill=255,
    )
    mask = big.resize((MASTER, MASTER), Image.LANCZOS)

    flat.putalpha(mask)
    return flat


def write_preview(master: Image.Image, path: str) -> None:
    """A check sheet: the master on a light and a dark background."""
    tiles = [(256, (245, 245, 248)), (256, (18, 18, 22)), (64, (245, 245, 248)), (32, (18, 18, 22))]
    pad = 16
    width = sum(t[0] for t in tiles) + pad * (len(tiles) + 1)
    height = max(t[0] for t in tiles) + pad * 2
    sheet = Image.new("RGB", (width, height), (128, 128, 128))
    x = pad
    for size, bg in tiles:
        cell = Image.new("RGB", (size, size), bg)
        sheet.paste(cell, (x, pad))
        sheet.paste(master.resize((size, size), Image.LANCZOS), (x, pad), master.resize((size, size), Image.LANCZOS))
        x += size + pad
    sheet.save(path)
    print(f"OK  png  {path} (preview)")


def main() -> int:
    ap = argparse.ArgumentParser(description="Build the Sweep icon assets.")
    ap.add_argument("--source", default=DEFAULT_SOURCE, help="source artwork")
    ap.add_argument("--ico", default=DEFAULT_ICO, help="Windows .ico destination")
    ap.add_argument("--png", default=DEFAULT_PNG, help="GUI .png destination")
    ap.add_argument("--preview", action="store_true", help="also write a check sheet")
    ap.add_argument("--skip-ico", action="store_true")
    ap.add_argument("--skip-png", action="store_true")
    args = ap.parse_args()

    if not os.path.isfile(args.source):
        print(f"source not found: {args.source}", file=sys.stderr)
        print("put Sweep-Logo.jpg at the repo root (or pass --source)", file=sys.stderr)
        return 1

    master = build_master(args.source)

    if not args.skip_png:
        parent = os.path.dirname(args.png)
        if parent:
            os.makedirs(parent, exist_ok=True)
        master.resize((PNG_SIZE, PNG_SIZE), Image.LANCZOS).save(args.png, optimize=True)
        print(f"OK  png  {args.png} ({os.path.getsize(args.png)} bytes)")

    if not args.skip_ico:
        parent = os.path.dirname(args.ico)
        if parent:
            os.makedirs(parent, exist_ok=True)
        master.save(args.ico, sizes=ICO_SIZES)
        print(f"OK  ico  {args.ico} ({os.path.getsize(args.ico)} bytes)")

    if args.preview:
        write_preview(master, os.path.join(HERE, "sweep-icon-preview.png"))

    return 0


if __name__ == "__main__":
    sys.exit(main())
