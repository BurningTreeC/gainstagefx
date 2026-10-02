//! The American 4x10 (Gallien-Krueger 410RBH) and its Cast Bass 10 (the
//! P10/200): GK's documented box, with its vents and crossover, and the
//! estimates the research log labels. See `docs/models/american_410.md`;
//! `examples/american_410_op.rs` prints the vents' tuning.
use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::mic::MicPlacement;
use gainstagefx::acoustics::speaker::{LoadValues, SpeakerProfile};
use gainstagefx::acoustics::stage::{AcousticStage, MicSlot};
use gainstagefx::dsp::complex::C;
use gainstagefx::params::{CabModel, SpeakerModel};
use gainstagefx::voice::{AcousticSettings, CabinetChoice, Chain, Gain, Settings, Tone};
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use nice_plug::prelude::Enum;

const CAB: CabinetProfile = CabinetProfile::AMERICAN_410;
const P10: SpeakerProfile = SpeakerProfile::CAST_BASS_10;
const RATE: f64 = 48_000.0;

/// The level, dB, at a microphone a metre out on the first ten's axis, of the
/// four tens in `cab` voltage driven: the driver's motion from its load in the
/// box, through the acoustic stage's radiation, ports and crossover included.
fn response(cab: &'static CabinetProfile, freqs: &[f64]) -> Vec<f64> {
    let placement = MicPlacement {
        position: 0.0,
        distance: 1.0,
        angle: 0.0,
    };
    let load = LoadValues::new(cab.default_speaker, &cab.mounting(), 1.0);
    let mut stage = AcousticStage::new(RATE);
    stage.configure(Some(cab), cab.default_speaker, MicSlot::Ideal, MicSlot::Off);
    stage.set_placement(placement, placement, 0.0, 0.0, 0.0, false, false);
    stage.reset();
    let impulse: Vec<f64> = (0..(1 << 15))
        .map(|k| stage.process(if k == 0 { 1.0 } else { 0.0 }))
        .collect();
    freqs
        .iter()
        .map(|&hz| {
            let w = std::f64::consts::TAU * hz / RATE;
            let step = C::new(w.cos(), -w.sin());
            let mut phasor = C::real(1.0);
            let mut sum = C::ZERO;
            for &y in &impulse {
                sum = sum + phasor * C::real(y);
                phasor = phasor * step;
            }
            let s = C::new(0.0, std::f64::consts::TAU * hz);
            let lossy = |l: f64, r: f64| {
                let sl = s * C::real(l);
                sl * C::real(r) / (C::real(r) + sl)
            };
            let coil = C::real(load.re) + lossy(load.l1, load.r1) + lossy(load.l2, load.r2);
            let motional = load.impedance(hz) - coil;
            let cone = motional * (coil + motional).recip() * s / C::real(load.bl);
            20.0 * (cone * sum).magnitude().max(1e-30).log10()
        })
        .collect()
}

fn leaked(cab: CabinetProfile) -> &'static CabinetProfile {
    Box::leak(Box::new(cab))
}

/// GK's owner's manual: 24 x 27.5 x 18 in, 3/4 in birch, four P10/200, two
/// front vents, an 18 dB/oct crossover; and nothing else in the catalogue has
/// either a vent or a crossover, so every other cabinet is built as it was.
#[test]
fn the_cabinet_is_gks_410rbh() {
    assert_eq!((CAB.width, CAB.height, CAB.depth), (0.6096, 0.6985, 0.4572));
    assert_eq!((CAB.drivers, CAB.compartments, CAB.wall), (4, 1, 0.019));
    assert_eq!(CAB.vent.map(|v| v.count), Some(2));
    assert!(CAB.crossover_hz.is_some() && !CAB.is_open());
    assert_eq!(CAB.default_speaker.id, "spk_gk_p10_200");
    for cab in CabinetProfile::ALL {
        if cab.id != CAB.id {
            assert!(
                cab.vent.is_none() && cab.crossover_hz.is_none(),
                "{}",
                cab.id
            );
        }
    }
    let ids = CabModel::ids().unwrap();
    assert_eq!(ids[CabModel::American410.to_index()], "cab_gk_410rbh");
    let speakers = SpeakerModel::ids().unwrap();
    assert_eq!(
        speakers[SpeakerModel::CastBass10.to_index()],
        "spk_gk_p10_200"
    );
}

