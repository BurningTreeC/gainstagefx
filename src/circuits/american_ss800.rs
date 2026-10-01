//! The American SS 800: the Gallien-Krueger 800RB's 300 W amplifier, the
//! LO master in front of it.
//!
//! Engineering reference (developer documentation, not panel text): GK drawing
//! 406-0044-B "100W + 300W POWER AMPS", 9/3/91, and the supply 406-0048-B, from
//! GK's own service manual. See `docs/models/american_800rb.md`.
//!
//! **Not a `PowerSpec`**, for the reason the JC-120's amplifier is not
//! (`circuits::jc120_power`): no inverter, no grid leaks, no output
//! transformer.
//!
//! ```text
//! LO master -- C85 -- R28 / R29 -- U1 LF353, local R30 + C8 and D7, global
//!     feedback R32 // C10 to +in against R31 and R33 + C3   (x62, 36 dB)
//!   -- R34 -- Q11 common base -- Q12 MJ15023 from +85 V through R37  (the VAS)
//!   -- the Q13 / Q14 bias spreader (R38A, BIAS R38, R39), C11 the Miller cap
//!   -- the bootstrapped load R40 / R41 with C14 from the output
//!   -- Q17 / Q21 drivers, three MJ15022 / MJ15023 a side behind 0.33 ohm each,
//!      Q15 / Q16 limiting the drive through D8 / D9
//!   -- the speaker, the 5 ohm + 0.1 uF Zobel, D11 / D12 to the rails
//! ```

use crate::acoustics::speaker::{self, LoadSlots, LoadValues};
use crate::circuits::power;
use crate::dsp::netlist::{BipolarSpec, Circuit, DiodeSpec, Fault, Netlist, Taper};

/// The LO master, R118 50 k B, at the amplifier's input: `power::MASTER`, so
/// the panel's Master knob turns it whenever this stage is chosen behind a
/// circuit without its own level control.
pub const MASTER: usize = power::MASTER;
/// R38 BIAS, 1 k, in the spreader: GK's procedure sets it for 5 mV across the
/// most positive of R48-R50 and the most negative of R54-R56 (15 mA a device).
pub const BIAS: usize = 2;

/// Where the bias pot rests: fitted to the procedure by
/// `examples/american_ss800_op.rs`.
pub const BIAS_REST: f64 = 0.2420;
/// The masters, as the manual has them for the cleanest sound: "near or on 10".
pub const MASTER_REST: f64 = 1.0;

/// The rails, each side the drawing's 85 V behind a resistance, with the
/// 3 x 1000 uF reservoir. The supply (406-0048-B) is a bridge and those
/// capacitors, unregulated: under a sustained tone the transformer's
/// regulation and the reservoir's ripple troughs take the rails well down,
/// and GK's clipping figures are where they end up. The solver's supplies are
/// direct, so the two are one resistance here, fitted to the turn-on
/// procedure's 36 V rms into 4 ohm at slight clipping
/// (`examples/american_ss800_op.rs`). ESTIMATED. Into 8 ohm it then clips
/// above the rated 200 W; a transient gets the full 85 V until the reservoir
/// sags, which is how an unregulated bass amplifier plays.
pub const RAIL_OPEN: f64 = 85.0;
pub const RAIL_SOURCE: f64 = 7.52;
const RESERVOIR: f64 = 3_000e-6;

/// U1, an LF353 on the +-15 V regulated rails: Texas's typical 4 MHz, 13 V/us
/// and 100 V/mV, and a swing of +-13.5 V (ESTIMATED).
const GBW_HZ: f64 = 4.0e6;
const SLEW: f64 = 13.0e6;
const OPEN_LOOP: f64 = 1.0e5;
const SWING: f64 = 13.5;
/// The integrating capacitor of the op-amp model; see `lf353`.
const INTEGRATOR: f64 = 1e-9;

