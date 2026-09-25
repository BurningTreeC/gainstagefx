#!/usr/bin/env python3
"""Offline speaker-parameter analysis for GainStageFX.

Inputs: Jensen manufacturer T/S + numeric SPL/impedance series (jensentone.com
spec sheets/charts), Celestion manufacturer Fs/Re/plots (digitised by eye from
celestion.com brochure PDFs, +-1.5 dB).

Outputs: fitted voice-coil semi-inductance, inferred Celestion Qts/Mms/Bl, and
breakup-resonator voicing. Printed as a Rust-ready table.
"""
import json, math
import numpy as np

RHO, C = 1.204, 343.0
P0 = 20e-6

from fitlib import nelder_mead, peq_db, lp2_db  # noqa: E402

def derived(ts):
    ws = 2 * math.pi * ts['fs']
    mms = ts['mms']; bl = ts['bl']; re = ts['re']
    cms = 1 / (ws * ws * mms)
    rms = ws * mms / ts['qms']
    return dict(cms=cms, rms=rms, res=bl * bl / rms, lces=bl * bl * cms, cmes=mms / (bl * bl))

def z_elec(ts, f, l1, r1, l2, r2):
    # Voice coil as two lossy inductances (eddy-current model): bounded above the band.
    w = 2 * math.pi * f; s = 1j * w
    d = derived(ts)
    zmot = 1 / (1 / d['res'] + 1 / (s * d['lces']) + s * d['cmes'])
    zvc = ts['re'] + (s * l1 * r1) / (r1 + s * l1) + (s * l2 * r2) / (r2 + s * l2)
    return zvc + zmot, zvc

def spl_infinite_baffle(ts, f, v, l1, r1, l2, r2, r=1.0):
    w = 2 * math.pi * f; s = 1j * w
    z, zvc = z_elec(ts, f, l1, r1, l2, r2)
    i = v / z
    d = derived(ts)
    zm = d['rms'] + s * ts['mms'] + 1 / (s * d['cms'])
    u = ts['bl'] * i / zm
    p = RHO * ts['sd'] * s * u / (2 * math.pi * r)  # half space
    return 20 * np.log10(np.abs(p) / P0)

def iec_baffle(f, dmin, dmax, n=24):
    # Rear wave around a finite open baffle: average path differences.
    k = 2 * math.pi * f / C
    h = 0
    for dl in np.linspace(dmin, dmax, n):
        h = h + (1 - np.exp(-1j * k * dl))
    return 20 * np.log10(np.abs(h / n) + 1e-12)

def smooth_octave(f, y, frac=6):
    out = np.empty_like(y)
    for i, fc in enumerate(f):
        m = (f >= fc * 2 ** (-0.5 / frac)) & (f <= fc * 2 ** (0.5 / frac))
        out[i] = 10 * np.log10(np.mean(10 ** (y[m] / 10)))
    return out

# ---------------------------------------------------------------------------
jensen = json.load(open('jensen_fr.json'))
JTS = {
    # published 8 ohm data (jensentone specification sheets)
    'p12r': dict(re=6.05, fs=80, qms=13.07, qes=2.50, mms=25.2e-3, bl=5.53, sd=490.9e-4, le=0.5e-3, nom=8),
    'p10r': dict(re=6.68, fs=97, qms=14.83, qes=1.82, mms=15.2e-3, bl=5.83, sd=330.1e-4, le=0.54e-3, nom=8),
    'c12n': dict(re=6.05, fs=113, qms=7.52, qes=1.18, mms=29.9e-3, bl=10.46, sd=490.9e-4, le=0.9e-3, nom=8),
    'p12n': dict(re=6.03, fs=90, qms=4.36, qes=0.94, mms=30.9e-3, bl=10.62, sd=490.9e-4, le=0.87e-3, nom=8),
    'p12q': dict(re=5.6, fs=90.5, qms=11.57, qes=2.46, mms=26.8e-3, bl=5.88, sd=490.9e-4, le=0.67e-3, nom=8),
}

def series(model, kind):
    x, y = jensen[f'{model}|{kind} - 8 Ω']
    return np.array(x), np.array(y)

