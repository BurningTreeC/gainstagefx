//! The Treble Boost (Dallas Rangemaster, stock OC44) against ElectroSmash's
//! analysis of it. See `docs/models/treble_boost.md`; `examples/treble_boost_op.rs`
//! prints what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::treble_boost::{self, BOOST};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::{Circuit, PedalModel};
use gainstagefx::voice::{Chain, Gain, Pedal, PedalSettings, Settings, Tone as Stack};
use nice_plug::prelude::Enum;

const RATE: f64 = 96_000.0;

fn at(source: f64, node: &str, boost: f64, hz: f64, volts: f64) -> measure::Measured {
    let mut s = Simulation::new(treble_boost::tap(source, 470_000.0, node).unwrap(), RATE);
    s.set_control(BOOST, boost);
    s.find_operating_point();
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 4.0) as usize, |x| s.process(x))
}

/// ElectroSmash: about 0.2 mA, and the collector "at -7.0 V" that every
/// account of the pedal agrees is its bias.
#[test]
fn it_is_biased_where_the_sources_put_it() {
    let circuit = treble_boost::build(10_000.0, 470_000.0).unwrap();
    let (e, c) = (
        circuit.unknown_named("e").unwrap(),
        circuit.unknown_named("c").unwrap(),
    );
    let mut s = Simulation::new(circuit, RATE);
    assert!(s.find_operating_point());
    let emitter_ma = -s.voltage_at(e) / 3_900.0 * 1e3;
    let collector = s.voltage_at(c);
    println!("emitter {emitter_ma:.3} mA, collector {collector:.2} V");
    assert!((0.15..0.3).contains(&emitter_ma), "{emitter_ma}");
    assert!((-7.5..-6.0).contains(&collector), "{collector}");
}

/// "Gv = 80 = 38 dB": the stage's own gain, collector over base, where C1 is
/// out of the way.
#[test]
fn the_stage_gives_its_thirty_eight_decibels() {
    let gain =
        at(1.0, "c", 1.0, 10_000.0, 1e-3).gain_db() - at(1.0, "b", 1.0, 10_000.0, 1e-3).gain_db();
    println!("{gain:.1} dB");
    assert!((36.0..40.0).contains(&gain), "{gain}");
}

/// C1's 5 nF into an input of about ten kilohms is the whole voicing: the
/// analysis puts the high-pass at 2.6 kHz. From a stiff source -- the
/// analysis has no source in it -- the response is down 3 dB from its plateau
/// somewhere between 1.5 and 3.5 kHz, and well down an octave below.
#[test]
fn the_input_capacitor_is_a_high_pass_near_two_and_a_half_kilohertz() {
    let plateau = at(1.0, "out", 1.0, 20_000.0, 1e-3).gain_db();
    let at_hz = |hz: f64| at(1.0, "out", 1.0, hz, 1e-3).gain_db() - plateau;
    let (low, high) = (at_hz(1_500.0), at_hz(3_500.0));
    println!(
        "1.5 kHz {low:+.1} dB, 3.5 kHz {high:+.1} dB, 300 Hz {:+.1}",
        at_hz(300.0)
    );
    assert!(low < -3.0 && high > -3.0, "{low} {high}");
    assert!(at_hz(300.0) < -15.0);
}

/// The knob is the collector load's wiper, so it sets how much of a fixed
/// gain reaches the jack and nothing else: off is silent, and what the
/// transistor does to a hot note is the same at any setting.
#[test]
fn boost_is_a_volume_after_a_fixed_gain() {
    let off = at(10_000.0, "out", 0.0, 5_000.0, 0.01).gain_db();
    let half = at(10_000.0, "out", 0.5, 5_000.0, 0.1);
    let full = at(10_000.0, "out", 1.0, 5_000.0, 0.1);
    println!(
        "off {off:+.1} dB; half {:+.1} dB {:.1} %; full {:+.1} dB {:.1} %",
        half.gain_db(),
        half.thd_percent(),
        full.gain_db(),
        full.thd_percent()
    );
    assert!(off < -30.0);
    assert!(full.gain_db() > half.gain_db() + 10.0);
    assert!((half.thd_percent() - full.thd_percent()).abs() < 0.2 * full.thd_percent());
}

