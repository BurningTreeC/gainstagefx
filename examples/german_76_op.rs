//! The German 76 (Telefunken V76) against the IRT's drawing S 1176 and its
//! Braunbuch acceptance sheet (Blatt 5-8).
//!
//! First the three values the model has to set itself, each by bisection,
//! for `german_76::Fit::SET`: the rail's open-circuit voltage for the
//! drawing's 300 V; 75, the loop resistor the drawing marks "abgeglichen",
//! for 76 dB at position 12; and the input transformer's knee for 1 % k3 at
//! 40 Hz with 34 dB of gain and +22 dB out. Then the drawing's node voltages,
//! the gain at all twelve positions, the response and both filter switches,
//! the input and output impedances and the distortion figures, each against
//! the sheet.
//!
//! Gain is referred to the generator's EMF, which is how the IRT measured it:
//! the padded positions show it. Referred to the input terminals instead,
//! position 4 (450 ohm in each leg, 200 ohm across) would read 25 dB.
//!
//! `cargo run --release --example german_76_op`

use gainstagefx::circuits::german_76::{self as v76, tap_with, Fit};
use gainstagefx::dsp::complex::C;
use gainstagefx::dsp::measure::{self, Measured, Tone};
use gainstagefx::dsp::time::Simulation;

const DB: [f64; 12] = [
    3.0, 9.0, 18.0, 24.0, 34.0, 40.0, 46.0, 52.0, 58.0, 64.0, 70.0, 76.0,
];

/// Both switches flat: the low cut's and "3 kHz"'s second positions.
const FLAT: (usize, usize) = (1, 1);

/// The knob position in the middle of switch position `n` (0-based).
fn position(n: usize) -> f64 {
    (n as f64 + 0.5) / 12.0
}

/// The generator EMF, peak, for an input level in dB re 0.775 V.
fn source_emf(level_db: f64) -> f64 {
    0.775 * 2f64.sqrt() * 10f64.powf(level_db / 20.0)
}

struct Run {
    out: Measured,
    /// The input terminals' and the generator's phasors over the window.
    terminal: C,
    emf: C,
}

#[allow(clippy::too_many_arguments)]
fn run(
    fit: Fit,
    at: &str,
    n: usize,
    (low, mid): (usize, usize),
    hz: f64,
    level_db: f64,
    load: f64,
    rate: f64,
) -> Run {
    let c = tap_with(200.0, load, fit, at).unwrap();
    let input = c.unknown_named("in").unwrap();
    let mut s = Simulation::new(c, rate);
    s.set_control(v76::GAIN, position(n));
    for (&slot, &value) in v76::LOW_CUT_SLOTS.iter().zip(&v76::LOW_CUT[low]) {
        s.set_value(slot, value);
    }
    s.set_value(v76::TREBLE_CUT_SLOTS[0], v76::TREBLE_CUT[mid][0]);
    s.find_operating_point();
    let window = 32_768.min((rate * 0.7) as usize);
    let t = Tone::near(rate, window, hz, source_emf(level_db));
    let settle = (rate * if hz < 100.0 { 2.0 } else { 0.5 }) as usize;
    let (mut xs, mut vs) = (Vec::new(), Vec::new());
    let out = measure::run(t, settle, |x| {
        let y = s.process(x);
        xs.push(x);
        vs.push(s.voltage_at(input));
        y
    });
    let phasor = |v: &[f64]| {
        let tail = &v[v.len() - window..];
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &x) in tail.iter().enumerate() {
            let a = std::f64::consts::TAU * t.bin * i as f64 / window as f64;
            re += x * a.cos();
            im -= x * a.sin();
        }
        C::new(re, im)
    };
    Run {
        terminal: phasor(&vs),
        emf: phasor(&xs),
        out,
    }
}

/// Gain at 1 kHz, +6 dB out, the sheet's conditions.
fn gain(fit: Fit, n: usize) -> Measured {
    run(fit, "out", n, FLAT, 1_000.0, 6.0 - DB[n], 300.0, 96_000.0).out
}

/// Response re a reference, at the sheet's -64 dB in at position 12.
fn response(fit: Fit, sw: (usize, usize), hz: f64, reference: f64) -> f64 {
    // Trapezoidal integration bends a 17 kHz low-pass well down into the band
    // at 48 kHz; the plugin oversamples, and so does this above 2 kHz -- and
    // further for the sheet's 40-200 kHz, which 192 kHz would read at 47.
    let rate = match hz.max(reference) {
        f if f > 30_000.0 => 768_000.0,
        f if f > 2_000.0 => 192_000.0,
        _ => 48_000.0,
    };
    let at = |f: f64| run(fit, "out", 11, sw, f, -64.0, 300.0, rate).out.gain_db();
    at(hz) - at(reference)
}

