"""Build both logo SVGs from brand/logo.png in ONE coordinate system.

Two traces come out of the same source:

  outline   — the true boundary of each colour region, as filled paths.
              Faithful, but a fill cannot be dash-animated.
  centerline— the white ribbon thinned to a one-pixel skeleton and fitted
              to beziers. Not faithful on its own, but it is an open path,
              so it CAN be dash-animated.

Emitting both against the same bbox and scale lets the centerline be used
as an animated mask that sweeps along the mark's real path, revealing the
faithful artwork underneath. That is how the hero gets both.

Outputs:
  brand/logo.svg          static, faithful, colours baked in
  brand/logo.paths.json   path data + measured mask width, for the site
"""

import json
import pathlib

import numpy as np
from PIL import Image
from scipy.ndimage import distance_transform_edt

ROOT = pathlib.Path(__file__).resolve().parent
SRC = ROOT / "logo.png"

BODY = np.array([0x05, 0x29, 0x47])
RIM = np.array([0x05, 0xA1, 0xEA])
INK = np.array([0xFD, 0xFD, 0xFD])
FILL = {"rim": "#05a1ea", "body": "#052947", "ink": "#fdfdfd"}


# --------------------------------------------------------------------- masks
def classify():
    a = np.asarray(Image.open(SRC).convert("RGBA")).astype(np.int32)
    rgb, alpha = a[..., :3], a[..., 3]
    solid = alpha > 128

    d = np.stack([((rgb - c) ** 2).sum(-1) for c in (BODY, RIM, INK)])
    nearest = np.argmin(d, axis=0)

    # Anti-aliased white/navy edges are mid-grey and fall to the dark side,
    # eroding the ribbon. Reclaim clearly-light pixels — but only NEUTRAL
    # ones, or the rim's cyan highlight gets painted white too.
    lum = rgb.mean(-1)
    neutral = (rgb[..., 2] - rgb[..., 0]) < 60
    ink = solid & ((nearest == 2) | ((lum > 150) & neutral))

    return {
        "solid": solid,
        "body": solid & (nearest == 0) & ~ink,
        "rim": solid & (nearest == 1) & ~ink,
        "ink": ink,
    }


# ------------------------------------------------------------------ outlines
def loops(mask):
    """Trace the pixel boundary of a mask into closed rings.

    Every foreground pixel contributes an oriented unit edge per side facing
    background. Oriented consistently they stitch into closed rings, and
    fill-rule evenodd sorts outers from holes.
    """
    m = np.pad(mask, 1)
    ys, xs = np.nonzero(m)
    S, E = [], []
    for sel, (sx, sy), (ex, ey) in (
        (~m[ys - 1, xs], (0, 0), (1, 0)),
        (~m[ys, xs + 1], (1, 0), (1, 1)),
        (~m[ys + 1, xs], (1, 1), (0, 1)),
        (~m[ys, xs - 1], (0, 1), (0, 0)),
    ):
        x, y = xs[sel], ys[sel]
        S.append(np.stack([x + sx, y + sy], 1))
        E.append(np.stack([x + ex, y + ey], 1))

    nxt = {}
    for s, e in zip(map(tuple, np.concatenate(S)), map(tuple, np.concatenate(E))):
        nxt.setdefault(s, []).append(e)

    out = []
    for origin in list(nxt):
        while nxt.get(origin):
            ring, cur = [origin], origin
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
    p = np.array(ring, float)
    return 0.5 * abs(
        np.dot(p[:, 0], np.roll(p[:, 1], 1)) - np.dot(p[:, 1], np.roll(p[:, 0], 1))
    )


def rdp(pts, eps):
    """Iterative Douglas-Peucker; recursion overflows on rings this long."""
    P = np.asarray(pts, float)
    keep = np.zeros(len(P), bool)
    keep[0] = keep[-1] = True
    stack = [(0, len(P) - 1)]
    while stack:
        i, j = stack.pop()
        if j <= i + 1:
            continue
        seg = P[j] - P[i]
        n = np.hypot(*seg)
        rel = P[i + 1 : j] - P[i]
        dist = (
            np.hypot(rel[:, 0], rel[:, 1])
            if n == 0
            else np.abs(seg[0] * rel[:, 1] - seg[1] * rel[:, 0]) / n
        )
        k = int(np.argmax(dist))
        if dist[k] > eps:
            k += i + 1
            keep[k] = True
            stack += [(i, k), (k, j)]
    return P[keep]


def bezier(P, closed, tension=0.5):
    """Catmull-Rom through the points, emitted as cubic beziers."""
    n = len(P)
    d = [f"M{P[0][0]:.2f} {P[0][1]:.2f}"]
    span = range(n) if closed else range(n - 1)
    for i in span:
        p0 = P[(i - 1) % n] if closed else P[max(i - 1, 0)]
        p1, p2 = P[i % n], P[(i + 1) % n]
        p3 = P[(i + 2) % n] if closed else P[min(i + 2, n - 1)]
        c1 = p1 + (p2 - p0) / 6 * tension * 2
        c2 = p2 - (p3 - p1) / 6 * tension * 2
        d.append(f"C{c1[0]:.2f} {c1[1]:.2f} {c2[0]:.2f} {c2[1]:.2f} {p2[0]:.2f} {p2[1]:.2f}")
    if closed:
        d.append("Z")
    return "".join(d)


