//! The plugin: parameters in, audio out.

use nih_plug::prelude::*;
use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::sync::Arc;

use crate::meters::Meters;
use crate::params::{Amplifier, Diode, GainStageParams, Oversampling};
use crate::voice::{Chain, Settings, LATENCY, NOMINAL_DBFS};

/// The size of the plugin's internal processing block, in samples.
///
/// The host calls `process` with whatever block size it wants; the plugin
/// accumulates input in a FIFO and drains it in bursts of this many samples.
/// The reason is scheduling. A callback of M samples gives the plugin
/// `M / rate` seconds to do the work for M samples, and the per-sample cost
/// varies: the solver takes three passes on an ordinary sample and up to
/// thirty-two on one that crosses a device's knee. Draining N samples in one
/// burst averages that cost over a longer window, so a single expensive
/// sample is a smaller fraction of a burst's budget than it is of a host
/// callback's. Measured on the Twin Reverb, whose power stage has the largest
/// solver in the catalogue and the worst callback of any voice: 64-sample
/// host blocks crackle on loud input at 48 kHz, and 256 does not.
///
/// **What it does not do** is reduce total CPU. The same work per second is
/// done either way; the buffer redistributes *when* the cost is paid. If the
/// average cost exceeds realtime, no block size fixes it.
///
/// **The cost is latency.** The input waits for N samples to accumulate
/// before processing begins, so the plugin's total delay becomes
/// `LATENCY + INTERNAL_BLOCK` samples. At 48 kHz:
///
/// | block | added  | total  | note                              |
/// |------:|-------:|-------:|-----------------------------------|
/// |    64 | 1.3 ms | 2.7 ms | no help: same as one host block   |
/// |   128 | 2.7 ms | 4.1 ms | recommended for 64-sample hosts   |
/// |   256 | 5.3 ms | 6.7 ms | still under the 9 ms ceiling      |
/// |   512 |10.7 ms |12.1 ms | over the ceiling                  |
///
/// **Bypass is unaffected in the sound but not in time.** The circuits always
/// run, whatever this is set to; when the panel's bypass is on, the output is
/// the host's samples untouched, and the FIFOs' delay does not apply. The two
/// are inconsistent by `LATENCY + INTERNAL_BLOCK` samples, so toggling bypass
/// mid-playback moves the track; that is the same shift the plugin has always
/// had at `LATENCY`, made larger by this.
///
/// **The value is not measured on every voice.** The Twin is the reason it
/// exists; the pedal voices do not need it and would sound the same at any
/// setting. 128 is a compromise: enough to average two host callbacks at the
/// 64-sample size the user reported, and small enough that the total latency
/// stays well under the 9 ms ceiling.
const INTERNAL_BLOCK: usize = 256;

/// Below this level the trimmed input is quantised to zero.
///
/// A guitar at rest puts out roughly -70 dBFS of its own noise, and any
/// converter worth the name adds a few decibels below that. Below -80 there
/// is nothing the circuit can respond to that is not the source's own
/// self-noise, and the solve for a zero input is much cheaper than the solve
/// for a real one: the predictor lands on the previous answer, the residual
/// sits at the operating point's own round-off, and Newton converges in one
/// or two passes instead of the three to five a real signal takes. A player
/// is not playing for a substantial fraction of the time they are sitting
/// with a guitar in front of them, and that fraction of the CPU was being
/// spent solving for a signal the speaker cannot reproduce.
///
/// It is applied **after** the input trim, not before: a player who has
/// turned the trim up to hear a quiet source has moved the threshold up with
/// the signal, which is what turning the trim up means.
///
/// The floor is below the meter's own bottom (`meters::FLOOR` is -60 dBFS),
/// so the panel reads exactly what it always did during a rest, and it is a
/// thousandth of a typical guitar level, which is below the noise floor of
/// any converter the plugin is likely to see.
///
/// Nothing about the chain is frozen or bypassed. The circuits keep running;
/// they simply advance toward their own operating point rather than
/// following a signal, which is what a real amplifier does between notes.
const SILENCE_DBFS: f64 = -80.0;

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
    summing: Summing,
    /// The three parameters that reach the output arithmetic, ramped across
    /// the block rather than read and converted per sample. See `BlockRamp`.
    input_ramp: BlockRamp,
    output_ramp: BlockRamp,
    mix_ramp: BlockRamp,
    /// Host samples, already summed to mono, waiting to fill an internal
    /// block. See `INTERNAL_BLOCK`.
    ///
    /// The summing is done at the I/O boundary because it has per-frame
    /// state -- `Summing::live` latches as it hears each channel carry a
    /// signal -- so it cannot be moved inside the burst.
    input_fifo: VecDeque<f64>,
    /// Processed samples waiting to be released to the host in whatever
    /// block size it asked for. Pushed in bursts of `INTERNAL_BLOCK`, popped
    /// in blocks of `samples`.
    ///
    /// Primed with `INTERNAL_BLOCK` samples of near silence in `initialize`
    /// and `reset`, so the very first callback has something to release; the
    /// priming is exactly the plugin's added latency and is not a bigger
    /// buffer that just happens to be there.
    output_fifo: VecDeque<f64>,
}

