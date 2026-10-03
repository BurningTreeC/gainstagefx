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
//! A chain with a pedal can also run its first half as two stages, the pedal
//! and the preamplifier on two cores; that is held to the same standard.
//!
//! Played sample by sample, the chain must agree with them exactly too --
//! unless a block speculated on its power stage's second half (`SpecBlock` in
//! `voice.rs`), which the per-sample path never does. A block that did starts
//! some Newton solves from a different point and converges to the same
//! tolerance, so there the two agree to that tolerance instead.
#[path = "support/allocations.rs"]
mod allocations;

use allocations::assert_no_heap;
use gainstagefx::circuits::wah;
use gainstagefx::presets::PRESETS;
use gainstagefx::stage_worker::StageWorker;
use gainstagefx::voice::{
    voice_at, Chain, DrySource, Gain, Pedal, PedalSettings, PipelineUse, Settings, WahSettings,
    VOICES,
};

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
            // Whether the worker has woken by the time a block's first half is
            // done is up to the scheduler: with the machine busy (a parallel
            // test run) it can miss every block, and the caller reclaims them
            // all. The output must match either way; the hand-over is only
            // exercised when it happens, so try a few times for it.
            let (shared, uses) = (0..5)
                .map(|_| {
                    let (run, uses, _) = blocks(&settings, stereo, Some(&lagging));
                    (run, uses)
                })
                .find(|(_, uses)| uses.contains(&PipelineUse::Shared))
                .unwrap_or_else(|| {
                    panic!("{gain:?} at {oversampling}x: a lagging worker never took a block")
                });
            assert!(
                uses.iter()
                    .all(|u| matches!(u, PipelineUse::Shared | PipelineUse::Reclaimed)),
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

/// A chain with a pedal can run its first half as two stages too: the pedal on
/// the calling thread and the preamplifier on the worker's second lane, with
/// the power stage behind it on the first (`THREE_STAGES_BELOW` in
/// `voice.rs`). Forced to three stages, every pedal -- with a wah ahead of it
/// on every other one, and the DI taken at the pedal, at the preamplifier and
/// at the input in turn -- is the serial block path to the bit, on a live
/// worker, on an inert one (both stages reclaimed) and on a lagging one (the
/// cabinet taken over behind both), across the rates and oversampling
/// factors.
#[test]
fn three_stages_are_the_serial_chain_to_the_bit() {
    gainstagefx::dsp::time::enable_ftz_daz();
    let live = StageWorker::new();
    let inert = StageWorker::inert();
    let lagging = StageWorker::lagging();
    let rates = [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0];
    let sources = [DrySource::Pedal, DrySource::Preamp, DrySource::Input];
    let pedals = Pedal::ALL.iter().filter(|p| **p != Pedal::None);
    for (k, &pedal) in pedals.enumerate() {
        let rate = rates[k % rates.len()];
        let settings = Settings {
            gain: Gain::Twin,
            drive: 0.7,
            pedal: PedalSettings::centred(pedal, 0.8, 0.5),
            wah: WahSettings {
                wah: (k % 2 == 0).then_some(wah::Build::CryBaby),
                ..WahSettings::default()
            },
            dry_source: sources[k % sources.len()],
            oversampling: [1, 2][k % 2],
            ..Settings::default()
        };
        let play = |worker: Option<&StageWorker>, three: bool| {
            let mut chain = Chain::new(rate);
            chain.apply(&settings);
            chain.settle();
            chain.find_operating_point();
            chain.set_speculation(false);
            chain.set_three_stages(Some(three));
            let (mut out, mut uses) = (Vec::new(), Vec::new());
            for b in 0..BLOCKS {
                let input: Vec<f64> = (0..BLOCK)
                    .map(|i| material_at(b * BLOCK + i, rate))
                    .collect();
                let mut dry: Vec<f64> = input.iter().map(|&x| chain.delayed_dry(x)).collect();
                let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
                uses.push(chain.process_block_with_dry(
                    &input, &mut left, &mut right, &mut dry, true, worker, true,
                ));
                out.extend(
                    left.iter()
                        .zip(&right)
                        .zip(&dry)
                        .map(|((l, r), d)| (l.to_bits(), r.to_bits(), d.to_bits())),
                );
            }
            let three_ran = chain.three_stage_blocks();
            (out, uses, three_ran)
        };
        let (serial, _, _) = play(None, false);
        let (live_run, _, ran) = play(Some(&live), true);
        assert_eq!(
            ran, BLOCKS as u64,
            "{pedal:?}: forced, every block runs as three"
        );
        let (reclaimed, uses, _) = play(Some(&inert), true);
        assert!(uses.iter().all(|u| *u == PipelineUse::Reclaimed));
        let (shared, _) = (0..5)
            .map(|_| {
                let (run, uses, _) = play(Some(&lagging), true);
                (run, uses)
            })
            .find(|(_, uses)| uses.contains(&PipelineUse::Shared))
            .unwrap_or_else(|| panic!("{pedal:?}: a lagging worker never took a block"));
        for (name, run) in [
            ("live", &live_run),
            ("reclaimed", &reclaimed),
            ("shared", &shared),
        ] {
            if let Some(i) = run.iter().zip(&serial).position(|(a, b)| a != b) {
                panic!(
                    "{pedal:?} at {rate} Hz, {}x, DI {:?}: three stages {name} differ from serial at sample {i}",
                    settings.oversampling, settings.dry_source
                );
            }
        }
    }
}

/// Three stages allocate nothing on the calling thread, at any rate the
/// plugin runs at: on a live worker, and on an inert one, where the calling
/// thread runs the preamplifier's job and the second half itself.
#[test]
fn three_stages_do_not_allocate() {
    gainstagefx::dsp::time::enable_ftz_daz();
    let live = StageWorker::new();
    let inert = StageWorker::inert();
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let settings = Settings {
            gain: Gain::Twin,
            drive: 0.7,
            pedal: PedalSettings::centred(Pedal::ALL[1], 0.8, 0.5),
            dry_source: DrySource::Preamp,
            ..Settings::default()
        };
        for worker in [&live, &inert] {
            let mut chain = Chain::new(rate);
            chain.apply(&settings);
            chain.settle();
            chain.find_operating_point();
            chain.set_three_stages(Some(true));
            let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
            let (mut input, mut dry) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
            assert_no_heap(|| {
                for b in 0..1024 / BLOCK {
                    for (i, x) in input.iter_mut().enumerate() {
                        *x = material_at(b * BLOCK + i, rate);
                    }
                    dry.copy_from_slice(&input);
                    chain.process_block_with_dry(
                        &input,
                        &mut left,
                        &mut right,
                        &mut dry,
                        false,
                        Some(worker),
                        true,
                    );
                }
            });
            assert_eq!(chain.three_stage_blocks(), (1024 / BLOCK) as u64, "{rate}");
        }
    }
}