print('== Jensen consistency: Qes from Bl/Mms/Re, Zmax')
for m, ts in JTS.items():
    qes = 2 * math.pi * ts['fs'] * ts['mms'] * ts['re'] / ts['bl'] ** 2
    fz, zz = series(m, 'Impedance')
    print(f"{m}: Qes calc {qes:.3f} pub {ts['qes']}; Zmax calc {ts['re']*(1+ts['qms']/qes):.1f} meas {zz[fz<400].max():.1f} at {fz[np.argmax(np.where(fz<400,zz,0))]:.0f} Hz")

print('\n== Semi-inductance fit (magnitude, 250 Hz..20 kHz, log error)')
fits = {}
for m, ts in JTS.items():
    fz, zz = series(m, 'Impedance')
    sel = (fz > 250)
    def err(p):
        l1, r1, l2, r2 = np.exp(p)
        z, _ = z_elec(ts, fz[sel], l1, r1, l2, r2)
        return float(np.mean((np.log(np.abs(z)) - np.log(zz[sel])) ** 2))
    best, e = None, None
    for st in ([0.3e-3, 60.0, 0.8e-3, 6.0], [0.5e-3, 30.0, 1.5e-3, 10.0], [0.2e-3, 150.0, 1e-3, 5.0]):
        p, pe = nelder_mead(err, np.log(st), [0.5] * 4, iters=6000)
        for _ in range(4):
            p, pe = nelder_mead(err, p, [0.2] * 4, iters=6000)
        if e is None or pe < e:
            best, e = p, pe
    l1, r1, l2, r2 = np.exp(best)
    fits[m] = (l1, r1, l2, r2)
    z, _ = z_elec(ts, fz, l1, r1, l2, r2)
    pure, _ = z_elec(ts, fz, 1e-12, 1e-9, ts['le'], 1e12)
    idx = [np.argmin(abs(fz - f)) for f in (80, 1000, 5000, 10000, 20000)]
    print(f"{m}: L1={l1*1e3:.4f} mH R1={r1:.2f} L2={l2*1e3:.4f} mH R2={r2:.3f} ohm rms-log-err={math.sqrt(e):.4f}")
    print('   meas', [f'{zz[i]:.1f}' for i in idx], ' fit', [f'{abs(z[i]):.1f}' for i in idx], ' pureLe', [f'{abs(pure[i]):.1f}' for i in idx])

print('\n== IEC baffle model calibration on Jensen SPL (40..400 Hz)')
def baffle_err(p, models=JTS):
    dmin, dmax = p
    if dmin < 0.2 or dmax < dmin + 0.05 or dmax > 3: return 1e9
    tot = 0
    for m, ts in models.items():
        fx, sp = series(m, 'Sound Pressure')
        sel = (fx >= 40) & (fx <= 400)
        coil = fits[m]
        v = math.sqrt(ts['nom'])  # 1 W into nominal
        model = spl_infinite_baffle(ts, fx[sel], v, *coil) + iec_baffle(fx[sel], dmin, dmax)
        ref = (fx >= 150) & (fx <= 400)
        off = np.mean(sp[ref] - (spl_infinite_baffle(ts, fx[ref], v, *coil) + iec_baffle(fx[ref], dmin, dmax)))
        tot += np.mean((sp[sel] - model - off) ** 2)
    return tot / len(models)
bb, be = nelder_mead(baffle_err, [0.6, 1.2], [0.2, 0.3])
print(f"baffle path distribution {bb[0]:.3f}..{bb[1]:.3f} m, rms {math.sqrt(be):.2f} dB")
for m, ts in JTS.items():
    fx, sp = series(m, 'Sound Pressure')
    v = math.sqrt(ts['nom'])
    mod = spl_infinite_baffle(ts, fx, v, *fits[m])
    ref = (fx >= 500) & (fx <= 1000)
    print(f"{m}: pistonic sens model {np.mean(mod[ref]):.1f} dB  measured {np.mean(sp[ref]):.1f} dB (500-1000 Hz)")

