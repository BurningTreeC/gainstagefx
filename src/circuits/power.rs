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
//! - **The speaker's impedance curve**, in `build`. That secondary works into a
//!   resistance, which every calibration was measured against and which old
//!   sessions keep. `build_with_speaker` stamps a loudspeaker's
//!   electromechanical equivalent there instead, so its resonance and coil
//!   inductance reach the tubes and the feedback loop.
//! - **Bias drift.** The bias supply is a fixed voltage. A real one moves with
//!   the mains and with how hard the amplifier is being driven.

use crate::acoustics::speaker::{self, LoadSlots, LoadValues};
use crate::dsp::netlist::{
    Circuit, CoreSpec, Fault, Netlist, PentodeSpec, RectifierSpec, Taper, TriodeSpec,
};

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

/// The resonance control, where a stage has one: only the Peavey 5150's
/// (`FeedbackNetwork`). No panel knob; it rests at its `rest`.
pub const RESONANCE: usize = 2;

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
    /// Coupling into the inverter's grid. Zero is a *direct* coupling: the
    /// grids hang on `pi_leak_upper` and `pi_leak_lower` straight off whatever
    /// drives them, which then also has to state its direct voltage in
    /// `driver_volts`.
    pub pi_couple: f64,
    /// The direct voltage the preamplifier's output sits at, for a directly
    /// coupled inverter. Zero everywhere else, where a coupling capacitor
    /// stands between the two.
    pub driver_volts: f64,
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
    /// Optional phase-inverter plate-to-plate stability capacitor.
    pub pi_plate_cap: f64,

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
    /// The fixed negative bias. Ignored when `cathode_bias` is set.
    pub bias: f64,
    /// A shared cathode resistor instead of a fixed bias supply, with its
    /// bypass capacitor. Zero is fixed bias.
    ///
    /// This is the other way to bias an output stage and it is a different
    /// amplifier: the valves set their own bias, so driving them harder pushes
    /// the cathode up and takes the bias with it. That is the compression a
    /// cathode-biased amplifier has and a fixed-bias one does not. The grid
    /// leaks return to ground rather than to a bias supply.
    pub cathode_bias: f64,
    pub cathode_bypass: f64,

    // --- the supplies -----------------------------------------------------
    pub plate_supply: f64,
    pub screen_supply: f64,
    /// What the supply looks back through. This is the sag: a stiff supply is
    /// a small resistance behind a large reservoir.
    pub supply_resistance: f64,
    /// A rectifier valve between that and the reservoir, where the amplifier
    /// has one. `None` is a silicon bridge, which is what a fixed resistance
    /// already describes.
    ///
    /// With one fitted the transformer's own winding resistance stays as
    /// `supply_resistance` and the valve goes in front of the reservoir, so the
    /// rail's drop under load is the valve's own law rather than a number.
    pub rectifier: Option<RectifierSpec>,
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
    /// Zero is an amplifier with no loop at all, which is a different kind of
    /// amplifier: nothing corrects the output stage, so the transformer's own
    /// response and the valves' distortion reach the speaker as they are.
    pub feedback: f64,
    /// The presence control shunts the feedback at the top of the band, so
    /// turning it up takes treble *out of the loop* rather than boosting it.
    pub presence_pot: f64,
    pub presence_cap: f64,
    /// A cut control: a rheostat in series with a capacitor, straight across
    /// the inverter's two outputs. Nothing to do with feedback -- it shorts the
    /// top of the band between the two sides before the output valves see it,
    /// which is how an amplifier with no loop gets a treble control at the
    /// back. Zero is not fitted.
    ///
    /// The control runs the way the knob on the amplifier does: up is more cut
    /// and a darker sound, because it is the amplifier's own control and not a
    /// presence.
    pub cut_pot: f64,
    pub cut_cap: f64,
    /// Where the cut control rests, since the panel has no knob for it.
    pub cut_rest: f64,
    /// A driver stage built inside the power stage, where the amplifier's own
    /// presence loop needs one: only the Hiwatt's. `None` everywhere else. See
    /// `DriverSpec`.
    pub driver: Option<&'static DriverSpec>,
    /// A feedback path of the Peavey kind, where the loop reaches the tail
    /// through a resonance network and a coupling capacitor and the presence
    /// control shunts the loop before it gets there. `None` is the Marshall
    /// kind above (`feedback` straight to the tail, `presence_pot` across it).
    pub feedback_network: Option<&'static FeedbackNetwork>,
}

/// The Peavey 5150's feedback path, from the original's preamp and tube-board
/// sheets ("VHALEN 120 PREAMP BD.", 81501510, 5-FEB-92, sheets 1 and 2):
///
/// `feedback` (R56, 39 k) from the output transformer into **VR8 RESONANCE**,
/// 1 M audio wired as a variable resistor with **C12 .0068** across it; then
/// **C30 22 uF** into the tail (R57's node). At that middle node **VR9
/// PRESENCE**, 10 k audio, hangs to ground with **C8 1 uF** to its top and
/// **C62 .033** to its wiper.
///
/// Resonance up puts resistance in series with the loop below C12's corner, so
/// the bottom is fed back less and comes up: the "depth" of this family.
/// Presence up (wiper to ground) shunts the top of the loop away, so the top
/// comes up. The clockwise arrows are the drawing's. C8's "1P10" is read as 1 uF
/// (PLAUSIBLE); the others are DOCUMENTED.
#[derive(Clone, Copy, Debug)]
pub struct FeedbackNetwork {
    pub resonance_pot: f64,
    pub resonance_cap: f64,
    /// Where the resonance control rests: there is no panel knob for it.
    pub resonance_rest: f64,
    pub coupling: f64,
    pub presence_pot: f64,
    pub presence_top_cap: f64,
    pub presence_wiper_cap: f64,
}

impl FeedbackNetwork {
    /// The original 5150's. The resonance rest is ESTIMATED: noon, where a
    /// control nobody has set sits.
    pub const PEAVEY_5150: FeedbackNetwork = FeedbackNetwork {
        resonance_pot: 1_000_000.0,
        resonance_cap: 0.0068e-6,
        resonance_rest: 0.5,
        coupling: 22e-6,
        presence_pot: 10_000.0,
        presence_top_cap: 1e-6,
        presence_wiper_cap: 0.033e-6,
    };
}

/// The last gain stage and the phase inverter's grid network, built inside the
/// power stage rather than at the end of the preamplifier -- which is where
/// the Hiwatt DR103's presence control needs them.
///
/// Hiwatt, "DR103 PREAMPLIFIER CIRCUIT DIAGRAM", Issue 4 (19/05/95), read at
/// 300 dpi (`docs/schematics/hiwatt_100w_dr103.pdf`, page 4):
///
/// - **V3a** is a plain gain stage from the master's wiper: 100 k plate from
///   H.T. SUPPLY 3, 1k5 cathode, unbypassed. Its plate goes through **47 nF**
///   straight to the inverter's driven grid. That is the signal path.
/// - **V3b** carries no signal. Its plate is on H.T. SUPPLY 3, its grid on a
///   divider from the same rail (1 M up, 220 k down), its cathode on 220 k: a
///   stiff direct voltage, about a fifth of the rail, which both inverter grids
///   return to -- the driven one through **100 k**, the other through **1 M**.
///   A grid held by a follower cannot drift when it is driven hard; that part of
///   the Hiwatt legend is right, but the follower holds the grids, it does not
///   drive them.
/// - The feedback node (the 22 k / 2k2 tail junction, where the 16 ohm tap's
///   10 k lands) has **470 R and 10 nF** in series to ground.
/// - **PRESENCE**, 100 k linear: one end through **100 nF** to the feedback
///   node, the other through **1000 pF** to V3a's plate, the wiper through
///   **470 R** to ground. Turned up (wiper at the feedback end) it shunts the
///   top of the band out of the loop, so less of it is fed back; turned down
///   (wiper at the plate end) it hangs 1000 pF and 470 R across V3a's plate and
///   takes the top off before the inverter. Which end is clockwise is not on the
///   drawing; up is the bright end, as on every presence control. ESTIMATED.
///
/// The `1000 pF`'s last digit is damaged on the scan; four figures and "pF" are
/// legible. PLAUSIBLE.
///
/// Until 2026-09-25 the model had V3b as the signal path -- its grid on "1.8 M
/// with 22 nF across it into 1 M" from V3a's plate, which is on neither of
/// Hiwatt's sheets -- and no presence at all.
#[derive(Clone, Copy, Debug)]
pub struct DriverSpec {
    /// Whether V3a is built. The Hiwatt's own chain has it; another
    /// preamplifier chosen in front of this power stage does not get it, and
    /// drives V3a's plate node -- the inverter's coupling capacitor and the
    /// presence loop -- through `plate_source` instead. See
    /// `PowerSpec::DR103_EL34_RETURN`.
    pub gain_stage: bool,
    /// V3a's output impedance, which is what drives that node when V3a is not
    /// built: its plate resistance with the unbypassed cathode,
    /// `rp + (mu + 1) Rk`, against its 100 k load. DERIVED from the ECC83's
    /// published rp 62.5 k and mu 100: 214 k in parallel with 100 k, 68 k. It
    /// matters: the node carries the presence loop and the grid returns to a
    /// stiff 100 k, and a stiffer source than a valve's plate drives grid
    /// current no Hiwatt could.
    pub plate_source: f64,
    pub tube: TriodeSpec,
    /// V3a's plate load and cathode resistor.
    pub plate: f64,
    pub cathode: f64,
    /// V3a's plate to the driven inverter grid.
    pub couple: f64,
    /// V3b's grid divider from the rail, and its cathode load.
    pub reference_upper: f64,
    pub reference_lower: f64,
    pub reference_load: f64,
    /// From V3b's cathode to the driven grid, and to the other grid.
    pub leak_driven: f64,
    pub leak_other: f64,
    /// The series resistor and capacitor from the feedback node to ground.
    pub tail_shunt_r: f64,
    pub tail_shunt_c: f64,
    /// The presence control: its track, the capacitor to the feedback node,
    /// the capacitor to V3a's plate, and the wiper's resistor to ground.
    pub presence_pot: f64,
    pub presence_feedback_cap: f64,
    pub presence_plate_cap: f64,
    pub presence_wiper: f64,
    /// Where the presence control rests.
    pub presence_rest: f64,
}

