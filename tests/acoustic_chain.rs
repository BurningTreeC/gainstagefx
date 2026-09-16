//! The physical speaker / cabinet / microphone path inside the full chain: legacy
//! preservation, routing, interaction, state, rates, blocks and switching.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::mic::{MicPlacement, MicProfile};
use gainstagefx::acoustics::speaker::SpeakerProfile;
use gainstagefx::acoustics::stage::MicSlot;
use gainstagefx::voice::{
    AcousticSettings, CabinetChoice, Cabinet, Chain, Gain, PowerAmp, Settings, SpeakerChoice, Tone,
    NOMINAL_DBFS,
};

fn signal(k: usize) -> f64 {
    0.05 * (k as f64 * 0.0571).sin() + 0.02 * (k as f64 * 0.311).sin()
}

fn physical(cab: &'static CabinetProfile) -> AcousticSettings {
    AcousticSettings {
        cabinet: CabinetChoice::Model(cab),
        speaker: SpeakerChoice::Matched,
        mic_a: MicSlot::Profile(&MicProfile::DYNAMIC_57),
        place_a: MicPlacement { position: 0.3, distance: 0.025, angle: 10.0 },
        ..AcousticSettings::default()
    }
}

fn render(chain: &mut Chain, n: usize) -> Vec<f64> {
    (0..n).map(|k| chain.process(signal(k))).collect()
}

#[test]
fn legacy_cabinet_ignores_every_new_control_bit_for_bit() {
    let base = Settings {
        gain: Gain::Boogie,
        cabinet: Cabinet::Stack,
        tone: Tone::Off,
        ..Settings::default()
    };
    let mut a = Chain::new(48_000.0);
    let mut b = Chain::new(48_000.0);
    a.apply(&base);
    b.apply(&Settings {
        acoustic: AcousticSettings {
            cabinet: CabinetChoice::Legacy,
            speaker: SpeakerChoice::Model(&SpeakerProfile::AMERICAN_ALNICO),
            mic_a: MicSlot::Profile(&MicProfile::RIBBON_38),
            mic_b: MicSlot::Profile(&MicProfile::CONDENSER_87),
            blend: 0.9,
            invert_b: true,
            ..AcousticSettings::default()
        },
        ..base
    });
    a.settle();
    b.settle();
    for k in 0..6_000 {
        assert_eq!(a.process(signal(k)), b.process(signal(k)));
    }
    assert!(!b.is_radiating());
}

#[test]
fn speaker_bypass_is_a_di_of_the_resistor_loaded_power_stage() {
    // Cabinet model selected but speaker bypassed: the legacy cabinet filter must not
    // apply, and the output must equal the legacy path with its cabinet off.
    let base = Settings {
        gain: Gain::Peavey,
        tone: Tone::Off,
        ..Settings::default()
    };
    let mut di = Chain::new(48_000.0);
    let mut legacy_off = Chain::new(48_000.0);
    di.apply(&Settings {
        cabinet: Cabinet::Stack,
        acoustic: AcousticSettings {
            speaker: SpeakerChoice::Bypass,
            ..physical(&CabinetProfile::BRIT_1960)
        },
        ..base
    });
    legacy_off.apply(&base);
    di.settle();
    legacy_off.settle();
    for k in 0..6_000 {
        assert_eq!(di.process(signal(k)), legacy_off.process(signal(k)));
    }
}

#[test]
fn physical_path_is_finite_level_matched_and_differs_by_driver() {
    let levels = |acoustic: AcousticSettings| {
        let mut chain = Chain::new(48_000.0);
        chain.apply(&Settings {
            gain: Gain::Boogie,
            tone: Tone::Off,
            cabinet: Cabinet::Stack,
            acoustic,
            ..Settings::default()
        });
        chain.settle();
        chain.find_operating_point();
        let out = render(&mut chain, 24_000);
        let rms = (out[12_000..].iter().map(|y| y * y).sum::<f64>() / 12_000.0).sqrt();
        assert!(out.iter().all(|y| y.is_finite()));
        let work = chain.solver_breakdown();
        assert_eq!(work.power.unsettled, 0);
        (rms, out)
    };
    let (legacy, _) = levels(AcousticSettings::default());
    let (v30, a) = levels(physical(&CabinetProfile::BRIT_V30));
    let (jensen, b) = levels(AcousticSettings {
        speaker: SpeakerChoice::Model(&SpeakerProfile::AMERICAN_VINTAGE_12),
        ..physical(&CabinetProfile::BRIT_V30)
    });
    let ratio_db = 20.0 * (v30 / legacy).log10();
    assert!(ratio_db.abs() < 12.0, "physical vs legacy Stack: {ratio_db} dB");
    let difference: f64 = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum::<f64>() / a.len() as f64;
    assert!(difference > 1e-3 * v30, "driver changes the sound: {difference} vs {jensen}");
}

