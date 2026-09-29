//! Stage-by-stage 48 kHz / 64-sample realtime cost benchmark.
//!
//! Purpose:
//!   Find where a complete GainStageFx callback spends its 1.333333 ms budget
//!   without instrumenting the inner per-sample DSP with timers.
//!
//! Run:
//!   cargo run --release --example stage_deadline
//!
//! For cleaner numbers:
//!   taskset -c 2 cargo run --release --example stage_deadline
//!
//! The rows are deliberately cumulative. Differences between adjacent rows are
//! useful estimates of the incremental cost of power, speaker loading, cabinet
//! radiation, microphone modelling, and the second microphone. They are not
//! cycle-perfect attribution: changing a load can also change nonlinear solver
//! behaviour, which is exactly why this benchmark prints solver health too.

use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::mic::{MicPlacement, MicProfile};
use gainstagefx::acoustics::stage::MicSlot;
use gainstagefx::voice::{
    AcousticSettings, Cabinet, CabinetChoice, Chain, Gain, PowerAmp, Settings, SpeakerChoice, Tone,
    NOMINAL_DBFS,
};
use std::hint::black_box;
use std::time::Instant;

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const CALLBACK_US: f64 = BLOCK as f64 / RATE * 1_000_000.0;
const WARMUP_BLOCKS: usize = 256;
const MEASURE_BLOCKS: usize = 4096;

#[derive(Clone, Copy)]
struct ResultRow {
    label: &'static str,
    mean: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    max: f64,
    misses: usize,
    over_1ms: usize,
    checksum: f64,
    power_passes: f64,
    power_unsettled: u64,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[i]
}

#[inline]
fn input(sample: usize) -> f64 {
    let t = sample as f64 / RATE;
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    amplitude
        * (0.65 * (std::f64::consts::TAU * 82.41 * t).sin()
            + 0.25 * (std::f64::consts::TAU * 329.63 * t).sin()
            + 0.10 * (std::f64::consts::TAU * 1318.51 * t).sin())
}

fn base_settings() -> Settings {
    Settings {
        gain: Gain::Twin,
        power_amp: PowerAmp::Matched,
        tone: Tone::Off,
        cabinet: Cabinet::Off,
        drive: 0.6,
        // Match the earlier acoustic_deadline benchmark.
        oversampling: 2,
        ..Settings::default()
    }
}

fn physical_cabinet(mic_a: MicSlot, mic_b: MicSlot) -> AcousticSettings {
    AcousticSettings {
        cabinet: CabinetChoice::Model(&CabinetProfile::BRIT_V30),
        speaker: SpeakerChoice::Matched,
        mic_a,
        place_a: MicPlacement {
            position: 0.20,
            distance: 0.025,
            angle: 0.0,
        },
        mic_b,
        place_b: MicPlacement {
            position: 0.55,
            distance: 0.30,
            angle: 20.0,
        },
        blend: 0.5,
        ..AcousticSettings::default()
    }
}

fn infinite_baffle(mic: MicSlot) -> AcousticSettings {
    AcousticSettings {
        cabinet: CabinetChoice::Bypass,
        speaker: SpeakerChoice::Matched,
        mic_a: mic,
        place_a: MicPlacement {
            position: 0.20,
            distance: 0.025,
            angle: 0.0,
        },
        mic_b: MicSlot::Off,
        ..AcousticSettings::default()
    }
}

fn resistor_di() -> AcousticSettings {
    AcousticSettings {
        // SpeakerChoice::Bypass keeps the power stage on its resistor load and
        // avoids the physical radiation path. CabinetChoice is irrelevant in
        // that case, but Bypass makes the intent explicit.
        cabinet: CabinetChoice::Bypass,
        speaker: SpeakerChoice::Bypass,
        mic_a: MicSlot::Ideal,
        mic_b: MicSlot::Off,
        ..AcousticSettings::default()
    }
}

fn measure(label: &'static str, settings: Settings) -> ResultRow {
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

    // Snapshot solver counters after warmup so reported health describes only
    // the measured interval.
    let before = chain.solver_breakdown();

    let mut times = Vec::with_capacity(MEASURE_BLOCKS);
    for _ in 0..MEASURE_BLOCKS {
        let start = Instant::now();
        for _ in 0..BLOCK {
            checksum += chain.process(input(sample));
            sample += 1;
        }
        times.push(start.elapsed().as_secs_f64() * 1_000_000.0);
    }

    let health = chain.solver_breakdown().saturating_delta(before);
    black_box(checksum);

    times.sort_by(|a, b| a.total_cmp(b));
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let p50 = percentile(&times, 0.50);
    let p95 = percentile(&times, 0.95);
    let p99 = percentile(&times, 0.99);
    let max = *times.last().unwrap();
    let misses = times.iter().filter(|&&x| x > CALLBACK_US).count();
    let over_1ms = times.iter().filter(|&&x| x > 1000.0).count();

    ResultRow {
        label,
        mean,
        p50,
        p95,
        p99,
        max,
        misses,
        over_1ms,
        checksum,
        power_passes: health.power.passes_per_solve(),
        power_unsettled: health.power.unsettled,
    }
}