impl DriverSpec {
    /// The Hiwatt DR103's, Issue 4. Every value DOCUMENTED except the presence
    /// control's direction (ESTIMATED) and the 1000 pF's damaged digit
    /// (PLAUSIBLE); see above.
    pub const HIWATT_DR103: DriverSpec = DriverSpec {
        gain_stage: true,
        plate_source: 68_000.0,
        tube: TriodeSpec::ECC83,
        plate: 100_000.0,
        cathode: 1_500.0,
        couple: 47e-9,
        reference_upper: 1_000_000.0,
        reference_lower: 220_000.0,
        reference_load: 220_000.0,
        leak_driven: 100_000.0,
        leak_other: 1_000_000.0,
        tail_shunt_r: 470.0,
        tail_shunt_c: 10e-9,
        presence_pot: 100_000.0,
        presence_feedback_cap: 100e-9,
        presence_plate_cap: 1_000e-12,
        presence_wiper: 470.0,
        presence_rest: 0.5,
    };

    /// The same network with no V3a: what another preamplifier meets when it
    /// is put in front of this power stage.
    pub const HIWATT_DR103_RETURN: DriverSpec = DriverSpec {
        gain_stage: false,
        ..Self::HIWATT_DR103
    };
}

impl PowerSpec {
    /// 1981 2203 EL34 circuit with Hammond replacement-iron data. The matched
    /// power stage of the Brit 800 preamplifier (`circuits::brit800`), whose
    /// treble wiper feeds the master pot this begins with.
    /// See docs/models/brit_el34.md for sources, revisions and approximation boundaries.
    pub const BRIT_EL34: PowerSpec = PowerSpec {
        name: "Brit EL34 (2203 1981)",
        master: 1_000_000.0,
        master_rest: 0.30,
        pi_couple: 22e-9,
        driver_volts: 0.0,
        pi_stopper: 0.0,
        pi_leak_upper: 1_000_000.0,
        pi_leak_lower: 1_000_000.0,
        pi_cathode: 470.0,
        pi_tail: 10_000.0,
        pi_tail_lower: 4_700.0,
        pi_cross: 0.1e-6,
        pi_plate_driven: 82_000.0,
        pi_plate_other: 100_000.0,
        pi_supply: 330.0,
        pi_tube: TriodeSpec::ECC83,
        pi_plate_cap: 47e-12,
        couple: 22e-9,
        grid_leak: 220_000.0,
        // Two parallel tubes per side: equivalent of 5k6 / 1k per tube.
        stopper: 2_800.0,
        screen_resistor: 500.0,
        tubes_per_side: 2.0,
        tube: PentodeSpec::EL34,
        bias: -42.0,
        cathode_bias: 0.0,
        cathode_bypass: 0.0,
        plate_supply: 470.0,
        screen_supply: 468.0,
        supply_resistance: 100.0,
        rectifier: None,
        reservoir: 50e-6,
        screen_resistance: 100.0,
        screen_reservoir: 50e-6,
        ratio: 20.615_528_128_088_304,
        primary_resistance: 15.96,
        primary_inductance: 8.85,
        leakage: 7.97e-3 / 425.0,
        saturation_volts: 28.284_271_247_461_902,
        saturation_hz: 70.0,
        core_sharpness: 6.0,
        speaker: 4.0,
        feedback: 100_000.0,
        presence_pot: 22_000.0,
        presence_cap: 0.1e-6,
        cut_pot: 0.0,
        cut_cap: 0.0,
        cut_rest: 0.0,
        driver: None,
        feedback_network: None,
    };

    /// The 1959 Super Lead's, as Unicord drew it in July 1970 (70-6-11 issue B):
    /// the matched power stage of the Brit Plexi preamplifier
    /// (`circuits::plexi`). See `docs/models/brit_plexi.md`.
    ///
    /// The same four EL34s, inverter and transformer family as the 2203's, and
    /// three differences that are the difference between the two amplifiers:
    ///
    /// - **No master.** The preamplifier's treble wiper drives the inverter's
    ///   coupling capacitor directly, so the master track is left open.
    /// - **More feedback.** 47 k from the 16 ohm tap where the 2203 has 100 k from
    ///   4 ohm: built from the 4 ohm secondary as 23.5 k, which passes the same
    ///   current, about four times the 2203's feedback.
    /// - **A 5 k presence control** at the bottom of the tail instead of 22 k
    ///   across a 4.7 k resistor.
    pub const PLEXI_EL34: PowerSpec = PowerSpec {
        name: "Brit Plexi EL34 (1959, 1970)",
        master: 1_000_000.0,
        master_rest: 1.0,
        pi_couple: 22e-9,
        driver_volts: 0.0,
        pi_stopper: 0.0,
        pi_leak_upper: 1_000_000.0,
        pi_leak_lower: 1_000_000.0,
        pi_cathode: 470.0,
        pi_tail: 10_000.0,
        // The presence pot's 5 k track is what the tail returns through.
        pi_tail_lower: 5_000.0,
        pi_cross: 0.1e-6,
        pi_plate_driven: 82_000.0,
        pi_plate_other: 100_000.0,
        // ESTIMATED: the inverter node behind Unicord's 20 k dropper, with the
        // inverter and the preamplifier drawing from it (`plexi::INVERTER_NODE`).
        pi_supply: crate::circuits::plexi::INVERTER_NODE,
        pi_tube: TriodeSpec::ECC83,
        pi_plate_cap: 47e-12,
        couple: 22e-9,
        grid_leak: 220_000.0,
        // 5.6 k per tube, two a side.
        stopper: 2_800.0,
        // 1 k per tube.
        screen_resistor: 500.0,
        tubes_per_side: 2.0,
        tube: PentodeSpec::EL34,
        // ESTIMATED: 36 mA and 16 W a valve at idle, 64 % of an EL34's 25 W
        // (`examples/plexi_op.rs`).
        bias: -37.0,
        cathode_bias: 0.0,
        cathode_bypass: 0.0,
        // Marshall's c. 1967 100 W drawing: 460 V at the plates.
        plate_supply: 460.0,
        screen_supply: 455.0,
        supply_resistance: 100.0,
        rectifier: None,
        reservoir: 50e-6,
        screen_resistance: 100.0,
        screen_reservoir: 50e-6,
        // APPROXIMATED: the Brit EL34's 1.7 k replacement-iron data.
        ratio: 20.615_528_128_088_304,
        primary_resistance: 15.96,
        primary_inductance: 8.85,
        leakage: 7.97e-3 / 425.0,
        saturation_volts: 28.284_271_247_461_902,
        saturation_hz: 70.0,
        core_sharpness: 6.0,
        speaker: 4.0,
        feedback: 23_500.0,
        presence_pot: 5_000.0,
        presence_cap: 0.1e-6,
        cut_pot: 0.0,
        cut_cap: 0.0,
        cut_rest: 0.0,
        driver: None,
        feedback_network: None,
    };

