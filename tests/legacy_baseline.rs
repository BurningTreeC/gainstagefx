//! Frozen outputs captured before modular routing; never regenerate to hide a regression.
//!
//! One deliberate exception, recorded here rather than by regenerating the file:
//! the Mark IIC+ voice was corrected on 2026-09-15 to include the lead return,
//! V2B, the Lead Master and V2A, which both of its drawings have and the frozen
//! model left out (`docs/models/cali_iic_plus.md`). Its blocks are no longer
//! compared; the 5150, Twin and 73P blocks still are, sample for sample.
use gainstagefx::voice::{Chain, Gain, Settings, Tone};
use std::f64::consts::TAU;

/// Samples per voice block in `render`: four amplitudes, five signals, 2048 each.
const BLOCK: usize = 4 * 5 * 2048;
/// The voices `render` walks, in order, at each rate.
const VOICES: [Gain; 4] = [Gain::Boogie, Gain::Peavey, Gain::Twin, Gain::Neve];

fn render() -> Vec<f32> {
    gainstagefx::dsp::time::enable_ftz_daz();
    let mut samples = Vec::new();
    for rate in [44100.0, 48000.0, 88200.0, 96000.0, 192000.0] {
        let mut chain = Chain::new(rate);
        for gain in VOICES {
            chain.apply(&Settings {
                gain,
                tone: Tone::Off,
                drive: 0.75,
                ..Settings::default()
            });
            chain.settle();
            for amplitude in [0.001, 0.03, 0.126, 0.7] {
                for signal in 0..5 {
                    chain.reset();
                    chain.find_operating_point();
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
                }
            }
        }
    }
    samples
}

#[test]
fn legacy_matched_output_is_preserved() {
    let actual = render();
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/legacy-matched.f32le"
    ))
    .unwrap();
    assert_eq!(bytes.len(), actual.len() * 4);
    let mut error = 0.0_f64;
    let mut energy = 0.0_f64;
    for (index, (&actual, bytes)) in actual.iter().zip(bytes.chunks_exact(4)).enumerate() {
        if VOICES[(index / BLOCK) % VOICES.len()] == Gain::Boogie {
            continue;
        }
        let expected = f32::from_le_bytes(bytes.try_into().unwrap());
        error += f64::from(actual - expected).powi(2);
        energy += f64::from(expected).powi(2);
        assert!(
            (actual - expected).abs() <= 2e-6 * (1.0 + expected.abs()),
            "{actual} != {expected}"
        );
    }
    eprintln!(
        "legacy relative RMS error: {}",
        (error / energy.max(1e-30)).sqrt()
    );
}

#[test]
#[ignore = "Run only against the pre-refactor source; overwrites the reference"]
fn capture_pre_refactor_baseline() {
    let bytes: Vec<u8> = render().iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/legacy-matched.f32le"
        ),
        bytes,
    )
    .unwrap();
}
