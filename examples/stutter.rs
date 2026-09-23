//! Why the American Deluxe and the Jazz 120 crackle, and at what input level.
//!
//! Reported from playing: both stutter on a strong attack, the Jazz 120 more
//! heavily, and the Deluxe once the input meter goes past its middle. That is
//! a *deadline* report, not a throughput one -- `examples/wherecpu.rs` averages
//! a whole second and would hide it -- and it is a *level-dependent* deadline
//! report, which nothing here measured before.
//!
//! So this measures one callback at a time, over a sweep of input levels, with
//! the presets set up exactly as they ship. The Twin is the control: it has
//! had the solver work and does not stutter, so whatever separates it from the
//! other two is the thing to fix.
//!
//! `cargo run --release --example stutter`

use gainstagefx::presets::PRESETS;
use gainstagefx::voice::{Chain, SolverBreakdown};

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
/// What one callback has to fit into, in microseconds.
const DEADLINE: f64 = BLOCK as f64 / RATE * 1e6;
const SECONDS: f64 = 3.0;

/// The presets under suspicion, with the Twin's two as the control.
///
/// Paired on purpose. `Blackface Clean` and `Blackface Throb` are the same
/// amplifier with the optical tremolo off and on; `Blackface Deluxe` and
/// `Deluxe Breakup` are the other amplifier the same way, and `Blackface
/// Normal` is that amplifier's other channel with neither reverb nor tremolo
/// in it at all. What separates any two of them is one thing.
const UNDER_TEST: [&str; 7] = [
    "Blackface Clean",
    "Blackface Throb",
    "Blackface Deluxe",
    "Deluxe Breakup",
    "Blackface Normal",
    "Jazz Clean",
    "Jazz Chorus",
];

/// Where the input meter sits, in dBFS at the plugin's input. The middle of the
/// meter is the level the report singles out for the Deluxe.
const LEVELS: [(&str, f64); 5] = [
    ("-24 dBFS", -24.0),
    ("-18 dBFS", -18.0),
    ("-12 dBFS (mid meter)", -12.0),
    ("-6 dBFS", -6.0),
    ("0 dBFS", 0.0),
];

/// A plucked note: a step into a nonlinear circuit, which is what makes a
/// warm-started solve start cold. Four notes a second, each with a hard edge.
fn attack(k: usize, amplitude: f64) -> f64 {
    let t = k as f64 / RATE;
    let phase = t * 4.0;
    let since = phase.fract() / 4.0;
    let envelope = (-since * 9.0).exp();
    let note = 82.4 * (1.0 + 0.5 * (phase as usize % 4) as f64);
    let body = (std::f64::consts::TAU * note * t).sin()
        + 0.7 * (std::f64::consts::TAU * note * 2.0 * t).sin()
        + 0.5 * (std::f64::consts::TAU * note * 3.0 * t).sin()
        + 0.3 * (std::f64::consts::TAU * note * 5.0 * t).sin();
    amplitude * (body / 2.5) * envelope
}

struct Run {
    micros: Vec<f64>,
    work: SolverBreakdown,
}

impl Run {
    fn at(&mut self, q: f64) -> f64 {
        self.micros
            .sort_by(|a, b| a.partial_cmp(b).expect("no NaN in a duration"));
        self.micros[((self.micros.len() - 1) as f64 * q).round() as usize]
    }
    fn worst(&self) -> f64 {
        self.micros.iter().cloned().fold(0.0, f64::max)
    }
    /// Callbacks that did not fit. This is the number the report is about.
    fn missed(&self) -> usize {
        self.micros.iter().filter(|m| **m > DEADLINE).count()
    }
}

fn measure(name: &str, dbfs: f64) -> Run {
    measure_at(name, dbfs, None)
}

/// `over` overrides the preset's own oversampling, for the comparison at the
/// end. `None` leaves the preset exactly as it ships.
fn measure_at(name: &str, dbfs: f64, over: Option<usize>) -> Run {
    let preset = PRESETS
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("no preset called {name}"));
    let mut settings = preset.settings();
    if let Some(factor) = over {
        settings.oversampling = factor;
    }
    let mut chain = Chain::new(RATE);
    chain.apply(&settings);
    chain.settle();
    chain.find_operating_point();

    let amplitude = 10f64.powf((dbfs + preset.input_trim as f64) / 20.0);
    let blocks = (RATE * SECONDS / BLOCK as f64) as usize;
    let mut micros = Vec::with_capacity(blocks);
    let mut k = 0usize;
    // A second of warm-up, untimed, so the first solve's cold start is not
    // reported as a deadline miss.
    for _ in 0..RATE as usize {
        std::hint::black_box(chain.process(attack(k, amplitude)));
        k += 1;
    }
    let before = chain.solver_breakdown();
    for _ in 0..blocks {
        let start = std::time::Instant::now();
        for _ in 0..BLOCK {
            std::hint::black_box(chain.process(attack(k, amplitude)));
            k += 1;
        }
        micros.push(start.elapsed().as_secs_f64() * 1e6);
    }
    Run {
        micros,
        work: chain.solver_breakdown().saturating_delta(before),
    }
}

fn main() {
    println!(
        "One callback at a time, {BLOCK} samples at {} kHz.",
        RATE / 1000.0
    );
    println!("Budget {DEADLINE:.0} us. Plucked attacks, four a second, at the");
    println!("level the input meter shows. Presets exactly as they ship.\n");

    for name in UNDER_TEST {
        println!("=== {name} ===");
        println!(
            "  {:<22}{:>9}{:>9}{:>9}{:>10}{:>9}",
            "input", "median", "p99", "worst", "of budget", "missed"
        );
        for (label, dbfs) in LEVELS {
            let mut run = measure(name, dbfs);
            let (median, p99, worst, missed) =
                (run.at(0.5), run.at(0.99), run.worst(), run.missed());
            println!(
                "  {label:<22}{median:>9.1}{p99:>9.1}{worst:>9.1}{:>9.0}%{missed:>9}",
                worst / DEADLINE * 100.0
            );
        }

        // And where the passes went, at the level the report names.
        let run = measure(name, -12.0);
        for (block, h) in [
            ("gain", run.work.gain),
            ("power", run.work.power),
            ("iron", run.work.iron),
            ("reverb return", run.work.reverb_return),
        ] {
            if h.solves == 0 {
                continue;
            }
            println!(
                "    {block:<16}{:>10} solves{:>8.2} passes/solve{:>8} unsettled{:>8} fallbacks",
                h.solves,
                h.passes_per_solve(),
                h.unsettled,
                h.fallbacks
            );
        }
        println!();
    }

    // And what the Oversampling control costs, which is the other half of the
    // question. Every shipped preset on a modelled circuit asks for host rate;
    // the control going up is a choice, and this is its price. It is measured
    // at -6 dBFS, a level a player actually reaches.
    println!("=== what the Oversampling control costs, at -6 dBFS ===");
    println!(
        "  {:<22}{:>10}{:>10}{:>9}{:>10}{:>9}",
        "preset", "median 1x", "median 2x", "worst 1x", "worst 2x", "missed 2x"
    );
    for name in UNDER_TEST {
        let mut one = measure_at(name, -6.0, Some(1));
        let mut two = measure_at(name, -6.0, Some(2));
        println!(
            "  {name:<22}{:>10.1}{:>10.1}{:>9.1}{:>10.1}{:>9}",
            one.at(0.5),
            two.at(0.5),
            one.worst(),
            two.worst(),
            two.missed(),
        );
    }
}