    /// The JCM800 2205's, from Marshall's "2205 STD Output Stage & PSU", issue 2
    /// of 4-5-88: the matched power stage of the Brit 2205 preamplifier
    /// (`circuits::brit2205`). See `docs/models/brit_2205.md`.
    ///
    /// The 2203's family with half the valves, and an inverter that is not the
    /// 2203's:
    ///
    /// - **Two EL34s**, one a side, for 50 W.
    /// - **No master here.** The later 2205 has its master in the
    ///   preamplifier, ahead of the loop and V4A, so this stage begins at
    ///   C25 with the track wide open, as the Rectifier's does.
    /// - **Low grid leaks**: R41 330 k on the driven grid and R44 120 k on the
    ///   other, where the 2203 has 1 M each, with a 22 k tail (R45) above the
    ///   4k7 (R48) and 100 pF (C33) across the plates.
    /// - The same 100 k of feedback from the 4 ohm tap and the same 22 k
    ///   presence as the 2203.
    pub const BRIT_2205_EL34: PowerSpec = PowerSpec {
        name: "Brit 2205 EL34 (JCM800 2205, 1988)",
        // The master is VR10, in the preamplifier. Wide open, so it is not one.
        master: 1_000_000.0,
        master_rest: 1.0,
        pi_couple: 22e-9, // C25
        driver_volts: 0.0,
        pi_stopper: 0.0,
        pi_leak_upper: 330_000.0,  // R41
        pi_leak_lower: 120_000.0,  // R44
        pi_cathode: 470.0,         // R43
        pi_tail: 22_000.0,         // R45
        pi_tail_lower: 4_700.0,    // R48
        pi_cross: 0.1e-6,          // C30
        pi_plate_driven: 82_000.0, // R49
        pi_plate_other: 100_000.0, // R50
        // ESTIMATED: X, behind R59 10 k from the screen node; derived in
        // `brit2205::RAIL`.
        pi_supply: crate::circuits::brit2205::RAIL,
        pi_tube: TriodeSpec::ECC83,
        pi_plate_cap: 100e-12, // C33
        couple: 22e-9,         // C32, C34
        grid_leak: 220_000.0,  // R52, R53
        // One valve a side: the drawing's values as they stand.
        stopper: 5_600.0,         // R60, R61
        screen_resistor: 1_000.0, // R62, R63
        tubes_per_side: 1.0,
        tube: PentodeSpec::EL34,
        // ESTIMATED: RV1 is a 22 k preset, so what the amplifier documents is
        // a current, not a voltage. Set for 38 mA and 17 W a valve at idle
        // (`examples/brit2205_op.rs`), inside the 32-40 mA a 460 V JCM800 is
        // biased to; Ampbooks' 2204 analysis gets its 40 mA at -40 V from a
        // real valve, and the catalogue's EL34 fit needs -36 V for the same.
        bias: -36.0,
        cathode_bias: 0.0,
        cathode_bypass: 0.0,
        // WIDELY REPORTED: 50 W JCM800s measure about 460 V at the plates and
        // 450 V at the screens (Ampbooks' 2204 analysis; owners' readings of
        // 425-475 V by year and transformer). The 2205 sheet takes the plates
        // off the reservoir, ahead of the T3 choke, and the screens behind it.
        // The 1981 sheet's 50 W stage prints 365 V, which no measured amplifier
        // and not the 2205's own rating of 70 W at 4 % bear out; see the log.
        plate_supply: 460.0,
        screen_supply: 450.0,
        // ESTIMATED, as for the 2203.
        supply_resistance: 100.0,
        rectifier: None,
        reservoir: 50e-6, // C43
        screen_resistance: 100.0,
        screen_reservoir: 50e-6, // C43, the screen node's
        // PUBLISHED-PARAMETER DERIVED replacement iron: Hammond 1750N, the 50 W
        // JMP/JCM800 drop-in, 3.2 k plate to plate; half-primary copper 41.74
        // and 43.14 ohm; 18.3 H and 13.41 mH at 1 kHz. The original 789-139's
        // data is not public. Into the 4 ohm tap: sqrt(3200 / 4).
        ratio: 28.284_271_247_461_902,
        primary_resistance: 42.44,
        primary_inductance: 18.3,
        leakage: 13.41e-3 / 800.0,
        // Sized, like the 2203's, to just hold rated power at 70 Hz: 50 W into
        // 4 ohm is 20 V peak.
        saturation_volts: 20.0,
        saturation_hz: 70.0,
        core_sharpness: 6.0,
        speaker: 4.0,
        feedback: 100_000.0,    // R47, from the 4 ohm tap
        presence_pot: 22_000.0, // VR11
        presence_cap: 0.1e-6,   // C31
        cut_pot: 0.0,
        cut_cap: 0.0,
        cut_rest: 0.0,
        driver: None,
        feedback_network: None,
    };

    /// The AC30/6 Top Boost's, from the 1974 Dallas drawing Sc/V/1313, checked
    /// against the 1971 Vox Sound Limited sheet. The matched power stage of the
    /// Brit AC30 preamplifier (`circuits::ac30`). See `docs/models/brit_ac30.md`.
    ///
    /// The one in the catalogue that is a different kind of amplifier:
    ///
    /// - **Cathode bias.** Four EL84s share a 50 ohm resistor with 250 uF across
    ///   it, so the valves set their own bias and give it back as compression
    ///   when they are driven.
    /// - **No feedback at all.** Nothing corrects the output stage, so the
    ///   transformer and the valves are what reaches the speaker.
    /// - **No master volume**, and a CUT control across the inverter's outputs
    ///   instead of a presence control in a loop it does not have.
    pub const AC30_EL84: PowerSpec = PowerSpec {
        name: "AC30 EL84 (Top Boost, 1974)",
        master: 1_000_000.0,
        master_rest: 1.0,
        pi_couple: 0.047e-6, // C5
        driver_volts: 0.0,
        pi_stopper: 0.0,
        pi_leak_upper: 1_000_000.0, // R8
        pi_leak_lower: 1_000_000.0, // R17
        pi_cathode: 1_200.0,        // R16
        pi_tail: 47_000.0,          // R15
        // The tail returns to ground: there is no loop to bring back to it.
        pi_tail_lower: 0.0,
        // C7 to the third channel's volume, which is at zero: the undriven grid's
        // AC ground.
        pi_cross: 0.047e-6,
        pi_plate_driven: 100_000.0, // R18
        pi_plate_other: 100_000.0,  // R19
        pi_supply: crate::circuits::ac30::INVERTER_NODE,
        pi_tube: TriodeSpec::ECC83,
        pi_plate_cap: 0.0,
        couple: 0.15e-6,      // C6, C9
        grid_leak: 220_000.0, // R20, R21
        // 1k5 per valve, two a side.
        stopper: 750.0,
        // 100 ohm per valve.
        screen_resistor: 50.0,
        tubes_per_side: 2.0,
        tube: PentodeSpec::EL84,
        // Not used: the valves bias themselves.
        bias: 0.0,
        cathode_bias: 50.0,     // R24
        cathode_bypass: 250e-6, // C11
        // ESTIMATED. The drawings carry no voltages. This is what the mains
        // transformer's secondary rectifies to with nothing drawn; the valve
        // rectifier below and the choke's copper take it down from there, and
        // what it settles at under load is checked in `tests/ac30.rs`.
        plate_supply: 378.0,
        screen_supply: 378.0,
        // The transformer's own winding resistance. The rest of what was here
        // before -- the rectifier's drop -- is now the valve.
        supply_resistance: 100.0,
        // The 1971 sheet has a rectifier valve; the 1974 one has silicon diodes.
        // This is the valve, which is what a JMI AC30 had.
        rectifier: Some(RectifierSpec::GZ34),
        reservoir: 32e-6,
        // The screens come through the choke, so they sag further and sooner.
        screen_resistance: 700.0,
        screen_reservoir: 32e-6,
        // 4 k plate to plate into 8 ohm.
        ratio: 22.360_679_774_997_896,
        // ESTIMATED: a 30 W transformer's copper, magnetising inductance and
        // core. No winding data for this transformer was found.
        primary_resistance: 80.0,
        primary_inductance: 9.0,
        leakage: 60e-6,
        // 30 W into 8 ohm is 21.9 V peak.
        saturation_volts: 21.908_902_300_206_645,
        saturation_hz: 70.0,
        core_sharpness: 6.0,
        speaker: 8.0,
        // No loop.
        feedback: 0.0,
        presence_pot: 0.0,
        presence_cap: 0.0,
        cut_pot: 250_000.0, // VR4
        cut_cap: 0.0047e-6, // C10
        // A little cut, which is where an AC30 usually sits. The panel has no
        // knob for it yet.
        cut_rest: 0.2,
        driver: None,
        feedback_network: None,
    };

