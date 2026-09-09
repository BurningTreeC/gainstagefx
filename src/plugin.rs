//! The plugin: parameters in, audio out.

use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::Arc;

use crate::meters::Meters;
use crate::params::{Amplifier, Diode, GainStageParams, Oversampling};
use crate::voice::{Chain, Settings, LATENCY, NOMINAL_DBFS};

pub struct GainStageFx {
    params: Arc<GainStageParams>,
    meters: Arc<Meters>,
    /// The one signal chain. It owns every circuit in the catalogue, so
    /// changing the selection while playing does not allocate.
    ///
    /// One, not one per channel: an amplifier has a single input jack, and the
    /// host's channels are summed to mono before the circuit. See `process`.
    chain: Option<Chain>,
    sample_rate: f64,
    oversampling: Oversampling,
    peak: f64,
    budget: Budget,
    /// Blocks the work budget had to pinch, and samples that finished at the
    /// ceiling because of it. Read outside the callback.
    pinched_blocks: u64,
    pinched_samples: u64,
    summing: Summing,
}

/// As many channels as the summing bookkeeping will track. The layouts above
/// ask for one or two; anything beyond this is summed without being counted,
/// which errs toward loud rather than silent.
const MAX_CHANNELS: usize = 16;

/// How the host's channels become the one signal the circuit sees.
///
/// An amplifier has a single input jack and a guitar has a single output, so
/// the channels are summed to mono *before* the circuit rather than run
/// through two copies of it. That is also what makes the catalogue meet its
/// deadline live: a dual-mono plugin fed a guitar was solving the same circuit
/// twice for the same answer, which is half the cost of the plugin spent
/// computing numbers it already had.
///
/// What the sum is divided by is the whole difference between this working and
/// not, and it is why this is a type with tests rather than three lines in the
/// callback.
///
/// **Dividing by the channel count is wrong, and wrong in the most common case
/// there is.** A mono track does not reach a plugin as one channel in most
/// hosts -- it reaches it as two, with the second one digital silence -- and a
/// guitar panned hard left is the same shape. Averaging those halves the
/// guitar, and the plugin is six decibels quiet before it has done anything.
///
/// **Dividing by nothing is wrong the other way**: a mono track that *is*
/// duplicated across both channels would arrive six decibels hot, into
/// circuits calibrated in volts.
///
/// So the divisor is the number of channels that have actually carried a
/// signal. A channel that has never been anything but zero is not a source, it
/// is an empty half of a container, and it does not get a share. `live`
/// latches rather than gating on a threshold, so a sine crossing zero does not
/// change the level and there is nothing to tune; and the divisor is walked
/// toward its target so a channel that arrives late is a ramp and not a step.
#[derive(Clone, Copy)]
pub struct Summing {
    live: [bool; MAX_CHANNELS],
    divisor: f64,
    sum: f64,
    counted: usize,
}

