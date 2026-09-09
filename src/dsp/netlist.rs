//! A circuit, described by name.
//!
//! Nodes are named and numbered here rather than by hand, because hand
//! numbering is where the first version's worst bugs came from and none of
//! them announced itself. A node nobody connected leaves an empty row in the
//! matrix; the factorisation then divides by zero and the whole circuit comes
//! out as `NaN`, silently, with nothing to say which node it was. Two builders
//! disagreeing about how many nodes a circuit had indexed past the end of the
//! matrix. Both are mechanical mistakes that a builder can simply refuse to
//! make.
//!
//! So: `netlist.node("plate")` returns the same index every time it is asked
//! for that name, `ground()` is always the reference, and [`Netlist::build`]
//! checks the result before anything downstream can trust it.

use std::collections::BTreeMap;
use std::fmt;

/// The reference node. Never appears in the matrix.
pub const GROUND: usize = usize::MAX;

/// How a potentiometer's track divides as it turns.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Taper {
    /// Resistance proportional to rotation.
    Linear,
    /// Roughly exponential, which is how a volume control has to behave to
    /// sound as though it moves evenly.
    Audio,
    /// The reverse of each, which is what a pot wired as a rheostat needs.
    ///
    /// With the wiper tied to one end the resistance left in circuit is
    /// `1 - fraction`, so a plain track runs backwards there: the control takes
    /// resistance *out* as it is turned up. A reverse taper is defined here as
    /// exactly `1 - forward(p)`, so a rheostat using it has a resistance that
    /// follows the forward law in the knob's own position.
    ///
    /// Both reverses have to mean the same thing. Defining one as `1 - f(p)`
    /// and the other as `1 - f(1 - p)` gives two controls that look like a
    /// matched pair and turn opposite ways.
    ReverseLinear,
    ReverseAudio,
    /// A geometric sweep over a stated span, and its reverse.
    ///
    /// A control whose effect goes as the *logarithm* of the track -- gain in
    /// decibels, a filter corner in octaves -- cannot be made to sweep evenly
    /// by a linear track, and not by an audio one either. A rheostat in series
    /// with a fixed resistance `Rf` covers a span `k = (Rf + R) / Rf`, and its
    /// effect in decibels goes as `log(Rf + R·f(p))`. That is even in `p` only
    /// when `f(p) = (k^p - 1) / (k - 1)`, so the law has to know the span it is
    /// covering: a track that suits a span of ten is wrong for a span of fifty.
    ///
    /// `Audio` is not this law: a volume track is two straight segments rather
    /// than a curve, and is written out separately.
    ///
    /// A span *below* one is the same law running the other way: the track
    /// then moves quickly at first and slowly at the end. That is what a
    /// rheostat needs when the thing it is fighting is small -- a 25 k bypass
    /// pot against a 1.5 k cathode does nothing at all until its last eighth,
    /// because everything above about 3 k is equally out of circuit.
    Log {
        span: f64,
    },
    ReverseLog {
        span: f64,
    },
}

/// What an audio track has left in circuit at half rotation.
///
/// This is the number an audio-taper potentiometer is specified by, and the
/// published range is ten to twenty per cent depending on the maker -- Jameco
/// and ISL both describe the same figure, ISL selling "fast 10 %, medium 20 %,
/// slow 30 %" as the choice on offer. Ten is the classic.
const AUDIO_AT_HALF: f64 = 0.10;

impl Taper {
    /// Fraction of the track between the wiper and the `b` end.
    pub fn fraction(self, position: f64) -> f64 {
        let p = position.clamp(0.0, 1.0);
        match self {
            Taper::Linear => p,
            Taper::ReverseLinear => 1.0 - p,
            Taper::Audio => Self::audio_law(p),
            Taper::ReverseAudio => 1.0 - Self::audio_law(p),
            Taper::Log { span } => Self::log_law(p, span),
            Taper::ReverseLog { span } => 1.0 - Self::log_law(p, span),
        }
    }

    /// A real audio track, which is two straight segments and not a curve.
    ///
    /// This was an exponential over a span of a thousand: three per cent of
    /// the track left at half rotation, where the specification is ten to
    /// twenty, and under one per cent at a quarter turn. It cost roughly
    /// sixteen decibels at the middle of every audio control in the catalogue,
    /// and the Mark IIC+ has two of them in series -- Volume 1 and the Lead
    /// Drive -- so its lead channel was short of about thirty decibels for no
    /// reason found on any drawing.
    ///
    /// Fixing the span alone was not enough, because the shape was wrong too.
    /// A log pot is not manufactured as a curve: it is two or three carbon
    /// segments of different resistivity laid end to end, so the track really
    /// is piecewise linear with a corner at the middle, and the earlier note
    /// here treated that corner as an artefact to be smoothed away rather than
    /// as the thing being modelled. An exponential through the same midpoint
    /// still sits nearly three times too low at an eighth of a turn, which is
    /// where a high-gain amplifier's control does all of its work.
    ///
    /// So: a straight line from nothing to `AUDIO_AT_HALF` over the first half
    /// of the rotation, and another from there to the whole track over the
    /// second.
    fn audio_law(p: f64) -> f64 {
        if p <= 0.5 {
            2.0 * p * AUDIO_AT_HALF
        } else {
            AUDIO_AT_HALF + 2.0 * (p - 0.5) * (1.0 - AUDIO_AT_HALF)
        }
    }

