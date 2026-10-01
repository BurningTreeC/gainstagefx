//! The Presence knob (2026-09-25): the power stage's presence control -- the
//! AC30's cut -- which existed in every power netlist and which nothing set.
//! See `Chain::set_presence`.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::dsp::measure::{self, Tone as Probe};
use gainstagefx::params::{Circuit, GainStageParams};
use gainstagefx::presets::{self, Stored, PRESETS};
use gainstagefx::voice::{Chain, Gain, PowerAmp, PowerModel, Settings, Tone, NOMINAL_DBFS};
use nice_plug::params::Params;
use nice_plug::prelude::Enum;
use std::collections::BTreeMap;

const RATE: f64 = 48_000.0;

/// Small-signal level through the whole chain, dB, at one frequency.
fn level(gain: Gain, power_amp: PowerAmp, presence: f64, hz: f64) -> f64 {
    let mut chain = Chain::new(RATE);
    chain.apply(&Settings {
        gain,
        power_amp,
        presence,
        drive: 0.3,
        tone: Tone::Off,
        ..Settings::default()
    });
    chain.settle();
    // Far below anything that clips, so this is the stage's response.
    let amplitude = 10f64.powf((NOMINAL_DBFS - 40.0) / 20.0);
    let probe = Probe::near(RATE, 8_192, hz, amplitude);
    let m = measure::run(probe, (RATE / 4.0) as usize, |x| chain.process(x));
    20.0 * m.fundamental().magnitude().max(1e-15).log10()
}

/// How much more the top than the bottom, at one presence setting. The
/// bottom is 200 Hz: a presence lift reaches down to a kilohertz on these
/// stages, so a 500 Hz reference moves with it and understates it.
fn tilt(gain: Gain, presence: f64) -> f64 {
    level(gain, PowerAmp::Matched, presence, 4_000.0)
        - level(gain, PowerAmp::Matched, presence, 200.0)
}

/// Turned up, a presence control takes treble out of the feedback loop and so
/// puts it back into the output. On the 2203's stage (22 k presence, .1 uF),
/// the 2205's and the 5150's, the top rises against the bottom -- measured
/// 2026-09-25: +4.4, +4.0 and +5.9 dB at 4 kHz, rising to +4.6, +3.9 and +8.0
/// at 10 kHz (the 1959's, with more feedback, +9.6 at 4 kHz; +11.8 since
/// 2026-09-30, when its capacitor moved to the wiper as drawn and zero became
/// off).
#[test]
fn presence_up_brightens_the_top() {
    for gain in [Gain::Brit800, Gain::Brit2205, Gain::Peavey] {
        let (down, up) = (tilt(gain, 0.0), tilt(gain, 1.0));
        println!(
            "{}: 4 kHz over 200 Hz {down:+.1} dB down, {up:+.1} dB up",
            gain.name()
        );
        assert!(
            up - down > 3.0,
            "{}: presence moves the top by {:.1} dB",
            gain.name(),
            up - down
        );
    }
}

/// The AC30 has no feedback loop and no presence; the same slot is its CUT
/// control, which does the opposite: turned up, it takes treble away.
#[test]
fn the_ac30_calls_it_cut_and_it_darkens() {
    assert_eq!(PowerModel::AC30EL84.presence_name(), Some("CUT"));
    let (down, up) = (tilt(Gain::AC30, 0.0), tilt(Gain::AC30, 1.0));
    println!("AC30: 4 kHz over 200 Hz {down:+.1} dB cut down, {up:+.1} dB cut up");
    assert!(down - up > 3.0, "cut moves the top by {:.1} dB", down - up);
}

/// Where the stage has nothing in the slot the knob does nothing at all, and
/// the panel greys it: the AB763s (no presence on the amplifier), the
/// transistor stages, and no power stage. The DR103 has one since its driver
/// moved into the power stage (see `tests/dr103.rs`). The Rectifier's stages
/// have none since 2026-09-30: in the mode modelled their loop is lifted, and
/// the presence is the preamplifier's (see below).
#[test]
fn no_presence_where_the_stage_has_none() {
    for model in [
        PowerModel::American6L6Clean,
        PowerModel::AmericanDeluxe6V6,
        PowerModel::Jazz120SS,
        PowerModel::British73Out,
        PowerModel::Recto6L6,
        PowerModel::Recto6L6Tube,
    ] {
        assert_eq!(model.presence_name(), None, "{model:?}");
    }
    for model in [
        PowerModel::Cali6L6,
        PowerModel::American6L6HighGain,
        PowerModel::BritEL34,
        PowerModel::BritPlexiEL34,
        PowerModel::Brit2205EL34,
        PowerModel::DR103EL34,
        PowerModel::DR103EL34Return,
    ] {
        assert_eq!(model.presence_name(), Some("PRESENCE"), "{model:?}");
    }
    for (gain, power) in [
        (Gain::Twin, PowerAmp::Matched),
        (Gain::Deluxe, PowerAmp::Matched),
    ] {
        let a = level(gain, power, 0.0, 4_000.0);
        let b = level(gain, power, 1.0, 4_000.0);
        assert_eq!(a, b, "{}: the knob reached something", gain.name());
    }
}

