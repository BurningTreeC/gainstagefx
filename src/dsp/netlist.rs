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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
        }
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
        }
    }

    /// Whether this part has a single frequency response. A device does not,
    /// so a circuit containing one cannot be handed to the AC solver.
    pub fn is_linear(&self) -> bool {
        !matches!(self, Part::Diode { .. } | Part::Triode { .. })
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

        Ok(Circuit {
            name: self.name,
            nodes: self.order.len(),
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
    pub names: Vec<String>,
    pub parts: Vec<Part>,
    pub output: usize,
    pub controls: usize,
}

impl Circuit {
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
