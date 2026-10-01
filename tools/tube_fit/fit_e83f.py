"""Koren pentode constants for the E83F, fitted to Philips's data sheet.

Philips "E83F S.Q. TUBE", December 1968 (Frank's electron tube data sheets,
`sheets/009/e/E83F.pdf`), page 2, characteristics, column I:

  anode 210 V, grid 3 at 0, screen 120 V, cathode resistor 165 ohm: anode
  current 10 mA, screen current 2.1 mA, mutual conductance 9 mA/V, internal
  resistance 0.5 Mohm, amplification factor grid 2 to grid 1 38; and the
  cut-off figure, 0.5 mA of anode current at -5 V of grid with the same anode
  and screen.

`mu` held at the sheet's 38. Four figures for the five other constants, so
the internal resistance goes in as a fifth with a light weight -- Koren's knee
cannot reach a pentode's megohms (the EF86 and 6V6GT fits say the same), and
it is there only to hold kvb in a sensible place. The Telefunken V76's output
valve (`docs/models/german_76.md`).

Fitted 2026-10-01: residual 2.8e-3; 10.46 mA, 2.11 mA, 8.78 mA/V, 0.50 Mohm and
0.49 mA at -5 V. `ex` runs to its 1.8 bound. Widened to 2.6 it runs there too,
with a tenth of the residual -- the cut-off figure is what pushes it -- and
the 1.8 result is already within 5 % of every figure, so it is kept: the
exponent decides the curvature, and a Koren pentode's sits near 1.35-1.5.

Run: python3 tools/tube_fit/fit_e83f.py
"""
import math
import sys
import numpy as np

sys.path.insert(0, "tools/speaker_fit")
sys.path.insert(0, "tools/tube_fit")
from fitlib import nelder_mead  # noqa: E402
from smallpentode import currents  # noqa: E402

MU = 38.0
VA, VS, RK = 210.0, 120.0, 165.0
IA, IG2, GM, RI = 10e-3, 2.1e-3, 9e-3, 0.5e6
CUT = (-5.0, 0.5e-3)


def unpack(q):
    return (MU, math.exp(q[0]), math.exp(q[1]), math.exp(q[2]), math.exp(q[3])), q[4]


def point(p, ex):
    lo, hi = 0.0, 10.0
    for _ in range(50):
        vk = 0.5 * (lo + hi)
        a, b = currents(p, VA - vk, VS - vk, -vk, ex)
        if (a + b) * RK > vk:
            lo = vk
        else:
            hi = vk
    vk = 0.5 * (lo + hi)
    a, b = currents(p, VA - vk, VS - vk, -vk, ex)
    h = 1e-4
    gm = (currents(p, VA - vk, VS - vk, -vk + h, ex)[0] - currents(p, VA - vk, VS - vk, -vk - h, ex)[0]) / (2 * h)
    ga = (currents(p, VA - vk + h, VS - vk, -vk, ex)[0] - currents(p, VA - vk - h, VS - vk, -vk, ex)[0]) / (2 * h)
    return a, b, gm, 1.0 / max(ga, 1e-15), vk


def cost(q):
    p, ex = unpack(q)
    _, kg1, kp, kvb, kg2 = p
    if not (1.1 < ex < 1.8 and 10.0 < kg1 < 1e5 and 1.0 < kp < 2000.0 and 0.5 < kvb < 2000.0 and 50.0 < kg2 < 1e6):
        return 1e9
    a, b, gm, ri, _ = point(p, ex)
    cut = currents(p, VA, VS, CUT[0], ex)[0]
    terms = [
        (a - IA) / IA,
        (b - IG2) / IG2,
        (gm - GM) / GM,
        0.5 * math.log(max(cut, 1e-12) / CUT[1]),
        0.1 * math.log(ri / RI),
    ]
    return float(np.sum(np.square(terms)))


def main():
    best, score = None, float("inf")
    for kg1, kp, kvb in ((200.0, 40.0, 20.0), (500.0, 100.0, 30.0), (100.0, 20.0, 10.0), (300.0, 60.0, 60.0)):
        start = [math.log(kg1), math.log(kp), math.log(kvb), math.log(1000.0), 1.35]
        q, v = nelder_mead(cost, start, [0.4, 0.4, 0.5, 0.4, 0.05], iters=3000)
        if v < score:
            best, score = q, v
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
    a, b, gm, ri, vk = point(p, ex)
    print(f"{a * 1e3:.2f} mA (10), screen {b * 1e3:.2f} mA (2.1), {gm * 1e3:.2f} mA/V (9), ri {ri / 1e6:.2f} Mohm (0.5), "
          f"cathode {vk:.2f} V; at -5 V: {currents(p, VA, VS, -5.0, ex)[0] * 1e3:.2f} mA (0.5)")


if __name__ == "__main__":
    main()
