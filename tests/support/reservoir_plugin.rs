//! The plugin with its DSP on the reservoir's worker, against the plugin with
//! its DSP on the host thread: the same audio to the bit, `D` samples later,
//! through every layout, block size, automation move and device change; the
//! latency reported is the latency there is; and nothing older than a reset
//! gets through.
use super::*;

const D: usize = 64;

#[derive(Clone, Copy, Debug)]
enum Feed {
    Mono,
    DualMono,
    Stereo,
    LeftOnly,
    RightOnly,
}

impl Feed {
    fn mono(self) -> bool {
        matches!(self, Feed::Mono)
    }

    fn frame(self, k: usize) -> (f32, f32) {
        let a = material(k) * 3.0;
        let b = -0.7 * material(k + 113) * 3.0;
        match self {
            Feed::Mono | Feed::DualMono => (a, a),
            Feed::Stereo => (a, b),
            Feed::LeftOnly => (a, 0.0),
            Feed::RightOnly => (0.0, b),
        }
    }
}

/// What a host does to the parameters between two calls -- nice-plug splits
/// a block at every change and calls `process` for each piece, so this is
/// sample-accurate automation as the plugin sees it.
fn automate(plugin: &mut GainStageFx, call: usize) {
    let p = Arc::get_mut(&mut plugin.params).expect("the parameters are unshared");
    match call {
        12 => p.drive.smoothed.set_target(48_000.0, 0.35),
        17 => p.mix.smoothed.set_target(48_000.0, 0.4),
        23 => p.treble.smoothed.set_target(48_000.0, 0.9),
        // A device change mid-stream: the new circuit must meet exactly the
        // frames played after it, not the ones queued before it.
        31 => p.circuit = EnumParam::new("Circuit", Circuit::Peavey),
        38 => p.bypass = BoolParam::new("Bypass", true),
        40 => p.bypass = BoolParam::new("Bypass", false),
        44 => p.output_trim.smoothed.set_target(48_000.0, -6.0),
        _ => {}
    }
}

/// Run the same calls through a synchronous plugin and one behind a reservoir
/// of `delay` (offline, so the worker is always waited for), and check that
/// the second is the first delayed by exactly `delay`, to the bit.
/// With `tolerance` zero the comparison is to the bit; otherwise it is the
/// largest difference allowed, and the largest found is returned.
fn compare(
    circuit: Circuit,
    feed: Feed,
    blocks: &[usize],
    calls: usize,
    delay: usize,
    speculation: bool,
    tolerance: f32,
) -> f32 {
    let mut reference = initialized_speculating(
        circuit,
        feed.mono(),
        48_000.0,
        0,
        ProcessMode::Realtime,
        speculation,
    );
    let mut reservoir = initialized_speculating(
        circuit,
        feed.mono(),
        48_000.0,
        delay,
        ProcessMode::Offline,
        speculation,
    );
    assert_eq!(reservoir.reservoir_delay(), delay);
    let (mut ref_l, mut ref_r, mut res_l, mut res_r) = (vec![], vec![], vec![], vec![]);
    let mut k = 0;
    for call in 0..calls {
        automate(&mut reference, call);
        automate(&mut reservoir, call);
        let n = blocks[call % blocks.len()];
        let frames: Vec<(f32, f32)> = (k..k + n).map(|i| feed.frame(i)).collect();
        k += n;
        let mut left: Vec<f32> = frames.iter().map(|f| f.0).collect();
        let mut right: Vec<f32> = frames.iter().map(|f| f.1).collect();
        let (mut left2, mut right2) = (left.clone(), right.clone());
        if feed.mono() {
            process(&mut reference, &mut left, None);
            process(&mut reservoir, &mut left2, None);
        } else {
            process(&mut reference, &mut left, Some(&mut right));
            process(&mut reservoir, &mut left2, Some(&mut right2));
            ref_r.extend_from_slice(&right);
            res_r.extend_from_slice(&right2);
        }
        ref_l.extend_from_slice(&left);
        res_l.extend_from_slice(&left2);
    }
    let label = format!("{} {feed:?} D={delay}", circuit.name());
    let mut largest = 0.0f32;
    for (reference, delayed, side) in [(&ref_l, &res_l, "left"), (&ref_r, &res_r, "right")] {
        for (t, &y) in delayed.iter().enumerate() {
            let expected = if t < delay { 0.0 } else { reference[t - delay] };
            if tolerance == 0.0 {
                assert!(
                    y.to_bits() == expected.to_bits(),
                    "{label} {side}: frame {t} is {y}, the synchronous plugin's {delay} earlier is {expected}"
                );
            }
            largest = largest.max((y - expected).abs());
        }
    }
    assert!(largest <= tolerance, "{label}: differs by {largest}");
    let stats = reservoir.reservoir_stats().expect("a reservoir is running");
    assert_eq!(stats.underruns(), 0, "{label}");
    assert_eq!(stats.input_overflows(), 0, "{label}");
    assert_eq!(stats.output_overflows(), 0, "{label}");
    // Something must have been heard, or the comparison proves nothing.
    assert!(
        ref_l.iter().chain(ref_r.iter()).any(|x| x.abs() > 1e-3),
        "{label}: silent"
    );
    largest
}