impl Summing {
    pub const fn new() -> Self {
        Self {
            live: [false; MAX_CHANNELS],
            divisor: 1.0,
            sum: 0.0,
            counted: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn start(&mut self) {
        self.sum = 0.0;
        self.counted = 0;
    }

    pub fn add(&mut self, index: usize, sample: f64) {
        self.sum += sample;
        match self.live.get_mut(index) {
            Some(seen) => {
                *seen |= sample != 0.0;
                self.counted += usize::from(*seen);
            }
            // Beyond what is tracked, count it: erring toward a larger divisor
            // is erring toward quiet, and quiet is the fault this exists to
            // avoid -- but an untracked channel is past the layouts this
            // plugin declares, so it never happens in practice.
            None => self.counted += 1,
        }
    }

    /// The one signal, with `ramp` the per-sample coefficient of the one pole
    /// the divisor follows.
    pub fn finish(&mut self, ramp: f64) -> f64 {
        let target = self.counted.max(1) as f64;
        self.divisor += (target - self.divisor) * ramp;
        self.sum / self.divisor
    }
}

impl Default for GainStageFx {
    fn default() -> Self {
        crate::dsp::time::enable_ftz_daz();
        Self {
            params: Arc::new(GainStageParams::default()),
            meters: Arc::new(Meters::default()),
            chain: None,
            sample_rate: 48_000.0,
            oversampling: Oversampling::Four,
            peak: 0.0,
            budget: Budget::new(),
            pinched_blocks: 0,
            pinched_samples: 0,
            summing: Summing::new(),
        }
    }
}

/// How a block that is running out of time finishes it anyway.
///
/// A plugin that "usually fits" does not fit. Live monitoring has no
/// lookahead: the host must have the block before its deadline or there is
/// nothing to play, which is why a recorded track through the Mark IIC+
/// stutters and a live guitar through it is silent. Making the solve faster
/// lowers the odds of that; it does not remove them, because the plugin does
/// not own the machine.
///
/// So the deadline is watched. The clock is read every `STRIDE` samples --
/// often enough to react inside a 64-sample block, rarely enough that the
/// reading itself is not the cost -- and when the block has used more of its
/// budget than it has samples left to justify, the Newton ceiling comes down
/// for the rest of it. The trade is a handful of less-converged samples
/// against a block the host does not get at all.
///
/// Three things it must do, or it is worse than nothing:
///
/// - **be bounded**, which `Simulation::set_pass_ceiling` enforces with a
///   floor that no caller can go under. BUG-008 measured what an unfloored
///   ceiling does.
/// - **be counted**, so a session that is degrading is visible rather than
///   quietly worse. `Chain::pinched`.
/// - **never engage where there is no deadline**, which is any render the host
///   is not doing in real time. An offline bounce has all the time it wants
///   and must produce the same audio as the last one.
#[derive(Clone, Copy)]
pub struct Budget {
    /// Whether there is a deadline to hold to at all. Off for an offline
    /// render; see `initialize`.
    pub armed: bool,
    /// Samples between clock readings.
    pub stride: usize,
    /// What fraction of the budget a block may have spent, per fraction of it
    /// done, before the ceiling comes down.
    slack: f64,
}

impl Budget {
    /// Whether capping Newton passes on a late block is a mechanism that
    /// works. **It is not, and this is off.**
    ///
    /// The idea is sound and the arithmetic is not. Trading iterations for
    /// time assumes the expensive samples are expensive because they iterate
    /// a lot, and on this solver they are not: every voice averages two to
    /// three and a half passes on real playing, and the cost is spread evenly
    /// across the block rather than concentrated in a few long solves.
    ///
    /// So a ceiling either sits above where the solve finishes -- saving
    /// nothing, which is what 32/24/18/12 against a floor of twelve did -- or
    /// it sits below, and then it *costs*: a solve that would have converged
    /// in three passes instead runs to the cap without converging, spends the
    /// whole allowance, and leaves the next sample a worse place to predict
    /// from, so that one fails too. Measured on real playing at 48 kHz / 64,
    /// with steps lowered to 32/8/6/4 so they would actually bite:
    ///
    /// | | budget off | budget on |
    /// |---|---|---|
    /// | 5150, callbacks missed | 1.6 % | 2.0 % |
    /// | Twin Reverb, missed | 0.5 % | 0.8 % |
    /// | Twin Reverb, worst callback | 1780 us | **3230 us** |
    ///
    /// The last row is the whole argument: 637 samples were pinched and the
    /// worst callback nearly doubled.
    ///
    /// The guarantee therefore has to come from the solve being faster, not
    /// from degrading it -- Layers 1 and 3 in `DEADLINE_STRATEGY.md`. The
    /// machinery here stays because the *shape* of it is right, and a
    /// mechanism that gives up something the solver does not need per pass --
    /// a stage, the transformer, the oversampling -- could use it. Capping
    /// iterations is not that mechanism.
    const WORKS: bool = false;

    pub const fn new() -> Self {
        Self {
            armed: false,
            stride: 16,
            slack: 1.25,
        }
    }

    /// The ceiling for a block that is `done` samples into `total`, having
    /// used `spent` of its `deadline`.
    ///
    /// Deliberately arithmetic and not a state machine: given the same
    /// numbers it returns the same answer, and given a block that is inside
    /// its budget it returns the full ceiling, so an offline render and a
    /// realtime one agree unless the clock actually said otherwise.
    pub fn ceiling(&self, done: usize, total: usize, spent: f64, deadline: f64) -> usize {
        if !self.armed || total == 0 || deadline <= 0.0 {
            return 32;
        }
        let expected = deadline * done as f64 / total as f64;
        let over = spent / (expected * self.slack).max(1e-9);
        // 32 while inside the slack, then down in defined steps.
        //
        // These have to *bind*, and the first set did not: 32, 24, 18, 12
        // against voices that average two to three and a half passes on real
        // playing. Every step was above where the solve finishes anyway, so
        // the budget saved nothing and cost the clock reads -- with it armed
        // the Twin Reverb went from 0.7 % of callbacks missed to 4.1 %.
        //
        // Eight is the first step because it is still above every voice's
        // normal finish, so a block that is only slightly late loses nothing.
        // Below that the steps are where a solve that has stopped converging
        // gets cut off. The floor is `Simulation`'s, not this one's.
        match over {
            o if o < 1.0 => 32,
            o if o < 1.25 => 8,
            o if o < 1.6 => 6,
            _ => 4,
        }
    }
}

impl Plugin for GainStageFx {
    const NAME: &'static str = "GainStageFx";
    const VENDOR: &'static str = "BurningTreeC";
    const URL: &'static str = "https://github.com/BurningTreeC/gainstagefx";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        crate::editor::create(
            self.params.clone(),
            self.meters.clone(),
            self.params.editor_state.clone(),
        )
    }

