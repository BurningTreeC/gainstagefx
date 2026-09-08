//! The power amplifier: phase inverter, output tubes, transformer, feedback.
//!
//! Every amplifier in this catalogue stopped at its preamplifier, and said so.
//! That is a real gap rather than a tidy boundary, because most of what
//! separates a hundred and twenty watt amplifier from a distortion pedal
//! happens after the preamplifier: a long-tailed pair that clips
//! asymmetrically and recovers slowly, output tubes whose grids draw current
//! and shift their own bias, a transformer whose core runs out of iron on a low
//! note, a supply that sags under the current the loud passages ask for, and a
//! feedback loop around all of it that gets weaker as the loop clips.
//!
//! None of that is a waveshaper. All of it is level dependent in a way a
//! preamplifier is not, which is why an amplifier's power stage is what makes
//! it feel like an amplifier rather than like a fuzz box.
//!
//! ## What is modelled
//!
//! ```text
//! in -- coupling -- long-tailed pair (two triodes, shared tail)
//!                     |                    |
//!                  coupling             coupling
//!                     |                    |
//!             grid leak to BIAS    grid leak to BIAS
//!                     |                    |
//!              output tubes (n)      output tubes (n)
//!                  screens fed from a sagging screen supply
//!                     |                    |
//!                +----+--- primary ---+----+
//!                |  magnetising branch, saturating
//!                |  centre tap to a sagging plate supply
//!                +--- ideal ratio ---> secondary --> speaker
//!                                          |
//!                                    feedback network
//!                                    back to the tail
//! ```
//!
//! ## What is not
//!
//! - **The rectifier.** The supplies here are a voltage behind a resistance
//!   with a reservoir across them, which is what a solid-state rectified
//!   supply looks like from the amplifier's side. An amplifier with a valve
//!   rectifier sags differently and would need one modelled.
//! - **Per-tube mismatch.** Tubes in parallel are one part passing `count`
//!   times the current. Real tubes are not matched, but the mismatch is a
//!   manufacturing tolerance rather than a circuit property, and inventing one
//!   would be inventing a sound.
//! - **The speaker's impedance curve.** The secondary works into a resistance.
//!   A real speaker is a resonant load whose impedance rises at its resonance
//!   and again at the top of the band, and that reflects back through the
//!   transformer and changes the damping. The cabinet section models the
//!   response; it does not model the load.
//! - **Bias drift.** The bias supply is a fixed voltage. A real one moves with
//!   the mains and with how hard the amplifier is being driven.

use crate::dsp::netlist::{Circuit, CoreSpec, Fault, Netlist, PentodeSpec, Taper, TriodeSpec};

/// The presence control.
pub const PRESENCE: usize = 0;

/// The master volume, which is the control between the preamplifier and the
/// power amplifier and is the reason a power stage can be driven at all.
///
/// The plugin does not expose it -- it has one Drive knob and that turns the
/// preamplifier's gain -- so it rests where a player leaves it. Which position
/// that is matters more here than anywhere else on the panel: a master is the
/// only thing standing between a preamplifier that can deliver forty volts and
/// a phase inverter that wants about five.
pub const MASTER: usize = 1;

/// Everything that differs between one amplifier's power stage and another's.
///
/// Grouped rather than passed as a row of bare numbers because they are all
/// resistances and capacitances and easy to transpose, and because a second
/// amplifier is meant to be a second one of these rather than a second copy of
/// the builder.
#[derive(Clone, Copy, Debug)]
pub struct PowerSpec {
    pub name: &'static str,

