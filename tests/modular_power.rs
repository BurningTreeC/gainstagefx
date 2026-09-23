//! Output selection must route a complete circuit and preserve independent state.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::voice::{Chain, Gain, PowerAmp, Settings, Tone};

fn signal(k: usize) -> f64 {
    0.03 * (k as f64 * 0.083).sin()
}

#[test]
fn matched_resolution_and_bypass_follow_the_preamp() {
    // `Gain::power_stage()` answers a *narrower* question than `Matched` does:
    // it names a valve `PowerSpec`, and two voices have an output block that is
    // not one -- the Jazz 120's complementary transistor amplifier and the
    // 73P's line driver. So the relationship is containment, not equality.
    //
    // It was equality here until 2026-09-23, and that is precisely the split
    // that had those two voices *calibrated* with their output stage by
    // `examples/calibrate` and *played* without it by `Chain`, because the two
    // asked different questions and got different answers. See `build_power`.
    for gain in Gain::ALL {
        if gain.power_stage().is_some() {
            assert!(
                PowerAmp::Matched.resolved(gain).is_some(),
                "{} has a valve power stage but Matched resolves to nothing",
                gain.name()
            );
        }
        assert_eq!(PowerAmp::Bypass.resolved(gain), None);
    }

    // And the two whose output block is not a valve stage resolve anyway,
    // which is the whole point of the correction.
    for gain in [Gain::Jazz120, Gain::Neve] {
        assert!(
            gain.power_stage().is_none(),
            "{} is not a valve power stage",
            gain.name()
        );
        assert!(
            PowerAmp::Matched.resolved(gain).is_some(),
            "{} has an output block and Matched must find it",
            gain.name()
        );
    }
}

#[test]
fn explicit_original_power_is_identical_to_matched() {
    for (gain, power_amp) in [
        (Gain::Boogie, PowerAmp::Cali6L6),
        (Gain::Peavey, PowerAmp::American6L6HighGain),
    ] {
        let base = Settings {
            gain,
            tone: Tone::Off,
            ..Settings::default()
        };
        let mut a = Chain::new(48000.0);
        let mut b = Chain::new(48000.0);
        a.apply(&base);
        b.apply(&Settings { power_amp, ..base });
        a.settle();
        b.settle();
        assert_no_heap(|| {
            for k in 0..4096 {
                assert_eq!(a.process(signal(k)), b.process(signal(k)));
            }
        });
    }
}

#[test]
fn bypass_stops_every_power_solve_but_keeps_the_studio_line_driver() {
    let mut chain = Chain::new(48000.0);
    for gain in [Gain::Boogie, Gain::Peavey, Gain::Twin, Gain::Neve] {
        let s = Settings {
            gain,
            power_amp: PowerAmp::Bypass,
            tone: Tone::Off,
            ..Settings::default()
        };
        assert_no_heap(|| {
            chain.apply(&s);
            chain.find_operating_point();
            let before = chain.solver_breakdown();
            for k in 0..2048 {
                assert!(chain.process(signal(k)).is_finite());
            }
            let work = chain.solver_breakdown().saturating_delta(before);
            assert_eq!(work.power.solves, 0);
            assert_eq!(work.line.solves > 0, gain == Gain::Neve);
            assert!(work.gain.solves > 0);
        });
    }
}

/// Bypassing an output stage a voice has not got must be exactly nothing.
///
/// The subject used to be `Gain::Neve`, and that stopped being the right one on
/// 2026-09-23: the 73P's OUTPUT block -- two BC109C into a TIP3055 and the
/// VTB1148 -- is now in that voice's path, so bypassing it is a real change and
/// the test below asserts that instead. A microphone preamplifier with no
/// output block of any kind is what this one was always about.
#[test]
fn studio_power_bypass_is_an_exact_noop() {
    let base = Settings {
        gain: Gain::Studio,
        tone: Tone::Off,
        ..Settings::default()
    };
    assert!(
        PowerAmp::Matched.resolved(Gain::Studio).is_none(),
        "this test needs a voice with no output block"
    );
    let mut a = Chain::new(48000.0);
    let mut b = Chain::new(48000.0);
    a.apply(&base);
    b.apply(&Settings {
        power_amp: PowerAmp::Bypass,
        ..base
    });
    a.settle();
    b.settle();
    for k in 0..4096 {
        assert_eq!(a.process(signal(k)), b.process(signal(k)));
    }
}

