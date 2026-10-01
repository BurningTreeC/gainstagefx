//! The American SVT (Ampeg D 591719 D) and the American 6550 (D 591720 H)
//! against their drawings, Ampeg's calibration procedure and Ampeg's own
//! published control ranges. See `docs/models/american_svt.md`;
//! `examples/american_svt_op.rs` and `examples/svt_op.rs` print what these
//! hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::american_svt as svt;
use gainstagefx::circuits::power::{self, FollowerFront, PowerSpec};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::{Circuit, PowerAmp};
use gainstagefx::voice::{Chain, Gain, PowerModel, Settings, Throw, Tone as Stack};
use nice_plug::prelude::Enum;

const RATE: f64 = 96_000.0;

fn preamp_gain(controls: &[(usize, f64)], hz: f64) -> f64 {
    preamp_gain_with(controls, &[], hz)
}

/// With some switch contacts set, `(slot, value)`.
fn preamp_gain_with(controls: &[(usize, f64)], values: &[(usize, f64)], hz: f64) -> f64 {
    let mut s = Simulation::new(svt::tap(10_000.0, 1_000_000.0, "out").unwrap(), RATE);
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    for &(slot, v) in values {
        s.set_value(slot, v);
    }
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, 1e-3);
    measure::run(t, (RATE / 2.0) as usize, |x| s.process(x)).gain_db()
}

const NOON: [(usize, f64); 4] = [
    (svt::VOLUME, 0.3),
    (svt::BASS, 0.5),
    (svt::MIDDLE, 0.5),
    (svt::TREBLE, 0.5),
];

fn with(control: usize, v: f64) -> Vec<(usize, f64)> {
    let mut k = NOON.to_vec();
    k.retain(|(c, _)| *c != control);
    k.push((control, v));
    k
}

/// The loop's operating point, which the sheet's voltages pin and which does
/// not depend on the 12DW7-era figures: V3b's cathode at 20 V, V4a's at
/// 1.2 V, V4b's at 135 V.
#[test]
fn the_preamp_sits_where_the_sheet_does() {
    let c = svt::tap(10_000.0, 1_000_000.0, "out").unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let (k3b, k4a, k4b, k1) = (at("k3b"), at("k4a"), at("k4b"), at("k1"));
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    let v = |i| s.voltage_at(i);
    println!(
        "k1 {:.2} k3b {:.2} k4a {:.2} k4b {:.1}",
        v(k1),
        v(k3b),
        v(k4a),
        v(k4b)
    );
    assert!((v(k1) - 1.75).abs() < 0.2);
    assert!((v(k3b) - 20.0).abs() < 2.5);
    assert!((v(k4a) - 1.2).abs() < 0.15);
    assert!((v(k4b) - 135.0).abs() < 8.0);
}

/// "The available frequencies are 220 Hz (left side of the switch engaged),
/// 800 Hz (switch in the center position) ... up to 20 dB of boost, or 20 dB
/// of cut at the selected frequency" -- Ampeg, SVT-VR owner's manual. The
/// centre position, which is the one built, with the toroid's ESTIMATED
/// sections.
#[test]
fn the_midrange_is_twenty_decibels_either_way_at_800_hz() {
    let centre = |hz| preamp_gain(&NOON, hz);
    let up = |hz| preamp_gain(&with(svt::MIDDLE, 1.0), hz) - centre(hz);
    let down = |hz| preamp_gain(&with(svt::MIDDLE, 0.0), hz) - centre(hz);
    let (u, d) = (up(800.0), down(800.0));
    println!("800 Hz: {u:+.1} / {d:+.1}");
    assert!((17.0..24.0).contains(&u), "{u}");
    assert!((-24.0..-17.0).contains(&d), "{d}");
    // Centred there: an octave either side is well down on the peak.
    for hz in [400.0, 1_600.0] {
        assert!(up(hz) < u - 8.0, "{hz}");
    }
    // And it leaves the bottom alone.
    assert!(up(60.0).abs() < 1.5);
}

/// Each control goes the way its knob is named and the sheet's arrows say:
/// up is more. BASS measured at 40 Hz and TREBLE at 4 kHz, where Ampeg rates
/// them (+/-12 dB on the SVT-VR). The parts as drawn give more treble range
/// than that -- the James stack's insertion loss is what a full treble boost
/// recovers -- so this holds direction and a floor, not Ampeg's figure.
#[test]
fn bass_and_treble_go_the_way_they_are_named() {
    for (control, hz) in [(svt::BASS, 40.0), (svt::TREBLE, 4_000.0)] {
        let centre = preamp_gain(&NOON, hz);
        let up = preamp_gain(&with(control, 1.0), hz) - centre;
        let down = preamp_gain(&with(control, 0.0), hz) - centre;
        println!("control {control} at {hz} Hz: {up:+.1} / {down:+.1}");
        assert!(up > 9.0 && down < -9.0, "control {control}");
    }
}

