//! Switching a voice while audio is playing.
//!
//! Reported as a loud spike when switching to the Mark IIC+ or the 5150 from
//! another model, and those two are the only voices with a power stage --
//! which is a good clue and, as it turned out, the wrong one.
//!
//! The cause was the make-up. `out_of` converts a circuit's own output into
//! the digital domain, and it glides, because the make-up moves with the
//! drive knob and a knob is a continuum. Two voices are not. An amplifier's
//! output after its power stage is tens of volts and its make-up is small; a
//! pedal's is near one and its make-up is large. Gliding between them applies
//! one voice's conversion to the other voice's output for the length of the
//! glide -- a one-pole at 0.02, so about five milliseconds.
//!
//! Measured on Big Muff -> 5150 before the fix: a peak of 20.25 against a
//! settled 0.125, with a hundred and thirty-seven samples over full scale.
//! `examples/switching.rs` is the measurement.

use gainstagefx::voice::{Cabinet, Chain, Gain, Settings, Tone as ToneSection};

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;

fn settings(gain: Gain) -> Settings {
    Settings {
        gain,
        drive: 0.7,
        tone: ToneSection::Scooping,
        cabinet: Cabinet::Stack,
        oversampling: 1,
        ..Settings::default()
    }
}

/// Plays a tone, switches voice half way, returns the whole render and where
/// the switch happened.
fn render(from: Gain, to: Gain, seconds: f64) -> (Vec<f64>, usize) {
    let total = (RATE * seconds) as usize;
    let switch = total / 2;
    let mut chain = Chain::new(RATE);
    chain.apply(&settings(from));
    chain.settle();
    chain.find_operating_point();

    let mut out = Vec::with_capacity(total);
    let mut k = 0;
    while k < total {
        let n = BLOCK.min(total - k);
        chain.apply(&settings(if k < switch { from } else { to }));
        for j in 0..n {
            out.push(chain.process(0.2 * ((k + j) as f64 * 0.028).sin()));
        }
        k += n;
    }
    (out, switch)
}

/// The voices worth switching between: the two with a power stage, reached
/// from a pedal and from a plain gain stage, because the fault was in the
/// difference between the two voices rather than in either one.
const SWITCHES: [(Gain, Gain); 6] = [
    (Gain::Muff, Gain::Peavey),
    (Gain::Muff, Gain::Boogie),
    (Gain::Crunch, Gain::Peavey),
    (Gain::Crunch, Gain::Boogie),
    (Gain::Screamer, Gain::Peavey),
    (Gain::Neve, Gain::Boogie),
];

/// Nothing a switch produces may exceed what the voice settles to by more
/// than a little. A switch is allowed to change the level -- a 5150 is not a
/// Big Muff -- so the test is against the *new* voice's own settled peak, not
/// against the old one's.
#[test]
fn switching_a_voice_does_not_spike() {
    for (from, to) in SWITCHES {
        let (y, switch) = render(from, to, 1.0);
        let peak = y[switch..].iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let settled = y[y.len() - RATE as usize / 10..]
            .iter()
            .fold(0.0f64, |m, v| m.max(v.abs()));
        assert!(
            settled > 0.0,
            "{} -> {} went silent",
            from.name(),
            to.name()
        );
        let ratio = peak / settled;
        println!(
            "{} -> {}: peak {peak:.3}, settled {settled:.3}, {ratio:.1}x",
            from.name(),
            to.name()
        );
        assert!(
            ratio < 2.0,
            "{} -> {} peaks at {peak:.3} against a settled {settled:.3}, \
             {ratio:.1} times over: the make-up of the voice being left is \
             being applied to the voice being arrived at",
            from.name(),
            to.name()
        );
    }
}

/// And it must never leave the digital domain at all. This is the plain
/// statement of the bug as it was heard: a loud spike.
#[test]
fn switching_a_voice_never_goes_over_full_scale() {
    for (from, to) in SWITCHES {
        let (y, switch) = render(from, to, 1.0);
        let over = y[switch..].iter().filter(|v| v.abs() > 1.0).count();
        assert_eq!(
            over,
            0,
            "{} -> {} put {over} samples over full scale, the largest {:.3}",
            y[switch..].iter().fold(0.0f64, |m, v| m.max(v.abs())),
            from.name(),
            to.name(),
        );
    }
}

/// Every switch is finite. A power stage that keeps its old charge across a
/// switch is not obviously unstable, so this states it.
#[test]
fn switching_a_voice_stays_finite() {
    for (from, to) in SWITCHES {
        let (y, _) = render(from, to, 0.5);
        assert!(
            y.iter().all(|v| v.is_finite()),
            "{} -> {} produced a non-finite sample",
            from.name(),
            to.name()
        );
    }
}
