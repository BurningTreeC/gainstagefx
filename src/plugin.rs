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
    /// One chain per channel. Each owns every circuit in the catalogue, so
    /// changing the selection while playing does not allocate.
    channels: Vec<Chain>,
    sample_rate: f64,
    oversampling: Oversampling,
    peak: f64,
}

impl Default for GainStageFx {
    fn default() -> Self {
        crate::dsp::time::enable_ftz_daz();
        Self {
            params: Arc::new(GainStageParams::default()),
            meters: Arc::new(Meters::default()),
            channels: Vec::new(),
            sample_rate: 48_000.0,
            oversampling: Oversampling::Four,
            peak: 0.0,
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
        layout: &AudioIOLayout,
        buffer: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer.sample_rate as f64;
        self.oversampling = self.params.oversampling.value();
        let channels = layout.main_output_channels.map_or(2, |c| c.get()) as usize;

        // Built here, on the main thread, where allocating and hunting for an
        // operating point are both allowed. Nothing after this point does
        // either.
        self.channels = (0..channels)
            .map(|_| {
                let mut chain = Chain::new(self.sample_rate);
                chain.set_oversampling(self.oversampling.factor());
                chain
            })
            .collect();

        // Reported once, and the same whatever the oversampling is set to.
        context.set_latency_samples(LATENCY);
        true
    }

    fn reset(&mut self) {
        for chain in &mut self.channels {
            chain.reset();
        }
        self.peak = 0.0;
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
            bass: self.params.bass.smoothed.next_step(samples) as f64,
            mid: self.params.mid.smoothed.next_step(samples) as f64,
            treble: self.params.treble.smoothed.next_step(samples) as f64,
            oversampling: oversampling.factor(),
        };

        // Everything that reaches a circuit moves once a block, not once a
        // sample. Switching a circuit resets it, and moving a control
        // invalidates the matrix so the next sample rebuilds it and hunts the
        // operating point again -- either of those at audio rate is ruinous.
        // It all goes through `Chain::apply`, so that forgetting one is a
        // change to that function rather than a line missing from here.
        for chain in &mut self.channels {
            chain.apply(&settings);
        }

        // Both channels run the same circuit with the same settings, so their
        // DC operating point is identical and only one of them needs to hunt
        // it. Channel 0 does, and the rest are handed the answer.
        //
        // Only when there is an answer to hand over. `apply_operating_point`
        // sets every capacitor's charge from the voltages it is given, so
        // calling it on a channel that is already running does not seed a
        // solve -- it throws that channel's running state away and replaces it
        // with channel 0's. Done once a block, as this used to be, the right
        // channel of a TS808 measured three decibels of residual against the
        // same channel run on its own, and its output changed with the host's
        // buffer size: 64 against 512 samples differed by three and a half
        // decibels of residual. `examples/sharing.rs` is that measurement.
        //
        // `split_at_mut` rather than a copy into a scratch vector, because a
        // `Vec` here is an allocation on the audio thread.
        if self.channels.len() >= 2 && self.channels[0].needs_operating_point() {
            let (first, rest) = self.channels.split_at_mut(1);
            first[0].find_operating_point();
            for chain in rest {
                chain.share_operating_point_from(first[0].operating_point());
                if let Some(op) = first[0].iron_operating_point() {
                    chain.share_iron_operating_point_from(op);
                }
                // The power stage as well. `Chain::find_operating_point` hunts
                // all three -- gain, iron and power -- but only two of them
                // were ever handed on, so every channel but the first was
                // left to hunt its own power stage on its next sample, inside
                // the callback, which is the one place it must not happen.
                if let Some(op) = first[0].power_operating_point() {
                    chain.share_power_operating_point_from(op);
                }
            }
        }

        // A peak that falls slowly enough to read but still follows playing.
        let decay = (-1.0 / (0.3 * self.sample_rate)).exp();
        let nominal = 10f64.powf(NOMINAL_DBFS / 20.0);

        for mut frame in buffer.iter_samples() {
            // Float parameters are read per-sample through nih-plug's 20ms
            // linear smoothers so that knob movements are ramped instead of
            // jumping at block boundaries.  Enum parameters (circuit, diode,
            // iron, ...) are read once per block in `settings` above.
            let input_trim = util::db_to_gain(self.params.input_trim.smoothed.next()) as f64;
            let output_trim = util::db_to_gain(self.params.output_trim.smoothed.next()) as f64;
            let mix = self.params.mix.smoothed.next() as f64;

            for (index, sample) in frame.iter_mut().enumerate() {
                let Some(chain) = self.channels.get_mut(index) else {
                    continue;
                };
                let input = *sample as f64 * input_trim;
                let wet = chain.process(input);
                let dry = chain.delayed_dry(input);
                *sample = ((dry * (1.0 - mix) + wet * mix) * output_trim) as f32;

                if index == 0 {
                    let level = input.abs();
                    self.peak = if level > self.peak {
                        level
                    } else {
                        self.peak * decay
                    };
                }
            }
        }

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
