//! Short real-time deadline benchmark for the physical cabinet/speaker/microphone path.
//!
//! Run with:
//!   cargo run --release --example acoustic_deadline
//!
//! This deliberately measures 64-sample blocks at 48 kHz. The host callback
//! period is therefore 1.333333 ms. It reports median/p95/p99/max block time,
//! deadline misses, and headroom. This is a CPU/deadline benchmark, not an
//! algorithmic-latency measurement.

use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::mic::{MicPlacement, MicProfile};
use gainstagefx::acoustics::speaker::SpeakerProfile;
use gainstagefx::acoustics::stage::{AcousticStage, MicSlot};
use gainstagefx::voice::{
    AcousticSettings, CabinetChoice, Chain, Gain, PowerAmp, Settings, SpeakerChoice, Tone,
    NOMINAL_DBFS,
};
use std::hint::black_box;
use std::time::Instant;

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const CALLBACK_US: f64 = BLOCK as f64 / RATE * 1_000_000.0;
const WARMUP_BLOCKS: usize = 256;
const MEASURE_BLOCKS: usize = 4_096; // ~5.46 s of audio

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

fn report(label: &str, mut times_us: Vec<f64>, checksum: f64) {
    times_us.sort_by(|a, b| a.total_cmp(b));
    let mean = times_us.iter().sum::<f64>() / times_us.len() as f64;
    let p50 = percentile(&times_us, 0.50);
    let p95 = percentile(&times_us, 0.95);
    let p99 = percentile(&times_us, 0.99);
    let max = *times_us.last().unwrap();
    let misses = times_us.iter().filter(|&&x| x > CALLBACK_US).count();
    let over_1ms = times_us.iter().filter(|&&x| x > 1_000.0).count();

    println!("\n{label}");
    println!("  callback budget : {CALLBACK_US:9.3} us");
    println!(
        "  mean            : {mean:9.3} us  ({:5.1}% budget)",
        mean / CALLBACK_US * 100.0
    );
    println!("  p50             : {p50:9.3} us");
    println!("  p95             : {p95:9.3} us");
    println!(
        "  p99             : {p99:9.3} us  ({:5.1}% budget)",
        p99 / CALLBACK_US * 100.0
    );
    println!(
        "  max             : {max:9.3} us  ({:5.1}% budget)",
        max / CALLBACK_US * 100.0
    );
    println!("  > 1.000 ms      : {over_1ms:9} / {}", times_us.len());
    println!("  deadline misses : {misses:9} / {}", times_us.len());
    println!("  p99 headroom    : {:+9.3} us", CALLBACK_US - p99);
    println!("  max headroom    : {:+9.3} us", CALLBACK_US - max);
    println!("  checksum        : {checksum:.6e}");
}

fn input(sample: usize) -> f64 {
    let t = sample as f64 / RATE;
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    amplitude
        * (0.65 * (std::f64::consts::TAU * 82.41 * t).sin()
            + 0.25 * (std::f64::consts::TAU * 329.63 * t).sin()
            + 0.10 * (std::f64::consts::TAU * 1318.51 * t).sin())
}

fn bench_stage() {
    // Deliberately exercise two microphones and a physical 4x12. This isolates
    // the new acoustic stage from the nonlinear amplifier solver.
    let mut stage = AcousticStage::new(RATE);
    stage.configure(
        Some(&CabinetProfile::BRIT_V30),
        &SpeakerProfile::BRIT_V30,
        MicSlot::Profile(&MicProfile::DYNAMIC_57),
        MicSlot::Profile(&MicProfile::RIBBON_121),
    );
    stage.set_placement(
        MicPlacement {
            position: 0.20,
            distance: 0.025,
            angle: 0.0,
        },
        MicPlacement {
            position: 0.55,
            distance: 0.30,
            angle: 20.0,
        },
        0.5,
        0.0,
        0.0,
        false,
        false,
    );

    let mut sample = 0usize;
    let mut checksum = 0.0;
    for _ in 0..WARMUP_BLOCKS {
        for _ in 0..BLOCK {
            checksum += stage.process(input(sample));
            sample += 1;
        }
    }

    let mut times = Vec::with_capacity(MEASURE_BLOCKS);
    for _ in 0..MEASURE_BLOCKS {
        let start = Instant::now();
        for _ in 0..BLOCK {
            checksum += stage.process(input(sample));
            sample += 1;
        }
        times.push(start.elapsed().as_secs_f64() * 1_000_000.0);
    }
    black_box(checksum);
    report(
        "AcousticStage only: physical 4x12 + two microphones",
        times,
        checksum,
    );
}

fn bench_chain() {
    // Full amplifier path plus the same physical acoustic configuration. This
    // answers the real question: can one complete 64-sample callback finish
    // inside the 1.333 ms host period?
    let acoustic = AcousticSettings {
        cabinet: CabinetChoice::Model(&CabinetProfile::BRIT_V30),
        speaker: SpeakerChoice::Matched,
        mic_a: MicSlot::Profile(&MicProfile::DYNAMIC_57),
        place_a: MicPlacement {
            position: 0.20,
            distance: 0.025,
            angle: 0.0,
        },
        mic_b: MicSlot::Profile(&MicProfile::RIBBON_121),
        place_b: MicPlacement {
            position: 0.55,
            distance: 0.30,
            angle: 20.0,
        },
        blend: 0.5,
        ..AcousticSettings::default()
    };
    let settings = Settings {
        gain: Gain::Twin,
        power_amp: PowerAmp::Matched,
        tone: Tone::Off,
        drive: 0.6,
        acoustic,
        ..Settings::default()
    };

    let mut chain = Chain::new(RATE);
    chain.apply(&settings);
    chain.settle();
    chain.find_operating_point();

    let mut sample = 0usize;
    let mut checksum = 0.0;
    for _ in 0..WARMUP_BLOCKS {
        for _ in 0..BLOCK {
            checksum += chain.process(input(sample));
            sample += 1;
        }
    }

    let mut times = Vec::with_capacity(MEASURE_BLOCKS);
    for _ in 0..MEASURE_BLOCKS {
        let start = Instant::now();
        for _ in 0..BLOCK {
            checksum += chain.process(input(sample));
            sample += 1;
        }
        times.push(start.elapsed().as_secs_f64() * 1_000_000.0);
    }
    black_box(checksum);
    report(
        "Full Chain: Twin + loaded speaker + physical 4x12 + two microphones",
        times,
        checksum,
    );

    let health = chain.solver_breakdown();
    println!(
        "  solver power    : {:.2} passes/solve, {} unsettled",
        health.power.passes_per_solve(),
        health.power.unsettled
    );
}

fn main() {
    println!("GainStageFx 48 kHz / 64-sample acoustic deadline benchmark");
    println!(
        "One callback every {CALLBACK_US:.3} us ({:.6} ms).",
        CALLBACK_US / 1000.0
    );
    println!("Warmup: {WARMUP_BLOCKS} blocks; measured: {MEASURE_BLOCKS} blocks.");
    println!("Run the release build on an otherwise idle machine for useful numbers.");

    bench_stage();
    bench_chain();
}
