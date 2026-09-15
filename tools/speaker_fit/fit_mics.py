#!/usr/bin/env python3
"""Fit GainStageFX microphone on-axis responses to digitised published charts.

Every curve below was read by eye from the document named beside it (about
+-1 dB on large charts, +-2 dB on small ones). The fitted structure is the
minimum-phase cascade the plugin runs: second-order high-pass, four peaking
sections and a second-order low-pass. See MICROPHONE_MODEL.md.
"""
import math
import numpy as np
from fitlib import nelder_mead, peq_db, lp2_db, hp2_db

CHARTS = {
    # Shure SM57 user guide v3.6 (2025-A), typical response.
    'sm57': {40: -11, 50: -9, 70: -6.5, 100: -4, 150: -1.2, 200: 0, 300: -1, 400: -1.5, 500: -1.5, 700: -0.7, 1000: 0, 1500: 0.2, 2000: 0.5, 3000: 2.3, 4000: 4, 5000: 5.5, 6000: 6.3, 6500: 6, 7000: 3.5, 7500: 3.2, 8000: 4, 9000: 3.5, 9500: 2, 10000: 2.5, 11000: 1.5, 12000: -1, 13500: -4, 15000: -8},
    # Sennheiser MD 421 II product sheet (small plot, bass switch 'M').
    'md421': {30: -6, 50: -4, 100: -3.5, 200: -3, 500: -1.5, 1000: 0, 2000: 1.5, 3000: 3, 4000: 4, 5000: 5, 6000: 4.5, 7000: 3.5, 8000: 3.5, 10000: 2, 12000: 1, 15000: -3, 17000: -6},
    # e906 flat position: SECONDARY source (RecordingHacks description of the Sennheiser chart).
    'e906': {40: -12, 60: -8, 80: -5, 120: -3, 200: -1, 500: 0, 1000: 0, 2000: 1.5, 3000: 3.5, 4200: 5, 6000: 3, 8000: 1, 10000: -1, 12000: -6, 15000: -12, 18000: -18},
    # Sennheiser MD 409 U 3 data sheet 18588/A 02 (1986), 100 cm curve.
    'md409': {50: -10, 70: -8, 100: -6, 150: -3.5, 200: -2, 300: -1, 500: 0, 1000: 0, 2000: 0.3, 3000: 0.5, 4000: 1.2, 5000: 0.5, 6000: 0, 7000: -0.2, 8000: 0.2, 10000: 1.5, 11500: 1.8, 13000: 0, 15000: -5, 17000: -10, 20000: -16},
    # Royer R-121 cut sheet (2002).
    'r121': {20: -4, 30: -2, 40: 0, 60: 1.5, 80: 1, 100: 0.5, 200: 0, 500: 0, 1000: 0.2, 2000: 0.8, 3000: 1, 5000: 0.8, 8000: 0, 10000: -0.5, 15000: -2.5, 18000: -3, 20000: -2},
    # beyerdynamic M 160 data sheet E7 (09.23), 1 m curve.
    'm160': {50: -11, 70: -7, 100: -4, 150: 0.5, 200: 1.2, 300: 0.8, 500: 0, 1000: 0, 2000: 1, 3000: 1.5, 5000: 2, 7000: 2.5, 10000: 0.5, 13000: -1, 15000: -1.2, 18000: -2, 20000: -4},
    # Coles 4038 data sheet (free field).
    'c4038': {30: -1, 40: 0, 50: 0.5, 100: 0.5, 200: 0.3, 500: 0.3, 700: -0.3, 1000: 0.3, 2000: 0.2, 3000: 0.5, 5000: 0.3, 8000: 0, 10000: -0.5, 12000: -0.3, 15000: -2},
    # Neumann U 87 Ai operating manual, cardioid.
    'u87': {20: -3.5, 30: -2, 50: -0.5, 70: 0, 1000: 0, 5000: 0, 6000: 0.3, 8000: 1.5, 9000: 2, 10000: 1.8, 12000: 0.5, 15000: -1.5, 20000: -5},
    # APPROXIMATED: published range/pattern only, chart not retrieved.
    'c414': {20: -2, 40: 0, 1000: 0, 5000: 0.5, 8000: 1, 12000: 2, 16000: 0, 20000: -3},
    'u67': {20: -4, 40: -1.5, 100: 0, 1000: 0, 3000: 0.5, 6000: 1.5, 9000: 3, 12000: 2, 15000: 0, 20000: -6},
    'u47fet': {30: -4, 40: -2, 60: 0, 150: 0.5, 1000: 0, 4000: 0.5, 8000: 2, 10000: 2.5, 13000: 1, 16000: -3, 20000: -10},
}

