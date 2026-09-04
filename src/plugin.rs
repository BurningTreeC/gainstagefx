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

    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

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
            drive: self.params.drive.value() as f64,
            bass: self.params.bass.value() as f64,
            mid: self.params.mid.value() as f64,
            treble: self.params.treble.value() as f64,
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

        let input_trim = util::db_to_gain(self.params.input_trim.value()) as f64;
        let output_trim = util::db_to_gain(self.params.output_trim.value()) as f64;
        let mix = self.params.mix.value() as f64;

        // A peak that falls slowly enough to read but still follows playing.
        let decay = (-1.0 / (0.3 * self.sample_rate)).exp();
        let nominal = 10f64.powf(NOMINAL_DBFS / 20.0);
        // The schematic models are physical mono devices: pedals and guitar
        // preamps with one input and one output. Run their wet path once for
        // a stereo host buffer, then present that mono device signal on both
        // channels. This halves their solver load, which is essential for
        // live use because each nonlinear model is already costly at the
        // host rate. Dry remains per-channel, so partial wet/dry mixes retain
        // the source channel layout.
        let mono_modelled = circuit.is_modelled() && self.channels.len() == 2;

        for mut frame in buffer.iter_samples() {
            if mono_modelled && frame.len() == 2 {
                let left = *frame.get_mut(0).expect("stereo left sample") as f64 * input_trim;
                let right = *frame.get_mut(1).expect("stereo right sample") as f64 * input_trim;
                let wet = self.channels[0].process((left + right) * 0.5);
                let dry_left = self.channels[0].delayed_dry(left);
                let dry_right = self.channels[1].delayed_dry(right);

                *frame.get_mut(0).expect("stereo left sample") =
                    ((dry_left * (1.0 - mix) + wet * mix) * output_trim) as f32;
                *frame.get_mut(1).expect("stereo right sample") =
                    ((dry_right * (1.0 - mix) + wet * mix) * output_trim) as f32;

                let level = left.abs().max(right.abs());
                self.peak = if level > self.peak {
                    level
                } else {
                    self.peak * decay
                };
                continue;
            }

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
