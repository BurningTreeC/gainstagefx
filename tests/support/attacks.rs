//! Attack regressions and offline timing. No timing or reporting is installed
//! in the production plugin.
use super::*;

fn treble_pick(k: usize, rate: f64) -> f32 {
    let t = k as f64 / rate;
    let since = t % 0.25;
    if since >= 0.12 {
        return 0.0;
    }
    let phase = std::f64::consts::TAU * 1757.0 * t;
    (0.8 * (-since * 30.0).exp() * (phase.sin() + 0.2 * (3.0 * phase).sin())) as f32
}

pub(super) fn configured(circuit: Circuit) -> GainStageFx {
    configured_layout(circuit, false)
}

pub(super) fn configured_layout(circuit: Circuit, mono: bool) -> GainStageFx {
    let mut p = initialized(circuit, mono, 48_000.0);
    let params = Arc::get_mut(&mut p.params).unwrap();
    params.iron = EnumParam::new("Iron", Iron::Off);
    params.tone = EnumParam::new("Tone", ToneStack::Wide);
    params.oversampling = EnumParam::new("Oversampling", Oversampling::Off);
    params.drive.smoothed.reset(0.75);
    params.bass.smoothed.reset(0.45);
    params.treble.smoothed.reset(0.65);
    params.reverb.smoothed.reset(0.0);
    params.intensity.smoothed.reset(0.0);
    params.mix.smoothed.reset(1.0);
    p
}

fn probe_configuration(circuit: Circuit) -> GainStageFx {
    let mut plugin = configured(circuit);
    if let Ok(name) = std::env::var("GAINSTAGEFX_ATTACK_PRESET") {
        apply_preset(&mut plugin, circuit, &name);
    }
    plugin
}

fn apply_preset(plugin: &mut GainStageFx, circuit: Circuit, name: &str) {
    let preset = crate::presets::PRESETS
        .iter()
        .find(|p| p.name == name)
        .expect("named preset exists");
    if preset.circuit != circuit {
        return;
    }
    let params = Arc::get_mut(&mut plugin.params).unwrap();
    params.iron = EnumParam::new("Iron", preset.iron);
    params.tone = EnumParam::new("Tone", preset.tone);
    params.cabinet = EnumParam::new("Cabinet", preset.cabinet);
    params.oversampling = EnumParam::new("Oversampling", preset.oversampling);
    for (param, value) in [
        (&params.drive, preset.drive),
        (&params.master, preset.master),
        (&params.bass, preset.bass),
        (&params.mid, preset.mid),
        (&params.treble, preset.treble),
        (&params.reverb, preset.reverb),
        (&params.speed, preset.speed),
        (&params.intensity, preset.intensity),
        (&params.input_trim, preset.input_trim),
        (&params.output_trim, preset.output_trim),
        (&params.mix, preset.mix),
    ] {
        param.smoothed.reset(value);
    }
    plugin.input_ramp.reset(util::db_to_gain(preset.input_trim));
    plugin
        .output_ramp
        .reset(util::db_to_gain(preset.output_trim));
    plugin.mix_ramp.reset(preset.mix);
    println!("preset={name}");
}