#[test]
fn no_power_stage_drives_the_speaker_directly() {
    let mut chain = Chain::new(48_000.0);
    for (gain, power_amp) in [(Gain::Screamer, PowerAmp::Matched), (Gain::Boogie, PowerAmp::Bypass)] {
        chain.apply(&Settings {
            gain,
            power_amp,
            tone: Tone::Off,
            acoustic: physical(&CabinetProfile::AMERICAN_OPEN_212),
            ..Settings::default()
        });
        chain.find_operating_point();
        let before = chain.solver_breakdown();
        assert_no_heap(|| {
            for k in 0..4_096 {
                assert!(chain.process(signal(k)).is_finite());
            }
        });
        let work = chain.solver_breakdown().saturating_delta(before);
        assert!(chain.is_radiating());
        // The voltage-driven speaker is reported as the output stage's work.
        assert!(work.power.solves > 0, "{gain:?}");
    }
}

#[test]
fn every_power_model_drives_every_cabinet_without_blowing_up() {
    let mut chain = Chain::new(48_000.0);
    for power_amp in [
        PowerAmp::Cali6L6,
        PowerAmp::American6L6Clean,
        PowerAmp::American6L6HighGain,
        PowerAmp::BritEL34,
    ] {
        for cab in CabinetProfile::ALL {
            chain.apply(&Settings {
                gain: Gain::HighGain,
                power_amp,
                drive: 0.9,
                master: 0.8,
                tone: Tone::Off,
                acoustic: physical(cab),
                ..Settings::default()
            });
            chain.find_operating_point();
            let mut peak: f64 = 0.0;
            for k in 0..3_000 {
                let y = chain.process(4.0 * signal(k));
                assert!(y.is_finite(), "{power_amp:?} {}", cab.id);
                peak = peak.max(y.abs());
            }
            assert!(peak < 50.0, "{power_amp:?} {}: {peak}", cab.id);
        }
    }
}

#[test]
fn dormant_channel_wakes_identically_and_then_diverges_independently() {
    let s = Settings {
        gain: Gain::Twin,
        tone: Tone::Off,
        acoustic: AcousticSettings {
            mic_b: MicSlot::Profile(&MicProfile::RIBBON_121),
            place_b: MicPlacement { position: 0.8, distance: 0.3, angle: 30.0 },
            ..physical(&CabinetProfile::AMERICAN_OPEN_212)
        },
        ..Settings::default()
    };
    let mut left = Chain::new(48_000.0);
    let mut right = Chain::new(48_000.0);
    left.apply(&s);
    right.apply(&s);
    left.settle();
    right.settle();
    left.find_operating_point();
    right.find_operating_point();
    for k in 0..4_096 {
        left.process(signal(k));
    }
    assert_no_heap(|| {
        right.copy_runtime_state_from(&left);
        for k in 4_096..6_000 {
            assert_eq!(left.process(signal(k)), right.process(signal(k)));
        }
        for k in 6_000..7_000 {
            let l = left.process(signal(k));
            let r = right.process(-signal(k));
            assert!(l.is_finite() && r.is_finite());
        }
    });
}

