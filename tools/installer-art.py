"""Makes the Windows installer's images from the app icon and fonts: src-tauri/installer/header.bmp (150x57, shown
at the top of each page) and sidebar.bmp (164x314, the welcome and finish pages). NSIS wants 24-bit BMPs.

Run from the repository root after changing the icon:  python tools/installer-art.py
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
FONTS = ROOT / "node_modules" / "@fontsource"
OUT = ROOT / "src-tauri" / "installer"

INK = (10, 9, 18)  # near-black, the app's background
PURPLE = (178, 107, 255)
CYAN = (34, 211, 238)
MUTED = (150, 146, 170)
WHITE = (244, 242, 250)

S = 4  # drawn at 4x, then scaled down for clean edges


def font(family: str, weight: int, size: int) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(str(FONTS / family / "files" / f"{family}-latin-{weight}-normal.woff"), size * S)


def logo(size: int) -> Image.Image:
    return Image.open(ROOT / "src-tauri" / "icons" / "icon.png").convert("RGBA").resize((size * S, size * S), Image.LANCZOS)


def finish(im: Image.Image, w: int, h: int, name: str) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    im.resize((w, h), Image.LANCZOS).convert("RGB").save(OUT / name, "BMP")


def sidebar() -> None:
    w, h = 164, 314
    im = Image.new("RGBA", (w * S, h * S), INK)
    d = ImageDraw.Draw(im)
    mark = logo(84)
    im.alpha_composite(mark, ((w * S - mark.width) // 2, 56 * S))
    title = font("chakra-petch", 700, 23)
    d.text((w * S // 2, 160 * S), "ResinCraft", font=title, fill=WHITE, anchor="mm")
    d.text((w * S // 2, 182 * S), "LAUNCHER", font=font("chakra-petch", 600, 10), fill=CYAN, anchor="mm")
    # A short accent rule under the name, in the brand purple.
    d.rectangle(((w // 2 - 14) * S, 196 * S, (w // 2 + 14) * S, 198 * S), fill=PURPLE)
    d.text((w * S // 2, (h - 22) * S), "by ResinCore", font=font("ibm-plex-sans", 400, 10), fill=MUTED, anchor="mm")
    finish(im, w, h, "sidebar.bmp")


def header() -> None:
    # The page header is white in the installer, so the image is too; the logo sits on the right.
    w, h = 150, 57
    im = Image.new("RGBA", (w * S, h * S), (255, 255, 255))
    d = ImageDraw.Draw(im)
    mark = logo(34)
    x = w - 10 - 34
    im.alpha_composite(mark, (x * S, (h - 34) // 2 * S))
    d.text(((x - 6) * S, (h // 2 - 2) * S), "ResinCraft", font=font("chakra-petch", 700, 15), fill=INK, anchor="rs")
    d.text(((x - 6) * S, (h // 2 + 11) * S), "LAUNCHER", font=font("chakra-petch", 600, 8), fill=(120, 70, 200), anchor="rs")
    finish(im, w, h, "header.bmp")


if __name__ == "__main__":
    sidebar()
    header()
    print("wrote", OUT / "sidebar.bmp", "and", OUT / "header.bmp")
