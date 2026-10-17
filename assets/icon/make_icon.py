"""Draws Biblo's icon: an open book on the app's blue.

Writes biblo.ico (the .exe icon, every size Windows uses) and biblo.png
(256 px, the window icon). Only needed to change the icon:

    python3 make_icon.py        (needs Pillow: pip install pillow)
"""
from pathlib import Path

from PIL import Image, ImageDraw

BLUE = "#2F6FED"  # header colour (theme::BLUE)
COVER = "#1E4FA3"
PAGE = "#FFFFFF"
LINES = "#BFD3FA"
SUN = "#FFD166"  # bookmark (theme::SUN)

S = 1024  # drawn large, then scaled down: smooth edges at every size


def draw() -> Image.Image:
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle((32, 32, S - 32, S - 32), radius=200, fill=BLUE)
    # Cover, seen around the pages.
    d.polygon([(150, 330), (512, 400), (874, 330), (874, 790), (512, 860), (150, 790)], fill=COVER)
    # Pages, dipping towards the spine.
    d.polygon([(190, 290), (500, 360), (500, 790), (190, 720)], fill=PAGE)
    d.polygon([(834, 290), (524, 360), (524, 790), (834, 720)], fill=PAGE)
    # Lines of text.
    for i in range(4):
        y = 400 + i * 85
        d.line([(245, y), (450, y + 46)], fill=LINES, width=26)
        d.line([(779, y), (574, y + 46)], fill=LINES, width=26)
    # Bookmark hanging from the right page.
    d.polygon([(690, 250), (770, 250), (770, 470), (730, 430), (690, 470)], fill=SUN)
    return img


def main() -> None:
    here = Path(__file__).parent
    big = draw()
    big.resize((256, 256), Image.LANCZOS).save(here / "biblo.png")
    big.save(here / "biblo.ico", sizes=[(n, n) for n in (16, 24, 32, 48, 64, 128, 256)])


if __name__ == "__main__":
    main()
