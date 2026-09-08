//! Moving a knob while audio is playing.
//!
//! A pot is a resistance. Turning it changes a conductance in the matrix and
//! nothing else: every capacitor keeps the charge it had, every core keeps its
//! flux, and the circuit walks to its new operating point through its own time
//! constants. That is what the hardware does.
//!
//! What the model used to do was throw all of it away. A control moving marks
//! the matrix dirty; the next sample rebuilt it, and the rebuild started every
//! reactance and every device from nothing and then hunted the operating point
//! *with no signal in the circuit* to fill them back in. The audio in flight
//! was deleted and replaced with a DC solution, once per block, for as long as
//! the knob was moving.
//!
//! Reported as "when we engage a knob while audio is playing, the audio gets
//! different, washed out, smoothed out, wrong".
//!
//! The test is a knob move so small it cannot change the circuit -- one part
//! in a million, alternating so it ends where it started -- applied at every
//! block boundary, against the same render with the knob left alone. Whatever
//! comes back is not the knob.

use gainstagefx::dsp::measure::Tone;
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain};

const RATE: f64 = 48_000.0;
const LEVEL: f64 = 0.1;

/// Every circuit worth asking, including one of each kind that has a state a
/// rebuild could lose: reactances everywhere, an op-amp in the Screamer, a
/// saturating core in the 73P's input transformer.
const CIRCUITS: [Gain; 6] = [
    Gain::Crunch,
    Gain::Distortion,
    Gain::Screamer,
    Gain::Boogie,
    Gain::Peavey,
    Gain::Neve,
];

fn render(gain: Gain, block: usize, nudge: bool) -> Vec<f64> {
    let netlist =
        voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve).expect("builds");
    let mut sim = Simulation::new(netlist, RATE);
    let which = gain.drive_control();
    sim.set_control(which, 0.5);
    sim.find_operating_point();

    let n = RATE as usize / 4;
    let tone = Tone::near(RATE, 16_384, 220.0, LEVEL);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if nudge && i % block == 0 {
            let wobble = if (i / block).is_multiple_of(2) {
                1e-6
            } else {
                -1e-6
            };
            sim.set_control(which, 0.5 + wobble);
        }
        out.push(sim.process(tone.at(i)));
    }
    out
}

fn residual_db(a: &[f64], b: &[f64]) -> f64 {
    let mut energy = 0.0;
    let mut reference = 0.0;
    for (x, y) in a.iter().zip(b) {
        energy += (x - y) * (x - y);
        reference += x * x;
    }
    let rms = (energy / a.len() as f64).sqrt();
    let level = (reference / a.len() as f64).sqrt().max(1e-30);
    20.0 * (rms / level).max(1e-30).log10()
}

/// Eighty decibels down is the bar. Before the fix the Mark IIC+ measured
/// **plus** 4.5 dB -- an error louder than the signal -- and the quietest of
/// the six was 15 dB down.
#[test]
fn a_knob_that_barely_moves_barely_changes_the_sound() {
    for gain in CIRCUITS {
        let still = render(gain, 64, false);
        for block in [64usize, 512] {
            let moved = render(gain, block, true);
            let db = residual_db(&still, &moved);
            println!("{} at {block}: {db:.1} dB", gain.name());
            assert!(
                db < -80.0,
                "{} loses its state when its knob moves: {db:.1} dB of residual \
                 at a block size of {block}",
                gain.name()
            );
        }
    }
}

/// And it must not matter how often the host asks. A residual that grows as
/// the block gets shorter is a per-rebuild discontinuity, which is what this
/// was.
#[test]
fn moving_it_more_often_does_not_make_it_worse() {
    for gain in CIRCUITS {
        let at_64 = residual_db(&render(gain, 64, false), &render(gain, 64, true));
        let at_512 = residual_db(&render(gain, 64, false), &render(gain, 512, true));
        println!(
            "{}: {at_64:.1} dB at 64, {at_512:.1} dB at 512",
            gain.name()
        );
        assert!(
            at_64 - at_512 < 20.0,
            "{} gets {:.1} dB worse when the knob is moved eight times as \
             often, so something is being lost on every rebuild",
            gain.name(),
            at_64 - at_512
        );
    }
}

/// The same complaint one level up, where the tests above could not see it.
///
/// Everything above runs a single `Simulation`, and a single simulation
/// handles a moving control correctly: `process` rebuilds the matrix on
/// `dirty` but only hunts the operating point `if self.at_rest`. What it does
/// not cover is what `Plugin::process` does *around* the circuits, and that is
/// where the audible fault was:
///
/// ```text
/// if channels.len() >= 2 && channels[0].needs_operating_point() {
///     first[0].find_operating_point();
///     for chain in rest { chain.share_operating_point_from(...) }
/// }
/// ```
///
/// `needs_operating_point` returned `self.dirty`, and `dirty` is set by any
/// pot moving. So with a knob under a finger this said yes once a block, and
/// both halves are destructive with audio in flight: the hunt solves channel 0
/// with no signal in it and sets every capacitor from the answer, and the
/// share then replaces every other channel's running state with that. The
/// second half is the one with a name -- one channel's audio arriving in
/// another -- and it measured about a tenth of the signal.
///
/// Reported as "when playing audio through the plugin and simultaneously
/// engaging a knob, the audio becomes more distorted, as if an additional
/// layer of audio would be put above the audio".
mod through_the_plugin {
    use gainstagefx::voice::{Cabinet, Chain, Gain, Settings, Tone as ToneSection};