const CIRCUITS: [Circuit; 4] = [
    Circuit::Boogie,
    Circuit::Twin,
    Circuit::Jazz120,
    Circuit::Crunch,
];
const FEEDS: [Feed; 5] = [
    Feed::Mono,
    Feed::DualMono,
    Feed::Stereo,
    Feed::LeftOnly,
    Feed::RightOnly,
];

#[test]
fn behind_a_reservoir_the_plugin_is_the_synchronous_plugin_delayed() {
    // Blocks no longer than the reservoir, so every call is processed as the
    // synchronous plugin processes it: identical to the bit, speculation and
    // all.
    let blocks = [64, 37, 5, 1, 63, 48, 17];
    for circuit in CIRCUITS {
        for feed in FEEDS {
            compare(circuit, feed, &blocks, 56, D, true, 0.0);
        }
    }
}

#[test]
fn a_block_longer_than_the_reservoir_is_split_exactly() {
    // Blocks longer than the reservoir are processed in two parts, the
    // frames the call returns first. Everything per call happens once and the
    // ramps run on across the parts, so where the boundary falls cannot
    // matter -- except to the power stage's speculation, which decides per
    // block. With it off, the split is exact to the bit.
    let blocks = [64, 90, 128, 65, 37, 200];
    for circuit in CIRCUITS {
        for feed in FEEDS {
            compare(circuit, feed, &blocks, 48, D, false, 0.0);
        }
    }
}

#[test]
fn where_the_power_stage_speculates_a_split_moves_only_rounding() {
    // With speculation on, a split block is another place for a block to end,
    // as a smaller host buffer would have been, and the speculating power
    // stage rounds differently there: a unit in the last place on this
    // material. (The JC-120's power stage can carry such a difference into a
    // different, equally converged fallback; on the real take at 256-sample
    // blocks that made short bursts of up to -48 dB -- as running the
    // synchronous plugin at a different block size does.)
    let blocks = [64, 90, 128, 65, 37, 200];
    for circuit in CIRCUITS {
        let largest = compare(circuit, Feed::DualMono, &blocks, 48, D, true, 1e-5);
        println!("{}: largest difference {largest:e}", circuit.name());
    }
}

#[test]
fn every_depth_is_exact() {
    for delay in [21, 32, 96, 128] {
        compare(
            Circuit::Boogie,
            Feed::DualMono,
            &[64, 17, 128, 48],
            40,
            delay,
            false,
            0.0,
        );
    }
}

#[test]
fn the_latency_reported_is_the_latency_there_is() {
    // Dry only, so an impulse goes through the dry path and nothing else:
    // it arrives after the reservoir plus the chain's own oversampling
    // latency, and that sum must be what the host was told.
    for rate in [44_100.0f32, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        for (oversampling, chain) in [(Oversampling::Off, 0u32), (Oversampling::Four, 56)] {
            for delay in [21usize, 64, 128] {
                let mut plugin = GainStageFx::default();
                {
                    let p = Arc::get_mut(&mut plugin.params).unwrap();
                    p.circuit = EnumParam::new("Circuit", Circuit::Boogie);
                    p.oversampling = EnumParam::new("Oversampling", oversampling);
                    p.mix = FloatParam::new("Mix", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 });
                }
                plugin.set_reservoir_delay(delay);
                ACTIVATED.with(|a| a.set(None));
                assert!(plugin.activate(
                    &GainStageFx::AUDIO_IO_LAYOUTS[1],
                    &BufferConfig {
                        sample_rate: rate,
                        min_buffer_size: Some(1),
                        max_buffer_size: 512,
                        process_mode: ProcessMode::Offline,
                    },
                    &mut Host,
                ));
                plugin.reset();
                let reported = ACTIVATED
                    .with(Cell::get)
                    .expect("activation reports a latency");
                assert_eq!(
                    reported,
                    delay as u32 + chain,
                    "{rate} Hz {oversampling:?} D={delay}"
                );
                assert_eq!(plugin.reported_latency(), reported);

                // An impulse a little way in, so priming is not what is
                // measured, through blocks that do not divide anything.
                let at = 300;
                let mut out = Vec::new();
                let mut k = 0;
                for n in [48usize, 100, 7, 64].iter().cycle().take(40) {
                    let mut block: Vec<f32> = (k..k + n)
                        .map(|i| if i == at { 0.5 } else { 0.0 })
                        .collect();
                    k += n;
                    process(&mut plugin, &mut block, None);
                    out.extend_from_slice(&block);
                }
                let arrived = out
                    .iter()
                    .position(|&x| x != 0.0)
                    .expect("the impulse came out");
                assert_eq!(
                    arrived - at,
                    reported as usize,
                    "{rate} Hz {oversampling:?} D={delay}"
                );
                assert_eq!(out[arrived], 0.5);
            }
        }
    }
}

