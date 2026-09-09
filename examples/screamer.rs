//! The TS808 taken apart: what each stage does to what it is handed.
//!
//! `TS808.md` section 15 asks for exactly this and for a reason -- a pedal can
//! have a plausible waveform at its output and be wrong in one stage.
//!
//! `cargo run --release --example screamer`

use gainstagefx::circuits::ts808::{self, DRIVE, LEVEL, TONE};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn at(node: &str, hz: f64, volts: f64, drive: f64) -> measure::Measured {
    let c = ts808::tap(10_000.0, 470_000.0, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(DRIVE, drive);
    sim.set_control(TONE, 0.5);
    sim.set_control(LEVEL, 1.0);
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x))
}

fn main() {
    level_law();
    println!("Every figure at 1 kHz. Volts are peak at the input jack.\n");

    println!("=== stage by stage, tiny signal (0.2 mV: the diodes are asleep) ===");
    println!("  {:<10}{:>10}", "node", "gain");
    for node in ["buf", "u1a", "u1b", "out"] {
        println!(
            "  {:<10}{:>9.1} dB",
            node,
            at(node, 1000.0, 0.0002, 1.0).gain_db()
        );
    }

    println!("\n=== the clipping stage alone, at the drive extremes ===");
    println!(
        "  {:<12}{:>10}{:>10}{:>10}{:>10}{:>9}{:>9}",
        "input", "gain d=0", "THD", "gain d=1", "THD", "2nd", "3rd"
    );
    for volts in [0.0002, 0.001, 0.005, 0.02, 0.05, 0.1, 0.2, 0.4, 0.8] {
        let shut = at("u1a", 1000.0, volts, 0.0);
        let wide = at("u1a", 1000.0, volts, 1.0);
        println!(
            "  {:<10.4} V{:>9.1} dB{:>9.1} %{:>9.1} dB{:>9.1} %{:>8.1} %{:>8.1} %",
            volts,
            shut.gain_db(),
            shut.thd_percent(),
            wide.gain_db(),
            wide.thd_percent(),
            wide.harmonic_percent(2),
            wide.harmonic_percent(3),
        );
    }

    // At 1 kHz the pedal's own post-clipping filter -- R7 into C5, 723 Hz --
    // has already taken the fundamental down and the harmonics further, so a
    // distortion figure measured there is mostly a measurement of that filter.
    // A guitar's open strings are 82 to 330 Hz. Both are reported, because the
    // difference between them *is* the pedal: it is a dark, mid-humped box and
    // the top of its band is deliberately not where its distortion lives.
    for hz in [110.0, 220.0, 1000.0] {
        println!("\n=== the whole pedal at {hz:.0} Hz, drive swept, guitar level (0.1 V) ===");
        println!(
            "  {:<8}{:>10}{:>10}{:>9}{:>9}{:>9}{:>9}",
            "drive", "gain", "THD", "2nd", "3rd", "4th", "5th"
        );
        for d in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0] {
            let m = at("out", hz, 0.1, d);
            println!(
                "  {:<8.2}{:>9.1} dB{:>9.1} %{:>8.1} %{:>8.1} %{:>8.1} %{:>8.1} %",
                d,
                m.gain_db(),
                m.thd_percent(),
                m.harmonic_percent(2),
                m.harmonic_percent(3),
                m.harmonic_percent(4),
                m.harmonic_percent(5),
            );
        }
    }

    println!("\n=== what the clipping stage puts out, in volts ===");
    println!("  A Screamer clamps the *added* part at a diode drop and lets the");
    println!("  dry signal through underneath, so the output should be roughly");
    println!("  the input plus six tenths of a volt, not a fixed ceiling.\n");
    println!(
        "  {:<12}{:>14}{:>14}{:>14}",
        "input", "out d=0", "out d=1", "in + 0.6"
    );
    for volts in [0.005, 0.02, 0.05, 0.1, 0.2, 0.4] {
        let shut = at("u1a", 1000.0, volts, 0.0);
        let wide = at("u1a", 1000.0, volts, 1.0);
        println!(
            "  {:<10.4} V{:>12.3} V{:>13.3} V{:>13.3} V",
            volts,
            volts * 10f64.powf(shut.gain_db() / 20.0),
            volts * 10f64.powf(wide.gain_db() / 20.0),
            volts + 0.6
        );
    }
    drive_range();

    // The pot law on its own, with the diodes asleep. R6 + the pot's own
    // resistance is the feedback, so the small-signal gain says directly what
    // fraction of 500 k the knob has dialled in.
    println!("\n=== the Drive pot's law, measured at 0.2 mV where nothing clips ===");
    println!(
        "  {:<10}{:>12}{:>16}{:>16}",
        "drive", "gain", "implied Rf", "of 500 k"
    );
    for d in [0.0f64, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0] {
        let g_db = at("u1a", 1000.0, 0.0002, d).gain_db();
        let g = 10f64.powf(g_db / 20.0);
        // gain = 1 + Rf/|Zg| at 1 kHz, with Zg = R4 in series with C3.
        let zg =
            (4700f64.powi(2) + (1.0 / (std::f64::consts::TAU * 1000.0 * 47e-9)).powi(2)).sqrt();
        let rf = (g - 1.0) * zg;
        println!(
            "  {:<10.2}{:>10.1} dB{:>13.0} ohm{:>14.1} %",
            d,
            g_db,
            rf,
            100.0 * (rf - 51_000.0).max(0.0) / 500_000.0
        );
    }
}

