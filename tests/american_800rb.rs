//! The American 800RB and American SS 800 (Gallien-Krueger 800RB, preamp
//! 406-0045-C and power amplifiers 406-0044-B) against GK's own service
//! manual. See `docs/models/american_800rb.md`; `examples/american_800rb_op.rs`
//! and `examples/american_ss800_op.rs` print what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::circuits::american_800rb::{self as gk, tap};
use gainstagefx::circuits::american_ss800::{self as ss, tap_with};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::{Circuit, PowerAmp};
use gainstagefx::voice::{
    AcousticSettings, CabinetChoice, Chain, Gain, PowerModel, Settings, Throw, Tone as Stack,
};
use nice_plug::prelude::Enum;

const RATE: f64 = 96_000.0;

fn rms(node: &str, controls: &[(usize, f64)], values: &[(usize, f64)], hz: f64, vin: f64) -> f64 {
    let mut s = Simulation::new(tap(600.0, 1_000_000.0, node).unwrap(), RATE);
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    for &(slot, v) in values {
        s.set_value(slot, v);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, vin * 2f64.sqrt());
    let settle = (RATE * if hz < 100.0 { 1.0 } else { 0.4 }) as usize;
    measure::run(t, settle, |x| s.process(x))
        .fundamental()
        .magnitude()
        / 2f64.sqrt()
}

const ALL_TEN: [(usize, f64); 6] = [
    (gk::VOLUME, 1.0),
    (gk::BASS, 1.0),
    (gk::LO_MID, 1.0),
    (gk::HI_MID, 1.0),
    (gk::TREBLE, 1.0),
    (gk::BOOST, 1.0),
];

fn noon(control: usize, at: f64) -> Vec<(usize, f64)> {
    let mut c = vec![
        (gk::VOLUME, 1.0),
        (gk::BASS, 0.5),
        (gk::LO_MID, 0.5),
        (gk::HI_MID, 0.5),
        (gk::TREBLE, 0.5),
        (gk::BOOST, 0.5),
    ];
    c.retain(|&(k, _)| k != control);
    c.push((control, at));
    c
}

/// The drawing's own test: 2 mV at 200 Hz in, every tone control and the
/// volume on 10, the boost on 10, the filters out, and an RMS voltage printed
/// at every stage. Within 2 dB of each -- they are meter readings -- and the
/// drain's 1.1 V within 1 dB.
#[test]
fn the_stages_carry_the_drawings_voltages() {
    for (node, want, within) in [
        ("u1a", 19e-3, 2.0),
        ("u2p", 18e-3, 2.0),
        ("u2a", 107e-3, 2.0),
        ("o3", 115e-3, 2.0),
        ("o4", 115e-3, 2.0),
        ("o5", 150e-3, 2.0),
        ("bi", 93e-3, 2.0),
        ("bl", 10e-3, 2.0),
        ("bus", 1.1, 1.0),
    ] {
        let got = rms(node, &ALL_TEN, &[], 200.0, 2e-3);
        let db = 20.0 * (got / want).log10();
        println!(
            "{node}: {:.2} mV against {:.0} mV ({db:+.2} dB)",
            got * 1e3,
            want * 1e3
        );
        assert!(db.abs() < within, "{node}: {db:+.2} dB");
    }
}

/// The operator's pages: LOW MID at 250 Hz, HIGH MID at 1 kHz, BASS at the
/// bottom and TREBLE at the top -- each band's boost is largest at its own
/// centre, and the cut is its mirror there.
#[test]
fn the_four_bands_work_where_the_manual_puts_them() {
    let at = |control: usize, setting: f64, hz: f64| {
        20.0 * (rms("o5", &noon(control, setting), &[], hz, 1e-4)
            / rms("o5", &noon(control, 0.5), &[], hz, 1e-4))
        .log10()
    };
    for (control, centre, away) in [
        (gk::LO_MID, 250.0, [62.5, 1_000.0]),
        (gk::HI_MID, 1_000.0, [250.0, 4_000.0]),
    ] {
        let peak = at(control, 1.0, centre);
        let dip = at(control, 0.0, centre);
        println!("band at {centre} Hz: {peak:+.1} / {dip:+.1} dB");
        assert!(peak > 5.0 && dip < -8.0, "{peak} {dip}");
        for hz in away {
            assert!(
                at(control, 1.0, hz) < peak - 2.5,
                "{centre}: not peaked against {hz}"
            );
        }
    }
    let (bass, treble) = (at(gk::BASS, 1.0, 60.0), at(gk::TREBLE, 1.0, 4_000.0));
    println!("bass {bass:+.1} dB at 60 Hz, treble {treble:+.1} dB at 4 kHz");
    assert!(bass > 8.0 && at(gk::BASS, 1.0, 1_000.0).abs() < 1.0);
    assert!(treble > 10.0 && at(gk::TREBLE, 1.0, 200.0).abs() < 1.0);
}

