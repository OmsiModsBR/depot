#!/bin/sh
# Render the application icons from assets/logos/pd-short-black.png: depot.icns (macOS, with the
# standard margin and rounded corners), depot.ico (Windows), the PNG window icons and the Android
# launcher icons. Needs Python 3 with Pillow.
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
python3 - <<'PY'
from PIL import Image, ImageDraw
L = Image.LANCZOS
icon = Image.open("assets/logos/pd-short-black.png").convert("RGB")
app = "assets/icons/app/"
icon.resize((256, 256), L).save(app + "depot-256.png", optimize=True)
icon.resize((512, 512), L).save(app + "depot-512.png", optimize=True)
icon.save(app + "depot.ico", sizes=[(s, s) for s in (256, 128, 64, 48, 32, 24, 16)])
big = icon.resize((824, 824), L).convert("RGBA")
mask = Image.new("L", (824, 824), 0)
ImageDraw.Draw(mask).rounded_rectangle((0, 0, 823, 823), radius=185, fill=255)
big.putalpha(mask)
canvas = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
canvas.paste(big, (100, 100), big)
canvas.save(app + "depot.icns")
for d, s in {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}.items():
    icon.resize((s, s), L).convert("RGBA").save(f"android/res/mipmap-{d}/ic_launcher.png", optimize=True)
PY
echo "icons written to assets/icons/app and android/res"
