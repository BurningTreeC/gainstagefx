# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy", "scipy", "pillow"]
# ///
"""Reconstruct a schematic's wire topology from a scan, by measurement.

    uv run tools/schematic/trace.py SHEET.png X Y W H [--blobs] [--overlay] [@x,y ...]

Reading a dense factory schematic off a screen is where this project has made
its worst mistakes: twice a netlist was written for a section whose junctions
had been judged by eye, and twice the DC solve threw it out. The distinction
that matters is a hair's breadth on screen and unambiguous in the pixels --
**a crossing with a dot is a connection and a crossing without one is not** --
so it should be measured rather than squinted at.

How it works, and why each step is what it is:

* **The stroke width is measured, not assumed.** Run lengths through the ink
  give a median; on the JC-120 sheet at 1334 ppi that is 10 px.
* **Junction dots are blobs wider than a wire.** An opening with a disk a
  little smaller than the stroke erases the wires and leaves the dots, which
  are filled circles about 3.5 times the stroke across. Glyphs survive too, so
  the candidates are filtered by size, aspect and fill.
* **Wires are long thin runs.** An opening with a 90 px bar in each direction
  leaves the horizontals and verticals and nothing else.
* **Nodes are wires joined by union-find.** A dot joins everything passing
  through it; and a wire that *ends* on another is joined whether or not a dot
  is drawn, because a corner and a T are connections by construction -- you
  cannot cross and stop. Only a crossing where both wires continue needs the
  dot, which is exactly the distinction the whole exercise turns on.

Two things it does not do, and the caller has to watch for. Connector boxes and
component outlines are rectangles, and their edges read as wires -- crop them
out or the box will short every row it contains together. And it finds no
components: it gives the nodes, and which part bridges which pair still has to
be read off the drawing.

`--overlay` writes the reconstruction back over the scan in colour, which is
how it gets checked; `@x,y` asks which node a point lies on.
"""

import sys

import numpy as np
from PIL import Image
from scipy import ndimage

# The sheet is a 17442 x 11747 scan, not a decompression bomb.
Image.MAX_IMAGE_PIXELS = None

path, x0, y0, w, h = sys.argv[1], *map(int, sys.argv[2:6])
img = Image.open(path).convert("L")
reg = np.asarray(img.crop((x0, y0, x0 + w, y0 + h)))
# The sheet is a stencil: ink is white. Make ink True.
ink = reg > 127
print(f"region ({x0},{y0}) {w}x{h}, ink {ink.mean() * 100:.1f}%")


def disk(r):
    y, x = np.ogrid[-r : r + 1, -r : r + 1]
    return x * x + y * y <= r * r


# --- stroke width, measured --------------------------------------------
widths = []
for x in range(0, w, 7):
    col = ink[:, x]
    d = np.diff(col.astype(np.int8))
    starts, ends = np.where(d == 1)[0], np.where(d == -1)[0]
    n = min(len(starts), len(ends))
    widths.extend((ends[:n] - starts[:n]).tolist())
widths = np.array([v for v in widths if 0 < v < 60])
med = int(np.median(widths))
print(f"stroke width: median {med} px, 90th pct {int(np.percentile(widths, 90))}")

# --- junction dots ------------------------------------------------------
opened = ndimage.binary_opening(ink, disk(med - 3))
lab, n = ndimage.label(opened)
areas = ndimage.sum(opened, lab, range(1, n + 1))
cents = ndimage.center_of_mass(opened, lab, range(1, n + 1))
# A dot is a filled circle of about twice the stroke width; glyph blobs are
# either much smaller or much larger and not round.
cand = []
objs = ndimage.find_objects(lab)
for i, (a, c) in enumerate(zip(areas, cents)):
    sy, sx = objs[i]
    bh, bw = sy.stop - sy.start, sx.stop - sx.start
    cand.append((int(a), bw, bh, a / (bh * bw), round(c[1]), round(c[0])))
if "--blobs" in sys.argv:
    for a, bw, bh, f, cx, cy in sorted(cand):
        print(f"   area {a:>5}  {bw:>3}x{bh:<3} fill {f:.2f}  at ({x0 + cx},{y0 + cy})")
# A junction dot is a filled circle of about twice the stroke width.
lo, hi = 2.8 * med, 4.4 * med
dots = [
    (cx, cy, a)
    for a, bw, bh, f, cx, cy in cand
    if lo <= bw <= hi and lo <= bh <= hi and f > 0.55
]
dots.sort(key=lambda d: (d[1], d[0]))
print(f"junction dots: {len(dots)}")


# --- wires --------------------------------------------------------------
def segments(mask, axis):
    lab, _ = ndimage.label(mask)
    out = []
    for sl in ndimage.find_objects(lab):
        ys, xs = sl
        length = (xs.stop - xs.start) if axis == "h" else (ys.stop - ys.start)
        if length < 60:
            continue
        out.append(
            (
                (xs.start + xs.stop) // 2 if axis == "v" else xs.start,
                (ys.start + ys.stop) // 2 if axis == "h" else ys.start,
                xs.stop if axis == "h" else ys.stop,
            )
        )
    return out


hbar = np.ones((1, 90), bool)
vbar = np.ones((90, 1), bool)
hw = segments(ndimage.binary_opening(ink, hbar), "h")
vw = segments(ndimage.binary_opening(ink, vbar), "v")
print(f"horizontal wires: {len(hw)}   vertical wires: {len(vw)}")

TOL = 14
print("\n-- horizontal wires (sheet coords): y | x from -> to")
for x, y, x2 in sorted(hw, key=lambda s: s[1]):
    print(f"   y={y0 + y:<6} x {x0 + x:>6} -> {x0 + x2:<6}")
