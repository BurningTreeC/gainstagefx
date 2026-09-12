//! Copying committed history must be equivalent to uninterrupted processing.
#[path = "support/allocations.rs"]
mod allocations;

use allocations::assert_no_heap;
use gainstagefx::dsp::device::{AnyDevice, Core, Device, Stamper};
use gainstagefx::dsp::netlist::CoreSpec;
use gainstagefx::dsp::oversample::Oversampler;
use gainstagefx::dsp::spring::Tank;
use gainstagefx::dsp::time::{enable_ftz_daz, Simulation};
use gainstagefx::dsp::tremolo::Tremolo;
use gainstagefx::voice::{self, Cabinet, Chain, Gain, Iron, Settings, Tone};

const RATE: f64 = 48_000.0;

fn input(k: usize) -> f64 {
    0.12 * (k as f64 * 0.057).sin() + 0.04 * (k as f64 * 0.213).sin()
}

#[test]
fn core_copy_includes_committed_flux_and_previous_voltage() {
    let mut source = AnyDevice::Core(Core::new(0, 1, CoreSpec::STEEL, RATE));
    let mut copied = AnyDevice::Core(Core::new(0, 1, CoreSpec::STEEL, RATE));
    let stamp = |device: &mut AnyDevice, volts| {
        let mut matrix = [0.0; 4];
        let mut rhs = [0.0; 2];
        device.stamp(
            &mut Stamper {
                matrix: &mut matrix,
                rhs: &mut rhs,
                n: 2,
                limiting: true,
                junction_held: false,
            },
            &[volts, 0.0],
        );
        device.advance();
        (matrix, rhs)
    };
    for _ in 0..256 {
        stamp(&mut source, 0.4);
    }
    let AnyDevice::Core(core) = &source else {
        unreachable!()
    };
    assert!(core.flux().abs() > 1e-5);
    assert_no_heap(|| copied.copy_runtime_state_from(&source));
    for k in 0..256 {
        assert_eq!(stamp(&mut source, input(k)), stamp(&mut copied, input(k)));
    }
}

#[test]
fn simulation_copy_preserves_reactive_histories_and_refreshed_linear_cache() {
    enable_ftz_daz();
    let net = gainstagefx::circuits::markiic::graphic(voice::SOURCE, voice::LOAD).unwrap();
    let mut source = Simulation::new(net.clone(), RATE);
    let mut copied = Simulation::new(net, RATE);
    for k in 0..2048 {
        if k % 64 == 0 {
            let position = 0.4 + 0.2 * (k as f64 * 0.001).sin();
            source.set_control(0, position);
            copied.set_control(0, position);
        }
        source.process(input(k));
    }
    let counters = copied.statistics();
    assert_no_heap(|| copied.copy_runtime_state_from(&source));
    assert_eq!(copied.statistics(), counters);
    assert!(!copied.needs_operating_point());
    assert_no_heap(|| {
        for k in 2048..4096 {
            assert_eq!(source.process(input(k)), copied.process(input(k)));
        }
    });
    assert_eq!(
        copied.statistics().3,
        counters.3,
        "wake-up rebuilt a circuit"
    );
}

#[test]
fn all_nonlinear_variants_continue_from_copied_solver_and_device_state() {
    enable_ftz_daz();
    for i in 0..voice::VOICES {
        let (gain, diode, amplifier) = voice::voice_at(i);
        let net = voice::build_voice(gain, diode, amplifier).unwrap();
        let mut source = Simulation::new(net.clone(), RATE);
        let mut copied = Simulation::new(net, RATE);
        source.set_control(gain.drive_control(), 0.8);
        copied.set_control(gain.drive_control(), 0.8);
        for k in 0..1024 {
            source.process(input(k));
        }
        let counters = copied.statistics();
        assert_no_heap(|| copied.copy_runtime_state_from(&source));
        assert_eq!(copied.statistics(), counters);
        assert_no_heap(|| {
            for k in 1024..1280 {
                let expected = source.process(input(k));
                let actual = copied.process(input(k));
                assert!(actual.is_finite());
                assert_eq!(actual, expected, "{}", gain.name());
            }
        });
        assert_eq!(copied.statistics().3, counters.3);
    }
}

#[test]
fn oversampler_copy_preserves_all_fir_and_delay_histories() {
    for factor in [1, 2, 4, 8] {
        let mut source = Oversampler::new(factor);
        let mut copied = Oversampler::new(4);
        for k in 0..1024 {
            source.process(input(k), &mut f64::tanh);
        }
        assert_no_heap(|| copied.copy_runtime_state_from(&source));
        assert_eq!(copied.factor(), factor);
        assert_no_heap(|| {
            for k in 1024..2048 {
                assert_eq!(
                    source.process(input(k), &mut f64::tanh),
                    copied.process(input(k), &mut f64::tanh)
                );
            }
        });
    }
}

#[test]
fn spring_copy_preserves_a_nonzero_decay_tail() {
    let mut source = Tank::accutronics(RATE);
    let mut copied = Tank::accutronics(RATE);
    for k in 0..4096 {
        source.process(if k < 512 { input(k) } else { 0.0 });
    }
    assert_no_heap(|| copied.copy_runtime_state_from(&source));
    let mut energy = 0.0;
    assert_no_heap(|| {
        for _ in 0..4096 {
            let expected = source.process(0.0);
            assert_eq!(copied.process(0.0), expected);
            energy += expected * expected;
        }
    });
    assert!(energy > 1e-10, "test must exercise an audible tail");
}

#[test]
fn tremolo_copy_preserves_phase_hysteresis_and_illumination() {
    let mut source = Tremolo::new(RATE);
    let mut copied = Tremolo::new(RATE);
    for _ in 0..2345 {
        source.resistance(0.7, 0.9);
    }
    assert_no_heap(|| copied.copy_runtime_state_from(&source));
    for _ in 0..4096 {
        assert_eq!(source.resistance(0.7, 0.9), copied.resistance(0.7, 0.9));
    }
}

#[test]
fn chain_copy_preserves_active_sections_delays_and_disabled_twin_tail() {
    enable_ftz_daz();
    for gain in [Gain::Boogie, Gain::Peavey, Gain::Twin, Gain::Crunch] {
        let mut source = Chain::new(RATE);
        let mut copied = Chain::new(RATE);
        let mut settings = Settings {
            gain,
            drive: 0.7,
            iron: Iron::Steel,
            tone: Tone::Scooping,
            cabinet: Cabinet::Stack,
            oversampling: 8,
            reverb: 0.7,
            speed: 0.8,
            intensity: 0.9,
            ..Settings::default()
        };
        for k in 0..4096 {
            if k % 64 == 0 {
                settings.drive += 0.0001;
                if k == 3584 {
                    settings.reverb = 0.0;
                    settings.intensity = 0.0;
                }
                source.apply(&settings);
                copied.apply(&settings);
            }
            source.delayed_dry(input(k));
            source.process(input(k));
        }
        let counters = copied.solver_health();
        assert_no_heap(|| copied.copy_runtime_state_from(&source));
        assert_eq!(copied.solver_health(), counters);
        assert!(!copied.needs_operating_point());
        settings.reverb = 0.7;
        settings.intensity = 0.9;
        assert_no_heap(|| {
            source.apply(&settings);
            copied.apply(&settings);
            for k in 4096..6144 {
                assert_eq!(source.delayed_dry(input(k)), copied.delayed_dry(input(k)));
                assert_eq!(
                    source.process(input(k)),
                    copied.process(input(k)),
                    "{}",
                    gain.name()
                );
            }
        });
    }
}
