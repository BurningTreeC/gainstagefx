"""Koren beam-tetrode constants for the 6550, fitted to General Electric's sheet.

General Electric, "6550-A Beam Pentode", Product Information, 4-72 (April
1972) -- a year and a half before Sunn drew the Model T this fit is for; the
6550-A is GE's direct replacement for the 6550 (Frank's electron tube data
sheets, `sheets/135/6/6550A.pdf`). Values per tube.

Fitted, four targets for four constants (mu, kg1, kp, kvb; `ex` held at 1.35
like every other power valve in the catalogue):

  page 2, average characteristics: plate 250 V, screen 250 V, grid -14 V,
  plate current 140 mA, transconductance 11,000 micromho;
  page 2, Class A1 single tube: 400 V, 225 V, -16.5 V, 87 mA;
  page 3, push-pull ultra-linear, screens tapped at 40 %, fixed bias: 450 V on
  plate and screen at -48 V, 150 mA for the pair, so 75 mA.

The ultra-linear row is in the fit rather than held out because it is the only
published point with the screen near where a Model T runs it (504 V on its
voltage chart), and the first attempt -- fitted to the pentode rows alone, all
at 310 V of screen or less -- extrapolated to 110 mA, 56 W a valve, at the
Model T's idle. With it, `mu` comes out at 8.2 unasked, against the sheet's
own triode amplification factor of 8.

The screen: Koren's screen law has no plate term, so it holds one ratio of
screen to plate current, and the sheet's two rows disagree -- 12/140 with the
plate at the screen's voltage, 4/87 with the plate well above it. An
ultra-linear stage idles with the two equal, so `kg2` is fitted to the average
point's 12 mA, and the ultra-linear row's 6 mA is the check.

Held out, all on the same sheet:

  push-pull AB1, pentode, fixed bias, 450 V / 310 V / -29.5 V: 75 mA, 4.5 mA;
  the same at 600 V / 300 V / -32.5 V: 50 mA, 2.5 mA;
  the ultra-linear row's screen, 6 mA;
  -40 V for 1 mA of plate current at 250 V / 250 V (page 2).

Run: python3 tools/tube_fit/fit_6550.py
"""
import math
import sys
import numpy as np

sys.path.insert(0, "tools/speaker_fit")
from fitlib import nelder_mead  # noqa: E402

EX = 1.35


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


# (plate V, screen V, grid V, plate A, screen A); per tube.
AVERAGE = (250.0, 250.0, -14.0, 0.140, 0.012)
CLASS_A1 = (400.0, 225.0, -16.5, 0.087, 0.004)
ULTRA_LINEAR = (450.0, 450.0, -48.0, 0.075, 0.006)
GM = 11.0e-3
# Held out of the fit entirely.
PP_450 = (450.0, 310.0, -29.5, 0.075, 0.0045)
PP_600 = (600.0, 300.0, -32.5, 0.050, 0.0025)
CUT_OFF = (250.0, 250.0, -40.0, 0.001)


def plate_cost(q):
    mu, kg1, kp, kvb = q[0], math.exp(q[1]), math.exp(q[2]), math.exp(q[3])
    if not (3.0 < mu < 14.0 and 100.0 < kg1 < 4000.0):
        return 1e9
    if not (5.0 < kp < 400.0 and 1.0 < kvb < 400.0):
        return 1e9
    p = (mu, kg1, kp, kvb, 1.0)
    terms = []
    for vp, vs, vg, ip, _ in (AVERAGE, CLASS_A1, ULTRA_LINEAR):
        terms.append((currents(p, vp, vs, vg)[0] - ip) / ip)
    h = 1e-4
    vp, vs, vg = AVERAGE[0], AVERAGE[1], AVERAGE[2]
    gm = (currents(p, vp, vs, vg + h)[0] - currents(p, vp, vs, vg - h)[0]) / (2.0 * h)
    terms.append((gm - GM) / GM)
    return float(np.sum(np.square(terms)))


def main():
    # Restarted from a spread of starting points, as `fit_6v6gt.py` is.
    best, score = None, float("inf")
    for mu in (5.0, 7.0, 9.0, 11.0):
        for kg1 in (300.0, 900.0, 2000.0):
            for kp in (10.0, 40.0, 150.0):
                for kvb in (10.0, 40.0, 120.0):
                    start = [mu, math.log(kg1), math.log(kp), math.log(kvb)]
                    step = [0.8, 0.25, 0.3, 0.35]
                    point, value = nelder_mead(plate_cost, start, step, iters=20000)
                    if value < score:
                        best, score = point, value
    mu, kg1, kp, kvb = best[0], math.exp(best[1]), math.exp(best[2]), math.exp(best[3])
    vp, vs, vg, _, isg = AVERAGE
    kg2 = currents((mu, kg1, kp, kvb, 1.0), vp, vs, vg)[1] / isg
    p = (mu, kg1, kp, kvb, kg2)
    print(f"plate residual {score:.3e}")
    print("PentodeSpec {")
    print(f"    mu: {mu:.4f},")
    print(f"    ex: {EX},")
    print(f"    kg1: {kg1:.3f},")
    print(f"    kp: {kp:.4f},")
    print(f"    kvb: {kvb:.4f},")
    print(f"    kg2: {kg2:.2f},")
    print("}")
    print()
    for name, (vp, vs, vg, ip, isg), note in (
        ("GE average 250/250/-14", AVERAGE, "fitted (screen too)"),
        ("GE class A1 400/225/-16.5", CLASS_A1, "fitted (plate)"),
        ("GE UL 40 % 450/450/-48", ULTRA_LINEAR, "fitted (plate)"),
        ("GE PP pentode 450/310/-29.5", PP_450, "HELD OUT"),
        ("GE PP pentode 600/300/-32.5", PP_600, "HELD OUT"),
    ):
        a, b = currents(p, vp, vs, vg)
        print(
            f"{name:<30} plate {a * 1e3:6.2f} mA (sheet {ip * 1e3:5.1f})"
            f"   screen {b * 1e3:5.2f} mA (sheet {isg * 1e3:4.1f})   {note}"
        )
    vp, vs, vg, ip = CUT_OFF
    cut = currents(p, vp, vs, vg)[0]
    print(f"{'GE cut-off 250/250/-40':<30} plate {cut * 1e3:6.2f} mA (sheet   1.0)   HELD OUT")
    h = 1e-4
    gm = (currents(p, 250, 250, -14 + h)[0] - currents(p, 250, 250, -14 - h)[0]) / (2 * h)
    ga = (currents(p, 250 + h, 250, -14)[0] - currents(p, 250 - h, 250, -14)[0]) / (2 * h)
    print(f"transconductance               {gm * 1e6:.0f} umho (sheet 11000)")
    print(f"plate resistance               {1.0 / ga / 1e3:.1f} kohm (sheet 15, not fitted)")
    # Where the Model T sits on its own chart: not a target, a reading.
    a, b = currents(p, 507.0, 504.0, -53.0)
    print(
        f"{'Model T idle 507/504/-53':<30} plate {a * 1e3:6.2f} mA   screen {b * 1e3:5.2f} mA"
        f"   ({a * 507.0:.1f} W on the plate; GE's design maximum is 42)"
    )
    hot = currents(p, 60.0, 450.0, 0.0)[0]
    print(f"zero bias, 60 V plate, 450 V screen: {hot * 1e3:.0f} mA")


if __name__ == "__main__":
    main()
