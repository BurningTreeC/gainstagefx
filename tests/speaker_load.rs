//! The loudspeaker load: analytic model, netlist stamp, time-domain solve, and its
//! interaction with the power amplifiers. See docs/models/speakers.md.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::acoustics::speaker::{
    self, LoadValues, Mounting, SpeakerProfile, MOTIONAL, RHO, SPEED_OF_SOUND,
};
use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::ac;
use gainstagefx::dsp::netlist::Netlist;
use gainstagefx::dsp::time::Simulation;
use std::f64::consts::TAU;

const SERIES: f64 = 1_000.0;

/// |Z| read back from the netlist: the driver behind a known series resistance.
fn netlist_impedance(values: &LoadValues, hz: f64) -> f64 {
    let mut net = Netlist::new("impedance jig");
    net.input("t", SERIES);
    speaker::stamp(&mut net, "t", values);
    let circuit = net.build("t").unwrap();
    let h = ac::solve(&circuit, &[], hz);
    // H = Z / (Z + Rs)  =>  Z = Rs H / (1 - H)
    let one = gainstagefx::dsp::complex::C::real(1.0);
    (gainstagefx::dsp::complex::C::real(SERIES) * h / (one - h)).magnitude()
}

fn sealed(volume: f64, drivers: usize) -> Mounting {
    Mounting {
        volume_per_driver: Some(volume),
        leakage_q: 7.0,
        drivers,
    }
}

#[test]
fn stamped_netlist_equals_the_analytic_impedance() {
    for profile in SpeakerProfile::ALL {
        for mounting in [Mounting::BAFFLE, sealed(0.034, 4), sealed(0.045, 1)] {
            for scale in [0.5, 1.0] {
                let values = LoadValues::new(profile, &mounting, scale);
                for hz in [20.0, 55.0, 80.0, 113.0, 400.0, 2_000.0, 9_000.0, 19_000.0] {
                    let analytic = values.impedance(hz).magnitude();
                    let stamped = netlist_impedance(&values, hz);
                    assert!(
                        (analytic - stamped).abs() <= 1e-6 * analytic,
                        "{} {mounting:?} {hz} Hz: {analytic} vs {stamped}",
                        profile.id
                    );
                }
            }
        }
    }
}

/// Points read from Jensen's published 8 ohm impedance charts (jensentone.com).
#[test]
fn free_air_impedance_tracks_the_manufacturer_curves() {
    let cases: [(&SpeakerProfile, [(f64, f64); 4], (f64, f64)); 3] = [
        (
            &SpeakerProfile::AMERICAN_VINTAGE_12,
            [(1_000.0, 8.9), (5_000.0, 15.9), (10_000.0, 22.8), (300.0, 7.0)],
            (79.6, 38.2),
        ),
        (
            &SpeakerProfile::AMERICAN_CERAMIC,
            [(1_000.0, 12.3), (5_000.0, 22.1), (10_000.0, 32.8), (300.0, 7.7)],
            (106.2, 42.4),
        ),
        (
            &SpeakerProfile::AMERICAN_ALNICO,
            [(1_000.0, 10.5), (5_000.0, 20.5), (10_000.0, 30.2), (300.0, 7.2)],
            (89.3, 33.9),
        ),
    ];
    for (profile, points, (peak_hz, peak_ohms)) in cases {
        let v = LoadValues::new(profile, &Mounting::BAFFLE, 1.0);
        for (hz, measured) in points {
            let model = v.impedance(hz).magnitude();
            assert!(
                (model / measured - 1.0).abs() < 0.15,
                "{} at {hz} Hz: model {model:.1} measured {measured}",
                profile.id
            );
        }
        let (mut best_hz, mut best) = (0.0, 0.0);
        let mut hz = 40.0;
        while hz < 250.0 {
            let z = v.impedance(hz).magnitude();
            if z > best {
                best = z;
                best_hz = hz;
            }
            hz *= 1.005;
        }
        assert!((best_hz / peak_hz - 1.0).abs() < 0.10, "{} peak at {best_hz}", profile.id);
        assert!((best / peak_ohms - 1.0).abs() < 0.12, "{} peak {best}", profile.id);
    }
}

#[test]
fn a_sealed_box_raises_resonance_by_its_compliance_ratio() {
    let p = &SpeakerProfile::BRIT_GREEN_25;
    let volume = 0.034;
    let cab = volume / (RHO * SPEED_OF_SOUND * SPEED_OF_SOUND * p.sd * p.sd);
    // One driver, so no shared radiation mass: pure compliance stiffening.
    let expected = p.fs * (1.0 + p.cms() / cab).sqrt();
    let v = LoadValues::new(p, &sealed(volume, 1), 1.0);
    let (mut best_hz, mut best) = (0.0, 0.0);
    let mut hz = 40.0;
    while hz < 300.0 {
        let z = v.impedance(hz).magnitude();
        if z > best {
            best = z;
            best_hz = hz;
        }
        hz *= 1.002;
    }
    assert!((best_hz / expected - 1.0).abs() < 0.03, "{best_hz} vs {expected}");
    // Midband stays near Re and the top climbs inductively.
    assert!(v.impedance(300.0).magnitude() < 1.5 * p.re);
    assert!(v.impedance(10_000.0).magnitude() > 2.0 * p.re);
    // Four drivers share air mass, which pulls the resonance back down.
    let array = LoadValues::new(p, &sealed(volume, 4), 1.0);
    assert!(array.cmes > v.cmes);
}