print("\n-- vertical wires: x | y from -> to")
for x, y, y2 in sorted(vw, key=lambda s: s[0]):
    print(f"   x={x0 + x:<6} y {y0 + y:>6} -> {y0 + y2:<6}")
print("\n-- junction dots (sheet coords), and what meets there")
for dx, dy, a in dots:
    hs = [
        f"H@y{y0 + y}"
        for x, y, x2 in hw
        if abs(y - dy) <= TOL and x - TOL <= dx <= x2 + TOL
    ]
    vs = [
        f"V@x{x0 + x}"
        for x, y, y2 in vw
        if abs(x - dx) <= TOL and y - TOL <= dy <= y2 + TOL
    ]
    print(f"   ({x0 + dx:>6},{y0 + dy:>6})  {' + '.join(hs + vs) or '(no wire found)'}")

# --- wires merged into electrical nodes ---------------------------------
# Union-find over the wire segments, joined wherever a junction dot sits on
# both. That turns a picture into a node list.
items = [("H", x, y, x2) for x, y, x2 in hw] + [("V", x, y, y2) for x, y, y2 in vw]
parent = list(range(len(items)))


def find(i):
    while parent[i] != i:
        parent[i] = parent[parent[i]]
        i = parent[i]
    return i


def union(a, b):
    ra, rb = find(a), find(b)
    if ra != rb:
        parent[ra] = rb


def on(i, px, py):
    kind, a, b, c = items[i]
    if kind == "H":
        return abs(b - py) <= TOL and a - TOL <= px <= c + TOL
    return abs(a - px) <= TOL and b - TOL <= py <= c + TOL


# A junction dot joins everything that passes through it.
for dx, dy, _ in dots:
    hit = [i for i in range(len(items)) if on(i, dx, dy)]
    for j in hit[1:]:
        union(hit[0], j)

# And a wire that *ends* on another is joined to it whether or not a dot is
# drawn: a corner and a T are connections by construction -- you cannot cross
# and stop. Only a crossing where both wires continue needs the dot, which is
# exactly the distinction this whole exercise turns on.
for i, (ki, ai, bi, ci) in enumerate(items):
    for j, (kj, aj, bj, cj) in enumerate(items):
        if ki == kj or i == j:
            continue
        h, v = (i, j) if ki == "H" else (j, i)
        hx, hy, hx2 = items[h][1], items[h][2], items[h][3]
        vx, vy, vy2 = items[v][1], items[v][2], items[v][3]
        if not (hx - TOL <= vx <= hx2 + TOL and vy - TOL <= hy <= vy2 + TOL):
            continue
        ends = (
            abs(vy - hy) <= TOL
            or abs(vy2 - hy) <= TOL
            or abs(hx - vx) <= TOL
            or abs(hx2 - vx) <= TOL
        )
        if ends:
            union(i, j)

groups = {}
for i in range(len(items)):
    groups.setdefault(find(i), []).append(i)


def describe(i):
    kind, a, b, c = items[i]
    if kind == "H":
        return f"H y={y0 + b} x {x0 + a}..{x0 + c}"
    return f"V x={x0 + a} y {y0 + b}..{y0 + c}"


print("\n-- electrical nodes (wires joined through junction dots)")
for k, (root, members) in enumerate(
    sorted(groups.items(), key=lambda kv: -len(kv[1])), start=1
):
    print(f"  node {k:>2} ({len(members)} wires)")
    for i in sorted(members, key=lambda i: (items[i][0], items[i][1])):
        print(f"      {describe(i)}")

# --- query: which node is a given point on? -----------------------------
for arg in sys.argv[6:]:
    if not arg.startswith("@"):
        continue
    px, py = (int(v) for v in arg[1:].split(","))
    hits = [i for i in range(len(items)) if on(i, px - x0, py - y0)]
    if not hits:
        print(f"\n  {arg}: no wire within {TOL} px")
        continue
    roots = {find(i) for i in hits}
    order = {
        root: k
        for k, (root, _) in enumerate(
            sorted(groups.items(), key=lambda kv: -len(kv[1])), start=1
        )
    }
    print(
        f"\n  {arg}: node {sorted(order[r] for r in roots)} via {[describe(i) for i in hits]}"
    )

# --- an overlay, so the reconstruction can be checked against the sheet --
if "--overlay" in sys.argv:
    from PIL import ImageDraw, ImageFont

    base = Image.fromarray((~ink * 255).astype(np.uint8)).convert("RGB")
    d = ImageDraw.Draw(base)
    palette = [
        (220, 30, 30),
        (30, 120, 220),
        (30, 160, 60),
        (200, 120, 0),
        (150, 40, 190),
        (0, 150, 150),
        (200, 0, 120),
        (110, 90, 30),
    ]
    try:
        font = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf", 34)
    except OSError:
        font = ImageFont.load_default()
    order = sorted(groups.items(), key=lambda kv: -len(kv[1]))
    for k, (root, members) in enumerate(order, start=1):
        col = palette[(k - 1) % len(palette)]
        for i in members:
            kind, a, b, c = items[i]
            if kind == "H":
                d.line([(a, b), (c, b)], fill=col, width=7)
                d.text((a + 6, b - 46), str(k), fill=col, font=font)
            else:
                d.line([(a, b), (a, c)], fill=col, width=7)
                d.text((a + 8, b + 6), str(k), fill=col, font=font)
    for dx, dy, _ in dots:
        d.ellipse([dx - 13, dy - 13, dx + 13, dy + 13], outline=(255, 0, 0), width=5)
    base.save("an/overlay.png")
    print("\noverlay written to an/overlay.png")