/// `Chain::overlapped`, which the plugin adds back to a callback's time for
/// `PipelineGovernor` to judge what it would cost serially, is zero for a block
/// that ran on one thread -- serial, or offered to a worker that never woke --
/// and positive for blocks that really ran on two or three.
#[test]
fn overlapped_is_what_running_apart_saved() {
    gainstagefx::dsp::time::enable_ftz_daz();
    let live = StageWorker::new();
    let inert = StageWorker::inert();
    let settings = Settings {
        gain: Gain::Twin,
        drive: 0.7,
        pedal: PedalSettings::centred(Pedal::ALL[1], 0.8, 0.5),
        ..Settings::default()
    };
    for (worker, pipeline, three) in [
        (None, false, false),
        (Some(&inert), true, false),
        (Some(&inert), true, true),
        (Some(&live), true, false),
        (Some(&live), true, true),
    ] {
        let mut chain = chain(&settings);
        chain.set_three_stages(Some(three));
        let mut overlapped = Vec::new();
        for b in 0..BLOCKS {
            let input: Vec<f64> = (0..BLOCK).map(|i| material(b * BLOCK + i)).collect();
            let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
            chain.process_block(&input, &mut left, &mut right, false, worker, pipeline);
            overlapped.push(chain.overlapped());
        }
        let live_worker = worker.is_some_and(|w| std::ptr::eq(w, &live));
        if live_worker {
            assert!(
                overlapped.iter().any(|&o| o > 0.0),
                "three {three}: a live worker overlapped nothing"
            );
        } else {
            assert!(
                overlapped.iter().all(|&o| o == 0.0),
                "pipeline {pipeline}, three {three}: one thread, yet {overlapped:?}"
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
    // Every fifth preset, the one through a cabinet with a horn, whose
    // stream crosses the hand-over beside the cone's, and the two with a wah
    // ahead of the circuit, one of them moved by its follower.
    for preset in gainstagefx::presets::PRESETS.iter().step_by(5).chain(
        PRESETS
            .iter()
            .filter(|p| p.name == "American 800RB Clank" || p.name.contains("Wah")),
    ) {
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