/// And bypassing one a voice *has* must be audible, which is the other half of
/// the same statement. The 73P is the case that was wrong: its output block was
/// in the calibration and not in the path, so switching it out did nothing at
/// all while the make-up still took out a gain that was never applied.
#[test]
fn bypassing_an_output_block_a_voice_has_changes_the_sound() {
    for gain in [Gain::Neve, Gain::Jazz120] {
        let base = Settings {
            gain,
            tone: Tone::Off,
            ..Settings::default()
        };
        let mut matched = Chain::new(48000.0);
        let mut bypassed = Chain::new(48000.0);
        matched.apply(&base);
        bypassed.apply(&Settings {
            power_amp: PowerAmp::Bypass,
            ..base
        });
        matched.settle();
        bypassed.settle();
        matched.find_operating_point();
        bypassed.find_operating_point();
        let mut difference = 0.0f64;
        for k in 0..4096 {
            let x = signal(k);
            difference = difference.max((matched.process(x) - bypassed.process(x)).abs());
        }
        assert!(
            difference > 1e-6,
            "{} sounds the same with its output block bypassed, so it is not \
             in the path",
            gain.name()
        );
    }
}

#[test]
fn custom_power_changes_audio_and_survives_channel_wake_without_heap_work() {
    let base = Settings {
        gain: Gain::Boogie,
        tone: Tone::Off,
        ..Settings::default()
    };
    let mut matched = Chain::new(48000.0);
    matched.apply(&base);
    matched.settle();
    let reference: Vec<_> = (0..4096).map(|k| matched.process(signal(k))).collect();
    let mut left = Chain::new(48000.0);
    let mut right = Chain::new(48000.0);
    for power_amp in PowerAmp::ALL {
        let s = Settings { power_amp, ..base };
        left.apply(&s);
        right.apply(&s);
        left.settle();
        right.settle();
        let mut difference = 0.0;
        assert_no_heap(|| {
            for (k, expected) in reference.iter().enumerate() {
                difference += (left.process(signal(k)) - expected).abs();
            }
            right.copy_runtime_state_from(&left);
            for k in 4096..4352 {
                assert_eq!(left.process(signal(k)), right.process(signal(k)));
            }
            // Changing the right channel must not alter the left channel state.
            for k in 4352..4608 {
                let l = left.process(signal(k));
                let r = right.process(-signal(k));
                assert!(l.is_finite() && r.is_finite());
            }
        });
        if power_amp == PowerAmp::American6L6Clean {
            assert!(difference > 0.01);
        }
    }
}

#[test]
fn custom_power_rates_resets_and_arbitrary_block_partitions_are_stable() {
    let mut chain = Chain::new(44100.0);
    for rate in [44100.0, 48000.0, 88200.0, 96000.0, 192000.0] {
        chain.set_rate(rate);
        for power_amp in PowerAmp::ALL {
            let s = Settings {
                gain: Gain::Peavey,
                power_amp,
                ..Settings::default()
            };
            assert_no_heap(|| {
                chain.apply(&s);
                chain.reset();
                chain.find_operating_point();
                for block in [1, 7, 32, 64, 127, 256, 512] {
                    chain.apply(&s);
                    for k in 0..block {
                        assert!(chain.process(signal(k)).is_finite());
                    }
                }
            });
        }
        assert_eq!(chain.latency(), 66);
    }
}

#[test]
fn a_power_stage_released_by_an_override_returns_to_its_own_master_position() {
    // Reported: American Twin on Matched power came out as nearly all reverb after the
    // Twin power stage had been used behind another preamplifier.
    let twin = Settings {
        gain: Gain::Twin,
        tone: Tone::Off,
        ..Settings::default()
    };
    let level = |chain: &mut Chain| {
        chain.find_operating_point();
        let mut sum = 0.0;
        for k in 0..24_000 {
            let y = chain.process(signal(k));
            if k >= 12_000 {
                sum += y * y;
            }
        }
        sum.sqrt()
    };
    let mut fresh = Chain::new(48000.0);
    fresh.apply(&twin);
    fresh.settle();
    let reference = level(&mut fresh);
    let mut chain = Chain::new(48000.0);
    for voice in [Gain::Boogie, Gain::Screamer, Gain::Peavey] {
        chain.apply(&Settings {
            gain: voice,
            power_amp: PowerAmp::American6L6Clean,
            ..twin
        });
        level(&mut chain);
        chain.apply(&twin);
        let back = level(&mut chain);
        let db = 20.0 * (back / reference).log10();
        assert!(db.abs() < 0.5, "after {voice:?}: {db:+.2} dB");
    }
}
