"""Koren pentode constants for the EF86, fitted to Philips's data sheet.

Philips "EF86", January 1970 (Frank's electron tube data sheets,
`sheets/010/e/EF86.pdf`), page 2:

  typical characteristics: plate 250 V, screen 140 V, grid -2.2 V: plate
  current 3.0 mA, screen current 0.6 mA, transconductance 2.2 mA/V,
  amplification factor grid 2 to grid 1 38, internal resistance 2.5 Mohm;

  operating characteristics as an A.F. amplifier: plate resistor 100 k, screen
  resistor 390 k from the supply, cathode resistor 1 k, the grid at ground;
  cathode current 3.2, 2.75, 2.4, 2.0, 1.55 and 1.05 mA at 400, 350, 300, 250,
  200 and 150 V; voltage gain 140, 134, 129, 123, 117 and 110.

`mu` is held at the sheet's 38, the screen's amplification factor, which is
what Koren's `mu` is. Fitted: kg1, kp, kvb, kg2 and ex against the typical
point's two currents and transconductance and the six cathode currents. That
operating table is the EMI REDD.47's own V1 -- 100 k plate, 390 k screen,
1 k cathode, from 205 V -- and its drawing's 1.35 mA is the check.

Held out: the table's voltage gains (the stage's transconductance into 100 k
in parallel with the next grid's 330 k and the valve's own resistance, the
cathode bypassed), and the internal resistance.

Fitted 2026-10-01: residual 3.7e-3; the typical point to 0.3 %, the six table
cathode currents within 3.5 %, the REDD.47's V1 at 1.50 mA against its
drawing's 1.35; internal resistance 1.24 Mohm against 2.5 (not fitted: the
Koren knee's limit, as for the 6V6GT). Not yet a `PentodeSpec`: it has no
consumer until the REDD.47 is built.

Run: python3 tools/tube_fit/fit_ef86.py
"""
import math
import sys
import numpy as np

sys.path.insert(0, "tools/speaker_fit")
sys.path.insert(0, "tools/tube_fit")
from fitlib import nelder_mead  # noqa: E402
from smallpentode import currents, stage  # noqa: E402

MU = 38.0
TYPICAL = (250.0, 140.0, -2.2, 3.0e-3, 0.6e-3, 2.2e-3)
TABLE = [  # supply, cathode current, voltage gain
    (400.0, 3.2e-3, 140.0),
    (350.0, 2.75e-3, 134.0),
    (300.0, 2.4e-3, 129.0),
    (250.0, 2.0e-3, 123.0),
    (200.0, 1.55e-3, 117.0),
    (150.0, 1.05e-3, 110.0),
]
RA, RG2, RK, NEXT = 100e3, 390e3, 1e3, 330e3


def unpack(q):
    return (MU, math.exp(q[0]), math.exp(q[1]), math.exp(q[2]), math.exp(q[3])), q[4]


def cost(q):
    p, ex = unpack(q)
    _, kg1, kp, kvb, kg2 = p
    if not (1.1 < ex < 1.8 and 20.0 < kg1 < 1e5 and 1.0 < kp < 2000.0 and 0.5 < kvb < 2000.0 and 100.0 < kg2 < 1e6):
        return 1e9
    vp, vs, vg, ia, ig2, gm_want = TYPICAL
    a, b = currents(p, vp, vs, vg, ex)
    h = 1e-4
    gm = (currents(p, vp, vs, vg + h, ex)[0] - currents(p, vp, vs, vg - h, ex)[0]) / (2 * h)
    terms = [(a - ia) / ia, (b - ig2) / ig2, (gm - gm_want) / gm_want]
    for vb, ik, _ in TABLE:
        a, b = stage(p, ex, vb, RA, RG2, RK)
        terms.append((a + b - ik) / ik)
    return float(np.sum(np.square(terms)))


def main():
    # Four restarts rather than a grid: every cost evaluation solves six
    # resistance-coupled stages by nested bisection, and a grid of fifty-four
    # took hours. The starts span the families the catalogue's pentodes sit in.
    best, score = None, float("inf")
    for kg1, kp, kvb in ((2000.0, 100.0, 30.0), (800.0, 40.0, 10.0), (6000.0, 300.0, 100.0), (1500.0, 60.0, 50.0)):
        start = [math.log(kg1), math.log(kp), math.log(kvb), math.log(10000.0), 1.4]
        point, value = nelder_mead(cost, start, [0.4, 0.4, 0.5, 0.4, 0.05], iters=2500)
        if value < score:
            best, score = point, value
    p, ex = unpack(best)
    print(f"residual {score:.3e}")
    print("PentodeSpec {")
    print(f"    mu: {MU},")
    print(f"    ex: {ex:.4f},")
    print(f"    kg1: {p[1]:.3f},")
    print(f"    kp: {p[2]:.4f},")
    print(f"    kvb: {p[3]:.4f},")
    print(f"    kg2: {p[4]:.2f},")
    print("}")
    vp, vs, vg, ia, ig2, gm_want = TYPICAL
    a, b = currents(p, vp, vs, vg, ex)
    h = 1e-4
    gm = (currents(p, vp, vs, vg + h, ex)[0] - currents(p, vp, vs, vg - h, ex)[0]) / (2 * h)
    ga = (currents(p, vp + h, vs, vg, ex)[0] - currents(p, vp - h, vs, vg, ex)[0]) / (2 * h)
    print(f"typical: {a * 1e3:.2f} mA (3.0), screen {b * 1e3:.2f} mA (0.6), {gm * 1e3:.2f} mA/V (2.2), "
          f"ri {1 / ga / 1e6:.2f} Mohm (2.5, not fitted)")
    for vb, ik, gain_want in TABLE:
        a, b = stage(p, ex, vb, RA, RG2, RK)
        print(f"  {vb:.0f} V: cathode {(a + b) * 1e3:.2f} mA (sheet {ik * 1e3:.2f})")
    a, b = stage(p, ex, 205.0, RA, RG2, 1_120.0)
    print(f"REDD.47 V1, 205 V, 100 k / 390 k / 1k12: cathode {(a + b) * 1e3:.2f} mA (drawing 1.35)")


if __name__ == "__main__":
    main()
