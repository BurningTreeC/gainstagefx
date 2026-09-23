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
//! **It runs in real time, and that is the point.** A loop that processes
//! blocks back to back keeps the core boosted, the caches hot and the branch
//! predictor trained, and it measures a throughput this plugin never actually
//! has to deliver. A host hands over a block every 1,333 us and leaves the
//! core idle in between, which is long enough for the clock to drop and for
//! something else to evict the working set. So this waits out each callback's
//! slot before starting the next one, exactly as a host does. It therefore
//! takes as long to run as the audio it measures, which is the price of the
//! number being the one that matters.
//!
//! **Real playing, when there is some.** Pass a mono or stereo WAV and it is
//! used as the material instead of the synthetic attacks, scaled so its peak
//! lands on each level in the sweep -- which makes the sweep mean what the
//! report meant by it, where the input meter sits. A DI take has a crest
//! factor near 20 dB and long quiet decays between notes, and neither is
//! something a synthetic pluck reproduces: the quiet parts let the operating
//! point drift back and every attack then starts the solve cold.
//!
//! ```text
//! cargo run --release --example stutter
//! cargo run --release --example stutter -- take.wav
//! cargo run --release --example stutter -- take.wav "Deluxe Breakup" "Jazz Chorus"
//! ```

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
/// The material under test: either the synthetic attacks or a recording.
///
/// A recording is held as one mono vector normalised to unity peak, so the
/// level sweep is a single multiply and the same take is heard at every level.
enum Material {
    Synthetic,
    Take { samples: Vec<f64>, source: String },
}

impl Material {
    /// One sample at `k`, with the peak scaled to `amplitude`.
    #[inline]
    fn at(&self, k: usize, amplitude: f64) -> f64 {
        match self {
            Material::Synthetic => attack(k, amplitude),
            Material::Take { samples, .. } => samples[k % samples.len()] * amplitude,
        }
    }

    fn describe(&self) -> String {
        match self {
            Material::Synthetic => "plucked attacks, four a second".into(),
            Material::Take { samples, source } => format!(
                "{source} -- {:.1} s of playing, scaled so its peak sits at each level",
                samples.len() as f64 / RATE
            ),
        }
    }
}

/// Enough of RIFF/WAVE to read a take out of a DAW: PCM 16/24/32 and float 32,
/// any channel count, summed to mono because the chain is.
///
/// Written here rather than pulled in as a dependency. The plugin does not
/// read files and should not grow a crate that does just so a measurement can.
fn read_wav(path: &str) -> Result<Vec<f64>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(format!("{path} is not a RIFF/WAVE file"));
    }
    let u16at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32at = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);

    let (mut format, mut channels, mut rate, mut bits) = (0u16, 0u16, 0u32, 0u16);
    let mut data: Option<(usize, usize)> = None;
    // Walk the chunks; a DAW writes several before `data`.
    let mut at = 12usize;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32at(at + 4) as usize;
        let body = at + 8;
        if id == b"fmt " && body + 16 <= bytes.len() {
            format = u16at(body);
            channels = u16at(body + 2);
            rate = u32at(body + 4);
            bits = u16at(body + 14);
            // WAVE_FORMAT_EXTENSIBLE carries the real tag in its GUID's first
            // two bytes, which is where a 24-bit DAW export usually lands.
            if format == 0xFFFE && body + 26 <= bytes.len() {
                format = u16at(body + 24);
            }
        } else if id == b"data" {
            data = Some((body, size.min(bytes.len().saturating_sub(body))));
        }
        // Chunks are word aligned.
        at = body + size + (size & 1);
    }
    let (start, size) = data.ok_or_else(|| format!("{path} has no data chunk"))?;
    if channels == 0 {
        return Err(format!("{path} has no fmt chunk"));
    }
    if (rate as f64 - RATE).abs() > 1.0 {
        return Err(format!(
            "{path} is {rate} Hz; this harness measures at {RATE} Hz, so resample it first"
        ));
    }

    let step = (bits / 8) as usize;
    if step == 0 {
        return Err(format!("{path} declares {bits} bits a sample"));
    }
    let frame = step * channels as usize;
    let frames = size / frame;
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut sum = 0.0;
        for c in 0..channels as usize {
            let i = start + f * frame + c * step;
            let v = match (format, bits) {
                // Integer PCM, little-endian two's complement.
                (1, 16) => i16::from_le_bytes([bytes[i], bytes[i + 1]]) as f64 / 32_768.0,
                (1, 24) => {
                    let raw = ((bytes[i] as i32)
                        | ((bytes[i + 1] as i32) << 8)
                        | ((bytes[i + 2] as i32) << 16))
                        << 8;
                    (raw >> 8) as f64 / 8_388_608.0
                }
                (1, 32) => {
                    i32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as f64
                        / 2_147_483_648.0
                }
                (3, 32) => {
                    f32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as f64
                }
                _ => {
                    return Err(format!(
                        "{path}: format {format}, {bits} bits is not handled"
                    ))
                }
            };
            sum += v;
        }
        out.push(sum / channels as f64);
    }
    // Normalise to unity peak so the level sweep is exactly the level sweep.
    let peak = out.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if peak <= 0.0 {
        return Err(format!("{path} is silent"));
    }
    for v in out.iter_mut() {
        *v /= peak;
    }
    Ok(out)
}

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