    /// The DR103's, from Hiwatt's own output-stage (Issue 1, 1994) and power
    /// supply (Issue 3, 1994) sheets. The matched power stage of the Brit DR103
    /// preamplifier (`circuits::dr103`). See `docs/models/brit_dr103.md`.
    ///
    /// Built for headroom, and every value says so: a supply near 480 V, 22 k
    /// grid stoppers where a Marshall has 5k6, and a feedback resistor small
    /// enough to hold the whole thing down. Its inverter brings the grid returns,
    /// the cathode resistor, the tail and the feedback to one node.
    pub const DR103_EL34: PowerSpec = PowerSpec {
        name: "DR103 EL34 (Hiwatt, Issue 1)",
        // The master volume is in the preamplifier, where the drawing has it.
        master: 1_000_000.0,
        master_rest: 1.0,
        // The grid network is the driver's (`DriverSpec::HIWATT_DR103`): V3a's
        // plate couples in through 47 nF and V3b holds both grids. These four
        // are unused with a driver.
        pi_couple: 0.0,
        driver_volts: 0.0,
        pi_stopper: 0.0,
        pi_leak_upper: 0.0,
        pi_leak_lower: 0.0,
        // The grid leaks return to the cathodes themselves.
        pi_cathode: 0.0,
        // 22 k from there to the tail node, 2k2 from the tail to ground, and the
        // feedback lands across that 2k2.
        pi_tail: 22_000.0,
        pi_tail_lower: 2_200.0,
        pi_cross: 100e-9,
        pi_plate_driven: 82_000.0,
        pi_plate_other: 91_000.0,
        // H.T. SUPPLY 3, which V3a, V3b and the inverter all hang on. ESTIMATED:
        // behind the supply sheet's 100 R and 1 k from the 480 V reservoir. The
        // preamplifier's V1 and V2 are further down the chain, behind the 10 k
        // droppers; see `circuits::dr103::RAIL` for the conflict between the two.
        pi_supply: 465.0,
        // Issue 4 labels every preamp valve ECC83; the late-60s amplifier had an
        // ECC81 here. See the research log.
        pi_tube: TriodeSpec::ECC83,
        pi_plate_cap: 0.0,
        couple: 47e-9,
        grid_leak: 100_000.0,
        // 22 k per valve, two a side. This is the Hiwatt signature.
        stopper: 11_000.0,
        // 100 ohm per valve.
        screen_resistor: 50.0,
        tubes_per_side: 2.0,
        tube: PentodeSpec::EL34,
        bias: -38.0,
        cathode_bias: 0.0,
        cathode_bypass: 0.0,
        // ESTIMATED: the sheets give no voltages. 480 V at the plates is what the
        // published reading of the supply sheet gives; the screens come through
        // the 470 R 10 W dropper.
        plate_supply: 490.0,
        screen_supply: 490.0,
        supply_resistance: 100.0,
        rectifier: None,
        reservoir: 220e-6,
        screen_resistance: 570.0,
        screen_reservoir: 50e-6,
        // APPROXIMATED: 1.7 k plate to plate into 8 ohm; no Hiwatt winding data
        // was found.
        ratio: 14.577_379_737_113_25,
        primary_resistance: 20.0,
        primary_inductance: 12.0,
        leakage: 20e-6,
        // 100 W into 8 ohm is 40 V peak, and this transformer is oversized.
        saturation_volts: 40.0,
        saturation_hz: 60.0,
        core_sharpness: 6.0,
        speaker: 8.0,
        // 10 k from the 16 ohm tap, which is 7.07 k from the 8 ohm one.
        feedback: 7_071.0,
        // The presence control is the driver's, not this Marshall-style one.
        presence_pot: 0.0,
        presence_cap: 0.0,
        cut_pot: 0.0,
        cut_cap: 0.0,
        cut_rest: 0.0,
        driver: Some(&DriverSpec::HIWATT_DR103),
        feedback_network: None,
    };

    /// The DR103's power stage as another preamplifier meets it: the Hiwatt's
    /// inverter, grid network, feedback and presence, with the input where
    /// V3a's plate would be rather than at V3a's grid. V3a is the Hiwatt's own
    /// last preamp valve, and putting a whole second preamplifier's output
    /// through it would be two preamplifiers, not a power amplifier.
    ///
    /// Its master is a 100 k track rather than 1 M: in a custom chain it rests
    /// at `OVERRIDE_MASTER_REST`, and a 1 M wiper there would be a couple of
    /// hundred kilohms of source into the 100 k grid return. APPROXIMATED, as
    /// every override master is -- the amplifier has no return jack.
    pub const DR103_EL34_RETURN: PowerSpec = PowerSpec {
        name: "DR103 EL34 (from its inverter)",
        master: 100_000.0,
        driver: Some(&DriverSpec::HIWATT_DR103_RETURN),
        ..Self::DR103_EL34
    };

    /// The Dual Rectifier's, from Mesa's own "DUAL RECTIFIER POWER AMP" sheet.
    /// The matched power stage of the Cali Rectifier preamplifier
    /// (`circuits::rectifier`). See `docs/models/cali_rectifier.md`.
    ///
    /// Four 6L6 on a -51 V fixed bias, the drawing's own inverter with its 120 pF
    /// plate capacitors, and the presence control in the feedback loop. The
    /// amplifier's switchable valve rectifier is **not** modelled: this supply is
    /// a voltage behind a resistance, which is its silicon setting.
    pub const RECTO_6L6: PowerSpec = PowerSpec {
        name: "Recto 6L6 (Dual Rectifier, Rev F)",
        // The red channel's master is in the preamplifier, where the sheet has it.
        master: 1_000_000.0,
        master_rest: 1.0,
        pi_couple: 0.1e-6,
        driver_volts: 0.0,
        pi_stopper: 0.0,
        pi_leak_upper: 1_000_000.0, // R214
        pi_leak_lower: 1_000_000.0, // R213
        pi_cathode: 470.0,          // R341
        pi_tail: 10_000.0,          // R353
        pi_tail_lower: 4_700.0,     // R372
        pi_cross: 0.1e-6,           // C40
        pi_plate_driven: 82_000.0,  // R281
        pi_plate_other: 90_000.0,   // R104
        // The sheet reads 280 V at both inverter plates.
        pi_supply: 415.0,
        pi_tube: TriodeSpec::ECC83,
        // C1 and C2, 120 pF from each plate. The sheet has one per plate to
        // ground; plate to plate is the same shape at half the value.
        pi_plate_cap: 60e-12,
        couple: 0.047e-6,       // C31, C32
        grid_leak: 220_000.0,   // R222, R223
        stopper: 750.0,         // 1k5 per valve, two a side
        screen_resistor: 500.0, // 1k per valve
        tubes_per_side: 2.0,
        tube: PentodeSpec::T6L6GC,
        // The sheet's own figure: "-51v 6L6".
        bias: -51.0,
        cathode_bias: 0.0,
        cathode_bypass: 0.0,
        // ESTIMATED: the sheet gives no rail voltage, and a Rectifier's runs high.
        plate_supply: 490.0,
        screen_supply: 485.0,
        supply_resistance: 120.0,
        rectifier: None,
        reservoir: 220e-6,
        screen_resistance: 470.0,
        screen_reservoir: 100e-6,
        // APPROXIMATED: no data for transformer #562105 was found. 2.2 k plate to
        // plate into 8 ohm, which is what four 6L6 on this supply are matched to
        // -- the Mark IIC+'s 3.6 k is a pair, not two pairs.
        ratio: 16.583_123_951_777,
        primary_resistance: 35.0,
        primary_inductance: 40.0,
        leakage: 25e-6,
        saturation_volts: 40.0,
        saturation_hz: 60.0,
        core_sharpness: 6.0,
        speaker: 8.0,
        // From the 8-16 ohm tap, through the presence network.
        feedback: 100_000.0,
        presence_pot: 25_000.0, // the sheet's 25 k
        presence_cap: 0.1e-6,   // C52
        cut_pot: 0.0,
        cut_cap: 0.0,
        cut_rest: 0.0,
        driver: None,
        feedback_network: None,
    };

    /// The same amplifier with its rectifier switch on VALVE rather than SILICON
    /// DIODE -- the switch it is named after.
    ///
    /// Every other value is the Rev F spec above. What changes is the supply:
    /// two 5U4GB instead of a bridge, so the rail sits lower and gives way under
    /// a chord instead of holding. See `docs/models/cali_rectifier.md`.
    pub const RECTO_6L6_TUBE: PowerSpec = PowerSpec {
        name: "Recto 6L6, valve rectifier",
        // The red channel's master is in the preamplifier, where the sheet has it.
        master: 1_000_000.0,
        master_rest: 1.0,
        pi_couple: 0.1e-6,
        driver_volts: 0.0,
        pi_stopper: 0.0,
        pi_leak_upper: 1_000_000.0, // R214
        pi_leak_lower: 1_000_000.0, // R213
        pi_cathode: 470.0,          // R341
        pi_tail: 10_000.0,          // R353
        pi_tail_lower: 4_700.0,     // R372
        pi_cross: 0.1e-6,           // C40
        pi_plate_driven: 82_000.0,  // R281
        pi_plate_other: 90_000.0,   // R104
        // The sheet reads 280 V at both inverter plates.
        pi_supply: 415.0,
        pi_tube: TriodeSpec::ECC83,
        // C1 and C2, 120 pF from each plate. The sheet has one per plate to
        // ground; plate to plate is the same shape at half the value.
        pi_plate_cap: 60e-12,
        couple: 0.047e-6,       // C31, C32
        grid_leak: 220_000.0,   // R222, R223
        stopper: 750.0,         // 1k5 per valve, two a side
        screen_resistor: 500.0, // 1k per valve
        tubes_per_side: 2.0,
        tube: PentodeSpec::T6L6GC,
        // The sheet's own figure: "-51v 6L6".
        bias: -51.0,
        cathode_bias: 0.0,
        cathode_bypass: 0.0,
        // ESTIMATED: the sheet gives no rail voltage, and a Rectifier's runs high.
        plate_supply: 490.0,
        screen_supply: 485.0,
        supply_resistance: 120.0,
        // Two 5U4GB in parallel, which is what the amplifier is named after:
        // one valve's drop at twice its current. Switching to them takes the
        // rail down and puts sag where a bridge holds firm.
        rectifier: Some(RectifierSpec {
            drop_volts: 50.0,
            at_amps: 0.5,
        }),
        reservoir: 220e-6,
        screen_resistance: 470.0,
        screen_reservoir: 100e-6,
        // APPROXIMATED: no data for transformer #562105 was found. 2.2 k plate to
        // plate into 8 ohm, which is what four 6L6 on this supply are matched to
        // -- the Mark IIC+'s 3.6 k is a pair, not two pairs.
        ratio: 16.583_123_951_777,
        primary_resistance: 35.0,
        primary_inductance: 40.0,
        leakage: 25e-6,
        saturation_volts: 40.0,
        saturation_hz: 60.0,
        core_sharpness: 6.0,
        speaker: 8.0,
        // From the 8-16 ohm tap, through the presence network.
        feedback: 100_000.0,
        presence_pot: 25_000.0, // the sheet's 25 k
        presence_cap: 0.1e-6,   // C52
        cut_pot: 0.0,
        cut_cap: 0.0,
        cut_rest: 0.0,
        driver: None,
        feedback_network: None,
    };