/// ULTRA HI is a bright capacitor across the volume: it lifts the top, more
/// with the volume down, which is what Ampeg says of it.
#[test]
fn ultra_hi_lifts_the_top_more_at_low_volume() {
    let lift = |volume: f64| {
        let mut on = Simulation::new(svt::tap(10_000.0, 1_000_000.0, "out").unwrap(), RATE);
        let mut off = Simulation::new(svt::tap(10_000.0, 1_000_000.0, "out").unwrap(), RATE);
        for s in [&mut on, &mut off] {
            for &(c, v) in &with(svt::VOLUME, volume) {
                s.set_control(c, v);
            }
        }
        off.set_value(svt::ULTRA_HI_SLOT, svt::ULTRA_HI_OFF_FARADS);
        let mut g = [0.0; 2];
        for (k, s) in [&mut on, &mut off].into_iter().enumerate() {
            s.find_operating_point();
            let t = Tone::near(RATE, 16_384, 8_000.0, 1e-3);
            g[k] = measure::run(t, (RATE / 2.0) as usize, |x| s.process(x)).gain_db();
        }
        g[0] - g[1]
    };
    let (low, high) = (lift(0.15), lift(0.9));
    println!("ultra hi at 8 kHz: volume 0.15 {low:+.1} dB, 0.9 {high:+.1} dB");
    assert!(low > 6.0);
    assert!(high < low - 3.0);
}

fn thrown(slots: &[usize], values: &[f64]) -> Vec<(usize, f64)> {
    slots.iter().copied().zip(values.iter().copied()).collect()
}

/// MIDRANGE SELECT: "220 Hz (left side of the switch engaged), 800 Hz (switch
/// in the center position), or 3 kHz (right side of the switch engaged)" --
/// Ampeg, SVT-VR manual. The boost centres on each, about 20 dB deep.
#[test]
fn the_midrange_select_moves_the_band_to_220_800_and_3000_hz() {
    let grid = [
        100.0, 150.0, 220.0, 330.0, 500.0, 800.0, 1_200.0, 2_000.0, 3_000.0, 4_500.0,
    ];
    for (position, want) in [(0, 220.0), (1, 800.0), (2, 3_000.0)] {
        let v = thrown(&svt::MID_SELECT_SLOTS, &svt::MID_SELECT[position]);
        let lifts: Vec<f64> = grid
            .iter()
            .map(|&f| {
                preamp_gain_with(&with(svt::MIDDLE, 1.0), &v, f) - preamp_gain_with(&NOON, &v, f)
            })
            .collect();
        let (peak, at) =
            lifts.iter().zip(grid).fold(
                (f64::MIN, 0.0),
                |(m, a), (&l, f)| if l > m { (l, f) } else { (m, a) },
            );
        println!("position {position}: {peak:+.1} dB at {at} Hz");
        assert_eq!(at, want);
        assert!(peak > 17.0, "{peak}");
    }
}

/// BASS SELECT. BASS CUT, C4 and C5 alone into VR1, takes the bottom away and
/// leaves the top; ULTRA LO, the twin-T, notches the middle near R8 and C4's
/// 530 Hz and keeps the bottom. Ampeg gives the Heritage SVT-CL's ULTRA LO as
/// -10 dB at 500 Hz and +2 dB at 40 Hz; this, the 1975 drawing's network,
/// notches deeper (about -18 dB) and keeps 40 Hz within 3 dB rather than
/// lifting it. Recorded in the log, not tuned.
#[test]
fn the_bass_select_cuts_the_bottom_or_scoops_the_middle() {
    let relative = |position: usize, hz: f64| {
        let v = thrown(&svt::BASS_SELECT_SLOTS, &svt::BASS_SELECT[position]);
        preamp_gain_with(&NOON, &v, hz) - preamp_gain(&NOON, hz)
    };
    let (cut_low, cut_high) = (relative(0, 40.0), relative(0, 3_000.0));
    let (lo_low, lo_mid) = (relative(2, 40.0), relative(2, 500.0));
    println!("BASS CUT: 40 Hz {cut_low:+.1}, 3 kHz {cut_high:+.1}; ULTRA LO: 40 Hz {lo_low:+.1}, 500 Hz {lo_mid:+.1}");
    assert!(cut_low < -10.0 && cut_high.abs() < 1.5);
    assert!(lo_mid < -10.0 && lo_low > -4.0);
}

