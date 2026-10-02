"""Koren beam-tetrode constants for the 7027A, fitted to RCA's sheet.

For the Ampeg VT-40 (`docs/models/american_vt40.md`), whose drawing puts its
two 7027As at 586 V on the plate, 589 V on the screen and -65 V on the grid.

RCA, "7027-A Beam Power Tube" (8-59; Frank's electron tube data sheets,
`sheets/049/7/7027A.pdf`). The 7027A is the 6L6GC in an octal base wired with
two screen pins and with higher ratings; its class A row is the 6L6GC's to the
figure. The solver's `T6L6GC`, fitted for the amplifiers built before this one
to a different pair of points, runs cold against this sheet's fixed-bias rows
(23 mA at 540 V / 400 V / -38 V where the sheet gives 50) and is held by the
frozen baselines, so the 7027A gets constants of its own.

Fitted, six targets for three constants (mu, kg1, kvb; `ex` held at 1.35 and
`kp` at 48 as for the KT66, which the rows below do not reach the cut-off to
settle), per valve:

  class A1: 250 V on plate and screen, -14 V: 72 mA, 6000 umho, 22.5 kohm;
  push-pull class AB1, fixed bias, no signal (two valves' currents halved):
  400 V / 300 V / -25 V, 51 mA; 450 V / 350 V / -30 V, 47.5 mA;
  540 V / 400 V / -38 V, 50 mA.

The screen: `kg2` from the class A row's 5 mA. The fixed-bias rows' screen
currents (3, 1.7 and 2.5 mA a valve), with the plate well above the screen,
are reported: Koren's screen law has no plate term.

Held out: the sheet's "average characteristics" at 300 V plate and screen,
read off the curve by eye at 10 V steps (about +/-3 mA): 9.5 mA at -40 V,
28 at -30, 66 at -20, 136 at -10, 250 at 0.

Run: uv run --no-project --with numpy python tools/tube_fit/fit_7027a.py
"""
import math
import sys
import numpy as np

sys.path.insert(0, "tools/speaker_fit")
from fitlib import nelder_mead  # noqa: E402

EX = 1.35
KP = 48.0


def currents(p, vp, vs, vg):
    """Plate and screen current, in the exact form `dsp::device` evaluates."""
    mu, kg1, kp, kvb, kg2 = p
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


# (plate V, screen V, grid V, plate A, screen A), per valve.
CLASS_A = (250.0, 250.0, -14.0, 0.072, 0.005)
GM = 6.0e-3
RA = 22.5e3
FIXED = (
    (400.0, 300.0, -25.0, 0.051, 0.003),
    (450.0, 350.0, -30.0, 0.0475, 0.0017),
    (540.0, 400.0, -38.0, 0.050, 0.0025),
)
# Held out: the 300 V / 300 V curve, read by eye.
CURVE = ((-40.0, 0.0095), (-30.0, 0.028), (-20.0, 0.066), (-10.0, 0.136), (0.0, 0.250))


def slopes(p, vp, vs, vg, h=1e-4):
    gm = (currents(p, vp, vs, vg + h)[0] - currents(p, vp, vs, vg - h)[0]) / (2.0 * h)
    ga = (currents(p, vp + h, vs, vg)[0] - currents(p, vp - h, vs, vg)[0]) / (2.0 * h)
    return gm, ga


def plate_cost(q):
    mu, kg1, kvb = q[0], math.exp(q[1]), math.exp(q[2])
    if not (3.0 < mu < 16.0 and 100.0 < kg1 < 6000.0 and 2.0 < kvb < 400.0):
        return 1e9
    p = (mu, kg1, KP, kvb, 1e12)
    terms = []
    for vp, vs, vg, ip, _ in (CLASS_A,) + FIXED:
        terms.append((currents(p, vp, vs, vg)[0] - ip) / ip)
    gm, ga = slopes(p, *CLASS_A[:3])
    terms.append((gm - GM) / GM)
    terms.append((1.0 / ga - RA) / RA if ga > 0.0 else 10.0)
    return float(np.sum(np.square(terms)))


def main():
    best, score = None, float("inf")
    for mu in (5.0, 7.0, 9.0, 11.0):
        for kg1 in (300.0, 900.0, 2000.0):
            for kvb in (5.0, 20.0, 80.0):
                start = [mu, math.log(kg1), math.log(kvb)]
                step = [0.8, 0.25, 0.35]
                point, value = nelder_mead(plate_cost, start, step, iters=20000)
                if value < score:
                    best, score = point, value
    mu, kg1, kvb = best[0], math.exp(best[1]), math.exp(best[2])
    unit = (mu, kg1, KP, kvb, 1.0)
    kg2 = currents(unit, *CLASS_A[:3])[1] / CLASS_A[4]
    p = (mu, kg1, KP, kvb, kg2)
    print(f"plate residual {score:.3e}")
    print("PentodeSpec {")
    print(f"    mu: {mu:.4f},")
    print(f"    ex: {EX},")
    print(f"    kg1: {kg1:.3f},")
    print(f"    kp: {KP},")
    print(f"    kvb: {kvb:.4f},")
    print(f"    kg2: {kg2:.2f},")
    print("}")
    print()
    for name, row in [("class A 250/250/-14", CLASS_A)] + [
        (f"AB1 fixed {r[0]:.0f}/{r[1]:.0f}/{r[2]:.0f}", r) for r in FIXED
    ]:
        a, b = currents(p, *row[:3])
        print(
            f"{name:<26} plate {a * 1e3:6.2f} mA (sheet {row[3] * 1e3:5.1f})"
            f"   screen {b * 1e3:5.2f} mA (sheet {row[4] * 1e3:.1f})"
        )
    gm, ga = slopes(p, *CLASS_A[:3])
    print(f"{'mutual conductance':<26} {gm * 1e3:.2f} mA/V (6.0)   fitted")
    print(f"{'plate resistance':<26} {1.0 / ga / 1e3:.1f} kohm (22.5)   fitted")
    for vg, ip in CURVE:
        a, b = currents(p, 300.0, 300.0, vg)
        print(f"300/300/{vg:<5.0f}                plate {a * 1e3:6.1f} mA (curve {ip * 1e3:5.1f})   HELD OUT")
    for vg in (-55.0, -60.0, -65.0, -70.0, -75.0):
        a, b = currents(p, 586.0, 589.0, vg)
        print(f"VT-40 at 586/589/{vg:.0f}: plate {a * 1e3:5.1f} mA, screen {b * 1e3:4.1f} mA, {a * 586:.1f} W")


if __name__ == "__main__":
    main()