/// MJ15022 / MJ15023: ON's 16 A, 200 V (250 W) complementary pair, here the VAS
/// (Q12), the drivers (Q17, Q21) and the outputs alike -- one part number at
/// three very different currents, and a power transistor's beta is nothing
/// like constant across them: hFE 15-60 at 8 A on ON's sheet, rising toward
/// lower currents. The model's beta is one figure a device, so each place gets
/// the figure for its own current: about 60 for the outputs at an ampere or
/// four, 100 for the drivers at a few hundred milliamperes, 80 for the VAS at
/// 15 mA. A saturation current for about 0.75 V at an ampere. ESTIMATED from
/// the curves of the class of part, as the JC-120's pair is. The drivers' and
/// outputs' betas multiply into the impedance the VAS works into, so they set
/// the open-loop gain the feedback has to work with.
const MJ15022: BipolarSpec = BipolarSpec {
    saturation: 3.0e-13,
    forward_beta: 100.0,
    reverse_beta: 3.0,
    early: 150.0,
};
const MJ15023: BipolarSpec = MJ15022;
/// Q12, the same part at the VAS's 15 mA.
const VAS: BipolarSpec = BipolarSpec {
    forward_beta: 80.0,
    ..MJ15022
};
/// The three output devices a side folded into one: three times the
/// saturation current, their beta at their current, behind a third of 0.33
/// ohm. Exact for identical devices, as the 2205's bridge folded to one diode
/// each way.
const OUTPUTS: f64 = 3.0;
const OUT_N: BipolarSpec = BipolarSpec {
    saturation: MJ15022.saturation * OUTPUTS,
    forward_beta: 60.0,
    ..MJ15022
};
const OUT_P: BipolarSpec = OUT_N;
const EMITTER_R: f64 = 0.33 / OUTPUTS;
/// MPSA06 / MPSA56: the small-signal pair of the common-base stage, the
/// spreader and the limiters. hFE at least 100; ESTIMATED.
const MPSA06: BipolarSpec = BipolarSpec {
    saturation: 1.0e-14,
    forward_beta: 200.0,
    reverse_beta: 3.0,
    early: 100.0,
};
const MPSA56: BipolarSpec = MPSA06;
/// The 1N4002s, as the catalogue's silicon diode.
const D1N4002: DiodeSpec = DiodeSpec::SILICON;

pub const OUTPUT: &str = "out";

/// U1 as the solver's transconductor into an integrator -- the part's own
/// bandwidth and slew -- behind a follower, its swing two diodes on the
/// integrator node, the way `rodent::lm308` builds an LM308.
///
/// Not the ideal op-amp every other stage here uses, and this is the one place
/// it matters: U1 runs with R30 over R29, 179 times, of local gain inside the
/// global loop, which a real LF353 has run out of gain-bandwidth for by about
/// 22 kHz. Built ideal, the amplifier latched U1 to a rail within a tenth of a
/// second of silence; built with the part's GBW, slew and open-loop gain
/// together, it idles (`tests/american_800rb.rs`). Which of the three matters
/// was not separated.
fn lf353(net: &mut Netlist, plus: &str, minus: &str, out: &str) {
    let gm = std::f64::consts::TAU * GBW_HZ * INTEGRATOR;
    let drop = 0.55;
    net.transconductor(plus, minus, "u1_int", "gnd", gm, SLEW * INTEGRATOR)
        .capacitor("u1_int", "gnd", INTEGRATOR)
        .resistor("u1_int", "gnd", OPEN_LOOP / gm)
        .supply("u1_high", 1.0, SWING - drop)
        .supply("u1_low", 1.0, -SWING + drop)
        .diode("u1_int", "u1_high", DiodeSpec::SILICON)
        .diode("u1_low", "u1_int", DiodeSpec::SILICON)
        .opamp(out, "u1_int", out, 1_000.0);
}