    // --- the phase inverter ---------------------------------------------
    /// The master volume's track, and where it rests. See `MASTER`.
    pub master: f64,
    pub master_rest: f64,
    /// Coupling into the inverter's grid.
    pub pi_couple: f64,
    /// Grid stopper on the driven side.
    pub pi_stopper: f64,
    /// The two halves of the grid-leak chain. Their junction is where the
    /// cathodes and the tail meet.
    pub pi_leak_upper: f64,
    pub pi_leak_lower: f64,
    /// From that junction to the joined cathodes.
    pub pi_cathode: f64,
    /// From that junction to the tail node.
    pub pi_tail: f64,
    /// From the tail node to ground.
    pub pi_tail_lower: f64,
    /// Couples the tail node to the undriven grid, which is what makes the
    /// pair a pair rather than two stages.
    pub pi_cross: f64,
    /// Plate loads. They are deliberately unequal: the undriven side has less
    /// gain, and the larger load evens the two outputs up.
    pub pi_plate_driven: f64,
    pub pi_plate_other: f64,
    pub pi_supply: f64,
    pub pi_tube: TriodeSpec,

    // --- the output stage -------------------------------------------------
    /// Coupling from each inverter plate to its output tubes.
    pub couple: f64,
    /// Grid leak, returning to the bias supply.
    pub grid_leak: f64,
    pub stopper: f64,
    pub screen_resistor: f64,
    /// How many tubes on each side.
    pub tubes_per_side: f64,
    pub tube: PentodeSpec,
    /// The fixed negative bias.
    pub bias: f64,

    // --- the supplies -----------------------------------------------------
    pub plate_supply: f64,
    pub screen_supply: f64,
    /// What the supply looks back through. This is the sag: a stiff supply is
    /// a small resistance behind a large reservoir.
    pub supply_resistance: f64,
    pub reservoir: f64,
    pub screen_resistance: f64,
    pub screen_reservoir: f64,

    // --- the output transformer -------------------------------------------
    /// Primary volts per secondary volt, plate to plate.
    pub ratio: f64,
    /// Copper, per half primary.
    pub primary_resistance: f64,
    /// The whole primary's inductance, plate to plate. This is what sets the
    /// bottom of the band: against the reflected load it is a high-pass, and a
    /// guitar transformer is deliberately small enough that the corner sits
    /// just under the open bottom string.
    pub primary_inductance: f64,
    /// Leakage, in the secondary, which is what sets the top of the band.
    pub leakage: f64,
    /// Where the iron runs out, as peak volts across the speaker at the
    /// lowest note the amplifier is expected to hold.
    ///
    /// Stated this way rather than as a flux, because flux is not a thing
    /// anyone knows about a transformer they have not wound. What a player
    /// knows is that a big amplifier turned up starts to go soft and blurred
    /// on the bottom string before it does anywhere else, and that is this: an
    /// output transformer is sized so its core just about survives full power
    /// at the bottom of the guitar's range, and below that it does not.
    ///
    /// The flux follows: a sine of peak `v` at `hz` puts `v / (2 pi hz)`
    /// volt-seconds through the winding.
    pub saturation_volts: f64,
    pub saturation_hz: f64,
    /// How sharply the core gives up past the knee.
    pub core_sharpness: f64,
    pub speaker: f64,

    // --- feedback ---------------------------------------------------------
    /// From the secondary back to the tail. A larger resistor is less
    /// feedback and a louder, looser amplifier.
    pub feedback: f64,
    /// The presence control shunts the feedback at the top of the band, so
    /// turning it up takes treble *out of the loop* rather than boosting it.
    pub presence_pot: f64,
    pub presence_cap: f64,
}