    /// `(k^p - 1) / (k - 1)`, which is the track that makes a logarithmic
    /// effect sweep evenly over a span of `k`. A span at or below one has no
    /// range to shape, so it falls back to a plain track rather than dividing
    /// by zero.
    fn log_law(p: f64, span: f64) -> f64 {
        // Only a span of one -- or a nonsense one -- has no shape to give.
        if span.is_nan() || span <= 0.0 || (span - 1.0).abs() < 1e-6 {
            return p;
        }
        (span.powf(p) - 1.0) / (span - 1.0)
    }
}

/// Saturation drain current, and the gate voltage that pinches it off.
///
/// Pinch-off is negative: an n-channel JFET conducts with no bias at all and
/// is turned *off* by pulling the gate down. That is why these stages are
/// self-biased by a source resistor rather than by a divider from the rail.
#[derive(Clone, Copy, Debug)]
pub struct JfetSpec {
    pub idss: f64,
    pub pinch_off: f64,
}

impl JfetSpec {
    /// The small-signal part in a great deal of discrete studio equipment.
    pub const J2N5457: JfetSpec = JfetSpec {
        idss: 3.0e-3,
        pinch_off: -1.5,
    };
    /// Higher current and a deeper pinch-off, so it swings further before it
    /// runs out of room.
    pub const J201: JfetSpec = JfetSpec {
        idss: 0.6e-3,
        pinch_off: -0.8,
    };
    /// A large-signal part, for a stage that has to drive something.
    pub const J113: JfetSpec = JfetSpec {
        idss: 20.0e-3,
        pinch_off: -3.0,
    };
}

#[derive(Clone, Copy, Debug)]
pub struct CoreSpec {
    /// Inductance below the knee, in henries. Large, so the branch is nearly
    /// invisible until the flux gets up.
    pub henry: f64,
    /// Flux linkage at the knee.
    pub knee: f64,
    /// How sharply the core gives up past it. Silicon steel has a hard corner;
    /// nickel is softer and starts bending sooner.
    pub sharpness: f64,
}

impl CoreSpec {
    /// Silicon steel: takes a lot before it does anything, then does a lot.
    pub const STEEL: CoreSpec = CoreSpec {
        henry: 40.0,
        knee: 0.012,
        sharpness: 7.0,
    };
    /// Nickel: bends earlier and more gently, which is the sound people buy
    /// input transformers for.
    pub const NICKEL: CoreSpec = CoreSpec {
        henry: 90.0,
        knee: 0.006,
        sharpness: 3.0,
    };
    /// A modern amorphous core: very little of its own until pushed hard.
    pub const AMORPHOUS: CoreSpec = CoreSpec {
        henry: 150.0,
        knee: 0.030,
        sharpness: 9.0,
    };
}

/// A bipolar transistor, by the Ebers-Moll transport model.
///
/// Saturation current, forward and reverse current gain, and the Early
/// voltage that gives the collector its finite output resistance.
#[derive(Clone, Copy, Debug)]
pub struct BipolarSpec {
    pub saturation: f64,
    pub forward_beta: f64,
    pub reverse_beta: f64,
    pub early: f64,
}

impl BipolarSpec {
    /// The small-signal NPN in a great deal of Japanese pedal circuitry --
    /// the 2SC3378 and its relatives. High beta, low noise.
    pub const NPN_2SC3378: BipolarSpec = BipolarSpec {
        saturation: 1.0e-14,
        forward_beta: 320.0,
        reverse_beta: 4.0,
        early: 100.0,
    };
    /// A general purpose small-signal NPN, for where a circuit only needs
    /// "a transistor".
    pub const NPN: BipolarSpec = BipolarSpec {
        saturation: 1.0e-14,
        forward_beta: 200.0,
        reverse_beta: 3.0,
        early: 80.0,
    };
}

