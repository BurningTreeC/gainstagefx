"""Koren triode constants for the 12AY7, fitted to RCA's data sheet.

RCA 12AY7 "Tentative Data", April 1, 1953, Class A1 characteristics (each unit):
plate 250 V, grid -4 V, amplification factor 40, plate resistance 22,800 ohm,
transconductance 1750 micromho, plate current 3 mA; grid voltage for 10 uA
plate current at 250 V: -11 V. Four targets, four constants (kvb held at 300,
as for the catalogue's other triodes). Run: python3 tools/tube_fit/fit_12ay7.py
"""
import math
import sys
import numpy as np

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
    if not (15 < p[0] < 90 and 1.0 < p[1] < 2.0):
        return 1e9
    h = 1e-3
    i0 = plate(p, 250, -4)
    gm = (plate(p, 250, -4 + h) - plate(p, 250, -4 - h)) / (2 * h)
    ga = (plate(p, 250 + h, -4) - plate(p, 250 - h, -4)) / (2 * h)
    ic = max(plate(p, 250, -11), 1e-12)
    terms = [
        (i0 - 3e-3) / 3e-3,
        (gm - 1.75e-3) / 1.75e-3,
        (1 / max(ga, 1e-12) - 22800) / 22800,
        0.3 * math.log(ic / 10e-6),
    ]
    return sum(t * t for t in terms)


best = None
for mu0 in (35.0, 40.0, 45.0):
    for ex0 in (1.2, 1.35, 1.5):
        for kp0 in (100.0, 300.0, 600.0):
            q, c = nelder_mead(cost, [mu0, ex0, math.log(1000.0), math.log(kp0)], [5, 0.1, 0.5, 0.5])
            if best is None or c < best[0]:
                best = (c, q)
p = unpack(best[1])
h = 1e-3
print("mu %.3f ex %.4f kg1 %.2f kp %.2f kvb %.0f  (cost %.2e)" % (p[0], p[1], p[2], p[3], KVB, best[0]))
print("Ip(250,-4) = %.3f mA  (3.0)" % (plate(p, 250, -4) * 1e3))
print("gm = %.3f mA/V  (1.75)" % ((plate(p, 250, -4 + h) - plate(p, 250, -4 - h)) / (2 * h) * 1e3))
print("rp = %.0f ohm  (22800)" % (1 / ((plate(p, 250 + h, -4) - plate(p, 250 - h, -4)) / (2 * h))))
print("Ip(250,-11) = %.1f uA  (10)" % (plate(p, 250, -11) * 1e6))
