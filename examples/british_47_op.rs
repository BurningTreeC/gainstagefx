//! The British 47 (EMI REDD.47) against EMI's drawing and description.
//!
//! The drawing's operating points (V1 anode 73 V, cathode 1.65 V, 1.35 mA from
//! the 205 V node; V2 anode 138 V, cathode 3.7 V, 18 mA), and REDD.M47's three
//! gains -- "with a 200 ohm load connected between terminals 2/5 and 2/6, the
//! voltage gain measured between input and output terminals is equivalent to
//! 34, 40, or 46 dB, selected by the switch" -- with the fine gain set so the
//! middle position reads 40, as EMI sets it. Prints the fine-gain shunt that
//! does that, for `british_47::FINE_GAIN_SHUNT`.
//!
//! `cargo run --release --example british_47_op`

use gainstagefx::circuits::british_47::{self as b47, tap};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn gain_db(position: f64, hz: f64, volts: f64) -> (f64, f64) {
    let c = tap(200.0, 200.0, "out").unwrap();
    let input = c.unknown_named("in").unwrap();
    let mut s = Simulation::new(c, RATE);
    s.set_control(b47::GAIN, position);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, volts);
    // Gain from the input terminals (after the 200 ohm source) to the load.
    let mut terminal = Vec::new();
    let m = measure::run(t, (RATE / 2.0) as usize, |x| {
        let y = s.process(x);
        terminal.push(s.voltage_at(input));
        y
    });
    let n = terminal.len();
    let tail = &terminal[n / 2..];
    let mean = tail.iter().sum::<f64>() / tail.len() as f64;
    let rms_in =
        (tail.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / tail.len() as f64).sqrt();
    let rms_src = volts / 2f64.sqrt();
    (
        m.gain_db() + 20.0 * (rms_src / rms_in).log10(),
        m.thd_percent(),
    )
}

/// The fine-gain shunt that puts the middle position on 40 dB, by bisection:
/// more shunt resistance, more of R5 left, less gain.
fn fit_fine_gain() -> f64 {
    let (mut lo, mut hi) = (200.0f64, 1e6f64);
    for _ in 0..40 {
        let mid = (lo * hi).sqrt();
        let g = gain_with(mid, 0.5);
        if g > 40.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo * hi).sqrt()
}

fn gain_with(shunt: f64, position: f64) -> f64 {
    let c = b47::tap_with(200.0, 200.0, shunt, "out").unwrap();
    let input = c.unknown_named("in").unwrap();
    let mut s = Simulation::new(c, RATE);
    s.set_control(b47::GAIN, position);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, 1_000.0, 1e-3);
    let mut terminal = Vec::new();
    let m = measure::run(t, (RATE / 2.0) as usize, |x| {
        let y = s.process(x);
        terminal.push(s.voltage_at(input));
        y
    });
    let tail = &terminal[terminal.len() / 2..];
    let mean = tail.iter().sum::<f64>() / tail.len() as f64;
    let rms_in =
        (tail.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / tail.len() as f64).sqrt();
    m.gain_db() + 20.0 * ((1e-3 / 2f64.sqrt()) / rms_in).log10()
}

fn main() {
    let shunt = fit_fine_gain();
    let r5 = 120.0 * shunt / (120.0 + shunt);
    println!("fine gain: shunt {shunt:.0} ohm puts 40 dB in the middle (R5 shunted to {r5:.1} ohm; EMI's limits 77-120)");
    let c = tap(200.0, 200.0, "out").unwrap();
    let nodes = [
        ("b1", "205"),
        ("p1", "73"),
        ("k1", "1.65"),
        ("s1", "screen"),
        ("p2", "138"),
        ("k2", "3.7"),
        ("j", "inner-loop junction"),
    ];
    let idx: Vec<usize> = nodes
        .iter()
        .map(|n| c.unknown_named(n.0).unwrap())
        .collect();
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    println!("operating point (drawing):");
    for ((n, want), i) in nodes.iter().zip(idx) {
        println!("  {n:3} {:8.2} V  ({want})", s.voltage_at(i));
    }
    println!(
        "  V1 cathode current {:.2} mA (1.35); V2 pair {:.1} mA (18)",
        s.voltage_at(c_idx("k1")) / 1_120.0 * 1e3,
        s.voltage_at(c_idx("k2")) / 200.0 * 1e3
    );
    println!(
        "\ngain into 200 ohm at 1 kHz, input terminals to output (REDD.M47: 34 / 40 / 46 dB):"
    );
    for (p, name) in [(0.15, "34"), (0.5, "40"), (0.85, "46")] {
        let (g, thd) = gain_db(p, 1_000.0, 1e-3);
        println!("  position {name}: {g:.2} dB, THD {thd:.3} %");
    }
    println!(
        "\nresponse at 40, source to load, dB re 1 kHz (two-second tones, so the low ones settle):"
    );
    let r = response(1_000.0);
    for hz in [
        10.0, 20.0, 50.0, 100.0, 5_000.0, 10_000.0, 15_000.0, 20_000.0,
    ] {
        print!(" {hz:.0} {:+.2}", response(hz) - r);
    }
    println!();
    println!("\nlevel at 40 dB into 200 ohm (REDD.M47: 0 dBm matched, +14 dBm on peaks):");
    for dbu in [-40.0, -30.0, -26.0, -22.0] {
        let v = 0.775 * 10f64.powf(dbu / 20.0) * 2f64.sqrt() * 2.0; // open-circuit source for that at the terminals
        let (g, thd) = gain_db(0.5, 1_000.0, v);
        println!(
            "  {dbu:+.0} dBu in: out {:+.1} dBu, THD {thd:.2} %",
            dbu + g
        );
    }
}

fn response(hz: f64) -> f64 {
    let rate = 48_000.0;
    let mut s = Simulation::new(tap(200.0, 200.0, "out").unwrap(), rate);
    s.set_control(b47::GAIN, 0.5);
    s.find_operating_point();
    let t = Tone::near(rate, 65_536, hz, 1e-3);
    measure::run(t, (rate * 2.0) as usize, |x| s.process(x)).gain_db()
}

fn c_idx(n: &str) -> usize {
    tap(200.0, 200.0, "out").unwrap().unknown_named(n).unwrap()
}