/// One part.
#[derive(Clone, Debug)]
pub enum Part {
    Resistor {
        a: usize,
        b: usize,
        ohms: f64,
    },
    Capacitor {
        a: usize,
        b: usize,
        farads: f64,
    },
    Inductor {
        a: usize,
        b: usize,
        henry: f64,
    },
    /// A potentiometer as the two halves of its track. Tie `wiper` to `b` to
    /// use it as a rheostat.
    Pot {
        a: usize,
        wiper: usize,
        b: usize,
        ohms: f64,
        taper: Taper,
        control: usize,
    },
    /// Where the signal arrives, through the impedance driving it.
    Input {
        node: usize,
        series: f64,
    },
    /// A supply rail behind its series resistance, as a Norton source.
    Supply {
        node: usize,
        series: f64,
        volts: f64,
    },
    /// A junction diode.
    Diode {
        a: usize,
        k: usize,
        spec: DiodeSpec,
    },
    /// A triode: plate, grid, cathode.
    Triode {
        p: usize,
        g: usize,
        k: usize,
        spec: TriodeSpec,
    },
    /// A beam tetrode or pentode. `s` is the screen, which is a node like any
    /// other so that the screen resistor's drop -- and the sag behind it --
    /// falls out of the solve rather than being assumed.
    ///
    /// `count` is how many tubes this part stands for. A hundred and twenty
    /// watt amplifier has two in each side of its push-pull pair, wired in
    /// parallel and sharing a screen resistor, and two tubes in parallel are
    /// one tube passing twice the current. Modelling them as one part halves
    /// the nonlinear work for a difference no measurement here can see: real
    /// tubes are not matched, but the mismatch is a manufacturing tolerance
    /// rather than a circuit property, and inventing one would be inventing a
    /// sound.
    Pentode {
        p: usize,
        g: usize,
        k: usize,
        s: usize,
        count: f64,
        spec: PentodeSpec,
    },
    /// An operational amplifier, holding its inputs together by whatever it
    /// has to put on its output -- until it runs out of rail.
    /// An op-amp. `reference` is what its rails are measured from -- ground
    /// for a split supply, and the bias point for a pedal running off one
    /// battery, where the whole circuit sits at half the supply.
    OpAmp {
        out: usize,
        plus: usize,
        minus: usize,
        reference: usize,
        rail: f64,
    },
    /// An ideal transformer, `ratio` primary turns to one secondary turn.
    /// Everything that colours a real one hangs off it as ordinary parts.
    Transformer {
        p1: usize,
        p2: usize,
        s1: usize,
        s2: usize,
        ratio: f64,
    },
    /// A junction FET: drain, gate, source.
    Jfet {
        d: usize,
        g: usize,
        s: usize,
        spec: JfetSpec,
    },
    /// A transformer's magnetising branch, which is the part that saturates.
    /// Put across a winding, alongside the `Transformer` that sets the ratio.
    Core {
        a: usize,
        b: usize,
        spec: CoreSpec,
    },
    /// A bipolar transistor: collector, base, emitter.
    Bipolar {
        c: usize,
        b: usize,
        e: usize,
        spec: BipolarSpec,
    },
}

/// Saturation current and emission coefficient.
#[derive(Clone, Copy, Debug)]
pub struct DiodeSpec {
    pub saturation: f64,
    pub emission: f64,
}

impl DiodeSpec {
    pub const SILICON: DiodeSpec = DiodeSpec {
        saturation: 2.52e-9,
        emission: 1.752,
    };
    /// Conducts at about a third of silicon's forward voltage, so a stage
    /// leaves its linear region far earlier.
    pub const GERMANIUM: DiodeSpec = DiodeSpec {
        saturation: 2.0e-7,
        emission: 1.2,
    };
    /// Needs roughly three times silicon's, so it stays clean where the others
    /// are already working and then arrives all at once.
    pub const LED: DiodeSpec = DiodeSpec {
        saturation: 1.0e-16,
        emission: 2.0,
    };
}

/// Koren's triode parameters: amplification factor, exponent, and the three
/// shaping constants.
#[derive(Clone, Copy, Debug)]
pub struct TriodeSpec {
    pub mu: f64,
    pub ex: f64,
    pub kg1: f64,
    pub kp: f64,
    pub kvb: f64,
}

/// A beam tetrode or pentode, by Koren's equations.
///
/// The extra terms over a triode are what a screen grid buys: `mu` becomes the
/// *screen* amplification factor, the plate's own influence on the cathode
/// current nearly disappears, and the plate curves flatten into the long
/// horizontal shelf that makes a power tube a current source rather than a
/// resistor. That shelf is why a push-pull pair clips the way it does -- hard
/// at the top, and asymmetrically once the grids start drawing.
#[derive(Clone, Copy, Debug)]
pub struct PentodeSpec {
    /// Screen amplification factor.
    pub mu: f64,
    pub ex: f64,
    pub kg1: f64,
    pub kp: f64,
    pub kvb: f64,
    /// The screen's own current constant. Koren gives the screen a formula of
    /// its own rather than a share of the cathode current: it follows the same
    /// `E1^ex` the plate does, but without the plate's knee, because the
    /// screen collects what it collects whatever the plate is doing.
    ///
    /// It is calibrated here against the data sheet rather than copied, and
    /// the difference matters more than it looks. The figure usually published
    /// alongside the plate constants gives a screen current around three times
    /// what a 6L6GC actually draws, and a screen resistor plus a supply
    /// impedance turn that into volts: the screen node collapses under drive,
    /// the tubes are starved exactly when they are asked for current, and the
    /// amplifier makes a few watts instead of a hundred. Nothing about that
    /// looks like a wrong constant -- it looks like a power stage that runs
    /// out early, which is what a power stage is supposed to do.
    pub kg2: f64,
}

