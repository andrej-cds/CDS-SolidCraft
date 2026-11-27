#!/usr/bin/env python3
"""CDS fork: the app icon on a black plate with a gold border, as the other CDS program icons.

Reads mark-gold-512.png (the gold SolidCraft mark) and ../../../branding/cds/cds-letters-mask.png
(the CDS lettering as an alpha mask), writes every export: Linux PNGs, Windows .ico files, the
macOS .icns, the web icon, and app_icon_512.rgba (what app_icon.rs draws the window icon from).
Run from the repository root: python3 assets/icons/app/shared/source/make_cds_icon.py
"""
import base64, io, os
from PIL import Image, ImageDraw

ROOT = os.getcwd()
SRC = os.path.join(ROOT, "assets/icons/app/shared/source")
GOLD = (200, 169, 110)  # #C8A96E
OUT = os.path.join(ROOT, "assets/icons/app")


def build(n=1024):
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    r, mar = int(n * 0.225), int(n * 0.02)
    d.rounded_rectangle([mar, mar, n - mar - 1, n - mar - 1], r, fill=(10, 10, 10, 255))
    inset, bw = int(n * 0.05), int(n * 0.028)
    d.rounded_rectangle([inset, inset, n - inset - 1, n - inset - 1], int(r * 0.82), outline=GOLD + (255,), width=bw)
    mk = Image.open(os.path.join(SRC, "mark-gold-512.png")).convert("RGBA")
    mk = mk.crop(mk.getbbox())
    mh = int(n * 0.47)
    mw = int(mk.width * mh / mk.height)
    img.alpha_composite(mk.resize((mw, mh), Image.LANCZOS), ((n - mw) // 2, int(n * 0.14)))
    m = Image.open(os.path.join(ROOT, "assets/branding/cds/cds-letters-mask.png")).convert("L")
    lw = int(n * 0.40)
    lh = int(m.height * lw / m.width)
    col = Image.new("RGBA", (lw, lh), GOLD + (255,))
    col.putalpha(m.resize((lw, lh), Image.LANCZOS))
    img.alpha_composite(col, ((n - lw) // 2, int(n * 0.685)))
    return img


big = build(1024)
sz = lambda s: big.resize((s, s), Image.LANCZOS)

for s in (16, 24, 32, 48, 64, 128, 256, 512):
    p = f"{OUT}/linux/exports/hicolor/{s}x{s}/apps/io.tangencylabs.solidcraft.png"
    if os.path.exists(os.path.dirname(p)):
        sz(s).save(p, optimize=True)
for name in ("solidcraft.ico", "solidcraft-executable.ico"):
    big.resize((256, 256), Image.LANCZOS).save(f"{OUT}/windows/exports/{name}", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
big.save(f"{OUT}/macos/exports/icon.icns")
# iOS rounds the corners itself: a full-bleed black square.
tile = Image.new("RGBA", (180, 180), (10, 10, 10, 255))
tile.alpha_composite(sz(180))
tile.convert("RGB").save(f"{OUT}/web/apple-touch-icon.png", optimize=True)
buf = io.BytesIO()
sz(256).save(buf, "PNG", optimize=True)
open(f"{OUT}/web/solidcraft-icon.svg", "w").write(
    '<?xml version="1.0" encoding="UTF-8"?>\n'
    '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="256" height="256" viewBox="0 0 256 256" role="img" aria-label="SolidCraft by CDS">\n'
    f'  <image width="256" height="256" xlink:href="data:image/png;base64,{base64.b64encode(buf.getvalue()).decode()}"/>\n'
    "</svg>\n"
)
open(f"{SRC}/app_icon_512.rgba", "wb").write(sz(512).tobytes())
print("icons written")
