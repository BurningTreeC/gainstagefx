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
    for gain in Gain::ALL {
        assert_eq!(
            PowerAmp::Matched.resolved(gain).is_some(),
            gain.power_stage().is_some()
        );
        assert_eq!(PowerAmp::Bypass.resolved(gain), None);
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

#[test]
fn studio_power_bypass_is_an_exact_noop() {
    let base = Settings {
        gain: Gain::Neve,
        tone: Tone::Off,
        ..Settings::default()
    };
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