/// The panel's selectors reach the circuit through `apply`, on the audio
/// thread: thrown both ways, the output changes, and nothing allocates.
#[test]
fn the_switches_are_thrown_from_the_panel_without_allocating() {
    let rms = |chain: &mut Chain| {
        let mut sum = 0.0;
        for k in 0..8_192 {
            let x = 0.1 * (k as f64 * std::f64::consts::TAU * 500.0 / 48_000.0).sin();
            let y = chain.process(x);
            if k > 4_096 {
                sum += y * y;
            }
        }
        sum.sqrt()
    };
    let mut chain = Chain::new(48_000.0);
    let base = Settings {
        gain: Gain::AmericanSvt,
        drive: 0.3,
        tone: Stack::Off,
        oversampling: 1,
        ..Settings::default()
    };
    chain.apply(&base);
    chain.settle();
    chain.reset();
    chain.find_operating_point();
    let centre = rms(&mut chain);
    let thrown = Settings {
        low_switch: Throw::Right,
        mid_switch: Throw::Left,
        ..base
    };
    let mut scooped = 0.0;
    assert_no_heap(|| {
        chain.apply(&thrown);
        scooped = rms(&mut chain);
        chain.apply(&base);
    });
    println!("500 Hz: centre {centre:.4}, ULTRA LO and 220 Hz {scooped:.4}");
    assert!(scooped < centre * 0.7, "{centre} {scooped}");
}

fn power_idle(spec: &PowerSpec) -> (Simulation, [usize; 8]) {
    let c = power::build(spec, 1_000.0).unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let idx = [
        at("ht"),
        at("scr"),
        at("f_neg"),
        at("pl_a"),
        at("ov1"),
        at("pl_b"),
        at("ov2"),
        at("f_fk1"),
    ];
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    (s, idx)
}

/// The calibration procedure: 0.072 V across each bank's 1 ohm, so 24 mA a
/// valve, with A, E and D at the drawing's 660 V, 350 V and -150 V. The bias
/// control's rest and the three open-circuit voltages are fitted to exactly
/// this by `examples/svt_op.rs`; the followers' cathodes then land near the
/// drawing's -47 V without being asked.
#[test]
fn the_power_stage_idles_where_the_procedure_sets_it() {
    let spec = PowerSpec::SVT_6550;
    let (s, [ht, scr, neg, pa, va, pb, vb, fk]) = power_idle(&spec);
    let v = |i| s.voltage_at(i);
    let r = spec.plate_resistor;
    let (ia, ib) = ((v(pa) - v(va)) / r / 3.0, (v(pb) - v(vb)) / r / 3.0);
    println!(
        "A {:.1} E {:.1} D {:.1}; {:.2} / {:.2} mA a valve; follower {:.1}",
        v(ht),
        v(scr),
        v(neg),
        ia * 1e3,
        ib * 1e3,
        v(fk)
    );
    assert!((v(ht) - 660.0).abs() < 1.0);
    assert!((v(scr) - 350.0).abs() < 1.0);
    assert!((v(neg) + 150.0).abs() < 1.0);
    for i in [ia, ib] {
        assert!((i - 24e-3).abs() < 0.5e-3, "{i}");
    }
    assert!((v(fk) + 47.0).abs() < 6.0);
    // The rest itself is a real position on the track.
    assert!((0.0..1.0).contains(&FollowerFront::SVT.bias_rest));
}

fn rms_at(spec: &PowerSpec, node: &str, volts: f64) -> f64 {
    let c = power::tap(spec, 1_000.0, "spk").unwrap();
    let i = c.unknown_named(node).unwrap();
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    let (n, from) = ((RATE * 0.5) as usize, (RATE * 0.25) as usize);
    let (mut sum, mut sq) = (0.0, 0.0);
    for k in 0..n {
        let x = volts * 2f64.sqrt() * (std::f64::consts::TAU * 1_000.0 * k as f64 / RATE).sin();
        s.process(x);
        if k >= from {
            let v = s.voltage_at(i);
            sum += v;
            sq += v * v;
        }
    }
    let m = sum / (n - from) as f64;
    (sq / (n - from) as f64 - m * m).sqrt()
}

