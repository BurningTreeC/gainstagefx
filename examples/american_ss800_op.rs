//! The American SS 800 (the GK 800RB's 300 W amplifier, 406-0044-B) against
//! GK's drawing and procedures, and the fits of its bias pot and its rails.
//!
//! The drawing's DC (VAS +1.1 V, the lower driver's base -1.1 V, the driver
//! emitters +-0.5 V, Q11's emitter -0.6 V, U1's output -8.8 V, the bootstrap
//! -29 V, 1.6 V across R36, 1.0 V across R37); its gain (0.65 V rms in, 41 V
//! out, 36 dB); GK's bias procedure (5 mV across each 0.33 ohm, 15 mA a
//! device), fitted with R38; and the clipping figures -- 36 V rms into 4 ohm
//! at slight clipping (the turn-on procedure of 8/19/91) and the rated 200 W
//! into 8 ohm (40 V rms) -- that the rails' open-circuit voltage and source
//! resistance are fitted to.
//!
//! `cargo run --release --example american_ss800_op`

use gainstagefx::circuits::american_ss800::{self as ss, tap_with};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn sim(load: f64, rails: (f64, f64), bias: f64, at: &str) -> Simulation {
    let mut s = Simulation::new(tap_with(1_000.0, load, rails.0, rails.1, at).unwrap(), RATE);
    s.set_control(ss::BIAS, bias);
    s.set_control(ss::MASTER, 1.0);
    s.find_operating_point();
    s
}

/// Quiescent current in one output device, from the drop across the folded
/// emitter resistor.
fn idle_ma(rails: (f64, f64), bias: f64) -> f64 {
    let c = tap_with(1_000.0, 4.0, rails.0, rails.1, "out").unwrap();
    let (eh, out) = (
        c.unknown_named("eh").unwrap(),
        c.unknown_named("out").unwrap(),
    );
    let mut s = Simulation::new(c, RATE);
    s.set_control(ss::BIAS, bias);
    s.find_operating_point();
    (s.voltage_at(eh) - s.voltage_at(out)) / (0.33 / 3.0) / 3.0 * 1e3
}

/// Output rms and THD for an input rms at 200 Hz into `load`.
fn drive(load: f64, rails: (f64, f64), bias: f64, vin: f64) -> (f64, f64) {
    let mut s = sim(load, rails, bias, "out");
    let t = Tone::near(RATE, 16_384, 200.0, vin * 2f64.sqrt());
    // Long enough for the rails to sag to where a sustained tone holds them.
    let m = measure::run(t, (RATE * 0.25) as usize, |x| s.process(x));
    (m.fundamental().magnitude() / 2f64.sqrt(), m.thd_percent())
}

/// The output rms where THD reaches 1 %: "slight clipping".
fn clip(load: f64, rails: (f64, f64), bias: f64) -> f64 {
    let (mut lo, mut hi) = (0.1f64, 2.0f64);
    for _ in 0..12 {
        let mid = (lo * hi).sqrt();
        if drive(load, rails, bias, mid).1 < 1.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    drive(load, rails, bias, lo).0
}

fn main() {
    let rails = (ss::RAIL_OPEN, ss::RAIL_SOURCE);
    // --- the bias: 15 mA a device ---
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    let more_at_top = idle_ma(rails, 1.0) > idle_ma(rails, 0.0);
    for _ in 0..30 {
        let mid = 0.5 * (lo + hi);
        if (idle_ma(rails, mid) > 15.0) == more_at_top {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let bias = 0.5 * (lo + hi);
    println!(
        "bias: R38 at {bias:.4} for 15 mA a device ({:.2} mA; ends {:.2} / {:.2} mA); BIAS_REST is {}",
        idle_ma(rails, bias),
        idle_ma(rails, 0.0),
        idle_ma(rails, 1.0),
        ss::BIAS_REST
    );

    // --- the drawing's DC ---
    let c = tap_with(1_000.0, 4.0, rails.0, rails.1, "out").unwrap();
    let names = [
        ("vas", "+1.1"),
        ("bot", "-1.1"),
        ("de1", "+0.5"),
        ("de2", "-0.5"),
        ("e11", "-0.6"),
        ("uo", "-8.8"),
        ("bs", "-29"),
        ("out", "0"),
        ("vp", "85"),
        ("qc", "R36: 1.6 below +85"),
        ("e12", "R37: 1.0 below +85"),
    ];
    let idx: Vec<usize> = names
        .iter()
        .map(|n| c.unknown_named(n.0).unwrap())
        .collect();
    let mut s = Simulation::new(c, RATE);
    s.set_control(ss::BIAS, bias);
    s.find_operating_point();
    println!("\nDC (drawing):");
    for ((n, want), &i) in names.iter().zip(&idx) {
        println!("  {n:4} {:9.3} V  ({want})", s.voltage_at(i));
    }

    // --- gain ---
    let (out, thd) = drive(1e6, rails, bias, 0.65);
    println!(
        "\n0.65 V rms in at 200 Hz, no load: {out:.2} V rms ({:.1} dB, THD {thd:.3} %; drawing 41 V, 36 dB)",
        20.0 * (out / 0.65).log10()
    );
    let (out, thd) = drive(4.0, rails, bias, 0.1);
    println!(
        "0.1 V rms into 4 ohm: {out:.2} V rms ({:.1} dB, THD {thd:.3} %)",
        20.0 * (out / 0.1).log10()
    );

    // --- the rails' resistance, for the turn-on procedure's 36 V rms into 4 ohm ---
    let (mut lo_r, mut hi_r) = (0.5f64, 20.0f64);
    for _ in 0..10 {
        let mid = (lo_r * hi_r).sqrt();
        if clip(4.0, (ss::RAIL_OPEN, mid), bias) > 36.0 {
            lo_r = mid;
        } else {
            hi_r = mid;
        }
    }
    println!(
        "\nrails: {} V open behind {:.2} ohm for 36 V rms into 4 ohm at slight clipping; RAIL_SOURCE is {}",
        ss::RAIL_OPEN,
        (lo_r * hi_r).sqrt(),
        ss::RAIL_SOURCE
    );

    // --- clipping with the rails as built ---
    println!(
        "\nslight clipping (1 % THD) with rails {:.1} V behind {:.2} ohm: 4 ohm {:.1} V rms, 8 ohm {:.1} V rms (GK: 36, 40)",
        rails.0,
        rails.1,
        clip(4.0, rails, bias),
        clip(8.0, rails, bias)
    );
}
