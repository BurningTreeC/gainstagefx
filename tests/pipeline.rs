//! The pipelined chain is the serial chain, to the bit.
//!
//! `Chain::process_block` can run the chain's second half (power stage, iron,
//! make-up, decimators, tone, cabinet) on a `StageWorker` while the calling
//! thread runs the first half of later samples. It is only worth having if it
//! changes nothing, so every voice is played sample by sample, as a serial
//! block, pipelined on a live worker, and pipelined on one that never wakes
//! (so the caller reclaims the second half), and all four must agree exactly,
//! at every oversampling factor the chain supports.
use gainstagefx::stage_worker::StageWorker;
use gainstagefx::voice::{voice_at, Chain, PipelineUse, Settings, VOICES};

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const BLOCKS: usize = 24;

fn material(k: usize) -> f64 {
    let t = k as f64 / RATE;
    // A decaying pluck every 1024 samples over a low chord: attacks, clipping
    // edges and quiet tails, which is what makes the solvers work.
    let envelope = (-((k % 1024) as f64) / 300.0).exp();
    0.3 * envelope
        * ((std::f64::consts::TAU * 82.4 * t).sin()
            + 0.6 * (std::f64::consts::TAU * 123.5 * t).sin()
            + 0.3 * (std::f64::consts::TAU * 196.0 * t).sin())
}

fn chain(settings: &Settings) -> Chain {
    let mut chain = Chain::new(RATE);
    chain.apply(settings);
    chain.settle();
    chain.find_operating_point();
    chain
}

fn per_sample(settings: &Settings, stereo: bool) -> Vec<(u64, u64)> {
    let mut chain = chain(settings);
    (0..BLOCK * BLOCKS)
        .map(|k| {
            let (l, r) = if stereo {
                chain.process_stereo(material(k))
            } else {
                let l = chain.process(material(k));
                (l, l)
            };
            (l.to_bits(), r.to_bits())
        })
        .collect()
}

fn blocks(
    settings: &Settings,
    stereo: bool,
    worker: Option<&StageWorker>,
) -> (Vec<(u64, u64)>, Vec<PipelineUse>) {
    let mut chain = chain(settings);
    let mut out = Vec::new();
    let mut uses = Vec::new();
    for b in 0..BLOCKS {
        let input: Vec<f64> = (0..BLOCK).map(|i| material(b * BLOCK + i)).collect();
        let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
        uses.push(chain.process_block(&input, &mut left, &mut right, stereo, worker));
        out.extend(
            left.iter()
                .zip(&right)
                .map(|(l, r)| (l.to_bits(), if stereo { r.to_bits() } else { l.to_bits() })),
        );
    }
    (out, uses)
}

#[test]
fn every_voice_pipelined_is_the_serial_chain_to_the_bit() {
    gainstagefx::dsp::time::enable_ftz_daz();
    let live = StageWorker::new();
    let inert = StageWorker::inert();
    for index in 0..VOICES {
        let (gain, diode, amplifier) = voice_at(index);
        for oversampling in [1, 2, 4, 8] {
            let settings = Settings {
                gain,
                diode,
                amplifier,
                drive: 0.8,
                oversampling,
                ..Settings::default()
            };
            let stereo = index % 2 == 0;
            let reference = per_sample(&settings, stereo);
            let (serial, _) = blocks(&settings, stereo, None);
            let (pipelined, _) = blocks(&settings, stereo, Some(&live));
            let (reclaimed, uses) = blocks(&settings, stereo, Some(&inert));
            assert!(uses.iter().all(|u| *u == PipelineUse::Reclaimed));
            for (name, run) in [
                ("serial", &serial),
                ("pipelined", &pipelined),
                ("reclaimed", &reclaimed),
            ] {
                if let Some(k) = run.iter().zip(&reference).position(|(a, b)| a != b) {
                    panic!("{gain:?}/{diode:?}/{amplifier:?} at {oversampling}x, {name} block path differs at sample {k}");
                }
            }
        }
    }
}

/// And the physical speaker/cabinet/microphone path, which the default
/// settings above leave on the legacy filter.
#[test]
fn the_acoustic_path_pipelined_is_the_serial_chain_to_the_bit() {
    gainstagefx::dsp::time::enable_ftz_daz();
    let live = StageWorker::new();
    for preset in gainstagefx::presets::PRESETS.iter().step_by(5) {
        let settings = preset.settings();
        let reference = per_sample(&settings, true);
        let (pipelined, _) = blocks(&settings, true, Some(&live));
        assert!(
            pipelined == reference,
            "{} pipelined differs from the per-sample chain",
            preset.name
        );
    }
}
