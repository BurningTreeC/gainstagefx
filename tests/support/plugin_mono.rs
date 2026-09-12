//! Actual Plugin::process regressions, included inside plugin.rs for private
//! access to the stereo latch and a forced two-chain reference implementation.
use super::*;
use crate::params::{Cabinet, Circuit, Iron, ToneStack};

#[path = "allocations.rs"]
mod allocations;
use allocations::assert_no_heap;

#[path = "attacks.rs"]
mod attacks;

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

fn material(k: usize) -> f32 {
    (0.12 * (k as f64 * 0.057).sin() + 0.04 * (k as f64 * 0.213).sin()) as f32
}

fn equivalent(circuit: Circuit, bypass_at_wakeup: bool) {
    let mut fast = initialized(circuit, false, 48_000.0);
    let mut reference = initialized(circuit, false, 48_000.0);
    reference.stereo_seen = true; // Existing independent stereo control flow.
    let mut stereo_difference = 0.0f32;
    for block in 0..100 {
        for p in [&mut fast, &mut reference] {
            if block == 12 || block == 48 {
                p.params.drive.smoothed.set_target(48_000.0, 0.61);
                p.params.master.smoothed.set_target(48_000.0, 0.58);
                p.params.bass.smoothed.set_target(48_000.0, 0.43);
                p.params.eq240.smoothed.set_target(48_000.0, 0.64);
                p.params.input_trim.smoothed.set_target(48_000.0, 2.0);
                p.params.output_trim.smoothed.set_target(48_000.0, -1.0);
                p.params.mix.smoothed.set_target(48_000.0, 0.78);
            }
            if block == 24 {
                Arc::get_mut(&mut p.params).unwrap().oversampling =
                    EnumParam::new("Oversampling", Oversampling::Eight);
            }
            if block == 40 {
                p.params.reverb.smoothed.reset(0.0);
                p.params.intensity.smoothed.reset(0.0);
            }
            if block == 50 {
                p.params.reverb.smoothed.reset(0.7);
                p.params.intensity.smoothed.reset(0.9);
            }
            // Bypass must keep the active circuit advancing, including at
            // wake-up. Returning to wet must agree with the running reference.
            if bypass_at_wakeup && (block == 44 || block == 49) {
                Arc::get_mut(&mut p.params).unwrap().bypass = BoolParam::new("Bypass", block == 44);
            }
        }
        let mut left = std::array::from_fn::<_, 64, _>(|j| {
            if block >= 80 {
                0.0
            } else {
                material(block * 64 + j)
            }
        });
        let mut right = left;
        if (48..54).contains(&block) {
            // First difference is the very last frame: detection must examine
            // the entire incoming block before modifying any host samples.
            right[63] += 0.001;
        }
        let (mut reference_left, mut reference_right) = (left, right);
        let before = fast.channels[1].solver_health().solves;
        process(
            &mut reference,
            &mut reference_left,
            Some(&mut reference_right),
        );
        process(&mut fast, &mut left, Some(&mut right));
        assert_eq!(left, reference_left, "left changed for {}", circuit.name());
        for (&actual, &expected) in right.iter().zip(&reference_right) {
            assert!(actual.is_finite());
            assert!(
                (actual - expected).abs() < 2e-6 * (1.0 + expected.abs()),
                "{} block {block}: right {actual} != reference {expected}",
                circuit.name()
            );
        }
        if block < 48 {
            assert!(!fast.stereo_seen);
            assert_eq!(left, right);
            assert_eq!(
                fast.channels[1].solver_health().solves,
                before,
                "the dormant right chain processed samples"
            );
        } else {
            assert!(
                fast.stereo_seen,
                "stereo latch cleared on equal input/silence"
            );
            assert!(fast.channels[1].solver_health().solves > before);
            for (&l, &r) in left.iter().zip(&right) {
                stereo_difference = stereo_difference.max((l - r).abs());
            }
        }
    }
    assert!(stereo_difference > 1e-7, "stereo difference was collapsed");
}