json.dump(dict(fits={k: list(v) for k, v in fits.items()}, baffle=list(bb)), open('fit_state.json', 'w'))

# ---------------------------------------------------------------------------
# Celestion: published Fs / Re / response plot only. Digitised plot points.
CEL = {
    'v30': dict(re=7.3, fs=75, sd=490.9e-4, nom=8, plot={20:77,30:81,40:83.5,50:85,60:87,70:89,80:90,100:91.5,150:95,200:96.5,300:98,400:98.5,500:99,600:100,700:100.5,800:100.5,1000:100.5,1200:99.5,1500:98.5,1800:100.5,2000:102.5,2300:106,2600:101.5,3000:105,3500:104,4000:103,4500:100,5000:92,5500:86,6000:83,7000:79,8000:78,10000:74,12000:76,15000:79,20000:73}),
    'g12m25': dict(re=6.7, fs=75, sd=490.9e-4, nom=8, plot={20:69,30:75,40:79,50:82,60:85,70:87,80:88,100:90.5,150:94,200:96,300:97,400:97,500:96.5,600:96,700:96,800:97.5,1000:99,1200:98,1500:96,1800:99,2000:102,2300:105,2600:100,3000:106,3500:103,4000:102,4500:100,5000:90,5500:82,6000:78,7000:79,8000:79,10000:76,12000:74,15000:72,20000:73}),
    'g12t75': dict(re=6.77, fs=85, sd=490.9e-4, nom=8, plot={20:70,30:77,40:80,50:81,60:82,70:83.5,80:85,100:88,150:95,200:97,300:96,400:97,500:98,700:98.5,900:97,1000:96,1200:93,1500:98,1800:103,2000:106,2300:107,2600:105,3000:106,3500:103,4000:101,4500:99,5000:92,6000:85,7000:84,8000:82,10000:81,12000:74,15000:71,20000:78}),
    # The G12K-85, from Celestion's own data for the G12K-100 (Legacy series): Re, Fs
    # and its 8 ohm plot, https://celestion.com/wp-content/uploads/2019/09/G12K-100-copy.jpg.
    # Not by eye: the red trace's pixels against the plot's own grid lines (decades at
    # x = 240/493/745, 5 dB per 25.2 px), sampled every 1/12 octave. See
    # docs/models/speakers.md for why the G12K-100 stands for the G12K-85.
    'g12k85': dict(re=7.0, fs=85, sd=490.9e-4, nom=8, plot={21.2:76.6,22.4:77.8,23.8:79.3,25.2:80.4,26.7:81.2,28.3:82.2,30:82.8,31.7:83.2,33.6:83.8,35.6:84.0,37.8:84.6,40:85.3,42.4:86.1,44.9:86.9,47.6:87.5,50.4:87.9,53.4:88.7,56.6:89.7,59.9:90.5,63.5:91.1,67.3:91.3,71.3:91.3,75.5:91.5,80:91.7,84.8:92.1,89.8:92.7,95.1:93.7,100.8:94.3,106.8:94.7,113.1:94.9,119.9:95.3,127:95.7,134.5:95.9,142.5:96.3,151:96.7,160:97.1,169.5:97.1,179.6:97.1,190.3:97.5,201.6:97.7,213.6:97.9,226.3:98.1,239.7:98.1,254:98.5,269.1:98.7,285.1:98.5,302:98.3,320:98.1,339:97.7,359.2:97.7,380.5:98.1,403.2:98.1,427.1:98.5,452.5:99.1,479.5:99.5,508:99.9,538.2:100.3,570.2:100.3,604.1:100.3,640:100.9,678.1:100.9,718.4:100.7,761.1:100.3,806.3:99.5,854.3:99.1,905.1:99.7,958.9:99.5,1015.9:99.1,1076.3:99.1,1140.4:98.5,1208.2:97.9,1280:96.7,1356.1:96.1,1436.8:96.3,1522.2:98.4,1612.7:100.6,1708.6:101.5,1810.2:102.1,1917.8:103.0,2031.9:103.8,2152.7:105.8,2280.7:107.6,2416.3:107.4,2560:106.2,2712.2:106.4,2873.5:106.2,3044.4:107.1,3225.4:107.6,3417.2:106.2,3620.4:104.2,3835.7:103.6,4063.7:102.5,4305.4:100.1,4561.4:98.5,4832.6:98.9,5120:98.7,5424.5:97.8,5747:95.0,6088.7:88.9,6450.8:83.5,6834.4:78.6,7240.8:79.8,7671.3:81.0,8127.5:79.1,8610.8:78.8,9122.8:79.6,9665.3:78.3,10240:78.7,10848.9:81.0,11494:79.2,12177.5:81.0,12901.6:79.5,13668.8:73.1,14481.5:69.4,15342.7:67.3,16255:72.8,17221.6:81.4,18245.6:79.8,19330.5:78.6}),
}
SEMI_CERAMIC = tuple(np.mean([fits['c12n'], fits['p12n']], axis=0))
SENS_OFFSET = None  # measured - model, Jensen
JOFF=[]
for m, ts in JTS.items():
    fx, sp = series(m, 'Sound Pressure'); r=(fx>=500)&(fx<=1000)
    JOFF.append(float(np.mean(sp[r]-spl_infinite_baffle(ts, fx[r], math.sqrt(8), *fits[m]))))