/// Half way is exactly where every stage rested before the knob existed, so
/// nothing that was measured, calibrated or saved moves: the chain at 0.5 is
/// bit-identical to the chain with the knob taken to an end and brought back.
#[test]
fn half_way_is_where_it_always_rested() {
    let render = |detour: Option<f64>| {
        let mut chain = Chain::new(RATE);
        let mut s = Settings {
            gain: Gain::Brit800,
            drive: 0.6,
            tone: Tone::Off,
            ..Settings::default()
        };
        if let Some(p) = detour {
            s.presence = p;
            chain.apply(&s);
            s.presence = 0.5;
        }
        chain.apply(&s);
        chain.settle();
        (0..4_800)
            .map(|k| chain.process(0.1 * (k as f64 * 0.05).sin()))
            .collect::<Vec<f64>>()
    };
    let plain = render(None);
    assert_eq!(plain, render(Some(1.0)));
    assert_eq!(plain, render(Some(0.0)));
}

/// A preset saved before the knob existed comes back with it in the middle.
#[test]
fn an_old_preset_migrates_to_the_middle() {
    let params = GainStageParams::default();
    let mut old = Stored {
        model_ids: BTreeMap::new(),
        name: "Before Presence".into(),
        values: [("drive".into(), 0.7)].into_iter().collect(),
        built_in: false,
        group: "",
    };
    presets::migrate(&mut old, &params);
    assert_eq!(old.values["presence"], 0.5);
    assert!(params.param_map().iter().any(|(id, _, _)| id == "presence"));
    // Every shipped preset carries the knob in its dials.
    for p in PRESETS {
        assert!(
            p.dials().iter().any(|(id, _)| *id == "presence"),
            "{}",
            p.name
        );
    }
    let _ = Circuit::ALL[0].to_index();
}

/// Turned all the way either way it plays at every rate -- and turned while it
/// plays, without touching the heap. The 2205's is a shunt on its tail, the
/// Mark IIC+'s a rheostat in series with its loop (2026-09-30), and the Cali
/// Rectifier's is in its preamplifier, reached through `Gain::own_presence`.
#[test]
fn presence_plays_at_every_rate() {
    for gain in [Gain::Brit2205, Gain::Boogie, Gain::Recto] {
        let mut chain = Chain::new(44_100.0);
        for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
            let settings = |presence: f64| Settings {
                gain,
                presence,
                drive: 0.9,
                tone: Tone::Off,
                ..Settings::default()
            };
            chain.set_rate(rate);
            chain.apply(&settings(0.0));
            chain.settle();
            chain.reset();
            chain.find_operating_point();
            let before = chain.solver_breakdown();
            let mut peak = 0.0f64;
            assert_no_heap(|| {
                for k in 0..(rate as usize / 10) {
                    if k % 512 == 0 {
                        chain.apply(&settings(((k / 512) % 2) as f64));
                    }
                    let x = 0.3 * (std::f64::consts::TAU * 110.0 * k as f64 / rate).sin();
                    let y = chain.process(x);
                    assert!(y.is_finite(), "{} {rate} {k}", gain.name());
                    peak = peak.max(y.abs());
                }
            });
            let work = chain.solver_breakdown().saturating_delta(before);
            println!(
                "{} {rate}: {} passes over {} power solves",
                gain.name(),
                work.power.passes,
                work.power.solves
            );
            assert!(peak > 1e-3, "{} {rate}: {peak}", gain.name());
            assert_eq!(
                work.gain.unsettled + work.power.unsettled,
                0,
                "{} {rate}",
                gain.name()
            );
        }
    }
}

