"""Koren beam-tetrode constants for the 6V6GT, fitted to published data.

Two operating points, from two sheets, because no single sheet gives enough:

  RCA, Class A1 single-ended: plate 250 V, screen 250 V, grid -12.5 V,
  plate current 45 mA, screen current 4.5 mA.

  General Electric, 1955, Class A pentode: plate 180 V, screen 180 V,
  grid -8.5 V, plate current 29 mA, screen current 3 mA, transconductance
  3700 micromho, plate resistance 50 kohm.

Held out as an independent check rather than fitted: RCA RC-30 receiving tube
manual, p. 557, Class AB1 push-pull pair -- plate 285 V, screen 285 V, grid
-19 V, zero-signal plate current 70 mA for the pair, 8 kohm plate to plate,
14 W out. That is 35 mA in one tube, and the fit below reproduces it to better
than a tenth of a milliamp without ever being shown it. The same table's screen
figure is *not* used at all: the sources available do not make clear whether it
is quoted per tube or for the pair, and a target that might be wrong by a factor
of two is worse than no target.

Five targets: two plate currents, two screen currents and the transconductance.
`ex` is held at 1.35, which is what every other power tube in this catalogue
uses (T6L6GC, EL34, EL84), so the fit stays inside the same family rather than
buying a better number with a constant nobody else has. That leaves mu, kg1,
kp, kvb and kg2 -- five constants for five targets.

The plate resistance is checked afterwards rather than fitted: a beam tetrode's
rp comes out of the knee term, and asking a five-constant model to hit six
numbers is how a fit stops describing the tube and starts describing the
weights. Run: python3 tools/tube_fit/fit_6v6gt.py
"""
import math
import sys
import numpy as np

sys.path.insert(0, "tools/speaker_fit")
from fitlib import nelder_mead  # noqa: E402

EX = 1.35
# Published, not fitted. RCA and the tube compendia give the 6V6's screen
# amplification factor as 10, and the residual surface is flat in it: leaving it
# free finds the same fit at 10, at 14 and at 40. A published constant is not a
# free parameter.
MU = 10.0


def currents(p, vp, vs, vg):
    """Plate and screen current, in the exact form `dsp::device` evaluates."""
    kg1, kp, kvb, kg2 = p
    mu = MU
    if vs <= 0.0 or vp <= 0.0:
        return 0.0, 0.0
    inner = kp * (1.0 / mu + vg / vs)
    soft = inner if inner > 30.0 else math.log1p(math.exp(inner))
    e1 = vs / kp * soft
    if e1 <= 0.0:
        return 0.0, 0.0
    powered = e1 ** EX
    knee = math.atan(vp / kvb)
    return powered / kg1 * knee, powered / kg2


def unpack(q):
    return [math.exp(q[0]), math.exp(q[1]), math.exp(q[2]), math.exp(q[3])]


# (plate V, screen V, grid V, plate A, screen A)
RCA = (250.0, 250.0, -12.5, 0.045, 0.0045)
GE = (180.0, 180.0, -8.5, 0.029, 0.003)
# Held out, plate current only. See the docstring.
RC30 = (285.0, 285.0, -19.0, 0.035, None)
GM = 3.7e-3


def cost(q):
    p = unpack(q)
    kg1, kp, kvb, kg2 = p
    # Bounds, because without them the search walks off to kp ~ 1e10 and
    # kvb ~ 1e6, which fits slightly better and is not a tube.
    if not (100.0 < kg1 < 4000.0 and 10.0 < kp < 400.0):
        return 1e9
    if not (1.0 < kvb < 400.0 and 500.0 < kg2 < 40_000.0):
        return 1e9
    terms = []
    for vp, vs, vg, ip, isg in (RCA, GE):
        a, b = currents(p, vp, vs, vg)
        terms.append((a - ip) / ip)
        terms.append((b - isg) / isg)
    h = 1e-4
    vp, vs, vg = GE[0], GE[1], GE[2]
    gm = (currents(p, vp, vs, vg + h)[0] - currents(p, vp, vs, vg - h)[0]) / (2.0 * h)
    terms.append((gm - GM) / GM)
    return float(np.sum(np.square(terms)))


def main():
    # Restarted from a spread of starting points: this surface has a shallow
    # valley and a single Nelder-Mead run lands in a different corner of it
    # depending on where it begins.
    best, score = None, float("inf")
    for kg1 in (400.0, 800.0, 1500.0):
        for kp in (30.0, 50.0, 90.0):
            for kvb in (10.0, 40.0, 120.0):
                start = [math.log(kg1), math.log(kp), math.log(kvb), math.log(6000.0)]
                step = [0.25, 0.25, 0.35, 0.25]
                point, value = nelder_mead(cost, start, step, iters=20000)
                if value < score:
                    best, score = point, value
    print(f"best of the restarts: {score:.3e}")
    p = unpack(best)
    kg1, kp, kvb, kg2 = p
    print(f"residual {cost(best):.3e}")
    print("PentodeSpec {")
    print(f"    mu: {MU},")
    print(f"    ex: {EX},")
    print(f"    kg1: {kg1:.3f},")
    print(f"    kp: {kp:.4f},")
    print(f"    kvb: {kvb:.4f},")
    print(f"    kg2: {kg2:.2f},")
    print("}")
    print()
    for name, (vp, vs, vg, ip, isg) in (
        ("RCA 250/250/-12.5", RCA),
        ("GE 180/180/-8.5", GE),
    ):
        a, b = currents(p, vp, vs, vg)
        print(
            f"{name:<22} plate {a * 1e3:6.2f} mA (sheet {ip * 1e3:.1f})"
            f"   screen {b * 1e3:5.2f} mA (sheet {isg * 1e3:.1f})"
        )
    vp, vs, vg, ip, _ = RC30
    held = currents(p, vp, vs, vg)[0]
    print(
        f"{'RC-30 285/285/-19':<22} plate {held * 1e3:6.2f} mA "
        f"(sheet {ip * 1e3:.1f}, HELD OUT of the fit)"
    )
    h = 1e-4
    gm = (currents(p, 180, 180, -8.5 + h)[0] - currents(p, 180, 180, -8.5 - h)[0]) / (2 * h)
    ga = (currents(p, 180 + h, 180, -8.5)[0] - currents(p, 180 - h, 180, -8.5)[0]) / (2 * h)
    print(f"transconductance       {gm * 1e6:.0f} umho (sheet 3700)")
    print(f"plate resistance       {1.0 / ga / 1e3:.1f} kohm (sheet 50, not fitted)")
    # The loud end, which is not a published number for this tube. Printed so
    # the limitation is visible rather than assumed away: see the research log.
    hot = currents(p, 50.0, 250.0, 0.0)[0]
    print(f"zero bias at 50 V      {hot * 1e3:.0f} mA (no published figure; curves suggest ~200)")


if __name__ == "__main__":
    main()