impl PentodeSpec {
    /// The 6L6GC, which is what nearly every American power amplifier in this
    /// catalogue runs. Koren's published constants for the type.
    /// The 6L6GC, fitted against the data sheet at the two points a push-pull
    /// output stage actually lives between.
    ///
    /// Not the constants usually published with Koren's model, and the
    /// difference is worth recording because it was invisible until the whole
    /// power stage was built. Those constants land within a couple of
    /// milliamps at the idle point -- 450 V plate, 400 V screen, −37 V grid,
    /// 26 mA -- and then give a transfer curve about half as steep as the real
    /// tube's: 186 mA at zero bias where the sheet shows something over three
    /// hundred. An amplifier built on them biases perfectly, amplifies, clips,
    /// and makes a sixth of its rated power, because at full swing the tubes
    /// simply run out of current. Nothing looks wrong; the power stage just
    /// gives up early, which is what a power stage is supposed to do.
    ///
    /// So `mu` and `kg1` are fitted to hold *both* ends: 26 mA at the idle
    /// point and something near three hundred at zero bias with the plate
    /// pulled down to fifty volts, which is where a class AB stage spends its
    /// loudest moment. `kg2` then follows from the sheet's 2.5 mA of screen
    /// current at idle.
    ///
    /// `mu` here is the screen amplification factor, and eleven is inside the
    /// range published for the type. This is a measured fit, not a
    /// transcription, and it is the honest way round: a constant that
    /// reproduces one operating point and misses the other is not a model of
    /// the tube, it is a model of that operating point.
    pub const T6L6GC: PentodeSpec = PentodeSpec {
        mu: 11.0,
        ex: 1.35,
        kg1: 588.0,
        kp: 48.0,
        kvb: 12.0,
        kg2: 3_960.0,
    };
    /// The EL34, for the British amplifiers on the roadmap. Higher screen
    /// amplification and a softer knee than a 6L6.
    pub const EL34: PentodeSpec = PentodeSpec {
        mu: 11.0,
        ex: 1.35,
        kg1: 650.0,
        kp: 60.0,
        kvb: 24.0,
        kg2: 9_000.0,
    };
    /// The EL84, for an AC30.
    pub const EL84: PentodeSpec = PentodeSpec {
        mu: 19.4,
        ex: 1.35,
        kg1: 650.0,
        kp: 42.0,
        kvb: 24.0,
        kg2: 9_500.0,
    };
}

impl TriodeSpec {
    pub const ECC83: TriodeSpec = TriodeSpec {
        mu: 100.0,
        ex: 1.4,
        kg1: 1060.0,
        kp: 600.0,
        kvb: 300.0,
    };
    /// A fifth of the amplification, and the commonest swap there is.
    pub const ECC82: TriodeSpec = TriodeSpec {
        mu: 21.5,
        ex: 1.3,
        kg1: 1180.0,
        kp: 84.0,
        kvb: 300.0,
    };
    /// 12AT7. Between the other two in amplification and well below the ECC83
    /// in plate resistance, which is why it is what drives things: a reverb
    /// transformer's primary in a blackface Fender, and a long-tailed pair
    /// where the pair has to swing two power tubes' grids.
    ///
    /// From the same published Koren set as the two above -- the ECC83 and
    /// ECC82 figures here are Koren's exactly, so this one is taken from the
    /// same family rather than mixed in from elsewhere.
    pub const ECC81: TriodeSpec = TriodeSpec {
        mu: 60.0,
        ex: 1.35,
        kg1: 460.0,
        kp: 300.0,
        kvb: 300.0,
    };
}

impl Part {
    /// Applies a node renumbering. See `cuthill_mckee`.
    fn renumber(&mut self, to: &[usize]) {
        let map = |n: &mut usize| {
            if *n != GROUND {
                *n = to[*n];
            }
        };
        match self {
            Part::Resistor { a, b, .. }
            | Part::Capacitor { a, b, .. }
            | Part::Inductor { a, b, .. }
            | Part::Core { a, b, .. } => {
                map(a);
                map(b);
            }
            Part::Pot { a, wiper, b, .. } => {
                map(a);
                map(wiper);
                map(b);
            }
            Part::Input { node, .. } | Part::Supply { node, .. } => map(node),
            Part::Diode { a, k, .. } => {
                map(a);
                map(k);
            }
            Part::Triode { p, g, k, .. } => {
                map(p);
                map(g);
                map(k);
            }
            Part::Pentode { p, g, k, s, .. } => {
                map(p);
                map(g);
                map(k);
                map(s);
            }
            Part::OpAmp {
                out,
                plus,
                minus,
                reference,
                ..
            } => {
                map(out);
                map(plus);
                map(minus);
                map(reference);
            }
            Part::Transformer { p1, p2, s1, s2, .. } => {
                map(p1);
                map(p2);
                map(s1);
                map(s2);
            }
            Part::Jfet { d, g, s, .. } => {
                map(d);
                map(g);
                map(s);
            }
            Part::Bipolar { c, b, e, .. } => {
                map(c);
                map(b);
                map(e);
            }
        }
    }

    /// Every node this part touches, ground included.
    pub fn touches(&self) -> Vec<usize> {
        match *self {
            Part::Resistor { a, b, .. }
            | Part::Capacitor { a, b, .. }
            | Part::Inductor { a, b, .. } => vec![a, b],
            Part::Pot { a, wiper, b, .. } => vec![a, wiper, b],
            Part::Pentode { p, g, k, s, .. } => vec![p, g, k, s],
            Part::Input { node, .. } | Part::Supply { node, .. } => vec![node],
            Part::Diode { a, k, .. } => vec![a, k],
            Part::Triode { p, g, k, .. } => vec![p, g, k],
            Part::OpAmp {
                out,
                plus,
                minus,
                reference,
                ..
            } => vec![out, plus, minus, reference],
            Part::Transformer { p1, p2, s1, s2, .. } => vec![p1, p2, s1, s2],
            Part::Jfet { d, g, s, .. } => vec![d, g, s],
            Part::Core { a, b, .. } => vec![a, b],
            Part::Bipolar { c, b, e, .. } => vec![c, b, e],
        }
    }

