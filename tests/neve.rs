//! The 73P card against the drawings it was traced from.
//!
//! The drawings in `docs/schematics/neve_73p_*.png` carry their own text, and
//! that text states four gain figures: PRE 1 is 30 dB, and PRE 2 is 18 dB on
//! its own, 23 dB with R61 on its GAIN pin and 28 dB with R62. Every one of
//! those is arithmetic on the feedback network -- `1 + R12 / (R13 || leg)` --
//! so a model that hits all four has the network wired the way it is drawn,
//! and one that misses them does not.
//!
//! This is the test that would have caught what was wrong before: R4 was at
//! the output instead of across R5, R67 was in series instead of a bleeder,
//! and the gain switch had the input attenuator's resistors in it. The card
//! measured 156 dB down and no test said so.

use gainstagefx::circuits::neve;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

/// Small enough that every stage is well inside its linear region, so what
/// comes back is the feedback arithmetic and nothing else.
const SMALL: f64 = 1e-5;

fn measure_at(node: &str, drive: f64, volts: f64, hz: f64, iron: bool) -> measure::Measured {
    let circuit = neve::tap(150.0, 10_000.0, node, iron).expect("the card builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(neve::GAIN, drive);
    sim.set_control(neve::TRIM, 1.0);
    assert!(
        sim.find_operating_point(),
        "{node} at drive {drive} never settled"
    );
    let tone = Tone::near(RATE, 16_384, hz, volts);
    // An eighth of a second. The operating point is already found and the
    // reactances are already at it, so this is only letting the tone's own
    // transient pass; it is not waiting for the card to charge. The one place
    // that *is* waiting for the card is the recovery test below, and it says
    // so in seconds.
    measure::run(tone, (RATE / 8.0) as usize, |x| sim.process(x))
}

fn gain_at(node: &str, drive: f64) -> f64 {
    measure_at(node, drive, SMALL, 1_000.0, false).gain_db()
}

/// `1 + R12 / (R13 || R5 || R4)` = `1 + 2200 / 70.9` = 32.0, which is 30.1 dB,
/// and the drawing's text says 30 dB. R4 is 91 R across R5, and putting it
/// anywhere else -- at the output, say -- costs the block its extra 12 dB and
/// the card fifty-five decibels.
#[test]
fn pre_one_has_the_thirty_decibels_the_drawing_says() {
    let got = gain_at("pre1_out", 0.0);
    println!("PRE 1: {got:.2} dB against 30 stated");
    assert!(
        (got - 30.0).abs() < 1.5,
        "PRE 1 should be near 30 dB and is {got:.2}"
    );
}

/// The three positions the gain switch can put on PRE 2's GAIN pin, which the
/// drawing's text calls 18, 23 and 28 dB. The model's Drive knob sweeps
/// between the outer two and passes through R61 at its middle, so those are
/// the three positions to look at.
#[test]
fn pre_two_reaches_the_three_gains_its_switch_selects() {
    let pre1 = gain_at("pre1_out", 0.0);
    for (drive, stated) in [(0.0, 18.0), (0.5, 23.0), (1.0, 28.0)] {
        let both = gain_at("pre2_out", drive);
        let alone = both - pre1;
        println!("PRE 2 at drive {drive:.1}: {alone:.2} dB against {stated:.0} stated");
        assert!(
            (alone - stated).abs() < 1.5,
            "PRE 2 at drive {drive:.1} should be near {stated:.0} dB and is {alone:.2}"
        );
    }
}

/// The Drive control has to move the card, and move it in the right
/// direction. Ten decibels end to end is what R71 against R62 buys.
#[test]
fn the_drive_control_climbs_the_whole_way() {
    let mut last = f64::NEG_INFINITY;
    let mut first = 0.0;
    for i in 0..=8 {
        let drive = i as f64 / 8.0;
        let got = gain_at("out", drive);
        println!("  drive {drive:.3}: {got:.2} dB");
        assert!(
            got > last - 0.05,
            "the control went backwards at {drive:.3}: {got:.2} after {last:.2}"
        );
        if i == 0 {
            first = got;
        }
        last = got;
    }
    let span = last - first;
    assert!(
        (span - 10.0).abs() < 2.0,
        "the control should cover about ten decibels and covers {span:.2}"
    );
}

/// The input transformer is stated on the drawing to amplify by 6 dB, and it
/// is the only thing between `mic` and the first card, so taking the card with
/// and without it is the measurement of that claim. Some loss against the
/// nominal figure is the core and the loading, not a mistake.
#[test]
fn the_input_transformer_steps_the_signal_up() {
    let bare = measure_at("out", 0.0, SMALL, 1_000.0, false).gain_db();
    let with_iron = measure_at("out", 0.0, SMALL, 1_000.0, true).gain_db();
    let step = with_iron - bare;
    println!("input transformer: {step:.2} dB against 6 stated");
    assert!(
        (step - 6.0).abs() < 2.0,
        "the transformer should step up about 6 dB, not {step:.2}"
    );
}

/// A microphone preamplifier is built not to run out of room. At a nominal
/// microphone level the card should be doing nothing audible to the signal,
/// and it should only start bending well above that.
#[test]
fn the_card_is_clean_at_microphone_level_and_bends_above_it() {
    let quiet = measure_at("out", 1.0, 1e-4, 1_000.0, true);
    println!("100 uV in: {:.3} % distortion", quiet.thd_percent());
    assert!(
        quiet.thd_percent() < 0.5,
        "a hundred microvolts should be clean and makes {:.2} %",
        quiet.thd_percent()
    );
    let loud = measure_at("out", 1.0, 3e-2, 1_000.0, true);
    println!("30 mV in: {:.2} % distortion", loud.thd_percent());
    assert!(
        loud.thd_percent() > 5.0,
        "thirty millivolts into a 54 dB preamplifier should be well past the \
         rails and makes only {:.2} %",
        loud.thd_percent()
    );
}

/// Flat where a microphone preamplifier has to be flat. The corners are the
/// card's own capacitors: 22 uF into 120 k at the bottom, and the compensation
/// around each transistor at the top.
#[test]
fn the_card_is_flat_across_the_band() {
    let middle = measure_at("out", 0.0, SMALL, 1_000.0, true).gain_db();
    for hz in [20.0, 50.0, 100.0, 5_000.0, 10_000.0, 20_000.0] {
        let got = measure_at("out", 0.0, SMALL, hz, true).gain_db();
        println!("  {hz:8.0} Hz: {:+.2} dB", got - middle);
        assert!(
            (got - middle).abs() < 1.5,
            "{hz:.0} Hz is {:+.2} dB off the middle of the band",
            got - middle
        );
    }
}

/// Every node has to settle, at both ends of the control. A three-transistor
/// block with feedback from the follower's emitter back to the input
/// transistor's emitter has exactly one operating point, and if the solver
/// cannot find it the model is not a model of anything.
#[test]
fn every_node_settles_at_both_ends_of_the_control() {
    for node in ["pre1_out", "pre2_out", "out"] {
        for drive in [0.0, 0.5, 1.0] {
            let circuit = neve::tap(150.0, 10_000.0, node, true).expect("the card builds");
            let mut sim = Simulation::new(circuit, RATE);
            sim.set_control(neve::GAIN, drive);
            assert!(
                sim.find_operating_point(),
                "{node} at drive {drive} never settled"
            );
            for v in sim.operating_point() {
                assert!(
                    v.is_finite(),
                    "{node} at drive {drive} settled on a non-number"
                );
                assert!(
                    *v > -1.0 && *v < neve::SUPPLY + 1.0,
                    "{node} at drive {drive} settled at {v:.2} V, which is off the rail"
                );
            }
        }
    }
}

/// Moving the control after the card has been driven into the rails has to
/// leave it somewhere sane. The coupling capacitors are large -- 22 uF into
/// 120 k is a two and a half second corner -- so recovery is slow, which is
/// what the hardware does; what it must not do is stay broken.
#[test]
fn the_card_recovers_from_being_driven_into_the_rails() {
    let circuit = neve::build(150.0, 10_000.0).expect("the card builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(neve::TRIM, 1.0);
    let mut settled: Vec<f64> = Vec::new();
    for (round, drive) in [0.0f64, 1.0, 0.5, 0.0, 1.0].into_iter().enumerate() {
        sim.set_control(neve::GAIN, drive);
        let loud = Tone::near(RATE, 16_384, 220.0, 1.0);
        measure::run(loud, (RATE / 20.0) as usize, |x| sim.process(x));
        let quiet = Tone::near(RATE, 16_384, 1_000.0, SMALL);
        // Three seconds. Measured in `examples/neve.rs`: the card is back
        // under one per cent by two, and settled by ten. Three is past the
        // knee with room to spare and is a quarter of the test's runtime.
        let m = measure::run(quiet, (RATE * 3.0) as usize, |x| sim.process(x));
        println!(
            "round {round} at drive {drive:.2}: {:.2} dB, {:.3} %",
            m.gain_db(),
            m.thd_percent()
        );
        assert!(
            m.gain_db().is_finite(),
            "round {round} came back a non-number"
        );
        assert!(
            m.thd_percent() < 1.0,
            "round {round} at drive {drive:.2} never recovered: {:.2} % distortion",
            m.thd_percent()
        );
        settled.push(m.gain_db());
    }
    // Rounds 0 and 3 are the same setting, and so are 1 and 4. Whatever the
    // card was doing in between must not have moved them.
    assert!(
        (settled[0] - settled[3]).abs() < 0.2,
        "drive 0.0 gave {:.2} dB and then {:.2} dB",
        settled[0],
        settled[3]
    );
    assert!(
        (settled[1] - settled[4]).abs() < 0.2,
        "drive 1.0 gave {:.2} dB and then {:.2} dB",
        settled[1],
        settled[4]
    );
}

// ---------------------------------------------------------------------------
// The OUTPUT block
// ---------------------------------------------------------------------------

/// The assembly guide states two numbers about the OUTPUT block and nothing
/// else, so those two numbers are the whole test: "The OUTPUT block provides
/// 18dB of gain, with 4dB of this provided by the output transformer."
///
/// They are independent of each other and of everything the block is built
/// from, which is what makes them worth testing against. The electronic gain
/// comes out of the feedback arithmetic -- R33 3.3 k back to an emitter
/// sitting on R38 1.2 k in parallel with R39 1.5 k, so `(3300 + 667) / 667`,
/// about six times -- and the transformer's comes out of a turns ratio. Two
/// different parts of the circuit have to be right for both to land.
#[test]
fn the_output_block_makes_the_gain_the_guide_states() {
    let whole = output_gain("spk", 1e-3, 1_000.0);
    let electronic = output_gain("out", 1e-3, 1_000.0);
    let iron = whole - electronic;
    println!("block {whole:.2} dB, electronic {electronic:.2}, transformer {iron:.2}");
    assert!(
        (16.5..=19.5).contains(&whole),
        "the guide says the block gives 18 dB and it gives {whole:.2}"
    );
    assert!(
        (3.0..=5.0).contains(&iron),
        "the guide says 4 dB of that is the transformer and it is {iron:.2}"
    );
}

/// A line driver is not a colouring stage. Whatever character the card has
/// comes from the PRE blocks and from iron being driven hard; the OUTPUT block
/// is there to make the level and the current, and it has fifteen decibels of
/// feedback around it to make sure of it.
#[test]
fn the_output_block_is_clean_and_flat() {
    for volts in [0.001f64, 0.01, 0.1, 1.0] {
        let thd = output_thd("spk", volts, 1_000.0);
        assert!(
            thd < 0.5,
            "at {volts} V the output block bends {thd:.2} %, which is not a line driver"
        );
    }
    let reference = output_gain("spk", 1e-3, 1_000.0);
    for hz in [20.0, 50.0, 200.0, 1_000.0, 5_000.0] {
        let here = output_gain("spk", 1e-3, hz);
        assert!(
            (here - reference).abs() < 1.0,
            "at {hz} Hz the block is {:.2} dB off its midband",
            here - reference
        );
    }
}

/// Silence in, silence out -- and the reason this one is here rather than
/// left to `tests/voice.rs` is that the 73P is what found the bug.
///
/// The block's collector load is a transformer primary, so it carries the
/// whole of T8's standing current. `find_operating_point` seeded every
/// capacitor's charge and no inductor's current, because until this block
/// existed no inductor in the catalogue carried any: a tone stack's and a
/// cabinet's sit across the signal. Starting from zero, the amplifier spent a
/// fifth of a second climbing to its own operating point and put a thump of
/// 1.5 out through the speaker on the way.
#[test]
fn the_output_block_starts_where_it_settles() {
    let circuit = neve::output(600.0, 10_000.0).expect("builds");
    let mut sim = Simulation::new(circuit, RATE);
    assert!(
        sim.find_operating_point(),
        "the operating point did not settle"
    );
    let mut worst = 0.0f64;
    for _ in 0..(RATE as usize / 4) {
        worst = worst.max(sim.process(0.0).abs());
    }
    assert!(
        worst < 1e-3,
        "it puts out {worst:.4} on silence, which is the transformer's primary \
         charging up to the current it should have started with"
    );
}

fn output_measured(node: &str, volts: f64, hz: f64) -> measure::Measured {
    let circuit = neve::output_tap(600.0, 10_000.0, node).expect("builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.find_operating_point();
    let tone = Tone::near(RATE, 16_384, hz, volts);
    // Two seconds. The emitter and feedback capacitors are a hundred
    // microfarads against a kilohm and a half, and half a second of settle
    // left enough drift in the window to read as eight decibels of bass lift
    // and four per cent of distortion at a millivolt. Neither was real.
    measure::run(tone, (RATE * 2.0) as usize, |x| sim.process(x))
}

fn output_gain(node: &str, volts: f64, hz: f64) -> f64 {
    output_measured(node, volts, hz).gain_db()
}

fn output_thd(node: &str, volts: f64, hz: f64) -> f64 {
    output_measured(node, volts, hz).thd_percent()
}