/// Adjustable parts in the time-domain solver agree with the AC solver, and the
/// motional node is readable while running.
#[test]
fn voltage_driven_speaker_matches_ac_and_survives_value_changes() {
    let rate = 48_000.0;
    let a = LoadValues::new(&SpeakerProfile::BRIT_V30, &sealed(0.034, 4), 1.0);
    let b = LoadValues::new(&SpeakerProfile::AMERICAN_CERAMIC, &Mounting::BAFFLE, 1.0);
    let (circuit, slots) = speaker::voltage_driven(&a).unwrap();
    let motional = circuit.unknown_named(MOTIONAL).unwrap();
    assert_eq!(motional, circuit.output);
    let mut sim = Simulation::new(circuit.clone(), rate);
    for hz in [70.0, 110.0, 1_000.0] {
        sim.reset();
        let expected = ac::solve(&circuit, &[], hz).magnitude();
        let mut peak: f64 = 0.0;
        let n = (rate * 0.6) as usize;
        for k in 0..n {
            let y = sim.process((TAU * hz * k as f64 / rate).sin());
            assert_eq!(y, sim.voltage_at(motional));
            if k > n * 2 / 3 {
                peak = peak.max(y.abs());
            }
        }
        assert!((peak / expected - 1.0).abs() < 0.02, "{hz} Hz: {peak} vs {expected}");
    }
    // Swap the driver mid-stream: one rebuild, state carried, no allocation.
    assert_no_heap(|| {
        for k in 0..4_800 {
            if k == 2_400 {
                slots.apply(&mut sim, &b);
            }
            assert!(sim.process((TAU * 200.0 * k as f64 / rate).sin()).is_finite());
        }
    });
    assert_eq!(sim.value(0), Some(b.re));
}

fn steady_peak(sim: &mut Simulation, hz: f64, amplitude: f64, rate: f64) -> f64 {
    sim.reset();
    sim.find_operating_point();
    let n = (rate * 0.4) as usize;
    let mut peak: f64 = 0.0;
    for k in 0..n {
        let y = sim.process(amplitude * (TAU * hz * k as f64 / rate).sin());
        assert!(y.is_finite());
        if k > n / 2 {
            peak = peak.max(y.abs());
        }
    }
    peak
}

/// A valve amplifier's output voltage follows the load impedance: the load is
/// coupled into the solve, not applied afterwards.
#[test]
fn every_power_stage_responds_to_the_speaker_impedance() {
    let rate = 48_000.0;
    let mounting = sealed(0.034, 4);
    for spec in [
        &PowerSpec::BRIT_EL34,
        &PowerSpec::TWIN,
        &PowerSpec::MARKIIC,
        &PowerSpec::EVH5150,
    ] {
        let values = LoadValues::new(&SpeakerProfile::BRIT_V30, &mounting, power::speaker_scale(spec));
        let mut resistive = Simulation::new(power::build(spec, 10_000.0).unwrap(), rate);
        let (circuit, _) = power::build_with_speaker(spec, 10_000.0, &values).unwrap();
        let mut loaded = Simulation::new(circuit, rate);
        let resonance = 110.0;
        let ratio = |hz: f64, r: &mut Simulation, l: &mut Simulation| {
            steady_peak(l, hz, 0.5, rate) / steady_peak(r, hz, 0.5, rate)
        };
        let at_resonance = ratio(resonance, &mut resistive, &mut loaded);
        let midband = ratio(250.0, &mut resistive, &mut loaded);
        let top = ratio(6_000.0, &mut resistive, &mut loaded);
        assert!(at_resonance > 1.4, "{}: {at_resonance}", spec.name);
        assert!((midband - 1.0).abs() < 0.15, "{}: {midband}", spec.name);
        assert!(top > 1.3, "{}: {top}", spec.name);
    }
}

/// The cases that exposed the missing winding capacitance: every power stage,
/// driven far into clipping including at high frequency, into boxed and open
/// drivers, at the lowest and highest supported rates. See `power::Parasitics`.
#[test]
fn speaker_loaded_power_is_bounded_and_settles_under_abuse() {
    let loads = [
        (&SpeakerProfile::BRIT_V30, sealed(0.034, 4)),
        (&SpeakerProfile::AMERICAN_VINTAGE_12, Mounting::BAFFLE),
        (&SpeakerProfile::AMERICAN_CERAMIC, Mounting::BAFFLE),
    ];
    for spec in [
        &PowerSpec::BRIT_EL34,
        &PowerSpec::TWIN,
        &PowerSpec::MARKIIC,
        &PowerSpec::EVH5150,
    ] {
        for (profile, mounting) in &loads {
            let values = LoadValues::new(profile, mounting, power::speaker_scale(spec));
            for rate in [48_000.0, 192_000.0] {
                for (amplitude, hz) in [(0.5, 82.0), (40.0, 82.0), (80.0, 1_000.0), (40.0, 7_000.0)] {
                    let (circuit, _) = power::build_with_speaker(spec, 10_000.0, &values).unwrap();
                    let mut sim = Simulation::new(circuit, rate);
                    sim.find_operating_point();
                    let n = (rate * 0.2) as usize;
                    let mut peak: f64 = 0.0;
                    for k in 0..n {
                        let y = sim.process(amplitude * (TAU * hz * k as f64 / rate).sin());
                        assert!(y.is_finite());
                        peak = peak.max(y.abs());
                    }
                    let (_, _, unsettled, _) = sim.statistics();
                    let label = format!("{} {} {rate} {amplitude} V {hz} Hz", spec.name, profile.id);
                    // Legacy resistive stages also need fallbacks on this input; one
                    // unsettled sample in a thousand is the bound, not a blow-up.
                    assert!(unsettled as usize <= n / 1_000, "{label}: {unsettled}");
                    assert!(peak < 150.0, "{label}: {peak}");
                }
            }
        }
    }
}
