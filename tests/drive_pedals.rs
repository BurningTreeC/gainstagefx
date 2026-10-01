//! The Gold Drive (Klon Centaur), Brit Drive (Marshall Guv'nor) and Clean
//! Boost (MXR MicroAmp) against ElectroSmash's analyses. See
//! `docs/models/gold_drive.md`, `brit_drive.md` and `clean_boost.md`;
//! `examples/drive_pedals_op.rs` prints what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::{brit_drive, clean_boost, gold_drive};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::Circuit;
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::{Circuit as Panel, PedalModel};
use gainstagefx::voice::{Chain, Gain, Level, Pedal, PedalSettings, Settings, Tone as Stack};
use nice_plug::prelude::Enum;

const RATE: f64 = 96_000.0;

fn measured(circuit: Circuit, controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    let mut s = Simulation::new(circuit, RATE);
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 4.0) as usize, |x| s.process(x))
}

fn gain(circuit: Circuit, controls: &[(usize, f64)], hz: f64) -> f64 {
    measured(circuit, controls, hz, 1e-4).gain_db()
}

fn gd(at: &str) -> Circuit {
    gold_drive::tap(10_000.0, 470_000.0, at).unwrap()
}

fn bd(at: &str) -> Circuit {
    brit_drive::tap(10_000.0, 470_000.0, at).unwrap()
}

fn cb(at: &str) -> Circuit {
    clean_boost::tap(10_000.0, 470_000.0, at).unwrap()
}

/// "The maximum gain is 40 dB (100 times) around 1 kHz" -- 39.9 here: U1B over its own
/// input side, after C3 -- the gain stage alone. From the buffer it reads
/// about 3 dB less, which is C3 into feed-forward 1's 1.5 k, a real load.
#[test]
fn the_gold_drives_gain_stage_makes_forty_decibels() {
    let c = [(gold_drive::GAIN, 1.0)];
    let stage = gain(gd("u1b"), &c, 1_000.0) - gain(gd("c3b"), &c, 1_000.0);
    println!("{stage:.1} dB");
    assert!((38.5..41.0).contains(&stage), "{stage}");
    // And it falls away below: at 100 Hz C7 has taken R11 out of the leg, so
    // the stage's own gain is about 30 dB -- the mid hump.
    let low = gain(gd("u1b"), &c, 100.0) - gain(gd("c3b"), &c, 100.0);
    println!("100 Hz {low:.1} dB");
    assert!(low < stage - 6.0, "{low}");
}

/// The second gang turns the second clean path down as the first turns the
/// gain stage up.
#[test]
fn the_gold_drives_second_gang_works_against_the_first() {
    let ff2 = |g: f64| gain(gd("ff2"), &[(gold_drive::GAIN, g)], 1_000.0);
    let (down, up) = (ff2(0.0), ff2(1.0));
    println!("feed-forward 2: gain 0 {down:+.1} dB, gain 1 {up:+.1} dB");
    assert!(down > up + 20.0);
}

/// The treble shelf, hinged at 408 Hz: "Gvmax = 8.16 (18.24 dB)", "Gvmin = 0.4
/// (-8 dB)", and unity well below the hinge.
#[test]
fn the_gold_drives_treble_is_a_shelf_of_plus_eighteen_and_minus_eight() {
    let shelf = |t: f64, hz: f64| {
        let c = [(gold_drive::TREBLE, t), (gold_drive::GAIN, 0.0)];
        gain(gd("u2b"), &c, hz) - gain(gd("u2a"), &c, hz)
    };
    let (up, down) = (shelf(1.0, 20_000.0), shelf(0.0, 20_000.0));
    println!("20 kHz: up {up:+.2} dB, down {down:+.2} dB");
    assert!((17.0..18.5).contains(&up), "{up}");
    assert!((-8.5..-7.0).contains(&down), "{down}");
    for t in [0.0, 1.0] {
        assert!(shelf(t, 60.0).abs() < 0.5);
    }
}

