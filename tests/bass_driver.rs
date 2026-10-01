//! The Bass Driver (Tech 21 SansAmp Bass Driver DI, V2) against the tracer's
//! own simulation of the traced circuit, stage by stage, and as a pedal and a
//! circuit in the chain. See `docs/models/bass_driver.md`;
//! `examples/bass_driver_op.rs` prints what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::bass_driver::{self as bd, tap};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::{Circuit, Kind, PedalModel};
use gainstagefx::voice::{Chain, Gain, Pedal, PedalSettings, Settings, Throw, Tone as Stack};
use nice_plug::prelude::Enum;

const RATE: f64 = 96_000.0;

fn gain(node: &str, controls: &[(usize, f64)], shifts: (usize, usize), hz: f64) -> f64 {
    let mut s = Simulation::new(tap(10_000.0, 1_000_000.0, node).unwrap(), RATE);
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    for (&slot, &v) in bd::BASS_SHIFT_SLOTS.iter().zip(&bd::BASS_SHIFT[shifts.0]) {
        s.set_value(slot, v);
    }
    for (&slot, &v) in bd::MID_SHIFT_SLOTS.iter().zip(&bd::MID_SHIFT[shifts.1]) {
        s.set_value(slot, v);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, 1e-4);
    let settle = (RATE * if hz < 100.0 { 1.0 } else { 0.25 }) as usize;
    measure::run(t, settle, |x| s.process(x)).gain_db()
}

fn stage(from: &str, to: &str, controls: &[(usize, f64)], shifts: (usize, usize), hz: f64) -> f64 {
    gain(to, controls, shifts, hz) - gain(from, controls, shifts, hz)
}

const BUILT: (usize, usize) = (1, 1);

/// The drive stages are two inverting op-amps sharing one track: turned up,
/// more of it is the first stage's feedback and less is in series into the
/// second, so both gain together -- 2.2 x 2.0 at the bottom (12.9 dB) and
/// 12.2 x 22 at the top (48.6 dB). The tracer's simulation: 12.9, 21.5, 28.5,
/// 35.5, 48.5 dB at the five settings. Within 0.3 dB of each, and flat.
#[test]
fn the_drive_stages_gain_as_the_trace_does() {
    for (drive, want) in [
        (0.0, 12.9),
        (0.25, 21.5),
        (0.5, 28.5),
        (0.75, 35.5),
        (1.0, 48.5),
    ] {
        let c = [(bd::DRIVE, drive)];
        let at_1k = stage("u1a", "o2", &c, BUILT, 1_000.0);
        let at_10k = stage("u1a", "o2", &c, BUILT, 10_000.0);
        println!("drive {drive}: {at_1k:.2} dB at 1 kHz, {at_10k:.2} at 10 kHz ({want})");
        assert!((at_1k - want).abs() < 0.3, "{drive}: {at_1k}");
        assert!((at_10k - at_1k).abs() < 0.2, "{drive}: not flat");
    }
}

/// Presence is U1A's loop: flat at the bottom, +27.5 dB near 8 kHz at the top
/// in the tracer's simulation, with the bass untouched.
#[test]
fn presence_lifts_the_top_as_the_trace_does() {
    let at = |p: f64, hz: f64| stage("nw", "u1a", &[(bd::PRESENCE, p)], BUILT, hz);
    let (flat, top, low) = (at(0.0, 8_000.0), at(1.0, 8_000.0), at(1.0, 100.0));
    println!("presence down {flat:.2} dB at 8 kHz; up {top:.2} at 8 kHz, {low:.2} at 100 Hz");
    assert!(flat.abs() < 0.2);
    assert!((top - 27.5).abs() < 1.0, "{top}");
    assert!(low < 2.0, "{low}");
}