#[test]
#[ignore = "offline level/deadline sweep; run alone in release mode"]
fn attack_level_sweep() {
    const BLOCK: usize = 256;
    #[cfg(debug_assertions)]
    panic!("use cargo test --release for timing");
    let picks: Vec<_> = (0..96_000).map(|k| treble_pick(k, 48_000.0)).collect();
    let crest = picks.iter().fold(0.0f32, |p, x| p.max(x.abs()));
    let mut failed = 0;
    println!("voice,drive,input_dbfs,meter_db,mean_us,p99_us,max_us,misses,unsettled");
    for circuit in [Circuit::Peavey, Circuit::Twin] {
        for drive in [0.5, 0.75, 1.0] {
            for dbfs in [-24.0f32, -18.0, -12.0, -6.0, 0.0] {
                let mut plugin = probe_configuration(circuit);
                plugin.params.drive.smoothed.reset(drive);
                let scale = util::db_to_gain(dbfs) / crest;
                let mut times = Vec::with_capacity(picks.len() / BLOCK);
                for chunk in picks.as_chunks::<BLOCK>().0 {
                    let mut left = std::array::from_fn::<_, BLOCK, _>(|j| chunk[j] * scale);
                    let mut right = left;
                    times.push(process(&mut plugin, &mut left, Some(&mut right)) * 1e6);
                    assert_eq!(left, right);
                    assert!(left.iter().all(|x| x.is_finite()));
                }
                let health = plugin.channels[0].solver_health();
                assert_eq!(health.nonfinite, 0);
                assert_eq!(plugin.channels[1].solver_health().solves, 0);
                failed += health.unsettled;
                let mean = times.iter().sum::<f64>() / times.len() as f64;
                let misses = times
                    .iter()
                    .filter(|&&t| t > 1e6 * BLOCK as f64 / 48_000.0)
                    .count();
                times.sort_by(f64::total_cmp);
                println!(
                    "{},{drive},{dbfs},{},{mean:.2},{:.2},{:.2},{misses},{}",
                    circuit.name(),
                    dbfs - NOMINAL_DBFS as f32,
                    times[(times.len() - 1) * 99 / 100],
                    times[times.len() - 1],
                    health.unsettled
                );
            }
        }
    }
    assert_eq!(
        failed, 0,
        "unconverged samples across the input-meter sweep"
    );
}

#[test]
fn power_stages_converge_through_high_pick_attacks() {
    // Actual 48 kHz / 256-frame callbacks with the saved guitar preset's
    // drive and master settings. The old solver failed 73 samples on the
    // 5150 and six on the Twin in this two-second signal.
    for circuit in [Circuit::Peavey, Circuit::Twin] {
        check_pick_attacks(configured(circuit));
    }
}

#[test]
fn blackface_throb_converges_through_high_pick_attacks() {
    let mut plugin = configured(Circuit::Twin);
    // The user's current patch includes Steel iron, spring reverb and tremolo.
    apply_preset(&mut plugin, Circuit::Twin, "Blackface Throb");
    check_pick_attacks(plugin);
}

fn check_pick_attacks(mut plugin: GainStageFx) {
    for block in 0..375 {
        let mut left = std::array::from_fn::<_, 256, _>(|j| treble_pick(block * 256 + j, 48_000.0));
        let mut right = left;
        process(&mut plugin, &mut left, Some(&mut right));
        assert_eq!(left, right);
        assert!(left.iter().all(|x| x.is_finite()));
    }
    let health = plugin.channels[0].solver_health();
    assert_eq!(health.unsettled, 0, "failed solves");
    assert_eq!(health.nonfinite, 0, "nonfinite corrections");
    assert_eq!(plugin.channels[1].solver_health().solves, 0);
}

