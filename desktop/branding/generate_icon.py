#!/usr/bin/env python3
"""Draws Weakup's app icon.

A script rather than a checked-in drawing, because the icon has to be *regenerable*: the
same mark is needed at 16 px in a menu bar and at 1024 px in the App Store, and hand-edited
exports of the two drift apart. Everything below is derived from one canvas size, so the
proportions hold at every output size `cargo tauri icon` produces from it.

    python3 generate_icon.py && cd ../src-tauri && cargo tauri icon ../branding/app-icon.png

The mark is a power symbol. That is a deliberate choice over something showing both features:
the tray icon on macOS is the app icon shrunk to roughly 18 px, and at that size a
two-element mark turns to mush. A power glyph is the one symbol a user already reads
instantly, and it points at the half of this app that does something irreversible.
"""

import math
from pathlib import Path

from PIL import Image, ImageDraw

# Drawn large and downsampled, which is what gives smooth edges: PIL has no antialiased
# stroke, so supersampling is the antialiasing.
OUTPUT = 1024
SCALE = 4
SIZE = OUTPUT * SCALE

# The app's accent green, straight from styles.css --accent. The icon and the UI being the
# same colour is the whole point of picking it here.
GREEN = (31, 95, 63, 255)
WHITE = (255, 255, 255, 255)

# macOS leaves a transparent margin around the rounded square rather than filling the
# canvas; matching that keeps Weakup the same visual weight as its neighbours in the Dock.
MARGIN = 0.098
CORNER = 0.225

GLYPH_RADIUS = 0.255
STROKE = 0.072
GAP_DEGREES = 38.0
BAR_TOP = 1.34
BAR_BOTTOM = 0.06


def draw_icon() -> Image.Image:
    image = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)

    inset = MARGIN * SIZE
    plate = (inset, inset, SIZE - inset, SIZE - inset)
    draw.rounded_rectangle(plate, radius=CORNER * (SIZE - 2 * inset), fill=GREEN)

    centre = SIZE / 2
    radius = GLYPH_RADIUS * SIZE
    stroke = STROKE * SIZE

    # The arc, with a gap at the top for the bar to pass through.
    start = 270 + GAP_DEGREES
    draw.arc(
        (centre - radius, centre - radius, centre + radius, centre + radius),
        start=start,
        end=start + (360 - 2 * GAP_DEGREES),
        fill=WHITE,
        width=int(stroke),
    )

    # PIL strokes have flat ends. Discs at both ends of the arc round them off, so the gap
    # reads as intentional rather than as a broken circle.
    #
    # Centred on `radius - stroke / 2`, not on `radius`: PIL draws an arc's width *inward*
    # from the bounding box, so the band it paints spans radii [radius - stroke, radius] and
    # its centreline sits half a stroke inside the nominal radius. Placing the caps on
    # `radius` puts them half a stroke too far out, which reads as two knobs stuck on the
    # ends rather than as rounded caps.
    cap_radius = radius - stroke / 2
    for angle in (270 + GAP_DEGREES, 270 - GAP_DEGREES):
        x = centre + cap_radius * math.cos(math.radians(angle))
        y = centre + cap_radius * math.sin(math.radians(angle))
        draw.ellipse((x - stroke / 2, y - stroke / 2, x + stroke / 2, y + stroke / 2), fill=WHITE)

    # The vertical bar, overshooting the arc so the two read as one glyph.
    draw.rounded_rectangle(
        (
            centre - stroke / 2,
            centre - radius * BAR_TOP,
            centre + stroke / 2,
            centre - radius * BAR_BOTTOM,
        ),
        radius=stroke / 2,
        fill=WHITE,
    )

    return image.resize((OUTPUT, OUTPUT), Image.LANCZOS)


def main() -> None:
    target = Path(__file__).parent / "app-icon.png"
    draw_icon().save(target)
    print(f"wrote {target} at {OUTPUT}x{OUTPUT}")


if __name__ == "__main__":
    main()