/// The emulation path at minimum drive and presence, input to the second
/// low-pass: the tracer's peak near 85 Hz, a notch near 700 Hz 32 dB under
/// it, and a second peak near 2.8 kHz 7 dB under the first.
#[test]
fn the_emulation_has_the_traces_notch_and_peaks() {
    let quiet = [(bd::DRIVE, 0.0), (bd::PRESENCE, 0.0)];
    let at = |hz: f64| gain("u2a", &quiet, BUILT, hz);
    let (p1, notch, p2) = (at(85.0), at(700.0), at(2_800.0));
    println!("85 Hz {p1:.1}, 700 Hz {notch:.1}, 2.8 kHz {p2:.1}");
    assert!(
        (p1 - notch - 32.0).abs() < 2.5,
        "notch depth {}",
        p1 - notch
    );
    assert!((p1 - p2 - 7.0).abs() < 2.0, "second peak {}", p1 - p2);
    assert!(
        notch < at(500.0) && notch < at(1_000.0),
        "not a notch at 700 Hz"
    );
}

/// MID: +-18 dB at about 450 Hz with the shift up (500), about 900 Hz with it
/// down (1000) -- the tracer's simulation, and the tracer's own note that the
/// panel's 500 / 1000 Hz measure 400-450 and 800-900.
#[test]
fn mid_moves_eighteen_decibels_at_both_shifts() {
    for (shift, hz) in [(1, 450.0), (0, 900.0)] {
        let up = stage("u3b", "u3a", &[(bd::MID, 1.0)], (1, shift), hz);
        let down = stage("u3b", "u3a", &[(bd::MID, 0.0)], (1, shift), hz);
        println!("shift {shift}: {up:+.1} / {down:+.1} dB at {hz} Hz");
        assert!((up - 18.0).abs() < 1.0 && (down + 18.0).abs() < 1.0);
    }
}

/// BASS: +14 dB near 45 Hz at 40, +11 near 90 Hz at 80, cuts of -17 and -13;
/// TREBLE +17 dB at 3 kHz -- the tracer's simulation, within a decibel.
#[test]
fn bass_and_treble_move_as_the_trace_does() {
    let bass =
        |b: f64, shift: usize, hz: f64| stage("u3a", "u6b", &[(bd::BASS, b)], (shift, 1), hz);
    let checks = [
        (bass(1.0, 0, 45.0), 14.0),
        (bass(0.0, 0, 45.0), -17.0),
        (bass(1.0, 1, 90.0), 11.0),
        (bass(0.0, 1, 90.0), -13.0),
        (
            stage("u3a", "u6b", &[(bd::TREBLE, 1.0)], BUILT, 3_000.0),
            17.0,
        ),
    ];
    for (got, want) in checks {
        println!("{got:+.1} dB ({want:+})");
        assert!((got - want).abs() < 1.0, "{got} against {want}");
    }
}

/// The level rests at unity through the whole pedal, every other control at
/// noon -- Blend too -- for a guitar's chord: the slot's Level knob at noon is
/// unity with the rest of its knobs where they start, as every pedal's is.
#[test]
fn the_level_rests_at_unity() {
    let mut s = Simulation::new(bd::build(10_000.0, 470_000.0).unwrap(), 48_000.0);
    s.set_control(bd::LEVEL, bd::LEVEL_REST);
    s.set_control(bd::BLEND, 0.5);
    s.find_operating_point();
    let x = |k: usize| {
        let t = k as f64 / 48_000.0;
        0.122
            * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.5
                + (std::f64::consts::TAU * 123.5 * t).sin() * 0.3
                + (std::f64::consts::TAU * 246.9 * t).sin() * 0.2)
    };
    for k in 0..24_000 {
        s.process(x(k));
    }
    let (mut x2, mut y2) = (0.0, 0.0);
    for k in 24_000..72_000 {
        let y = s.process(x(k));
        x2 += x(k) * x(k);
        y2 += y * y;
    }
    let db = 10.0 * (y2 / x2).log10();
    println!("through the pedal at the level's rest: {db:+.2} dB");
    assert!(db.abs() < 0.5, "{db}");
}