#[test]
fn a_changed_oversampling_reports_the_reservoir_too() {
    let mut plugin = initialized_with(Circuit::Boogie, true, 48_000.0, D, ProcessMode::Offline);
    REPORTED.with(|r| r.set(None));
    let mut block = vec![0.0f32; 64];
    process(&mut plugin, &mut block, None);
    assert_eq!(
        REPORTED.with(Cell::get),
        None,
        "nothing changed, nothing reported"
    );
    Arc::get_mut(&mut plugin.params).unwrap().oversampling =
        EnumParam::new("Oversampling", Oversampling::Four);
    process(&mut plugin, &mut block, None);
    assert_eq!(REPORTED.with(Cell::get), Some(D as u32 + 56));
}

#[test]
fn after_a_reset_nothing_from_before_comes_out() {
    // Both plugins get the same calls and the same reset in the middle. The
    // reservoir's output after the reset is silence for D frames and then
    // the synchronous plugin's post-reset output: the D frames it still held
    // from before the reset are never played.
    let mut reference = initialized(Circuit::Twin, true, 48_000.0);
    let mut reservoir = initialized_with(Circuit::Twin, true, 48_000.0, D, ProcessMode::Offline);
    let run = |plugin: &mut GainStageFx, from: usize, calls: usize| {
        let mut out = Vec::new();
        for call in 0..calls {
            let mut block: Vec<f32> = (0..64)
                .map(|i| material(from + call * 64 + i) * 4.0)
                .collect();
            process(plugin, &mut block, None);
            out.extend_from_slice(&block);
        }
        out
    };
    let before_ref = run(&mut reference, 0, 20);
    let before_res = run(&mut reservoir, 0, 20);
    reference.reset();
    reservoir.reset();
    let after_ref = run(&mut reference, 50_000, 20);
    let after_res = run(&mut reservoir, 50_000, 20);
    for t in 0..before_res.len() {
        let expected = if t < D { 0.0 } else { before_ref[t - D] };
        assert_eq!(
            before_res[t].to_bits(),
            expected.to_bits(),
            "before, frame {t}"
        );
    }
    for t in 0..after_res.len() {
        let expected = if t < D { 0.0 } else { after_ref[t - D] };
        assert_eq!(
            after_res[t].to_bits(),
            expected.to_bits(),
            "after, frame {t}"
        );
    }
}

#[test]
fn behind_a_reservoir_the_callback_does_not_allocate() {
    // Real time, calls back to back: the callback waits for the worker only
    // as far as the host is ahead, and allocates nothing either way -- not
    // in `process`, not in a reset from the audio thread.
    let mut plugin = initialized_with(Circuit::Boogie, false, 48_000.0, D, ProcessMode::Realtime);
    for call in 0..200 {
        let mut left: [f32; 64] = std::array::from_fn(|i| material(call * 64 + i));
        let mut right = left;
        process(&mut plugin, &mut left, Some(&mut right));
        if call == 100 {
            crate::test_allocations::assert_no_heap(|| plugin.reset());
        }
    }
    let stats = plugin.reservoir_stats().unwrap();
    assert!(
        stats
            .callback
            .callbacks
            .load(std::sync::atomic::Ordering::Relaxed)
            >= 200
    );
}

#[test]
fn instances_do_not_share_a_reservoir() {
    let mut a = initialized_with(Circuit::Boogie, true, 48_000.0, D, ProcessMode::Offline);
    let mut b = initialized_with(Circuit::Jazz120, true, 48_000.0, 96, ProcessMode::Offline);
    let mut ref_a = initialized(Circuit::Boogie, true, 48_000.0);
    let mut ref_b = initialized(Circuit::Jazz120, true, 48_000.0);
    let (mut out_a, mut out_b, mut out_ra, mut out_rb) = (vec![], vec![], vec![], vec![]);
    for call in 0..30 {
        let source: [f32; 64] = std::array::from_fn(|i| material(call * 64 + i) * 3.0);
        for (plugin, out) in [
            (&mut a, &mut out_a),
            (&mut b, &mut out_b),
            (&mut ref_a, &mut out_ra),
            (&mut ref_b, &mut out_rb),
        ] {
            let mut block = source;
            process(plugin, &mut block, None);
            out.extend_from_slice(&block);
        }
    }
    for (out, reference, delay) in [(&out_a, &out_ra, D), (&out_b, &out_rb, 96)] {
        for t in delay..out.len() {
            assert_eq!(
                out[t].to_bits(),
                reference[t - delay].to_bits(),
                "frame {t}"
            );
        }
    }
}

#[test]
fn a_queued_call_is_small() {
    // Every host call queues one `Controls`; the header ring holds up to 512
    // of them per instance. Keep it a plain value of a few hundred bytes.
    let size = std::mem::size_of::<Controls>();
    println!(
        "Controls: {size} bytes; 512 queued: {} KiB",
        size * 512 / 1024
    );
    assert!(size <= 1024, "Controls has grown to {size} bytes");
}