impl PowerSpec {
    /// The Peavey EVH 5150's, read off page 5 of Peavey's drawing at 200 dpi.
    ///
    /// Four 6L6GC, two a side, into a 70500207 transformer with 8 and 4 ohm
    /// taps. The reference numbers are the drawing's own.
    pub const EVH5150: PowerSpec = PowerSpec {
        name: "EVH 5150 power amp",
        // VR7, marked ULTRA POST. Two thirds up, which is a loud room.
        master: 1_000_000.0,
        master_rest: 0.66,
        pi_couple: 0.022e-6,        // C49
        pi_stopper: 100_000.0,      // R48
        pi_leak_upper: 1_000_000.0, // R50
        pi_leak_lower: 1_000_000.0, // R53
        pi_cathode: 470.0,          // R52
        pi_tail: 10_000.0,          // R51
        pi_tail_lower: 4_700.0,     // R57
        pi_cross: 0.047e-6,         // C31
        pi_plate_driven: 82_000.0,  // R46
        pi_plate_other: 100_000.0,  // R58
        pi_supply: 410.0,           // V3, not printed -- see the note below
        pi_tube: TriodeSpec::ECC83,

        couple: 0.047e-6,       // C28, C29
        grid_leak: 220_000.0,   // R54, R55
        stopper: 2_200.0,       // R202, R206
        screen_resistor: 100.0, // R203, R207, 5 W
        tubes_per_side: 2.0,    // V5+V7 and V6+V8
        tube: PentodeSpec::T6L6GC,
        bias: -46.0,

        plate_supply: 470.0,
        screen_supply: 465.0,
        supply_resistance: 150.0,
        reservoir: 220e-6,
        screen_resistance: 400.0, // R210, 10 W
        screen_reservoir: 100e-6,

        ratio: 15.4,
        primary_resistance: 30.0,
        primary_inductance: 32.0,
        leakage: 30e-6,
        // A hundred and twenty watts into eight ohms is 31 V rms, 44 V peak.
        saturation_volts: 44.0,
        saturation_hz: 82.0,
        core_sharpness: 7.0,
        speaker: 8.0,

        feedback: 39_000.0,     // R56
        presence_pot: 10_000.0, // VR9
        presence_cap: 0.033e-6, // C62
    };

    /// The Mesa Boogie Mark IIC+'s.
    ///
    /// `CLAUDE.md` §9.2 names the 1983-era **60 W** C+, which is a pair of
    /// 6L6GC -- one a side, not two. The hundred watt version doubles them up,
    /// and building that by mistake makes an amplifier that measures 118 W and
    /// is a different amplifier.
    ///
    /// A pair wants a plate-to-plate load of about 3.6 k, which is the figure
    /// the tube's own data sheet gives for two of them at 450 V, so the ratio
    /// is `sqrt(3600 / 8)`. Mesa's drawing gives the phase inverter and the
    /// output stage but, as with the preamplifier, prints no rail voltage.
    pub const MARKIIC: PowerSpec = PowerSpec {
        name: "Mark IIC+ power amp",
        // The Lead Master. Lower than the Peavey's, because the Mark IIC+'s
        // preamplifier hands it a great deal more and because a IIC+ is played
        // with its master well down -- that is the amplifier's whole method:
        // gain in the preamplifier, level at the master.
        master: 1_000_000.0,
        master_rest: 0.30,
        pi_couple: 0.1e-6,
        pi_stopper: 100_000.0,
        pi_leak_upper: 1_000_000.0,
        pi_leak_lower: 1_000_000.0,
        pi_cathode: 470.0,
        pi_tail: 10_000.0,
        pi_tail_lower: 4_700.0,
        pi_cross: 0.047e-6,
        pi_plate_driven: 82_000.0,
        pi_plate_other: 100_000.0,
        pi_supply: 410.0,
        pi_tube: TriodeSpec::ECC83,

        couple: 0.1e-6,
        grid_leak: 220_000.0,
        stopper: 1_500.0,
        screen_resistor: 470.0,
        tubes_per_side: 1.0, // a pair, not two pairs -- the 60 W C+
        tube: PentodeSpec::T6L6GC,
        bias: -44.0,

        plate_supply: 440.0,
        screen_supply: 435.0,
        // Mesa runs a stiffer supply than Peavey and it is part of why the
        // amplifier feels tighter under the hand.
        supply_resistance: 110.0,
        reservoir: 330e-6,
        screen_resistance: 470.0,
        screen_reservoir: 100e-6,

        ratio: 21.2, // 3.6 k plate to plate into 8 ohms
        primary_resistance: 35.0,
        primary_inductance: 40.0,
        leakage: 26e-6,
        // Sixty watts into eight ohms is 22 V rms, 31 V peak. Mesa's
        // transformer is generously sized for the power it carries, so it
        // holds a little further down than the Peavey's.
        saturation_volts: 31.0,
        saturation_hz: 70.0,
        core_sharpness: 6.0,
        speaker: 8.0,

        feedback: 100_000.0,
        presence_pot: 5_000.0,
        presence_cap: 0.1e-6,
    };
}