/// Registered as a pedal and a circuit. As a pedal its five tone knobs are
/// presence, bass, mid, treble and blend; as a circuit Bass, Mid and Treble
/// are the stack's, Presence the panel's, Blend the circuit's own control
/// beyond the stack (the knob the Metal Zone's Mid Freq uses), and the two
/// shifts the panel's low and mid switches, built at 80 Hz and 500 Hz.
#[test]
fn it_is_registered_as_a_pedal_and_a_circuit() {
    let circuits = Circuit::ids().unwrap();
    assert_eq!(
        circuits[Circuit::BassDriver.to_index()],
        "pedal_sansamp_bass_driver_circuit"
    );
    let pedals = PedalModel::ids().unwrap();
    assert_eq!(
        pedals[PedalModel::BassDriver.to_index()],
        "pedal_sansamp_bass_driver"
    );
    assert_eq!(Circuit::BassDriver.kind(), Kind::Pedal);
    assert_eq!(PedalModel::BassDriver.voice(), Pedal::BassDriver);
    assert_eq!(Pedal::BassDriver.as_circuit(), Some(Gain::BassDriver));
    assert_eq!(
        Pedal::BassDriver.tone_labels(),
        [
            Some("presence"),
            Some("bass"),
            Some("mid"),
            Some("treble"),
            Some("blend")
        ]
    );
    let gain = Gain::BassDriver;
    assert!(gain.is_modelled() && gain.power_stage().is_none());
    assert_eq!(gain.own_tone(), Some((bd::BASS, bd::MID, bd::TREBLE)));
    assert_eq!(gain.own_presence(), Some(bd::PRESENCE));
    assert_eq!(gain.own_sweep(), Some((bd::BLEND, "BLEND")));
    assert!(gain.own_single_tone().is_none());
    assert_eq!(gain.low_switch().unwrap().labels, ["40 Hz", "80 Hz"]);
    assert_eq!(gain.mid_switch().unwrap().labels, ["1 kHz", "500 Hz"]);
    assert_eq!(
        gain.low_switch().unwrap().at(Throw::Centre),
        &bd::BASS_SHIFT[1]
    );
    assert_eq!(
        gain.mid_switch().unwrap().at(Throw::Centre),
        &bd::MID_SHIFT[1]
    );
}

/// In the pedal slot, its fifth tone knob reaches Blend: down, the pedal is
/// its EQ with the dry bass in it; up, the emulation. The two sound apart.
#[test]
fn the_slots_fifth_knob_is_the_blend() {
    let rate = 48_000.0;
    let run = |blend: f64| {
        let mut chain = Chain::new(rate);
        chain.apply(&Settings {
            gain: Gain::Clean,
            drive: 0.3,
            tone: Stack::Off,
            oversampling: 1,
            pedal: PedalSettings {
                pedal: Pedal::BassDriver,
                drive: 0.5,
                tone: [0.5, 0.5, 0.5, 0.5, blend],
                level: 0.5,
            },
            ..Settings::default()
        });
        chain.settle();
        let mut out = Vec::new();
        for k in 0..9_600 {
            let x = 0.1 * (k as f64 * std::f64::consts::TAU * 700.0 / rate).sin();
            out.push(chain.process(x));
        }
        let tail = &out[4_800..];
        (tail.iter().map(|v| v * v).sum::<f64>() / tail.len() as f64).sqrt()
    };
    let (dry, wet) = (run(0.0), run(1.0));
    println!("700 Hz through the slot: blend down {dry:.4}, up {wet:.4}");
    // At 700 Hz the emulation has its notch; the dry path does not.
    assert!(dry > 2.0 * wet, "{dry} {wet}");
}

/// At every rate the plugin runs at, as a circuit with Drive up and with its
/// shifts thrown from the panel, finite and without allocating.
#[test]
fn it_is_realtime_safe_at_every_rate() {
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        let base = Settings {
            gain: Gain::BassDriver,
            drive: 0.9,
            tone: Stack::Off,
            oversampling: 1,
            ..Settings::default()
        };
        chain.apply(&base);
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let thrown = Settings {
            low_switch: Throw::Left,
            mid_switch: Throw::Left,
            ..base
        };
        assert_no_heap(|| {
            for k in 0..4_096 {
                if k == 2_048 {
                    chain.apply(&thrown);
                }
                let x = 0.5 * (k as f64 * std::f64::consts::TAU * 55.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate}");
            }
        });
    }
}
