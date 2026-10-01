//! The American 8x10 (Ampeg SVT-810E) and its American Bass 10s (Eminence
//! Legend B810). See `docs/models/cabinets.md` and `docs/models/speakers.md`.
#[path = "support/jazz_measurement.rs"]
mod measurement;
use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::enclosure::Enclosure;
use gainstagefx::acoustics::speaker::{LoadValues, SpeakerProfile, RHO, SPEED_OF_SOUND};
use gainstagefx::params::{CabModel, SpeakerModel};
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{CabinetChoice, Chain, SpeakerChoice, NOMINAL_DBFS};
use measurement::modelled_in;

const CAB: CabinetProfile = CabinetProfile::AMERICAN_810;
const B810: SpeakerProfile = SpeakerProfile::AMERICAN_BASS_10;

/// Eminence's sheet for the 32 ohm part, referred to the catalogue's 8 ohm:
/// impedances a quarter, Bl a half, the rest as published -- and the
/// published Qes follows from the others.
#[test]
fn the_b810_is_eminences_sheet_referred_to_8_ohm() {
    assert_eq!(B810.re * 4.0, 27.5);
    assert!((B810.bl * 2.0 - 17.7).abs() < 1e-9);
    assert_eq!(
        (B810.fs, B810.qms, B810.mms, B810.sd),
        (52.07, 13.91, 24.0e-3, 350.1e-4)
    );
    let qes = B810.qes();
    println!("Qes {qes:.3} (published 0.68)");
    assert!((qes - 0.68).abs() < 0.015);
}

/// Ampeg's size, eight drivers in four sealed chambers of two, the B810 as
/// its Matched speaker, and both appended to their parameter lists.
#[test]
fn the_cabinet_is_ampegs_8x10() {
    assert_eq!((CAB.width, CAB.height, CAB.depth), (0.6604, 1.219, 0.4064));
    assert_eq!((CAB.drivers, CAB.compartments), (8, 4));
    assert!(!CAB.is_open());
    assert_eq!(CAB.default_speaker.id, "spk_eminence_legend_b810");
    assert_eq!(CAB.driver_positions().len(), 8);
    // Two to a chamber: every chamber's centre has a driver either side.
    let h = CAB.compartment_height();
    let (_, inside, _) = CAB.internal();
    for chamber in 0..4 {
        let centre = -inside / 2.0 + h / 2.0 + chamber as f64 * (h + CAB.wall);
        let count = CAB
            .driver_positions()
            .iter()
            .filter(|&&(_, y)| (y - centre).abs() < h / 2.0)
            .count();
        assert_eq!(count, 2, "chamber {chamber}");
    }
    assert_eq!(CabModel::ALL.last(), Some(&CabModel::American810));
    assert_eq!(
        SpeakerModel::ALL.last(),
        Some(&SpeakerModel::AmericanBass10)
    );
    // And every other cabinet is one chamber, built as it always was.
    for cab in CabinetProfile::ALL {
        if cab.id != CAB.id {
            assert_eq!(cab.compartments, 1, "{}", cab.id);
        }
    }
}

/// The chambers change what depends on a chamber's size and nothing else: the
/// first standing wave is a 281 mm chamber's, not a 1.18 m box's, while the
/// sealed air the cones work against is the whole box's -- a closed-box
/// resonance near `fs sqrt(1 + Vas / Vb)`, about 95 Hz with 29 L a driver.
#[test]
fn the_chambers_set_the_modes_and_the_whole_box_sets_the_bottom() {
    let e = Enclosure::new(&CAB, &B810);
    let first = e.modes[0].hz;
    let whole_box_height_mode = SPEED_OF_SOUND / (2.0 * CAB.internal().1);
    println!("first mode {first:.0} Hz (one box would have {whole_box_height_mode:.0})");
    assert!(first > 400.0, "{first}");
    assert_eq!(e.volume, CAB.volume());

    let per_driver = CAB.volume() / CAB.drivers as f64;
    let vas = RHO * SPEED_OF_SOUND * SPEED_OF_SOUND * B810.sd * B810.sd * B810.cms();
    let fc = B810.fs * (1.0 + vas / per_driver).sqrt();
    let load = LoadValues::new(&B810, &CAB.mounting(), 0.5);
    let (peak_hz, peak) = (300..2_000)
        .map(|i| i as f64 / 10.0)
        .map(|hz| (hz, load.impedance(hz).magnitude()))
        .fold((0.0, 0.0), |b, (hz, z)| if z > b.1 { (hz, z) } else { b });
    println!(
        "{:.1} L a driver, Vas {:.1} L: closed-box fc {fc:.0} Hz; impedance peak {peak:.1} ohm at {peak_hz:.0} Hz",
        per_driver * 1e3,
        vas * 1e3
    );
    assert!((80.0..115.0).contains(&fc), "{fc}");
    assert!((peak_hz - fc).abs() < 0.2 * fc, "{peak_hz} against {fc}");
}

