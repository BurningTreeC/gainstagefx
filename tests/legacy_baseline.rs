//! Frozen outputs captured before modular routing; never regenerate to hide a regression.
//!
//! Rendered at **1x** deliberately. Every voice here is a modelled circuit, and
//! when this fixture was captured those were forced to the host rate whatever
//! the oversampling control said. They now follow it as far as
//! `voice::MODELLED_MAX_OVERSAMPLING`, so `Settings::default()` -- which asks
//! for 2x -- would render a different, cleaner signal and this file would stop
//! matching. Asking for 1x reproduces the condition the fixture was taken under,
//! which keeps the comparison sample-for-sample honest: at 1x the code path
//! through these circuits is unchanged. The cap has its own test
//! (`modelled_circuits_are_capped_not_pinned`), and what it means for a saved
//! session is in the README.
//!
//! **Re-baselined once, on 2026-09-16, and this is the reason.** The make-up that
//! level matches the circuit list was built from the gain of the *fundamental*
//! of a single 220 Hz sine. That is not loudness, and the error grows with
//! distortion: measured broadband the catalogue spanned 14.2 dB while being
//! nominally matched, with the most distorted circuits ten decibels above the
//! cleanest. The make-up now normalises the whole output, which changes every
//! voice's level -- including all four of the ones this file holds -- so the
//! old capture could not survive the correction. The table is anchored so the
//! catalogue's *average* loudness is unchanged (+0.48 dB), and this fixture was
//! captured again against the corrected make-up. It guards these voices from
//! here on exactly as before.
//!
//! This is a deliberate, documented change of the measurement, not a
//! regeneration to make a failing test pass. The rule stands: if this file stops
//! matching and you did not mean to change what the plugin does, the plugin is
//! what is wrong.
//!
//! One earlier exception, recorded the same way rather than by regenerating:
//! the Mark IIC+ voice was corrected on 2026-09-15 to include the lead return,
//! V2B, the Lead Master and V2A, which both of its drawings have and the frozen
//! model left out (`docs/models/cali_iic_plus.md`). Its blocks are no longer
//! compared. The Twin was likewise deliberately replaced in 2026-09 by the
//! completed AB763 circuit (coupled reverb recovery/mixer, optical tremolo
//! loading, corrected PI/power supply/output stage), so its old frozen samples
//! are no longer a valid regression oracle either. The 5150 and 73P blocks
//! remain frozen and are still compared sample for sample.
//!
//! **A third exception, 2026-09-23, and it leaves only the 5150 compared.** The
//! 73P's OUTPUT block -- two BC109C into a TIP3055 and the VTB1148 -- was in
//! that voice's *calibration* and not in its *path*: `Chain` built its output
//! stages from `Gain::power_stage`, which can only name a valve `PowerSpec`,
//! while `examples/calibrate` has always used `build_power`, which knows about
//! the blocks that are not one. So the make-up table took out a gain the
//! rendered signal never had, and what this fixture froze was an incomplete
//! 73P: the PRE blocks alone. Putting the block in the path changes the voice,
//! which is the correction, not a regression.
//!
//! That is a real loss of coverage and it is not disguised: of the four voices
//! here, three are now excluded and only the 5150 is still compared sample for
//! sample. The fixture is worth less than it was. What it still does is guard
//! the one untouched voice and the solver's stability budget across all four,
//! and the right way to restore the rest is a fresh frozen capture of the
//! corrected circuits -- a new fixture, deliberately taken, not a quiet
//! regeneration of this one.
//!
//! **A fourth exception, 2026-09-25, and it leaves no voice compared.** The
//! 5150 was built from Peavey's own preamp sheet with its tone stack left out:
//! the drawing's stack stood unread and a 33 k resistor stood in for it, with
//! the plugin's generic stack after the power stage. The stack is on the same
//! sheet (`docs/models/american_5150.md`) and is now built, which changes the
//! voice. That is a correction, and it is the last of the four.
//!
//! So this file no longer compares any sample, and says so rather than
//! pretending. What it still does: it renders all four voices through the chain
//! at every rate and amplitude and fails on a non-finite sample, and it holds
//! the 5150's runs to the fallback budget below -- the solver's stability
//! guard, counted on the voice as it is now. Restoring a sample-for-sample
//! guard needs a fresh capture of the corrected circuits: a new fixture,
//! deliberately taken by the owner, not a regeneration of this one.
use gainstagefx::voice::{Chain, Gain, Settings, Tone};
use std::f64::consts::TAU;

/// Samples per voice block in `render`: four amplitudes, five signals, 2048 each.
const BLOCK: usize = 4 * 5 * 2048;
/// One run: a reset, an operating point, and 2048 samples of one signal.
const RUN: usize = 2048;
/// How many guarded 5150/73P runs are allowed to reach the solver's fallback.
///
/// Not zero, and that is the finding this test carries. Measured with a
/// perturbation the size of a different libm's rounding, the 5150 settles every
/// sample at every amplitude and every rate; the **73P** can reach its fallback
/// at the hottest amplitudes. A run that falls back is not reproducible across
/// machines, because *which* samples fail depends on the last bit of
/// arithmetic, and a different glibc's `exp` and `sin` differ there. So those
/// guarded runs are not compared, and this budget fails if more of them appear.
const FALLBACK_BUDGET: usize = 40;
/// The voices `render` walks, in order, at each rate.
const VOICES: [Gain; 4] = [Gain::Boogie, Gain::Peavey, Gain::Twin, Gain::Neve];

