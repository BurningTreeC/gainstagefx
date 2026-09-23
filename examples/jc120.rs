//! Check the JC-120's CH-1 against the figures printed on its own schematic,
//! and against Roland's published tone-control ranges.
//!
//! `cargo run --release --example jc120`
//!
//! Nothing here is fitted. Every value in `src/circuits/jazz120.rs` is read
//! off the 1334 ppi PA BOARD sheet of the Sep. 2000 JC-120UT/JT service notes,
//! so this harness is a check rather than a search. There are three
//! independent sets of figures to check against, and they do not come from the
//! same place:
//!
//! **The sheet's own test points**, with `VOL, EQ MAX` and `BRI OFF`:
//!
//! | node | printed | as gain |
//! |---|---|---|
//! | CH1 INPUT HIGH, 1 kHz | -30 dBm, 69.3 mVpp | |
//! | J3 drain | -10 dBm, 693 mVpp | +20 dB |
//! | CN3 | +13 dBm, 9.79 Vpp | +43 dB |
//!
//! **The published input impedance**, 680 kOhm, which the gate return sets.
//!
//! The tone-control ranges are printed too -- Treble 17 dB at 10 kHz, Middle
//! 13 dB at 350 Hz, Bass 14 dB at 50 Hz, Bright 5 dB at 10 kHz -- but **they
//! describe the earlier amplifier, not this one**, and they are printed here
//! only so the size of that difference stays visible. The JC-120 of serials
//! 789500-248099 has a different CH-1: a two-JFET input compound, and a tone
//! network built on a 50 k Treble, a 100 k Bass, a 5 k Middle, an 18 k slope
//! and .0015/.47/.15 uF caps. The amplifier modelled here has a 250 k Treble,
//! a 250 k Bass, a 10 k Middle, a 100 k slope and 150 pF/.082/.056 uF, which
//! is a far more authoritative network. Expecting one set of figures to
//! describe the other is what sent the first attempt at this circuit wrong.
//! See `docs/models/jazz_120.md`.
//!
//! Run it after any change to the channel. `tests/jazz120.rs` holds the model
//! to the same numbers; this prints them so a disagreement can be read rather
//! than merely failed.
use gainstagefx::circuits::jazz120::{
    self, StackValues, BASS, BRIGHT_OFF_OHMS, BRIGHT_ON_OHMS, BRIGHT_SLOT, CHANNEL_OUT, HEAD_OUT,
    MIDDLE, TREBLE, VOLUME,
};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
/// A guitar's source impedance, and the load the main amplifier board's input
/// presents at CN3.
const SOURCE: f64 = 10_000.0;
const LOAD: f64 = 1_000_000.0;

fn level(s: &mut Simulation, hz: f64) -> f64 {
    let tone = Tone::near(RATE, 16_384, hz, 0.005);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x)).gain_db()
}

/// The sheet's own measurement condition: everything wide open, Bright off.
fn wide_open(at: &str) -> Simulation {
    let mut s = Simulation::new(jazz120::tap(SOURCE, LOAD, at).expect("builds"), RATE);
    for control in [VOLUME, TREBLE, MIDDLE, BASS] {
        s.set_control(control, 1.0);
    }
    s.set_value(BRIGHT_SLOT, BRIGHT_OFF_OHMS);
    s
}

/// The condition Roland's published control ranges are taken under: Volume at
/// centre, Bass and Treble fully clockwise, Middle fully counter-clockwise.
fn at_reference(v: StackValues, at: &str) -> Simulation {
    let mut s = Simulation::new(
        jazz120::tap_with(v, SOURCE, LOAD, at).expect("builds"),
        RATE,
    );
    s.set_control(VOLUME, 0.5);
    s.set_control(BASS, 1.0);
    s.set_control(TREBLE, 1.0);
    s.set_control(MIDDLE, 0.0);
    s.set_value(BRIGHT_SLOT, BRIGHT_OFF_OHMS);
    s
}

fn ranges(v: StackValues) -> (f64, f64, f64) {
    let mut out = [0.0; 3];
    for (i, (control, hz)) in [(TREBLE, 10_000.0), (MIDDLE, 350.0), (BASS, 50.0)]
        .into_iter()
        .enumerate()
    {
        let mut s = at_reference(v, "vol");
        s.set_control(control, 0.0);
        let low = level(&mut s, hz);
        s.set_control(control, 1.0);
        let high = level(&mut s, hz);
        out[i] = (high - low).abs();
    }
    (out[0], out[1], out[2])
}

/// The published control ranges of the *earlier* JC-120, for scale only.
const EARLIER: (f64, f64, f64) = (17.0, 13.0, 14.0);
const EARLIER_BRIGHT: f64 = 5.0;

fn main() {
    // --- the direct-current solve -------------------------------------
    let circuit = jazz120::build(SOURCE, LOAD).expect("builds");
    let mut dc = Simulation::new(circuit.clone(), RATE);
    println!(
        "jc120_dc,unknowns={},operating_point={}",
        dc.unknowns(),
        dc.find_operating_point()
    );
    for name in [
        "vo", "j3_bias", "j3_g", "j3_s", "j3_d", "q1_bias", "q1_g", "q1_s", "q1_d", "q2_e",
    ] {
        if let Some(node) = circuit.unknown_named(name) {
            println!("jc120_dc,{name}={:.2}V", dc.voltage_at(node));
        }
    }
    println!(
        "  the sheet wants J3's drain near the middle of +VO and Q2's emitter \
         with room for 9.79 Vpp\n"
    );

    // --- the sheet's own test points ----------------------------------
    let mut head = wide_open(HEAD_OUT);
    let head_db = level(&mut head, 1_000.0);
    let mut whole = wide_open(CHANNEL_OUT);
    let whole_db = level(&mut whole, 1_000.0);
    println!("the schematic's test points, VOL/EQ MAX and BRI OFF, at 1 kHz:");
    println!("  jack -> J3 drain  {head_db:>6.1} dB   (sheet: -30 dBm -> -10 dBm, +20)");
    println!("  jack -> CN3       {whole_db:>6.1} dB   (sheet: -30 dBm -> +13 dBm, +43)\n");

    // --- the published control ranges ---------------------------------
    println!("tone control ranges, Volume centre, Bass/Treble max, Middle min:");
    println!("variant                           treble  middle   bass");
    for (name, v) in [
        ("JC-120JT / new UT (C9 150 pF)", StackValues::AS_READ),
        ("old JC-120UT (C9 120 pF)", StackValues::OLD_U),
    ] {
        let (t, m, b) = ranges(v);
        println!("{name:<34}{t:>6.1}{m:>8.1}{b:>7.1}");
    }
    println!(
        "{:<34}{:>6.1}{:>8.1}{:>7.1}   <- a different stack\n",
        "earlier JC-120, published", EARLIER.0, EARLIER.1, EARLIER.2
    );

    // --- the Bright switch --------------------------------------------
    let mut bright = at_reference(StackValues::AS_READ, "vol");
    bright.set_value(BRIGHT_SLOT, BRIGHT_OFF_OHMS);
    let off = level(&mut bright, 10_000.0);
    bright.set_value(BRIGHT_SLOT, BRIGHT_ON_OHMS);
    let on = level(&mut bright, 10_000.0);
    println!(
        "BRI at 10 kHz: {:.1} dB   (earlier JC-120, published {EARLIER_BRIGHT:.1})\n",
        on - off
    );
}
