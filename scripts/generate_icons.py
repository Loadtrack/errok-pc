from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri" / "icons"
OUT.mkdir(parents=True, exist_ok=True)


def make_icon(size: int) -> Image.Image:
    scale = size / 256
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle((4 * scale, 4 * scale, 252 * scale, 252 * scale), radius=62 * scale, fill="#6c5ce7")
    draw.rounded_rectangle((57 * scale, 62 * scale, 199 * scale, 202 * scale), radius=20 * scale, fill="#ffffff")
    draw.rectangle((57 * scale, 89 * scale, 199 * scale, 108 * scale), fill="#6c5ce7")
    draw.rounded_rectangle((78 * scale, 46 * scale, 94 * scale, 78 * scale), radius=8 * scale, fill="#ffffff")
    draw.rounded_rectangle((162 * scale, 46 * scale, 178 * scale, 78 * scale), radius=8 * scale, fill="#ffffff")
    for x, y in [(82, 132), (128, 132), (82, 166), (128, 166), (174, 132)]:
        draw.rounded_rectangle(((x - 9) * scale, (y - 7) * scale, (x + 9) * scale, (y + 7) * scale), radius=5 * scale, fill="#6c5ce7")
    return image


for size, name in [(32, "32x32.png"), (128, "128x128.png"), (256, "128x128@2x.png")]:
    make_icon(size).save(OUT / name, optimize=True)
make_icon(256).save(OUT / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
print(f"Значки сохранены: {OUT}")