    /// The Peavey EVH 5150's, read off page 5 of Peavey's drawing at 200 dpi.
    ///
    /// Four 6L6GC, two a side, into a 70500207 transformer with 8 and 4 ohm
    /// taps. The reference numbers are the drawing's own.
    pub const EVH5150: PowerSpec = PowerSpec {
        name: "EVH 5150 power amp",
        // VR7, marked ULTRA POST. Two thirds up, which is a loud room.
        master: 1_000_000.0,
        master_rest: 0.66,
        pi_couple: 0.022e-6, // C49
        driver_volts: 0.0,
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
        pi_plate_cap: 0.0,

        couple: 0.047e-6,       // C28, C29
        grid_leak: 220_000.0,   // R54, R55
        stopper: 2_200.0,       // R202, R206
        screen_resistor: 100.0, // R203, R207, 5 W
        tubes_per_side: 2.0,    // V5+V7 and V6+V8
        tube: PentodeSpec::T6L6GC,
        bias: -46.0,
        cathode_bias: 0.0,
        cathode_bypass: 0.0,

        plate_supply: 470.0,
        screen_supply: 465.0,
        supply_resistance: 150.0,
        rectifier: None,
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

        feedback: 39_000.0, // R56
        // The presence is the feedback network's (`FeedbackNetwork`).
        presence_pot: 0.0,
        presence_cap: 0.0,
        cut_pot: 0.0,
        cut_cap: 0.0,
        cut_rest: 0.0,
        driver: None,
        feedback_network: Some(&FeedbackNetwork::PEAVEY_5150),
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
        // Stands in for the amplifier's MASTER, which on the drawing is a 1 M
        // control after the V2A recovery stage and ahead of the equaliser; the
        // Lead Master itself is in the preamplifier (`markiic::LEAD_MASTER`).
        // Well down, because a IIC+ is played that way: gain in the
        // preamplifier, level at the master.
        master: 1_000_000.0,
        master_rest: 0.30,
        pi_couple: 0.1e-6,
        driver_volts: 0.0,
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
        pi_plate_cap: 0.0,

        couple: 0.1e-6,
        grid_leak: 220_000.0,
        stopper: 1_500.0,
        screen_resistor: 470.0,
        tubes_per_side: 1.0, // a pair, not two pairs -- the 60 W C+
        tube: PentodeSpec::T6L6GC,
        bias: -44.0,
        cathode_bias: 0.0,
        cathode_bypass: 0.0,

        plate_supply: 440.0,
        screen_supply: 435.0,
        // Mesa runs a stiffer supply than Peavey and it is part of why the
        // amplifier feels tighter under the hand.
        supply_resistance: 110.0,
        rectifier: None,
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
        cut_pot: 0.0,
        cut_cap: 0.0,
        cut_rest: 0.0,
        driver: None,
        feedback_network: None,
    };

    /// The Fender Twin Reverb AB763's, off the manufacturer's schematic.
    ///
    /// Four 6L6GC, two a side, behind a 12AT7 long-tailed pair, into a
    /// 125A29A. The amplifier this catalogue's clean reference is built on,
    /// and the half of it that makes it one: eighty-five watts is not loudness
    /// for its own sake, it is the reason the amplifier does not run out of
    /// room where the British ones do.
    ///
    /// Three things separate it from the two amplifiers already here.
    ///
    /// **A 12AT7 inverter, not a 12AX7.** A third of the amplification and a
    /// good deal less plate resistance, which is what lets the pair swing four
    /// output grids without running out of drive itself. Mesa and Peavey both
    /// use the higher-mu tube because their preamplifiers hand the power stage
    /// far more to work with.
    ///
    /// **No master volume, and no presence.** The AB763 has neither. The
    /// channel Volume is the only level control, so the master here rests wide
    /// open; and the drawing takes the feedback from the secondary straight to
    /// the tail through 820 ohms with nothing tapping it. The presence figures
    /// below are how "there is no presence control" is written in a spec that
    /// assumes one: a capacitor small enough to be an open circuit at any
    /// frequency the amplifier passes.
    ///
    /// **A stiff supply.** Solid-state rectification and a choke, and the
    /// whole design intent is that it does not sag. That is set here by the
    /// supply resistance, which is lower than Mesa's, rather than by a larger
    /// reservoir -- the AB763's filters are small by modern standards and the
    /// stiffness comes from the transformer and the rectifier.
    /// The AB763 Deluxe Reverb's: one 6V6GT a side, fixed bias, behind a GZ34.
    /// The matched power stage of the American Deluxe preamplifier. Every value
    /// below is read off the original Fender drawing, which is in
    /// `docs/schematics/fender_deluxe_reverb_ab763.pdf`. See
    /// `docs/models/american_deluxe.md`.
    ///
    /// Worth reading beside `TWIN` above, because the two are the *same
    /// circuit* at the phase inverter -- 0.001 uF in, 1 M/1 M leaks, a 470 ohm
    /// cathode, a 22 k tail, 0.1 uF across to the undriven grid and 82 k/100 k
    /// plates -- and completely different behind it. That is the whole point of
    /// having both: 22 W of 6V6 behind a valve rectifier against 85 W of 6L6
    /// behind silicon.
    pub const DELUXE_6V6: PowerSpec = PowerSpec {
        name: "Deluxe Reverb 6V6 (AB763)",
        // No master volume on an AB763. Wide open, so it is not one.
        master: 1_000_000.0,
        master_rest: 1.0,
        // The drawing's 0.001 uF is modelled in `circuits::deluxe`, on the
        // mixer plate where it is drawn, so that the channel hands off an AC
        // signal like every other circuit here. This is a short across the
        // module boundary so the capacitor is counted once -- 10 uF into the
        // 2 MOhm leak chain is a corner at eight thousandths of a hertz. It is
        // deliberately not zero: zero means a *direct*-coupled inverter, which
        // rearranges `pi_leak_upper` into a series resistor, and that is the
        // Hiwatt's cathode-follower topology, not this one.
        pi_couple: 10e-6,
        driver_volts: 0.0,
        // As on the Twin: no separate grid stopper here. The 22 k on the
        // drawing is the long-tail resistor.
        pi_stopper: 0.0,
        pi_leak_upper: 1_000_000.0,
        pi_leak_lower: 1_000_000.0,
        pi_cathode: 470.0,
        pi_tail: 22_000.0,
        // 47 ohms, where the Twin has 100. This is the one component that sets
        // how much of the loop comes back, and it is why the Deluxe is the
        // livelier of the two: less feedback, sooner.
        pi_tail_lower: 47.0,
        pi_cross: 0.1e-6,
        // 82 k driven, 100 k undriven, both 5 % parts on the drawing.
        pi_plate_driven: 82_000.0,
        pi_plate_other: 100_000.0,
        // The drawing labels the inverter's supply node +325 V, against the
        // Twin's 450. A 12AT7 long-tailed pair with a third less rail runs out
        // of room a good deal earlier, which is most of why a Deluxe starts to
        // compress where a Twin is still clean.
        pi_supply: 325.0,
        pi_tube: TriodeSpec::ECC81,
        pi_plate_cap: 0.0,

        couple: 0.1e-6,
        grid_leak: 220_000.0,
        // One valve a side here, so these are the drawing's values as they
        // stand -- no paralleling to divide out, unlike the Twin.
        stopper: 1_500.0,
        screen_resistor: 470.0,
        tubes_per_side: 1.0,
        tube: PentodeSpec::T6V6GT,
        // The drawing's bias supply: 470 ohm, one diode, 25 uF and a 10 k
        // linear bias control, labelled -35 V.
        bias: -35.0,
        cathode_bias: 0.0,
        cathode_bypass: 0.0,

        // +415 V on both plates on the drawing. The screens hang on the same
        // node through their 470 ohm 1 W resistors rather than on a separate
        // choke-fed rail, so the screen supply is that node too.
        plate_supply: 415.0,
        screen_supply: 415.0,
        // ESTIMATED. The drawing carries no winding resistance. This is the
        // secondary copper of a 22 W mains transformer, and it is deliberately
        // higher than the Twin's 70 ohms: the GZ34 in front of it is the larger
        // part of this amplifier's sag and is modelled as the valve it is.
        supply_resistance: 150.0,
        rectifier: Some(RectifierSpec::GZ34),
        // 16 uF/450 V on the drawing, which is a small reservoir even by
        // blackface standards and is the rest of why the rail moves.
        reservoir: 16e-6,
        // Shared node: a small resistance rather than zero, so the screens
        // still see their own local impedance.
        screen_resistance: 10.0,
        screen_reservoir: 16e-6,

        // 125A1A, 6.6 k plate to plate into 8 ohms: `sqrt(6600 / 8)`.
        ratio: 28.7228132326901,
        // ESTIMATED: no winding data for the 125A1A was found. A 22 W
        // transformer wound for 6.6 k needs more turns than the Twin's 1.9 k
        // one, so more copper and more magnetising inductance; 17 H against
        // 6.6 k puts the bottom corner near 62 Hz, which is where a small
        // Fender's bass gives out.
        primary_resistance: 180.0,
        primary_inductance: 17.0,
        leakage: 45e-6,
        // Twenty-two watts into eight ohms is 13.3 V rms, 18.8 V peak.
        saturation_volts: 18.7616630392937,
        saturation_hz: 70.0,
        core_sharpness: 6.0,
        speaker: 8.0,

        // 820 ohms from the secondary to the tail, as on the Twin. What
        // differs is the 47 ohm leg above, not this.
        feedback: 820.0,
        // No presence control. One femtofarad is an open circuit.
        presence_pot: 5_000.0,
        presence_cap: 1e-15,
        cut_pot: 0.0,
        cut_cap: 0.0,
        cut_rest: 0.0,
        driver: None,
        feedback_network: None,
    };