    fn initialize(
        &mut self,
        _layout: &AudioIOLayout,
        buffer: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer.sample_rate as f64;
        // The work budget exists because a realtime callback has a deadline.
        // An offline render has none: the host calls `process` back to back
        // until the material is done, and a bounce must come out the same
        // however fast or slow the machine was. Without this the budget would
        // read its own compute time against a deadline nobody is holding it
        // to, and pinch a render that had all the time in the world -- and any
        // voice slower than realtime would be pinched in *every* bounce.
        //
        // `Buffered` is the VST3 mode where a host works ahead to loosen the
        // constraint. There is still a deadline behind it, so the budget stays
        // on.
        self.budget.armed = Budget::WORKS && !matches!(buffer.process_mode, ProcessMode::Offline);
        self.oversampling = self.params.oversampling.value();

        // Built here, on the main thread, where allocating and hunting for an
        // operating point are both allowed. Nothing after this point does
        // either.
        //
        // One chain whatever the layout says, because the circuit is fed a
        // mono sum of the host's channels. A second one would solve the same
        // equations for the same answer.
        let mut chain = Chain::new(self.sample_rate);
        chain.set_oversampling(self.oversampling.factor());
        self.chain = Some(chain);

        // Reported once, and the same whatever the oversampling is set to.
        context.set_latency_samples(LATENCY);
        true
    }

