#!/usr/bin/env python3
"""Deterministic generator for the macOS DMG background image.

Produces ``dmg-background.png`` next to this script:

  - Canvas 1320x800 px = 2x the 660x400 pt DMG window declared in
    ``tauri.conf.json`` (bundle.macOS.dmg.windowSize).
  - DPI metadata 144.07 (Pillow writes an integer pixels-per-meter pHYs
    chunk: 5672 ppm == 144.0688 dpi, the closest representable value),
    so Finder maps the image back to exactly 660x400 points -- sharp on
    Retina, no scrollbar-rounding artifact.
  - Mid-to-light neutral gradient wash. Single image, no dark variant:
    the tonal range keeps both black and white Finder label text legible.
  - Baked-in text: app name + one English instruction line, positioned in
    the y ~= 80-220 px title band (40-110 pt).
  - NOTHING is painted inside the +/-95 pt (190 px) clear discs around the
    two icon drop targets at (360, 380) px and (960, 380) px -- Finder draws
    the real Weakup.app icon and the Applications folder there. Only a
    chevron arrow in the 220 px-wide gap between the discs (x 550-770 at
    the icon row).
  - Every painted element stays >= 80 px (40 pt) from every canvas edge.

Deterministic: no randomness anywhere; run it twice, get identical bytes
(modulo zlib being deterministic, which it is). The committed PNG is
canonical; fonts fall back through a fixed list, so a different host may
shift glyph outlines slightly but never layout.

Usage:  python3 generate_dmg_background.py
"""

from __future__ import annotations

import os

from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
OUT_PATH = os.path.join(HERE, "dmg-background.png")

W, H = 1320, 800          # 2x of 660x400 pt
DPI = (144.07, 144.07)    # -> pHYs 5672 ppm

# Icon drop-target centres in px (tauri.conf.json appPosition /
# applicationFolderPosition, times 2) and the mandatory clear radius.
ICON_CENTERS = ((360, 380), (960, 380))
CLEAR_R = 190             # +/- 95 pt

MARGIN = 80               # >= 40 pt from every edge

# Fixed font preference list (macOS host). First hit wins.
FONT_CANDIDATES_BOLD = (
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
)
FONT_CANDIDATES_REGULAR = (
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
)

TITLE_TEXT = "Weakup"
SUBTITLE_TEXT = "Drag to Applications"


def load_font(candidates: tuple[str, ...], size: int, bold_index: int | None = None):
    """Load the first available font; .ttc collections take a face index."""
    for path in candidates:
        if os.path.exists(path):
            try:
                index = bold_index if (path.endswith(".ttc") and bold_index is not None) else 0
                return ImageFont.truetype(path, size, index=index)
            except OSError:
                continue
    return ImageFont.load_default()


def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def gradient_row(y: int) -> tuple[int, int, int]:
    """Piecewise-linear vertical gradient: lighter at top and bottom,
    deepest at the icon row so white Finder labels there have contrast."""
    stops = (
        (0,   (207, 211, 217)),   # light
        (260, (169, 176, 186)),
        (400, (152, 160, 171)),   # deepest, icon row
        (560, (166, 173, 183)),
        (800, (194, 199, 206)),   # light again
    )
    for (y0, c0), (y1, c1) in zip(stops, stops[1:]):
        if y <= y1:
            t = (y - y0) / (y1 - y0)
            return (
                int(round(lerp(c0[0], c1[0], t))),
                int(round(lerp(c0[1], c1[1], t))),
                int(round(lerp(c0[2], c1[2], t))),
            )
    return stops[-1][1]


def build_background() -> Image.Image:
    img = Image.new("RGB", (W, H))
    px = img.load()
    for y in range(H):
        row = gradient_row(y)
        for x in range(W):
            px[x, y] = row

    # Soft radial highlight behind the title band (deterministic geometry,
    # feathered with a Gaussian blur). Keeps the headline area airy.
    hl = Image.new("L", (W, H), 0)
    hd = ImageDraw.Draw(hl)
    hd.ellipse((160, -260, W - 160, 420), fill=46)  # +18/255 max lift, faded
    hl = hl.filter(ImageFilter.GaussianBlur(120))
    img = Image.composite(Image.new("RGB", (W, H), (255, 255, 255)), img, hl)

    # Matching whisper of shade under the icon row: helps white labels.
    sh = Image.new("L", (W, H), 0)
    sd = ImageDraw.Draw(sh)
    sd.ellipse((120, 240, W - 120, 640), fill=22)
    sh = sh.filter(ImageFilter.GaussianBlur(140))
    img = Image.composite(Image.new("RGB", (W, H), (58, 64, 72)), img, sh)

    return img