    pub const TWIN: PowerSpec = PowerSpec {
        name: "Twin Reverb power amp",
        // There is no master volume on an AB763. Left wide open so it is not
        // one, rather than pretending to a control the amplifier has not got.
        master: 1_000_000.0,
        master_rest: 1.0,
        pi_couple: 0.001e-6,
        driver_volts: 0.0,
        // The stock AB763 has no separate 22 kΩ series grid stopper here;
        // that value belongs to the long-tail resistor.
        pi_stopper: 0.0,
        pi_leak_upper: 1_000_000.0,
        pi_leak_lower: 1_000_000.0,
        pi_cathode: 470.0,
        pi_tail: 22_000.0,
        pi_tail_lower: 100.0,
        pi_cross: 0.1e-6,
        // 82 k and 100 k, both five per cent parts on the drawing. Unequal on
        // purpose: the undriven side has less gain and the larger load evens
        // the two halves up before they reach the output grids.
        pi_plate_driven: 82_000.0,
        pi_plate_other: 100_000.0,
        pi_supply: 450.0,
        pi_tube: TriodeSpec::ECC81,
        pi_plate_cap: 0.0,

        couple: 0.1e-6,
        grid_leak: 220_000.0,
        // The nonlinear Pentode below represents the two parallel 6L6GCs on
        // one phase as one device with twice the current. The real amp gives
        // *each* valve a 1.5 kΩ grid stopper and 470 Ω screen resistor, so the
        // exact equivalent seen by the aggregate device is the parallel value.
        stopper: 750.0,
        screen_resistor: 235.0,
        tubes_per_side: 2.0, // four 6L6GC
        tube: PentodeSpec::T6L6GC,
        bias: -52.0,
        cathode_bias: 0.0,
        cathode_bypass: 0.0,

        plate_supply: 460.0,
        screen_supply: 458.0,
        // Stiffer than the Mesa, which is the whole point of the amplifier.
        supply_resistance: 70.0,
        rectifier: None,
        // The original supply uses the much smaller blackface filter bank,
        // followed by the choke-fed screen node; do not give the Twin a
        // modern 200/100 uF reservoir by accident. Two 70 uF series caps are
        // about 35 uF effective on the plate reservoir and the B node is 20 uF.
        reservoir: 35e-6,
        screen_resistance: 100.0,
        screen_reservoir: 20e-6,

        // Roughly 1.9 k plate to plate into the 4 ohms two eight-ohm speakers
        // make in parallel: `sqrt(1900 / 4)`.
        ratio: 21.8,
        primary_resistance: 30.0,
        // Larger iron than the Mesa's, which puts the bottom corner lower --
        // an amplifier known for its bottom end rather than for tightness.
        primary_inductance: 60.0,
        leakage: 22e-6,
        // Eighty-five watts into four ohms is 18.4 V rms, 26 V peak. Fender
        // sized this transformer generously, so it holds further down than the
        // Mesa's before the core gives up: 55 Hz against 70.
        saturation_volts: 26.0,
        saturation_hz: 55.0,
        core_sharpness: 6.0,
        speaker: 4.0,

        // 820 ohms from the secondary to the tail, read off the drawing. A
        // great deal more feedback than the Mesa's 100 k, and it is why the
        // amplifier is clean, flat and hard to make misbehave.
        feedback: 820.0,
        // No presence control. One femtofarad is an open circuit.
        presence_pot: 5_000.0,
        presence_cap: 1e-15,
        cut_pot: 0.0,
        cut_cap: 0.0,
        cut_rest: 0.0,
        driver: None,
        feedback_network: None,
    };
}

/// The power amplifier as a netlist, taking volts at its input and giving
/// volts across the speaker.
impl PowerSpec {
    /// What this stage's control 0 (`PRESENCE`) is on its own front panel,
    /// or `None` where it has nothing there.
    ///
    /// Most have a **presence** control in the feedback loop. The AC30 has no
    /// loop and puts a **cut** control across its output grids in the same
    /// slot. The AB763s have neither (their presence capacitor is written as
    /// an open circuit). The DR103's is part of its driver (`DriverSpec`).
    pub fn presence_name(&self) -> Option<&'static str> {
        if self.driver.is_some_and(|d| d.presence_pot > 0.0)
            || self.feedback_network.is_some_and(|f| f.presence_pot > 0.0)
        {
            Some("PRESENCE")
        } else if self.cut_pot > 0.0 {
            Some("CUT")
        } else if self.feedback > 0.0 && self.presence_pot > 0.0 && self.presence_cap > 1e-12 {
            Some("PRESENCE")
        } else {
            None
        }
    }
}

pub fn build(spec: &PowerSpec, source: f64) -> Result<Circuit, Fault> {
    tap(spec, source, "spk")
}

/// The same amplifier brought out at a chosen node, for measuring one stage at
/// a time.
pub fn tap(spec: &PowerSpec, source: f64, at: &str) -> Result<Circuit, Fault> {
    assemble(spec, source, at, None).map(|(circuit, _)| circuit)
}

/// The amplifier driving a loudspeaker instead of a resistor. The driver is
/// stamped into this netlist, at the transformer tap the spec was drawn for, so
/// the tubes, the transformer and the feedback loop all see its impedance. See
/// `acoustics::speaker`.
pub fn build_with_speaker(
    spec: &PowerSpec,
    source: f64,
    load: &LoadValues,
) -> Result<(Circuit, LoadSlots), Fault> {
    build_with_speaker_and(spec, source, load, &Parasitics::of(spec))
}

/// As `build_with_speaker`, with the high-frequency parasitics stated explicitly.
pub fn build_with_speaker_and(
    spec: &PowerSpec,
    source: f64,
    load: &LoadValues,
    parasitics: &Parasitics,
) -> Result<(Circuit, LoadSlots), Fault> {
    assemble(spec, source, "spk", Some((load, parasitics)))
        .map(|(circuit, slots)| (circuit, slots.expect("a speaker was stamped")))
}

/// The capacitance a resistor-loaded power netlist leaves out.
///
/// Into a resistor it barely matters, and the legacy netlists every calibration
/// was measured on do not have it. Into a loudspeaker it does. When the output
/// tubes cut off hard into an inductive load, only the transformer's winding
/// capacitance limits how fast the plates can fly. Without it, a heavily fed-back
/// stage (Twin, 5150) driven far into clipping at high frequency failed to
/// converge and swung to a kilovolt at 192 kHz. Measured with the lossy coil:
/// 1.5-4.7 nF removed every unsettled solve and bounded the output, and changed
/// moderate-drive output by under 0.1 %.
///
/// No winding capacitance is published for any of these transformers. Hammond's
/// 1750U (2203 replacement) response tolerance, 50 Hz-12 kHz at 0/-1 dB, bounds it
/// below about 11 nF. So `of` is ESTIMATED by one rule: the capacitance that puts the
/// loaded primary's high-frequency corner at 30 kHz. The published tube
/// interelectrode capacitances (JJ EL34, RCA 6L6GC) were tried as well. With this
/// otherwise incomplete high-frequency network they *reduced* stability margins,
/// so they are not stamped. See docs/models/speakers.md.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parasitics {
    /// Across the whole primary, plate to plate, farads. Zero means not stamped.
    pub primary_cap: f64,
}

