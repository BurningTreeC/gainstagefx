//! Frozen outputs of the corrected circuits; never regenerate to hide a regression.
//!
//! **Captured 2026-09-25, deliberately, by the owner's instruction**, against the
//! tree at commit 19e2d6f ("DS-1, Peavey 4x12, Presence, and MT-2/DR103/5150
//! corrections"). `tests/legacy_baseline.rs` froze the Mark IIC+, 5150, Twin and
//! 73P before modular routing, and one by one each was corrected and recorded
//! there as an exception -- the Mark IIC+'s lead return, the completed AB763
//! Twin, the 73P's output block, and last the 5150's own tone stack -- until it
//! compared no sample at all. Its header said what would restore the guard: a
//! fresh capture of the corrected circuits, deliberately taken, as a new file.
//! This is that file, and those four voices are what it holds.
//!
//! The probe is the legacy one, unchanged, so the two are read the same way: the
//! same four voices, the five rates, four amplitudes, five signals (an impulse, a
//! sweep, four stepped tones, white and pink noise), 2,048 samples a run, Drive
//! 0.75, the plugin's tone section off, and **1x** -- see `legacy_baseline.rs`
//! for why a modelled circuit is frozen at the host rate.
//!
//! The rule is the legacy file's. If this stops matching and you did not mean to
//! change what the plugin does, the plugin is what is wrong. If you did mean to
//! change one of these voices, record the exception in this header, by date and
//! by reason, and stop comparing that voice -- do not recapture to make the test
//! pass. A new capture is a new, deliberate decision, taken by the owner.
use gainstagefx::voice::{Chain, Gain, Settings, Tone};
use std::f64::consts::TAU;

/// Samples per voice block in `render`: four amplitudes, five signals, 2048 each.
const BLOCK: usize = 4 * 5 * 2048;
/// One run: a reset, an operating point, and 2048 samples of one signal.
const RUN: usize = 2048;
/// The voices `render` walks, in order, at each rate.
const VOICES: [Gain; 4] = [Gain::Boogie, Gain::Peavey, Gain::Twin, Gain::Neve];
/// How many runs may reach the solver's fallback, over all four voices.
///
/// A run that falls back is not reproducible across machines -- which samples
/// fail depends on the last bit of `exp` and `sin` -- so those runs are not
/// compared. At capture 28 of the 400 runs fell back: 23 of the 73P's, at its
/// two hottest amplitudes and at every rate (the legacy capture had the same
/// 23); 3 of the Twin's and 2 of the 5150's, all at 0.7 and 96 kHz or above.
/// The budget is the legacy file's 40, which leaves room for a different libm to
/// tip a few more over and fails if the solve has become less stable.
const FALLBACK_BUDGET: usize = 40;
/// Runs that fell back at capture, as the settled mask beside the samples has it.
const CAPTURED_FALLBACKS: usize = 28;

const RATES: [f64; 5] = [44100.0, 48000.0, 88200.0, 96000.0, 192000.0];
const AMPLITUDES: [f64; 4] = [0.001, 0.03, 0.126, 0.7];

/// The rendered samples, and for each 2048-sample run whether the solver
/// settled every sample of it. The legacy fixture's probe, unchanged.
fn render() -> (Vec<f32>, Vec<bool>) {
    gainstagefx::dsp::time::enable_ftz_daz();
    let mut samples = Vec::new();
    let mut settled = Vec::new();
    for rate in RATES {
        let mut chain = Chain::new(rate);
        for gain in VOICES {
            chain.apply(&Settings {
                gain,
                tone: Tone::Off,
                drive: 0.75,
                oversampling: 1,
                ..Settings::default()
            });
            chain.settle();
            for amplitude in AMPLITUDES {
                for signal in 0..5 {
                    chain.reset();
                    chain.find_operating_point();
                    let before = chain.solver_breakdown();
                    let mut seed = 0x12345678u32;
                    let mut pink = 0.0;
                    let mut phase = 0.0;
                    for k in 0..2048 {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        let white = f64::from(seed) / f64::from(u32::MAX) * 2.0 - 1.0;
                        pink = 0.98 * pink + 0.02 * white;
                        let hz = match signal {
                            1 => 40.0 * 250.0_f64.powf(k as f64 / 2048.0),
                            2 => [82.0, 440.0, 1757.0, 6000.0][k / 512],
                            _ => 440.0,
                        };
                        phase += TAU * hz / rate;
                        let input = amplitude
                            * match signal {
                                0 => {
                                    if k == 0 {
                                        1.0
                                    } else {
                                        0.0
                                    }
                                }
                                1 | 2 => phase.sin(),
                                3 => white,
                                _ => pink * 5.0,
                            };
                        let y = chain.process(input);
                        assert!(y.is_finite(), "{gain:?} at {rate}");
                        samples.push(y as f32);
                    }
                    let work = chain.solver_breakdown().saturating_delta(before);
                    settled.push(work.gain.unsettled + work.power.unsettled == 0);
                }
            }
        }
    }
    (samples, settled)
}