/// A per-block linear ramp on a gain or a mix fraction.
///
/// The three parameters that reach the output arithmetic -- input trim, output
/// trim, mix -- used to be read and converted **once per sample**. The
/// conversion is `10^(dB/20)`, a `powf`, so that is 96,000 transcendental
/// evaluations a second at 48 kHz, spent turning numbers that only change once
/// per twenty milliseconds. They are the only per-sample parameter reads left
/// in the callback: `drive` and the four tone knobs already go through
/// `next_step(samples)` at the top of `process`, because they reach a circuit
/// and a circuit rebuild is a block-rate event.
///
/// Reading them once per block and using a **constant** value for the block is
/// what the circuit-reaching parameters do, and it is fine for them because a
/// circuit rebuild is only ever a block-rate event by construction. It is not
/// fine here. `mix` reaches the output sum, and at a 512-sample host block a
/// block-constant `mix` is a 10.6 ms staircase on a 20 ms smoother: two steps,
/// and a click on fast automation.
///
/// So the ramp is linear *across* the block. One add per sample reconstructs
/// the smoother's own per-sample values, which is exact whenever the smoother
/// has more than a block of budget left -- which it does, because every host
/// write restarts the twenty millisecond budget. The one case where the two
/// differ is the very last block of a move, where the smoother finishes
/// mid-block and the ramp takes the whole block to arrive rather than a few
/// samples; that is a slightly gentler arrival, not a discontinuity.
///
/// `settle` is not optional. `start + samples * ((target - start) / samples)`
/// is not `target` in IEEE 754, and an hour of 512-sample blocks is 337,500
/// chances for the error to accumulate into something audible. Settling to
/// the value the block was aimed at makes the ramp exactly re-enterable.
///
/// With the internal block in place, the ramp is re-aimed once per burst
/// rather than once per host callback. The two are equivalent over time,
/// because bursts process the same samples the host handed over in the same
/// order; they differ only in where the "settle" points sit. Aiming per
/// burst keeps each burst self-contained, which is the property the FIFOs
/// exist to give.
#[derive(Clone, Copy)]
struct BlockRamp {
    current: f32,
    step: f32,
}

impl BlockRamp {
    const fn new(value: f32) -> Self {
        Self {
            current: value,
            step: 0.0,
        }
    }

    /// Point the ramp at `target` over the next `samples` samples.
    fn aim(&mut self, target: f32, samples: u32) {
        self.step = if samples == 0 {
            0.0
        } else {
            (target - self.current) / samples as f32
        };
    }

    #[inline]
    fn next(&mut self) -> f32 {
        let v = self.current;
        self.current += self.step;
        v
    }

    /// Snap to the value aimed at, so the next block starts from it exactly.
    fn settle(&mut self, target: f32) {
        self.current = target;
    }