#[test]
fn mark_duplicated_mono_and_stereo_wakeup_match_two_chains() {
    for bypass in [false, true] {
        equivalent(Circuit::Boogie, bypass);
    }
}
#[test]
fn peavey_duplicated_mono_and_stereo_wakeup_match_two_chains() {
    for bypass in [false, true] {
        equivalent(Circuit::Peavey, bypass);
    }
}
#[test]
fn twin_duplicated_mono_and_stereo_wakeup_preserve_disabled_tail() {
    for bypass in [false, true] {
        equivalent(Circuit::Twin, bypass);
    }
}
#[test]
fn oversampled_duplicated_mono_and_stereo_wakeup_preserve_firs() {
    equivalent(Circuit::Crunch, false);
}

#[test]
fn bit_detection_stereo_latch_reset_and_mono_layout() {
    let mut plugin = initialized(Circuit::Clean, false, 48_000.0);
    for right_value in [-0.0, f32::from_bits(0.1f32.to_bits() + 1)] {
        plugin.reset();
        let left_value = if right_value == 0.0 { 0.0 } else { 0.1 };
        process(
            &mut plugin,
            &mut [left_value; 16],
            Some(&mut [right_value; 16]),
        );
        assert!(
            plugin.stereo_seen,
            "bit-different input was treated as mono"
        );
        process(&mut plugin, &mut [0.0; 16], Some(&mut [0.0; 16]));
        assert!(plugin.stereo_seen);
    }
    plugin.reset();
    let before = plugin.channels[1].solver_health().solves;
    process(&mut plugin, &mut [0.0; 16], Some(&mut [0.0; 16]));
    assert!(!plugin.stereo_seen);
    assert_eq!(plugin.channels[1].solver_health().solves, before);
    assert!(plugin.initialize(
        &GainStageFx::AUDIO_IO_LAYOUTS[0],
        &BufferConfig {
            sample_rate: 96_000.0,
            min_buffer_size: Some(1),
            max_buffer_size: 64,
            process_mode: ProcessMode::Realtime
        },
        &mut Host
    ));
    assert!(!plugin.stereo_seen);

    let mut mono = initialized(Circuit::Clean, true, 48_000.0);
    let mut stereo = initialized(Circuit::Clean, false, 48_000.0);
    for _ in 0..12 {
        let mut l = [0.04; 64];
        let mut r = l;
        let mut m = l;
        process(&mut mono, &mut m, None);
        process(&mut stereo, &mut l, Some(&mut r));
        assert_eq!(m, l);
        assert_eq!(l, r);
    }
    assert!(!mono.stereo_seen);
}

#[test]
fn selections_while_dormant_and_on_the_wakeup_block_preserve_the_left_stream() {
    let mut fast = initialized(Circuit::Clean, false, 48_000.0);
    let mut reference = initialized(Circuit::Clean, false, 48_000.0);
    reference.stereo_seen = true;
    for block in 0..48 {
        let circuit = match block / 8 {
            0 => Circuit::Clean,
            1 => Circuit::Boogie,
            2 => Circuit::Twin,
            3 => Circuit::Crunch,
            4 => Circuit::Peavey,
            _ => Circuit::Muff,
        };
        for p in [&mut fast, &mut reference] {
            let params = Arc::get_mut(&mut p.params).unwrap();
            params.circuit = EnumParam::new("Circuit", circuit);
            params.tone = EnumParam::new(
                "Tone",
                if block < 24 {
                    ToneStack::Off
                } else {
                    ToneStack::Wide
                },
            );
            params.iron = EnumParam::new("Iron", if block < 16 { Iron::Off } else { Iron::Steel });
        }
        let mut l = std::array::from_fn::<_, 32, _>(|i| material(block * 32 + i));
        let mut r = l;
        if block == 32 {
            r[0] += 0.0001;
        }
        let (mut rl, mut rr) = (l, r);
        process(&mut reference, &mut rl, Some(&mut rr));
        process(&mut fast, &mut l, Some(&mut r));
        assert_eq!(l, rl);
        for (actual, expected) in r.into_iter().zip(rr) {
            assert!((actual - expected).abs() < 2e-6 * (1.0 + expected.abs()));
        }
        assert_eq!(fast.stereo_seen, block >= 32);
    }
}