/// The P10/200's published figures, and what the estimates make of them:
/// 32 ohm referred to 8, Fs 43 Hz, a Qes of 0.45.
#[test]
fn the_p10_200_is_what_is_published_and_estimated() {
    assert_eq!(P10.re * 4.0, 27.0);
    assert_eq!(P10.fs, 43.0);
    let qes = P10.qes();
    println!("Qes {qes:.3}");
    assert!((qes - 0.45).abs() < 0.01);
}

/// The vents tune the box: the tens' impedance in it has two peaks either
/// side of a dip at the box and vents' Helmholtz frequency, 42 Hz -- and
/// sealed, the same box has one peak above it.
#[test]
fn the_vents_tune_the_box_at_forty_two_hertz() {
    let curve = |cab: &CabinetProfile| {
        let load = LoadValues::new(&P10, &cab.mounting(), 1.0);
        let mut points = Vec::new();
        let mut hz = 15.0;
        while hz < 200.0 {
            points.push((hz, load.impedance(hz).magnitude()));
            hz *= 1.01;
        }
        points
    };
    let vented = curve(&CAB);
    let peaks = vented
        .windows(3)
        .filter(|w| w[1].1 > w[0].1 && w[1].1 > w[2].1)
        .count();
    let dip = vented
        .windows(3)
        .find(|w| w[1].1 < w[0].1 && w[1].1 < w[2].1)
        .map(|w| w[1].0)
        .unwrap();
    let sealed = curve(&CabinetProfile { vent: None, ..CAB });
    let sealed_peaks = sealed
        .windows(3)
        .filter(|w| w[1].1 > w[0].1 && w[1].1 > w[2].1)
        .count();
    println!("vented: {peaks} peaks, tuned at {dip:.1} Hz; sealed: {sealed_peaks} peak");
    assert_eq!((peaks, sealed_peaks), (2, 1));
    assert!((40.0..=44.0).contains(&dip), "{dip}");
}

/// Small's vented-box function for these drivers in this box, over the same
/// drivers sealed in it, dB: what the vents should add.
fn small(f: f64) -> f64 {
    let speaker = &P10;
    let vas = 1.18 * 343.0 * 343.0 * speaker.sd * speaker.sd * speaker.cms();
    let alpha = CAB.drivers as f64 * vas / CAB.volume();
    let qts = speaker.qes() * speaker.qms / (speaker.qes() + speaker.qms);
    let vent = CAB.vent.unwrap();
    let fb = 343.0 / std::f64::consts::TAU
        * (vent.count as f64 * vent.area / (CAB.volume() * vent.effective_length())).sqrt();
    let (fs, ql) = (speaker.fs, CAB.leakage_q);
    let h = fb / fs;
    let t0 = 1.0 / (std::f64::consts::TAU * (fs * fb).sqrt());
    let a1 = (ql + h * qts) / (h.sqrt() * ql * qts);
    let a2 = (h + (alpha + 1.0 + h * h) * ql * qts) / (h * ql * qts);
    let a3 = (h * ql + qts) / (h.sqrt() * ql * qts);
    let s = C::new(0.0, std::f64::consts::TAU * f * t0);
    let s2 = s * s;
    let vented = s2
        * s2
        * (s2 * s2 + C::real(a1) * s2 * s + C::real(a2) * s2 + C::real(a3) * s + C::real(1.0))
            .recip();
    let fc = fs * (1.0 + alpha).sqrt();
    let qtc = qts * (1.0 + alpha).sqrt();
    let x = C::new(0.0, f / fc);
    let sealed = x * x * (x * x + x * C::real(1.0 / qtc) + C::real(1.0)).recip();
    20.0 * (vented.magnitude() / sealed.magnitude()).log10()
}