    /// Whether this part imposes a voltage rather than a conductance, and so
    /// needs an unknown of its own in the matrix.
    ///
    /// A conductance goes on the diagonal and keeps the matrix well behaved.
    /// A part that says "these two nodes shall differ by this much" cannot be
    /// written that way at all: it needs its own current as a variable, and
    /// the row it adds has a zero where the diagonal would be. That is why the
    /// factorisation has to pivot.
    pub fn needs_branch(&self) -> bool {
        matches!(self, Part::OpAmp { .. } | Part::Transformer { .. })
    }

    /// Whether this part has a single frequency response. A device does not,
    /// so a circuit containing one cannot be handed to the AC solver.
    pub fn is_linear(&self) -> bool {
        !matches!(
            self,
            Part::Diode { .. }
                | Part::Triode { .. }
                | Part::Pentode { .. }
                | Part::OpAmp { .. }
                | Part::Jfet { .. }
                | Part::Core { .. }
                | Part::Bipolar { .. }
        )
    }

    fn name(&self) -> &'static str {
        match self {
            Part::Resistor { .. } => "resistor",
            Part::Capacitor { .. } => "capacitor",
            Part::Inductor { .. } => "inductor",
            Part::Pot { .. } => "pot",
            Part::Input { .. } => "input",
            Part::Supply { .. } => "supply",
            Part::Diode { .. } => "diode",
            Part::Triode { .. } => "triode",
            Part::Pentode { .. } => "pentode",
            Part::Jfet { .. } => "JFET",
            Part::Core { .. } => "core",
            Part::Bipolar { .. } => "transistor",
            Part::OpAmp { .. } => "op-amp",
            Part::Transformer { .. } => "transformer",
        }
    }
}

/// What a netlist can be wrong about, said in terms of the names you used.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    /// A node nothing is connected to, or connected only once, which leaves a
    /// row of the matrix empty or unsolvable.
    Dangling { node: String, connections: usize },
    /// The output is not reachable from the input through any part, so
    /// whatever the circuit does cannot arrive anywhere.
    Unreachable { from: String, to: String },
    /// No input, or no output, or a part with a value that cannot be stamped.
    Malformed(String),
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::Dangling { node, connections } => write!(
                f,
                "node '{node}' has {connections} connection(s); it needs at least two \
                 or its row of the matrix is empty and the solve returns NaN"
            ),
            Fault::Unreachable { from, to } => {
                write!(f, "no path from '{from}' to '{to}'")
            }
            Fault::Malformed(what) => write!(f, "{what}"),
        }
    }
}

/// A circuit under construction.
pub struct Netlist {
    name: &'static str,
    names: BTreeMap<String, usize>,
    order: Vec<String>,
    parts: Vec<Part>,
    controls: usize,
    initial_voltages: Vec<(usize, f64)>,
    resting: Vec<(usize, f64)>,
}