impl Parasitics {
    pub const NONE: Parasitics = Parasitics { primary_cap: 0.0 };

    /// Where the loaded primary's high-frequency corner is put. See above.
    pub const PRIMARY_CORNER_HZ: f64 = 30_000.0;

    pub fn of(spec: &PowerSpec) -> Parasitics {
        let raa = spec.ratio * spec.ratio * spec.speaker;
        Parasitics {
            primary_cap: 1.0 / (std::f64::consts::TAU * Self::PRIMARY_CORNER_HZ * raa),
        }
    }
}

/// The impedance multiplier that refers an 8 ohm driver to this spec's tap.
pub fn speaker_scale(spec: &PowerSpec) -> f64 {
    spec.speaker / crate::acoustics::speaker::SpeakerProfile::NOMINAL_OHMS
}

fn assemble(
    spec: &PowerSpec,
    source: f64,
    at: &str,
    load: Option<(&LoadValues, &Parasitics)>,
) -> Result<(Circuit, Option<LoadSlots>), Fault> {
    let mut net = Netlist::new(spec.name);

    // --- the long-tailed pair ----------------------------------------------
    // The grid-leak chain is two resistors in series and the junction of them
    // is not ground: it carries the joined cathodes through `pi_cathode`, and
    // the tail hangs off it. That is what makes this a pair rather than two
    // stages sharing a socket -- the current one half stops drawing, the other
    // half takes, and the tail voltage is what tells it to.
    // A direct-coupled inverter is handed the driver's direct voltage as well as
    // its signal; a capacitor-coupled one is handed the signal alone.
    let twin_supply = spec.name == PowerSpec::TWIN.name;
    net.input_at("in", source, spec.driver_volts);
    // The AB763 Twin has no master volume. Every other modular power model may
    // keep its real/synthetic interstage master, but inserting a 1 MΩ pot into
    // the Twin adds both a nonexistent divider and a parallel load at the exact
    // hand-off we are trying to model. For the Twin the channel-mix source goes
    // straight to the stock 0.001 µF phase-inverter coupling capacitor.
    let master_node = if twin_supply {
        "in"
    } else {
        net.rest(MASTER, spec.master_rest).pot(
            "in",
            "master",
            "gnd",
            spec.master,
            Taper::Audio,
            MASTER,
        );
        "master"
    };
    // How the inverter's grids get their direct voltage, which is most of what
    // decides whether an amplifier stays clean when it is driven hard.
    //
    // Nearly all of them couple in through a capacitor and return the grids to a
    // junction of their own: `pi_couple` into `pi_a`, then the leaks to `pi_b`.
    // A capacitor there charges when the grid draws current on a peak and takes a
    // moment to give it back, which is the bias shift that makes a Marshall break
    // up the way it does.
    //
    // A Hiwatt does not have one. Its inverter's grids hang on resistors straight
    // off the driver's cathode follower, which holds them at about 75 V however
    // hard it is played. `pi_couple` of zero says so, and then `pi_leak_upper`
    // and `pi_leak_lower` are those two resistors rather than a leak chain.
    let direct = spec.pi_couple <= 0.0;
    if let Some(d) = spec.driver {
        // The Hiwatt's: V3a amplifies and couples in through a capacitor, V3b
        // holds both grids and carries no signal. See `DriverSpec`. Without
        // the gain stage the input stands where V3a's plate would.
        if d.gain_stage {
            net.resistor("v3a_k", "gnd", d.cathode)
                .supply("v3a_p", d.plate, spec.pi_supply)
                .triode("v3a_p", master_node, "v3a_k", d.tube);
        } else {
            net.resistor(master_node, "v3a_p", d.plate_source);
        }
        net.supply("v3b_g", d.reference_upper, spec.pi_supply)
            .resistor("v3b_g", "gnd", d.reference_lower)
            .supply("v3b_p", 1.0, spec.pi_supply)
            .triode("v3b_p", "v3b_g", "v3b_k", d.tube)
            .resistor("v3b_k", "gnd", d.reference_load)
            .capacitor("v3a_p", "pi_a", d.couple)
            .resistor("v3b_k", "pi_a", d.leak_driven)
            .resistor("v3b_k", "pi_g2", d.leak_other);
    } else if direct {
        net.resistor(master_node, "pi_a", spec.pi_leak_upper);
    } else {
        net.capacitor(master_node, "pi_a", spec.pi_couple).resistor(
            "pi_a",
            "pi_b",
            spec.pi_leak_upper,
        );
    }
    let driven_grid = if spec.pi_stopper > 0.0 {
        net.resistor("pi_a", "pi_g1", spec.pi_stopper);
        "pi_g1"
    } else {
        "pi_a"
    };
    // The tail. With a feedback loop it lands on a node of its own, because
    // that node is where the loop comes back; with no loop the tail resistor
    // simply goes to ground and `pi_tail_lower` is zero.
    // The cathodes, and the tail below them. With a leak junction the cathodes
    // hang off it through `pi_cathode` and the tail leaves that junction; with a
    // driver holding the grids there is no junction, and the tail simply leaves
    // the cathodes.
    //
    // The order the parts are added in is the order it was in before any of
    // these variations existed, because the nodes are numbered in that order and
    // the frozen fixture in `tests/legacy_baseline.rs` is compared sample for
    // sample.
    let cathode_top = if direct {
        "pi_k"
    } else {
        net.resistor("pi_b", "pi_k", spec.pi_cathode);
        "pi_b"
    };
    // The second grid comes off the same place the first one does: the leak
    // junction when there is one, the driver when there is not. (A driver
    // stage has already returned it to V3b.)
    if spec.driver.is_none() {
        net.resistor(
            if direct { master_node } else { "pi_b" },
            "pi_g2",
            spec.pi_leak_lower,
        );
    }
    // Where the tail lands: a node of its own when a feedback loop comes back to
    // it, ground when there is no loop to bring back.
    let tail = if spec.pi_tail_lower > 0.0 {
        "tail"
    } else {
        "gnd"
    };
    if spec.pi_tail > 0.0 {
        net.resistor(cathode_top, tail, spec.pi_tail);
    }
    if spec.pi_tail_lower > 0.0 {
        net.resistor(tail, "gnd", spec.pi_tail_lower);
    }
    // And what the undriven grid is tied to, which is that node when there is a
    // tail resistor and the cathodes themselves when there is not.
    let tail = if spec.pi_tail > 0.0 {
        tail
    } else {
        cathode_top
    };
    let cathode = "pi_k";
    if spec.pi_cross > 0.0 {
        net.capacitor(tail, "pi_g2", spec.pi_cross);
    }
    if twin_supply {
        // AB763: both PI plate loads return to the shared +450 V C node, not
        // to two independent ideal supplies. The C node itself is built from
        // the common reservoir/choke chain below.
        net.resistor("pi_c", "pi_p1", spec.pi_plate_driven)
            .resistor("pi_c", "pi_p2", spec.pi_plate_other);
    } else {
        net.supply("pi_p1", spec.pi_plate_driven, spec.pi_supply)
            .supply("pi_p2", spec.pi_plate_other, spec.pi_supply);
    }
    net.triode("pi_p1", driven_grid, cathode, spec.pi_tube)
        .triode("pi_p2", "pi_g2", cathode, spec.pi_tube);
    if spec.pi_plate_cap > 0.0 {
        net.capacitor("pi_p1", "pi_p2", spec.pi_plate_cap);
    }

    // --- the supplies ------------------------------------------------------
    // A voltage behind a resistance with a reservoir across it. Under load the
    // node falls, and it comes back with the time constant of the two -- which
    // for a couple of hundred microfarads behind a hundred and fifty ohms is
    // about thirty milliseconds, or the length of a chord's attack. That is
    // sag, and it is why the note blooms.
    match spec.rectifier {
        None if twin_supply => {
            // Original AB763 doghouse: two 70 uF cans in series (35 uF
            // effective) with 220 k balancing resistors at the +460 V A node,
            // then the 125C1A choke (4 H, ~104 ohm DCR) to +458 V B, then
            // 1 k to the +450 V PI C node. Keeping these as shared nodes means
            // screen-current changes and PI current now move the same supply
            // instead of two unrelated Norton sources.
            net.supply("ht", spec.supply_resistance, spec.plate_supply)
                .capacitor("ht", "ht_mid", 70e-6)
                .capacitor("ht_mid", "gnd", 70e-6)
                .resistor("ht", "ht_mid", 220_000.0)
                .resistor("ht_mid", "gnd", 220_000.0)
                .resistor("ht", "choke_in", 104.0)
                .inductor("choke_in", "scr", 4.0)
                .capacitor("scr", "gnd", 20e-6)
                .resistor("scr", "pi_c", 1_000.0)
                .capacitor("pi_c", "gnd", 20e-6);
        }
        None => {
            net.supply("ht", spec.supply_resistance, spec.plate_supply)
                .capacitor("ht", "gnd", spec.reservoir)
                .supply("scr", spec.screen_resistance, spec.screen_supply)
                .capacitor("scr", "gnd", spec.screen_reservoir);
        }
        Some(valve) => {
            // The transformer behind the valve, the valve, then the reservoir.
            // The screens come off the same rail through their own dropper, so
            // they sag with it and a little further, which is what they do.
            net.supply("raw", spec.supply_resistance, spec.plate_supply)
                .rectifier("raw", "ht", valve)
                .capacitor("ht", "gnd", spec.reservoir)
                .resistor("ht", "scr", spec.screen_resistance)
                .capacitor("scr", "gnd", spec.screen_reservoir);
        }
    }
    // Where the grid leaks return to, and where the valves' cathodes sit.
    // Fixed bias: a stiff negative supply behind the leaks, cathodes on ground.
    // Cathode bias: leaks on ground, cathodes on their own shared resistor.
    let (leak_return, cathode) = if spec.cathode_bias > 0.0 {
        net.resistor("ok", "gnd", spec.cathode_bias)
            .capacitor("ok", "gnd", spec.cathode_bypass);
        ("gnd", "ok")
    } else if twin_supply {
        // AB763 fixed-bias supply. The transformer bias tap/diode is represented
        // by its rectified Thevenin source, followed by the stock 470 Ω feed,
        // 25 µF filter and 10 kΩ linear balance track over the 27 kΩ ground leg.
        // The unexposed balance pot is parked at its electrical midpoint; the
        // source value is chosen so that the unloaded wiper is the schematic's
        // -52 V after the divider and 470 Ω feed. Grid current can now charge
        // and recover through the real filter instead of seeing a naked 22 kΩ
        // synthetic source.
        net.supply("bias_raw", 470.0, -60.888_75)
            .capacitor("bias_raw", "gnd", 25e-6)
            .resistor("bias_raw", "bias", 5_000.0)
            .resistor("bias", "bias_leg", 5_000.0)
            .resistor("bias_leg", "gnd", 27_000.0);
        ("bias", "gnd")
    } else {
        // Stiff generic fixed-bias source used by the other power models.
        net.supply("bias", 22_000.0, spec.bias);
        ("bias", "gnd")
    };

    // --- the output tubes ---------------------------------------------------
    for (side, plate, from) in [(1usize, "pl_a", "pi_p1"), (2, "pl_b", "pi_p2")] {
        let grid = format!("og{side}");
        let node = format!("on{side}");
        let screen = format!("os{side}");
        net.capacitor(from, &node, spec.couple)
            .resistor(&node, leak_return, spec.grid_leak)
            .resistor(&node, &grid, spec.stopper)
            .resistor("scr", &screen, spec.screen_resistor)
            .pentode(
                plate,
                &grid,
                cathode,
                &screen,
                spec.tubes_per_side,
                spec.tube,
            );
    }
    // The cut control, across the two grids it has just built.
    if spec.cut_pot > 0.0 {
        net.rest(PRESENCE, spec.cut_rest)
            .pot("on1", "cut", "cut", spec.cut_pot, Taper::Audio, PRESENCE)
            .capacitor("cut", "on2", spec.cut_cap);
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
    // The centre tap *is* the supply node. It was joined to it through an
    // invented one ohm, which is the thing §59.1 says not to do: an unknown
    // in a matrix solved fifty thousand times a second, bought to make the
    // drawing read more like the drawing. It also put a one siemens branch
    // next to grid leaks of a few microsiemens, which is six orders of
    // conditioning spent on nothing.
    if twin_supply {
        // The 125A29A primary is centre tapped: each half has its own copper
        // resistance and magnetising path from the +460 V tap to its plate.
        // A single resistor before the split makes one phase's current sag the
        // other phase through copper that is not shared, and doubles the DC
        // drop for a balanced pair. Keep the two real half-windings explicit.
        net.resistor("ht", "m_a", spec.primary_resistance)
            .inductor("m_a", "pl_a", half)
            .resistor("ht", "m_b", spec.primary_resistance)
            .inductor("m_b", "pl_b", half);
    } else {
        net.resistor("ht", "m", spec.primary_resistance)
            .inductor("m", "pl_a", half)
            .inductor("m", "pl_b", half);
    }
    net.transformer("pl_a", "ht", "sec", "gnd", each)
        .transformer("ht", "pl_b", "sec", "gnd", each);
    if twin_supply {
        // The Twin's low-frequency output-transformer saturation is the one
        // measured source of audible 48 kHz fold-back. Antialias only this
        // nonlinear excess branch; the 31-unknown power solve stays at 1x.
        net.core_antialiased("sec", "gnd", secondary_core);
    } else {
        net.core("sec", "gnd", secondary_core);
    }
    net.inductor("sec", "spk", spec.leakage);
    // The load, in the same place in the part order either way: the resistive
    // netlist has to stay exactly the one every calibration was measured on.
    let slots = match load {
        None => {
            net.resistor("spk", "gnd", spec.speaker);
            None
        }
        Some((values, parasitics)) => {
            if parasitics.primary_cap > 0.0 {
                net.capacitor("pl_a", "pl_b", parasitics.primary_cap);
            }
            Some(speaker::stamp(&mut net, "spk", values))
        }
    };

    // --- feedback and presence ----------------------------------------------
    // Back to the tail, where it opposes the signal. The presence control
    // shunts the feedback to ground through a capacitor, so turning it up
    // removes treble *from the loop* -- which puts treble back in the output.
    // A presence control is the only tone control that works by taking
    // something away from the amplifier's own correction.
    if let (true, Some(f)) = (spec.feedback > 0.0, spec.feedback_network) {
        // The Peavey's: R56 into the resonance network, then C30 into the tail,
        // with the presence hanging off the node between. `pot(a, wiper, b)`
        // puts `R f(p)` between wiper and `b`: resonance is a variable resistor
        // that puts resistance in as it turns up (the reverse law, as a bass
        // control's), presence has its wiper at ground turned up.
        net.resistor("spk", "fb_r", spec.feedback) // R56
            .capacitor("fb_r", "fb_p", f.resonance_cap) // C12
            .rest(RESONANCE, f.resonance_rest)
            .pot(
                "fb_r",
                "fb_p",
                "fb_p",
                f.resonance_pot,
                Taper::ReverseAudio,
                RESONANCE,
            ) // VR8
            .capacitor("fb_p", tail, f.coupling) // C30
            .capacitor("fb_p", "pres_top", f.presence_top_cap) // C8
            .capacitor("fb_p", "pres_w", f.presence_wiper_cap) // C62
            .pot(
                "gnd",
                "pres_w",
                "pres_top",
                f.presence_pot,
                Taper::Audio,
                PRESENCE,
            ); // VR9
    } else if spec.feedback > 0.0 {
        net.resistor("spk", tail, spec.feedback);
        // Not every amplifier with a loop puts a presence control in it. The
        // Hiwatt's is in its preamplifier and is not built; see `circuits::dr103`.
        if spec.presence_pot > 0.0 {
            net.capacitor(tail, "pres", spec.presence_cap).pot(
                "pres",
                "gnd",
                "gnd",
                spec.presence_pot,
                Taper::Audio,
                PRESENCE,
            );
        }
        // The Hiwatt's feedback node and presence, which reach back to V3a's
        // plate. `pot(a, wiper, b)` puts `R f(p)` between wiper and `b`, so
        // turned up the wiper is at the feedback end. See `DriverSpec`.
        if let Some(d) = spec.driver {
            net.resistor(tail, "fb_shunt", d.tail_shunt_r)
                .capacitor("fb_shunt", "gnd", d.tail_shunt_c)
                .capacitor(tail, "pres_fb", d.presence_feedback_cap)
                .rest(PRESENCE, d.presence_rest)
                .pot(
                    "pres_fb",
                    "pres_w",
                    "pres_plate",
                    d.presence_pot,
                    Taper::Linear,
                    PRESENCE,
                )
                .resistor("pres_w", "gnd", d.presence_wiper)
                .capacitor("pres_plate", "v3a_p", d.presence_plate_cap);
        }
    }

    Ok((net.build(at)?, slots))
}

#[cfg(test)]
mod twin_reduction_tests {
    use super::*;
    use crate::dsp::time::Simulation;

    #[test]
    fn twin_power_uses_minimal_exact_nonlinear_schur_core() {
        let circuit = build(&PowerSpec::TWIN, 10_000.0).expect("Twin power amp builds");
        let sim = Simulation::new(circuit, 48_000.0);
        let reduction = sim
            .nonlinear_reduction()
            .expect("Twin power should use exact nonlinear Schur reduction");
        println!(
            "twin_power_schur,unknowns={},boundary={},internal={},boundary_names={:?}",
            sim.unknowns(),
            reduction.0,
            reduction.1,
            sim.nonlinear_boundary_names()
        );
        assert_eq!(sim.unknowns(), 31, "unexpected Twin power MNA size");
        assert_eq!(
            reduction,
            (13, 18),
            "Twin power exact Schur core changed: every PI/6L6/core terminal touched by a nonlinear stamp must remain on the Newton boundary"
        );
    }
}