/// In both lists under stable ids; one knob in the slot, and it is the level.
#[test]
fn it_is_a_pedal_with_one_knob_and_a_circuit() {
    let pedal_ids = PedalModel::ids().unwrap();
    assert_eq!(
        pedal_ids[PedalModel::TrebleBoost.to_index()],
        "pedal_dallas_rangemaster"
    );
    assert_eq!(PedalModel::TrebleBoost.voice(), Pedal::TrebleBoost);
    assert_eq!(PedalModel::TrebleBoost.name(), "Treble Boost");
    let circuit_ids = Circuit::ids().unwrap();
    assert_eq!(
        circuit_ids[Circuit::TrebleBoost.to_index()],
        "pedal_dallas_rangemaster_circuit"
    );
    assert_eq!(Circuit::TrebleBoost.voice(), Gain::TrebleBoost);
    assert_eq!(PedalModel::ALL[10], PedalModel::TrebleBoost);
    assert_eq!(Circuit::ALL[32], Circuit::TrebleBoost);

    assert!(!Pedal::TrebleBoost.has_drive());
    assert!(Pedal::Green808.has_drive());
    assert!(!Pedal::TrebleBoost.has_tone());
    assert_eq!(Pedal::TrebleBoost.drive_and_level_labels().1, "boost");
    assert_eq!(Gain::TrebleBoost.drive_control(), BOOST);
    assert_eq!(Gain::TrebleBoost.drive_name(), "BOOST");
    assert!(Gain::TrebleBoost.drive_is_channel_volume());
    assert!(Gain::TrebleBoost.level_control().is_none());
    assert!(Gain::TrebleBoost.power_stage().is_none());
    assert!(Gain::TrebleBoost.is_modelled());
}

/// The slot's level knob reaches the pot and its drive knob reaches nothing:
/// turning drive changes not one sample.
#[test]
fn the_slots_drive_knob_does_nothing_and_its_level_knob_is_the_boost() {
    let render = |drive: f64, level: f64| {
        let mut chain = Chain::new(48_000.0);
        chain.apply(&Settings {
            gain: Gain::Clean,
            tone: Stack::Off,
            oversampling: 1,
            pedal: PedalSettings::centred(Pedal::TrebleBoost, drive, level),
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        (0..4_096)
            .map(|k| {
                chain.process(0.05 * (k as f64 * std::f64::consts::TAU * 2_000.0 / 48_000.0).sin())
            })
            .collect::<Vec<f64>>()
    };
    assert_eq!(render(0.1, 0.5), render(0.9, 0.5));
    let quiet: f64 = render(0.5, 0.2).iter().map(|y| y * y).sum();
    let loud: f64 = render(0.5, 0.9).iter().map(|y| y * y).sum();
    assert!(loud > 2.0 * quiet, "{quiet} {loud}");
}

/// At every rate the plugin runs at, as the circuit and in the slot, hot and
/// finite, and without allocating.
#[test]
fn it_is_realtime_safe_at_every_rate() {
    let amplitude = 0.97;
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        for (gain, pedal) in [
            (Gain::TrebleBoost, Pedal::None),
            (Gain::Clean, Pedal::TrebleBoost),
        ] {
            chain.set_rate(rate);
            chain.apply(&Settings {
                gain,
                drive: 1.0,
                tone: Stack::Off,
                oversampling: 1,
                pedal: PedalSettings::centred(pedal, 0.5, 1.0),
                ..Settings::default()
            });
            chain.settle();
            chain.reset();
            chain.find_operating_point();
            assert_no_heap(|| {
                for k in 0..4_096 {
                    let x = amplitude * (k as f64 * std::f64::consts::TAU * 330.0 / rate).sin();
                    assert!(chain.process(x).is_finite(), "{gain:?} {pedal:?} {rate}");
                }
            });
        }
    }
}