/// At no gain the clean paths carry the pedal and it does not distort; at full
/// gain the germanium pair is working.
#[test]
fn the_gold_drive_is_clean_at_no_gain_and_clips_at_full() {
    let thd = |g: f64| {
        measured(
            gd("out"),
            &[(gold_drive::GAIN, g), (gold_drive::LEVEL, 1.0)],
            220.0,
            0.122,
        )
        .thd_percent()
    };
    let (clean, full) = (thd(0.0), thd(1.0));
    println!("THD at 0.122 V: gain 0 {clean:.2} %, gain 1 {full:.1} %");
    assert!(clean < 0.5);
    assert!(full > 10.0);
}

/// The Guv'nor's first stage, "1 + (100 K / 2.2 K) = 46.45 (33.3 dB)" between
/// its 723 Hz shelf and its 13.2 kHz top -- at 5 kHz the arithmetic with C2 in
/// gives 32.6 dB -- and unity with the gain down.
#[test]
fn the_brit_drives_first_stage_goes_from_unity_to_thirty_three_decibels() {
    let stage = |g: f64| {
        let c = [(brit_drive::GAIN, g)];
        gain(bd("u1a"), &c, 5_000.0) - gain(bd("p1"), &c, 5_000.0)
    };
    let (down, up) = (stage(0.0), stage(1.0));
    println!("U1A at 5 kHz: {down:+.2} dB, {up:+.2} dB");
    assert!(down.abs() < 0.3);
    assert!((31.8..33.5).contains(&up), "{up}");
}

/// One track, two jobs: turned up, it also takes itself out of series with R4,
/// so the second stage's gain rises with the first's -- from 680 k / 110 k
/// towards 680 k / 10 k.
#[test]
fn the_brit_drives_gain_pot_raises_the_second_stage_too() {
    let second = |g: f64| {
        let c = [(brit_drive::GAIN, g)];
        gain(bd("u2b"), &c, 1_000.0) - gain(bd("u1a"), &c, 1_000.0)
    };
    let (down, up) = (second(0.0), second(1.0));
    println!("U2B at 1 kHz: {down:+.1} dB, {up:+.1} dB");
    assert!(up > down + 15.0);
    assert!((32.0..36.0).contains(&up), "{up}");
}

/// Each tone control raises the band it is named for.
#[test]
fn the_brit_drives_tone_controls_go_the_way_they_are_named() {
    let at = |control: usize, v: f64, hz: f64| {
        let mut c = vec![
            (brit_drive::BASS, 0.5),
            (brit_drive::MIDDLE, 0.5),
            (brit_drive::TREBLE, 0.5),
            (brit_drive::GAIN, 0.0),
        ];
        c.retain(|(k, _)| *k != control);
        c.push((control, v));
        gain(bd("t_w"), &c, hz) - gain(bd("clip"), &c, hz)
    };
    for (control, hz) in [
        (brit_drive::BASS, 80.0),
        (brit_drive::MIDDLE, 1_000.0),
        (brit_drive::TREBLE, 8_000.0),
    ] {
        let (down, up) = (at(control, 0.0, hz), at(control, 1.0, hz));
        println!("control {control} at {hz} Hz: {down:+.1} -> {up:+.1} dB");
        assert!(up > down + 5.0, "control {control}");
    }
}

/// "The clipping point is around 1.8 to 2 V": two red LEDs to ground. The
/// catalogue's red LED clips the node at 1.58 V, a little under.
#[test]
fn the_brit_drive_clips_on_its_leds() {
    let c = bd("clip");
    let mut s = Simulation::new(c, RATE);
    s.set_control(brit_drive::GAIN, 1.0);
    s.find_operating_point();
    let mut peak = 0.0f64;
    for n in 0..(RATE * 0.3) as usize {
        let y = s.process(0.3 * (std::f64::consts::TAU * 440.0 * n as f64 / RATE).sin());
        if n > (RATE * 0.1) as usize {
            peak = peak.max(y.abs());
        }
    }
    println!("clip node peak {peak:.2} V");
    assert!((1.4..2.2).contains(&peak), "{peak}");
}