/// Drawn through the plugin's own path -- the driver's motional response, a
/// close microphone on one cone of the 8x10 -- the B810 keeps the shape of
/// Eminence's plot: its 2.5 kHz peak well over 1 kHz (98.8 dB against 93.5),
/// and the fall past 5 kHz (about 70 dB at 7 kHz).
#[test]
fn the_b810_keeps_eminences_shape_in_the_8x10() {
    let freqs: Vec<f64> = (0..240).map(|i| 500.0 * 1.012f64.powi(i)).collect();
    let db = modelled_in(&CabinetProfile::AMERICAN_810, B810, &freqs);
    let at = |hz: f64| {
        freqs
            .iter()
            .zip(&db)
            .min_by(|a, b| (a.0 - hz).abs().total_cmp(&(b.0 - hz).abs()))
            .map(|(_, d)| *d)
            .unwrap()
    };
    let (peak_at, peak) = freqs
        .iter()
        .zip(&db)
        .filter(|(f, _)| (1_500.0..4_000.0).contains(*f))
        .fold(
            (0.0, f64::MIN),
            |b, (f, d)| if *d > b.1 { (*f, *d) } else { b },
        );
    let (one_k, seven_k) = (at(1_000.0), at(7_000.0));
    println!("peak {peak:.1} at {peak_at:.0} Hz, 1 kHz {one_k:.1}, 7 kHz {seven_k:.1}");
    assert!(
        (2_000.0..3_200.0).contains(&peak_at),
        "peak at {peak_at:.0} Hz"
    );
    assert!(peak - one_k > 3.0, "only {:.1} dB over 1 kHz", peak - one_k);
    assert!(
        peak - seven_k > 15.0,
        "only {:.1} dB above 7 kHz",
        peak - seven_k
    );
}

/// The American SVT's preset plays through its own cabinet, and the SVT's
/// power stage drives the eight-driver load at every rate the plugin runs at.
#[test]
fn the_svt_plays_through_its_8x10_at_every_rate() {
    let p = PRESETS
        .iter()
        .find(|p| p.name == "American SVT Grind")
        .unwrap();
    assert_eq!(p.cab_model, CabModel::American810);
    assert_eq!(p.speaker, SpeakerModel::Matched);
    let s = p.settings();
    assert_eq!(
        s.acoustic.cabinet,
        CabinetChoice::Model(&CabinetProfile::AMERICAN_810)
    );
    assert_eq!(s.acoustic.speaker, SpeakerChoice::Matched);
    assert_eq!(
        s.acoustic.resolved_speaker().map(|s| s.id),
        Some("spk_eminence_legend_b810")
    );
    let amplitude = 10f64.powf((NOMINAL_DBFS + p.input_trim as f64) / 20.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let mut chain = Chain::new(rate);
        chain.apply(&s);
        chain.settle();
        let mut peak = 0.0f64;
        for k in 0..(rate as usize / 5) {
            let x = amplitude * (std::f64::consts::TAU * 55.0 * k as f64 / rate).sin();
            let y = chain.process(x);
            assert!(y.is_finite(), "{rate}");
            peak = peak.max(y.abs());
        }
        println!("{rate}: peak {peak:.3}");
        assert!(peak > 1e-3, "{rate}: silent");
    }
}
