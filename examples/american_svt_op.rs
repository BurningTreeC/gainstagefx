//! The American SVT preamplifier (Ampeg D 591719 D) against its drawing and
//! against Ampeg's published controls.
//!
//! Prints the operating point beside the sheet's DC voltages, and the three
//! tone controls' ranges against the SVT-VR manual's figures: BASS +/-12 dB at
//! 40 Hz, TREBLE +/-12 dB at 4 kHz, MIDRANGE +/-20 dB at the selected
//! frequency (800 Hz in the centre position), each flat at centre; and the two
//! slide switches: BASS SELECT against Ampeg's figure for the Heritage SVT-CL
//! (ULTRA LO +2 dB at 40 Hz and -10 dB at 500 Hz), and MIDRANGE SELECT's three
//! centres against 220 Hz, 800 Hz and 3 kHz.
//!
//! `cargo run --release --example american_svt_op`

use gainstagefx::circuits::american_svt::{self as svt, tap};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn gain(controls: &[(usize, f64)], hz: f64) -> f64 {
    gain_with(controls, &[], hz)
}

/// With some of the switches' contacts set: `(slot, value)`.
fn gain_with(controls: &[(usize, f64)], values: &[(usize, f64)], hz: f64) -> f64 {
    let mut s = Simulation::new(tap(10_000.0, 1_000_000.0, "out").unwrap(), RATE);
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    for &(slot, v) in values {
        s.set_value(slot, v);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, 1e-3);
    measure::run(t, (RATE / 2.0) as usize, |x| s.process(x)).gain_db()
}

fn main() {
    let c = tap(10_000.0, 1_000_000.0, "out").unwrap();
    let nodes = [
        ("bp", "+300"),
        ("p1", "170"),
        ("k1", "1.75"),
        ("k1b", "190"),
        ("p3a", "200 (12DW7 era)"),
        ("k3a", "3.2 (12DW7 era)"),
        ("p3b", "180"),
        ("k3b", "20"),
        ("p4a", "130"),
        ("k4a", "1.2"),
        ("k4b", "135"),
        ("k5", "170"),
    ];
    let idx: Vec<usize> = nodes
        .iter()
        .map(|n| c.unknown_named(n.0).unwrap())
        .collect();
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    println!("operating point (sheet):");
    for ((n, want), i) in nodes.iter().zip(idx) {
        println!("  {n:4} {:7.2} V  ({want})", s.voltage_at(i));
    }

    let flat = [
        (svt::VOLUME, 0.3),
        (svt::BASS, 0.5),
        (svt::MIDDLE, 0.5),
        (svt::TREBLE, 0.5),
    ];
    let with = |c: usize, v: f64| {
        let mut k = flat.to_vec();
        k.retain(|(x, _)| *x != c);
        k.push((c, v));
        k
    };
    println!("\nresponse, all at noon, volume 0.3 (dB, in to out):");
    for hz in [40.0, 100.0, 300.0, 800.0, 2_000.0, 4_000.0, 10_000.0] {
        print!(" {hz:.0} {:+.1}", gain(&flat, hz));
    }
    println!();
    for (name, c, hz) in [
        ("BASS", svt::BASS, 40.0),
        ("TREBLE", svt::TREBLE, 4_000.0),
        ("MIDRANGE", svt::MIDDLE, 800.0),
    ] {
        let centre = gain(&flat, hz);
        let up = gain(&with(c, 1.0), hz) - centre;
        let down = gain(&with(c, 0.0), hz) - centre;
        println!("{name} at {hz} Hz: {up:+.1} / {down:+.1} dB around noon");
    }
    println!("\nMIDRANGE up and down against noon, by frequency:");
    for hz in [100.0, 200.0, 400.0, 600.0, 800.0, 1_000.0, 1_500.0, 3_000.0] {
        let centre = gain(&flat, hz);
        print!(
            " {hz:.0} {:+.1}/{:+.1}",
            gain(&with(svt::MIDDLE, 1.0), hz) - centre,
            gain(&with(svt::MIDDLE, 0.0), hz) - centre
        );
    }
    println!();
    let throw = |slots: &[usize], values: &[f64]| -> Vec<(usize, f64)> {
        slots.iter().copied().zip(values.iter().copied()).collect()
    };
    println!("\nBASS SELECT against OFF (dB; SVT-CL's ULTRA LO: +2 at 40 Hz, -10 at 500 Hz):");
    for (name, position) in [("BASS CUT", 0), ("ULTRA LO", 2)] {
        let v = throw(&svt::BASS_SELECT_SLOTS, &svt::BASS_SELECT[position]);
        print!("  {name:9}");
        for hz in [40.0, 60.0, 100.0, 200.0, 500.0, 1_000.0, 3_000.0] {
            print!(
                " {hz:.0} {:+.1}",
                gain_with(&flat, &v, hz) - gain(&flat, hz)
            );
        }
        println!();
    }
    println!("\nMIDRANGE SELECT, MIDRANGE up against noon, by position:");
    for (name, position) in [("1 (220 Hz)", 0), ("2 (800 Hz)", 1), ("3 (3 kHz)", 2)] {
        let v = throw(&svt::MID_SELECT_SLOTS, &svt::MID_SELECT[position]);
        let lift = |hz: f64| gain_with(&with(svt::MIDDLE, 1.0), &v, hz) - gain_with(&flat, &v, hz);
        let grid = [
            100.0, 150.0, 220.0, 330.0, 500.0, 800.0, 1_200.0, 2_000.0, 3_000.0, 4_500.0,
        ];
        let lifts: Vec<f64> = grid.iter().map(|&f| lift(f)).collect();
        let (peak, at) =
            lifts.iter().zip(grid).fold(
                (f64::MIN, 0.0),
                |(m, a), (&l, f)| if l > m { (l, f) } else { (m, a) },
            );
        print!("  {name:10}");
        for (f, l) in grid.iter().zip(&lifts) {
            print!(" {f:.0} {l:+.1}");
        }
        println!("   peak {peak:+.1} dB near {at:.0} Hz");
    }

    println!("\nVolume against the output at 400 Hz (small signal):");
    for v in [0.1, 0.3, 0.5, 0.7, 1.0] {
        print!(" {v:.1} {:+.1}", gain(&with(svt::VOLUME, v), 400.0));
    }
    println!();
}