impl Netlist {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            names: BTreeMap::new(),
            order: Vec::new(),
            parts: Vec::new(),
            controls: 0,
            initial_voltages: Vec::new(),
            resting: Vec::new(),
        }
    }

    /// The index for a named node, allocating it the first time it is asked
    /// for. Asking twice gives the same answer, which is the whole point.
    pub fn node(&mut self, name: &str) -> usize {
        if let Some(&at) = self.names.get(name) {
            return at;
        }
        let at = self.order.len();
        self.names.insert(name.to_string(), at);
        self.order.push(name.to_string());
        at
    }

    pub fn ground(&self) -> usize {
        GROUND
    }

    pub fn resistor(&mut self, a: &str, b: &str, ohms: f64) -> &mut Self {
        let (a, b) = (self.pin(a), self.pin(b));
        self.parts.push(Part::Resistor { a, b, ohms });
        self
    }

    pub fn capacitor(&mut self, a: &str, b: &str, farads: f64) -> &mut Self {
        let (a, b) = (self.pin(a), self.pin(b));
        self.parts.push(Part::Capacitor { a, b, farads });
        self
    }

    pub fn inductor(&mut self, a: &str, b: &str, henry: f64) -> &mut Self {
        let (a, b) = (self.pin(a), self.pin(b));
        self.parts.push(Part::Inductor { a, b, henry });
        self
    }

    pub fn pot(
        &mut self,
        a: &str,
        wiper: &str,
        b: &str,
        ohms: f64,
        taper: Taper,
        control: usize,
    ) -> &mut Self {
        let (a, wiper, b) = (self.pin(a), self.pin(wiper), self.pin(b));
        self.controls = self.controls.max(control + 1);
        self.parts.push(Part::Pot {
            a,
            wiper,
            b,
            ohms,
            taper,
            control,
        });
        self
    }

    pub fn input(&mut self, node: &str, series: f64) -> &mut Self {
        let node = self.pin(node);
        self.parts.push(Part::Input { node, series });
        self
    }

    pub fn supply(&mut self, node: &str, series: f64, volts: f64) -> &mut Self {
        let node = self.pin(node);
        self.parts.push(Part::Supply {
            node,
            series,
            volts,
        });
        self
    }

    pub fn diode(&mut self, a: &str, k: &str, spec: DiodeSpec) -> &mut Self {
        let (a, k) = (self.pin(a), self.pin(k));
        self.parts.push(Part::Diode { a, k, spec });
        self
    }

    pub fn jfet(&mut self, d: &str, g: &str, s: &str, spec: JfetSpec) -> &mut Self {
        let (d, g, s_) = (self.pin(d), self.pin(g), self.pin(s));
        self.parts.push(Part::Jfet { d, g, s: s_, spec });
        self
    }

    pub fn bipolar(&mut self, c: &str, b: &str, e: &str, spec: BipolarSpec) -> &mut Self {
        let (c, b, e) = (self.pin(c), self.pin(b), self.pin(e));
        self.parts.push(Part::Bipolar { c, b, e, spec });
        self
    }

    /// The magnetising branch of a winding: what makes iron sound like iron.
    pub fn core(&mut self, a: &str, b: &str, spec: CoreSpec) -> &mut Self {
        let (a, b) = (self.pin(a), self.pin(b));
        self.parts.push(Part::Core { a, b, spec });
        self
    }

    pub fn triode(&mut self, p: &str, g: &str, k: &str, spec: TriodeSpec) -> &mut Self {
        let (p, g, k) = (self.pin(p), self.pin(g), self.pin(k));
        self.parts.push(Part::Triode { p, g, k, spec });
        self
    }

    /// One power tube, or `count` of them in parallel. See `Part::Pentode`.
    pub fn pentode(
        &mut self,
        p: &str,
        g: &str,
        k: &str,
        screen: &str,
        count: f64,
        spec: PentodeSpec,
    ) -> &mut Self {
        let (p, g, k, s) = (self.pin(p), self.pin(g), self.pin(k), self.pin(screen));
        self.parts.push(Part::Pentode {
            p,
            g,
            k,
            s,
            count,
            spec,
        });
        self
    }

    /// An op-amp on a split supply, clipping symmetrically about ground.
    pub fn opamp(&mut self, out: &str, plus: &str, minus: &str, rail: f64) -> &mut Self {
        let (out, plus, minus) = (self.pin(out), self.pin(plus), self.pin(minus));
        self.parts.push(Part::OpAmp {
            out,
            plus,
            minus,
            reference: GROUND,
            rail,
        });
        self
    }

    /// An op-amp on a single supply, clipping about the bias point the whole
    /// circuit sits at.
    pub fn opamp_biased(
        &mut self,
        out: &str,
        plus: &str,
        minus: &str,
        reference: &str,
        rail: f64,
    ) -> &mut Self {
        let (out, plus, minus, reference) = (
            self.pin(out),
            self.pin(plus),
            self.pin(minus),
            self.pin(reference),
        );
        self.parts.push(Part::OpAmp {
            out,
            plus,
            minus,
            reference,
            rail,
        });
        self
    }

    pub fn transformer(&mut self, p1: &str, p2: &str, s1: &str, s2: &str, ratio: f64) -> &mut Self {
        let (p1, p2, s1, s2) = (self.pin(p1), self.pin(p2), self.pin(s1), self.pin(s2));
        self.parts.push(Part::Transformer {
            p1,
            p2,
            s1,
            s2,
            ratio,
        });
        self
    }

    /// Set an initial voltage on a node for the DC solver. This helps the
    /// Newton solver find the operating point for circuits with AC-coupled
    /// floating nodes.
    pub fn initial_voltage(&mut self, node: &str, volts: f64) -> &mut Self {
        let node = self.pin(node);
        self.initial_voltages.push((node, volts));
        self
    }

    /// Where a control sits when nothing turns it.
    ///
    /// Not every knob on a front panel reaches the plugin's, and the ones that
    /// do not still have to be *somewhere*. Left alone they sit at the middle
    /// of their travel, which is a position no player would choose and, on an
    /// audio track, is twenty decibels down. A circuit that has such a control
    /// says here where its player leaves it, and every harness, test and
    /// instance then agrees without having to remember.
    pub fn rest(&mut self, control: usize, position: f64) -> &mut Self {
        self.controls = self.controls.max(control + 1);
        self.resting.push((control, position.clamp(0.0, 1.0)));
        self
    }

    /// `"gnd"` and `"ground"` are the reference; anything else is a node.
    fn pin(&mut self, name: &str) -> usize {
        if name == "gnd" || name == "ground" {
            GROUND
        } else {
            self.node(name)
        }
    }

    /// Checks the circuit and hands back something the solvers can use.
    pub fn build(self, output: &str) -> Result<Circuit, Fault> {
        let Some(&out) = self.names.get(output) else {
            return Err(Fault::Malformed(format!(
                "'{}' has no node called '{output}' to take its output from",
                self.name
            )));
        };
        let input = self
            .parts
            .iter()
            .find_map(|p| match p {
                Part::Input { node, .. } => Some(*node),
                _ => None,
            })
            .ok_or_else(|| Fault::Malformed(format!("'{}' has no input", self.name)))?;

        for part in &self.parts {
            let bad = match *part {
                Part::Resistor { ohms, .. } => ohms <= 0.0,
                Part::Capacitor { farads, .. } => farads <= 0.0,
                Part::Inductor { henry, .. } => henry <= 0.0,
                Part::Pot { ohms, .. } => ohms <= 0.0,
                Part::Input { series, .. } | Part::Supply { series, .. } => series <= 0.0,
                Part::Diode { spec, .. } => spec.saturation <= 0.0 || spec.emission <= 0.0,
                Part::Triode { spec, .. } => spec.mu <= 0.0 || spec.kg1 <= 0.0,
                Part::Pentode { spec, count, .. } => {
                    spec.mu <= 0.0 || spec.kg1 <= 0.0 || count <= 0.0
                }
                // Pinch-off is negative for an n-channel part, and a core with
                // no knee would saturate at zero signal.
                Part::Jfet { spec, .. } => spec.idss <= 0.0 || spec.pinch_off >= 0.0,
                Part::Core { spec, .. } => {
                    spec.henry <= 0.0 || spec.knee <= 0.0 || spec.sharpness <= 1.0
                }
                Part::Bipolar { spec, .. } => {
                    spec.saturation <= 0.0
                        || spec.forward_beta <= 0.0
                        || spec.reverse_beta <= 0.0
                        || spec.early <= 0.0
                }
                Part::OpAmp { rail, .. } => rail <= 0.0,
                Part::Transformer { ratio, .. } => ratio <= 0.0,
            };
            if bad {
                return Err(Fault::Malformed(format!(
                    "a {} in '{}' has a value of zero or less",
                    part.name(),
                    self.name
                )));
            }
        }

        // Every node has to be connected at least twice: once is a dead end
        // whose row carries a single conductance to nowhere.
        let mut degree = vec![0usize; self.order.len()];
        for part in &self.parts {
            for pin in part.touches() {
                if pin != GROUND {
                    degree[pin] += 1;
                }
            }
        }
        if let Some(at) = degree.iter().position(|&d| d < 2) {
            return Err(Fault::Dangling {
                node: self.order[at].clone(),
                connections: degree[at],
            });
        }

        if !self.reaches(input, out) {
            return Err(Fault::Unreachable {
                from: self.order[input].clone(),
                to: output.to_string(),
            });
        }

        // Renumber the nodes so the matrix is nearly banded.
        //
        // The elimination already stops at each row's last nonzero, which for a
        // circuit matrix ought to be a large saving -- a part only touches the
        // nodes it is wired to, so a row holds three or four entries out of
        // thirty. It was worth seven per cent, because fill-in destroys the
        // structure almost immediately: eliminating column three can put an
        // entry in column twenty-nine, and from then on every row reaches the
        // far edge.
        //
        // How much fill-in there is depends entirely on the *order* the nodes
        // are numbered in, and until now that order was whichever order the
        // builder happened to mention them. Reverse Cuthill-McKee numbers them
        // by breadth-first distance from a corner of the graph, which for a
        // circuit -- a long chain of stages, each touching its neighbours -- is
        // very nearly the signal path, and leaves the matrix close to banded.
        //
        // This is a permutation and nothing else: the same equations in a
        // different order. What it changes numerically is which pivots the
        // elimination picks, so results move in the last bits and no further.
        let nodes = self.order.len();
        let permutation = cuthill_mckee(&self.parts, nodes);
        let branches = self.parts.iter().filter(|p| p.needs_branch()).count();

        // Every unknown gets a name, branch rows included -- they are rows in
        // the matrix like any other and a report that skipped them numbered
        // its own output wrongly.
        let mut order = vec![String::new(); nodes + branches];
        for (old, name) in self.order.into_iter().enumerate() {
            order[permutation[old]] = name;
        }
        // Where each part's own current ended up. Worked out here rather than
        // counted on demand, because the ordering has interleaved the branch
        // rows with the nodes and the old "how many branch-carrying parts came
        // before this one" no longer describes anything.
        let mut branch_row = vec![usize::MAX; self.parts.len()];
        let mut next = nodes;
        for (index, part) in self.parts.iter().enumerate() {
            if part.needs_branch() {
                let row = permutation[next];
                branch_row[index] = row;
                order[row] = format!("i{}", next - nodes);
                next += 1;
            }
        }

        let mut parts = self.parts;
        for part in &mut parts {
            part.renumber(&permutation);
        }
        let out = permutation[out];
        let initial_voltages: Vec<(usize, f64)> = self
            .initial_voltages
            .into_iter()
            .map(|(node, volts)| (permutation[node], volts))
            .collect();

        Ok(Circuit {
            name: self.name,
            nodes,
            branches,
            names: order,
            branch_row,
            parts,
            output: out,
            controls: self.controls,
            initial_voltages,
            resting: self.resting,
        })
    }

    /// Whether a signal can get from one node to another through any part.
    /// Ground is not a path: everything touches it.
    fn reaches(&self, from: usize, to: usize) -> bool {
        let mut seen = vec![false; self.order.len()];
        let mut stack = vec![from];
        seen[from] = true;
        while let Some(at) = stack.pop() {
            if at == to {
                return true;
            }
            for part in &self.parts {
                let pins = part.touches();
                if !pins.contains(&at) {
                    continue;
                }
                for pin in pins {
                    if pin != GROUND && !seen[pin] {
                        seen[pin] = true;
                        stack.push(pin);
                    }
                }
            }
        }
        false
    }
}

