//! The pipelined chain is the serial chain, to the bit.
//!
//! `Chain::process_block` can run the chain's second half (power stage, iron,
//! make-up, decimators, tone, cabinet) on a `StageWorker` while the calling
//! thread runs the first half of later samples. It is only worth having if it
//! changes nothing, so every voice is played as a serial block, pipelined on a
//! live worker, pipelined on one that never wakes (so the caller reclaims the
//! second half), and pipelined on one that always lags (so the caller takes
//! the cabinet over part-way through every block), and all four must agree
//! exactly, at every oversampling factor the chain supports.
//!
//! Played sample by sample, the chain must agree with them exactly too --
//! unless a block speculated on its power stage's second half (`SpecBlock` in
//! `voice.rs`), which the per-sample path never does. A block that did starts
//! some Newton solves from a different point and converges to the same
//! tolerance, so there the two agree to that tolerance instead.
#[path = "support/allocations.rs"]
mod allocations;

use allocations::assert_no_heap;
use gainstagefx::presets::PRESETS;
use gainstagefx::stage_worker::StageWorker;
use gainstagefx::voice::{voice_at, Chain, PipelineUse, Settings, VOICES};

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const BLOCKS: usize = 24;

fn material(k: usize) -> f64 {
    material_at(k, RATE)
}

fn material_at(k: usize, rate: f64) -> f64 {
    let t = k as f64 / rate;
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
) -> (Vec<(u64, u64)>, Vec<PipelineUse>, u64) {
    let mut chain = chain(settings);
    let mut out = Vec::new();
    let mut uses = Vec::new();
    for b in 0..BLOCKS {
        let input: Vec<f64> = (0..BLOCK).map(|i| material(b * BLOCK + i)).collect();
        let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
        uses.push(chain.process_block(&input, &mut left, &mut right, stereo, worker, true));
        out.extend(
            left.iter()
                .zip(&right)
                .map(|(l, r)| (l.to_bits(), if stereo { r.to_bits() } else { l.to_bits() })),
        );
    }
    (out, uses, chain.speculated_blocks())
}

/// `run` against the per-sample `reference`: to the bit where no block
/// speculated, and otherwise within the solver's tolerance of it. Returns the
/// largest difference relative to the reference's peak.
fn against_per_sample(run: &[(u64, u64)], reference: &[(u64, u64)], speculated: u64) -> f64 {
    let peak = reference
        .iter()
        .flat_map(|&(l, r)| [f64::from_bits(l).abs(), f64::from_bits(r).abs()])
        .fold(0.0, f64::max);
    let worst = run
        .iter()
        .zip(reference)
        .flat_map(|(&(a, b), &(c, d))| [(a, c), (b, d)])
        .map(|(a, b)| (f64::from_bits(a) - f64::from_bits(b)).abs())
        .fold(0.0, f64::max)
        / peak.max(f64::MIN_POSITIVE);
    if speculated == 0 {
        assert!(
            run == reference,
            "no block speculated, yet the block path differs"
        );
    }
    worst
}

#[test]
fn every_voice_pipelined_is_the_serial_chain_to_the_bit() {
    gainstagefx::dsp::time::enable_ftz_daz();
    let live = StageWorker::new();
    let inert = StageWorker::inert();
    let lagging = StageWorker::lagging();
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
            let (serial, _, speculated) = blocks(&settings, stereo, None);
            let (pipelined, _, _) = blocks(&settings, stereo, Some(&live));
            let (reclaimed, uses, _) = blocks(&settings, stereo, Some(&inert));
            assert!(uses.iter().all(|u| *u == PipelineUse::Reclaimed));
            // A lagging worker hands the cabinet over whenever it has claimed
            // the block; when it has not woken in time the caller reclaims.
            let (shared, uses, _) = blocks(&settings, stereo, Some(&lagging));
            assert!(
                uses.iter()
                    .all(|u| matches!(u, PipelineUse::Shared | PipelineUse::Reclaimed))
                    && uses.contains(&PipelineUse::Shared),
                "a lagging worker should hand the cabinet over: {uses:?}"
            );
            for (name, run) in [
                ("pipelined", &pipelined),
                ("reclaimed", &reclaimed),
                ("shared", &shared),
            ] {
                if let Some(k) = run.iter().zip(&serial).position(|(a, b)| a != b) {
                    panic!("{gain:?}/{diode:?}/{amplifier:?} at {oversampling}x, {name} block path differs from the serial one at sample {k}");
                }
            }
            let worst = against_per_sample(&serial, &reference, speculated);
            println!(
                "{gain:?}/{diode:?}/{amplifier:?} at {oversampling}x: {speculated} blocks speculated, {:.1} dB from per-sample",
                20.0 * worst.max(1e-300).log10()
            );
            assert!(
                worst < 1e-6,
                "{gain:?}/{diode:?}/{amplifier:?} at {oversampling}x: {speculated} speculated blocks moved the output {:.1} dB",
                20.0 * worst.log10()
            );
        }
    }
}

