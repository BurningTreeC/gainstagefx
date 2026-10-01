"""Koren triode constants for the 12BH7-A, fitted to RCA's data sheet.

RCA 12BH7-A "Tentative Data", March 1, 1955, Class A1 characteristics (each
unit): plate 250 V, grid -10.5 V, amplification factor 16.5, plate resistance
5300 ohm, transconductance 3100 micromho, plate current 11.5 mA; plate current
at -14 V: 4 mA; grid voltage for 50 uA plate current: -23 V. Five targets, four
constants (kvb held at 300, as for the catalogue's other triodes), so the fit
is over-determined by one and the residuals say how well Koren's law holds.
The Ampeg SVT's drivers. Run: python3 tools/tube_fit/fit_12bh7.py
"""
import math
import sys

sys.path.insert(0, "tools/speaker_fit")
from fitlib import nelder_mead  # noqa: E402

KVB = 300.0


def plate(p, vp, vg):
    mu, ex, kg1, kp = p
    inner = kp * (1.0 / mu + vg / math.sqrt(KVB + vp * vp))
    soft = inner if inner > 30 else math.log1p(math.exp(inner))
    e1 = vp / kp * soft
    return 2.0 * e1 ** ex / kg1 if e1 > 0 else 0.0


def unpack(q):
    return [q[0], q[1], math.exp(q[2]), math.exp(q[3])]


def cost(q):
    p = unpack(q)
    if not (8 < p[0] < 40 and 1.0 < p[1] < 2.0):
        return 1e9
    h = 1e-3
    i0 = plate(p, 250, -10.5)
    gm = (plate(p, 250, -10.5 + h) - plate(p, 250, -10.5 - h)) / (2 * h)
    ga = (plate(p, 250 + h, -10.5) - plate(p, 250 - h, -10.5)) / (2 * h)
    i14 = plate(p, 250, -14)
    ic = max(plate(p, 250, -23), 1e-12)
    terms = [
        (i0 - 11.5e-3) / 11.5e-3,
        (gm - 3.1e-3) / 3.1e-3,
        (1 / max(ga, 1e-12) - 5300) / 5300,
        (i14 - 4e-3) / 4e-3,
        0.3 * math.log(ic / 50e-6),
    ]
    return sum(t * t for t in terms)


best = None
for mu0 in (14.0, 17.0, 20.0):
    for ex0 in (1.2, 1.35, 1.5):
        for kp0 in (50.0, 150.0, 400.0):
            q, c = nelder_mead(cost, [mu0, ex0, math.log(1000.0), math.log(kp0)], [3, 0.1, 0.5, 0.5])
            if best is None or c < best[0]:
                best = (c, q)
p = unpack(best[1])
h = 1e-3
print("mu %.3f ex %.4f kg1 %.2f kp %.2f kvb %.0f  (cost %.2e)" % (p[0], p[1], p[2], p[3], KVB, best[0]))
print("Ip(250,-10.5) = %.2f mA  (11.5)" % (plate(p, 250, -10.5) * 1e3))
print("gm = %.3f mA/V  (3.1)" % ((plate(p, 250, -10.5 + h) - plate(p, 250, -10.5 - h)) / (2 * h) * 1e3))
print("rp = %.0f ohm  (5300)" % (1 / ((plate(p, 250 + h, -10.5) - plate(p, 250 - h, -10.5)) / (2 * h))))
print("mu = %.1f  (16.5)" % ((plate(p, 250, -10.5 + h) - plate(p, 250, -10.5 - h)) / (plate(p, 250 + h, -10.5) - plate(p, 250 - h, -10.5))))
print("Ip(250,-14) = %.2f mA  (4)" % (plate(p, 250, -14) * 1e3))
print("Ip(250,-23) = %.1f uA  (50)" % (plate(p, 250, -23) * 1e6))