SENS_OFFSET=float(np.mean(JOFF)); print('Jensen 500-1000 Hz offsets', [round(x,2) for x in JOFF])
print(f"\n== Celestion inference (semi-inductance from Jensen 38 mm ceramic/alnico mean {SEMI_CERAMIC}), sensitivity offset {SENS_OFFSET:.2f} dB")
cel_ts = {}
# Priors: medians of published 12-inch guitar-driver data (Jensen P12R/P12Q/C12N/P12N,
# Eminence Legend GB128, WGS Retro 30/Green Beret manufacturer tables).
MMS_PRIOR = float(np.median([25.2, 26.8, 29.9, 30.9, 28.0, 27.48, 29.2])) * 1e-3
QMS_PRIOR = float(np.median([13.07, 11.57, 7.52, 4.36, 19.54, 9.73, 9.52]))
print(f"Mms prior {MMS_PRIOR*1e3:.2f} g, Qms prior {QMS_PRIOR:.2f}")
for m, c in CEL.items():
    fp = np.array(sorted(c['plot'])); sp = np.array([c['plot'][k] for k in fp])
    fg = np.geomspace(20, 20000, 240)
    sg = np.interp(np.log(fg), np.log(fp), sp)
    ref = (fg >= 500) & (fg <= 1000)
    lf = (fg >= 40) & (fg <= 300)
    ts = dict(re=c['re'], fs=c['fs'], qms=QMS_PRIOR, mms=MMS_PRIOR, sd=c['sd'], bl=10.0, nom=8)
    lo, hi = 2.0, 40.0
    for _ in range(60):
        ts['bl'] = 0.5 * (lo + hi)
        lvl = np.mean(spl_infinite_baffle(ts, fg[ref], math.sqrt(8), *SEMI_CERAMIC) + iec_baffle(fg[ref], *bb)) + SENS_OFFSET
        if lvl < np.mean(sg[ref]): lo = ts['bl']
        else: hi = ts['bl']
    qes = 2 * math.pi * c['fs'] * ts['mms'] * c['re'] / ts['bl'] ** 2
    qts = ts['qms'] * qes / (ts['qms'] + qes)
    cms = 1 / ((2 * math.pi * c['fs']) ** 2 * ts['mms'])
    vas = RHO * C * C * c['sd'] ** 2 * cms
    model = spl_infinite_baffle(ts, fg, math.sqrt(8), *SEMI_CERAMIC) + iec_baffle(fg, *bb) + SENS_OFFSET
    e = float(np.sqrt(np.mean((model[lf] - sg[lf]) ** 2)))
    print(f"{m}: Bl={ts['bl']:.2f} Qes={qes:.3f} Qts={qts:.3f} Cms={cms*1e6:.0f} um/N Vas={vas*1e3:.1f} L Zmax={c['re']*(1+ts['qms']/qes):.0f} ohm; lf shape rms vs plot {e:.2f} dB (informational)")
    ts['qes'] = qes
    cel_ts[m] = ts