    fn reset(&mut self) {
        if let Some(chain) = self.chain.as_mut() {
            chain.reset();
        }
        self.peak = 0.0;
        self.summing.reset();
        self.meters.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let samples = buffer.samples() as u32;
        let oversampling = self.params.oversampling.value();
        if oversampling != self.oversampling {
            self.oversampling = oversampling;
        }

        let circuit = self.params.circuit.value();
        // A circuit with no diodes in it ignores the choice, so it is pinned
        // rather than left wherever the greyed-out control happens to sit --
        // otherwise the same patch would load into a different array slot
        // depending on a control that does nothing.
        let settings = Settings {
            gain: circuit.voice(),
            diode: if circuit.has_diodes() {
                self.params.diode.value().voice()
            } else {
                Diode::Silicon.voice()
            },
            amplifier: if circuit.has_amplifier() {
                self.params.amplifier.value().voice()
            } else {
                Amplifier::Valve.voice()
            },
            iron: self.params.iron.value().voice(),
            tone: self.params.tone.value().voice(),
            cabinet: self.params.cabinet.value().voice(),
            // `next_step`, not `next`. These four reach a circuit, and
            // anything that reaches a circuit moves once a block -- so the
            // smoother has to be advanced by the whole block, not by one
            // sample. Advancing it by one made a twenty millisecond ramp take
            // twenty milliseconds times the block size, which at 512 samples
            // is ten seconds, and made the ramp's length depend on the host's
            // buffer setting.
            drive: self.params.drive.smoothed.next_step(samples) as f64,
            master: self.params.master.smoothed.next_step(samples) as f64,
            graphic: [
                self.params.eq60.smoothed.next_step(samples) as f64,
                self.params.eq240.smoothed.next_step(samples) as f64,
                self.params.eq750.smoothed.next_step(samples) as f64,
                self.params.eq2200.smoothed.next_step(samples) as f64,
                self.params.eq6600.smoothed.next_step(samples) as f64,
            ],
            bass: self.params.bass.smoothed.next_step(samples) as f64,
            mid: self.params.mid.smoothed.next_step(samples) as f64,
            treble: self.params.treble.smoothed.next_step(samples) as f64,
            reverb: self.params.reverb.smoothed.next_step(samples) as f64,
            speed: self.params.speed.smoothed.next_step(samples) as f64,
            intensity: self.params.intensity.smoothed.next_step(samples) as f64,
            oversampling: oversampling.factor(),
        };

        // Everything that reaches a circuit moves once a block, not once a
        // sample. Switching a circuit resets it, and moving a control
        // invalidates the matrix so the next sample rebuilds it and hunts the
        // operating point again -- either of those at audio rate is ruinous.
        // It all goes through `Chain::apply`, so that forgetting one is a
        // change to that function rather than a line missing from here.
        let Some(chain) = self.chain.as_mut() else {
            return ProcessStatus::Normal;
        };
        chain.apply(&settings);

        // Only when there is an answer to hunt. `find_operating_point` solves
        // the circuit with no signal in it and sets every capacitor's charge
        // from the result, so calling it on a chain that is already running
        // throws the audio in flight away. `needs_operating_point` is false
        // the moment a sample has been processed -- see `tests/knobs.rs`.
        if chain.needs_operating_point() {
            chain.find_operating_point();
        }

        // A peak that falls slowly enough to read but still follows playing.
        let decay = (-1.0 / (0.3 * self.sample_rate)).exp();
        // One pole toward the summing divisor, about fifty milliseconds.
        let ramp = 1.0 - (-1.0 / (0.05 * self.sample_rate)).exp();
        let nominal = 10f64.powf(NOMINAL_DBFS / 20.0);

        // The work budget. See `Budget`.
        //
        // Every block starts with the full ceiling, so a block that fits is
        // bit-for-bit what it would have been without any of this, and two
        // renders of the same material agree unless the clock intervened.
        let bypassed = self.params.bypass.value();
        let total = samples as usize;
        let deadline = total as f64 / self.sample_rate as f64;
        let started = std::time::Instant::now();
        let mut ceiling = 32usize;
        chain.set_pass_ceiling(ceiling);
        let before = chain.pinched();
        // Copied out because `chain` holds a mutable borrow of `self` for the
        // length of the block, and these are only read.
        let budget = Budget { ..self.budget };
        let mut peak = self.peak;

        for (done, mut frame) in buffer.iter_samples().enumerate() {
            // Read the clock every `stride` samples rather than every one: at
            // twenty-odd nanoseconds a reading, per sample it would be a tenth
            // of a per cent of a 64-sample budget spent measuring how much of
            // the budget is left.
            if budget.armed && done > 0 && done % budget.stride == 0 {
                let want = budget.ceiling(done, total, started.elapsed().as_secs_f64(), deadline);
                if want != ceiling {
                    ceiling = want;
                    chain.set_pass_ceiling(ceiling);
                }
            }
            // Float parameters are read per-sample through nih-plug's 20ms
            // linear smoothers so that knob movements are ramped instead of
            // jumping at block boundaries.  Enum parameters (circuit, diode,
            // iron, ...) are read once per block in `settings` above.
            let input_trim = util::db_to_gain(self.params.input_trim.smoothed.next()) as f64;
            let output_trim = util::db_to_gain(self.params.output_trim.smoothed.next()) as f64;
            let mix = self.params.mix.smoothed.next() as f64;

            // One jack, one signal. See `Summing` for why the divisor is
            // what it is; it is the part that has to be right.
            self.summing.start();
            for (index, sample) in frame.iter_mut().enumerate() {
                self.summing.add(index, *sample as f64);
            }
            let input = self.summing.finish(ramp) * input_trim;

            let dry = chain.delayed_dry(input);
            // Bypassed, the output is the *delayed* dry signal. A plugin that
            // reports latency and then hands back the undelayed input when it
            // is switched out makes the track jump forward by the reported
            // amount, which is why nih-plug asks a latent plugin to implement
            // its own bypass. The circuits still run: they cost what they cost
            // either way, and a chain frozen while it is switched out comes
            // back holding a stale answer, which is a click.
            let wet = chain.process(input);
            let out = if bypassed {
                (dry * output_trim) as f32
            } else {
                ((dry * (1.0 - mix) + wet * mix) * output_trim) as f32
            };
            for sample in frame.iter_mut() {
                *sample = out;
            }

            let level = input.abs();
            peak = if level > peak { level } else { peak * decay };
        }

        // What the budget cost, if anything. Counted rather than logged: the
        // audio thread may not log, and a session that is degrading has to be
        // visible somewhere other than in the sound.
        let after = chain.pinched();
        self.peak = peak;
        if ceiling < 32 {
            self.pinched_blocks = self.pinched_blocks.saturating_add(1);
        }
        self.pinched_samples = self.pinched_samples.saturating_add(after - before);

        // The meter reads relative to the level the voices were calibrated at,
        // so its zero is where the panel means what it says.
        self.meters
            .set_input_db(20.0 * (self.peak / nominal).max(1e-6).log10() as f32);
        ProcessStatus::Normal
    }
}

