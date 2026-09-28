//! Absolute pressure and phase checked against independent acoustic solutions.
use gainstagefx::acoustics::filters::DelayLine;
use gainstagefx::acoustics::radiation::PressureField;
use gainstagefx::acoustics::speaker::{RHO, SPEED_OF_SOUND};
use gainstagefx::dsp::complex::C;
use std::f64::consts::TAU;

fn response(rate: f64, radius: f64, at: [f64; 3], hz: f64) -> C {
    let reference = PressureField::first_arrival(radius, at);
    let mut field = PressureField::default();
    field.add_piston(
        rate,
        radius,
        at,
        [0.0, 0.0, -1.0],
        (1.0, 0.0),
        0.0,
        reference,
    );
    field.commit(true);
    let mut acceleration = DelayLine::new();
    let velocity = DelayLine::new();
    let mut result = C::ZERO;
    for n in 0..2048 {
        acceleration.write(if n == 0 { 1.0 } else { 0.0 });
        let phase = -TAU * hz * n as f64 / rate;
        result = result
            + C::new(phase.cos(), phase.sin()) * C::real(field.process(&acceleration, &velocity));
    }
    result
}

#[test]
fn finite_piston_pressure_and_phase_match_the_closed_form_on_axis() {
    let radius: f64 = 0.125;
    for rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
        for z in [0.01, 0.05, 0.3, 1.0] {
            for hz in [80.0, 700.0, 2000.0, 5000.0] {
                let k = TAU * hz / SPEED_OF_SOUND;
                let spread = (radius * radius + z * z).sqrt() - z;
                let expected = C::real(RHO)
                    * (C::real(1.0) - C::new((k * spread).cos(), -(k * spread).sin()))
                    / C::new(0.0, k);
                let actual = response(rate, radius, [0.0, 0.0, z], hz);
                let error = (actual - expected).magnitude() / (RHO * spread);
                assert!(error < 0.003, "{rate} Hz, {z} m, {hz} Hz: error {error}");
            }
        }
    }
}

#[test]
fn far_field_has_the_first_bessel_null_and_signed_side_lobe() {
    // J1's independently tabulated first root, rather than the implementation's
    // own Bessel approximation, determines the null frequency.
    let radius: f64 = 0.125;
    let angle = 35f64.to_radians();
    let r = 10.0;
    let at = [r * angle.sin(), 0.0, r * angle.cos()];
    let hz = 3.831_705_970_208 * SPEED_OF_SOUND / (TAU * radius * angle.sin());
    let off = response(192_000.0, radius, at, hz);
    let on = response(192_000.0, radius, [0.0, 0.0, r], hz);
    assert!(off.magnitude() / on.magnitude() < 0.015);
    let hz = hz * 1.34;
    let near = PressureField::first_arrival(radius, at);
    let phase = TAU * hz * (r - near) / SPEED_OF_SOUND;
    let lobe = response(192_000.0, radius, at, hz) * C::new(phase.cos(), phase.sin());
    assert!(lobe.re < 0.0, "the side lobe reverses pressure polarity");
}

#[test]
fn far_field_pressure_halves_when_distance_doubles_without_level_makeup() {
    let a = response(96_000.0, 0.125, [0.0, 0.0, 2.0], 100.0).magnitude();
    let b = response(96_000.0, 0.125, [0.0, 0.0, 4.0], 100.0).magnitude();
    assert!((a / b - 2.0).abs() < 0.005);
}

#[test]
fn quasistatic_pressure_at_the_rim_is_two_over_pi_of_the_centre() {
    // Independent surface-potential result at z -> 0: centre integral 2*pi*a,
    // rim integral 4*a. This LF placement change exists even for an omni.
    let a = 0.125;
    let centre = response(192_000.0, a, [0.0, 0.0, 0.00001], 1.0).magnitude();
    let edge = response(192_000.0, a, [a, 0.0, 0.00001], 1.0).magnitude();
    assert!((edge / centre - 2.0 / std::f64::consts::PI).abs() < 0.002);
}

#[test]
fn array_interference_uses_the_other_cones_distance_and_phase() {
    let rate = 192_000.0;
    let at: [f64; 3] = [15.0, 0.0, 25.980_762_113_532];
    let separation: f64 = 0.30;
    let a = [at[0] - separation / 2.0, at[1], at[2]];
    let b = [at[0] + separation / 2.0, at[1], at[2]];
    let length = |v: [f64; 3]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
    let hz = SPEED_OF_SOUND / (2.0 * (length(b) - length(a)));
    let mut field = PressureField::default();
    let reference = PressureField::first_arrival(0.125, a);
    for receiver in [a, b] {
        field.add_piston(
            rate,
            0.125,
            receiver,
            [0.0, 0.0, -1.0],
            (1.0, 0.0),
            0.0,
            reference,
        );
    }
    field.commit(true);
    let mut acceleration = DelayLine::new();
    let velocity = DelayLine::new();
    let mut pressure = C::ZERO;
    for n in 0..2048 {
        acceleration.write(if n == 0 { 1.0 } else { 0.0 });
        let phase = -TAU * hz * n as f64 / rate;
        pressure = pressure
            + C::new(phase.cos(), phase.sin()) * C::real(field.process(&acceleration, &velocity));
    }
    let single = response(rate, 0.125, a, hz).magnitude();
    assert!(
        pressure.magnitude() < 0.025 * single,
        "two cones should cancel at their half-wavelength path difference: ratio {}",
        pressure.magnitude() / single
    );
}

