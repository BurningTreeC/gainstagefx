//! What the spring tank actually does.
//!
//! `cargo run --release --example tank`

use gainstagefx::dsp::spring::Tank;

const RATE: f64 = 48_000.0;

fn main() {
    let mut tank = Tank::accutronics(RATE);
    // An impulse, which is what a tank is judged on: the "boing" is the
    // chirp its dispersion makes out of a single click.
    let n = (RATE * 5.0) as usize;
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        out.push(tank.process(if k == 0 { 1.0 } else { 0.0 }));
    }

    println!("Accutronics-style tank, impulse response, {RATE} Hz.\n");

    // Where the energy is, over time.
    println!("  {:<10}{:>12}{:>12}", "window", "rms", "vs peak");
    let peak = out.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    for (label, from, to) in [
        ("0-50 ms", 0.0, 0.05),
        ("50-200 ms", 0.05, 0.2),
        ("0.2-1 s", 0.2, 1.0),
        ("1-2 s", 1.0, 2.0),
        ("2-3 s", 2.0, 3.0),
        ("3-4 s", 3.0, 4.0),
        ("4-5 s", 4.0, 5.0),
    ] {
        let a = (from * RATE) as usize;
        let b = ((to * RATE) as usize).min(out.len());
        let rms = (out[a..b].iter().map(|v| v * v).sum::<f64>() / (b - a) as f64).sqrt();
        println!(
            "  {label:<10}{rms:>12.6}{:>11.1} dB",
            20.0 * (rms / peak).max(1e-12).log10()
        );
    }

    // The chirp: where the first arrivals sit in frequency, early against
    // late. A dispersive line sends the top first.
    let band = |from: usize, to: usize, lo: f64, hi: f64| {
        let seg = &out[from..to.min(out.len())];
        let n = seg.len();
        let mut total = 0.0;
        let k0 = ((lo / RATE) * n as f64) as usize;
        let k1 = (((hi / RATE) * n as f64) as usize).min(n / 2);
        for k in k0..=k1.max(k0) {
            let (mut re, mut im) = (0.0, 0.0);
            let w = std::f64::consts::TAU * k as f64 / n as f64;
            for (i, &v) in seg.iter().enumerate() {
                re += v * (w * i as f64).cos();
                im -= v * (w * i as f64).sin();
            }
            total += re * re + im * im;
        }
        total
    };
    println!("\n  the chirp: where the energy is, early against late");
    println!(
        "  {:<14}{:>12}{:>12}{:>12}",
        "window", "100-800", "0.8-2k", "2-5k"
    );
    for (label, a, b) in [
        ("0-40 ms", 0, (0.04 * RATE) as usize),
        ("40-120 ms", (0.04 * RATE) as usize, (0.12 * RATE) as usize),
        ("0.5-1 s", (0.5 * RATE) as usize, (1.0 * RATE) as usize),
    ] {
        let lo = band(a, b, 100.0, 800.0);
        let mid = band(a, b, 800.0, 2000.0);
        let hi = band(a, b, 2000.0, 5000.0);
        let t = (lo + mid + hi).max(1e-30);
        println!(
            "  {label:<14}{:>11.0} %{:>11.0} %{:>11.0} %",
            100.0 * lo / t,
            100.0 * mid / t,
            100.0 * hi / t
        );
    }
    let finite = out.iter().all(|v| v.is_finite());
    println!("\n  peak {peak:.4}, all finite: {finite}");
    tremolo();
}

/// The tremolo, as a shape rather than a depth.
fn tremolo() {
    use gainstagefx::dsp::tremolo::Tremolo;
    println!("\n\n=== the tremolo, at the phase inverter's grid ===");
    // A 12AX7 plate with a 100 k load looks like about 38 k; the phase
    // inverter's grid leak is 1 M.
    let (source, load) = (38_000.0, 1_000_000.0);
    println!(
        "  {:<10}{:>8}{:>12}{:>12}{:>12}{:>10}",
        "speed", "Hz", "intensity", "deepest", "shallowest", "duty"
    );
    for speed in [0.0f64, 0.5, 1.0] {
        for intensity in [0.0f64, 0.3, 0.6, 1.0] {
            let mut t = Tremolo::new(RATE);
            // Two seconds, enough for several cycles at the slowest setting.
            let n = (RATE * 2.0) as usize;
            let mut lo = f64::MAX;
            let mut hi: f64 = 0.0;
            let mut below = 0usize;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                let a = t.attenuation(speed, intensity, source, load);
                v.push(a);
                lo = lo.min(a);
                hi = hi.max(a);
            }
            let mid = (lo + hi) / 2.0;
            for a in &v {
                if *a < mid {
                    below += 1;
                }
            }
            // Count cycles by zero crossings of (a - mid).
            let crossings = v
                .windows(2)
                .filter(|w| (w[0] < mid) != (w[1] < mid))
                .count();
            let hz = crossings as f64 / 2.0 / 2.0;
            println!(
                "  {speed:<10.2}{hz:>8.1}{intensity:>12.2}{:>11.1} dB{:>11.1} dB{:>9.0} %",
                20.0 * lo.max(1e-9).log10(),
                20.0 * hi.max(1e-9).log10(),
                100.0 * below as f64 / n as f64
            );
        }
    }
    println!("\n  'deepest'/'shallowest' are the attenuation at the two ends of");
    println!("  the sweep. Intensity at zero never strikes the bulb, which is");
    println!("  the control switching the tremolo off rather than fading it.");
}
