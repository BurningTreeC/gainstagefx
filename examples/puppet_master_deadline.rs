//! Realtime callback benchmark for the shipped "Puppet Master '86" preset.
//!
//! This loads the preset exactly as shipped, then changes ONLY the requested
//! oversampling factor. Pedal, amplifier, power stage, speaker, cabinet,
//! microphones, placements, input calibration, etc. remain the preset's values.
//!
//! Run:
//!   cargo run --release --example puppet_master_deadline
//!
//! Optional second run pinned to one CPU:
//!   taskset -c 2 cargo run --release --example puppet_master_deadline
//!
//! Important: modelled circuits may internally cap the active oversampling
//! factor for realtime safety. This benchmark deliberately uses the public
//! Settings path, so it measures what GainStageFx actually does when the user
//! requests Off/2x/4x/8x rather than bypassing those safeguards.

use gainstagefx::params::Oversampling;
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Chain, NOMINAL_DBFS};
use std::hint::black_box;
use std::time::Instant;

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const CALLBACK_US: f64 = BLOCK as f64 / RATE * 1_000_000.0;
const WARMUP_BLOCKS: usize = 256;
const MEASURE_BLOCKS: usize = 4096;
const PRESET_NAME: &str = "Puppet Master '86";

#[derive(Clone, Copy)]
struct Row {
    name: &'static str,
    requested_factor: usize,
    mean: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    max: f64,
    over_1ms: usize,
    misses: usize,
    checksum: f64,
    pedal_passes: f64,
    pedal_unsettled: u64,
    gain_passes: f64,
    gain_unsettled: u64,
    power_passes: f64,
    power_unsettled: u64,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

/// Broadband low-guitar stimulus, with the preset's own input trim applied.
/// This mirrors the spirit of presetlevel rather than benchmarking silence.
#[inline]
fn input(sample: usize, amplitude: f64) -> f64 {
    let t = sample as f64 / RATE;
    amplitude
        * (0.50 * (std::f64::consts::TAU * 82.4 * t).sin()
            + 0.30 * (std::f64::consts::TAU * 123.5 * t).sin()
            + 0.20 * (std::f64::consts::TAU * 246.9 * t).sin())
}

fn measure(os: Oversampling) -> Row {
    let preset = PRESETS
        .iter()
        .find(|p| p.name == PRESET_NAME)
        .unwrap_or_else(|| panic!("shipped preset {PRESET_NAME:?} not found"));

    // Load the shipped preset exactly, then vary only oversampling.
    let mut settings = preset.settings();
    settings.oversampling = os.factor();

    let amplitude = 10f64.powf((NOMINAL_DBFS + preset.input_trim as f64) / 20.0);

    let mut chain = Chain::new(RATE);
    chain.apply(&settings);
    chain.settle();
    chain.find_operating_point();

    let mut sample = 0usize;
    let mut checksum = 0.0;

    for _ in 0..WARMUP_BLOCKS {
        for _ in 0..BLOCK {
            checksum += chain.process(input(sample, amplitude));
            sample += 1;
        }
    }

    let before = chain.solver_breakdown();
    let mut times = Vec::with_capacity(MEASURE_BLOCKS);

    for _ in 0..MEASURE_BLOCKS {
        let start = Instant::now();
        for _ in 0..BLOCK {
            checksum += chain.process(input(sample, amplitude));
            sample += 1;
        }
        times.push(start.elapsed().as_secs_f64() * 1_000_000.0);
    }

    let health = chain.solver_breakdown().saturating_delta(before);
    black_box(checksum);

    times.sort_by(|a, b| a.total_cmp(b));

    Row {
        name: os.name(),
        requested_factor: os.factor(),
        mean: times.iter().sum::<f64>() / times.len() as f64,
        p50: percentile(&times, 0.50),
        p95: percentile(&times, 0.95),
        p99: percentile(&times, 0.99),
        max: *times.last().unwrap(),
        over_1ms: times.iter().filter(|&&x| x > 1000.0).count(),
        misses: times.iter().filter(|&&x| x > CALLBACK_US).count(),
        checksum,
        pedal_passes: health.pedal.passes_per_solve(),
        pedal_unsettled: health.pedal.unsettled,
        gain_passes: health.gain.passes_per_solve(),
        gain_unsettled: health.gain.unsettled,
        power_passes: health.power.passes_per_solve(),
        power_unsettled: health.power.unsettled,
    }
}

fn print_row(row: Row, baseline: Option<Row>) {
    println!(
        "\nRequested oversampling: {} ({}x)",
        row.name, row.requested_factor
    );
    println!(
        "  mean            : {:9.3} us  ({:5.1}% budget)",
        row.mean,
        row.mean / CALLBACK_US * 100.0
    );
    println!("  p50             : {:9.3} us", row.p50);
    println!("  p95             : {:9.3} us", row.p95);
    println!(
        "  p99             : {:9.3} us  ({:5.1}% budget)",
        row.p99,
        row.p99 / CALLBACK_US * 100.0
    );
    println!(
        "  max             : {:9.3} us  ({:5.1}% budget)",
        row.max,
        row.max / CALLBACK_US * 100.0
    );
    println!(
        "  > 1.000 ms      : {:9} / {}",
        row.over_1ms, MEASURE_BLOCKS
    );
    println!("  deadline misses : {:9} / {}", row.misses, MEASURE_BLOCKS);
    println!("  p99 headroom    : {:+9.3} us", CALLBACK_US - row.p99);
    println!("  max headroom    : {:+9.3} us", CALLBACK_US - row.max);

    if let Some(off) = baseline {
        println!(
            "  vs OS Off       : {:+9.3} us mean ({:+5.1}%)",
            row.mean - off.mean,
            (row.mean / off.mean - 1.0) * 100.0
        );
    }

    println!(
        "  solver pedal    : {:.2} passes/solve, {} unsettled",
        row.pedal_passes, row.pedal_unsettled
    );
    println!(
        "  solver gain     : {:.2} passes/solve, {} unsettled",
        row.gain_passes, row.gain_unsettled
    );
    println!(
        "  solver power    : {:.2} passes/solve, {} unsettled",
        row.power_passes, row.power_unsettled
    );
    println!("  checksum        : {:.6e}", row.checksum);
}

fn main() {
    let preset = PRESETS
        .iter()
        .find(|p| p.name == PRESET_NAME)
        .unwrap_or_else(|| panic!("shipped preset {PRESET_NAME:?} not found"));

    println!("GainStageFx {PRESET_NAME:?} oversampling deadline benchmark");
    println!(
        "48 kHz / 64 samples = {CALLBACK_US:.3} us ({:.6} ms) per callback.",
        CALLBACK_US / 1000.0
    );
    println!("Warmup: {WARMUP_BLOCKS} blocks; measured: {MEASURE_BLOCKS} blocks per setting.");
    println!(
        "Shipped preset requests {} oversampling; benchmark varies only that setting.",
        preset.oversampling.name()
    );
    println!(
        "Input trim: {:+.1} dB; output trim is irrelevant to measured Chain::process cost.",
        preset.input_trim
    );
    println!("Run the release build on an otherwise idle machine for useful numbers.");

    let mut rows = Vec::new();
    let mut baseline = None;

    for os in Oversampling::ALL {
        let row = measure(os);
        print_row(row, baseline);
        if baseline.is_none() {
            baseline = Some(row);
        }
        rows.push(row);
    }

    println!("\n================ SUMMARY ================");
    println!(
        "{:>9} {:>10} {:>10} {:>10} {:>9} {:>9}",
        "request", "mean us", "p99 us", "max us", ">1ms", "misses"
    );
    for row in &rows {
        println!(
            "{:>9} {:>10.1} {:>10.1} {:>10.1} {:>9} {:>9}",
            row.name, row.mean, row.p99, row.max, row.over_1ms, row.misses
        );
    }

    let off = rows[0];
    println!("\nMean cost relative to OS Off:");
    for row in &rows {
        println!(
            "  {:>3}: {:+8.1} us, {:6.2}x total cost",
            row.name,
            row.mean - off.mean,
            row.mean / off.mean
        );
    }

    println!("\nInterpretation:");
    println!("  hard callback deadline : {CALLBACK_US:.1} us");
    println!("  desired p99            : < 800-900 us");
    println!("  desired misses         : 0");
    println!("  Compare mean/p50 first; isolated max spikes can be OS scheduling jitter.");
    println!("  If 4x/8x resemble a lower row, the modelled-circuit oversampling cap is active.");
}
