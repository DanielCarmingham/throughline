"""Outline-trace the Throughline logo PNG into a faithful, scalable SVG.

Unlike the centerline trace (which skeletonises the white ribbon and strokes
it), this follows the true boundary of each colour region and emits filled
paths — so varying stroke widths, the bevel and the real arrowhead shape all
survive.

Regions are traced CUMULATIVELY (silhouette, then body+ink, then ink) and
stacked back-to-front. Tracing them individually would leave hairline seams
where anti-aliased edges meet; stacking cannot.
"""

import numpy as np
from PIL import Image

SRC = "/Users/daniel/Developer/DanielCarmingham/throughline/brand/logo.png"
OUT = "/Users/daniel/Developer/DanielCarmingham/throughline/brand/logo.svg"

BODY = np.array([0x05, 0x29, 0x47])
RIM = np.array([0x05, 0xA1, 0xEA])
INK = np.array([0xFD, 0xFD, 0xFD])


def classify():
    im = Image.open(SRC).convert("RGBA")
    a = np.asarray(im).astype(np.int32)
    rgb, alpha = a[..., :3], a[..., 3]
    solid = alpha > 128

    d_body = ((rgb - BODY) ** 2).sum(-1)
    d_rim = ((rgb - RIM) ** 2).sum(-1)
    d_ink = ((rgb - INK) ** 2).sum(-1)
    nearest = np.argmin(np.stack([d_body, d_rim, d_ink]), axis=0)

    # Nearest-colour alone erodes the white ribbon: anti-aliased pixels on the
    # white/navy boundary are mid-grey and fall to the darker side. Reclaim
    # anything clearly light as ink so the ribbon keeps its true weight.
    lum = rgb.mean(-1)
    # ...but only for NEUTRAL brights. The rim's cyan highlight is also light,
    # and without a saturation guard it gets painted white.
    neutral = (rgb[..., 2] - rgb[..., 0]) < 60
    ink = solid & ((nearest == 2) | ((lum > 150) & neutral))

    return {
        "body": solid & (nearest == 0) & ~ink,
        "rim": solid & (nearest == 1) & ~ink,
        "ink": ink,
        "solid": solid,
    }


def loops(mask):
    """Trace the pixel boundary of a binary mask into closed loops.

    Each foreground pixel contributes an oriented unit edge for every side
    facing background. Oriented consistently, the edges stitch into closed
    rings without needing to know which are outers and which are holes —
    fill-rule evenodd sorts that out.
    """
    m = np.pad(mask, 1)
    ys, xs = np.nonzero(m)
    starts, ends = [], []

    up = ~m[ys - 1, xs]
    dn = ~m[ys + 1, xs]
    lf = ~m[ys, xs - 1]
    rt = ~m[ys, xs + 1]

    for sel, (sx, sy), (ex, ey) in (
        (up, (0, 0), (1, 0)),
        (rt, (1, 0), (1, 1)),
        (dn, (1, 1), (0, 1)),
        (lf, (0, 1), (0, 0)),
    ):
        x, y = xs[sel], ys[sel]
        starts.append(np.stack([x + sx, y + sy], 1))
        ends.append(np.stack([x + ex, y + ey], 1))

    S = np.concatenate(starts)
    E = np.concatenate(ends)

    nxt = {}
    for s, e in zip(map(tuple, S), map(tuple, E)):
        nxt.setdefault(s, []).append(e)

    out = []
    for origin in list(nxt):
        while nxt.get(origin):
            ring = [origin]
            cur = origin
            while True:
                opts = nxt.get(cur)
                if not opts:
                    break
                nx = opts.pop()
                if not opts:
                    del nxt[cur]
                ring.append(nx)
                cur = nx
                if cur == origin:
                    break
            if len(ring) > 8:
                out.append(ring)
    return out


def area(ring):
    p = np.array(ring, dtype=float)
    x, y = p[:, 0], p[:, 1]
    return 0.5 * abs(np.dot(x, np.roll(y, 1)) - np.dot(y, np.roll(x, 1)))


def rdp(pts, eps):
    """Iterative Douglas-Peucker (recursion blows the stack on big rings)."""
    keep = np.zeros(len(pts), bool)
    keep[0] = keep[-1] = True
    stack = [(0, len(pts) - 1)]
    P = np.array(pts, dtype=float)
    while stack:
        i, j = stack.pop()
        if j <= i + 1:
            continue
        seg = P[j] - P[i]
        n = np.hypot(*seg)
        rel = P[i + 1 : j] - P[i]
        if n == 0:
            d = np.hypot(rel[:, 0], rel[:, 1])
        else:
            d = np.abs(seg[0] * rel[:, 1] - seg[1] * rel[:, 0]) / n
        k = int(np.argmax(d))
        if d[k] > eps:
            k += i + 1
            keep[k] = True
            stack.append((i, k))
            stack.append((k, j))
    return P[keep]


def smooth_closed(P, tension=0.5):
    """Catmull-Rom through a closed ring, emitted as cubic beziers."""
    n = len(P)
    d = [f"M{P[0][0]:.2f} {P[0][1]:.2f}"]
    for i in range(n):
        p0 = P[(i - 1) % n]
        p1 = P[i]
        p2 = P[(i + 1) % n]
        p3 = P[(i + 2) % n]
        c1 = p1 + (p2 - p0) / 6 * tension * 2
        c2 = p2 - (p3 - p1) / 6 * tension * 2
        d.append(
            f"C{c1[0]:.2f} {c1[1]:.2f} {c2[0]:.2f} {c2[1]:.2f} {p2[0]:.2f} {p2[1]:.2f}"
        )
    d.append("Z")
    return "".join(d)


masks = classify()
print({k: int(v.sum()) for k, v in masks.items()})

# Back-to-front cumulative layers, mirroring how the mark is built.
layers = [
    ("rim", masks["solid"]),
    ("body", masks["body"] | masks["ink"]),
    ("ink", masks["ink"]),
]

ys, xs = np.nonzero(masks["solid"])
x0, x1 = xs.min() - 1, xs.max() + 2
y0, y1 = ys.min() - 1, ys.max() + 2
scale = 1000.0 / (x1 - x0)
vb_h = (y1 - y0) * scale

paths = []
for name, mask in layers:
    rings = loops(mask)
    rings = [r for r in rings if area(r) > 400]  # drop AA speckle
    rings.sort(key=area, reverse=True)
    d = []
    for ring in rings:
        P = rdp(ring, eps=1.0)
        if len(P) < 4:
            continue
        P = (P - np.array([x0, y0])) * scale
        d.append(smooth_closed(P))
    print(f"{name}: {len(rings)} rings, {sum(len(x) for x in d)} chars")
    paths.append((name, "".join(d)))

svg = [
    f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 {vb_h:.1f}" '
    f'role="img" aria-label="Throughline">',
    "  <title>Throughline</title>",
]
# Colours are baked in so the file renders standalone (favicon, README,
# social cards) but keep their classes so CSS can still recolour them inline.
FILL = {"rim": "#05a1ea", "body": "#052947", "ink": "#fdfdfd"}
for name, d in paths:
    svg.append(
        f'  <path class="tl-{name}" fill="{FILL[name]}" '
        f'fill-rule="evenodd" d="{d}"/>'
    )
svg.append("</svg>")

open(OUT, "w").write("\n".join(svg) + "\n")
print(f"wrote {OUT}  viewBox 0 0 1000 {vb_h:.1f}")