/// The drawing's boxed AC voltages at full output: 0.257 V in, 34.6 V across
/// 4 ohm (299 W). The model gives about 0.7 dB less -- its V1 is the 12AX7 the
/// sheet names, and the sheet's own V1 voltages are flagged "approximate" --
/// and the stages after the inverter land on their boxes: 37.5 V at the
/// follower's grid for the inverter's output is 7.3 times, the followers pass
/// 0.885 of it, and the transformer's ratio is the drawing's.
#[test]
fn the_power_stage_makes_three_hundred_watts_from_the_drawings_input() {
    let spec = PowerSpec::SVT_6550;
    let out = rms_at(&spec, "spk", 0.257);
    println!("{out:.2} V rms, {:.0} W", out * out / 4.0);
    assert!((29.0..37.0).contains(&out), "{out}");
    let (pi, fg, fk) = (
        rms_at(&spec, "f_p2", 0.257),
        rms_at(&spec, "f_fg1", 0.257),
        rms_at(&spec, "f_fk1", 0.257),
    );
    println!(
        "driver gain {:.2} (7.30), follower {:.3} (0.885)",
        fg / pi,
        fk / fg
    );
    assert!((fg / pi - 7.3).abs() < 0.8);
    assert!((fk / fg - 0.885).abs() < 0.05);
}

/// The output grids are held, not coupled: driven to three times full output
/// the followers never draw grid current -- their grids stay well below their
/// cathodes -- while the output grids go positive, into class AB2, held by
/// their 47 k stoppers. (The 12BH7 gain stages ahead of the followers are
/// capacitor-coupled and do block; `examples/svt_lf.rs` prints that.)
#[test]
fn the_followers_drive_the_output_grids_positive_without_grid_current_of_their_own() {
    let c = power::tap(&PowerSpec::SVT_6550, 1_000.0, "spk").unwrap();
    let at = |n: &str| c.unknown_named(n).unwrap();
    let (fg, fk, og) = (at("f_fg1"), at("f_fk1"), at("og1"));
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    let (mut vgk, mut grid) = (f64::MIN, f64::MIN);
    let peak = 0.257 * 3.0 * 2f64.sqrt();
    for k in 0..(RATE * 0.2) as usize {
        s.process(peak * (std::f64::consts::TAU * 110.0 * k as f64 / RATE).sin());
        vgk = vgk.max(s.voltage_at(fg) - s.voltage_at(fk));
        grid = grid.max(s.voltage_at(og));
    }
    println!("follower grid-cathode at most {vgk:+.2} V; output grid at most {grid:+.2} V");
    assert!(vgk < -5.0, "{vgk}");
    assert!(grid > 0.0 && grid < 5.0, "{grid}");
}

/// Registered under the roadmap's ids, matched to its own stage, with its
/// own three tone controls and its ULTRA HI on the Bright switch.
#[test]
fn it_is_registered_with_its_own_stage_and_controls() {
    let circuits = Circuit::ids().unwrap();
    assert_eq!(circuits[Circuit::AmericanSvt.to_index()], "amp_ampeg_svt");
    let powers = PowerAmp::ids().unwrap();
    assert_eq!(powers[PowerAmp::Svt6550.to_index()], "power_svt_6550");
    let gain = Circuit::AmericanSvt.voice();
    assert_eq!(gain, Gain::AmericanSvt);
    assert!(gain.is_modelled());
    assert_eq!(
        PowerAmp::Matched.voice().resolved(gain),
        Some(PowerModel::Svt6550)
    );
    assert_eq!(gain.own_tone(), Some((svt::BASS, svt::MIDDLE, svt::TREBLE)));
    assert_eq!(gain.drive_name(), "VOLUME");
    assert!(gain.level_control().is_none());
    let bright = gain.bright_switch().unwrap();
    assert_eq!(bright.slot, svt::ULTRA_HI_SLOT);
    assert_eq!(
        gain.low_switch().unwrap().labels,
        ["Bass Cut", "Off", "Ultra Lo"]
    );
    assert_eq!(
        gain.mid_switch().unwrap().labels,
        ["220 Hz", "800 Hz", "3 kHz"]
    );
    assert!(Gain::Plexi.low_switch().is_none() && Gain::Plexi.mid_switch().is_none());
}

/// At every rate the plugin runs at, through its own power stage, hot and
/// finite, and without allocating.
#[test]
fn it_is_realtime_safe_at_every_rate() {
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        chain.apply(&Settings {
            gain: Gain::AmericanSvt,
            drive: 0.8,
            tone: Stack::Off,
            oversampling: 1,
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        assert_no_heap(|| {
            for k in 0..4_096 {
                let x = 0.5 * (k as f64 * std::f64::consts::TAU * 55.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate}");
            }
        });
    }
}