const SAMPLES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/corrected-2026-09-25.f32le"
);
const SETTLED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/corrected-2026-09-25.settled"
);

#[test]
fn corrected_output_is_preserved() {
    let (actual, settled) = render();
    let bytes = std::fs::read(SAMPLES).expect("the capture is committed");
    assert_eq!(bytes.len(), actual.len() * 4);
    let (expected_samples, rest) = bytes.as_chunks::<4>();
    assert!(
        rest.is_empty(),
        "the fixture is not a whole number of samples"
    );
    let captured_settled = std::fs::read(SETTLED).expect("the settled mask is committed");
    assert_eq!(
        captured_settled.len(),
        settled.len(),
        "the settled mask is the wrong length"
    );
    assert_eq!(
        captured_settled.iter().filter(|s| **s == 0).count(),
        CAPTURED_FALLBACKS,
        "the settled mask is not the one this file describes"
    );

    let fell_back = settled.iter().filter(|now| !**now).count();
    assert!(
        fell_back <= FALLBACK_BUDGET,
        "{fell_back} of {} runs reached the solver's fallback, more than the \
         {FALLBACK_BUDGET} this test allows: something has made the solve less stable",
        settled.len(),
    );

    let (mut compared, mut error, mut energy) = (0usize, 0.0_f64, 0.0_f64);
    for (index, (&actual, bytes)) in actual.iter().zip(expected_samples).enumerate() {
        // A run the solver did not settle, now or at capture, is not
        // reproducible on another machine; see `FALLBACK_BUDGET`.
        if !settled[index / RUN] || captured_settled[index / RUN] == 0 {
            continue;
        }
        compared += 1;
        let expected = f32::from_le_bytes(*bytes);
        error += f64::from(actual - expected).powi(2);
        energy += f64::from(expected).powi(2);
        let voice = VOICES[(index / BLOCK) % VOICES.len()];
        let run_in_voice = (index % BLOCK) / RUN;
        let (amplitude, signal) = (AMPLITUDES[run_in_voice / 5], run_in_voice % 5);
        let sample = index % RUN;
        let rate = RATES[index / (BLOCK * VOICES.len())];
        assert!(
            (actual - expected).abs() <= 2e-6 * (1.0 + expected.abs()),
            "{voice:?} rate={rate} amplitude={amplitude} signal={signal} sample={sample}: \
             {actual} != {expected}"
        );
    }
    eprintln!(
        "corrected relative RMS error: {} over {compared} samples; {fell_back} of {} runs \
         reached the fallback and were skipped",
        (error / energy.max(1e-30)).sqrt(),
        settled.len(),
    );
    assert!(
        compared > actual.len() * 3 / 4,
        "only {compared} of {} samples were comparable; too few for a regression guard",
        actual.len()
    );
}

/// Writes the reference. Taken once, on 2026-09-25, on the owner's instruction;
/// see the header before running it again.
#[test]
#[ignore = "Captures the reference: a deliberate owner decision, never a way to make a change pass"]
fn capture_corrected_baseline() {
    let (samples, settled) = render();
    let bytes: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(SAMPLES, bytes).unwrap();
    std::fs::write(
        SETTLED,
        settled.iter().map(|ok| u8::from(*ok)).collect::<Vec<u8>>(),
    )
    .unwrap();
    eprintln!(
        "captured {} samples; {} of {} runs fell back",
        samples.len(),
        settled.iter().filter(|ok| !**ok).count(),
        settled.len()
    );
}