#[test]
fn initial_four_times_quality_keeps_the_existing_timestep_on_wakeup() {
    for duplicate_blocks in [0, 3] {
        let mut fast = initialized(Circuit::Crunch, false, 48_000.0);
        let mut reference = initialized(Circuit::Crunch, false, 48_000.0);
        for plugin in [&mut fast, &mut reference] {
            Arc::get_mut(&mut plugin.params).unwrap().oversampling =
                EnumParam::new("Oversampling", Oversampling::Four);
            assert!(plugin.initialize(
                &GainStageFx::AUDIO_IO_LAYOUTS[0],
                &BufferConfig {
                    sample_rate: 48_000.0,
                    min_buffer_size: Some(1),
                    max_buffer_size: 64,
                    process_mode: ProcessMode::Realtime
                },
                &mut Host
            ));
        }
        reference.stereo_seen = true;
        for block in 0..12 {
            let mut left = std::array::from_fn::<_, 64, _>(|i| material(block * 64 + i));
            let mut right = left;
            if block >= duplicate_blocks {
                right[63] += 0.001;
            }
            let (mut rl, mut rr) = (left, right);
            if block > duplicate_blocks {
                for plugin in [&fast, &reference] {
                    plugin.params.drive.smoothed.set_target(48_000.0, 0.6);
                }
            }
            process(&mut fast, &mut left, Some(&mut right));
            process(&mut reference, &mut rl, Some(&mut rr));
            assert_eq!(left, rl);
            for (actual, expected) in right.into_iter().zip(rr) {
                assert!((actual - expected).abs() < 2e-6 * (1.0 + expected.abs()));
            }
        }
    }
}