/// The voicing switches: Mid Contour "a notch at about 500Hz", Lo Cut "a bass
/// roll-off", Hi Boost "edge" -- which bridges the volume, so it shows with the
/// volume down and not at 10 -- and -10 dB, R3 across R5: the drawn values
/// give 13.8 dB (GK's sensitivity figures, 2 and 6 mV, would be 9.5).
#[test]
fn the_voicing_switches_do_what_the_manual_says() {
    let base = noon(gk::BASS, 0.5);
    let ratio = |values: &[(usize, f64)], controls: &[(usize, f64)], hz: f64| {
        20.0 * (rms("bi", controls, values, hz, 1e-4) / rms("bi", controls, &[], hz, 1e-4)).log10()
    };
    let contour: Vec<(usize, f64)> = gk::CONTOUR_SLOTS
        .iter()
        .zip(gk::CONTOUR[0])
        .map(|(&s, v)| (s, v))
        .collect();
    let notch = ratio(&contour, &base, 500.0);
    let (below, above) = (
        ratio(&contour, &base, 100.0),
        ratio(&contour, &base, 3_000.0),
    );
    println!("contour: {notch:+.1} dB at 500 Hz, {below:+.1} at 100, {above:+.1} at 3 k");
    assert!(notch < -7.0 && below > -2.0 && above > -2.0);
    let lo_cut = [(gk::LO_CUT_SLOTS[0], gk::LO_CUT[0][0])];
    let cut = (ratio(&lo_cut, &base, 40.0), ratio(&lo_cut, &base, 1_000.0));
    println!("lo cut: {:+.1} dB at 40 Hz, {:+.1} at 1 kHz", cut.0, cut.1);
    assert!(cut.0 < -10.0 && cut.1.abs() < 1.0);
    let low_volume = noon(gk::VOLUME, 0.3);
    let hi_boost = [(gk::HI_BOOST_SLOT, gk::SWITCH_CLOSED)];
    let edge = ratio(&hi_boost, &low_volume, 6_000.0);
    let at_ten = ratio(&hi_boost, &base, 6_000.0);
    println!("hi boost at 6 kHz: {edge:+.1} dB with the volume at 0.3, {at_ten:+.1} at 10");
    assert!(edge > 3.0 && at_ten.abs() < 0.5);
    let pad = ratio(&[(gk::PAD_SLOT, gk::SWITCH_CLOSED)], &base, 1_000.0);
    println!("-10 dB: {pad:+.1} dB");
    assert!((pad + 13.8).abs() < 0.5);
}

fn power(load: f64, at: &str) -> Simulation {
    let mut s = Simulation::new(
        tap_with(1_000.0, load, ss::RAIL_OPEN, ss::RAIL_SOURCE, at).unwrap(),
        RATE,
    );
    s.find_operating_point();
    s
}

