#!/usr/bin/env python3
"""Paint the TunnelYard mark: a circular tunnel portal on a coral squircle.

The concept was explored with Grok ImageGen (`public/icon-master.png`), then
rebuilt geometrically here so every shipped size has true alpha, pixel-clean
curves and no raster fringe from the generated master.
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
CORAL = (242, 90, 42, 255)  # #F25A2A
WHITE = (255, 255, 255, 255)
MASTER = ROOT / "public" / "icon-master.png"


def squircle(size: int) -> Image.Image:
    """Flat coral badge with transparent corners and a comfortable safe area."""
    badge = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    inset = size * 0.055
    ImageDraw.Draw(badge).rounded_rectangle(
        (inset, inset, size - inset, size - inset),
        radius=size * 0.215,
        fill=CORAL,
    )
    return badge


def draw_mark(base: Image.Image) -> Image.Image:
    """Concentric portal: a thick white ring around a solid connection core."""
    s = base.size[0]
    layer = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    cx = cy = s / 2
    outer = s * 0.305
    hole = s * 0.215
    core = s * 0.125
    d.ellipse((cx - outer, cy - outer, cx + outer, cy + outer), fill=WHITE)
    d.ellipse((cx - hole, cy - hole, cx + hole, cy + hole), fill=CORAL)
    d.ellipse((cx - core, cy - core, cx + core, cy + core), fill=WHITE)
    return Image.alpha_composite(base, layer)


def _premul(im: Image.Image) -> Image.Image:
    r, g, b, a = im.split()
    return Image.merge(
        "RGBA",
        (
            ImageChops.multiply(r, a),
            ImageChops.multiply(g, a),
            ImageChops.multiply(b, a),
            a,
        ),
    )


def _unpremul(im: Image.Image) -> Image.Image:
    r, g, b, a = im.split()
    out = Image.new("RGBA", im.size)
    rp, gp, bp, ap, op = r.load(), g.load(), b.load(), a.load(), out.load()
    for y in range(im.size[1]):
        for x in range(im.size[0]):
            alpha = ap[x, y]
            if alpha == 0:
                op[x, y] = (0, 0, 0, 0)
            elif alpha == 255:
                op[x, y] = (rp[x, y], gp[x, y], bp[x, y], 255)
            else:
                op[x, y] = (
                    min(255, rp[x, y] * 255 // alpha),
                    min(255, gp[x, y] * 255 // alpha),
                    min(255, bp[x, y] * 255 // alpha),
                    alpha,
                )
    return out


def render(size: int) -> Image.Image:
    ss = 4 if size >= 32 else 6
    hi = draw_mark(squircle(size * ss))
    return _unpremul(_premul(hi).resize((size, size), Image.Resampling.LANCZOS))


def portal_svg() -> str:
    return """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" fill="none">
  <rect x="28" y="28" width="456" height="456" rx="110" fill="#f25a2a"/>
  <circle cx="256" cy="256" r="156" fill="#fff"/>
  <circle cx="256" cy="256" r="110" fill="#f25a2a"/>
  <circle cx="256" cy="256" r="64" fill="#fff"/>
</svg>
"""


def write_svg(path: Path) -> None:
    path.write_text(portal_svg(), encoding="utf-8")


def master_is_present() -> bool:
    return MASTER.is_file() and MASTER.stat().st_size > 32


def main() -> None:
    if not master_is_present():
        raise SystemExit(
            "missing public/icon-master.png — drop the generated 1:1 PNG there first"
        )

    public = ROOT / "public"
    build = ROOT / "build"
    public.mkdir(exist_ok=True)
    build.mkdir(exist_ok=True)

    sizes = {
        public / "icon.png": 256,
        public / "icon-64.png": 64,
        public / "icon-32.png": 32,
        build / "icon.png": 512,
    }
    for dest, size in sizes.items():
        im = render(size)
        im.save(dest, "PNG", optimize=True)
        print(f"wrote {dest.relative_to(ROOT)} {im.size}")

    ico_sizes = [16, 24, 32, 48, 64, 128, 256]
    ico_images = [render(n) for n in ico_sizes]
    ico_path = public / "icon.ico"
    ico_images[-1].save(
        ico_path,
        format="ICO",
        sizes=[(n, n) for n in ico_sizes],
        append_images=ico_images[:-1],
    )
    print(f"wrote {ico_path.relative_to(ROOT)}")
    write_svg(public / "icon.svg")
    print("wrote public/icon.svg")


if __name__ == "__main__":
    main()
