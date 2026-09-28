//! Switching a voice while audio is playing.
//!
//! Reported as a loud spike when switching to the Mark IIC+ or the 5150 from
//! another model. At the time those were the only voices with a power stage,
//! but the fault was in their level conversion, not the power stages.
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
    // Settings change at block boundaries. Report the boundary actually used,
    // including for durations whose midpoint falls inside a block.
    let switch = (total / 2).div_ceil(BLOCK) * BLOCK;
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

/// The voices worth switching between: the two from the original report, reached
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

fn peak(samples: &[f64]) -> f64 {
    samples.iter().fold(0.0, |m, v| m.max(v.abs()))
}

/// The fade retains audio from the old voice, which can be louder than the
/// new one. Bound the transition against BOTH settled levels, then require
/// the destination's own level after 10 ms (the 256-sample fade plus 66-sample
/// latency is under 7 ms at this rate, leaving time for the cabinet tail).
///
/// The corrected 5150 made the old assertion fail on Crunch -> 5150: the
/// first post-switch sample is exactly the last Crunch sample, 0.129 in
/// magnitude, against a new settled peak of 0.044. That is continuity, not
/// a spike. Neither a larger multiplier nor a quieter circuit fixes the test.
#[test]
fn switching_a_voice_does_not_spike() {
    for (from, to) in SWITCHES {
        let (y, switch) = render(from, to, 1.0);
        let window = RATE as usize / 10;
        let before = peak(&y[switch - window..switch]);
        let transition = peak(&y[switch..]);
        let after_fade = peak(&y[switch + RATE as usize / 100..]);
        let settled = peak(&y[y.len() - window..]);
        assert!(
            settled > 0.0,
            "{} -> {} went silent",
            from.name(),
            to.name()
        );
        let reference = before.max(settled);
        println!(
            "{} -> {}: before {before:.3}, transition {transition:.3}, \
             after fade {after_fade:.3}, settled {settled:.3}",
            from.name(),
            to.name()
        );
        assert!(
            transition < 2.0 * reference,
            "{} -> {} peaks at {transition:.3} against settled levels \
             {before:.3} -> {settled:.3}: the transition exceeds both voices",
            from.name(),
            to.name()
        );
        assert!(
            after_fade < 2.0 * settled,
            "{} -> {} peaks at {after_fade:.3} after the fade against a settled \
             {settled:.3}: the old voice's level must not persist",
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