/// And it radiates as a vented box: at a metre, what the vents add over the
/// same box sealed is what Small's theory of the vented box gives for these
/// drivers and this box, to a decibel, across the bass -- the cavity, the
/// ports' mass and their radiation from the baffle, the drivers' motion in
/// the load, all of it. (A first expectation here, of 5 dB or more at 40 Hz,
/// was the test's and not the box's: Small's own figure for this alignment is
/// 2.4.)
#[test]
fn the_vents_add_what_smalls_theory_gives() {
    let freqs = [35.0, 40.0, 50.0, 60.0, 80.0, 120.0];
    let vented = response(leaked(CAB), &freqs);
    let sealed = response(leaked(CabinetProfile { vent: None, ..CAB }), &freqs);
    for (i, &f) in freqs.iter().enumerate() {
        let model = vented[i] - sealed[i];
        let theory = small(f);
        println!("{f:4.0} Hz: vents add {model:+.2} dB, Small {theory:+.2}");
        assert!(
            (model - theory).abs() < 1.0,
            "{f}: {model} against {theory}"
        );
    }
}

/// The crossover: a third-order low-pass on the woofers, 18 dB an octave
/// past its corner and nothing an octave below it.
#[test]
fn the_crossover_takes_the_tens_out_of_the_treble() {
    let freqs = [1_500.0, 6_000.0, 12_000.0];
    let with = response(leaked(CAB), &freqs);
    let without = response(
        leaked(CabinetProfile {
            crossover_hz: None,
            ..CAB
        }),
        &freqs,
    );
    let d: Vec<f64> = with.iter().zip(&without).map(|(a, b)| a - b).collect();
    println!(
        "crossover: {:+.1} dB at 1.5 kHz, {:+.1} at 6 kHz, {:+.1} at 12 kHz",
        d[0], d[1], d[2]
    );
    assert!(d[0].abs() < 1.5);
    assert!(d[1] < -14.0 && d[1] > -22.0);
    assert!(d[2] - d[1] < -14.0);
}