/// A checked circuit.
#[derive(Clone)]
pub struct Circuit {
    pub name: &'static str,
    pub nodes: usize,
    /// How many parts carry a current of their own as an unknown.
    pub branches: usize,
    /// One per unknown: a node's name, or `i(part)` for a branch row.
    pub names: Vec<String>,
    /// Where each part's own current unknown sits, or `usize::MAX` for a part
    /// that has none. See `branch_of`.
    pub branch_row: Vec<usize>,
    pub parts: Vec<Part>,
    pub output: usize,
    pub controls: usize,
    /// Initial node voltages for the DC solver. (node_index, voltage)
    pub initial_voltages: Vec<(usize, f64)>,
    /// Where the controls nobody turns are left. (control, position)
    pub resting: Vec<(usize, f64)>,
}

impl Circuit {
    /// How big the matrix is: a row for every node, and one more for every
    /// part that imposes a voltage.
    pub fn unknowns(&self) -> usize {
        self.nodes + self.branches
    }

    /// Where a branch-carrying part's own unknown sits.
    ///
    /// Looked up, not counted. Branch rows used to be appended after every
    /// node in the order the parts were added, which made this a count of the
    /// branch-carrying parts before it -- and made the matrix's band as wide
    /// as the matrix on any circuit with a transformer in it. They are now
    /// ordered along with the nodes, so where one landed is a fact to be
    /// recorded rather than derived. See `cuthill_mckee`.
    pub fn branch_of(&self, part: usize) -> usize {
        self.branch_row[part]
    }