/// The rendered samples, and for each 2048-sample run whether the solver
/// settled every sample of it.
fn render() -> (Vec<f32>, Vec<bool>) {
    gainstagefx::dsp::time::enable_ftz_daz();
    let mut samples = Vec::new();
    let mut settled = Vec::new();
    for rate in [44100.0, 48000.0, 88200.0, 96000.0, 192000.0] {
        let mut chain = Chain::new(rate);
        for gain in VOICES {
            chain.apply(&Settings {
                gain,
                tone: Tone::Off,
                drive: 0.75,
                // See the note at the top of this file.
                oversampling: 1,
                ..Settings::default()
            });
            chain.settle();
            for amplitude in [0.001, 0.03, 0.126, 0.7] {
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

#[test]
fn legacy_matched_output_is_preserved() {
    let (actual, settled) = render();
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/legacy-matched.f32le"
    ))
    .unwrap();
    assert_eq!(bytes.len(), actual.len() * 4);
    let mut error = 0.0_f64;
    let mut energy = 0.0_f64;
    let (expected_samples, rest) = bytes.as_chunks::<4>();
    assert!(
        rest.is_empty(),
        "the fixture is not a whole number of samples"
    );
    // Which runs settled when the fixture was captured. A run is compared only
    // if it settled *both* then and now: if it fell back at capture the stored
    // samples came from the fallback and no other machine will reproduce them,
    // and if it falls back now this machine will not reproduce the stored ones.
    let captured_settled = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/legacy-matched.settled"
    ))
    .expect("the settled mask is captured beside the samples");
    assert_eq!(
        captured_settled.len(),
        settled.len(),
        "the settled mask is the wrong length"
    );

    // No voice is compared any more; the 5150's runs still carry the solver's
    // stability budget, counted on the voice as it is now. See the header.
    let guarded_runs = settled.len() / VOICES.len();
    let fell_back = settled
        .iter()
        .enumerate()
        .filter(|(run, _)| VOICES[(*run / (4 * 5)) % VOICES.len()] == Gain::Peavey)
        .filter(|(_, now)| !**now)
        .count();
    assert!(
        fell_back <= FALLBACK_BUDGET,
        "{fell_back} of {} runs reached the solver's fallback, which is more than the \
         {FALLBACK_BUDGET} this test records. Those runs are not comparable across \
         machines; if the number has grown, something has made the solve less stable.",
        guarded_runs,
    );

    let mut compared = 0usize;
    const RATES: [f64; 5] = [44100.0, 48000.0, 88200.0, 96000.0, 192000.0];
    const AMPLITUDES: [f64; 4] = [0.001, 0.03, 0.126, 0.7];

    for (index, (&actual, bytes)) in actual.iter().zip(expected_samples).enumerate() {
        let voice = VOICES[(index / BLOCK) % VOICES.len()];
        // All four voices were deliberately changed after this fixture was
        // captured, so their historical samples are not valid regression
        // references. See the exceptions in the header.
        if matches!(voice, Gain::Boogie | Gain::Twin | Gain::Neve | Gain::Peavey) {
            continue;
        }
        // A run the solver did not settle is not reproducible on another
        // machine: which samples reached the fallback depends on the last bit
        // of arithmetic, and libm differs there between one glibc and another.
        if !settled[index / RUN] || captured_settled[index / RUN] == 0 {
            continue;
        }
        compared += 1;
        let expected = f32::from_le_bytes(*bytes);
        error += f64::from(actual - expected).powi(2);
        energy += f64::from(expected).powi(2);
        let voice_block = index % BLOCK;
        let run_in_voice = voice_block / RUN;
        let amplitude = AMPLITUDES[run_in_voice / 5];
        let signal = run_in_voice % 5;
        let sample = voice_block % RUN;
        let rate = RATES[index / (BLOCK * VOICES.len())];
        assert!(
            (actual - expected).abs() <= 2e-6 * (1.0 + expected.abs()),
            "{voice:?} rate={rate} amplitude={amplitude} signal={signal} sample={sample}: {actual} != {expected}"
        );
    }
    eprintln!(
        "legacy relative RMS error: {} over {compared} samples; {fell_back} of {} runs \
         reached the fallback and were skipped",
        (error / energy.max(1e-30)).sqrt(),
        guarded_runs,
    );
    // Nothing is compared since the fourth exception; this states it, so a
    // voice quietly re-admitted above is noticed rather than trusted.
    assert_eq!(
        compared, 0,
        "a voice is being compared against a stale capture"
    );
}

#[test]
#[ignore = "Run only against the pre-refactor source; overwrites the reference"]
fn capture_pre_refactor_baseline() {
    let (samples, settled) = render();
    let bytes: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/legacy-matched.f32le"
        ),
        bytes,
    )
    .unwrap();
    // Which runs the solver settled, so the comparison can tell the
    // reproducible part of this capture from the part that came from a
    // fallback. See `FALLBACK_BUDGET`.
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/legacy-matched.settled"
        ),
        settled.iter().map(|ok| u8::from(*ok)).collect::<Vec<u8>>(),
    )
    .unwrap();
}