/// The power amplifier as a netlist, taking volts at its input and giving
/// volts across the speaker.
pub fn build(spec: &PowerSpec, source: f64) -> Result<Circuit, Fault> {
    tap(spec, source, "spk")
}

/// The same amplifier brought out at a chosen node, for measuring one stage at
/// a time.
pub fn tap(spec: &PowerSpec, source: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new(spec.name);

    // --- the long-tailed pair ----------------------------------------------
    // The grid-leak chain is two resistors in series and the junction of them
    // is not ground: it carries the joined cathodes through `pi_cathode`, and
    // the tail hangs off it. That is what makes this a pair rather than two
    // stages sharing a socket -- the current one half stops drawing, the other
    // half takes, and the tail voltage is what tells it to.
    net.input("in", source)
        .rest(MASTER, spec.master_rest)
        .pot("in", "master", "gnd", spec.master, Taper::Audio, MASTER)
        .capacitor("master", "pi_a", spec.pi_couple)
        .resistor("pi_a", "pi_b", spec.pi_leak_upper)
        .resistor("pi_a", "pi_g1", spec.pi_stopper)
        .resistor("pi_b", "pi_k", spec.pi_cathode)
        .resistor("pi_b", "pi_g2", spec.pi_leak_lower)
        .resistor("pi_b", "tail", spec.pi_tail)
        .resistor("tail", "gnd", spec.pi_tail_lower)
        .capacitor("tail", "pi_g2", spec.pi_cross)
        .supply("pi_p1", spec.pi_plate_driven, spec.pi_supply)
        .supply("pi_p2", spec.pi_plate_other, spec.pi_supply)
        .triode("pi_p1", "pi_g1", "pi_k", spec.pi_tube)
        .triode("pi_p2", "pi_g2", "pi_k", spec.pi_tube);

    // --- the supplies ------------------------------------------------------
    // A voltage behind a resistance with a reservoir across it. Under load the
    // node falls, and it comes back with the time constant of the two -- which
    // for a couple of hundred microfarads behind a hundred and fifty ohms is
    // about thirty milliseconds, or the length of a chord's attack. That is
    // sag, and it is why the note blooms.
    net.supply("ht", spec.supply_resistance, spec.plate_supply)
        .capacitor("ht", "gnd", spec.reservoir)
        .supply("scr", spec.screen_resistance, spec.screen_supply)
        .capacitor("scr", "gnd", spec.screen_reservoir)
        // The bias supply. Stiff: it feeds two grid leaks and nothing else
        // until the grids start drawing, and what happens then is the point.
        .supply("bias", 22_000.0, spec.bias);

    // --- the output tubes ---------------------------------------------------
    for (side, plate, from) in [(1usize, "pl_a", "pi_p1"), (2, "pl_b", "pi_p2")] {
        let grid = format!("og{side}");
        let node = format!("on{side}");
        let screen = format!("os{side}");
        net.capacitor(from, &node, spec.couple)
            .resistor(&node, "bias", spec.grid_leak)
            .resistor(&node, &grid, spec.stopper)
            .resistor("scr", &screen, spec.screen_resistor)
            .pentode(plate, &grid, "gnd", &screen, spec.tubes_per_side, spec.tube);
    }

    // --- the output transformer ---------------------------------------------
    // The primary is centre tapped to the supply, so each plate reaches it
    // through half the winding's copper. The magnetising branch goes across
    // the whole primary and is a saturating core rather than a plain
    // inductance: an amplifier asked for a low note at full power runs out of
    // iron before it runs out of tubes, and that is a sound rather than a
    // fault.
    // The centre tap, which took four tries and is the whole output stage.
    //
    // **One.** Feeding it through the copper alone puts the two half-winding
    // resistances in series *across* the primary -- sixty ohms across nineteen
    // hundred -- which is very nearly a short. Forty milliwatts out of a
    // hundred and twenty watt amplifier, with every stage biased correctly.
    //
    // **Two.** Feeding it through the core does not work either: this core
    // integrates the volts across it, so it presents megohms at the sample
    // rate and megohms at rest, and a plate hung off one has no path to its
    // supply at all. Both plates floated at twenty kilovolts.
    //
    // **Three.** A choke big enough to be invisible -- a thousand henries --
    // stamps as a microhm at rest and two hundred megohms at the sample rate,
    // and a matrix holding both has a condition number around ten to the
    // fourteen. The output came back with more harmonic energy than
    // fundamental, which is a solve that did not converge rather than a circuit
    // that clipped.
    //
    // **Four**, and this is the one that mattered. With a single transformer
    // across the two plates, the only path a signal current has is *through*
    // the primary, in at one plate and out at the other -- so every milliamp
    // tube A passes has to be swallowed by tube B. In class A both tubes
    // conduct all the time and that is very nearly true. In class AB, which is
    // where an amplifier makes its power, tube B is cut off for half the cycle
    // and can swallow nothing, so the model simply cannot deliver the current:
    // it is a class A amplifier wearing a class AB bias, and it makes about
    // half the power. That is the missing three decibels.
    //
    // A real centre-tapped winding does not work that way. Each tube drives its
    // own half into the shared secondary and the current returns to the supply
    // through the tap, so a cut-off tube contributes nothing and costs nothing.
    // The cut-off tube's *voltage* still swings, because the winding it is
    // attached to is coupled to the one doing the work -- which is why a plate
    // in a push-pull amplifier flies up to nearly twice the supply.
    //
    // So: two transformers, one per half winding, sharing a secondary. Each
    // takes half the turns, so each conducting tube sees `(ratio/2)^2` times
    // the speaker -- a quarter of the plate-to-plate load, which is the
    // textbook figure for a push-pull pair and is what the tubes are chosen
    // against.
    //
    // The chokes remain, but only as what they always were: the magnetising
    // inductance and the direct current path. The copper sits between the tap
    // and them, which keeps the two plates from being shorted together at rest
    // -- an exact short there would state the transformers' own constraint a
    // second time, and two ways of saying one thing is a singular matrix.
    let half = spec.primary_inductance / 2.0;
    // Saturation goes on the secondary, where a transformer carries no
    // standing current and this core's inability to pass any does not matter.
    // Referred through the ratio it is the same branch, simply on the side
    // where it can be modelled -- and it is the right side for the push-pull
    // cancellation too, because the two halves' direct currents oppose and a
    // balanced pair puts no standing flux in the iron at all.
    //
    // Its `henry` is deliberately far larger than the winding's: the linear
    // part of the magnetising branch is already the two half windings, and a
    // core repeating it would count the same inductance twice. What this one
    // is here for is the knee.
    let secondary_core = CoreSpec {
        henry: spec.primary_inductance * 40.0 / (spec.ratio * spec.ratio),
        knee: spec.saturation_volts / (std::f64::consts::TAU * spec.saturation_hz),
        sharpness: spec.core_sharpness,
    };
    let each = spec.ratio / 2.0;
    net.resistor("ct", "m", spec.primary_resistance)
        .inductor("m", "pl_a", half)
        .inductor("m", "pl_b", half)
        .resistor("ct", "ht", 1.0)
        .transformer("pl_a", "ct", "sec", "gnd", each)
        .transformer("ct", "pl_b", "sec", "gnd", each)
        .core("sec", "gnd", secondary_core)
        .inductor("sec", "spk", spec.leakage)
        .resistor("spk", "gnd", spec.speaker);

    // --- feedback and presence ----------------------------------------------
    // Back to the tail, where it opposes the signal. The presence control
    // shunts the feedback to ground through a capacitor, so turning it up
    // removes treble *from the loop* -- which puts treble back in the output.
    // A presence control is the only tone control that works by taking
    // something away from the amplifier's own correction.
    net.resistor("spk", "tail", spec.feedback)
        .capacitor("tail", "pres", spec.presence_cap)
        .pot(
            "pres",
            "gnd",
            "gnd",
            spec.presence_pot,
            Taper::Audio,
            PRESENCE,
        );

    net.build(at)
}
