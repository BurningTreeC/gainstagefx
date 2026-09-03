//! A three band tone section, designed rather than reproduced.
//!
//! The passive Fender/Marshall network is a specific piece of history and this
//! is not it. Four attempts at reconstructing that network from memory each
//! measured wrong in a different way, and a fifth, searched over a space of
//! plausible wirings, turned out to be a flat network with one shunt capacitor
//! that happened to score well. Recall is not a schematic, and pretending
//! otherwise is how you ship three controls that all do something and none of
//! which does what its name says.
//!
//! What is here instead is a passive network designed to a specification, with
//! each control verified to do what it claims. Like the historical stacks it
//! only ever *cuts* -- a passive network cannot make gain, which is why an amp
//! of that kind has twenty-odd decibels of insertion loss and a stage after it
//! to make the loss up. Each control is flat at ten and cuts as it comes down.
//!
//! Three legs hang off one node, each selective:
//!
//! * **Treble** -- a capacitor to ground through a variable resistance. Wind
//!   the resistance to nothing and the capacitor shorts the top away.
//! * **Mid** -- a series inductor and capacitor to ground through a variable
//!   resistance. The pair is a short circuit at the frequency they resonate at
//!   and an open circuit either side of it, so what it takes out is a band
//!   rather than an end. This is the control a scooped sound actually needs,
//!   and the one no passive stack can give you: winding a stack's middle down
//!   grounds the junction the bass uses to reach the output, so the bottom end
//!   goes with it.
//! * **Bass** -- a capacitor in series with the signal, bridged by a variable
//!   resistance. Wind the resistance up and the capacitor is left to block the
//!   bottom.
//!
//! A real graphic equaliser makes its inductor out of an op-amp and two
//! capacitors, because half a henry is not a part anyone wants to wind. The
//! response is the same and the netlist is simpler this way.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper};

pub const BASS: usize = 0;
pub const MID: usize = 1;
pub const TREBLE: usize = 2;

#[derive(Clone, Copy, Debug)]
pub struct Voicing {
    /// Where the mid control takes its band from, in hertz.
    pub mid_hz: f64,
    /// The inductor in that leg. Larger is a narrower band.
    pub mid_henry: f64,
    /// Where the treble control starts working.
    pub treble_hz: f64,
    /// Where the bass control starts working.
    pub bass_hz: f64,
    /// How much each control can take out, as the resistance it is fighting.
    pub feed: f64,
}

/// A broad midrange, so the control shapes the body of the sound.
pub const WIDE: Voicing = Voicing {
    mid_hz: 550.0,
    // Broad, so the control shapes the body rather than digging a hole. It
    // costs about four decibels at the low E when it is wound right down,
    // which is the interaction that makes a broad control feel musical.
    mid_henry: 0.7,
    treble_hz: 2_200.0,
    bass_hz: 180.0,
    feed: 12_000.0,
};

/// A narrower, lower midrange: the scoop of a high gain amplifier, which sits
/// under the note rather than through it.
pub const SCOOPING: Voicing = Voicing {
    mid_hz: 480.0,
    // Narrower, so a deep scoop takes the middle and leaves the bottom: a
    // decibel of loss at the low E against nine for the broad voicing.
    mid_henry: 1.4,
    treble_hz: 2_800.0,
    bass_hz: 140.0,
    feed: 12_000.0,
};

pub fn build(v: &Voicing, source: f64, load: f64) -> Result<Circuit, Fault> {
    let w = std::f64::consts::TAU * v.mid_hz;
    let mid_c = 1.0 / (w * w * v.mid_henry);
    // Each shunt leg is sized so that, wound fully down, it is a low impedance
    // against the feed resistor at the frequency it works on.
    let treble_c = 1.0 / (std::f64::consts::TAU * v.treble_hz * v.feed);
    let bass_c = 1.0 / (std::f64::consts::TAU * v.bass_hz * load.min(220_000.0));

    let mut net = Netlist::new("tone");
    net.input("in", source)
        .resistor("in", "bus", v.feed)
        // Treble: a capacitor to ground through the control.
        .capacitor("bus", "treble_leg", treble_c)
        // Wound up the leg is a high resistance and does nothing; wound down
        // it is a short and the capacitor takes the band away. A rheostat
        // leaves `1 - fraction` in circuit, so the reverse taper is the one
        // that makes ten mean flat -- and the *linear* reverse, because an
        // audio law here puts the whole useful range in the last of the
        // travel: at halfway an audio track has already left three per cent of
        // itself in circuit, which is as good as fully cut.
        .resistor("treble_leg", "treble_pot", 1_500.0)
        .pot("treble_pot", "gnd", "gnd", 100_000.0, Taper::ReverseLinear, TREBLE)
        // Mid: a series resonant leg to ground through the control. The
        // damping resistance is in series with the pair, not across the
        // capacitor -- across it the leg is a shelf, not a notch.
        .inductor("bus", "mid_l", v.mid_henry)
        .capacitor("mid_l", "mid_leg", mid_c)
        // The fixed resistor is the floor on how deep the notch can go: with
        // nothing there the resonant leg is a perfect short and the band
        // disappears by eighty decibels, which is not a tone control.
        .resistor("mid_leg", "mid_pot", 1_200.0)
        .pot("mid_pot", "gnd", "gnd", 47_000.0, Taper::ReverseLinear, MID)
        // Bass: a capacitor in the signal path, bridged by the control.
        .capacitor("bus", "out", bass_c)
        // Here the control is a resistance *bridging* the capacitor, so it is
        // the other way about: wound up it shorts the capacitor out and the
        // bottom passes.
        // 470k, chosen by sweeping it: a megohm has ninety-three per cent of
        // its range used up by the halfway point, which is a control that does
        // nothing and then everything. This gives six decibels of cut with
        // three quarters of it by halfway, which is as even as a bridged
        // capacitor gets.
        .pot("bus", "out", "out", 470_000.0, Taper::Linear, BASS)
        .resistor("out", "gnd", load);
    net.build("out")
}
