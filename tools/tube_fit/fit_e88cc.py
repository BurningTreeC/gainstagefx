"""Koren triode constants for the E88CC, fitted to Philips's data sheet.

Philips "E88CC S.Q. TUBE", December 1968 (Frank's electron tube data sheets,
`sheets/009/e/E88CC.pdf`), page 2, characteristics, column I:

  anode supply 100 V, grid supply +9 V, cathode resistor 680 ohm: anode
  current 15 mA, mutual conductance 12.5 mA/V, amplification factor 33;
  anode supply 90 V, cathode resistor 120 ohm (grid at 0 V): anode current
  12 mA, mutual conductance 11.5 mA/V.

Both are taken at their own operating points: the cathode carries the anode
current through its resistor, so the first is 89.8 V across the valve at
-1.2 V of grid, the second 88.6 V at -1.44 V. Five targets -- two currents,
two transconductances and the amplification factor -- for four constants
(mu, ex, kg1, kp; kvb held at 300 as for every triode in the catalogue).

Checked afterwards, not fitted: the EMI REDD.47's paralleled E88CC, which its
drawing puts at 3.7 V on 200 ohm (18.5 mA for the pair) with its plates at
138 V. With the grid at ground that is 134 V across each half at -3.7 V, and
the fit gives 4.9 mA a half where the drawing implies about 9 -- as the sheet's
own two points imply too, so the open question is the drawing, not the fit:
R8 330 k and R9 1.6 M may hold V2's grid above ground. To be traced before the
REDD.47 is built (`docs/models/british_47.md`).

Fitted 2026-10-01: residual 3.1e-4; 15.2 mA / 12.48 mA/V / mu 33.0 and
11.9 mA / 11.48 mA/V at the sheet's two points. Not yet a `TriodeSpec`: it has
no consumer until the REDD.47 is built.

Run: python3 tools/tube_fit/fit_e88cc.py
"""
import math
import sys
import numpy as np

sys.path.insert(0, "tools/speaker_fit")
from fitlib import nelder_mead  # noqa: E402

KVB = 300.0


def plate(p, vp, vg):
    """Koren's triode, in the form `dsp::device` evaluates."""
    mu, ex, kg1, kp = p
    inner = kp * (1.0 / mu + vg / math.sqrt(KVB + vp * vp))
    soft = inner if inner > 30 else math.log1p(math.exp(inner))
    e1 = vp / kp * soft
    return 2.0 * e1 ** ex / kg1 if e1 > 0 else 0.0


def slopes(p, vp, vg, h=1e-4):
    gm = (plate(p, vp, vg + h) - plate(p, vp, vg - h)) / (2 * h)
    ga = (plate(p, vp + h, vg) - plate(p, vp - h, vg)) / (2 * h)
    return gm, ga


A = (89.8, -1.2, 15e-3, 12.5e-3)
B = (88.56, -1.44, 12e-3, 11.5e-3)
MU = 33.0


def unpack(q):
    return [q[0], q[1], math.exp(q[2]), math.exp(q[3])]


def cost(q):
    p = unpack(q)
    if not (10 < p[0] < 80 and 1.0 < p[1] < 2.0):
        return 1e9
    terms = []
    for vp, vg, ia, gm_want in (A, B):
        terms.append((plate(p, vp, vg) - ia) / ia)
        gm, _ = slopes(p, vp, vg)
        terms.append((gm - gm_want) / gm_want)
    gm, ga = slopes(p, A[0], A[1])
    terms.append((gm / ga - MU) / MU)
    return float(np.sum(np.square(terms)))


def main():
    best, score = None, float("inf")
    for mu in (20.0, 33.0, 45.0):
        for ex in (1.2, 1.4, 1.6):
            for kg1 in (50.0, 200.0, 800.0):
                for kp in (50.0, 200.0, 600.0):
                    start = [mu, ex, math.log(kg1), math.log(kp)]
                    point, value = nelder_mead(cost, start, [4.0, 0.1, 0.3, 0.3], iters=20000)
                    if value < score:
                        best, score = point, value
    p = unpack(best)
    print(f"residual {score:.3e}")
    print("TriodeSpec {")
    print(f"    mu: {p[0]:.4f},")
    print(f"    ex: {p[1]:.4f},")
    print(f"    kg1: {p[2]:.4f},")
    print(f"    kp: {p[3]:.4f},")
    print(f"    kvb: {KVB},")
    print("}")
    for name, (vp, vg, ia, gm_want) in (("100 V / 680 ohm", A), ("90 V / 120 ohm", B)):
        gm, ga = slopes(p, vp, vg)
        print(
            f"{name:<16} {plate(p, vp, vg) * 1e3:6.2f} mA (sheet {ia * 1e3:.1f}), "
            f"{gm * 1e3:5.2f} mA/V (sheet {gm_want * 1e3:.1f}), mu {gm / ga:.1f}"
        )
    print(f"REDD.47 half, 134 V at -3.7 V: {plate(p, 134.0, -3.7) * 1e3:.2f} mA (drawing: about 9)")


if __name__ == "__main__":
    main()