#[test]
#[ignore = "offline attack/timing probe; GAINSTAGEFX_REALTIME_WAV, release mode"]
fn attack_recording_probe() {
    const BLOCK: usize = 256;
    #[cfg(debug_assertions)]
    panic!("use cargo test --release for timing");
    let path = std::env::var("GAINSTAGEFX_REALTIME_WAV").unwrap();
    let (rate, recording) = read_mono_pcm24(&path);
    assert_eq!(rate, 48_000);
    let seconds = std::env::var("GAINSTAGEFX_REALTIME_SECONDS")
        .map(|v| v.parse::<f64>().expect("seconds"))
        .unwrap_or(8.0);
    assert!(seconds.is_finite() && seconds >= 0.0);
    let frames = if seconds == 0.0 {
        recording.len()
    } else {
        (seconds * rate as f64) as usize
    }
    .min(recording.len());
    assert!(frames >= BLOCK);
    let mut energy: f64 = recording[..frames]
        .iter()
        .map(|&v| (v as f64).powi(2))
        .sum();
    let (mut best, mut offset) = (energy, 0);
    for end in frames..recording.len() {
        energy += (recording[end] as f64).powi(2) - (recording[end - frames] as f64).powi(2);
        if energy > best {
            best = energy;
            offset = end + 1 - frames;
        }
    }
    let picks: Vec<f32> = (0..96_000).map(|k| treble_pick(k, rate as f64)).collect();
    println!(
        "recording={path}, rate={rate}, block={BLOCK}, frames={frames}, start_seconds={:.6}",
        offset as f64 / rate as f64,
    );
    println!("voice,material,mean_us,p99_us,max_us,misses,passes,unsettled,backtracks,fallbacks,nonfinite,peak,max_step");
    for circuit in [Circuit::Peavey, Circuit::Twin] {
        for (label, input) in [
            ("recording", &recording[offset..offset + frames]),
            ("treble_picks", picks.as_slice()),
        ] {
            let mut plugin = probe_configuration(circuit);
            if matches!(circuit, Circuit::Twin) {
                for (key, param) in [
                    ("GAINSTAGEFX_ATTACK_REVERB", &plugin.params.reverb),
                    ("GAINSTAGEFX_ATTACK_INTENSITY", &plugin.params.intensity),
                ] {
                    if let Ok(value) = std::env::var(key) {
                        let value = value.parse::<f32>().expect("effect amount");
                        assert!((0.0..=1.0).contains(&value));
                        param.smoothed.reset(value);
                        println!("{key}={value}");
                    }
                }
            }
            let mut times = Vec::with_capacity(input.len().div_ceil(BLOCK));
            let mut trace = Vec::with_capacity(times.capacity());
            let mut output = Vec::with_capacity(input.len());
            let mut previous = 0.0f32;
            let (mut peak, mut step) = (0.0f32, 0.0f32);
            let mut misses = 0;
            for (block, chunk) in input.chunks(BLOCK).enumerate() {
                let mut left = [0.0; BLOCK];
                left[..chunk.len()].copy_from_slice(chunk);
                let mut right = left;
                let before = plugin.channels[0].solver_health();
                let elapsed = process(
                    &mut plugin,
                    &mut left[..chunk.len()],
                    Some(&mut right[..chunk.len()]),
                ) * 1e6;
                if elapsed > chunk.len() as f64 / rate as f64 * 1e6 {
                    misses += 1;
                }
                let after = plugin.channels[0].solver_health();
                let mut block_step = 0.0f32;
                for &sample in &left[..chunk.len()] {
                    assert!(sample.is_finite());
                    peak = peak.max(sample.abs());
                    block_step = block_step.max((sample - previous).abs());
                    previous = sample;
                }
                step = step.max(block_step);
                times.push(elapsed);
                output.extend_from_slice(&left[..chunk.len()]);
                trace.push((
                    block,
                    elapsed,
                    after.passes - before.passes,
                    after.unsettled - before.unsettled,
                    after.backtracks - before.backtracks,
                    block_step,
                ));
            }
            let health = plugin.channels[0].solver_health();
            let mean = times.iter().sum::<f64>() / times.len() as f64;
            times.sort_by(f64::total_cmp);
            println!(
                "{},{label},{mean:.2},{:.2},{:.2},{misses},{},{},{},{},{},{peak:.6},{step:.6}",
                circuit.name(),
                times[(times.len() - 1) * 99 / 100],
                times[times.len() - 1],
                health.passes,
                health.unsettled,
                health.backtracks,
                health.fallbacks,
                health.nonfinite
            );
            assert_eq!(plugin.channels[1].solver_health().solves, 0);
            let power = plugin.channels[0].test_power().unwrap();
            println!(
                "power: statistics={:?}, health={:?}",
                power.statistics(),
                power.health()
            );
            if let Ok(directory) = std::env::var("GAINSTAGEFX_ATTACK_OUTPUT") {
                let directory = std::path::Path::new(&directory);
                std::fs::create_dir_all(directory).unwrap();
                let stem = format!("{}-{label}", circuit.name().replace(' ', "_"));
                let mut csv =
                    String::from("block,microseconds,passes,unsettled,backtracks,max_step\n");
                for (b, t, p, u, bt, s) in trace {
                    use std::fmt::Write;
                    writeln!(&mut csv, "{b},{t},{p},{u},{bt},{s}").unwrap();
                }
                std::fs::write(directory.join(format!("{stem}.csv")), csv).unwrap();
                let bytes: Vec<_> = output.iter().flat_map(|v| v.to_le_bytes()).collect();
                std::fs::write(directory.join(format!("{stem}.f32")), bytes).unwrap();
            }
        }
    }
}