impl ClapPlugin for GainStageFx {
    const CLAP_ID: &'static str = "com.burningtreec.gainstagefx";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Circuit modelled gain stages, from subtle saturation to high gain");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Distortion,
    ];
}

impl Vst3Plugin for GainStageFx {
    const VST3_CLASS_ID: [u8; 16] = *b"GainStageFxBTC01";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_clap!(GainStageFx);
nih_export_vst3!(GainStageFx);

#[cfg(test)]
mod budget {
    use super::Budget;

    /// A block inside its budget is not touched, and that is what makes two
    /// renders of the same material agree. The ceiling only moves when the
    /// clock says the block is running out of time.
    #[test]
    fn a_block_that_fits_is_left_alone() {
        let b = Budget::new();
        // Halfway through, having used a third of the time.
        assert_eq!(b.ceiling(32, 64, 0.000_444, 0.001_333), 32);
        // Halfway through, having used exactly half: on the line, inside the
        // slack, still untouched.
        assert_eq!(b.ceiling(32, 64, 0.000_666, 0.001_333), 32);
        // And a block that has used none of it.
        assert_eq!(b.ceiling(16, 64, 0.0, 0.001_333), 32);
    }

    /// Past the slack it comes down in defined steps, and never below what
    /// `Simulation` will accept -- the floor is enforced there, not here, so
    /// no caller can get under it.
    #[test]
    fn a_block_running_late_is_pinched_in_steps() {
        // Armed explicitly: the shipped budget is off, and why is
        // `Budget::WORKS`. This is a test of the arithmetic, which still has
        // to be right for the day a mechanism that works uses it.
        let mut b = Budget::new();
        b.armed = true;
        let deadline = 0.001_333;
        let expected = deadline / 2.0;
        let at = |factor: f64| b.ceiling(32, 64, expected * factor, deadline);
        assert_eq!(at(1.0), 32, "on pace");
        assert_eq!(at(1.3), 8, "over the slack");
        assert_eq!(at(1.9), 6);
        assert_eq!(at(3.0), 4, "a long way over");
        assert_eq!(at(50.0), 4, "and no further");
    }

    /// The budget ships disarmed, and that is a measurement rather than an
    /// oversight. See `Budget::WORKS`: armed, it made the misses worse on
    /// every voice that had any, and nearly doubled the Twin Reverb's worst
    /// callback. A guarantee that costs more than it saves is not a guarantee.
    #[test]
    fn the_budget_is_off_because_it_did_not_work() {
        assert!(!Budget::WORKS);
        let b = Budget::new();
        assert!(!b.armed);
        // And with it off, no block is ever pinched however late it runs.
        assert_eq!(b.ceiling(32, 64, 1.0, 0.001), 32);
    }

    /// An offline render has no deadline. The host calls `process` back to
    /// back until the material is done, and a bounce must come out the same
    /// however fast the machine was -- so the budget is disarmed for it in
    /// `initialize`. Without this, any voice slower than realtime would be
    /// pinched in every bounce.
    #[test]
    fn an_offline_render_is_never_pinched() {
        let mut b = Budget::new();
        b.armed = false;
        assert_eq!(b.ceiling(32, 64, 10.0, 0.001_333), 32);
        assert_eq!(b.ceiling(63, 64, 1_000.0, 0.001_333), 32);
    }

    /// Degenerate inputs do not produce a ceiling of zero, which would mean a
    /// sample that never solves at all.
    #[test]
    fn nonsense_still_gives_a_usable_ceiling() {
        let b = Budget::new();
        assert_eq!(b.ceiling(0, 0, 1.0, 0.001), 32);
        assert_eq!(b.ceiling(1, 64, 1.0, 0.0), 32);
        assert!(b.ceiling(1, 64, f64::MAX, 0.001) >= 12);
    }
}

#[cfg(test)]
mod summing {
    use super::Summing;