/// The presence controls as the drawings have them (2026-09-30). The 1981
/// JCM800 sheet's phase splitter prints its presence "22K LIN", and the 2205
/// has the same part; the 1959's 5 K is the bottom of the tail itself, with
/// the capacitor on its wiper (Unicord's 1970 sheets). Until then every one was
/// an audio track, and the 1959's was built as the JCM800's beside a fixed
/// 5 k -- which left the bottom half of the 2203's knob doing nothing and the
/// 1959's never quite off.
#[test]
fn the_presence_parts_are_the_drawings() {
    use gainstagefx::circuits::power::PowerSpec;
    use gainstagefx::dsp::netlist::Taper;
    for spec in [PowerSpec::BRIT_EL34, PowerSpec::BRIT_2205_EL34] {
        assert_eq!(
            (
                spec.presence_pot,
                spec.presence_cap,
                spec.presence_taper,
                spec.presence_on_tail
            ),
            (22_000.0, 0.1e-6, Taper::Linear, false),
            "{}",
            spec.name
        );
    }
    let plexi = PowerSpec::PLEXI_EL34;
    assert_eq!(
        (
            plexi.presence_pot,
            plexi.presence_cap,
            plexi.presence_taper,
            plexi.presence_on_tail
        ),
        (5_000.0, 0.1e-6, Taper::Linear, true)
    );
    assert_eq!(
        plexi.pi_tail_lower, plexi.presence_pot,
        "the track is the tail"
    );
}

/// On the 1959 the presence capacitor hangs off the pot's wiper, so with the
/// wiper at ground the capacitor is shorted and the stage answers exactly as
/// if it were not there. The builder's JCM800 topology, which stood in for it
/// until 2026-09-30, still shunted treble through the whole 5 k at zero.
#[test]
fn the_1959_presence_is_out_of_circuit_at_zero() {
    use gainstagefx::circuits::power::{self, PowerSpec, PRESENCE};
    use gainstagefx::dsp::time::Simulation;
    // Small-signal level through the power stage alone, dB, at one frequency.
    let response = |spec: &PowerSpec, presence: f64, hz: f64| {
        let circuit = power::build(spec, 10_000.0).expect("builds");
        let mut sim = Simulation::new(circuit, RATE);
        sim.set_control(PRESENCE, presence);
        sim.find_operating_point();
        let probe = Probe::near(RATE, 8_192, hz, 0.05);
        let m = measure::run(probe, (RATE / 4.0) as usize, |x| sim.process(x));
        20.0 * m.fundamental().magnitude().max(1e-15).log10()
    };
    let without = PowerSpec {
        presence_cap: 1e-15,
        ..PowerSpec::PLEXI_EL34
    };
    for hz in [200.0, 1_000.0, 4_000.0, 10_000.0] {
        let at_zero = response(&PowerSpec::PLEXI_EL34, 0.0, hz);
        let open = response(&without, 0.0, hz);
        let up = response(&PowerSpec::PLEXI_EL34, 1.0, hz);
        println!("{hz:>6} Hz: presence 0 {at_zero:+.3} dB, no capacitor {open:+.3} dB, presence 1 {up:+.3} dB");
        assert!(
            (at_zero - open).abs() < 0.05,
            "{hz} Hz: presence at zero still does {:.2} dB",
            at_zero - open
        );
    }
    assert!(
        response(&PowerSpec::PLEXI_EL34, 1.0, 4_000.0) - response(&without, 0.0, 4_000.0) > 3.0,
        "turned up it must lift the top"
    );
}

/// The Dual Rectifier's sheet (the RF-1F set, 6-93) has a mode table, and in
/// RD NORM -- the red channel's Modern mode, the one modelled -- LDR19, the
/// loop, is OFF. So the power stage has no feedback and nothing for a presence
/// to work on, and the red channel's PRSNC is in the preamplifier: R254 22 k
/// and C8 .003 into a 25 k rheostat, a treble shunt across the stack's output.
/// The knob turns that, and turned up the top comes back (2026-09-30).
#[test]
fn the_rectifier_presence_is_in_its_preamplifier() {
    use gainstagefx::circuits::{power::PowerSpec, rectifier};
    assert_eq!(PowerSpec::RECTO_6L6.feedback, 0.0);
    assert_eq!(PowerSpec::RECTO_6L6_TUBE.feedback, 0.0);
    assert_eq!(Gain::Recto.own_presence(), Some(rectifier::PRESENCE));
    // The one other circuit with a presence of its own: the Bass Driver's,
    // U1A's treble lift, which the panel's Presence knob turns.
    use gainstagefx::circuits::bass_driver;
    assert_eq!(Gain::BassDriver.own_presence(), Some(bass_driver::PRESENCE));
    for gain in Gain::ALL {
        if gain != Gain::Recto && gain != Gain::BassDriver {
            assert_eq!(gain.own_presence(), None, "{}", gain.name());
        }
    }
    let (down, up) = (tilt(Gain::Recto, 0.0), tilt(Gain::Recto, 1.0));
    println!("Recto: 4 kHz over 200 Hz {down:+.1} dB down, {up:+.1} dB up");
    assert!(
        up - down > 3.0,
        "presence moves the top by {:.1} dB",
        up - down
    );
    // It is a shunt on the top, not a level control: the bottom stays put.
    let low = |p| level(Gain::Recto, PowerAmp::Matched, p, 200.0);
    assert!(
        (low(1.0) - low(0.0)).abs() < 1.0,
        "{} {}",
        low(0.0),
        low(1.0)
    );
}