#[test]
fn live_switching_and_automation_is_bounded_and_allocation_free() {
    let mut chain = Chain::new(48_000.0);
    let cabs = [
        CabinetChoice::Legacy,
        CabinetChoice::Bypass,
        CabinetChoice::Model(&CabinetProfile::BRIT_GREEN),
        CabinetChoice::Model(&CabinetProfile::AMERICAN_OPEN_112),
    ];
    let speakers = [
        SpeakerChoice::Matched,
        SpeakerChoice::Bypass,
        SpeakerChoice::Model(&SpeakerProfile::BRIT_T75),
    ];
    let mics = [
        MicSlot::Ideal,
        MicSlot::Profile(&MicProfile::DYNAMIC_421),
        MicSlot::Profile(&MicProfile::FET_CONDENSER_47),
    ];
    let mut k = 0usize;
    chain.apply(&Settings { gain: Gain::Boogie, ..Settings::default() });
    chain.find_operating_point();
    assert_no_heap(|| {
        for block in 0..240 {
            let acoustic = AcousticSettings {
                cabinet: cabs[block / 20 % cabs.len()],
                speaker: speakers[block / 30 % speakers.len()],
                mic_a: mics[block / 15 % mics.len()],
                mic_b: if block % 40 < 20 { MicSlot::Off } else { mics[block % 3] },
                place_a: MicPlacement {
                    position: (block as f64 * 0.13).fract(),
                    distance: 0.01 + (block as f64 * 0.071).fract() * 0.9,
                    angle: (block as f64 * 7.0) % 90.0,
                },
                blend: (block as f64 * 0.37).fract(),
                invert_b: block % 11 == 0,
                align: block % 13 == 0,
                ..AcousticSettings::default()
            };
            chain.apply(&Settings {
                gain: Gain::Boogie,
                power_amp: if block % 50 < 25 { PowerAmp::Matched } else { PowerAmp::BritEL34 },
                acoustic,
                ..Settings::default()
            });
            if chain.needs_operating_point() {
                chain.find_operating_point();
            }
            for _ in 0..64 {
                let y = chain.process(signal(k));
                assert!(y.is_finite() && y.abs() < 20.0, "block {block}: {y}");
                k += 1;
            }
        }
    });
}

#[test]
fn physical_path_survives_rates_resets_and_block_partitions() {
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        for power_amp in [PowerAmp::Matched, PowerAmp::Bypass, PowerAmp::BritEL34] {
            let s = Settings {
                gain: Gain::Peavey,
                power_amp,
                acoustic: physical(&CabinetProfile::CALI_OVERSIZED),
                ..Settings::default()
            };
            assert_no_heap(|| {
                chain.apply(&s);
                chain.reset();
                chain.find_operating_point();
                for block in [1, 7, 32, 64, 127, 256, 512] {
                    chain.apply(&s);
                    for k in 0..block {
                        assert!(chain.process(signal(k)).is_finite(), "{rate} {power_amp:?}");
                    }
                }
            });
        }
        assert_eq!(chain.latency(), 66);
    }
}

/// What the cabinet row's **Bypass** is, and what it is not.
///
/// Bypass there means *no box*: the driver still radiates, on an open baffle,
/// and a microphone still picks it up. It is not a way past the acoustic path
/// -- that is the **speaker** row's own Bypass, which is the power stage's
/// terminal voltage into a resistor, a DI.
///
/// Reported as the two sounding different when they were expected to be the
/// same thing. They are not the same thing, and the way to say so precisely is
/// that the DI *is* bit-identical to the legacy resistor load with its filter
/// off, while the open baffle is a different sound entirely.
#[test]
fn cabinet_bypass_is_no_box_and_speaker_bypass_is_the_di() {
    let render = |cabinet, speaker| {
        let mut chain = Chain::new(48_000.0);
        chain.apply(&Settings {
            gain: Gain::Brit800,
            drive: 0.6,
            tone: Tone::Off,
            cabinet: Cabinet::Off,
            acoustic: AcousticSettings {
                cabinet,
                speaker,
                mic_a: MicSlot::Profile(&MicProfile::DYNAMIC_57),
                ..AcousticSettings::default()
            },
            ..Settings::default()
        });
        chain.settle();
        chain.find_operating_point();
        let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
        let out: Vec<f64> = (0..12_000)
            .map(|k| {
                let t = k as f64 / 48_000.0;
                chain.process(
                    amplitude
                        * ((std::f64::consts::TAU * 110.0 * t).sin() * 0.7
                            + (std::f64::consts::TAU * 330.0 * t).sin() * 0.3),
                )
            })
            .collect();
        (out, chain.is_radiating())
    };

    let (legacy, legacy_radiating) = render(CabinetChoice::Legacy, SpeakerChoice::Matched);
    let (di, di_radiating) = render(CabinetChoice::Bypass, SpeakerChoice::Bypass);
    let (baffle, baffle_radiating) = render(CabinetChoice::Bypass, SpeakerChoice::Matched);

    assert!(!legacy_radiating && !di_radiating, "neither drives a speaker");
    assert_eq!(legacy, di, "the speaker row's Bypass is the legacy resistor load exactly");

    assert!(baffle_radiating, "cabinet Bypass still radiates: it is a driver with no box");
    let rms = |x: &[f64]| (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
    let difference = rms(
        &legacy
            .iter()
            .zip(&baffle)
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(difference > 0.2 * rms(&legacy), "an open baffle is a sound, not a bypass: {difference}");
}