/// Actual Plugin::process timing, unlike examples/realtime.rs which invokes
/// two Chains directly and therefore cannot exercise the plugin's fast path.
///
/// The checked-in guitar take is deliberately driven harder than recorded:
/// its loudest sample is trimmed to +12 dB on GainStageFx's input meter. The
/// meter is centred at the circuit calibration level (-18 dBFS), so this puts
/// the test peak at about -6 dBFS -- three quarters of the way across the
/// panel meter, well above its midpoint without digitally clipping the input.
///
/// Run alone in release mode. This is a wall-clock regression for the normal
/// duplicated-mono fast path used by a mono guitar source feeding the stereo
/// plugin. The reference two-chain and mono-layout paths are still timed and
/// printed for diagnostics, but only the duplicated-mono fast path is required
/// to finish every 256-frame callback before the host's 5.33 ms deadline.
#[test]
#[ignore = "wall-clock dropout probe using the bundled guitar WAV; run alone in release mode"]
fn realtime_recording() {
    const BLOCK: usize = 256;
    const TARGET_METER_DB: f32 = 12.0;
    #[cfg(debug_assertions)]
    panic!("use cargo test --release for timing");
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/01-260911_0537.wav"
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
    assert!(frames >= BLOCK);
    // Select the loudest contiguous excerpt, or the whole take for seconds=0.
    // Scanning, decoding, and allocating output storage happen before timing.
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
    let input_peak = input.iter().fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    assert!(input_peak > 0.0, "recording excerpt is silent");
    let input_peak_dbfs = 20.0 * input_peak.log10();
    let target_peak_dbfs = NOMINAL_DBFS as f32 + TARGET_METER_DB;
    let input_trim_db = target_peak_dbfs - input_peak_dbfs;
    assert!(
        (-24.0..=24.0).contains(&input_trim_db),
        "fixture needs {input_trim_db:.2} dB of trim to reach the dropout-test level"
    );
    println!(
        "recording={path}, rate={rate}, start_seconds={:.3}, seconds={:.3}, raw_peak_dbfs={input_peak_dbfs:.2}, input_trim_db={input_trim_db:.2}, target_meter_db={TARGET_METER_DB:.2}",
        offset as f64 / rate as f64,
        frames as f64 / rate as f64
    );
    println!("voice,mode,blocks,mean_us,p99_us,max_us,budget_percent,misses,right_solves,max_meter_db");
    for circuit in [Circuit::Boogie, Circuit::Peavey, Circuit::Twin] {
        let mut reference_output = vec![0.0f32; frames];
        let mut means = [0.0; 3];
        for (mode, mean_slot) in means.iter_mut().enumerate() {
            let mono_layout = mode == 2;
            let mut plugin = attacks::configured_layout(circuit, mono_layout);
            // Drive the real plugin input trim rather than pre-scaling the WAV.
            // Reset both layers because these callback tests bypass NIH-plug's
            // host-side parameter seeding.
            plugin.params.input_trim.smoothed.reset(input_trim_db);
            plugin.input_ramp.reset(util::db_to_gain(input_trim_db));
            plugin.stereo_seen = mode == 0;
            let mut times = Vec::with_capacity(frames.div_ceil(BLOCK));
            let mut misses = 0;
            let mut max_meter_db = f32::NEG_INFINITY;
            for (block, chunk) in input.chunks(BLOCK).enumerate() {
                let mut left = [0.0; BLOCK];
                left[..chunk.len()].copy_from_slice(chunk);
                let mut right = left;
                let elapsed = if mono_layout {
                    process(&mut plugin, &mut left[..chunk.len()], None)
                } else {
                    process(
                        &mut plugin,
                        &mut left[..chunk.len()],
                        Some(&mut right[..chunk.len()]),
                    )
                };
                times.push(elapsed * 1e6);
                if elapsed > chunk.len() as f64 / rate as f64 {
                    misses += 1;
                }
                max_meter_db = max_meter_db.max(plugin.meters.input_db());
                let expected = &mut reference_output[block * BLOCK..block * BLOCK + chunk.len()];
                if mode == 0 {
                    expected.copy_from_slice(&left[..chunk.len()]);
                } else {
                    assert_eq!(&left[..chunk.len()], expected, "left output changed");
                    if mode == 1 {
                        assert_eq!(left, right, "duplicated mono no longer matches");
                    }
                }
            }
            let mean = times.iter().sum::<f64>() / times.len() as f64;
            *mean_slot = mean;
            times.sort_by(f64::total_cmp);
            let p99 = times[((times.len() - 1) as f64 * 0.99).round() as usize];
            let right_solves = plugin.channels[1].solver_health().solves;
            if mode == 1 {
                assert_eq!(right_solves, 0);
            }
            let mode_name = match mode {
                0 => "two_chains",
                1 => "duplicated_mono",
                _ => "left_only",
            };
            println!(
                "{},{mode_name},{},{mean:.2},{p99:.2},{:.2},{:.2},{misses},{right_solves},{max_meter_db:.2}",
                circuit.name(),
                times.len(),
                times[times.len() - 1],
                times.iter().sum::<f64>() / (frames as f64 / rate as f64 * 1e6) * 100.0
            );
            assert!(
                max_meter_db >= TARGET_METER_DB - 0.25,
                "{} {mode_name} only reached {max_meter_db:.2} dB on the input meter; the dropout probe must run well above centre",
                circuit.name()
            );
            if mode == 1 {
                assert_eq!(
                    misses,
                    0,
                    "{} {mode_name} dropped {misses} blocks while the input meter was driven to about +{TARGET_METER_DB:.0} dB",
                    circuit.name()
                );
            }
        }
        assert!(
            means[1] < means[0] * 0.75,
            "fast path did not reduce CPU substantially"
        );
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
