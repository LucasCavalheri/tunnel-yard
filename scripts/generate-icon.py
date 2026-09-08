#!/usr/bin/env python3
"""Paint the TunnelYard mark: two tunnel arches meeting at a small hub.

The concept was explored with OpenAI ImageGen, then rebuilt geometrically here
so every shipped size has true alpha, pixel-clean curves and no raster artifacts.
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
CORAL = (242, 90, 42, 255)  # #F25A2A
WHITE = (255, 255, 255, 255)


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


def cubic(
    p0: tuple[float, float],
    p1: tuple[float, float],
    p2: tuple[float, float],
    p3: tuple[float, float],
    steps: int = 28,
) -> list[tuple[float, float]]:
    points: list[tuple[float, float]] = []
    for i in range(steps + 1):
        t = i / steps
        u = 1.0 - t
        points.append(
            (
                u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1],
            )
        )
    return points


def draw_mark(base: Image.Image) -> Image.Image:
    """Two friendly tunnel arches sharing a small connection hub."""
    s = base.size[0]
    layer = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    left_foot = (s * 0.25, s * 0.70)
    left_shoulder = (s * 0.25, s * 0.46)
    hub = (s * 0.50, s * 0.57)
    right_shoulder = (s * 0.75, s * 0.46)
    right_foot = (s * 0.75, s * 0.70)
    stroke = max(int(s * 0.075), 2)

    path = [left_foot, left_shoulder]
    path += cubic(
        left_shoulder,
        (s * 0.25, s * 0.28),
        (s * 0.50, s * 0.28),
        hub,
    )[1:]
    path += cubic(
        hub,
        (s * 0.50, s * 0.28),
        (s * 0.75, s * 0.28),
        right_shoulder,
    )[1:]
    path.append(right_foot)
    d.line(path, fill=WHITE, width=stroke, joint="curve")
    r = stroke / 2
    for pt in (left_foot, right_foot):
        d.ellipse((pt[0] - r, pt[1] - r, pt[0] + r, pt[1] + r), fill=WHITE)
    # Ring-shaped hub remains distinct from the arches down to the 16 px copy.
    nr = s * 0.065
    d.ellipse(
        (hub[0] - nr, hub[1] - nr, hub[0] + nr, hub[1] + nr),
        fill=WHITE,
    )
    inner = s * 0.030
    d.ellipse(
        (hub[0] - inner, hub[1] - inner, hub[0] + inner, hub[1] + inner),
        fill=CORAL,
    )
    return Image.alpha_composite(base, layer)


def _premul(im: Image.Image) -> Image.Image:
    r, g, b, a = im.split()
    return Image.merge("RGBA", (ImageChops.multiply(r, a), ImageChops.multiply(g, a), ImageChops.multiply(b, a), a))


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


def write_svg(path: Path) -> None:
    path.write_text(
        """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" fill="none">
  <rect x="28" y="28" width="456" height="456" rx="110" fill="#f25a2a"/>
  <path d="M128 358 V236 C128 111 256 132 256 292 C256 132 384 111 384 236 V358"
        stroke="#fff" stroke-width="38" stroke-linecap="round" stroke-linejoin="round"/>
  <circle cx="256" cy="292" r="33" fill="#fff"/>
  <circle cx="256" cy="292" r="15" fill="#f25a2a"/>
</svg>
""",
        encoding="utf-8",
    )


def main() -> None:
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
    images: dict[int, Image.Image] = {}
    for dest, size in sizes.items():
        im = render(size)
        images[size] = im
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
