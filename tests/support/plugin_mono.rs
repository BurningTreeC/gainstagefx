//! Actual Plugin::process regressions, included inside plugin.rs for private
//! access to negotiated mono/stereo layout state and solver-health counters.
use super::*;
use crate::params::{Cabinet, Circuit, Iron, ToneStack};

#[path = "allocations.rs"]
mod allocations;
use allocations::assert_no_heap;

#[path = "attacks.rs"]
mod attacks;

#[cfg(target_os = "linux")]
#[repr(C)]
struct ThreadTimespec {
    tv_sec: std::os::raw::c_long,
    tv_nsec: std::os::raw::c_long,
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn clock_gettime(clock_id: std::os::raw::c_int, tp: *mut ThreadTimespec) -> std::os::raw::c_int;
}

#[cfg(target_os = "linux")]
fn thread_cpu_seconds() -> f64 {
    // Linux CLOCK_THREAD_CPUTIME_ID. This measures CPU actually consumed by
    // this thread, excluding time spent preempted or asleep.
    const CLOCK_THREAD_CPUTIME_ID: std::os::raw::c_int = 3;
    let mut ts = ThreadTimespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is valid writable storage for `clock_gettime()`, and the
    // clock id is the Linux per-thread CPU clock documented by clock_gettime(2).
    let result = unsafe { clock_gettime(CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(result, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID) failed");
    ts.tv_sec as f64 + ts.tv_nsec as f64 * 1e-9
}

#[cfg(not(target_os = "linux"))]
fn thread_cpu_seconds() -> f64 {
    // The realtime recording probe is a Linux/JACK diagnostic. Keep other
    // targets buildable; on them the CPU metric falls back to wall time in
    // `process_profiled()`.
    f64::NAN
}

#[derive(Clone, Copy, Debug)]
struct CallbackTiming {
    wall_seconds: f64,
    cpu_seconds: f64,
}

struct Host;
impl InitContext<GainStageFx> for Host {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn execute(&self, _: ()) {}
    fn set_latency_samples(&self, samples: u32) {
        assert_eq!(samples, LATENCY);
    }
    fn set_current_voice_capacity(&self, _: u32) {
        unreachable!()
    }
}
impl ProcessContext<GainStageFx> for Host {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn execute_background(&self, _: ()) {
        unreachable!()
    }
    fn execute_gui(&self, _: ()) {
        unreachable!()
    }
    fn transport(&self) -> &Transport {
        unreachable!()
    }
    fn next_event(&mut self) -> Option<PluginNoteEvent<GainStageFx>> {
        None
    }
    fn send_event(&mut self, _: PluginNoteEvent<GainStageFx>) {
        unreachable!()
    }
    fn set_latency_samples(&self, _: u32) {
        unreachable!()
    }
    fn set_current_voice_capacity(&self, _: u32) {
        unreachable!()
    }
}

fn initialized(circuit: Circuit, mono: bool, rate: f32) -> GainStageFx {
    let mut plugin = GainStageFx::default();
    let p = Arc::get_mut(&mut plugin.params).unwrap();
    // NIH-plug's host wrapper normally seeds these from parameter values.
    // Direct callback tests bypass that wrapper; an unseeded Master smoother
    // otherwise drives the power stage with its pot shut instead of at 0.5.
    for param in [
        &p.drive,
        &p.master,
        &p.bass,
        &p.mid,
        &p.treble,
        &p.eq60,
        &p.eq240,
        &p.eq750,
        &p.eq2200,
        &p.eq6600,
        &p.reverb,
        &p.speed,
        &p.intensity,
        &p.input_trim,
        &p.output_trim,
        &p.mix,
    ] {
        param.smoothed.reset(param.value());
    }
    p.circuit = EnumParam::new("Circuit", circuit);
    p.iron = EnumParam::new("Iron", Iron::Steel);
    p.tone = EnumParam::new("Tone", ToneStack::Scooping);
    p.cabinet = EnumParam::new("Cabinet", Cabinet::Stack);
    p.drive.smoothed.reset(0.8);
    p.reverb.smoothed.reset(0.7);
    p.intensity.smoothed.reset(0.9);
    p.speed.smoothed.reset(0.7);
    p.mix.smoothed.reset(0.65);
    assert!(plugin.initialize(
        &GainStageFx::AUDIO_IO_LAYOUTS[usize::from(mono)],
        &BufferConfig {
            sample_rate: rate,
            min_buffer_size: Some(1),
            max_buffer_size: 1024,
            process_mode: ProcessMode::Realtime,
        },
        &mut Host,
    ));
    plugin.reset();
    plugin
}

fn process(plugin: &mut GainStageFx, left: &mut [f32], right: Option<&mut [f32]>) -> f64 {
    let mut buffer = Buffer::default();
    // Both slices remain live, disjoint, and equally sized for this entire
    // call. Buffer's reference-vector allocation is host setup, outside the
    // measured and allocation-checked plugin callback.
    unsafe {
        buffer.set_slices(left.len(), |slices| {
            slices.push(left);
            if let Some(right) = right {
                slices.push(right);
            }
        });
    }
    let mut aux = AuxiliaryBuffers {
        inputs: &mut [],
        outputs: &mut [],
    };
    assert_no_heap(|| {
        let start = std::time::Instant::now();
        plugin.process(&mut buffer, &mut aux, &mut Host);
        start.elapsed().as_secs_f64()
    })
}

fn process_profiled(
    plugin: &mut GainStageFx,
    left: &mut [f32],
    right: Option<&mut [f32]>,
) -> CallbackTiming {
    let mut buffer = Buffer::default();
    // Match `process()` exactly, but measure both wall time and Linux
    // per-thread CPU time around the actual plugin callback.
    unsafe {
        buffer.set_slices(left.len(), |slices| {
            slices.push(left);
            if let Some(right) = right {
                slices.push(right);
            }
        });
    }
    let mut aux = AuxiliaryBuffers {
        inputs: &mut [],
        outputs: &mut [],
    };
    assert_no_heap(|| {
        let wall_start = std::time::Instant::now();
        let cpu_start = thread_cpu_seconds();
        plugin.process(&mut buffer, &mut aux, &mut Host);
        let cpu_finish = thread_cpu_seconds();
        let wall_seconds = wall_start.elapsed().as_secs_f64();
        let cpu_seconds = if cpu_start.is_finite() && cpu_finish.is_finite() {
            (cpu_finish - cpu_start).max(0.0)
        } else {
            wall_seconds
        };
        CallbackTiming {
            wall_seconds,
            cpu_seconds,
        }
    })
}

fn material(k: usize) -> f32 {
    (0.12 * (k as f64 * 0.057).sin() + 0.04 * (k as f64 * 0.213).sin()) as f32
}


#[test]
fn negotiated_layout_builds_exactly_one_chain_per_host_channel() {
    let mono = initialized(Circuit::Clean, true, 48_000.0);
    let stereo = initialized(Circuit::Clean, false, 48_000.0);
    assert_eq!(mono.channels.len(), 1, "mono layout must own exactly one chain");
    assert_eq!(stereo.channels.len(), 2, "stereo layout must own exactly two chains");
}

#[test]
fn mono_layout_processes_exactly_one_chain() {
    let mut mono = initialized(Circuit::Boogie, true, 48_000.0);
    assert_eq!(mono.channels.len(), 1);
    let before = mono.channels[0].solver_health().solves;
    for block in 0..12 {
        let mut samples = std::array::from_fn::<_, 64, _>(|i| material(block * 64 + i));
        process(&mut mono, &mut samples, None);
        assert!(samples.iter().all(|sample| sample.is_finite()));
    }
    assert!(mono.channels[0].solver_health().solves > before);
    assert_eq!(mono.channels.len(), 1, "mono processing must never create a hidden right chain");
}

#[test]
fn stereo_host_buffer_processes_exact_dual_mono_once() {
    for circuit in [Circuit::Boogie, Circuit::Peavey, Circuit::Twin, Circuit::Crunch] {
        let mut stereo = initialized(circuit, false, 48_000.0);
        assert_eq!(stereo.channels.len(), 2);
        let left_before = stereo.channels[0].solver_health().solves;
        let right_before = stereo.channels[1].solver_health().solves;
        for block in 0..12 {
            let mut left = std::array::from_fn::<_, 64, _>(|i| material(block * 64 + i));
            let mut right = left;
            process(&mut stereo, &mut left, Some(&mut right));
            assert_eq!(left, right, "dual-mono output diverged for {}", circuit.name());
        }
        assert!(stereo.channels[0].solver_health().solves > left_before);
        assert_eq!(
            stereo.channels[1].solver_health().solves,
            right_before,
            "{} exact dual-mono input unexpectedly ran the right nonlinear chain",
            circuit.name()
        );
        assert!(
            !stereo.stereo_seen,
            "{} exact dual-mono input was incorrectly latched as stereo",
            circuit.name()
        );
    }
}

#[test]
fn first_different_stereo_block_wakes_right_chain_and_latches_stereo() {
    let mut stereo = initialized(Circuit::Peavey, false, 48_000.0);

    // Begin with exact dual-mono so only the left chain advances.
    for block in 0..8 {
        let mut left = std::array::from_fn::<_, 64, _>(|i| material(block * 64 + i));
        let mut right = left;
        process(&mut stereo, &mut left, Some(&mut right));
    }
    let right_before = stereo.channels[1].solver_health().solves;
    assert!(!stereo.stereo_seen);

    // A single real difference must wake the right path for this same block.
    let mut left = std::array::from_fn::<_, 64, _>(|i| material(8 * 64 + i));
    let mut right = left;
    right[17] = -right[17];
    process(&mut stereo, &mut left, Some(&mut right));
    assert!(stereo.stereo_seen, "first differing block did not latch stereo");
    assert!(
        stereo.channels[1].solver_health().solves > right_before,
        "right chain did not run on the first differing stereo block"
    );

    // Once histories can differ, later equal input must remain two-chain stereo.
    let right_before_equal = stereo.channels[1].solver_health().solves;
    let mut left = std::array::from_fn::<_, 64, _>(|i| material(9 * 64 + i));
    let mut right = left;
    process(&mut stereo, &mut left, Some(&mut right));
    assert!(stereo.channels[1].solver_health().solves > right_before_equal);
}

#[test]
fn dual_mono_sleep_then_stereo_wake_matches_always_stereo_reference() {
    for circuit in [Circuit::Boogie, Circuit::Peavey, Circuit::Twin, Circuit::Crunch] {
        let mut auto = initialized(circuit, false, 48_000.0);
        let mut reference = initialized(circuit, false, 48_000.0);
        // Force the reference to process two independent chains from the first
        // sample. If the dormant-right synchronization is complete, waking the
        // auto path after identical history must produce the exact same output.
        reference.stereo_seen = true;

        for block in 0..20 {
            let source = std::array::from_fn::<_, 64, _>(|i| material(block * 64 + i));
            let mut auto_left = source;
            let mut ref_left = source;
            let mut auto_right = if block < 12 {
                source
            } else {
                std::array::from_fn::<_, 64, _>(|i| -0.45 * material(block * 64 + i + 9))
            };
            let mut ref_right = auto_right;

            process(&mut auto, &mut auto_left, Some(&mut auto_right));
            process(&mut reference, &mut ref_left, Some(&mut ref_right));

            assert_eq!(auto_left, ref_left, "left mismatch for {} at block {block}", circuit.name());
            assert_eq!(auto_right, ref_right, "right mismatch for {} at block {block}", circuit.name());
        }
        assert!(auto.stereo_seen, "{} never latched stereo after divergence", circuit.name());
    }
}

#[test]
fn stereo_layout_processes_both_chains_for_one_sided_input() {
    let mut stereo = initialized(Circuit::Peavey, false, 48_000.0);
    let left_before = stereo.channels[0].solver_health().solves;
    let right_before = stereo.channels[1].solver_health().solves;
    for block in 0..12 {
        let mut left = std::array::from_fn::<_, 64, _>(|i| material(block * 64 + i));
        let mut right = [0.0; 64];
        process(&mut stereo, &mut left, Some(&mut right));
        assert!(left.iter().all(|sample| sample.is_finite()));
        assert!(right.iter().all(|sample| sample.is_finite()));
    }
    assert!(stereo.channels[0].solver_health().solves > left_before);
    assert!(
        stereo.channels[1].solver_health().solves > right_before,
        "stereo right chain must run even when the current right input is silent"
    );
}

#[test]
fn mono_matches_each_side_of_identical_stereo() {
    for circuit in [Circuit::Clean, Circuit::Boogie, Circuit::Peavey, Circuit::Twin, Circuit::Crunch] {
        let mut mono = initialized(circuit, true, 48_000.0);
        let mut stereo = initialized(circuit, false, 48_000.0);
        for block in 0..16 {
            let source = std::array::from_fn::<_, 64, _>(|i| material(block * 64 + i));
            let mut mono_samples = source;
            let mut left = source;
            let mut right = source;
            process(&mut mono, &mut mono_samples, None);
            process(&mut stereo, &mut left, Some(&mut right));
            assert_eq!(mono_samples, left, "mono != stereo left for {}", circuit.name());
            assert_eq!(left, right, "stereo L/R mismatch for {}", circuit.name());
        }
    }
}

#[test]
fn stereo_channels_keep_independent_dynamic_history() {
    let mut stereo = initialized(Circuit::Twin, false, 48_000.0);
    let mut observed_difference = 0.0f32;
    for block in 0..24 {
        let mut left = std::array::from_fn::<_, 64, _>(|i| material(block * 64 + i));
        let mut right = std::array::from_fn::<_, 64, _>(|i| -0.5 * material(block * 64 + i + 11));
        if block >= 12 {
            // Equal input after divergent history must not collapse the two
            // dynamic signal paths back into one shared channel.
            right = left;
        }
        process(&mut stereo, &mut left, Some(&mut right));
        if block >= 12 {
            for (&l, &r) in left.iter().zip(&right) {
                observed_difference = observed_difference.max((l - r).abs());
            }
        }
    }
    assert!(
        observed_difference > 1e-7,
        "stereo dynamic histories were collapsed when L/R input later became equal"
    );
}

/// Actual Plugin::process timing for negotiated mono and stereo layouts.
///
/// This probe deliberately models a record-armed/live-monitored track rather
/// than an offline or anticipative playback render. A live input callback
/// cannot process future blocks early: every block arrives on the hardware
/// clock and must finish before the following period.
///
/// The bundled live guitar take is driven so that its loudest sample reaches
/// +12 dB on GainStageFx's input meter. The meter is centred at the circuit
/// calibration level (-18 dBFS), so the test peak is about -6 dBFS without
/// clipping the digital input.
///
/// The passes separate host bus width from actual signal topology:
///
/// * `playback_ahead_mono` uses the negotiated 1->1 layout without pacing.
/// * `live_paced_mono` uses the same 1->1 layout on the hardware clock.
/// * `live_paced_auto_dual_mono` uses a negotiated 2->2 host buffer carrying
///   exact L=R samples. Signal detection must run one nonlinear chain and mirror
///   its result, matching the mono workload despite the stereo transport bus.
/// * `live_paced_stereo_left_only` uses the same 2->2 host layout with a silent
///   right input. L != R is real stereo information to the plugin, so both
///   chains run; signal inspection never guesses that a hard-panned source is
///   semantically mono.
///
/// Run this ignored test alone in release mode on the machine where the live
/// dropout is observed. General-purpose CI runners are not realtime systems;
/// their wake-up jitter is useful diagnostic data but is not a substitute for
/// running the probe beside the actual audio configuration.
#[test]
#[ignore = "wall-clock live-input dropout probe using the bundled guitar WAV; run alone in release mode"]
fn realtime_recording() {
    const BLOCK: usize = 64;
    const TARGET_METER_DB: f32 = 12.0;
    const LIVE_SETTLE_BLOCKS: usize = 16;
    #[cfg(debug_assertions)]
    panic!("use cargo test --release for timing");

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/01-260912_1044.wav"
    );
    let (rate, recording) = read_mono_pcm24(path);
    assert_eq!(rate, 48_000, "this probe matches the live REAPER session");

    let seconds = std::env::var("GAINSTAGEFX_REALTIME_SECONDS")
        .map(|s| s.parse::<f64>().expect("seconds"))
        .unwrap_or(8.0);
    assert!(seconds.is_finite() && seconds >= 0.0);
    let frames = if seconds == 0.0 {
        recording.len()
    } else {
        ((seconds * rate as f64) as usize).min(recording.len())
    };
    assert!(frames >= BLOCK * (LIVE_SETTLE_BLOCKS + 1));

    let energy = |x: f32| (x as f64) * (x as f64);
    let mut sum: f64 = recording[..frames].iter().map(|&x| energy(x)).sum();
    let (mut best, mut offset) = (sum, 0);
    for end in frames..recording.len() {
        sum += energy(recording[end]) - energy(recording[end - frames]);
        if sum > best {
            best = sum;
            offset = end + 1 - frames;
        }
    }
    let input = &recording[offset..offset + frames];
    let settle_frames = LIVE_SETTLE_BLOCKS * BLOCK;
    let warmup_start = offset.saturating_sub(settle_frames);
    let warmup = &recording[warmup_start..offset];
    let input_peak = input
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    assert!(input_peak > 0.0, "recording excerpt is silent");
    let input_peak_dbfs = 20.0 * input_peak.log10();
    let target_peak_dbfs = NOMINAL_DBFS as f32 + TARGET_METER_DB;
    let input_trim_db = target_peak_dbfs - input_peak_dbfs;
    assert!(
        (-24.0..=24.0).contains(&input_trim_db),
        "fixture needs {input_trim_db:.2} dB of trim to reach the dropout-test level"
    );

    let period = std::time::Duration::from_secs_f64(BLOCK as f64 / rate as f64);
    println!(
        "recording={path}, rate={rate}, block={BLOCK}, callback_us={:.2}, start_seconds={:.3}, seconds={:.3}, raw_peak_dbfs={input_peak_dbfs:.2}, input_trim_db={input_trim_db:.2}, target_meter_db={TARGET_METER_DB:.2}, requested_preset={}",
        period.as_secs_f64() * 1e6,
        offset as f64 / rate as f64,
        frames as f64 / rate as f64,
        std::env::var("GAINSTAGEFX_ATTACK_PRESET").unwrap_or_else(|_| "<standard-test-config>".into())
    );
    println!(
        "voice,mode,blocks,mean_us,p99_us,max_us,compute_misses,cpu_mean_us,cpu_p99_us,cpu_max_us,cpu_compute_misses,preempt_mean_us,preempt_p99_us,preempt_max_us,deadline_misses,late_starts,max_start_late_us,max_finish_late_us,max_meter_db,right_solves,first_compute_miss,first_cpu_compute_miss,first_deadline_miss,gain_passes_per_solve,gain_unsettled,gain_backtracks,power_passes_per_solve,power_unsettled,power_backtracks,iron_passes_per_solve,reverb_passes_per_solve"
    );

    let mut failures = Vec::<String>::new();

    for circuit in [Circuit::Boogie, Circuit::Peavey, Circuit::Twin] {
        let mut playback_output = vec![0.0f32; frames];
        let playback_mono = run_realtime_pass(
            circuit,
            warmup,
            input,
            input_trim_db,
            rate,
            false,
            ProbeLayout::Mono,
            &mut playback_output,
        );
        print_realtime_pass(circuit, "playback_ahead_mono", &playback_mono);

        let mut live_mono_output = vec![0.0f32; frames];
        let live_mono = run_realtime_pass(
            circuit,
            warmup,
            input,
            input_trim_db,
            rate,
            true,
            ProbeLayout::Mono,
            &mut live_mono_output,
        );
        print_realtime_pass(circuit, "live_paced_mono", &live_mono);

        let mut stereo_duplicated_output = vec![0.0f32; frames];
        let stereo_duplicated = run_realtime_pass(
            circuit,
            warmup,
            input,
            input_trim_db,
            rate,
            true,
            ProbeLayout::AutoDualMono,
            &mut stereo_duplicated_output,
        );
        print_realtime_pass(
            circuit,
            "live_paced_auto_dual_mono",
            &stereo_duplicated,
        );

        let mut stereo_left_only_output = vec![0.0f32; frames];
        let stereo_left_only = run_realtime_pass(
            circuit,
            warmup,
            input,
            input_trim_db,
            rate,
            true,
            ProbeLayout::StereoLeftOnly,
            &mut stereo_left_only_output,
        );
        print_realtime_pass(circuit, "live_paced_stereo_left_only", &stereo_left_only);

        if playback_output != live_mono_output {
            failures.push(format!(
                "{}: pacing changed mono-layout audio output",
                circuit.name()
            ));
        }
        if live_mono.max_meter_db < TARGET_METER_DB - 0.25 {
            failures.push(format!(
                "{}: live_paced_mono only reached {:.2} dB on the input meter",
                circuit.name(), live_mono.max_meter_db
            ));
        }
        if live_mono.right_solves != 0 {
            failures.push(format!(
                "{}: mono layout unexpectedly has right-channel solves ({})",
                circuit.name(), live_mono.right_solves
            ));
        }
        if stereo_duplicated_output != live_mono_output {
            failures.push(format!(
                "{}: 2-channel exact dual-mono did not match true mono output",
                circuit.name()
            ));
        }
        if stereo_duplicated.right_solves != 0 {
            failures.push(format!(
                "{}: exact dual-mono signal unexpectedly ran the right chain ({})",
                circuit.name(), stereo_duplicated.right_solves
            ));
        }
        if live_mono.cpu_compute_misses != 0 {
            failures.push(format!(
                "{}: live_paced_mono consumed more than the {:.2} us hardware budget on {} callbacks of thread CPU time (first CPU miss {:?}; wall-time overruns {})",
                circuit.name(),
                period.as_secs_f64() * 1e6,
                live_mono.cpu_compute_misses,
                live_mono.first_cpu_compute_miss,
                live_mono.compute_misses
            ));
        }
        if live_mono.deadline_misses != 0 {
            failures.push(format!(
                "{}: live_paced_mono missed {} absolute deadlines (max finish lateness {:.2} us, {} late starts, first miss {:?})",
                circuit.name(), live_mono.deadline_misses, live_mono.max_finish_late_us, live_mono.late_starts, live_mono.first_deadline_miss
            ));
        }
        for (mode, result) in [
            ("live_paced_auto_dual_mono", &stereo_duplicated),
            ("live_paced_stereo_left_only", &stereo_left_only),
        ] {
            if result.max_meter_db < TARGET_METER_DB - 0.25 {
                failures.push(format!(
                    "{}: {mode} only reached {:.2} dB on the input meter",
                    circuit.name(), result.max_meter_db
                ));
            }
        }
        if stereo_left_only.right_solves == 0 {
            failures.push(format!(
                "{}: live_paced_stereo_left_only did not process the right stereo chain",
                circuit.name()
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "live realtime probe found realtime or signal-layout regressions:\n{}",
            failures.join("\n")
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeLayout {
    Mono,
    AutoDualMono,
    StereoLeftOnly,
}

impl ProbeLayout {
    fn is_mono(self) -> bool {
        matches!(self, Self::Mono)
    }
}

#[derive(Debug)]
struct RealtimePass {
    blocks: usize,
    mean_us: f64,
    p99_us: f64,
    max_us: f64,
    compute_misses: usize,
    cpu_mean_us: f64,
    cpu_p99_us: f64,
    cpu_max_us: f64,
    cpu_compute_misses: usize,
    preempt_mean_us: f64,
    preempt_p99_us: f64,
    preempt_max_us: f64,
    deadline_misses: usize,
    late_starts: usize,
    max_start_late_us: f64,
    max_finish_late_us: f64,
    max_meter_db: f32,
    right_solves: u64,
    first_compute_miss: Option<usize>,
    first_cpu_compute_miss: Option<usize>,
    first_deadline_miss: Option<usize>,
    solver: crate::voice::SolverBreakdown,
}

// Realtime probe inputs are kept explicit so each timing dimension is visible at the call site.
#[allow(clippy::too_many_arguments)]
fn run_realtime_pass(
    circuit: Circuit,
    warmup: &[f32],
    input: &[f32],
    input_trim_db: f32,
    rate: u32,
    live_paced: bool,
    layout: ProbeLayout,
    output: &mut [f32],
) -> RealtimePass {
    const BLOCK: usize = 64;
    const LIVE_SETTLE_BLOCKS: usize = 16;

    assert_eq!(input.len(), output.len());
    let period = std::time::Duration::from_secs_f64(BLOCK as f64 / rate as f64);
    let mut plugin = attacks::probe_configuration_layout(circuit, layout.is_mono());
    plugin.params.input_trim.smoothed.reset(input_trim_db);
    plugin.input_ramp.reset(util::db_to_gain(input_trim_db));
    assert_eq!(plugin.channels.len(), if layout.is_mono() { 1 } else { 2 });

    let max_warmup = LIVE_SETTLE_BLOCKS * BLOCK;
    let warmup = if warmup.len() > max_warmup {
        &warmup[warmup.len() - max_warmup..]
    } else {
        warmup
    };
    for chunk in warmup.chunks(BLOCK) {
        let mut left = [0.0; BLOCK];
        left[..chunk.len()].copy_from_slice(chunk);
        match layout {
            ProbeLayout::Mono => {
                process(&mut plugin, &mut left[..chunk.len()], None);
            }
            ProbeLayout::AutoDualMono => {
                let mut right = left;
                process(
                    &mut plugin,
                    &mut left[..chunk.len()],
                    Some(&mut right[..chunk.len()]),
                );
                assert_eq!(left, right);
            }
            ProbeLayout::StereoLeftOnly => {
                let mut right = [0.0; BLOCK];
                process(
                    &mut plugin,
                    &mut left[..chunk.len()],
                    Some(&mut right[..chunk.len()]),
                );
            }
        }
    }

    let solver_before = plugin.channels[0].solver_breakdown();
    let capacity = input.len().div_ceil(BLOCK);
    let mut times = Vec::with_capacity(capacity);
    let mut cpu_times = Vec::with_capacity(capacity);
    let mut preempt_times = Vec::with_capacity(capacity);
    let mut compute_misses = 0usize;
    let mut cpu_compute_misses = 0usize;
    let mut deadline_misses = 0usize;
    let mut first_compute_miss = None;
    let mut first_cpu_compute_miss = None;
    let mut first_deadline_miss = None;
    let mut late_starts = 0usize;
    let mut max_start_late_us = 0.0f64;
    let mut max_finish_late_us = 0.0f64;
    let mut max_meter_db = f32::NEG_INFINITY;

    let mut release = std::time::Instant::now()
        + if live_paced {
            std::time::Duration::from_millis(10)
        } else {
            std::time::Duration::ZERO
        };

    for (block, chunk) in input.chunks(BLOCK).enumerate() {
        if live_paced {
            wait_until(release);
        }
        let callback_start = std::time::Instant::now();
        let start_late = callback_start.saturating_duration_since(release);
        if live_paced {
            max_start_late_us = max_start_late_us.max(start_late.as_secs_f64() * 1e6);
            if start_late > std::time::Duration::from_micros(20) {
                late_starts += 1;
            }
        }

        let mut left = [0.0; BLOCK];
        left[..chunk.len()].copy_from_slice(chunk);
        let timing = match layout {
            ProbeLayout::Mono => process_profiled(&mut plugin, &mut left[..chunk.len()], None),
            ProbeLayout::AutoDualMono => {
                let mut right = left;
                let timing = process_profiled(
                    &mut plugin,
                    &mut left[..chunk.len()],
                    Some(&mut right[..chunk.len()]),
                );
                assert_eq!(left, right, "identical stereo input diverged");
                timing
            }
            ProbeLayout::StereoLeftOnly => {
                let mut right = [0.0; BLOCK];
                process_profiled(
                    &mut plugin,
                    &mut left[..chunk.len()],
                    Some(&mut right[..chunk.len()]),
                )
            }
        };
        let elapsed = timing.wall_seconds;
        let cpu_elapsed = timing.cpu_seconds;
        let callback_finish = std::time::Instant::now();
        let callback_budget = std::time::Duration::from_secs_f64(chunk.len() as f64 / rate as f64);
        let deadline = release + callback_budget;

        let wall_us = elapsed * 1e6;
        let cpu_us = cpu_elapsed * 1e6;
        let preempt_us = (wall_us - cpu_us).max(0.0);
        times.push(wall_us);
        cpu_times.push(cpu_us);
        preempt_times.push(preempt_us);
        if elapsed > callback_budget.as_secs_f64() {
            compute_misses += 1;
            first_compute_miss.get_or_insert(block);
        }
        if cpu_elapsed > callback_budget.as_secs_f64() {
            cpu_compute_misses += 1;
            first_cpu_compute_miss.get_or_insert(block);
        }
        if live_paced && callback_finish > deadline {
            deadline_misses += 1;
            first_deadline_miss.get_or_insert(block);
            max_finish_late_us = max_finish_late_us.max(
                callback_finish.duration_since(deadline).as_secs_f64() * 1e6,
            );
        }
        max_meter_db = max_meter_db.max(plugin.meters.input_db());
        let begin = block * BLOCK;
        output[begin..begin + chunk.len()].copy_from_slice(&left[..chunk.len()]);

        if live_paced {
            release += callback_budget;
            let now = std::time::Instant::now();
            while release + period < now {
                release += period;
            }
        } else {
            release = callback_finish;
        }
    }

    fn summarize(samples: &mut [f64]) -> (f64, f64, f64) {
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        samples.sort_by(f64::total_cmp);
        let p99 = samples[((samples.len() - 1) as f64 * 0.99).round() as usize];
        let max = samples[samples.len() - 1];
        (mean, p99, max)
    }

    let (mean_us, p99_us, max_us) = summarize(&mut times);
    let (cpu_mean_us, cpu_p99_us, cpu_max_us) = summarize(&mut cpu_times);
    let (preempt_mean_us, preempt_p99_us, preempt_max_us) = summarize(&mut preempt_times);
    let solver = plugin.channels[0]
        .solver_breakdown()
        .saturating_delta(solver_before);
    RealtimePass {
        blocks: times.len(),
        mean_us,
        p99_us,
        max_us,
        compute_misses,
        cpu_mean_us,
        cpu_p99_us,
        cpu_max_us,
        cpu_compute_misses,
        preempt_mean_us,
        preempt_p99_us,
        preempt_max_us,
        deadline_misses,
        late_starts,
        max_start_late_us,
        max_finish_late_us,
        max_meter_db,
        right_solves: plugin
            .channels
            .get(1)
            .map(|chain| chain.solver_health().solves)
            .unwrap_or(0),
        first_compute_miss,
        first_cpu_compute_miss,
        first_deadline_miss,
        solver,
    }
}

fn print_realtime_pass(circuit: Circuit, mode: &str, result: &RealtimePass) {
    println!(
        "{},{mode},{},{:.2},{:.2},{:.2},{},{:.2},{:.2},{:.2},{},{:.2},{:.2},{:.2},{},{},{:.2},{:.2},{:.2},{},{:?},{:?},{:?},{:.3},{},{},{:.3},{},{},{:.3},{:.3}",
        circuit.name(),
        result.blocks,
        result.mean_us,
        result.p99_us,
        result.max_us,
        result.compute_misses,
        result.cpu_mean_us,
        result.cpu_p99_us,
        result.cpu_max_us,
        result.cpu_compute_misses,
        result.preempt_mean_us,
        result.preempt_p99_us,
        result.preempt_max_us,
        result.deadline_misses,
        result.late_starts,
        result.max_start_late_us,
        result.max_finish_late_us,
        result.max_meter_db,
        result.right_solves,
        result.first_compute_miss,
        result.first_cpu_compute_miss,
        result.first_deadline_miss,
        result.solver.gain.passes_per_solve(),
        result.solver.gain.unsettled,
        result.solver.gain.backtracks,
        result.solver.power.passes_per_solve(),
        result.solver.power.unsettled,
        result.solver.power.backtracks,
        result.solver.iron.passes_per_solve(),
        result.solver.reverb_return.passes_per_solve(),
    );
}

fn wait_until(deadline: std::time::Instant) {
    // Sleep most of the interval so the CPU behaves like an actual live audio
    // workload instead of a hot offline render, then spin briefly to avoid
    // making the OS sleep quantum itself dominate the measurement.
    const SPIN: std::time::Duration = std::time::Duration::from_micros(80);
    loop {
        let now = std::time::Instant::now();
        if now >= deadline {
            return;
        }
        let remaining = deadline.duration_since(now);
        if remaining > SPIN {
            std::thread::sleep(remaining - SPIN);
        } else {
            std::hint::spin_loop();
        }
    }
}

fn read_mono_pcm24(path: &str) -> (u32, Vec<f32>) {
    let bytes = std::fs::read(path).expect("read recording");
    assert!(bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE");
    let mut at = 12;
    let mut rate = 0;
    let mut data = None;
    while at + 8 <= bytes.len() {
        let tag = &bytes[at..at + 4];
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let start = at + 8;
        let end = start.checked_add(len).expect("chunk size overflow");
        assert!(end <= bytes.len(), "truncated WAV chunk");
        if tag == b"fmt " {
            assert!(len >= 16);
            let u16_at = |offset| {
                u16::from_le_bytes(
                    bytes[start + offset..start + offset + 2]
                        .try_into()
                        .unwrap(),
                )
            };
            assert_eq!(
                (u16_at(0), u16_at(2), u16_at(12), u16_at(14)),
                (1, 1, 3, 24),
                "this fixture reader expects mono PCM24, as recorded by REAPER"
            );
            rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
        } else if tag == b"data" {
            data = Some(&bytes[start..end]);
        }
        at = end + (len & 1);
    }
    assert!(rate > 0);
    let data = data.expect("WAV data chunk");
    assert_eq!(data.len() % 3, 0);
    (
        rate,
        data.as_chunks::<3>().0.iter().map(|b| {
                let value = i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8;
                value as f32 / 8_388_608.0
            })
            .collect(),
    )
}
