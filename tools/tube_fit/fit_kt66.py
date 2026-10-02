"""Koren beam-tetrode constants for the KT66, fitted to Marconi's sheet.

For the Marshall JTM45 (`docs/models/brit_jtm45.md`), whose chart puts its
KT66s at 430 V on the plate and 440 V on the screen.

Two sheets, and they disagree:

  Marconi, "KT66 Output Tetrode" (Frank's electron tube data sheets,
  `sheets/179/k/KT66.pdf`), the original maker's data;
  The M-O Valve Co. (GEC), "KT66 Beam Tetrode", brief data, Issue 5, April
  1977 (`docs/schematics/valves/kt66_mov_brief.pdf`), later production.

Marconi's rows are all cathode-biased with their resistors given, so each row's
grid voltage is its resistor times its current and the set holds together
under one law: the class A point and the AB1 point at 400 V / 300 V give an
amplification factor of 7.4 between them, and at that the AB1 point at 258 V
and the sheet's mutual conductance come out within 2 %. GEC's later sheet does
not join them: its ultra-linear row (425 V on plate and screen, -35 V, 62.5 mA)
asks for twice the amplification factor its own AB1 row allows, and a fit that
took GEC's rows drove the constants to their bounds. Marconi's are the points;
GEC's are held out and reported.

Fitted, five targets for three constants (mu, kg1, kvb; `ex` held at 1.35 like
every other power valve in the catalogue; `kp`, which only softens the cut-off
and which none of these rows reach -- a free fit takes it anywhere from 400 to
860 with the same residual -- held at the 6L6GC's 48, the KT66 being Britain's
6L6), Marconi's unless marked:

  single valve, class A: 250 V on plate and screen, 170 ohm of cathode
  resistor: 85 mA of plate, 6.3 mA of screen -- so -15.5 V;
  its mutual conductance, 6.3 mA/V "at Ea = Es = 250 V, Eg = -15 V";
  two valves, class AB1, no signal: 258 V on plate and screen, 200 ohm each:
  81 mA and 6 mA a valve -- so -17.4 V;
  the same, 400 V plate, 300 V screen, 400 ohm each: 62.5 mA and 2.5 mA a
  valve -- so -26 V;
  GEC's internal resistance at 250 V / 250 V / -15 V, 22.5 kohm (Marconi gives
  none), which sets the knee.

The screen: Koren's screen law has no plate term. A JTM45 runs its screens at
its plates' voltage, which is where Marconi's first two rows are, so `kg2` is
fitted to their 6.3 mA and 6 mA; the AB1 row at 400 V / 300 V, with the plate
well above the screen, draws less (2.5 mA) than the law can give, and is
reported.

Held out, from GEC's 1977 sheet: AB1 tetrode 415 V / 300 V / -27 V, 52 mA;
ultra-linear 425 V / -35 V, 62.5 mA of plate and screen; mutual conductance
7 mA/V at 250 V / 250 V / -15 V; triode connection 7.3 mA/V and 1.3 kohm.

Run: uv run --no-project --with numpy python tools/tube_fit/fit_kt66.py
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


# (plate V, screen V, grid V, plate A, screen A); per valve; Marconi's.
CLASS_A = (250.0, 250.0, -170.0 * 0.0913, 0.085, 0.0063)
AB1_LOW = (258.0, 258.0, -200.0 * 0.087, 0.081, 0.006)
AB1_HIGH = (400.0, 300.0, -400.0 * 0.065, 0.0625, 0.0025)
GM = 6.3e-3
# GEC's, the knee.
RA = 22.5e3
# Held out: GEC 1977.
GEC_AB1 = (415.0, 300.0, -27.0, 0.052)
GEC_UL = (425.0, 425.0, -35.0, 0.0625)


def plate_cost(q):
    mu, kg1, kp, kvb = q[0], math.exp(q[1]), KP, math.exp(q[2])
    if not (3.0 < mu < 16.0 and 100.0 < kg1 < 6000.0 and 5.0 < kvb < 400.0):
        return 1e9
    p = (mu, kg1, kp, kvb, 1e12)
    terms = []
    for vp, vs, vg, ip, _ in (CLASS_A, AB1_LOW, AB1_HIGH):
        terms.append((currents(p, vp, vs, vg)[0] - ip) / ip)
    h = 1e-4
    gm = (currents(p, 250, 250, -15 + h)[0] - currents(p, 250, 250, -15 - h)[0]) / (2.0 * h)
    terms.append((gm - GM) / GM)
    ga = (currents(p, 250 + h, 250, -15)[0] - currents(p, 250 - h, 250, -15)[0]) / (2.0 * h)
    terms.append((1.0 / ga - RA) / RA if ga > 0.0 else 10.0)
    return float(np.sum(np.square(terms)))


def main():
    best, score = None, float("inf")
    for mu in (5.0, 7.0, 9.0, 11.0):
        for kg1 in (300.0, 900.0, 2000.0):
            for kvb in (10.0, 40.0, 120.0):
                start = [mu, math.log(kg1), math.log(kvb)]
                step = [0.8, 0.25, 0.35]
                point, value = nelder_mead(plate_cost, start, step, iters=20000)
                if value < score:
                    best, score = point, value
    mu, kg1, kp, kvb = best[0], math.exp(best[1]), KP, math.exp(best[2])
    # The screen from the two rows with the screen at the plate's voltage.
    unit = (mu, kg1, kp, kvb, 1.0)
    kg2 = 0.5 * sum(currents(unit, *row[:3])[1] / row[4] for row in (CLASS_A, AB1_LOW))
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
    for name, row in (
        ("Marconi class A 250/250/-15.5", CLASS_A),
        ("Marconi AB1 258/258/-17.4", AB1_LOW),
        ("Marconi AB1 400/300/-26", AB1_HIGH),
    ):
        a, b = currents(p, *row[:3])
        print(
            f"{name:<32} plate {a * 1e3:6.2f} mA (sheet {row[3] * 1e3:4.1f})"
            f"   screen {b * 1e3:4.2f} mA (sheet {row[4] * 1e3:.1f})   fitted"
        )
    h = 1e-4
    gm = (currents(p, 250, 250, -15 + h)[0] - currents(p, 250, 250, -15 - h)[0]) / (2 * h)
    ga = (currents(p, 250 + h, 250, -15)[0] - currents(p, 250 - h, 250, -15)[0]) / (2 * h)
    print(f"{'mutual conductance 250/250/-15':<32} {gm * 1e3:.2f} mA/V (Marconi 6.3, GEC 7.0)   fitted to Marconi")
    print(f"{'internal resistance':<32} {1.0 / ga / 1e3:.1f} kohm (GEC 22.5)   fitted")
    a, _ = currents(p, *GEC_AB1[:3])
    print(f"{'GEC AB1 415/300/-27':<32} plate {a * 1e3:6.2f} mA (sheet 52.0)   HELD OUT")
    a, b = currents(p, *GEC_UL[:3])
    print(f"{'GEC UL 425/425/-35':<32} plate+screen {(a + b) * 1e3:6.2f} mA (sheet 62.5)   HELD OUT")

    def triode(v, vg):
        a, b = currents(p, v, v, vg)
        return a + b

    gmt = (triode(250, -15 + h) - triode(250, -15 - h)) / (2 * h)
    gat = (triode(250 + h, -15) - triode(250 - h, -15)) / (2 * h)
    print(f"{'GEC triode connection':<32} {gmt * 1e3:.2f} mA/V (7.3), {1.0 / gat / 1e3:.2f} kohm (1.3)   HELD OUT")
    for vg in (-30.0, -35.0, -40.0, -45.0):
        a, b = currents(p, 430.0, 440.0, vg)
        print(f"JTM45 at 430/440/{vg:.0f}: plate {a * 1e3:5.1f} mA, screen {b * 1e3:4.1f} mA, {a * 430:.1f} W")


if __name__ == "__main__":
    main()