/// "Gv max = 20.6 (26.2 dB)", unity (with the divider, half a decibel over)
/// at the stop, and the reverse-log track's 50 k region at half rotation:
/// 2.06 from the op-amp, 5.9 dB at the jack. Flat across the band.
#[test]
fn the_clean_boost_goes_from_unity_to_twenty_six_decibels_and_is_flat() {
    let at = |g: f64, hz: f64| gain(cb("out"), &[(clean_boost::GAIN, g)], hz);
    let (stop, half, full) = (at(0.0, 1_000.0), at(0.5, 1_000.0), at(1.0, 1_000.0));
    println!("{stop:+.2} / {half:+.2} / {full:+.2} dB");
    assert!(stop.abs() < 0.8);
    assert!((5.3..6.5).contains(&half), "{half}");
    assert!((25.8..26.5).contains(&full), "{full}");
    for hz in [50.0, 200.0, 5_000.0, 15_000.0] {
        assert!((at(1.0, hz) - full).abs() < 0.5, "{hz}");
    }
}

/// In both lists under stable ids, with the knobs each box has.
#[test]
fn they_are_pedals_and_circuits_with_their_own_knobs() {
    let pedals = PedalModel::ids().unwrap();
    let circuits = Panel::ids().unwrap();
    for (pedal, pid, circuit, cid, gain) in [
        (
            PedalModel::GoldDrive,
            "pedal_klon_centaur",
            Panel::GoldDrive,
            "pedal_klon_centaur_circuit",
            Gain::GoldDrive,
        ),
        (
            PedalModel::BritDrive,
            "pedal_marshall_guvnor",
            Panel::BritDrive,
            "pedal_marshall_guvnor_circuit",
            Gain::BritDrive,
        ),
        (
            PedalModel::CleanBoost,
            "pedal_mxr_microamp",
            Panel::CleanBoost,
            "pedal_mxr_microamp_circuit",
            Gain::CleanBoost,
        ),
    ] {
        assert_eq!(pedals[pedal.to_index()], pid);
        assert_eq!(circuits[circuit.to_index()], cid);
        assert_eq!(circuit.voice(), gain);
        assert_eq!(pedal.voice().as_circuit(), Some(gain));
        assert!(gain.is_modelled());
        assert!(gain.power_stage().is_none());
        assert_eq!(gain.drive_name(), "GAIN");
    }
    assert_eq!(
        Pedal::GoldDrive.tone_labels(),
        [Some("treble"), None, None, None]
    );
    assert_eq!(
        Pedal::BritDrive.tone_labels(),
        [Some("bass"), Some("middle"), Some("treble"), None]
    );
    assert!(!Pedal::CleanBoost.has_level());
    assert!(Pedal::CleanBoost.has_drive());
    assert!(Gain::CleanBoost.drive_is_channel_volume());
    assert!(Gain::CleanBoost.level_control().is_none());
    assert_eq!(
        Gain::GoldDrive.level_control(),
        Some(Level::Circuit(gold_drive::LEVEL))
    );
    assert_eq!(
        Gain::BritDrive.own_tone(),
        Some((brit_drive::BASS, brit_drive::MIDDLE, brit_drive::TREBLE))
    );
    assert_eq!(
        Gain::GoldDrive.own_single_tone(),
        Some((gold_drive::TREBLE, "TREBLE", false))
    );
}

/// At every rate the plugin runs at, as circuits and in the slot, hot and
/// finite, and without allocating.
#[test]
fn they_are_realtime_safe_at_every_rate() {
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        for (gain, pedal) in [
            (Gain::GoldDrive, Pedal::None),
            (Gain::BritDrive, Pedal::None),
            (Gain::CleanBoost, Pedal::None),
            (Gain::Clean, Pedal::GoldDrive),
            (Gain::Clean, Pedal::BritDrive),
            (Gain::Clean, Pedal::CleanBoost),
        ] {
            chain.set_rate(rate);
            chain.apply(&Settings {
                gain,
                drive: 1.0,
                tone: Stack::Off,
                oversampling: 1,
                pedal: PedalSettings::centred(pedal, 1.0, 0.8),
                ..Settings::default()
            });
            chain.settle();
            chain.reset();
            chain.find_operating_point();
            assert_no_heap(|| {
                for k in 0..4_096 {
                    let x = 0.97 * (k as f64 * std::f64::consts::TAU * 330.0 / rate).sin();
                    assert!(chain.process(x).is_finite(), "{gain:?} {pedal:?} {rate}");
                }
            });
        }
    }
}