/// In a custom chain the Rectifier's presence still takes the knob, and the
/// power stage behind it rests where it was voiced: one knob, one control.
#[test]
fn a_circuit_presence_leaves_the_power_stage_at_rest() {
    let recto = |presence: f64| Settings {
        gain: Gain::Recto,
        power_amp: PowerAmp::BritEL34,
        presence,
        drive: 0.6,
        tone: Tone::Off,
        ..Settings::default()
    };
    let render = |detour: Option<Settings>, settings: Settings| {
        let mut chain = Chain::new(RATE);
        if let Some(d) = detour {
            chain.apply(&d);
        }
        chain.apply(&settings);
        chain.settle();
        (0..4_800)
            .map(|k| chain.process(0.1 * (k as f64 * 0.05).sin()))
            .collect::<Vec<f64>>()
    };
    // Behind the Brit EL34 stage, whose own presence would otherwise move.
    assert_ne!(
        render(None, recto(0.0)),
        render(None, recto(1.0)),
        "the knob reached nothing"
    );
    // The stage's presence turned right up behind its own preamplifier, then
    // the Rectifier's put in front of it with the knob anywhere: the stage's
    // pot is back at rest, so the chain is the one that never took the detour.
    let brit_up = Settings {
        gain: Gain::Brit800,
        power_amp: PowerAmp::BritEL34,
        presence: 1.0,
        drive: 0.6,
        tone: Tone::Off,
        ..Settings::default()
    };
    for presence in [0.0, 1.0] {
        assert_eq!(
            render(None, recto(presence)),
            render(Some(brit_up), recto(presence)),
            "{presence}"
        );
    }
}

/// The Mark IIC+'s presence as both redraws have it: a 250 k rheostat in
/// series with the loop, ahead of R60 1k5 and R61 56k || C61 .0047. Turned up
/// it takes the treble's feedback away first, so the top comes up -- and,
/// unlike a shunt presence, it takes some of the rest too, so the whole band
/// rises a little and the amplifier loosens (2026-09-30).
#[test]
fn the_mark_presence_is_in_series_with_its_loop() {
    use gainstagefx::circuits::power::{PowerSpec, SeriesLoop};
    let spec = PowerSpec::MARKIIC;
    assert_eq!(PowerModel::Cali6L6.presence_name(), Some("PRESENCE"));
    let l = spec.series_loop.expect("the Mark's loop");
    assert_eq!(
        (l.presence_pot, l.series, spec.feedback, l.shelf_cap),
        (250_000.0, 1_500.0, 56_000.0, 0.0047e-6)
    );
    assert_eq!(
        (spec.pi_tail, l.tail_tap, spec.pi_tail_lower, l.tail_bypass),
        (22_000.0, 1_500.0, 3_300.0, 0.047e-6)
    );
    assert_eq!(spec.presence_pot, 0.0, "no shunt presence as well");
    let _ = SeriesLoop::MARK_IIC_PLUS;
    let (down, up) = (tilt(Gain::Boogie, 0.0), tilt(Gain::Boogie, 1.0));
    let low = |p| level(Gain::Boogie, PowerAmp::Matched, p, 200.0);
    println!(
        "Mark IIC+: 4 kHz over 200 Hz {down:+.1} dB down, {up:+.1} dB up; 200 Hz {:+.1} -> {:+.1} dB",
        low(0.0),
        low(1.0)
    );
    assert!(
        up - down > 1.0,
        "presence moves the top by {:.1} dB",
        up - down
    );
    assert!(low(1.0) > low(0.0), "less loop is more level");
}
