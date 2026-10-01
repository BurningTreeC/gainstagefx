"""Koren pentode constants for the EF804S, fitted to Telefunken's data sheet.

Telefunken "EF804S", sheet 331-332 (Frank's electron tube data sheets,
`sheets/128/e/EF804S.pdf`):

  measuring values: plate supply 250 V, screen 140 V, cathode resistor 500
  ohm, the grid at ground: plate current 3.2 mA, screen current 0.6 mA,
  transconductance 2.0 mA/V, internal resistance 2 Mohm, amplification
  factor grid 2 to grid 1 38;

  typical operation as a resistance-coupled amplifier, plate resistor Ra,
  screen resistor Rg2 from the supply, cathode resistor Rk, the grid at ground:
  250 V / 0.3 M / 1.5 M / 2 k: 0.61 mA plate, 0.11 mA screen;
  250 V / 0.2 M / 1.0 M / 1.5 k: 0.87, 0.16;
  100 V / 0.3 M / 1.2 M / 5 k: 0.21, 0.045;
  100 V / 0.2 M / 1.0 M / 3 k: 0.29, 0.055.
  (The two grid-leak-biased rows, Rk 0 with 10 M, are left out: their bias is
  the grid's own contact current, which the model does not have.)

`mu` is held at the sheet's 38, the screen's amplification factor, which is
what Koren's `mu` is. Fitted: kg1, kp, kvb, kg2 and ex against the measuring
point's three figures and the four rows' plate and screen currents. Held out:
the internal resistance. The Telefunken V76 runs three of these, and its
drawing's node voltages are the check (`docs/models/german_76.md`).

Fitted 2026-10-01: residual 5.5e-3; the measuring point to 0.5 % (3.20 mA, 0.603
mA, 1.99 mA/V), the four rows' plate currents within 3 % and screens within 9 %.
Internal resistance 12 Mohm against the sheet's 2 (not fitted).

Run: python3 tools/tube_fit/fit_ef804s.py
"""
import math
import sys
import numpy as np

sys.path.insert(0, "tools/speaker_fit")
sys.path.insert(0, "tools/tube_fit")
from fitlib import nelder_mead  # noqa: E402
from smallpentode import currents, stage  # noqa: E402

MU = 38.0
# plate supply, screen supply, cathode resistor; plate, screen, transconductance
MEASURING = (250.0, 140.0, 500.0, 3.2e-3, 0.6e-3, 2.0e-3)
TABLE = [  # supply, Ra, Rg2, Rk, plate, screen
    (250.0, 0.3e6, 1.5e6, 2.0e3, 0.61e-3, 0.11e-3),
    (250.0, 0.2e6, 1.0e6, 1.5e3, 0.87e-3, 0.16e-3),
    (100.0, 0.3e6, 1.2e6, 5.0e3, 0.21e-3, 0.045e-3),
    (100.0, 0.2e6, 1.0e6, 3.0e3, 0.29e-3, 0.055e-3),
]


def unpack(q):
    return (MU, math.exp(q[0]), math.exp(q[1]), math.exp(q[2]), math.exp(q[3])), q[4]


def measuring_point(p, ex):
    """The self-biased measuring point: the cathode carries both currents."""
    vb, vs, rk, *_ = MEASURING
    lo, hi = 0.0, 10.0
    for _ in range(50):
        vk = 0.5 * (lo + hi)
        a, b = currents(p, vb - vk, vs - vk, -vk, ex)
        if (a + b) * rk > vk:
            lo = vk
        else:
            hi = vk
    vk = 0.5 * (lo + hi)
    a, b = currents(p, vb - vk, vs - vk, -vk, ex)
    h = 1e-4
    gm = (currents(p, vb - vk, vs - vk, -vk + h, ex)[0] - currents(p, vb - vk, vs - vk, -vk - h, ex)[0]) / (2 * h)
    ga = (currents(p, vb - vk + h, vs - vk, -vk, ex)[0] - currents(p, vb - vk - h, vs - vk, -vk, ex)[0]) / (2 * h)
    return a, b, gm, 1.0 / max(ga, 1e-15), vk


def cost(q):
    p, ex = unpack(q)
    _, kg1, kp, kvb, kg2 = p
    if not (1.1 < ex < 1.8 and 20.0 < kg1 < 1e5 and 1.0 < kp < 2000.0 and 0.5 < kvb < 2000.0 and 100.0 < kg2 < 1e6):
        return 1e9
    a, b, gm, _, _ = measuring_point(p, ex)
    _, _, _, ia, ig2, gm_want = MEASURING
    terms = [(a - ia) / ia, (b - ig2) / ig2, (gm - gm_want) / gm_want]
    for vb, ra, rg2, rk, pa, ps in TABLE:
        x, y = stage(p, ex, vb, ra, rg2, rk)
        terms += [(x - pa) / pa, 0.5 * (y - ps) / ps]
    return float(np.sum(np.square(terms)))


def main():
    best, score = None, float("inf")
    for kg1, kp, kvb in ((800.0, 230.0, 25.0), (2000.0, 100.0, 30.0), (600.0, 400.0, 10.0)):
        start = [math.log(kg1), math.log(kp), math.log(kvb), math.log(3000.0), 1.3]
        point, value = nelder_mead(cost, start, [0.4, 0.4, 0.5, 0.4, 0.05], iters=2000)
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
    a, b, gm, ri, vk = measuring_point(p, ex)
    print(f"measuring point: {a * 1e3:.2f} mA (3.2), screen {b * 1e3:.3f} mA (0.6), {gm * 1e3:.2f} mA/V (2.0), "
          f"ri {ri / 1e6:.2f} Mohm (2, not fitted), cathode {vk:.2f} V")
    for vb, ra, rg2, rk, pa, ps in TABLE:
        x, y = stage(p, ex, vb, ra, rg2, rk)
        print(f"  {vb:.0f} V / {ra / 1e6:.1f} M / {rg2 / 1e6:.1f} M / {rk / 1e3:.1f} k: "
              f"{x * 1e3:.3f} mA ({pa * 1e3:.3f}), screen {y * 1e3:.3f} ({ps * 1e3:.3f})")


if __name__ == "__main__":
    main()