#[test]
fn diffraction_routes_cover_all_faces_and_preserve_mirror_symmetry() {
    use gainstagefx::acoustics::{cabinet::CabinetProfile, diffraction::paths};
    let cab = &CabinetProfile::CLOSED_112;
    // Front, rear, side and rear oblique observation points all receive the
    // appropriate direct/diffracted field, without a nearest-edge switch.
    for at in [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, -0.15],
        [0.8, 0.5, -0.8],
        [-0.8, -0.5, 0.8],
    ] {
        for source in 0..7 {
            let rays = paths(cab, source, at);
            assert!((rays.iter().map(|p| p.weight).sum::<f64>() - 1.0).abs() < 1e-12);
            assert!(rays
                .iter()
                .all(|p| p.weight >= 0.0 && p.distance.is_finite()));
            assert!(rays
                .iter()
                .skip(1)
                .all(|p| p.distance + 1e-12 >= rays[0].distance));
        }
        let left = paths(cab, 3, at);
        let right = paths(cab, 4, [-at[0], at[1], at[2]]);
        let transfer = |rays: [gainstagefx::acoustics::diffraction::Path; 13]| {
            rays.iter()
                .filter(|r| r.weight > 0.0)
                .fold(C::ZERO, |s, r| {
                    let phase = -TAU * 250.0 * r.distance / SPEED_OF_SOUND;
                    s + C::new(phase.cos(), phase.sin()) * C::real(r.weight / r.distance)
                })
        };
        assert!((transfer(left) - transfer(right)).magnitude() < 1e-12);
    }
    let front = paths(cab, 0, [0.0, 0.0, 1.0]);
    assert_eq!(front[0].weight, 0.0, "no ray travels through the box");
    assert_eq!(
        front.iter().filter(|p| p.weight > 0.0).count(),
        4,
        "rear radiation arrives around all four front edges"
    );
}

#[test]
fn opening_and_panel_share_cavity_pressure_and_the_rear_wave_has_opposite_polarity() {
    use gainstagefx::acoustics::cabinet::CabinetProfile;
    use gainstagefx::acoustics::enclosure::{CavityRadiation, Enclosure, RADIATORS};
    use gainstagefx::acoustics::speaker::LoadValues;
    for cab in [
        &CabinetProfile::AMERICAN_OPEN_112,
        &CabinetProfile::CLOSED_112,
    ] {
        let driver = cab.default_speaker;
        let enclosure = Enclosure::new(cab, driver);
        let values = LoadValues::new(driver, &cab.mounting(), 1.0);
        let reference = driver.bl.powi(2) / (cab.drivers as f64 * driver.sd.powi(2));
        let compliance = enclosure.volume / (RHO * SPEED_OF_SOUND.powi(2));
        for hz in [10.0, 70.0, 200.0, 1000.0] {
            let s = C::new(0.0, TAU * hz);
            let opening = if enclosure.opening_area > 0.0 {
                C::real(1.0)
                    / (C::real(reference / values.opening_r)
                        + s * C::real(RHO * enclosure.opening_length / enclosure.opening_area))
            } else {
                C::ZERO
            };
            let panels = enclosure.panels.map(|panel| {
                C::real(panel.count as f64 * panel.area.powi(2) / panel.mass)
                    / (s + C::real(panel.loss * TAU * panel.hz)
                        + C::real((TAU * panel.hz).powi(2)) / s)
            });
            let mobility = panels.iter().fold(opening, |sum, p| sum + *p);
            let pressure = C::real(-(cab.drivers as f64) * driver.sd)
                / (s * C::real(compliance) + C::real(values.rbox / reference) + mobility);
            let expected = [
                opening,
                panels[0],
                panels[1],
                panels[2] * C::real(0.5),
                panels[2] * C::real(0.5),
                panels[3] * C::real(0.5),
                panels[3] * C::real(0.5),
            ]
            .map(|y| pressure * y);
            let mut model = CavityRadiation::default();
            model.configure(48_000.0, cab, driver);
            let mut actual = [C::ZERO; RADIATORS];
            for n in 0..48_000 {
                let phase = TAU * hz * n as f64 / 48_000.0;
                let sample = model.process(phase.cos());
                if n >= 24_000 {
                    for i in 0..RADIATORS {
                        actual[i] = actual[i]
                            + C::new(phase.cos(), -phase.sin()) * C::real(sample[i] / 12_000.0);
                    }
                }
            }
            for i in 0..RADIATORS {
                assert!(
                    (actual[i] - expected[i]).magnitude() < expected[i].magnitude() * 0.005 + 1e-7,
                    "{} {hz} source {i}: {actual:?} vs {expected:?}",
                    cab.id
                );
            }
            if hz == 10.0 && enclosure.opening_area > 0.0 {
                assert!(actual[0].re < -0.95 * cab.drivers as f64 * driver.sd);
            }
        }
    }
}