/// The 300 W amplifier rests where the drawing and GK's procedure put it: the
/// output on 0 V, U1 near its -8.8 V, the drivers' bases a volt and a quarter
/// either side, and the bias at its rest giving the procedure's 5 mV across
/// each 0.33 ohm (15 mA a device). And it stays there in silence: U1 built as
/// an ideal op-amp latched to a rail within a tenth of a second, which is why
/// it is built with the LF353's own bandwidth.
#[test]
fn the_power_stage_idles_where_gk_sets_it_and_stays() {
    let c = tap_with(1_000.0, 4.0, ss::RAIL_OPEN, ss::RAIL_SOURCE, "out").unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let (out, uo, vas, bot, eh) = (at("out"), at("uo"), at("vas"), at("bot"), at("eh"));
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    let idle = (s.voltage_at(eh) - s.voltage_at(out)) / (0.33 / 3.0) / 3.0;
    println!(
        "out {:.3} V, U1 {:.2} V, VAS {:.2} V, lower base {:.2} V, {:.2} mA a device",
        s.voltage_at(out),
        s.voltage_at(uo),
        s.voltage_at(vas),
        s.voltage_at(bot),
        idle * 1e3
    );
    assert!(s.voltage_at(out).abs() < 0.05);
    assert!((s.voltage_at(uo) + 8.8).abs() < 1.0);
    assert!((s.voltage_at(vas) - 1.1).abs() < 0.4 && (s.voltage_at(bot) + 1.1).abs() < 0.4);
    assert!((idle - 15e-3).abs() < 1.5e-3, "{idle}");
    let mut drift: f64 = 0.0;
    for _ in 0..(RATE as usize) {
        s.process(0.0);
        drift = drift.max(s.voltage_at(out).abs());
    }
    println!("a second of silence: the output moved {drift:.4} V at most");
    assert!(drift < 0.05);
}

/// Its gain is the feedback network's, 1 + 56 k / (12 k || 1 k): 35.8 dB, the
/// drawing's 0.65 V to 41 V (36 dB); and it clips into 4 ohm where GK's
/// turn-on procedure has it, 36 V rms "at slight clipping", which the rails'
/// resistance is fitted to. Into 8 ohm, above the rated 200 W (40 V rms).
#[test]
fn the_power_stage_has_gks_gain_and_clips_where_gk_says() {
    let run = |load: f64, vin: f64| {
        let mut s = power(load, "out");
        let t = Tone::near(RATE, 16_384, 200.0, vin * 2f64.sqrt());
        let m = measure::run(t, (RATE * 0.25) as usize, |x| s.process(x));
        (m.fundamental().magnitude() / 2f64.sqrt(), m.thd_percent())
    };
    let (out, thd) = run(4.0, 0.1);
    let gain = 20.0 * (out / 0.1).log10();
    println!("gain into 4 ohm {gain:.2} dB, THD {thd:.3} %");
    assert!((gain - 36.0).abs() < 1.2 && thd < 0.1);
    let clip = |load: f64| {
        let (mut lo, mut hi) = (0.1f64, 2.0f64);
        for _ in 0..12 {
            let mid = (lo * hi).sqrt();
            if run(load, mid).1 < 1.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        run(load, lo).0
    };
    let (four, eight) = (clip(4.0), clip(8.0));
    println!("1 % THD: {four:.1} V rms into 4 ohm (GK 36), {eight:.1} into 8 ohm (rated 40)");
    assert!((four - 36.0).abs() < 1.5, "{four}");
    assert!(eight > 40.0 && eight < 50.0, "{eight}");
}

/// Registered under the roadmap's ids, its own stage on Matched and selectable
/// behind anything, its four bands and three switches on the panel.
#[test]
fn it_is_registered_with_its_stage_and_controls() {
    let circuits = Circuit::ids().unwrap();
    assert_eq!(circuits[Circuit::American800RB.to_index()], "amp_gk_800rb");
    let powers = PowerAmp::ids().unwrap();
    assert_eq!(powers[PowerAmp::AmericanSS800.to_index()], "power_gk_800rb");
    let gain = Circuit::American800RB.voice();
    assert_eq!(gain, Gain::American800RB);
    assert!(gain.is_modelled());
    assert_eq!(
        PowerAmp::Matched.voice().resolved(gain),
        Some(PowerModel::AmericanSS800)
    );
    assert_eq!(
        PowerAmp::AmericanSS800.voice().resolved(Gain::Plexi),
        Some(PowerModel::AmericanSS800)
    );
    assert_eq!(gain.drive_name(), "VOLUME");
    assert_eq!(gain.level_name(), "BOOST");
    assert_eq!(gain.own_tone(), Some((gk::BASS, gk::LO_MID, gk::TREBLE)));
    assert_eq!(gain.own_sweep(), Some((gk::HI_MID, "HI MID")));
    assert_eq!(gain.bright_switch().unwrap().on_label, "Hi Boost");
    let jacks = gain.input_jacks().unwrap();
    assert_eq!((jacks.high_label, jacks.low_label), ("Normal", "-10 dB"));
    assert_eq!(gain.low_switch().unwrap().labels, ["Lo Cut", "Flat"]);
    assert_eq!(gain.mid_switch().unwrap().labels, ["Contour", "Flat"]);
}

/// At every rate the plugin runs at, through its own 300 W stage into the
/// American 8x10 -- the speaker-loaded path -- hot, finite, and without
/// allocating, with its switches thrown while it plays.
#[test]
fn it_is_realtime_safe_at_every_rate() {
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        let base = Settings {
            gain: Gain::American800RB,
            drive: 0.7,
            master: 0.7,
            tone: Stack::Off,
            oversampling: 1,
            acoustic: AcousticSettings {
                cabinet: CabinetChoice::Model(&CabinetProfile::AMERICAN_810),
                ..AcousticSettings::default()
            },
            ..Settings::default()
        };
        chain.apply(&base);
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let thrown = Settings {
            low_switch: Throw::Left,
            mid_switch: Throw::Left,
            twin_bright: true,
            twin_low_input: true,
            ..base
        };
        let mut peak: f64 = 0.0;
        assert_no_heap(|| {
            for k in 0..4_096 {
                if k == 2_048 {
                    chain.apply(&thrown);
                }
                let x = 0.5 * (k as f64 * std::f64::consts::TAU * 55.0 / rate).sin();
                let y = chain.process(x);
                assert!(y.is_finite(), "{rate}");
                peak = peak.max(y.abs());
            }
        });
        assert!(peak > 1e-3, "{rate}: silent");
    }
}

