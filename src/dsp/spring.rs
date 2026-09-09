//! The reverb tank, which is not a circuit and cannot be one.
//!
//! Everything else in this plugin is a netlist. A spring tank is not: TR4
//! drives a transducer, a torsional wave travels along a helical spring,
//! reflects off the far end and comes back to a pickup. The drawing knows
//! this and says so -- the `REVERB UNIT` on the AB763 is a dashed box with an
//! input and an output and nothing inside it. So the netlist stops at the
//! transformer's secondary, this goes between, and the recovery stage's grid
//! takes what comes back. See `CLAUDE.md` §42.5.
//!
//! ## What makes a spring sound like a spring
//!
//! **Dispersion.** A torsional wave in a helix does not travel at one speed:
//! the high frequencies arrive first. Strike a tank and the "boing" is a
//! descending chirp, and that chirp is the whole character. It is not a
//! decaying echo with an equaliser on it.
//!
//! The standard way to build it, and the one used here, is a cascade of
//! allpass sections in the delay loop. An allpass passes every frequency at
//! full level and delays each one differently, which is precisely a dispersion
//! relation; enough of them in series and a single impulse leaves as a chirp.
//! Cheap, too, which matters when the amplifier in front of it is already
//! spending a third of realtime on its power stage.
//!
//! **More than one spring.** A Fender long-decay tank carries three, at
//! slightly different lengths, so their reflections never line up and the tail
//! does not flutter. Equal lengths would ring.
//!
//! **A narrow band.** The transducers and the springs themselves pass roughly
//! a hundred hertz to four or five kilohertz. A tank does not reverberate the
//! top of a guitar's range and never did, which is why a blackface reverb sits
//! under the dry signal rather than on top of it.
//!
//! ## What this is not
//!
//! Not a measured impulse response, which §42.5 also permits. A convolution
//! would be more faithful to one particular tank and would cost more, would
//! need a file, and could not be driven -- a real tank's springs are shaken
//! harder by a louder signal and this model can be given that later. It is
//! marked as the engineering approximation it is.

/// A plain delay line.
struct Line {
    buf: Vec<f64>,
    pos: usize,
}

impl Line {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            pos: 0,
        }
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let out = self.buf[self.pos];
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
        out
    }

    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0.0);
        self.pos = 0;
    }
}

/// A first-order allpass: unity magnitude, frequency-dependent delay.
///
/// `y[n] = a x[n] + x[n-1] - a y[n-1]`
#[derive(Clone, Copy, Default)]
struct Allpass {
    a: f64,
    x1: f64,
    y1: f64,
}

impl Allpass {
    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let y = self.a * x + self.x1 - self.a * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

/// One spring: a delay line with a dispersive allpass chain in the loop.
struct Spring {
    line: Line,
    chain: Vec<Allpass>,
    feedback: f64,
    /// The band the springs and their transducers actually pass, as a
    /// one-pole high-pass and a one-pole low-pass in the loop.
    low: f64,
    high: f64,
    lp: f64,
    hp: f64,
    /// What goes back round the loop next sample.
    fed: f64,
}

impl Spring {
    fn new(rate: f64, delay_seconds: f64, decay_seconds: f64, sections: usize) -> Self {
        let samples = (delay_seconds * rate).round().max(1.0) as usize;
        // The loop gain that decays by sixty decibels in `decay_seconds`,
        // given how often the wave goes round.
        let trips = decay_seconds / delay_seconds;
        let feedback = 10f64.powf(-3.0 / trips);
        // The allpass coefficient sets where the chain's group delay turns
        // over, and with it the pitch the chirp sweeps through. Negative,
        // which is what makes the high frequencies lead.
        let chain = vec![
            Allpass {
                a: -0.62,
                ..Default::default()
            };
            sections
        ];
        let corner = |hz: f64| (-std::f64::consts::TAU * hz / rate).exp();
        Self {
            line: Line::new(samples),
            chain,
            feedback,
            low: corner(4_500.0),
            high: corner(120.0),
            lp: 0.0,
            hp: 0.0,
            fed: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let delayed = self.line.process(x + self.fed);
        let mut v = delayed;
        for section in &mut self.chain {
            v = section.process(v);
        }
        // The band the tank passes, in the loop so it narrows with every trip
        // the way a real one does.
        self.lp = v * (1.0 - self.low) + self.lp * self.low;
        let mut banded = self.lp;
        self.hp = banded * (1.0 - self.high) + self.hp * self.high;
        banded -= self.hp;
        self.fed = banded * self.feedback;
        banded
    }

    fn reset(&mut self) {
        self.line.reset();
        self.chain.iter_mut().for_each(|a| {
            a.x1 = 0.0;
            a.y1 = 0.0;
        });
        self.lp = 0.0;
        self.hp = 0.0;
        self.fed = 0.0;
    }
}

/// The tank as a whole: three springs, summed.
pub struct Tank {
    springs: Vec<Spring>,
    scale: f64,
}

impl Tank {
    /// An Accutronics-style long-decay tank, which is what a Twin carries.
    ///
    /// Three springs whose lengths are deliberately unequal, and a decay near
    /// three seconds -- the "C" of a 4AB3C1B is 2.75 to 4.0 seconds. The
    /// delays and the number of allpass sections are **estimates of that part
    /// rather than measurements of one**; nothing here came off a drawing,
    /// because a tank does not have one.
    pub fn accutronics(rate: f64) -> Self {
        let springs = [(0.0371f64, 2.9f64), (0.0410, 3.1), (0.0447, 2.7)]
            .iter()
            .map(|&(delay, decay)| Spring::new(rate, delay, decay, 48))
            .collect::<Vec<_>>();
        let scale = 1.0 / springs.len() as f64;
        Self { springs, scale }
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let mut sum = 0.0;
        for spring in &mut self.springs {
            sum += spring.process(x);
        }
        sum * self.scale
    }

    pub fn reset(&mut self) {
        self.springs.iter_mut().for_each(Spring::reset);
    }
}