def draw_haloed_text(
    base: Image.Image,
    xy: tuple[int, int],
    text: str,
    font,
    fill: tuple[int, int, int],
    anchor: str = "mm",
) -> None:
    """Dark ink over a mid-tone field gets a faint light drop-shadow so the
    baked-in text pops regardless of which gradient stop is underneath."""
    overlay = Image.new("RGBA", base.size, (0, 0, 0, 0))
    od = ImageDraw.Draw(overlay)
    x, y = xy
    od.text((x + 2, y + 4), text, font=font, fill=(255, 255, 255, 150), anchor=anchor)
    od.text((x, y), text, font=font, fill=fill + (255,), anchor=anchor)
    base.paste(overlay, (0, 0), overlay)


def in_clear_zone(x: float, y: float, pad: float = 0.0) -> bool:
    """True if a point at (x, y) lies inside any icon clear disc (+pad)."""
    for cx, cy in ICON_CENTERS:
        if (x - cx) ** 2 + (y - cy) ** 2 <= (CLEAR_R + pad) ** 2:
            return True
    return False


def draw_decorations(img: Image.Image) -> None:
    d = ImageDraw.Draw(img, "RGBA")

    # Chevron between the two drop targets, inside the guaranteed-clear
    # corridor x 550..770 at the icon row (both discs end/start there).
    ax0, ay = 636, 332
    pts_chevron = [(ax0, ay), (ax0 + 48, 380), (ax0, 380 + 48)]
    assert not any(in_clear_zone(px_, py_, pad=6) for px_, py_ in pts_chevron)
    d.line(pts_chevron, fill=(58, 64, 72, 80), width=14, joint="curve")
    d.line([(x - 2, y - 2) for x, y in pts_chevron], fill=(255, 255, 255, 165),
           width=12, joint="curve")

    # Corner ticks, one per corner, all >= 80 px from every edge.
    tick = 44
    inset = 108
    w_edge, h_edge = W - inset, H - inset
    for cx, cy, sx, sy in (
        (inset, inset, 1, 1),
        (w_edge, inset, -1, 1),
        (inset, h_edge, 1, -1),
        (w_edge, h_edge, -1, -1),
    ):
        d.line([(cx, cy), (cx + sx * tick, cy + sy * tick)],
               fill=(255, 255, 255, 95), width=8)
        d.line([(cx + sx * 10, cy - sy * 10),
                (cx + sx * (tick + 10), cy - sy * (tick + 10))],
               fill=(58, 64, 72, 55), width=8)

    # Three quiet dots, bottom centre. Lowest pixel: 709 -> 91 px margin.
    for i, dx in enumerate((-30, 0, 30)):
        r = 9 if i == 0 else 7  # first dot slightly emphasized
        alpha = 135 if i == 0 else 95
        d.ellipse((660 + dx - r, 700 - r, 660 + dx + r, 700 + r),
                  fill=(255, 255, 255, alpha))


def main() -> None:
    img = build_background()
    draw_decorations(img)

    title_font = load_font(FONT_CANDIDATES_BOLD, 96, bold_index=1)
    sub_font = load_font(FONT_CANDIDATES_REGULAR, 38)

    # Title band y ~= 80-220 px (40-110 pt), horizontally centred.
    # Both blocks stay within x ~460..860 -- the clear discs at those rows
    # span x 268..452 and 868..1052, so nothing encroaches on them.
    draw_haloed_text(img, (660, 132), TITLE_TEXT, title_font, (35, 39, 46))
    draw_haloed_text(img, (660, 214), SUBTITLE_TEXT, sub_font, (58, 64, 73))

    rgb = img.convert("RGB")
    rgb.save(OUT_PATH, format="PNG", dpi=DPI, optimize=True)
    print(f"wrote {OUT_PATH} ({W}x{H}, dpi {DPI})")


if __name__ == "__main__":
    main()