/// And the physical speaker/cabinet/microphone path, which the default
/// settings above leave on the legacy filter.
#[test]
fn the_acoustic_path_pipelined_is_the_serial_chain_to_the_bit() {
    gainstagefx::dsp::time::enable_ftz_daz();
    let live = StageWorker::new();
    let lagging = StageWorker::lagging();
    for preset in gainstagefx::presets::PRESETS.iter().step_by(5) {
        let settings = preset.settings();
        let reference = per_sample(&settings, true);
        let (serial, _, speculated) = blocks(&settings, true, None);
        let (pipelined, _, _) = blocks(&settings, true, Some(&live));
        assert!(
            pipelined == serial,
            "{} pipelined differs from the serial block path",
            preset.name
        );
        let (shared, _, _) = blocks(&settings, true, Some(&lagging));
        assert!(
            shared == serial,
            "{} with the cabinet handed over differs from the serial block path",
            preset.name
        );
        let worst = against_per_sample(&serial, &reference, speculated);
        println!(
            "{}: {speculated} blocks speculated, {:.1} dB from per-sample",
            preset.name,
            20.0 * worst.max(1e-300).log10()
        );
        assert!(
            worst < 1e-6,
            "{}: {speculated} speculated blocks moved the output {:.1} dB",
            preset.name,
            20.0 * worst.log10()
        );
    }
}

/// The presets whose power stages the speculation is for.
const HARD: [&str; 2] = ["Jazz Chorus", "Brown '84"];

/// `preset` set up at `rate`, and the scale its input takes: its own input
/// trim and twelve decibels more, so the plucks drive the power stage hard.
fn hard_chain(name: &str, rate: f64) -> (Chain, f64) {
    let preset = PRESETS.iter().find(|p| p.name == name).expect("preset");
    let mut chain = Chain::new(rate);
    chain.apply(&preset.settings());
    chain.settle();
    chain.find_operating_point();
    (chain, 10f64.powf((preset.input_trim as f64 + 12.0) / 20.0))
}

fn hard_blocks(
    name: &str,
    rate: f64,
    blocks: usize,
    worker: Option<&StageWorker>,
    pipeline: bool,
) -> (Vec<(u64, u64)>, u64) {
    let (mut chain, scale) = hard_chain(name, rate);
    let mut out = Vec::new();
    let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
    for b in 0..blocks {
        let input: Vec<f64> = (0..BLOCK)
            .map(|i| scale * material_at(b * BLOCK + i, rate))
            .collect();
        chain.process_block(&input, &mut left, &mut right, true, worker, pipeline);
        out.extend(
            left.iter()
                .zip(&right)
                .map(|(l, r)| (l.to_bits(), r.to_bits())),
        );
    }
    (out, chain.speculated_blocks())
}

/// Where the speculation fires, it must fire the same way and give the same
/// answers on every block path, at every rate the plugin runs at: serial,
/// serial with a worker taking the shadow (live, never waking, or claiming
/// late), pipelined, reclaimed, and with the cabinet handed over.
#[test]
fn speculation_is_the_same_on_every_block_path_at_every_rate() {
    gainstagefx::dsp::time::enable_ftz_daz();
    let live = StageWorker::new();
    let inert = StageWorker::inert();
    let lagging = StageWorker::lagging();
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        for name in HARD {
            // A pluck every 1024 samples; eight of them.
            let blocks = 8 * 1024 / BLOCK;
            let (serial, speculated) = hard_blocks(name, rate, blocks, None, false);
            assert!(speculated > 0, "{name} at {rate} Hz never speculated");
            for (path, worker, pipeline) in [
                ("helped", &live, false),
                ("helper never woke", &inert, false),
                ("helper lagged", &lagging, false),
                ("pipelined", &live, true),
                ("reclaimed", &inert, true),
                ("shared", &lagging, true),
            ] {
                let (run, count) = hard_blocks(name, rate, blocks, Some(worker), pipeline);
                assert_eq!(
                    count, speculated,
                    "{name} at {rate} Hz, {path}: a different set of blocks speculated"
                );
                if let Some(k) = run.iter().zip(&serial).position(|(a, b)| a != b) {
                    panic!("{name} at {rate} Hz, {path} block path differs from the serial one at sample {k}");
                }
            }
            let (mut chain, scale) = hard_chain(name, rate);
            let reference: Vec<(u64, u64)> = (0..blocks * BLOCK)
                .map(|k| {
                    let (l, r) = chain.process_stereo(scale * material_at(k, rate));
                    (l.to_bits(), r.to_bits())
                })
                .collect();
            let worst = against_per_sample(&serial, &reference, speculated);
            println!(
                "{name} at {rate} Hz: {speculated} of {blocks} blocks speculated, {:.1} dB from per-sample",
                20.0 * worst.max(1e-300).log10()
            );
            assert!(
                worst < 1e-6,
                "{name} at {rate} Hz: speculation moved the output {:.1} dB",
                20.0 * worst.log10()
            );
        }
    }
}

/// The shadow, its follow and the hand-over of its answers allocate nothing,
/// on the serial path, with or without a worker to take the shadow, and on
/// the calling thread's side of the pipelined ones.
#[test]
fn speculation_does_not_allocate() {
    gainstagefx::dsp::time::enable_ftz_daz();
    let live = StageWorker::new();
    let inert = StageWorker::inert();
    for (worker, pipeline) in [
        (None, false),
        (Some(&live), false),
        (Some(&live), true),
        (Some(&inert), true),
    ] {
        let (mut chain, scale) = hard_chain("Jazz Chorus", RATE);
        let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
        let mut input = vec![0.0; BLOCK];
        let before = chain.speculated_blocks();
        assert_no_heap(|| {
            for b in 0..4 * 1024 / BLOCK {
                for (i, x) in input.iter_mut().enumerate() {
                    *x = scale * material(b * BLOCK + i);
                }
                chain.process_block(&input, &mut left, &mut right, true, worker, pipeline);
            }
        });
        assert!(chain.speculated_blocks() > before, "nothing speculated");
    }
}