fn measure(material: &Material, name: &str, dbfs: f64) -> Run {
    measure_at(material, name, dbfs, None)
}

/// `over` overrides the preset's own oversampling, for the comparison at the
/// end. `None` leaves the preset exactly as it ships.
fn measure_at(material: &Material, name: &str, dbfs: f64, over: Option<usize>) -> Run {
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
    // A recording is measured over its whole length; the synthetic material
    // repeats, so a fixed span is enough.
    let seconds = match material {
        Material::Synthetic => SECONDS,
        Material::Take { samples, .. } => samples.len() as f64 / RATE,
    };
    let blocks = (RATE * seconds / BLOCK as f64) as usize;
    let mut micros = Vec::with_capacity(blocks);
    let mut k = 0usize;
    // A second of warm-up, untimed, so the first solve's cold start is not
    // reported as a deadline miss. Run flat out: this is about settling the
    // circuit's state, not about timing.
    for _ in 0..RATE as usize {
        std::hint::black_box(chain.process(material.at(k, amplitude)));
        k += 1;
    }
    // And start the timed pass at the beginning of the take rather than a
    // second in, so the whole performance is measured.
    k = 0;
    let before = chain.solver_breakdown();

    // Paced, not flat out. Each callback waits for its slot, so the core gets
    // the same idle gap between blocks that it gets under a host -- which is
    // what lets its clock fall back and its caches go cold. A quarter of a
    // second of paced blocks before the first timed one, so the very first
    // measurement is not the only one taken at boost.
    let slot = std::time::Duration::from_secs_f64(BLOCK as f64 / RATE);
    let warm = (RATE / 4.0 / BLOCK as f64) as usize;
    let epoch = std::time::Instant::now();
    for b in 0..(warm + blocks) {
        // Absolute schedule rather than a sleep per block, so a callback that
        // overruns does not push every later one out with it.
        let due = epoch + slot.mul_f64(b as f64);
        let now = std::time::Instant::now();
        if due > now {
            std::thread::sleep(due - now);
        }
        let start = std::time::Instant::now();
        for _ in 0..BLOCK {
            std::hint::black_box(chain.process(material.at(k, amplitude)));
            k += 1;
        }
        let taken = start.elapsed().as_secs_f64() * 1e6;
        if b >= warm {
            micros.push(taken);
        }
    }
    Run {
        micros,
        work: chain.solver_breakdown().saturating_delta(before),
    }
}

fn main() {
    // `stutter [WAV] [preset ...]`. A first argument ending in .wav is the
    // material; anything else, and anything after it, names presets.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (material, named) = match args.split_first() {
        Some((first, rest)) if first.to_ascii_lowercase().ends_with(".wav") => {
            let samples = read_wav(first).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });
            (
                Material::Take {
                    samples,
                    source: std::path::Path::new(first)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| first.clone()),
                },
                rest.to_vec(),
            )
        }
        _ => (Material::Synthetic, args.clone()),
    };
    let presets: Vec<&str> = if named.is_empty() {
        UNDER_TEST.to_vec()
    } else {
        named.iter().map(String::as_str).collect()
    };

    println!(
        "One callback at a time, {BLOCK} samples at {} kHz.",
        RATE / 1000.0
    );
    println!("Budget {DEADLINE:.0} us. Presets exactly as they ship.");
    println!("Material: {}.\n", material.describe());

    for name in &presets {
        println!("=== {name} ===");
        println!(
            "  {:<22}{:>9}{:>9}{:>9}{:>10}{:>9}",
            "input", "median", "p99", "worst", "of budget", "missed"
        );
        for (label, dbfs) in LEVELS {
            let mut run = measure(&material, name, dbfs);
            let (median, p99, worst, missed) =
                (run.at(0.5), run.at(0.99), run.worst(), run.missed());
            println!(
                "  {label:<22}{median:>9.1}{p99:>9.1}{worst:>9.1}{:>9.0}%{missed:>9}",
                worst / DEADLINE * 100.0
            );
        }

        // And where the passes went, at the level the report names.
        let run = measure(&material, name, -12.0);
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
    for name in &presets {
        let mut one = measure_at(&material, name, -6.0, Some(1));
        let mut two = measure_at(&material, name, -6.0, Some(2));
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
