"""Shared numerical helpers for the GainStageFX fitting scripts."""
import math
import numpy as np


def nelder_mead(f, x0, step, iters=4000, tol=1e-10):
    n = len(x0)
    pts = [np.array(x0, float)]
    for i in range(n):
        p = np.array(x0, float); p[i] += step[i]; pts.append(p)
    vals = [f(p) for p in pts]
    for _ in range(iters):
        order = np.argsort(vals); pts = [pts[i] for i in order]; vals = [vals[i] for i in order]
        if abs(vals[-1] - vals[0]) < tol: break
        cen = np.mean(pts[:-1], axis=0)
        xr = cen + (cen - pts[-1]); fr = f(xr)
        if fr < vals[0]:
            xe = cen + 2 * (cen - pts[-1]); fe = f(xe)
            if fe < fr: pts[-1], vals[-1] = xe, fe
            else: pts[-1], vals[-1] = xr, fr
        elif fr < vals[-2]:
            pts[-1], vals[-1] = xr, fr
        else:
            xc = cen + 0.5 * (pts[-1] - cen); fc = f(xc)
            if fc < vals[-1]: pts[-1], vals[-1] = xc, fc
            else:
                for i in range(1, len(pts)):
                    pts[i] = pts[0] + 0.5 * (pts[i] - pts[0]); vals[i] = f(pts[i])
    i = int(np.argmin(vals)); return pts[i], vals[i]


def peq_db(f, fc, g, q):
    s = 1j * f / fc
    A = 10 ** (g / 40)
    h = (s * s + s * (A / q) + 1) / (s * s + s / (A * q) + 1)
    return 20 * np.log10(np.abs(h))


def lp2_db(f, fc, q):
    s = 1j * f / fc
    return 20 * np.log10(np.abs(1 / (s * s + s / q + 1)))


def hp2_db(f, fc, q):
    s = 1j * f / fc
    return 20 * np.log10(np.abs(s * s / (s * s + s / q + 1)))
