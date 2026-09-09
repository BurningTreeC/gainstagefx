//! What a Tube Screamer is actually for.
//!
//! The pedal's own distortion is mild and always was: two diodes in an
//! op-amp's feedback loop, a stage whose gain falls as they conduct, and a
//! post-clipping filter at 723 Hz that takes the top off what is left. On its
//! own it is a mid-humped, slightly gritty boost, and measuring it on its own
//! is measuring the wrong thing. It is called a Tube Screamer because it makes
//! a *valve amplifier* scream: the mid hump and the compression hit the front
//! end of an already-working amp and push it somewhere it will not go by
//! itself.
//!
//! This measures that difference, because the plugin currently cannot: a
//! `Chain` runs one circuit, so the Screamer is only ever heard into a wire.
//!
//! `cargo run --release --example infront`

use gainstagefx::circuits::{preamp, ts808};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{LOAD, SOURCE};

/// What a guitar puts out, stated rather than inferred. See `CLAUDE.md` 59.3.
const GUITAR_VOLTS: f64 = 0.122;

const RATE: f64 = 96_000.0;

fn measured(chain: &mut dyn FnMut(f64) -> f64, hz: f64, volts: f64) -> measure::Measured {
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 10.0) as usize, chain)
}

fn main() {
    println!(
        "A guitar at {GUITAR_VOLTS:.3} V into: the pedal alone, the amplifier\n\
         alone, and the pedal in front of the amplifier. 220 Hz.\n"
    );
    println!(
        "  {:<28}{:>10}{:>10}{:>9}{:>9}{:>9}",
        "path", "gain", "THD", "2nd", "3rd", "5th"
    );

    for (name, values) in [
        ("two valve stages", preamp::CRUNCH),
        ("three valve stages", preamp::HIGH_GAIN),
    ] {
        for drive in [0.0f64, 0.85] {
            let pedal = ts808::build(SOURCE, LOAD).expect("builds");
            let amp = preamp::build(&values, SOURCE, LOAD).expect("builds");

            // The amplifier on its own.
            let mut a = Simulation::new(amp.clone(), RATE);
            a.set_control(preamp::GAIN, 0.8);
            a.find_operating_point();
            let alone = measured(&mut |x| a.process(x), 220.0, GUITAR_VOLTS);

            // The pedal on its own.
            let mut p = Simulation::new(pedal.clone(), RATE);
            p.set_control(ts808::DRIVE, drive);
            p.find_operating_point();
            let pedal_only = measured(&mut |x| p.process(x), 220.0, GUITAR_VOLTS);

            // And the pedal into the amplifier.
            let mut p2 = Simulation::new(pedal, RATE);
            p2.set_control(ts808::DRIVE, drive);
            p2.find_operating_point();
            let mut a2 = Simulation::new(amp, RATE);
            a2.set_control(preamp::GAIN, 0.8);
            a2.find_operating_point();
            let stacked = measured(&mut |x| a2.process(p2.process(x)), 220.0, GUITAR_VOLTS);

            if drive == 0.0 {
                println!(
                    "  {:<28}{:>9.1} dB{:>9.1} %{:>8.1} %{:>8.1} %{:>8.1} %",
                    format!("{name}, alone"),
                    alone.gain_db(),
                    alone.thd_percent(),
                    alone.harmonic_percent(2),
                    alone.harmonic_percent(3),
                    alone.harmonic_percent(5),
                );
            }
            println!(
                "  {:<28}{:>9.1} dB{:>9.1} %{:>8.1} %{:>8.1} %{:>8.1} %",
                format!("TS808 d={drive:.2}, alone"),
                pedal_only.gain_db(),
                pedal_only.thd_percent(),
                pedal_only.harmonic_percent(2),
                pedal_only.harmonic_percent(3),
                pedal_only.harmonic_percent(5),
            );
            println!(
                "  {:<28}{:>9.1} dB{:>9.1} %{:>8.1} %{:>8.1} %{:>8.1} %",
                format!("  -> into {name}"),
                stacked.gain_db(),
                stacked.thd_percent(),
                stacked.harmonic_percent(2),
                stacked.harmonic_percent(3),
                stacked.harmonic_percent(5),
            );
        }
        println!();
    }
}
