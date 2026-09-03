//! Preamplifiers: the two ways a channel gets its gain, and the iron at
//! either end of them.
//!
//! Neither of these is a guitar amplifier, and the difference is not a matter
//! of degree. A guitar preamplifier is *supposed* to run out of room -- that
//! is where the sound is. A microphone preamplifier is built so that it does
//! not, and everything interesting about it happens in the last few decibels
//! before it does, or in the iron rather than the gain stage.
//!
//! **Console.** An input transformer stepping up into a discrete stage. The
//! transformer is not there for the sound: it is there because it gives
//! voltage gain for nothing, isolates the input and rejects what arrives on
//! both wires at once. That it colours the bottom end when pushed is a
//! consequence, and it is the consequence everyone means by "the sound of the
//! desk". Iron at both ends, so it is there twice.
//!
//! **Studio.** An op-amp doing the gain, and iron only on the way out. Far
//! more headroom, far less of its own, and what it has arrives suddenly and
//! from the output transformer rather than gradually from the gain device.
//! This is the one to reach for when the point is not to hear the preamplifier.

use super::{iron, jfet, valve};
use crate::dsp::device::OpAmp;
use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper};

/// The gain control, which is a volume between the stages -- see `preamp`.
pub const GAIN: usize = 0;
/// The gain device's own bypass, left in circuit.
pub const BYPASS: usize = 1;

/// What does the amplifying.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amplifier {
    /// A triode. Even harmonics, arriving gradually.
    Valve,
    /// A junction FET. Second harmonic and almost nothing else until it runs
    /// out of room.
    Jfet,
    /// An op-amp: enough loop gain that the stage contributes nothing of its
    /// own until it hits the rail, at which point it contributes a great deal.
    OpAmp,
}

#[derive(Clone, Copy, Debug)]
pub struct Values {
    pub amplifier: Amplifier,
    /// The transformer on the way in, if there is one. A console channel has
    /// one; a studio channel with an electronically balanced input does not.
    pub input_iron: Option<iron::Values>,
    /// The transformer on the way out.
    pub output_iron: Option<iron::Values>,
    /// How much the op-amp arrangement multiplies by, where that is what is
    /// doing the work.
    pub feedback: f64,
    pub leg: f64,
}

/// A console channel: iron in, a discrete stage, iron out.
pub const CONSOLE: Values = Values {
    amplifier: Amplifier::Jfet,
    input_iron: Some(iron::INPUT),
    output_iron: Some(iron::OUTPUT),
    feedback: 47_000.0,
    leg: 1_000.0,
};

/// The valve version of the same channel, which is what a valve microphone
/// preamplifier is: the same iron, a bottle instead of a transistor.
pub const VALVE_CHANNEL: Values = Values {
    amplifier: Amplifier::Valve,
    input_iron: Some(iron::INPUT),
    output_iron: Some(iron::OUTPUT),
    feedback: 47_000.0,
    leg: 1_000.0,
};

/// A studio channel: an op-amp doing the gain, iron only on the way out.
pub const STUDIO: Values = Values {
    amplifier: Amplifier::OpAmp,
    input_iron: None,
    output_iron: Some(iron::OUTPUT),
    feedback: 100_000.0,
    leg: 1_000.0,
};

pub fn build(v: &Values, source: f64, load: f64) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("preamplifier");
    net.input("in", source);
    let mut node = String::from("in");

    if let Some(ref values) = v.input_iron {
        node = iron::stage(&mut net, "ti", &node, values);
    }

    node = match v.amplifier {
        // The gain control goes *in front of* the discrete stage, not between
        // it and the output transformer.
        //
        // Behind it, the pot is what drives the iron, and a pot's wiper
        // impedance is highest in the middle of its travel and lowest at the
        // ends -- around 210 k at half and 22 k wide open here. A transformer
        // colours according to what feeds it, so the channel measured 18 per
        // cent distortion at half gain and 5 at full: the knob inverted the
        // character as it crossed the middle. In front, the stage itself
        // drives the iron at a constant impedance and turning down cleans up,
        // which is what the control is for and what the hardware does.
        Amplifier::Valve => {
            net.pot(&node, "vol", "gnd", 1_000_000.0, Taper::Audio, GAIN);
            valve::stage(&mut net, "v", "vol", &valve::CLASSIC, BYPASS)
        }
        Amplifier::Jfet => {
            net.pot(&node, "vol", "gnd", 1_000_000.0, Taper::Audio, GAIN);
            jfet::stage(&mut net, "j", "vol", &jfet::CLASSIC, BYPASS)
        }
        Amplifier::OpAmp => {
            // Non-inverting, with the gain control in the feedback path where
            // it sweeps evenly -- see `clipper`, where the same arrangement is
            // measured. A studio rail, not a nine volt battery: this is the
            // headroom the whole thing is for.
            net.opamp("amp", &node, "minus", OpAmp::STUDIO)
                .pot(
                    "amp",
                    "feedback",
                    "feedback",
                    v.feedback,
                    Taper::ReverseLog { span: (v.leg + v.feedback) / v.leg },
                    GAIN,
                )
                .resistor("feedback", "minus", v.leg)
                .resistor("minus", "gnd", v.leg)
                // Something for the iron to be driven from. Fed from a perfect
                // source a core can saturate all it likes and the terminal
                // voltage never moves.
                .resistor("amp", "opout", 100.0);
            String::from("opout")
        }
    };

    if let Some(ref values) = v.output_iron {
        node = iron::stage(&mut net, "to", &node, values);
    }

    net.resistor(&node, "gnd", load);
    net.build(&node)
}
