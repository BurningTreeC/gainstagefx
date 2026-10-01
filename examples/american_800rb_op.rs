//! The American 800RB (Gallien-Krueger 800RB, preamp 406-0045-C) against GK's
//! service manual: the stage voltages the drawing prints for 2 mV at 200 Hz
//! with every tone control and the volume on 10, the boost on 10 and the
//! filters out (19 mV after U1A, 18 into U2A, 107 out of it, 115 / 115 / 150
//! through the bands, 93 at the boost, 1.1 V at the drain), and each band's
//! centre and reach against the operator's pages (60 Hz, 250 Hz, 1 kHz,
//! 4 kHz), the contour's notch "at about 500Hz", Lo Cut, Hi Boost and -10 dB.
//!
//! `cargo run --release --example american_800rb_op`

use gainstagefx::circuits::american_800rb::{self as gk, tap};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

struct Setup {
    controls: Vec<(usize, f64)>,
    values: Vec<(usize, f64)>,
}

fn rms(node: &str, setup: &Setup, hz: f64, volts_rms: f64) -> f64 {
    let mut s = Simulation::new(tap(600.0, 1_000_000.0, node).unwrap(), RATE);
    for &(c, v) in &setup.controls {
        s.set_control(c, v);
    }
    for &(slot, v) in &setup.values {
        s.set_value(slot, v);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, volts_rms * 2f64.sqrt());
    let settle = (RATE * if hz < 100.0 { 1.0 } else { 0.4 }) as usize;
    let m = measure::run(t, settle, |x| s.process(x));
    m.fundamental().magnitude() / 2f64.sqrt()
}

fn all_ten() -> Setup {
    Setup {
        controls: vec![
            (gk::VOLUME, 1.0),
            (gk::BASS, 1.0),
            (gk::LO_MID, 1.0),
            (gk::HI_MID, 1.0),
            (gk::TREBLE, 1.0),
            (gk::BOOST, 1.0),
        ],
        values: vec![],
    }
}

fn flat(control: usize, at: f64) -> Setup {
    let mut c = vec![
        (gk::VOLUME, 1.0),
        (gk::BASS, 0.5),
        (gk::LO_MID, 0.5),
        (gk::HI_MID, 0.5),
        (gk::TREBLE, 0.5),
        (gk::BOOST, 0.5),
    ];
    c.retain(|&(k, _)| k != control);
    c.push((control, at));
    Setup {
        controls: c,
        values: vec![],
    }
}

fn main() {
    println!("stage voltages at 2 mV, 200 Hz, everything on 10 (drawing in brackets):");
    let s = all_ten();
    for (node, want) in [
        ("u1a", "19 mV"),
        ("u2p", "18 mV"),
        ("u2a", "107 mV"),
        ("o3", "115 mV"),
        ("o4", "115 mV"),
        ("o5", "150 mV"),
        ("bi", "93 mV"),
        ("bl", "10 mV"),
        ("bus", "1.1 V"),
    ] {
        let v = rms(node, &s, 200.0, 2e-3);
        println!("  {node:4} {:8.2} mV  ({want})", v * 1e3);
    }

    println!("\neach band, U2A -> U3B out at 1e-4 V, others at noon, dB re its own noon:");
    for (name, control, freqs) in [
        ("bass", gk::BASS, [30.0, 60.0, 120.0, 500.0]),
        ("low mid", gk::LO_MID, [125.0, 250.0, 500.0, 2_000.0]),
        ("high mid", gk::HI_MID, [500.0, 1_000.0, 2_000.0, 5_000.0]),
        ("treble", gk::TREBLE, [1_000.0, 4_000.0, 8_000.0, 15_000.0]),
    ] {
        for at in [0.0, 1.0] {
            print!("  {name:8} {at:.0}:");
            for hz in freqs {
                let r = rms("o5", &flat(control, at), hz, 1e-4)
                    / rms("o5", &flat(control, 0.5), hz, 1e-4);
                print!(" {hz:.0} {:+.1}", 20.0 * r.log10());
            }
            println!();
        }
    }

    println!("\nswitches, at the boost input, dB re flat:");
    let base = flat(gk::BASS, 0.5);
    let with = |slot: usize, v: f64| Setup {
        controls: base.controls.clone(),
        values: vec![(slot, v)],
    };
    for (name, slot, v, freqs) in [
        (
            "lo cut",
            gk::LO_CUT_SLOTS[0],
            gk::LO_CUT[0][0],
            [40.0, 80.0, 160.0, 1_000.0],
        ),
        (
            "hi boost",
            gk::HI_BOOST_SLOT,
            gk::SWITCH_CLOSED,
            [1_000.0, 3_000.0, 6_000.0, 10_000.0],
        ),
        (
            "-10 dB",
            gk::PAD_SLOT,
            gk::SWITCH_CLOSED,
            [100.0, 1_000.0, 5_000.0, 10_000.0],
        ),
    ] {
        print!("  {name:8}:");
        for hz in freqs {
            let r = rms("bi", &with(slot, v), hz, 1e-4) / rms("bi", &base, hz, 1e-4);
            print!(" {hz:.0} {:+.1}", 20.0 * r.log10());
        }
        println!();
    }
    let contour = Setup {
        controls: base.controls.clone(),
        values: gk::CONTOUR_SLOTS
            .iter()
            .zip(gk::CONTOUR[0])
            .map(|(&s, v)| (s, v))
            .collect(),
    };
    print!("  contour :");
    for hz in [100.0, 250.0, 400.0, 500.0, 700.0, 1_000.0, 3_000.0] {
        let r = rms("bi", &contour, hz, 1e-4) / rms("bi", &base, hz, 1e-4);
        print!(" {hz:.0} {:+.1}", 20.0 * r.log10());
    }
    println!();
}