/// Chosen behind somebody else's preamplifier -- the Plexi's here -- the 300 W
/// stage takes the panel's Master on its LO master, and plays at every rate
/// without allocating.
#[test]
fn its_stage_plays_behind_another_preamplifier() {
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        let base = Settings {
            gain: Gain::Plexi,
            power_amp: gainstagefx::voice::PowerAmp::AmericanSS800,
            drive: 0.6,
            master: 0.5,
            tone: Stack::Off,
            oversampling: 1,
            ..Settings::default()
        };
        chain.apply(&base);
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let mut peak: f64 = 0.0;
        assert_no_heap(|| {
            for k in 0..4_096 {
                let x = 0.3 * (k as f64 * std::f64::consts::TAU * 110.0 / rate).sin();
                let y = chain.process(x);
                assert!(y.is_finite(), "{rate}");
                peak = peak.max(y.abs());
            }
        });
        println!("{rate}: peak {peak:.3}");
        assert!(peak > 1e-3, "{rate}: silent");
    }
}

/// The direct-out table (`src/direct_out.rs`, written by
/// `examples/directout.rs`) is what the circuit does from its input to the
/// return, re-measured here at three drive positions so that a change to the
/// circuit cannot leave the DI levelled by a stale table.
#[test]
fn the_direct_out_table_is_the_circuits_own_gain() {
    use gainstagefx::voice::{knot_position, DIRECT_OUT_DB};
    let table = DIRECT_OUT_DB
        .iter()
        .find(|(g, _)| *g == Gain::American800RB)
        .map(|(_, t)| t)
        .unwrap();
    for knot in [8, 20, 32] {
        let drive = knot_position(knot);
        let mut s = Simulation::new(tap(10_000.0, 1_000_000.0, gk::DIRECT_OUT).unwrap(), RATE);
        s.set_control(gk::VOLUME, drive);
        s.find_operating_point();
        let t = Tone::near(RATE, 16_384, 1_000.0, 1e-3);
        let m = measure::run(t, (RATE * 0.3) as usize, |x| s.process(x));
        let db = 20.0 * (m.fundamental().magnitude() / 1e-3).log10();
        println!("drive {drive:.3}: {db:+.2} dB, table {:+.2}", table[knot]);
        assert!(
            (db - table[knot]).abs() < 0.05,
            "{knot}: {db} against {}",
            table[knot]
        );
    }
}
