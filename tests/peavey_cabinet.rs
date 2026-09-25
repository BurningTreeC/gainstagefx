//! The late-80s Peavey 4x12 and its Celestion G12K-85s: the American Closed
//! 4x12 and the Brit K85. See `docs/models/cabinets.md` and
//! `docs/models/speakers.md`.
#[path = "support/jazz_measurement.rs"]
mod measurement;
use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::speaker::SpeakerProfile;
use gainstagefx::params::{CabModel, SpeakerModel};
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{CabinetChoice, Chain, SpeakerChoice, NOMINAL_DBFS};
use measurement::modelled_in;

/// Celestion describes the G12K-85 as the G12T-75 with a larger magnet, and
/// the profile is built that way: every figure the two share is the same, and
/// the motor fitted to the G12K-100's plot comes out stronger -- a higher Bl
/// and a lower Qes, which is what fifty ounces of ceramic does.
#[test]
fn the_k85_is_a_t75_with_a_bigger_magnet() {
    let (k, t) = (SpeakerProfile::BRIT_K85, SpeakerProfile::BRIT_T75);
    assert_eq!((k.fs, k.mms, k.qms, k.sd), (t.fs, t.mms, t.qms, t.sd));
    assert_eq!((k.l1, k.r1, k.l2, k.r2), (t.l1, t.r1, t.l2, t.r2));
    // Celestion's published Re and Fs for the G12K-100.
    assert_eq!((k.re, k.fs), (7.0, 85.0));
    println!(
        "Bl {:.2} vs {:.2}, Qes {:.3} vs {:.3}",
        k.bl,
        t.bl,
        k.qes(),
        t.qes()
    );
    assert!(k.bl > 1.2 * t.bl, "Bl {} against {}", k.bl, t.bl);
    assert!(k.qes() < 0.7 * t.qes());
}

/// Drawn through the plugin's own path -- the driver's motional response,
/// then a close microphone on one cone of the Peavey -- the K85 keeps the
/// shape of Celestion's plot: a presence peak between 2.2 and 3.6 kHz well
/// above its 1 kHz level (the plot: 107.6 dB against 99.1), and a cliff by
/// 7 kHz (the plot: 78.6 dB at 6.8 kHz).
#[test]
fn the_k85_keeps_celestions_shape_in_the_peavey() {
    let freqs: Vec<f64> = (0..240).map(|i| 500.0 * 1.012f64.powi(i)).collect();
    let db = modelled_in(
        &CabinetProfile::AMERICAN_CLOSED_412,
        SpeakerProfile::BRIT_K85,
        &freqs,
    );
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
        .filter(|(f, _)| (1_500.0..6_000.0).contains(*f))
        .fold(
            (0.0, f64::MIN),
            |b, (f, d)| if *d > b.1 { (*f, *d) } else { b },
        );
    let (one_k, seven_k) = (at(1_000.0), at(7_000.0));
    println!("peak {peak:.1} at {peak_at:.0} Hz, 1 kHz {one_k:.1}, 7 kHz {seven_k:.1}");
    assert!(
        (2_200.0..3_600.0).contains(&peak_at),
        "peak at {peak_at:.0} Hz"
    );
    assert!(peak - one_k > 5.0, "only {:.1} dB over 1 kHz", peak - one_k);
    assert!(
        peak - seven_k > 15.0,
        "only {:.1} dB above 7 kHz",
        peak - seven_k
    );
}

/// The cabinet is the one the sources describe: a closed, straight-fronted
/// Peavey 4x12 of Peavey's usual shell, and its Matched speaker is the K85.
#[test]
fn the_cabinet_is_a_closed_peavey_4x12() {
    let cab = CabinetProfile::AMERICAN_CLOSED_412;
    assert_eq!((cab.width, cab.height, cab.depth), (0.765, 0.816, 0.362));
    assert_eq!(cab.drivers, 4);
    assert!(!cab.is_open());
    assert_eq!(cab.slant, 0.0);
    assert_eq!(cab.default_speaker.id, "spk_celestion_g12k85");
    assert!(CabinetProfile::ALL
        .iter()
        .any(|c| c.id == "cab_peavey_412m"));
    assert!(SpeakerProfile::ALL
        .iter()
        .any(|s| s.id == "spk_celestion_g12k85"));
    // Appended to both parameter lists, so nothing recorded against them moves.
    assert_eq!(CabModel::ALL.last(), Some(&CabModel::AmericanClosed412));
    assert_eq!(SpeakerModel::ALL.last(), Some(&SpeakerModel::BritK85));
}

/// *Machine Rage '92* plays through "a 1987 Peavey 4x12 cabinet with Celestion
/// G12K-85 speakers" now, and not the generic box and G12T-75 that stood in.
#[test]
fn machine_rage_plays_through_the_peavey() {
    let p = PRESETS
        .iter()
        .find(|p| p.name == "Machine Rage '92")
        .unwrap();
    assert_eq!(p.cab_model, CabModel::AmericanClosed412);
    assert_eq!(p.speaker, SpeakerModel::Matched);
    let s = p.settings();
    assert_eq!(
        s.acoustic.cabinet,
        CabinetChoice::Model(&CabinetProfile::AMERICAN_CLOSED_412)
    );
    assert_eq!(s.acoustic.speaker, SpeakerChoice::Matched);
    assert_eq!(
        s.acoustic.resolved_speaker().map(|s| s.id),
        Some("spk_celestion_g12k85")
    );
}

/// The 2205's power stage drives the new load at every rate the plugin runs
/// at, and stays finite and audible doing it.
#[test]
fn the_peavey_rig_plays_at_every_rate() {
    let p = PRESETS
        .iter()
        .find(|p| p.name == "Machine Rage '92")
        .unwrap();
    let amplitude = 10f64.powf((NOMINAL_DBFS + p.input_trim as f64) / 20.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let mut chain = Chain::new(rate);
        chain.apply(&p.settings());
        chain.settle();
        let mut peak = 0.0f64;
        for k in 0..(rate as usize / 5) {
            let x = amplitude * (std::f64::consts::TAU * 110.0 * k as f64 / rate).sin();
            let y = chain.process(x);
            assert!(y.is_finite(), "{rate}");
            peak = peak.max(y.abs());
        }
        println!("{rate}: peak {peak:.3}");
        assert!(peak > 1e-3, "{rate}: {peak}");
    }
}