fn complete_netlist(source: f64, load: f64, rail_open: f64, rail_source: f64) -> Netlist {
    let mut net = Netlist::new("American SS 800");

    // --- the rails -------------------------------------------------------------------
    net.supply("vp", rail_source, rail_open)
        .capacitor("vp", "gnd", RESERVOIR) // C9-C11
        .supply("vn", rail_source, -rail_open)
        .capacitor("vn", "gnd", RESERVOIR); // C12-C14

    // --- the LO master and U1 ---------------------------------------------------------
    net.input("in", source)
        .rest(MASTER, MASTER_REST)
        .pot("in", "mw", "gnd", 50_000.0, Taper::Linear, MASTER) // R118
        .capacitor("mw", "pin", 0.22e-6) // C85
        .resistor("pin", "gnd", 100_000.0) // R28
        .resistor("pin", "um", 5_600.0); // R29
    lf353(&mut net, "up", "um", "uo"); // U1
    net.capacitor("um", "c8", 0.1e-6) // C8
        .resistor("c8", "uo", 1_000_000.0) // R30
        .diode("uo", "um", D1N4002) // D7
        .resistor("up", "gnd", 12_000.0) // R31
        .resistor(OUTPUT, "up", 56_000.0) // R32
        .capacitor(OUTPUT, "up", 47e-12) // C10
        .resistor("up", "c3", 1_000.0) // R33
        .capacitor("c3", "gnd", 10e-6); // C3

    // --- the common-base stage and the VAS -----------------------------------------------
    net.resistor("uo", "e11", 3_300.0) // R34
        .resistor(OUTPUT, "e11", 56_000.0) // R35
        .bipolar("qc", "gnd", "e11", MPSA06) // Q11
        .resistor("vp", "qc", 1_000.0) // R36
        .resistor("vp", "e12", 100.0) // R37
        .bipolar_pnp("vas", "qc", "e12", VAS) // Q12
        .capacitor("qc", "vas", 100e-12); // C11

    // --- the spreader and the bootstrapped load -------------------------------------------
    // Q14 from the VAS to the lower driver's base, its base on Q13's collector;
    // Q13 sensing R38A / BIAS / R39 between the two; C12 across Q14's base.
    net.resistor("vas", "s1", 3_300.0) // R38A
        .rest(BIAS, BIAS_REST)
        .pot("s1", "sw", "s2", 1_000.0, Taper::Linear, BIAS) // R38
        .resistor("s2", "bot", 1_000.0) // R39
        .bipolar_pnp("bot", "s14b", "vas", MPSA56) // Q14
        .bipolar("s14b", "sw", "bot", MPSA06) // Q13
        .capacitor("s14b", "bot", 10e-9) // C12
        .resistor("bot", "bs", 2_700.0) // R40
        .resistor("bs", "vn", 2_700.0) // R41
        .capacitor("bs", OUTPUT, 220e-6); // C14

    // --- drivers, outputs and the limiters ---------------------------------------------------
    net.bipolar("vp", "vas", "de1", MJ15022) // Q17
        .bipolar_pnp("vn", "bot", "de2", MJ15023) // Q21
        .resistor("de1", "n1", 22.0) // R47
        .diode("n1", "n2", D1N4002) // D10
        .resistor("n2", "de2", 22.0) // R53
        .resistor("n1", OUTPUT, 470.0) // R51
        .resistor("n2", OUTPUT, 470.0) // R52
        .bipolar("vp", "de1", "eh", OUT_N) // Q18-Q20
        .resistor("eh", OUTPUT, EMITTER_R) // R48-R50
        .bipolar_pnp("vn", "de2", "el", OUT_P) // Q22-Q24
        .resistor("el", OUTPUT, EMITTER_R) // R54-R56
        // Q15 senses Q18's emitter resistor through R43 / R44 and pulls the
        // VAS toward the output through D8; Q16 the mirror of it.
        .resistor("eh", "b15", 1_000.0) // R43
        .resistor("b15", OUTPUT, 470.0) // R44
        .diode("vas", "c15", D1N4002) // D8
        .bipolar("c15", "b15", OUTPUT, MPSA06) // Q15
        .resistor("el", "b16", 1_000.0) // R46
        .resistor("b16", OUTPUT, 470.0) // R45
        .diode("c16", "bot", D1N4002) // D9
        .bipolar_pnp("c16", "b16", OUTPUT, MPSA56); // Q16

    // --- the output network ---------------------------------------------------------------------
    net.resistor(OUTPUT, "zobel", 5.0) // R57
        .capacitor("zobel", "gnd", 0.1e-6) // C13
        .diode(OUTPUT, "vp", D1N4002) // D11
        .diode("vn", OUTPUT, D1N4002); // D12
    if load > 0.0 {
        net.resistor(OUTPUT, "gnd", load);
    }
    net
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    complete_netlist(source, load, RAIL_OPEN, RAIL_SOURCE).build(OUTPUT)
}

/// The same amplifier read at a chosen node, with the rails stated, for
/// probing and for fitting them.
pub fn tap_with(
    source: f64,
    load: f64,
    rail_open: f64,
    rail_source: f64,
    at: &str,
) -> Result<Circuit, Fault> {
    complete_netlist(source, load, rail_open, rail_source).build(at)
}

/// Into a real driver rather than a resistor, for the acoustics path. See
/// `jc120_power::build_with_speaker` for why it matters.
pub fn build_with_speaker(source: f64, load: &LoadValues) -> Result<(Circuit, LoadSlots), Fault> {
    let mut net = complete_netlist(source, 0.0, RAIL_OPEN, RAIL_SOURCE);
    let slots = speaker::stamp(&mut net, OUTPUT, load);
    Ok((net.build(speaker::MOTIONAL)?, slots))
}