/// The 800RB through it, at every rate, finite and without trouble.
#[test]
fn the_800rb_plays_through_it_at_every_rate() {
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        chain.apply(&Settings {
            gain: Gain::American800RB,
            drive: 0.6,
            tone: Tone::Off,
            oversampling: 1,
            acoustic: AcousticSettings {
                cabinet: CabinetChoice::Model(&CabinetProfile::AMERICAN_410),
                ..AcousticSettings::default()
            },
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let mut peak: f64 = 0.0;
        for k in 0..4_096 {
            let x = 0.4 * (k as f64 * std::f64::consts::TAU * 41.2 / rate).sin();
            let y = chain.process(x);
            assert!(y.is_finite(), "{rate}");
            peak = peak.max(y.abs());
        }
        assert!(peak > 1e-3, "{rate}: silent");
    }
}

/// A clean amplifier through `cab` with an ideal omni `distance` in front of
/// the first ten and the horn at `horn`, a tone at `hz`: its level, dB.
fn played(cab: &'static CabinetProfile, distance: f64, horn: f64, hz: f64, rate: f64) -> f64 {
    let mut chain = Chain::new(rate);
    chain.apply(&Settings {
        gain: Gain::Clean,
        drive: 0.3,
        tone: Tone::Off,
        oversampling: 1,
        acoustic: AcousticSettings {
            cabinet: CabinetChoice::Model(cab),
            mic_a: MicSlot::Ideal,
            place_a: MicPlacement {
                position: 0.0,
                distance,
                angle: 0.0,
            },
            horn,
            ..AcousticSettings::default()
        },
        ..Settings::default()
    });
    chain.settle();
    chain.reset();
    chain.find_operating_point();
    let n = (rate * 0.3) as usize;
    let mut sum = 0.0;
    for k in 0..n {
        let y = chain.process(0.05 * (std::f64::consts::TAU * hz * k as f64 / rate).sin());
        if k > n / 2 {
            sum += y * y;
        }
    }
    10.0 * (sum / (n - n / 2 - 1) as f64).log10()
}

/// The horn carries the treble the crossover takes from the tens. A metre
/// out, where the horn is nearly on its axis, it puts 6 kHz back level with
/// the tens' own band (a kilohertz, horn off) -- the tens alone are some 40 dB
/// down there behind the crossover. On a close microphone at a ten's cone the
/// horn is far off its axis and adds a few decibels at most. Nothing in the
/// bass.
#[test]
fn the_horn_carries_the_treble_where_it_points() {
    let cab = &CabinetProfile::AMERICAN_410;
    let band = played(cab, 1.0, 0.0, 1_000.0, RATE);
    let (off, full) = (
        played(cab, 1.0, 0.0, 6_000.0, RATE),
        played(cab, 1.0, 1.0, 6_000.0, RATE),
    );
    let near = played(cab, 0.025, 1.0, 6_000.0, RATE) - played(cab, 0.025, 0.0, 6_000.0, RATE);
    let bass = played(cab, 1.0, 1.0, 100.0, RATE) - played(cab, 1.0, 0.0, 100.0, RATE);
    println!(
        "a metre out: the tens' band {band:+.1} dB at 1 kHz; at 6 kHz {off:+.1} horn off, {full:+.1} horn full; close: horn adds {near:+.1}; bass {bass:+.2}"
    );
    assert!(full - off > 20.0, "{off} {full}");
    assert!((full - band).abs() < 6.0, "{full} against {band}");
    assert!(near < 6.0, "{near}");
    assert!(bass.abs() < 0.1, "{bass}");
}

/// With the attenuator down the horn is gone: the chain's output is the same
/// cabinet's without a horn, to the bit.
#[test]
fn the_horn_attenuated_fully_is_no_horn() {
    let cab = &CabinetProfile::AMERICAN_410;
    let hornless = leaked(CabinetProfile { horn: None, ..CAB });
    let run = |cab: &'static CabinetProfile| {
        let mut chain = Chain::new(RATE);
        chain.apply(&Settings {
            gain: Gain::American800RB,
            drive: 0.5,
            tone: Tone::Off,
            oversampling: 1,
            acoustic: AcousticSettings {
                cabinet: CabinetChoice::Model(cab),
                horn: 0.0,
                ..AcousticSettings::default()
            },
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        (0..4_096)
            .map(|k| chain.process(0.3 * (k as f64 * 0.07).sin()).to_bits())
            .collect::<Vec<u64>>()
    };
    assert!(run(cab) == run(hornless));
}

/// The horn's stream at every rate and every oversampling it can run at --
/// its own decimator and padding included -- finite and without allocating.
#[test]
fn the_horn_is_realtime_safe_at_every_rate() {
    let mut chain = Chain::new(44_100.0);
    for (rate, oversampling) in [
        (44_100.0, 1),
        (48_000.0, 2),
        (88_200.0, 4),
        (96_000.0, 2),
        (192_000.0, 1),
    ] {
        chain.set_rate(rate);
        chain.apply(&Settings {
            gain: Gain::Clean,
            drive: 0.5,
            tone: Tone::Off,
            oversampling,
            acoustic: AcousticSettings {
                cabinet: CabinetChoice::Model(&CabinetProfile::AMERICAN_410),
                horn: 1.0,
                ..AcousticSettings::default()
            },
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let mut peak: f64 = 0.0;
        assert_no_heap(|| {
            for k in 0..4_096 {
                let x = 0.2 * (k as f64 * std::f64::consts::TAU * 3_500.0 / rate).sin();
                let y = chain.process(x);
                assert!(y.is_finite(), "{rate}");
                peak = peak.max(y.abs());
            }
        });
        assert!(peak > 1e-4, "{rate}: silent");
    }
}