    /// The ramp coefficient the tests use. One at a time, so the divisor
    /// arrives at its target immediately and each case can be read as the
    /// steady state it settles to.
    const NOW: f64 = 1.0;

    fn run(frames: &[&[f64]]) -> Vec<f64> {
        let mut s = Summing::new();
        frames
            .iter()
            .map(|frame| {
                s.start();
                for (index, &sample) in frame.iter().enumerate() {
                    s.add(index, sample);
                }
                s.finish(NOW)
            })
            .collect()
    }

    /// The one that was shipped wrong.
    ///
    /// A mono track does not reach a plugin as one channel in most hosts. It
    /// reaches it as two, with the second one digital silence -- and a guitar
    /// panned hard left is the same shape. Dividing by the channel *count*
    /// halves it, and the plugin is six decibels quiet before it has done
    /// anything. Reported from a DAW as "the input signal is way too low.
    /// Really low."
    #[test]
    fn a_silent_second_channel_does_not_halve_the_guitar() {
        let out = run(&[&[0.5, 0.0], &[-0.8, 0.0], &[0.25, 0.0]]);
        assert_eq!(out, vec![0.5, -0.8, 0.25]);
    }

    /// And the other way round, which is what the divisor is for at all: a
    /// mono track that *is* duplicated across both channels must not arrive
    /// six decibels hot, into circuits calibrated in volts.
    #[test]
    fn a_duplicated_mono_track_arrives_at_its_own_level() {
        let out = run(&[&[0.5, 0.5], &[-0.8, -0.8], &[0.25, 0.25]]);
        assert_eq!(out, vec![0.5, -0.8, 0.25]);
    }

    /// A genuinely stereo source is summed and shared out, which is what
    /// plugging one into a single amplifier does.
    #[test]
    fn a_stereo_source_is_averaged() {
        let out = run(&[&[0.4, 0.2], &[0.6, -0.2]]);
        assert_eq!(out, vec![0.30000000000000004, 0.19999999999999998]);
    }

    /// A single channel is itself, untouched.
    #[test]
    fn one_channel_is_left_alone() {
        let out = run(&[&[0.5], &[-0.3]]);
        assert_eq!(out, vec![0.5, -0.3]);
    }

    /// `live` latches, so a sine crossing zero does not change the divisor.
    /// This is why it is not a threshold: gating on "is this sample zero"
    /// would make the level jump every half cycle.
    #[test]
    fn a_zero_crossing_does_not_change_the_level() {
        // Both channels have been heard from, then both pass through zero.
        let out = run(&[&[0.5, 0.5], &[0.0, 0.0], &[0.5, 0.5]]);
        assert_eq!(out, vec![0.5, 0.0, 0.5]);
    }

    /// A channel that arrives late is a ramp, not a step. Six decibels
    /// between two samples is a click.
    #[test]
    fn a_channel_arriving_late_is_ramped() {
        let mut s = Summing::new();
        let mut feed = |a: f64, b: f64, ramp: f64| {
            s.start();
            s.add(0, a);
            s.add(1, b);
            s.finish(ramp)
        };
        // A realistic coefficient: fifty milliseconds at 48 kHz.
        let ramp = 1.0 - (-1.0f64 / (0.05 * 48_000.0)).exp();
        assert_eq!(feed(0.5, 0.0, ramp), 0.5);
        // The right channel opens up. The sum doubles, and the divisor has
        // barely moved, so the step must be small -- but it must be moving.
        let first = feed(0.5, 0.5, ramp);
        assert!(
            (first - 1.0).abs() < 0.001,
            "the first sample after a channel opens should still be at the \
             old divisor: {first}"
        );
        for _ in 0..48_000 {
            feed(0.5, 0.5, ramp);
        }
        let settled = feed(0.5, 0.5, ramp);
        assert!(
            (settled - 0.5).abs() < 1e-6,
            "and it should settle on the average: {settled}"
        );
    }

    /// Silence in, silence out, whatever the bookkeeping thinks.
    #[test]
    fn silence_stays_silent() {
        assert_eq!(run(&[&[0.0, 0.0], &[0.0, 0.0]]), vec![0.0, 0.0]);
    }
}