def fit_voicing(name, fg, resid, npk=3):
    base = (fg >= 300) & (fg <= 700)
    off = float(np.mean(resid[base]))
    sel = (fg >= 300) & (fg <= 14000)
    fsel = fg[sel]; rsel = resid[sel] - off
    def unpack(p):
        peaks = [(math.exp(p[3*i]), p[3*i+1], math.exp(p[3*i+2])) for i in range(npk)]
        return peaks, math.exp(p[3*npk]), math.exp(p[3*npk+1])
    def err(p):
        peaks, lpf, lpq = unpack(p)
        pen = 0
        for fc, g, q in peaks:
            if not (600 < fc < 8000): pen += 1e3
            if not (-12 < g < 12): pen += 1e3
            if not (0.7 < q < 6): pen += 1e3
            pen += 0.002 * g * g
        if not (3000 < lpf < 12000 and 0.5 < lpq < 2.5): pen += 1e3
        y = lp2_db(fsel, lpf, lpq)
        for fc, g, q in peaks: y = y + peq_db(fsel, fc, g, q)
        w = np.where(fsel < 7000, 1.0, 0.25)
        return float(np.mean(w * (y - rsel) ** 2)) + pen
    best = None
    for f1, f2, f3, lpf in [(1000, 2300, 3500, 5000), (1500, 2500, 4000, 6000), (800, 2000, 3000, 4500), (1200, 2800, 5000, 7000)]:
        st = [math.log(f1), 1, math.log(1.5), math.log(f2), 5, math.log(2), math.log(f3), 4, math.log(2), math.log(lpf), math.log(1.0)]
        p, e = nelder_mead(err, st, [0.2, 2, 0.3] * npk + [0.2, 0.2], iters=6000)
        for _ in range(5):
            p, e = nelder_mead(err, p, [0.1, 1, 0.2] * npk + [0.1, 0.1], iters=6000)
        if best is None or e < best[1]: best = (p, e)
    peaks, lpf, lpq = unpack(best[0])
    y = lp2_db(fsel, lpf, lpq)
    for fc, g, q in peaks: y = y + peq_db(fsel, fc, g, q)
    rms = float(np.sqrt(np.mean(((y - rsel)[fsel < 7000]) ** 2)))
    print(f"{name}: offset {off:+.2f} dB rms(300-7k) {rms:.2f} dB  LP {lpf:.0f} Hz Q {lpq:.2f}  peaks " + ', '.join(f"({fc:.0f} Hz {g:+.1f} dB Q {q:.2f})" for fc, g, q in sorted(peaks)))
    return dict(peaks=sorted(peaks), lp=(lpf, lpq), off=off, rms=rms)

print('\n== Breakup voicing fits (300 Hz..14 kHz; fixed offset = mean residual 300-700 Hz)')
voicing = {}
for m, ts in JTS.items():
    fx, sp = series(m, 'Sound Pressure')
    fg = np.geomspace(100, 20000, 300)
    sg = np.interp(np.log(fg), np.log(fx), smooth_octave(fx, sp, 6))
    model = spl_infinite_baffle(ts, fg, math.sqrt(8), *fits[m]) + iec_baffle(fg, *bb)
    voicing[m] = fit_voicing(m, fg, sg - model)
for m, ts in cel_ts.items():
    c = CEL[m]
    fp = np.array(sorted(c['plot'])); sp = np.array([c['plot'][k] for k in fp])
    fg = np.geomspace(100, 20000, 300)
    sg = np.interp(np.log(fg), np.log(fp), sp)
    model = spl_infinite_baffle(ts, fg, math.sqrt(8), *SEMI_CERAMIC) + iec_baffle(fg, *bb)
    voicing[m] = fit_voicing(m, fg, sg - model)
json.dump(dict(cel_ts=cel_ts, voicing=voicing, semi_ceramic=list(SEMI_CERAMIC), fits={k: list(v) for k, v in fits.items()}, baffle=list(bb)), open('fit_state2.json', 'w'), default=float)
