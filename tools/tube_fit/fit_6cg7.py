"""Koren triode constants for the 6CG7, fitted to Tung-Sol's data sheet.

Tung-Sol "6CG7 Tentative Data", December 1, 1959 (Frank's electron tube data
sheets, `sheets/127/6/6CG7.pdf`), class A1, each unit:

  90 V, 0 V grid: amplification factor 20, plate resistance 6700 ohm,
  transconductance 3000 micromho, 10 mA; 10 microamps at -7 V;
  250 V, -8 V: amplification factor 20, 7700 ohm, 2600 micromho, 9 mA;
  10 microamps at -18 V; 1.3 mA at -12.5 V.

The Ampeg VT-40's reverb driver (`docs/models/american_vt40.md`). Fitted to
the two operating points' currents and transconductances, the 250 V point's
plate resistance and the 1.3 mA at -12.5 V: six targets for four constants
(`kvb` held at 300, as for the catalogue's other triodes). Held out: the 90 V
point's plate resistance and the two cut-off voltages.

Run: uv run --no-project python tools/tube_fit/fit_6cg7.py
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


def slopes(p, vp, vg, h=1e-3):
    gm = (plate(p, vp, vg + h) - plate(p, vp, vg - h)) / (2 * h)
    ga = (plate(p, vp + h, vg) - plate(p, vp - h, vg)) / (2 * h)
    return gm, ga


def cost(q):
    p = unpack(q)
    if not (10 < p[0] < 40 and 1.0 < p[1] < 2.0):
        return 1e9
    gm90, _ = slopes(p, 90, 0.0)
    gm250, ga250 = slopes(p, 250, -8.0)
    terms = [
        (plate(p, 90, 0.0) - 10e-3) / 10e-3,
        (gm90 - 3.0e-3) / 3.0e-3,
        (plate(p, 250, -8.0) - 9e-3) / 9e-3,
        (gm250 - 2.6e-3) / 2.6e-3,
        (1 / max(ga250, 1e-12) - 7700) / 7700,
        (plate(p, 250, -12.5) - 1.3e-3) / 1.3e-3,
    ]
    return sum(t * t for t in terms)


best = None
for mu0 in (16.0, 20.0, 24.0):
    for ex0 in (1.2, 1.35, 1.5):
        for kp0 in (50.0, 150.0, 400.0):
            for kg0 in (500.0, 1500.0):
                q, c = nelder_mead(cost, [mu0, ex0, math.log(kg0), math.log(kp0)], [3, 0.1, 0.5, 0.5])
                if best is None or c < best[0]:
                    best = (c, q)
p = unpack(best[1])
print("mu %.4f ex %.4f kg1 %.3f kp %.3f kvb %.0f  (cost %.2e)" % (p[0], p[1], p[2], p[3], KVB, best[0]))
for vp, vg, want in ((90, 0.0, 10.0), (250, -8.0, 9.0), (250, -12.5, 1.3)):
    print("Ip(%d, %.1f) = %.2f mA  (%.1f)" % (vp, vg, plate(p, vp, vg) * 1e3, want))
for vp, vg, gm_s, ra_s in ((90, 0.0, 3.0, 6700), (250, -8.0, 2.6, 7700)):
    gm, ga = slopes(p, vp, vg)
    print("at %d V, %.1f V: gm %.3f mA/V (%.1f), ra %.0f ohm (%d), mu %.1f (20)" % (vp, vg, gm * 1e3, gm_s, 1 / ga, ra_s, gm / ga))
for vp, vg in ((90, -7.0), (250, -18.0)):
    print("cut-off: Ip(%d, %.0f) = %.1f uA  (10)  HELD OUT" % (vp, vg, plate(p, vp, vg) * 1e6))