    /// Whether every part has a frequency response, and therefore whether the
    /// AC solver can be asked about it.
    pub fn is_linear(&self) -> bool {
        self.parts.iter().all(Part::is_linear)
    }

    /// What a node is called, for anything that has to report a problem.
    pub fn node_name(&self, at: usize) -> &str {
        if at == GROUND {
            "gnd"
        } else {
            &self.names[at]
        }
    }
}

/// Reverse Cuthill-McKee: a node numbering that keeps the matrix near its
/// diagonal.
///
/// The elimination's cost is decided by how far each row reaches to the right,
/// and that is decided by fill-in, and fill-in is decided by the order the
/// nodes are numbered in. Numbering them in the order a builder happens to
/// mention them is arbitrary; numbering them by breadth-first distance from
/// the least-connected node, then reversing, is the standard way to make a
/// sparse symmetric-patterned matrix nearly banded.
///
/// For these circuits the result is very nearly the signal path, which is what
/// one would draw by hand: a chain of stages, each touching only its
/// neighbours.
fn cuthill_mckee(parts: &[Part], nodes: usize) -> Vec<usize> {
    // Every unknown, not every node.
    //
    // A part that imposes a voltage -- an inductor, a transformer, a source --
    // carries its own current as an extra row, and those rows used to be
    // appended after *every* node with no ordering at all. That is fine for a
    // circuit with none, and ruinous for one with several: a branch row
    // couples to the nodes its part touches, so wherever those nodes ended up,
    // the row reaches from there to the far end of the matrix. The elimination
    // is bounded by that reach, so the band was as wide as the matrix.
    //
    // Measured, on the output transformer's stage -- two ideal transformers and
    // three inductors, 27 unknowns: a band of 23 and 1966 elimination updates,
    // against the Big Muff's band of 3 and 62 updates for the same 27
    // unknowns. Thirty-two times the work for the same size, and all of it in
    // the numbering.
    //
    // Put in the graph, the branches are ordered next to the nodes they
    // constrain: band 23 -> 9, and 1966 updates -> 1174.
    let branches = parts.iter().filter(|p| p.needs_branch()).count();
    let total = nodes + branches;
    if total == 0 {
        return Vec::new();
    }
    // Who touches whom. Ground is not a row in the matrix, so it joins nothing.
    let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); total];
    let mut branch = nodes;
    for part in parts {
        let pins: Vec<usize> = part
            .touches()
            .into_iter()
            .filter(|&p| p != GROUND)
            .collect();
        for (i, &a) in pins.iter().enumerate() {
            for &b in &pins[i + 1..] {
                if a != b {
                    neighbours[a].push(b);
                    neighbours[b].push(a);
                }
            }
        }
        if part.needs_branch() {
            for &a in &pins {
                neighbours[branch].push(a);
                neighbours[a].push(branch);
            }
            branch += 1;
        }
    }
    for list in &mut neighbours {
        list.sort_unstable();
        list.dedup();
    }

    // Breadth first from the least connected node, taking each level's
    // neighbours in ascending degree so the front stays narrow. A circuit can
    // be several disconnected pieces -- a bias chain that only meets the signal
    // through a device, say -- so every piece gets a turn.
    let degree = |n: usize| neighbours[n].len();
    let mut visited = vec![false; total];
    let mut sequence = Vec::with_capacity(total);
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    while sequence.len() < total {
        let start = (0..total)
            .filter(|&n| !visited[n])
            .min_by_key(|&n| (degree(n), n))
            .expect("a node that has not been visited");
        visited[start] = true;
        queue.push_back(start);
        while let Some(node) = queue.pop_front() {
            sequence.push(node);
            let mut next: Vec<usize> = neighbours[node]
                .iter()
                .copied()
                .filter(|&n| !visited[n])
                .collect();
            next.sort_by_key(|&n| (degree(n), n));
            for n in next {
                visited[n] = true;
                queue.push_back(n);
            }
        }
    }

    // Reversed, which is what makes it *Reverse* Cuthill-McKee: the same
    // bandwidth, but less fill-in during the elimination.
    let mut to = vec![0usize; total];
    for (position, &node) in sequence.iter().rev().enumerate() {
        to[node] = position;
    }
    to
}