/// Where the Drive control actually controls something.
///
/// A Screamer's minimum gain is already `1 + R6/R4`, about 20 dB, so a hot
/// enough signal is clipping before the knob is touched and turning it only
/// adds level. Too quiet and the diodes never conduct at all. The knob is
/// worth something in between, and where that is is a fact about the circuit.
fn drive_range() {
    println!("\n=== how much the Drive knob is worth, against input level ===");
    println!(
        "  {:<12}{:>9}{:>9}{:>9}{:>9}{:>9}{:>10}",
        "input", "d=0", "d=0.25", "d=0.5", "d=0.75", "d=1", "spread"
    );
    for volts in [0.005, 0.01, 0.02, 0.04, 0.08, 0.122, 0.2, 0.4] {
        let mut row = Vec::new();
        for d in [0.0, 0.25, 0.5, 0.75, 1.0] {
            row.push(at("out", 1000.0, volts, d).thd_percent());
        }
        let spread = row[4] - row[0];
        print!("  {volts:<10.3} V");
        for t in &row {
            print!("{t:>8.1} %");
        }
        println!("{spread:>9.1} pts");
    }
}

/// Where the Level control has to sit for the pedal to deliver what it was
/// built to deliver.
///
/// The plugin reaches a circuit's Drive and its tone control and nothing else,
/// so any *other* front-panel knob sits wherever `Simulation::new` leaves it,
/// which is the middle of its travel. On a hundred-kilohm audio track the
/// middle is a tenth of the track -- about twenty decibels down -- and no
/// player has ever left a Tube Screamer's Level there. See `Netlist::rest`.
fn level_law() {
    println!("\n=== the Level control, at a guitar level and drive 0.85 ===");
    println!("  {:<10}{:>12}{:>14}", "level", "gain", "vs unity");
    for l in [0.0, 0.25, 0.5, 0.6, 0.7, 0.75, 0.8, 0.9, 1.0] {
        let c = ts808::tap(10_000.0, 470_000.0, "out").expect("builds");
        let mut sim = Simulation::new(c, RATE);
        sim.set_control(DRIVE, 0.85);
        sim.set_control(TONE, 0.5);
        sim.set_control(LEVEL, l);
        let t = Tone::near(RATE, 16_384, 220.0, 0.122);
        let g = measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x)).gain_db();
        println!("  {l:<10.2}{g:>11.1} dB{:>13.1} dB", g);
    }
}
