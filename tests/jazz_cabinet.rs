//! The Roland JC-120's own cabinet and speakers: the Jazz Open 2x12 and the
//! Jazz 12. See `docs/models/cabinets.md` and `docs/models/speakers.md`.
#[path = "support/jazz_measurement.rs"]
mod measurement;
use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::speaker::SpeakerProfile;
use gainstagefx::params::{CabModel, SpeakerModel};
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{CabinetChoice, Chain, SpeakerChoice, NOMINAL_DBFS};
use measurement::{modelled, rms_error};

/// The breakup voicing was fitted to a JC-120 measured through its return jack,
/// and this holds the fit: drawn at the measurement's placement, the Jazz 12
/// follows the measured curve to within 1.3 dB rms from 90 Hz to 12 kHz --
/// closer than any of the Celestion profiles follow their own plots.
#[test]
fn the_jazz_12_draws_the_jc120_measurement() {
    let e = rms_error(SpeakerProfile::JAZZ_12);
    println!("{e:.3} dB rms");
    assert!(e < 1.3, "{e:.2} dB rms from the measurement");
}

/// The measurement's author names two features as the JC-120's hardness: a
/// sharp peak at 3.6 kHz and another at 9.2 kHz, with a deep trough between.
/// Both are in the model where the measurement has them.
#[test]
fn the_two_peaks_the_measurement_names_are_there() {
    let freqs: Vec<f64> = (0..400).map(|i| 2_000.0 * 1.006f64.powi(i)).collect();
    let db = modelled(SpeakerProfile::JAZZ_12, &freqs);
    let at = |lo: f64, hi: f64| -> (f64, f64) {
        freqs
            .iter()
            .zip(&db)
            .filter(|(f, _)| (lo..hi).contains(*f))
            .fold(
                (0.0, f64::MIN),
                |best, (f, d)| if *d > best.1 { (*f, *d) } else { best },
            )
    };
    let trough = freqs
        .iter()
        .zip(&db)
        .filter(|(f, _)| (6_000.0..7_600.0).contains(*f))
        .map(|(_, d)| *d)
        .fold(f64::MAX, f64::min);
    let (upper, upper_db) = at(2_800.0, 4_800.0);
    let (top, top_db) = at(8_000.0, 11_000.0);
    println!("{upper:.0} Hz at {upper_db:.1}, trough {trough:.1}, {top:.0} Hz at {top_db:.1}");
    assert!((3_200.0..4_100.0).contains(&upper), "{upper:.0} Hz");
    assert!((8_500.0..10_500.0).contains(&top), "{top:.0} Hz");
    assert!(upper_db - trough > 10.0, "{upper_db:.1} over {trough:.1}");
    assert!(top_db - trough > 1.5, "{top_db:.1} over {trough:.1}");
}

/// Roland's own service notes: 750 x 540 x 270 mm without casters, two 30 cm
/// speakers. Open-backed, and its Matched speaker is the Jazz 12.
#[test]
fn the_cabinet_is_the_one_roland_documents() {
    let cab = CabinetProfile::JAZZ_OPEN_212;
    assert_eq!((cab.width, cab.height, cab.depth), (0.750, 0.540, 0.270));
    assert_eq!(cab.drivers, 2);
    assert!(cab.is_open());
    assert_eq!(cab.default_speaker.id, "spk_roland_30_103d");
    assert!(CabinetProfile::ALL.iter().any(|c| c.id == cab.id));
    assert!(SpeakerProfile::ALL
        .iter()
        .any(|s| s.id == "spk_roland_30_103d"));
}

/// Both JC-120 presets play through the amplifier's own cabinet and speakers,
/// and not the Fender cabinet and Jensen that stood in for them.
#[test]
fn the_jazz_presets_play_through_their_own_cabinet() {
    for name in ["Jazz Clean", "Jazz Chorus"] {
        let p = PRESETS.iter().find(|p| p.name == name).expect(name);
        assert_eq!(p.cab_model, CabModel::JazzOpen212, "{name}");
        assert_eq!(p.speaker, SpeakerModel::Matched, "{name}");
        let s = p.settings();
        assert_eq!(
            s.acoustic.cabinet,
            CabinetChoice::Model(&CabinetProfile::JAZZ_OPEN_212)
        );
        assert_eq!(s.acoustic.speaker, SpeakerChoice::Matched);
    }
}

/// The JC-120's complementary transistor stage drives the new load at every
/// rate the plugin runs at, and stays finite and audible doing it.
#[test]
fn the_jazz_rig_plays_at_every_rate() {
    let p = PRESETS.iter().find(|p| p.name == "Jazz Chorus").unwrap();
    let amplitude = 10f64.powf((NOMINAL_DBFS + p.input_trim as f64) / 20.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let mut chain = Chain::new(rate);
        chain.apply(&p.settings());
        chain.settle();
        let mut peak = 0.0f64;
        for k in 0..(rate as usize / 4) {
            let x = amplitude * (std::f64::consts::TAU * 196.0 * k as f64 / rate).sin();
            let y = chain.process(x);
            assert!(y.is_finite(), "{rate}");
            peak = peak.max(y.abs());
        }
        assert!(peak > 1e-3, "{rate}: {peak}");
    }
}
