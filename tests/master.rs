//! The circuit's own level control.
//!
//! Every device here except the abstract typologies has a knob on its face
//! that sets how loud it is -- a pedal's Level or Volume, the 73P's trim, an
//! amplifier's master -- and every one of them was frozen at a resting
//! position with no way to reach it from the panel. That is BUG-023's fault
//! one level up: a control that exists on the hardware and turns nothing.
//!
//! The one thing that had to be got right in wiring them up is what the middle
//! of the knob means, and it is not half the pot's travel. Each circuit rests
//! its level control somewhere different, and those positions are the ones the
//! make-up table was measured at. So the middle of the knob is the voicing,
//! and the first test here is the one that says so.

use gainstagefx::voice::{Cabinet, Chain, Gain, Level, Settings, Tone, MASTER_MIDDLE};

const RATE: f64 = 48_000.0;

fn settings(gain: Gain, master: f64) -> Settings {
    Settings {
        gain,
        drive: 0.7,
        master,
        tone: Tone::Off,
        cabinet: Cabinet::Off,
        oversampling: 1,
        ..Settings::default()
    }
}

fn render(gain: Gain, master: f64) -> Vec<f64> {
    let mut c = Chain::new(RATE);
    c.apply(&settings(gain, master));
    c.settle();
    c.find_operating_point();
    (0..4_000)
        .map(|k| {
            let t = k as f64 / RATE;
            c.process(0.3 * (std::f64::consts::TAU * 220.0 * t).sin())
        })
        .collect()
}

fn rms(x: &[f64]) -> f64 {
    (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt()
}

fn db(a: &[f64], b: &[f64]) -> f64 {
    20.0 * (rms(a) / rms(b).max(1e-30)).max(1e-30).log10()
}

/// The middle of the knob is where the voice was calibrated, so a patch that
/// leaves it alone sounds exactly as it did before there was a knob at all.
///
/// Bit-identical, not close: the control is set to the same position the
/// circuit's own `Netlist::rest` put it at, so the matrix is the same matrix.
#[test]
fn the_middle_of_the_knob_is_the_voicing() {
    for gain in Gain::ALL {
        let mut rested = Chain::new(RATE);
        // `Settings::default` carries the middle, which is the point.
        rested.apply(&Settings {
            gain,
            drive: 0.7,
            tone: Tone::Off,
            cabinet: Cabinet::Off,
            oversampling: 1,
            ..Settings::default()
        });
        rested.settle();
        rested.find_operating_point();
        let left_alone: Vec<f64> = (0..4_000)
            .map(|k| {
                let t = k as f64 / RATE;
                rested.process(0.3 * (std::f64::consts::TAU * 220.0 * t).sin())
            })
            .collect();
        assert_eq!(
            left_alone,
            render(gain, MASTER_MIDDLE),
            "{} moves when the Master knob is left where it starts",
            gain.name()
        );
    }
}

/// And a circuit that has one has to answer to it, in the direction a volume
/// control works.
#[test]
fn a_circuit_with_a_level_control_answers_to_it() {
    for gain in Gain::ALL {
        let Some(_) = gain.level_control() else {
            continue;
        };
        let quiet = render(gain, 0.15);
        let middle = render(gain, MASTER_MIDDLE);
        let loud = render(gain, 1.0);
        let down = db(&quiet, &middle);
        let up = db(&loud, &middle);
        println!("{}: {down:+.1} dB down, {up:+.1} dB up", gain.name());
        assert!(
            down < -3.0,
            "{} barely quietens: {down:+.1} dB at the bottom of its Master",
            gain.name()
        );
        assert!(
            up > 0.5,
            "{} barely lifts: {up:+.1} dB at the top of its Master",
            gain.name()
        );
    }
}

/// A circuit whose drawing has no such control must not pretend to have one.
///
/// The Twin Reverb is the one to watch: an AB763 has no master volume and no
/// presence, and its channel Volume is already the Drive knob. A Master knob
/// that did something here would be a control the amplifier does not own.
#[test]
fn a_circuit_without_one_does_not_pretend() {
    for gain in Gain::ALL {
        if gain.level_control().is_some() {
            continue;
        }
        assert_eq!(
            render(gain, 0.0),
            render(gain, 1.0),
            "{} has no level control on its drawing, so the Master knob must \
             reach nothing -- and the panel greys it for the same reason",
            gain.name()
        );
    }
    assert_eq!(
        Gain::Twin.level_control(),
        None,
        "an AB763 has no master volume"
    );
}

/// The two amplifiers with a master have it in the power stage, and the pedals
/// have theirs in the circuit. Which one a voice uses is a judgement about the
/// hardware, so it is written down and checked rather than inferred.
#[test]
fn each_level_control_is_where_the_drawing_puts_it() {
    use gainstagefx::circuits::{bigmuff, neve, power, ts808};
    assert_eq!(
        Gain::Screamer.level_control(),
        Some(Level::Circuit(ts808::LEVEL))
    );
    assert_eq!(
        Gain::Muff.level_control(),
        Some(Level::Circuit(bigmuff::VOLUME))
    );
    assert_eq!(Gain::Neve.level_control(), Some(Level::Circuit(neve::TRIM)));
    // A Lead Master and a post gain are both after the preamplifier.
    assert_eq!(
        Gain::Boogie.level_control(),
        Some(Level::Power(power::MASTER))
    );
    assert_eq!(
        Gain::Peavey.level_control(),
        Some(Level::Power(power::MASTER))
    );
}
