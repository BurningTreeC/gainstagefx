//! The core's Newton tangent and limiter must follow its existing flux curve.
use gainstagefx::dsp::device::{Core, Device, Stamper};
use gainstagefx::dsp::netlist::{CoreSpec, GROUND};

fn stamp(core: &mut Core, volts: f64, limiting: bool) -> (f64, f64) {
    let mut matrix = [0.0];
    let mut rhs = [0.0];
    core.stamp(
        &mut Stamper {
            matrix: &mut matrix,
            rhs: &mut rhs,
            n: 1,
            limiting,
            junction_held: false,
        },
        &[volts],
    );
    (matrix[0], rhs[0])
}

#[test]
fn stamped_core_tangent_matches_the_current_curve() {
    for spec in [CoreSpec::STEEL, CoreSpec::NICKEL, CoreSpec::AMORPHOUS] {
        for rate in [44_100.0, 48_000.0, 96_000.0] {
            for multiple in [-2.0, -1.0, -0.1, 0.0, 0.1, 1.0, 2.0] {
                let mut core = Core::new(0, GROUND, spec, rate);
                let flux = spec.knee * multiple;
                let volts = flux * 2.0 * rate;
                let (g, rhs) = stamp(&mut core, volts, false);
                let h = spec.knee * 1e-6;
                let slope = (core.magnetising(flux + h) - core.magnetising(flux - h)) / (2.0 * h);
                let expected = slope / (2.0 * rate);
                assert!((g - expected).abs() < expected.abs() * 1e-8);
                let current = core.magnetising(flux);
                assert!((g * volts - rhs - current).abs() < 1e-12 * (1.0 + current.abs()));
            }
        }
    }
}

#[test]
fn core_limiter_bounds_newton_flux_steps_without_clipping_the_answer() {
    let spec = CoreSpec::STEEL;
    for rate in [48_000.0, 96_000.0] {
        let mut core = Core::new(0, GROUND, spec, rate);
        // More than eight volts, but well inside the nearly linear region.
        let small_flux = spec.knee / 100.0;
        let small_volts = small_flux * 2.0 * rate;
        assert!(small_volts > 8.0);
        stamp(&mut core, small_volts, true);
        assert!(core.settled(1e-6));
        assert!((core.flux() - small_flux).abs() < 1e-15);

        let mut core = Core::new(0, GROUND, spec, rate);
        let step = spec.knee / spec.sharpness;
        let wanted = 4.0 * step;
        let volts = wanted * 2.0 * rate;
        stamp(&mut core, volts, true);
        assert!(!core.settled(1e-6));
        assert!((core.flux() - step).abs() < 1e-15);
        for _ in 0..6 {
            stamp(&mut core, volts, true);
        }
        assert!(core.settled(1e-6));
        assert!((core.flux() - wanted).abs() < 1e-15);
    }
}