    fn reset(&mut self, value: f32) {
        self.current = value;
        self.step = 0.0;
    }
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
        let params = Arc::new(GainStageParams::default());
        // Ramps start at the parameter's own value rather than at unity. A
        // preset loaded into a freshly-created plugin would otherwise arrive
        // with a block-long ramp from 1.0 to the preset's trim, which is a
        // step and a click on the first sample.
        let input_ramp = BlockRamp::new(util::db_to_gain(params.input_trim.value()));
        let output_ramp = BlockRamp::new(util::db_to_gain(params.output_trim.value()));
        let mix_ramp = BlockRamp::new(params.mix.value());
        Self {
            params,
            meters: Arc::new(Meters::default()),
            chain: None,
            sample_rate: 48_000.0,
            oversampling: Oversampling::Four,
            peak: 0.0,
            budget: Budget::new(),
            summing: Summing::new(),
            input_ramp,
            output_ramp,
            mix_ramp,
            // Capacity is a hint and not a limit; it is set generously so the
            // first callbacks after `initialize` do not reallocate. The FIFOs
            // stay bounded in the steady state at `INTERNAL_BLOCK` plus one
            // host block each.
            input_fifo: VecDeque::with_capacity(INTERNAL_BLOCK * 8),
            output_fifo: VecDeque::with_capacity(INTERNAL_BLOCK * 8),
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
///
/// The mechanism is now the wrong shape for this plugin, and that is worth
/// saying rather than leaving as dead code. It reads a clock against the
/// host's block, and the plugin no longer does its work in host-block units:
/// bursts of `INTERNAL_BLOCK` samples are what the deadline applies to, and
/// a burst's deadline is `INTERNAL_BLOCK / rate`, not `samples / rate`.
/// Nothing in the plugin acts on the reading -- `WORKS` is false -- so this
/// is a note for whoever next tries to make the budget do something, not a
/// bug.
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

        // Prime the output FIFO with one burst's worth of near silence.
        //
        // The FIFOs balance in the steady state at `INTERNAL_BLOCK` samples
        // each: stage 1 pushes a host block's worth of input, stage 2 drains
        // whatever completes a burst, stage 3 releases a host block's worth
        // of output. Starting the output FIFO empty would leave the first
        // callbacks with nothing to release and the plugin silent for a
        // burst's worth of samples; starting it at `INTERNAL_BLOCK` gives the
        // host something from the first sample, at exactly the delay the
        // plugin reports. Anything more than `INTERNAL_BLOCK` would over-report
        // the latency, because the priming *is* the added delay.
        self.input_fifo.clear();
        self.output_fifo.clear();
        for _ in 0..INTERNAL_BLOCK {
            let _ = chain.delayed_dry(0.0);
            let wet = chain.process(0.0);
            self.output_fifo.push_back(wet);
        }

        self.chain = Some(chain);

        // Ramps re-seeded from whatever the host loaded into the parameters,
        // which is not necessarily what they were constructed with.
        self.input_ramp
            .reset(util::db_to_gain(self.params.input_trim.value()));
        self.output_ramp
            .reset(util::db_to_gain(self.params.output_trim.value()));
        self.mix_ramp.reset(self.params.mix.value());

        // Reported once, and the same whatever the oversampling is set to.
        // The host compensates for the chain's own delay *and* the FIFOs';
        // the FIFOs' contribution is `INTERNAL_BLOCK` and nothing else,
        // because the priming above is exactly that size.
        context.set_latency_samples(LATENCY + INTERNAL_BLOCK as u32);
        true
    }