NPK = 4

def model(f, p):
    hpf, hpq = math.exp(p[0]), math.exp(p[1])
    y = hp2_db(f, hpf, hpq)
    for i in range(NPK):
        y = y + peq_db(f, math.exp(p[2 + 3 * i]), p[3 + 3 * i], math.exp(p[4 + 3 * i]))
    lpf, lpq = math.exp(p[2 + 3 * NPK]), math.exp(p[3 + 3 * NPK])
    return y + lp2_db(f, lpf, lpq)

def fit(name, chart):
    fp = np.array(sorted(chart), float)
    yp = np.array([chart[k] for k in sorted(chart)], float)
    fg = np.geomspace(fp[0], fp[-1], 160)
    yg = np.interp(np.log(fg), np.log(fp), yp)
    def err(p):
        pen = 0
        if not (5 < math.exp(p[0]) < 400 and 0.3 < math.exp(p[1]) < 2): pen += 1e3
        for i in range(NPK):
            fc, g, q = math.exp(p[2 + 3 * i]), p[3 + 3 * i], math.exp(p[4 + 3 * i])
            if not (60 < fc < 16000 and -9 < g < 9 and 0.4 < q < 8): pen += 1e3
            pen += 0.004 * g * g
        if not (6000 < math.exp(p[2 + 3 * NPK]) < 40000 and 0.3 < math.exp(p[3 + 3 * NPK]) < 2.5): pen += 1e3
        return float(np.mean((model(fg, p) - yg) ** 2)) + pen
    best = None
    for hp in (20, 60, 110):
        for lp in (12000, 16000, 22000):
            st = [math.log(hp), math.log(0.7),
                  math.log(400), -1, math.log(1),
                  math.log(3000), 2, math.log(1),
                  math.log(6000), 3, math.log(2),
                  math.log(10000), 1, math.log(2),
                  math.log(lp), math.log(0.8)]
            p, e = nelder_mead(err, st, [0.3, 0.2] + [0.3, 1.5, 0.3] * NPK + [0.2, 0.2], iters=5000)
            for _ in range(4):
                p, e = nelder_mead(err, p, [0.1, 0.1] + [0.15, 0.7, 0.15] * NPK + [0.1, 0.1], iters=5000)
            if best is None or e < best[1]:
                best = (p, e)
    p = best[0]
    rms = float(np.sqrt(np.mean((model(fg, p) - yg) ** 2)))
    peaks = [(math.exp(p[2 + 3 * i]), p[3 + 3 * i], math.exp(p[4 + 3 * i])) for i in range(NPK)]
    print(f"{name}: rms {rms:.2f} dB  hp=({math.exp(p[0]):.1f}, {math.exp(p[1]):.3f}) "
          f"peaks=[{', '.join(f'({fc:.0f}, {g:.2f}, {q:.2f})' for fc, g, q in sorted(peaks))}] "
          f"lp=({math.exp(p[2 + 3 * NPK]):.0f}, {math.exp(p[3 + 3 * NPK]):.3f})")

if __name__ == '__main__':
    for name, chart in CHARTS.items():
        fit(name, chart)