    const RATE: f64 = 48_000.0;
    const BLOCK: usize = 64;
    const SECONDS: f64 = 0.25;

    fn settings(gain: Gain, drive: f64) -> Settings {
        Settings {
            gain,
            drive,
            tone: ToneSection::Scooping,
            cabinet: Cabinet::Stack,
            oversampling: 1,
            ..Settings::default()
        }
    }

    fn left(k: usize) -> f64 {
        0.25 * (k as f64 * 0.031).sin()
    }

    fn right(k: usize) -> f64 {
        0.25 * (k as f64 * 0.017).sin() + 0.1 * (k as f64 * 0.09).sin()
    }

    /// `Plugin::process`, reproduced: settings to every chain once a block,
    /// the operating-point hand-over under its guard, then the block.
    fn render(gain: Gain, sources: &[fn(usize) -> f64]) -> Vec<Vec<f64>> {
        let total = (RATE * SECONDS) as usize;
        let mut chains: Vec<Chain> = sources
            .iter()
            .map(|_| {
                let mut c = Chain::new(RATE);
                c.apply(&settings(gain, 0.15));
                c.settle();
                c
            })
            .collect();
        let mut out: Vec<Vec<f64>> = sources.iter().map(|_| Vec::new()).collect();

        let mut k = 0;
        while k < total {
            let n = BLOCK.min(total - k);
            // A knob under a finger.
            let set = settings(gain, 0.15 + 0.7 * (k as f64 / total as f64));
            for chain in &mut chains {
                chain.apply(&set);
            }
            if chains.len() >= 2 && chains[0].needs_operating_point() {
                let (first, rest) = chains.split_at_mut(1);
                first[0].find_operating_point();
                for chain in rest {
                    chain.share_operating_point_from(first[0].operating_point());
                }
            }
            for j in 0..n {
                for (c, chain) in chains.iter_mut().enumerate() {
                    out[c].push(chain.process(sources[c](k + j)));
                }
            }
            k += n;
        }
        out
    }

    fn residual_db(a: &[f64], b: &[f64]) -> f64 {
        let mut num = 0.0;
        let mut den = 0.0;
        for i in 0..a.len().min(b.len()) {
            num += (a[i] - b[i]).powi(2);
            den += a[i] * a[i];
        }
        if den <= 0.0 {
            return f64::NEG_INFINITY;
        }
        10.0 * (num / den).log10()
    }

    /// Two channels carrying different audio, with the same knob swept on
    /// both, must each come out exactly as they would have alone. The plugin
    /// is dual mono; one channel cannot reach the other.
    #[test]
    fn a_moving_knob_does_not_let_one_channel_into_another() {
        for gain in [Gain::Peavey, Gain::Boogie, Gain::Screamer, Gain::Crunch] {
            let both = render(gain, &[left, right]);
            let alone_l = render(gain, &[left]);
            let alone_r = render(gain, &[right]);

            for (name, mixed, alone) in [
                ("left", &both[0], &alone_l[0]),
                ("right", &both[1], &alone_r[0]),
            ] {
                let residual = residual_db(mixed, alone);
                println!("{} {name}: {residual:.1} dB", gain.name());
                assert!(
                    residual < -100.0,
                    "{}'s {name} channel is {residual:.1} dB from the same \
                     channel run on its own while a knob moves -- the other \
                     channel is audible in it",
                    gain.name()
                );
            }
        }
    }

    /// And the invariant underneath it, stated directly: once audio is in
    /// flight there is never anything to hand over, however many controls
    /// move. `at_rest` is what says so.
    #[test]
    fn nothing_is_worth_sharing_once_audio_is_in_flight() {
        for gain in [Gain::Peavey, Gain::Boogie, Gain::Screamer, Gain::Crunch] {
            let mut chain = Chain::new(RATE);
            chain.apply(&settings(gain, 0.2));
            assert!(
                chain.needs_operating_point(),
                "{} has never solved, so its operating point is worth hunting",
                gain.name()
            );

            chain.find_operating_point();
            chain.process(0.1);

            // Now turn knobs, the way a player does.
            for step in 1..=8 {
                chain.apply(&settings(gain, 0.2 + 0.08 * step as f64));
                assert!(
                    !chain.needs_operating_point(),
                    "{} asked to be re-hunted after a knob moved with audio in \
                     flight, which deletes the audio and hands the wreckage to \
                     every other channel",
                    gain.name()
                );
                chain.process(0.1);
            }
        }
    }
}