# ---------------------------------------------------------------- centerline
def thin(img):
    """Zhang-Suen thinning to a one-pixel skeleton."""
    img = img.astype(np.uint8).copy()
    changed = True
    while changed:
        changed = False
        for step in (0, 1):
            p = np.pad(img, 1)
            P2, P3, P4, P5, P6, P7, P8, P9 = (
                p[0:-2, 1:-1], p[0:-2, 2:], p[1:-1, 2:], p[2:, 2:],
                p[2:, 1:-1], p[2:, 0:-2], p[1:-1, 0:-2], p[0:-2, 0:-2],
            )
            B = P2 + P3 + P4 + P5 + P6 + P7 + P8 + P9
            seq = [P2, P3, P4, P5, P6, P7, P8, P9, P2]
            A = sum(((seq[i] == 0) & (seq[i + 1] == 1)).astype(np.uint8) for i in range(8))
            c1, c2 = (P2 * P4 * P6, P4 * P6 * P8) if step == 0 else (P2 * P4 * P8, P2 * P6 * P8)
            kill = (img == 1) & (B >= 2) & (B <= 6) & (A == 1) & (c1 == 0) & (c2 == 0)
            if kill.any():
                img[kill] = 0
                changed = True
    return img


NB = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]


def longest_run(skel):
    pts = {(int(y), int(x)) for y, x in zip(*np.nonzero(skel))}

    def nbrs(p):
        return [(p[0] + dy, p[1] + dx) for dy, dx in NB if (p[0] + dy, p[1] + dx) in pts]

    def bfs(start):
        prev, order, i = {start: None}, [start], 0
        while i < len(order):
            cur = order[i]
            i += 1
            for n in nbrs(cur):
                if n not in prev:
                    prev[n] = cur
                    order.append(n)
        return prev, order[-1]

    ends = [p for p in pts if len(nbrs(p)) == 1] or [min(pts)]
    _, far = bfs(ends[0])
    prev, other = bfs(far)          # double BFS = graph diameter
    path, cur = [], other
    while cur is not None:
        path.append(cur)
        cur = prev[cur]
    return path


# --------------------------------------------------------------------- build
masks = classify()
solid = masks["solid"]
ys, xs = np.nonzero(solid)
x0, x1 = xs.min() - 1, xs.max() + 2
y0, y1 = ys.min() - 1, ys.max() + 2
scale = 1000.0 / (x1 - x0)
vb_h = (y1 - y0) * scale
norm = lambda P: (np.asarray(P, float) - np.array([x0, y0])) * scale

paths = {}
for name, mask in (("rim", solid), ("body", masks["body"] | masks["ink"]), ("ink", masks["ink"])):
    rings = sorted((r for r in loops(mask) if area(r) > 400), key=area, reverse=True)
    d = "".join(bezier(norm(rdp(r, 1.0)), closed=True) for r in rings if len(rdp(r, 1.0)) >= 4)
    paths[name] = d
    print(f"{name:5s} {len(rings)} rings, {len(d)} chars")

skel = thin(masks["ink"])
run = longest_run(skel)

# Cut where the skeleton runs into the arrowhead, and replace the genuinely
# straight tail with one clean line.
raw = [(float(c), float(r)) for r, c in run]
if raw[0][0] > raw[-1][0]:
    raw.reverse()
raw = raw[: max(range(len(raw)), key=lambda i: raw[i][0]) + 1]
tail_y = float(np.median([p[1] for p in raw if p[0] > raw[-1][0] - 0.35 * (x1 - x0)]))
band = [i for i, p in enumerate(raw) if abs(p[1] - tail_y) < 12]
entry, x_end = band[0], max(raw[i][0] for i in band)
curve = norm(rdp(raw[: entry + 1], 2.0))
end = norm([[x_end, tail_y]])[0]
center = bezier(curve, closed=False) + f" L{end[0]:.2f} {end[1]:.2f}"

# Mask width: twice the largest inscribed radius along the mark, so the sweep
# always covers it. Measured, not guessed — and it must stay under the gap
# between the spiral's arms or the sweep bleeds across them.
edt = distance_transform_edt(solid)
along = np.array([edt[r, c] for r, c in run])
mask_w = float(np.percentile(along, 99) * 2 * scale * 1.15)

# The sweep alone can never reveal everything: the arrowhead is far wider
# than the ribbon, and the bar's left wedge is navy-only, so the white
# ribbon's skeleton never passes through it. Rather than chase those
# coordinates, the mask carries a full-coverage panel that settles in as the
# arrow lands — which also means future artwork changes cannot silently clip
# the mark.

print(f"mask stroke width {mask_w:.1f}  (viewBox 0 0 1000 {vb_h:.1f})")

svg = [
    f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 {vb_h:.1f}" '
    f'role="img" aria-label="Throughline">',
    "  <title>Throughline</title>",
]
for name in ("rim", "body", "ink"):
    svg.append(
        f'  <path class="tl-{name}" fill="{FILL[name]}" fill-rule="evenodd" d="{paths[name]}"/>'
    )
svg.append("</svg>")
(ROOT / "logo.svg").write_text("\n".join(svg) + "\n")

(ROOT / "logo.paths.json").write_text(
    json.dumps(
        {
            "viewBox": f"0 0 1000 {vb_h:.1f}",
            "height": round(vb_h, 1),
            "fills": FILL,
            "outline": paths,
            "centerline": center,
            "maskWidth": round(mask_w, 1),
        },
        indent=2,
    )
    + "\n"
)
print("wrote logo.svg and logo.paths.json")