fn bisect(mut lo: f64, mut hi: f64, geometric: bool, mut above: impl FnMut(f64) -> bool) -> f64 {
    for _ in 0..30 {
        let mid = if geometric {
            (lo * hi).sqrt()
        } else {
            0.5 * (lo + hi)
        };
        if above(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

fn main() {
    // --- the three fits ---
    let mut fit = Fit::SET;
    fit.rail_open = bisect(290.0, 340.0, false, |open| {
        let c = tap_with(
            200.0,
            300.0,
            Fit {
                rail_open: open,
                ..fit
            },
            "out",
        )
        .unwrap();
        let b0 = c.unknown_named("b0").unwrap();
        let mut s = Simulation::new(c, 48_000.0);
        s.set_control(v76::GAIN, position(11));
        s.find_operating_point();
        s.voltage_at(b0) > 300.0
    });
    // More of 75, less of V3/V4's loop, more gain.
    fit.r75 = bisect(40_000.0, 160_000.0, true, |r75| {
        gain(Fit { r75, ..fit }, 11).gain_db() > 76.0
    });
    // A higher knee, less third.
    fit.t1_knee = bisect(0.01, 0.5, true, |knee| {
        let r = run(
            Fit {
                t1_knee: knee,
                ..fit
            },
            "out",
            4,
            FLAT,
            40.0,
            22.0 - 34.0,
            300.0,
            48_000.0,
        );
        r.out.harmonic_percent(3) < 1.0
    });
    println!(
        "fit: rail_open {:.1} V, r75 {:.0} ohm (drawn 80 k), t1_knee {:.4} V s",
        fit.rail_open, fit.r75, fit.t1_knee
    );
    println!(
        "  Fit::SET is {:.1} V, {:.0} ohm, {:.4} V s",
        Fit::SET.rail_open,
        Fit::SET.r75,
        Fit::SET.t1_knee
    );
    let fit = Fit::SET;

    // --- the drawing's operating point ---
    let c = tap_with(200.0, 300.0, fit, "out").unwrap();
    let names = [
        ("b0", "300"),
        ("b2", "279"),
        ("b1", "242"),
        ("k1", "11"),
        ("s1", "97"),
        ("p1", "67"),
        ("k2", "2.3"),
        ("s2", "165"),
        ("p2", "175"),
        ("k3", "16"),
        ("s3", "71"),
        ("p3", "80"),
        ("k4", "2.2"),
        ("s4", "140"),
        ("p4", "170"),
    ];
    let idx: Vec<usize> = names
        .iter()
        .map(|n| c.unknown_named(n.0).unwrap())
        .collect();
    let mut s = Simulation::new(c, 48_000.0);
    s.set_control(v76::GAIN, position(11));
    s.find_operating_point();
    let v = |i: usize| s.voltage_at(idx[i]);
    println!("\noperating point (drawing):");
    for (i, (n, want)) in names.iter().enumerate() {
        println!("  {n:3} {:8.2} V  ({want})", v(i));
    }
    println!(
        "  rail {:.1} mA (21); B1 branch {:.2} mA (6.1); V4 plate {:.1} mA (11.7)",
        (fit.rail_open - v(0)) / v76::RAIL_SOURCE * 1e3,
        (v(1) - v(2)) / 6_000.0 * 1e3,
        (v(1) - v(14)) / 9_300.0 * 1e3,
    );

    println!("\nwhere the band edges and the 40 Hz distortion come from, position 12, dB re 1 kHz at each node:");
    for node in ["g1", "p2", "g3", "p4", "t2p", "out"] {
        print!("  {node:4}");
        for hz in [40.0, 60.0, 10_000.0, 15_000.0] {
            let rate = if hz > 2_000.0 { 192_000.0 } else { 48_000.0 };
            let at = |f: f64| {
                run(fit, node, 11, FLAT, f, -64.0, 300.0, rate)
                    .out
                    .gain_db()
            };
            print!(" {hz:.0} {:+.2}", at(hz) - at(1_000.0));
        }
        let m = run(fit, node, 11, FLAT, 40.0, 22.0 - 76.0, 300.0, 48_000.0).out;
        println!(
            "  | +22 out 40 Hz k2 {:.3} k3 {:.3}",
            m.harmonic_percent(2),
            m.harmonic_percent(3)
        );
    }

    // --- the sheet ---
    println!("\ngain at 1 kHz, +6 dB out, re the generator (Braunbuch +-0.5 dB):");
    for (n, want) in DB.iter().enumerate() {
        let m = gain(fit, n);
        println!(
            "  position {:2}: {:6.2} dB ({want:2.0}), THD {:.3} %",
            n + 1,
            m.gain_db(),
            m.thd_percent()
        );
    }

    println!("\nresponse at position 12, -64 dB in, dB re 1 kHz (60 Hz-15 kHz +-0.5, 40 Hz +0.5/-1.5, 15 Hz <= -12, 40-200 kHz <= -20):");
    for hz in [
        15.0, 25.0, 40.0, 60.0, 100.0, 300.0, 5_000.0, 10_000.0, 15_000.0, 20_000.0, 40_000.0,
        100_000.0, 200_000.0,
    ] {
        print!(" {hz:.0} {:+.2}", response(fit, FLAT, hz, 1_000.0));
    }
    println!();

    println!("\nlow cut, re 1 kHz (80 Hz: <=3 at 80, >=12 at 40; 300 Hz: <=4 at 300, >=14 at 40; 80+300: <=4 at 300, >=28 at 40):");
    for (name, low) in [("80 Hz", 0), ("300 Hz", 2), ("80+300", 3)] {
        print!("  {name:7}");
        for hz in [40.0, 80.0, 300.0] {
            print!(" {hz:.0} {:+.2}", response(fit, (low, 1), hz, 1_000.0));
        }
        let loss = run(fit, "out", 11, (low, 1), 1_000.0, -64.0, 300.0, 48_000.0)
            .out
            .gain_db()
            - run(fit, "out", 11, FLAT, 1_000.0, -64.0, 300.0, 48_000.0)
                .out
                .gain_db();
        println!(" (1 kHz {loss:+.2} re flat; the sheet allows 1.5)");
    }
    println!("\n3 kHz, re 500 Hz (<=0.6 at 1 kHz, 5-7 at 10 kHz, 6-8 at 15 kHz):");
    for hz in [1_000.0, 3_000.0, 10_000.0, 15_000.0] {
        print!(" {hz:.0} {:+.2}", response(fit, (1, 0), hz, 500.0));
    }
    println!();

    println!("\ninput impedance at position 5, -38 dB in (>= 600 ohm 60-8000 Hz, >= 300 ohm 40-60 Hz and 8-15 kHz):");
    for hz in [40.0, 60.0, 1_000.0, 8_000.0, 15_000.0] {
        let rate = if hz < 100.0 { 48_000.0 } else { 192_000.0 };
        let r = run(fit, "out", 4, FLAT, hz, -38.0, 300.0, rate);
        let z = C::new(200.0, 0.0) * r.terminal / (r.emf - r.terminal);
        print!(" {hz:.0} {:.0}", z.magnitude());
    }
    println!(" ohm");

    println!("\noutput impedance at position 12 (<= 30 ohm at 1 kHz, <= 40 ohm 40 Hz-15 kHz):");
    for hz in [40.0, 1_000.0, 15_000.0] {
        let rate = if hz < 100.0 { 48_000.0 } else { 192_000.0 };
        let at = |load: f64| {
            run(fit, "out", 11, FLAT, hz, -76.0, load, rate)
                .out
                .gain_db()
        };
        let ratio = 10f64.powf((at(3_000.0) - at(300.0)) / 20.0);
        // V = E Z_L / (Z_L + Z), with two loads: Z from the ratio.
        let z = (ratio - 1.0) / (1.0 / 300.0 - ratio / 3_000.0);
        print!(" {hz:.0} {z:.1}");
    }
    println!(" ohm");

    println!("\ndistortion, k2 / k3 % (the sheet's maxima in brackets):");
    for (n, out, hz, limit) in [
        (11, 12.0, 40.0, "0.2 / 0.2"),
        (11, 12.0, 1_000.0, "0.1 / 0.1"),
        (11, 12.0, 5_000.0, "0.1 / 0.1"),
        (11, 22.0, 40.0, "0.3 / 0.2"),
        (11, 22.0, 1_000.0, "0.2 / 0.2"),
        (4, 22.0, 40.0, "1.0 / 1.5"),
        (4, 22.0, 1_000.0, "0.2 / 0.2"),
        (4, 22.0, 5_000.0, "0.2 / 0.2"),
    ] {
        let rate = if hz < 100.0 { 48_000.0 } else { 96_000.0 };
        let m = run(fit, "out", n, FLAT, hz, out - DB[n], 300.0, rate).out;
        println!(
            "  {:2} dB, {out:+.0} dB out, {hz:5.0} Hz: {:.3} / {:.3}  ({limit})",
            DB[n],
            m.harmonic_percent(2),
            m.harmonic_percent(3)
        );
    }
}