fn print_row(row: ResultRow, previous: Option<ResultRow>) {
    let delta_mean = previous.map(|p| row.mean - p.mean);
    let delta_p99 = previous.map(|p| row.p99 - p.p99);

    println!("\n{}", row.label);
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
    if let (Some(dm), Some(dp)) = (delta_mean, delta_p99) {
        println!("  vs previous     : {dm:+9.3} us mean, {dp:+9.3} us p99");
    }
    println!(
        "  solver power    : {:.2} passes/solve, {} unsettled",
        row.power_passes, row.power_unsettled
    );
    println!("  checksum        : {:.6e}", row.checksum);
}

fn main() {
    println!("GainStageFx cumulative stage deadline benchmark");
    println!(
        "48 kHz / 64 samples = {CALLBACK_US:.3} us ({:.6} ms) per callback.",
        CALLBACK_US / 1000.0
    );
    println!("Warmup: {WARMUP_BLOCKS} blocks; measured: {MEASURE_BLOCKS} blocks per row.");
    println!("Rows are cumulative; 'vs previous' is an approximate incremental cost.");
    println!("Do not compare checksums between rows: the signal path intentionally changes.\n");

    let base = base_settings();

    // 1. Twin preamp, but no valve power stage and no physical speaker path.
    let preamp_only = Settings {
        power_amp: PowerAmp::Bypass,
        acoustic: resistor_di(),
        ..base
    };

    // 2. Add the matched Twin power amplifier, still into the non-radiating
    // resistor/DI path. Difference from row 1 estimates the power-stage cost.
    let power_resistor = Settings {
        power_amp: PowerAmp::Matched,
        acoustic: resistor_di(),
        ..base
    };

    // 3. Replace the resistor load with the physical loudspeaker, on an
    // infinite baffle, using an ideal microphone. This includes the
    // speaker-loaded power solve plus basic propagation/radiation.
    let speaker_ideal = Settings {
        acoustic: infinite_baffle(MicSlot::Ideal),
        ..base
    };

    // 4. Same speaker path, but add the Dynamic 57 microphone model.
    let speaker_mic = Settings {
        acoustic: infinite_baffle(MicSlot::Profile(&MicProfile::DYNAMIC_57)),
        ..base
    };

    // 5. Add the physical 4x12 cabinet/reflection model, one microphone.
    let cabinet_one_mic = Settings {
        acoustic: physical_cabinet(MicSlot::Profile(&MicProfile::DYNAMIC_57), MicSlot::Off),
        ..base
    };

    // 6. Add the second Ribbon 121 at 30 cm.
    let cabinet_two_mics = Settings {
        acoustic: physical_cabinet(
            MicSlot::Profile(&MicProfile::DYNAMIC_57),
            MicSlot::Profile(&MicProfile::RIBBON_121),
        ),
        ..base
    };

    let cases = [
        (
            "1. Twin preamp only / power bypass / resistor DI",
            preamp_only,
        ),
        (
            "2. + matched Twin power stage / resistor DI",
            power_resistor,
        ),
        (
            "3. + physical speaker / infinite baffle / ideal mic",
            speaker_ideal,
        ),
        ("4. + Dynamic 57 microphone model", speaker_mic),
        ("5. + physical Brit V30 4x12 / one mic", cabinet_one_mic),
        ("6. + second Ribbon 121 microphone", cabinet_two_mics),
    ];

    let mut previous = None;
    let mut rows = Vec::with_capacity(cases.len());
    for (label, settings) in cases {
        let row = measure(label, settings);
        print_row(row, previous);
        previous = Some(row);
        rows.push(row);
    }

    println!("\n================ SUMMARY ================");
    println!(
        "{:<52} {:>9} {:>9} {:>9} {:>7}",
        "path", "mean us", "p99 us", "max us", "misses"
    );
    for row in &rows {
        println!(
            "{:<52} {:>9.1} {:>9.1} {:>9.1} {:>7}",
            row.label, row.mean, row.p99, row.max, row.misses
        );
    }

    println!("\nApproximate incremental mean costs:");
    for pair in rows.windows(2) {
        println!(
            "  {} -> {} : {:+.1} us",
            pair[0].label,
            pair[1].label,
            pair[1].mean - pair[0].mean
        );
    }

    println!("\nInterpretation target:");
    println!("  hard deadline : {CALLBACK_US:.1} us");
    println!("  desired p99   : < 800-900 us");
    println!("  desired misses: 0");
    println!("  Large positive adjacent-row deltas identify the next optimization target.");
}