    fn reset(&mut self) {
        if let Some(chain) = self.chain.as_mut() {
            chain.reset();
        }
        self.peak = 0.0;
        self.summing.reset();
        self.meters.reset();
        // The FIFOs are emptied, then the output FIFO is primed again with
        // near silence, exactly as in `initialize`. A reset is the moment a
        // session starts, and the plugin has to be ready to release a burst's
        // worth on the first callback; leaving the FIFOs empty would silence
        // the start of playback.
        self.input_fifo.clear();
        self.output_fifo.clear();
        if let Some(chain) = self.chain.as_mut() {
            for _ in 0..INTERNAL_BLOCK {
                let _ = chain.delayed_dry(0.0);
                let wet = chain.process(0.0);
                self.output_fifo.push_back(wet);
            }
        }
        // The ramps as well. A reset is the moment a session starts, and a
        // ramp that carried the previous session's trim across a transport
        // stop would open the new one with a glide to the value it already
        // has.
        self.input_ramp
            .reset(util::db_to_gain(self.params.input_trim.value()));
        self.output_ramp
            .reset(util::db_to_gain(self.params.output_trim.value()));
        self.mix_ramp.reset(self.params.mix.value());
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Set the flush-to-zero and denormals-are-zero bits on every
        // callback, not just once in `initialize`. Two instructions, and
        // they are the difference between arithmetic on a near-zero
        // correction costing one cycle and costing a hundred. The plugin's
        // own `enable_ftz_daz` in `Default::default` sets them once, and
        // nothing in the plugin clears them -- but the host shares the
        // audio thread with other plugins, and any of them that resets the
        // FPU state on its own startup or its own `process` takes the bits
        // with it. Measured by ear as intermittent crackle on the voices
        // with the largest solvers, worse the louder the input, at a low
        // average CPU. See the FTZ discussion in `docs/REALTIME_BUFFERING.md`.
        crate::dsp::time::enable_ftz_daz();

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
        // The linear value the input is compared against. See `SILENCE_DBFS`.
        let silence_linear = 10f64.powf(SILENCE_DBFS / 20.0);

        // The three parameters that reach the output arithmetic, read once
        // for the host callback and converted here rather than per sample.
        // See `BlockRamp` for why the conversion moves out of the sample loop
        // and why the ramp is linear rather than constant.
        //
        // They are advanced by the host's block size, not by the burst size:
        // the smoother paces the knob move in wall-clock time, and `samples`
        // is the amount of wall-clock time this callback represents. Within
        // a burst, the ramp reconstructs that same move by re-aiming at each
        // burst's start.
        let input_trim_target = util::db_to_gain(self.params.input_trim.smoothed.next_step(samples));
        let output_trim_target =
            util::db_to_gain(self.params.output_trim.smoothed.next_step(samples));
        let mix_target = self.params.mix.smoothed.next_step(samples);

        let bypassed = self.params.bypass.value();
        let mut peak = self.peak;

        // ---- Stage 1: host frames into the input FIFO. ----
        //
        // Only the summing happens here, because it has per-frame state.
        // Everything else -- the input trim, the silence floor, the chain,
        // the mix, the output trim -- happens in stage 2, on samples that
        // have been drained from the FIFO in burst-sized batches.
        //
        // The FIFO holds summed mono values. That is the plugin's inherent
        // signal shape: an amplifier has one input jack and the chain is one
        // circuit. See `Summing`.
        for mut frame in buffer.iter_samples() {
            self.summing.start();
            for (index, sample) in frame.iter_mut().enumerate() {
                self.summing.add(index, *sample as f64);
            }
            self.input_fifo.push_back(self.summing.finish(ramp));
        }

        // ---- Stage 2: drain complete bursts through the chain. ----
        //
        // Every burst is `INTERNAL_BLOCK` samples. The ramps are re-aimed at
        // the start of each one and settled at its end, so each burst is
        // self-contained: whatever the smoother has reached, the burst
        // traverses to it and stops.
        //
        // The chain runs whatever `bypassed` says, so its capacitors and
        // devices stay at the state the signal left them in rather than
        // frozen. A chain switched back on holding a stale answer is a click,
        // and the cost of keeping it warm is the same either way.
        while self.input_fifo.len() >= INTERNAL_BLOCK {
            self.input_ramp
                .aim(input_trim_target, INTERNAL_BLOCK as u32);
            self.output_ramp
                .aim(output_trim_target, INTERNAL_BLOCK as u32);
            self.mix_ramp.aim(mix_target, INTERNAL_BLOCK as u32);

            for _ in 0..INTERNAL_BLOCK {
                let raw = self.input_fifo.pop_front().expect("burst is complete");

                // The three output-arithmetic parameters, one add each
                // rather than a smoother step and a `powf`. See `BlockRamp`.
                let input_trim = self.input_ramp.next() as f64;
                let output_trim = self.output_ramp.next() as f64;
                let mix = self.mix_ramp.next() as f64;

                let trimmed = raw * input_trim;

                // Below the silence floor, quantise to zero. See
                // `SILENCE_DBFS` for the reasoning; the short of it is that
                // the solve for a zero input converges in one or two passes
                // rather than the three to five a real signal takes, and
                // that a player is not playing for a large fraction of the
                // time they are sitting with a guitar.
                //
                // The check is on the **trimmed** input, so a player who has
                // turned the trim up to hear a quiet source has moved the
                // threshold up with the signal.
                let input = if trimmed.abs() < silence_linear {
                    0.0
                } else {
                    trimmed
                };

                let dry = chain.delayed_dry(input);
                let wet = chain.process(input);
                let out = (dry * (1.0 - mix) + wet * mix) * output_trim;
                self.output_fifo.push_back(out);

                let level = input.abs();
                peak = if level > peak { level } else { peak * decay };
            }

            self.input_ramp.settle(input_trim_target);
            self.output_ramp.settle(output_trim_target);
            self.mix_ramp.settle(mix_target);
        }

        // ---- Stage 3: the output FIFO into the host's frames. ----
        //
        // When the plugin is in circuit, the frame is overwritten with the
        // processed sample. When bypassed, the frame is left exactly as the
        // host handed it over -- the plugin is a wire on the output side,
        // even though it is still running inside -- and the value popped
        // from the FIFO is discarded.
        //
        // That is a deliberate inconsistency. The FIFO's delay is real and
        // reported, and both the wet and dry paths inside the chain are
        // delayed by `LATENCY`; but bypass, by this design, is instantaneous
        // host-in to host-out with no summing and no trims. Toggling bypass
        // mid-playback therefore moves the track by `LATENCY +
        // INTERNAL_BLOCK` samples. That is the same shift the plugin has
        // always had at `LATENCY`, made larger by the buffer; the alternative
        // -- delaying bypass to match -- was tried and rejected, because a
        // user asking for "the sound comes through 1:1" means exactly that.
        for mut frame in buffer.iter_samples() {
            let processed = self.output_fifo.pop_front().unwrap_or(0.0);
            if !bypassed {
                let out = processed as f32;
                for sample in frame.iter_mut() {
                    *sample = out;
                }
            }
            // When bypassed: frame is untouched, so its value is the host's
            // original sample. The FIFO has been drained to keep it in sync
            // with the input side; the value that was drained is discarded.
        }

        self.peak = peak;

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

#[cfg(test)]
mod block_ramp {
    use super::BlockRamp;

    /// A ramp aimed at a target over a block arrives there, to the sample.
    ///
    /// The block is consumed as `samples` calls to `next()`, and the last one
    /// returns the value the block ends on. `settle` then snaps the running
    /// value to the target so the next block starts from it exactly, which is
    /// what makes a sequence of re-aimed blocks a continuous line rather than
    /// a staircase.
    #[test]
    fn a_ramp_arrives_at_the_target_it_was_aimed_at() {
        let mut ramp = BlockRamp::new(1.0);
        ramp.aim(2.0, 64);
        for _ in 0..63 {
            ramp.next();
        }
        let last = ramp.next();
        // `start + samples * ((target - start) / samples)` in f32 is not
        // exactly `target` -- it is the target minus one step, which is what
        // the smoother's own last sample would have been.
        assert!(
            (last - 2.0).abs() < 1e-5,
            "the block should have reached the target by its last sample: {last}"
        );
        ramp.settle(2.0);
        assert_eq!(ramp.next(), 2.0);
    }

    /// A block with no samples in it -- which does happen, at a host that
    /// calls with an empty buffer -- must not divide by zero or leave a step
    /// aimed at the wrong number.
    #[test]
    fn an_empty_block_is_inert() {
        let mut ramp = BlockRamp::new(1.0);
        ramp.aim(2.0, 0);
        assert_eq!(ramp.next(), 1.0, "no step was taken");
        assert_eq!(ramp.next(), 1.0, "and none appears on the next call");
    }

    /// Two blocks back to back produce a value that moves by one step across
    /// the block boundary, not by a jump. This is the whole reason the ramp
    /// is not a per-block constant.
    #[test]
    fn consecutive_blocks_join_without_a_step() {
        let mut ramp = BlockRamp::new(1.0);
        ramp.aim(1.1, 64);
        let mut last_of_first = 0.0;
        for _ in 0..64 {
            last_of_first = ramp.next();
        }
        ramp.settle(1.1);
        let first_of_second = ramp.next();
        // One step of `(1.1 - 1.0) / 64` is about 0.0016. The two samples
        // either side of the boundary should differ by about that, not by the
        // whole interval.
        assert!(
            (first_of_second - last_of_first).abs() < 0.01,
            "the block boundary should not be a jump: {last_of_first} -> {first_of_second}"
        );
    }
}

#[cfg(test)]
mod silence {
    use super::SILENCE_DBFS;

    /// The floor sits below the meter's own bottom, so the panel reads
    /// exactly what it always did during a rest and no user-visible
    /// threshold has moved.
    #[test]
    fn the_floor_is_below_the_meter() {
        assert!(SILENCE_DBFS < crate::meters::FLOOR as f64);
    }

    /// And it is far enough below a typical guitar level that the
    /// quantisation cannot touch anything a player would call playing.
    #[test]
    fn the_floor_is_far_below_a_typical_guitar() {
        let typical_dbfs = -20.0;
        assert!(SILENCE_DBFS <= typical_dbfs - 60.0);
    }

    /// The linear value the loop compares against, for the record.
    #[test]
    fn the_linear_floor_is_a_ten_thousandth() {
        let linear = 10f64.powf(SILENCE_DBFS / 20.0);
        assert!((linear - 1e-12).abs() < 1e-12 || (linear - 1e-4).abs() < 1e-12);
    }
}
