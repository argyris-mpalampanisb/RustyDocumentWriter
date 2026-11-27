"""Crop the Blender renders to the shape, fade the floor shadow toward the edges, downscale.

    python3 tools/shapes/finish.py <render_dir> assets/shapes
"""

import math
import sys
from pathlib import Path

from PIL import Image

SIZE = 384


def finish(src: Path, dst: Path) -> None:
    im = Image.open(src).convert("RGBA")
    w, h = im.size
    a = im.getchannel("A")
    # The shape itself is opaque; the shadow catcher is translucent black.
    box = a.point(lambda v: 255 if v >= 250 else 0).getbbox()
    x0, y0, x1, y1 = box
    bw, bh = x1 - x0, y1 - y0
    m = max(bw, bh)
    side, top, bottom = int(m * 0.10), int(m * 0.05), int(m * 0.16)
    crop = (max(0, x0 - side), max(0, y0 - top), min(w, x1 + side), min(h, y1 + bottom))
    px = im.load()
    # Fade the translucent shadow with an elliptical falloff centred on the shape's base,
    # reaching zero inside the crop so no straight edges show.
    cx, cy = (x0 + x1) / 2, y1 - bh * 0.1
    rx = (crop[2] - crop[0]) / 2
    ry_up, ry_down = cy - crop[1], crop[3] - cy
    for y in range(crop[1], crop[3]):
        for x in range(crop[0], crop[2]):
            r, g, b, al = px[x, y]
            if al >= 250:
                continue
            ry = ry_up if y < cy else ry_down
            d = math.hypot((x - cx) / rx, (y - cy) / ry)
            fade = 1.0 if d < 0.55 else max(0.0, 1.0 - (d - 0.55) / 0.4) ** 2
            if (r + g + b) < 30:  # shadow, not the shape's anti-aliased edge
                al = int(al * 0.85 * fade)
            px[x, y] = (r, g, b, al)
    out = im.crop(crop)
    out.thumbnail((SIZE, SIZE), Image.LANCZOS)
    out.save(dst, optimize=True)
    print(f"{dst.name}: {out.size[0]}x{out.size[1]}, {dst.stat().st_size // 1024} KiB")


if __name__ == "__main__":
    src_dir, dst_dir = Path(sys.argv[1]), Path(sys.argv[2])
    dst_dir.mkdir(parents=True, exist_ok=True)
    for p in sorted(src_dir.glob("*.png")):
        finish(p, dst_dir / p.name)
