//! The Gold Drive, Brit Drive and Clean Boost against ElectroSmash's analyses:
//! operating points, the gains and corners the analyses work out, and which
//! way each tone pot turns.
//!
//! `cargo run --release --example drive_pedals_op`

use gainstagefx::circuits::{brit_drive, clean_boost, gold_drive};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::Circuit;
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn gain(circuit: Circuit, controls: &[(usize, f64)], hz: f64, volts: f64) -> f64 {
    let mut s = Simulation::new(circuit, RATE);
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 4.0) as usize, |x| s.process(x)).gain_db()
}

fn dc(circuit: Circuit, nodes: &[&str]) {
    let idx: Vec<usize> = nodes
        .iter()
        .map(|n| circuit.unknown_named(n).unwrap())
        .collect();
    let mut s = Simulation::new(circuit, RATE);
    s.find_operating_point();
    for (n, i) in nodes.iter().zip(idx) {
        print!(" {n} {:.2}", s.voltage_at(i));
    }
    println!();
}

fn main() {
    // --- Gold Drive ---
    let gd = |at: &str| gold_drive::tap(10_000.0, 470_000.0, at).unwrap();
    print!("Gold Drive DC:");
    dc(
        gd("out"),
        &["vref", "buf", "p5", "u1b", "clip", "u2a", "u2b"],
    );
    println!("gain stage, U1B over the buffer, gain full (ElectroSmash: 40 dB peak near 1 kHz):");
    for hz in [100.0, 300.0, 1_000.0, 3_000.0, 10_000.0] {
        let g = gain(gd("u1b"), &[(gold_drive::GAIN, 1.0)], hz, 1e-4)
            - gain(gd("buf"), &[(gold_drive::GAIN, 1.0)], hz, 1e-4);
        print!(" {hz:.0} Hz {g:+.1}");
    }
    println!();
    println!("treble shelf, U2B over U2A at 5 kHz and 100 Hz (ElectroSmash: +18.2 / -8 dB):");
    for t in [0.0, 0.5, 1.0] {
        let at = |hz| {
            gain(
                gd("u2b"),
                &[(gold_drive::TREBLE, t), (gold_drive::GAIN, 0.0)],
                hz,
                1e-3,
            ) - gain(
                gd("u2a"),
                &[(gold_drive::TREBLE, t), (gold_drive::GAIN, 0.0)],
                hz,
                1e-3,
            )
        };
        println!(
            "  treble {t:.1}: 5 kHz {:+.1}, 100 Hz {:+.1}",
            at(5_000.0),
            at(100.0)
        );
    }
    println!("whole pedal, 0.122 V at 220 Hz, level full:");
    for g in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut s = Simulation::new(gd("out"), RATE);
        s.set_control(gold_drive::GAIN, g);
        s.set_control(gold_drive::LEVEL, 1.0);
        s.find_operating_point();
        let m = measure::run(
            Tone::near(RATE, 16_384, 220.0, 0.122),
            (RATE / 4.0) as usize,
            |x| s.process(x),
        );
        println!(
            "  gain {g:.2}: {:+.1} dB, THD {:.1} %",
            m.gain_db(),
            m.thd_percent()
        );
    }

    // --- Brit Drive ---
    let bd = |at: &str| brit_drive::tap(10_000.0, 470_000.0, at).unwrap();
    print!("\nBrit Drive DC:");
    dc(bd("out"), &["vref", "p1", "u1a", "u2b", "clip"]);
    println!("stage gains at 1 kHz, gain 0 / 1 (ElectroSmash: 0 to 33 dB; 36 dB at full):");
    for g in [0.0, 1.0] {
        let c = [(brit_drive::GAIN, g)];
        let u1 = gain(bd("u1a"), &c, 1_000.0, 1e-5) - gain(bd("p1"), &c, 1_000.0, 1e-5);
        let u2 = gain(bd("u2b"), &c, 1_000.0, 1e-5) - gain(bd("u1a"), &c, 1_000.0, 1e-5);
        println!("  gain {g:.0}: U1A {u1:+.1} dB, U2B {u2:+.1} dB");
    }
    println!("tone stack (output over clip node), each control 0 / 1 with the others at half:");
    for (name, control) in [
        ("bass", brit_drive::BASS),
        ("middle", brit_drive::MIDDLE),
        ("treble", brit_drive::TREBLE),
    ] {
        for v in [0.0, 1.0] {
            let mut c = vec![
                (brit_drive::BASS, 0.5),
                (brit_drive::MIDDLE, 0.5),
                (brit_drive::TREBLE, 0.5),
                (brit_drive::LEVEL, 1.0),
                (brit_drive::GAIN, 0.0),
            ];
            c.retain(|(k, _)| *k != control);
            c.push((control, v));
            print!("  {name} {v:.0}:");
            for hz in [80.0, 300.0, 1_000.0, 3_000.0, 8_000.0] {
                let g = gain(bd("t_w"), &c, hz, 1e-3) - gain(bd("clip"), &c, hz, 1e-3);
                print!(" {hz:.0} {g:+.1}");
            }
            println!();
        }
    }

    // --- Clean Boost ---
    let cb = |at: &str| clean_boost::tap(10_000.0, 470_000.0, at).unwrap();
    print!("\nClean Boost DC:");
    dc(cb("out"), &["vref", "pp", "u"]);
    println!("gain at 1 kHz against the knob (ElectroSmash: 0 to 26.2 dB):");
    for g in [0.0, 0.25, 0.5, 0.75, 1.0] {
        println!(
            "  gain {g:.2}: {:+.2} dB",
            gain(cb("out"), &[(clean_boost::GAIN, g)], 1_000.0, 1e-3)
        );
    }
}
