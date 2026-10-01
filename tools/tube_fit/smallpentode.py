"""Shared pieces for fitting Koren's pentode to a small-signal valve's sheet.

A small-signal pentode's sheet gives one fixed operating point and a table of
resistance-coupled stages: a supply, a plate resistor, a screen resistor from
the same supply, a cathode resistor with the grid at ground, and the cathode
current (or plate and screen currents) the stage settles at. Each row is a
self-consistent operating point, which `stage` solves for with the model's
own currents, so the table is a fit target as it stands.
"""
import math


def currents(p, vp, vs, vg, ex):
    """Plate and screen current, in the exact form `dsp::device` evaluates."""
    mu, kg1, kp, kvb, kg2 = p
    if vs <= 0.0 or vp <= 0.0:
        return 0.0, 0.0
    inner = kp * (1.0 / mu + vg / vs)
    soft = inner if inner > 30.0 else math.log1p(math.exp(inner))
    e1 = vs / kp * soft
    if e1 <= 0.0:
        return 0.0, 0.0
    powered = e1 ** ex
    return powered / kg1 * math.atan(vp / kvb), powered / kg2


def stage(p, ex, vb, ra, rg2, rk, screen_supply=None):
    """The resistance-coupled stage's operating point: (plate current, screen
    current), with the grid at ground and the cathode on `rk`. The screen is
    fed through `rg2` from `screen_supply` (the plate supply when `None`).

    Solved by bisection on the cathode voltage, and for each cathode voltage
    by bisection on the screen's, since the screen current depends on its own
    voltage and the plate's does not feed back into it."""
    vs_supply = vb if screen_supply is None else screen_supply

    def at_cathode(vk):
        # The screen: v = vs_supply - Ig2(v) rg2, monotonic in v.
        lo, hi = 0.0, vs_supply
        for _ in range(40):
            v = 0.5 * (lo + hi)
            ig2 = currents(p, max(vb - vk, 1e-6), v - vk, -vk, ex)[1]
            if v > vs_supply - ig2 * rg2:
                hi = v
            else:
                lo = v
        vs = 0.5 * (lo + hi)
        # The plate: v = vb - Ia(v) ra, monotonic too.
        lo, hi = vk, vb
        for _ in range(40):
            v = 0.5 * (lo + hi)
            ia = currents(p, v - vk, vs - vk, -vk, ex)[0]
            if v > vb - ia * ra:
                hi = v
            else:
                lo = v
        va = 0.5 * (lo + hi)
        ia, ig2 = currents(p, va - vk, vs - vk, -vk, ex)
        return ia, ig2

    if rk <= 0.0:
        return at_cathode(0.0)
    lo, hi = 0.0, 20.0
    for _ in range(40):
        vk = 0.5 * (lo + hi)
        ia, ig2 = at_cathode(vk)
        if (ia + ig2) * rk > vk:
            lo = vk
        else:
            hi = vk
    return at_cathode(0.5 * (lo + hi))
