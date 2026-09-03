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
    /// `Audio` is this law at a span of a thousand, which is where a volume
    /// control wants to be and why it is written out separately.
    ///
    /// A span *below* one is the same law running the other way: the track
    /// then moves quickly at first and slowly at the end. That is what a
    /// rheostat needs when the thing it is fighting is small -- a 25 k bypass
    /// pot against a 1.5 k cathode does nothing at all until its last eighth,
    /// because everything above about 3 k is equally out of circuit.
    Log { span: f64 },
    ReverseLog { span: f64 },
}

impl Taper {
    /// Fraction of the track between the wiper and the `b` end.
    pub fn fraction(self, position: f64) -> f64 {
        let p = position.clamp(0.0, 1.0);
        match self {
            Taper::Linear => p,
            Taper::ReverseLinear => 1.0 - p,
            // A single exponential is close enough to a real audio track and
            // avoids the kink a two-segment approximation puts in the middle.
            Taper::Audio => (p * 6.908).exp_m1() / 999.0,
            Taper::ReverseAudio => 1.0 - (p * 6.908).exp_m1() / 999.0,
            Taper::Log { span } => Self::log_law(p, span),
            Taper::ReverseLog { span } => 1.0 - Self::log_law(p, span),
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

/// One part.
#[derive(Clone, Debug)]
pub enum Part {
    Resistor { a: usize, b: usize, ohms: f64 },
    Capacitor { a: usize, b: usize, farads: f64 },
    Inductor { a: usize, b: usize, henry: f64 },
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
    Input { node: usize, series: f64 },
    /// A supply rail behind its series resistance, as a Norton source.
    Supply { node: usize, series: f64, volts: f64 },
    /// A junction diode.
    Diode { a: usize, k: usize, spec: DiodeSpec },
    /// A triode: plate, grid, cathode.
    Triode { p: usize, g: usize, k: usize, spec: TriodeSpec },
    /// An operational amplifier, holding its inputs together by whatever it
    /// has to put on its output -- until it runs out of rail.
    OpAmp { out: usize, plus: usize, minus: usize, rail: f64 },
    /// An ideal transformer, `ratio` primary turns to one secondary turn.
    /// Everything that colours a real one hangs off it as ordinary parts.
    Transformer { p1: usize, p2: usize, s1: usize, s2: usize, ratio: f64 },
}

/// Saturation current and emission coefficient.
#[derive(Clone, Copy, Debug)]
pub struct DiodeSpec {
    pub saturation: f64,
    pub emission: f64,
}

impl DiodeSpec {
    pub const SILICON: DiodeSpec = DiodeSpec { saturation: 2.52e-9, emission: 1.752 };
    /// Conducts at about a third of silicon's forward voltage, so a stage
    /// leaves its linear region far earlier.
    pub const GERMANIUM: DiodeSpec = DiodeSpec { saturation: 2.0e-7, emission: 1.2 };
    /// Needs roughly three times silicon's, so it stays clean where the others
    /// are already working and then arrives all at once.
    pub const LED: DiodeSpec = DiodeSpec { saturation: 1.0e-16, emission: 2.0 };
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

impl TriodeSpec {
    pub const ECC83: TriodeSpec =
        TriodeSpec { mu: 100.0, ex: 1.4, kg1: 1060.0, kp: 600.0, kvb: 300.0 };
    /// A fifth of the amplification, and the commonest swap there is.
    pub const ECC82: TriodeSpec =
        TriodeSpec { mu: 21.5, ex: 1.3, kg1: 1180.0, kp: 84.0, kvb: 300.0 };
}

impl Part {
    /// Every node this part touches, ground included.
    fn touches(&self) -> Vec<usize> {
        match *self {
            Part::Resistor { a, b, .. }
            | Part::Capacitor { a, b, .. }
            | Part::Inductor { a, b, .. } => vec![a, b],
            Part::Pot { a, wiper, b, .. } => vec![a, wiper, b],
            Part::Input { node, .. } | Part::Supply { node, .. } => vec![node],
            Part::Diode { a, k, .. } => vec![a, k],
            Part::Triode { p, g, k, .. } => vec![p, g, k],
            Part::OpAmp { out, plus, minus, .. } => vec![out, plus, minus],
            Part::Transformer { p1, p2, s1, s2, .. } => vec![p1, p2, s1, s2],
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
        !matches!(self, Part::Diode { .. } | Part::Triode { .. } | Part::OpAmp { .. })
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
}

impl Netlist {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            names: BTreeMap::new(),
            order: Vec::new(),
            parts: Vec::new(),
            controls: 0,
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
        self.parts.push(Part::Pot { a, wiper, b, ohms, taper, control });
        self
    }

    pub fn input(&mut self, node: &str, series: f64) -> &mut Self {
        let node = self.pin(node);
        self.parts.push(Part::Input { node, series });
        self
    }

    pub fn supply(&mut self, node: &str, series: f64, volts: f64) -> &mut Self {
        let node = self.pin(node);
        self.parts.push(Part::Supply { node, series, volts });
        self
    }

    pub fn diode(&mut self, a: &str, k: &str, spec: DiodeSpec) -> &mut Self {
        let (a, k) = (self.pin(a), self.pin(k));
        self.parts.push(Part::Diode { a, k, spec });
        self
    }

    pub fn triode(&mut self, p: &str, g: &str, k: &str, spec: TriodeSpec) -> &mut Self {
        let (p, g, k) = (self.pin(p), self.pin(g), self.pin(k));
        self.parts.push(Part::Triode { p, g, k, spec });
        self
    }

    pub fn opamp(&mut self, out: &str, plus: &str, minus: &str, rail: f64) -> &mut Self {
        let (out, plus, minus) = (self.pin(out), self.pin(plus), self.pin(minus));
        self.parts.push(Part::OpAmp { out, plus, minus, rail });
        self
    }

    pub fn transformer(
        &mut self,
        p1: &str,
        p2: &str,
        s1: &str,
        s2: &str,
        ratio: f64,
    ) -> &mut Self {
        let (p1, p2, s1, s2) = (self.pin(p1), self.pin(p2), self.pin(s1), self.pin(s2));
        self.parts.push(Part::Transformer { p1, p2, s1, s2, ratio });
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
            .ok_or_else(|| {
                Fault::Malformed(format!("'{}' has no input", self.name))
            })?;

        for part in &self.parts {
            let bad = match *part {
                Part::Resistor { ohms, .. } => ohms <= 0.0,
                Part::Capacitor { farads, .. } => farads <= 0.0,
                Part::Inductor { henry, .. } => henry <= 0.0,
                Part::Pot { ohms, .. } => ohms <= 0.0,
                Part::Input { series, .. } | Part::Supply { series, .. } => series <= 0.0,
                Part::Diode { spec, .. } => spec.saturation <= 0.0 || spec.emission <= 0.0,
                Part::Triode { spec, .. } => spec.mu <= 0.0 || spec.kg1 <= 0.0,
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

        let branches = self.parts.iter().filter(|p| p.needs_branch()).count();
        Ok(Circuit {
            name: self.name,
            nodes: self.order.len(),
            branches,
            names: self.order,
            parts: self.parts,
            output: out,
            controls: self.controls,
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
    pub names: Vec<String>,
    pub parts: Vec<Part>,
    pub output: usize,
    pub controls: usize,
}

impl Circuit {
    /// How big the matrix is: a row for every node, and one more for every
    /// part that imposes a voltage.
    pub fn unknowns(&self) -> usize {
        self.nodes + self.branches
    }

    /// Where a branch-carrying part's own unknown sits, counting the parts in
    /// the order they were added.
    pub fn branch_of(&self, part: usize) -> usize {
        let before = self.parts[..part].iter().filter(|p| p.needs_branch()).count();
        self.nodes + before
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
