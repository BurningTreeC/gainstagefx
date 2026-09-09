//! Real audio through the plugin, measured the way a host would.
//!
//! Every other harness here drives the circuits with something synthetic: a
//! sine, or a decaying pluck built from three of them. Those are right for
//! measuring distortion and convergence, and wrong for the one question a
//! player asks, which is whether it holds up on a take. A real performance has
//! silences, string noise, palm mutes, chords with beating partials and a
//! dynamic range no envelope generates.
//!
//! It is also the only harness here that moves the controls. Everything else
//! measures a circuit standing still, and standing still is not what a plugin
//! does: a player clicks through the models to choose one, and turns the drive
//! while listening. **Both of those change the circuit inside the callback** --
//! selecting a voice resets it and sends it hunting for an operating point,
//! and that hunt is a bounded solve with the block's deadline still running.
//! A voice that fits comfortably can still put a hole in the audio the moment
//! it is selected, and nothing here was looking for that.
//!
//! ```text
//! cargo run --release --example render -- take.wav live [seconds] [drive]
//! cargo run --release --example render -- take.wav cost
//! cargo run --release --example render -- take.wav switch
//! cargo run --release --example render -- take.wav level [seconds]
//! cargo run --release --example render -- take.wav iron
//! cargo run --release --example render -- take.wav write peavey 0.8 out.wav
//! ```
//!
//! | mode | what it answers |
//! |---|---|
//! | `live` | what each voice costs per callback against the input level, controls still |
//! | `cost` | what each voice costs per callback on real material, drive swept |
//! | `switch` | the spike when the model, the iron or the drive changes under audio |
//! | `level` | whether every voice comes out at the same level at the same drive |
//! | `iron` | what the transformer choice costs and what it changes |
//! | `write` | render one voice to a file to listen to |
//!
//! The work budget is **not** applied: the ceiling stays at 32 passes, so what
//! is reported is what a voice costs rather than what it was allowed to spend.
//! A budget can only make these numbers look better.

use gainstagefx::plugin::{Budget, Summing};
use gainstagefx::voice::{self, Cabinet, Chain, Gain, Iron, Settings, Tone as ToneSection};
use std::io::Write;

const BLOCK: usize = 64;

// ---------------------------------------------------------------------------
// WAV, by hand
// ---------------------------------------------------------------------------
// Written out rather than pulled in, because this crate has no dependencies at
// all and one harness is not a reason to start. Handles what a DAW writes:
// PCM 16, 24 and 32 bit integer, and 32 bit float, in any channel count.

struct Wav {
    rate: f64,
    channels: Vec<Vec<f64>>,
}

fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn read_wav(path: &str) -> Result<Wav, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(format!("{path} is not a RIFF/WAVE file"));
    }
    // Walk the chunks rather than assuming `fmt ` then `data`: a DAW writes
    // several others, and REAPER in particular writes its own between them.
    let (mut format, mut bits, mut channels, mut rate) = (0u16, 0u16, 0usize, 0f64);
    let mut data: Option<(usize, usize)> = None;
    let mut at = 12usize;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32le(&bytes, at + 4) as usize;
        let body = at + 8;
        match id {
            b"fmt " if body + 16 <= bytes.len() => {
                format = u16le(&bytes, body);
                channels = u16le(&bytes, body + 2) as usize;
                rate = u32le(&bytes, body + 4) as f64;
                bits = u16le(&bytes, body + 14);
                // WAVE_FORMAT_EXTENSIBLE hides the real format in a GUID whose
                // first two bytes are the tag it stands for.
                if format == 0xFFFE && body + 26 <= bytes.len() {
                    format = u16le(&bytes, body + 24);
                }
            }
            b"data" => data = Some((body, size.min(bytes.len() - body))),
            _ => {}
        }
        // Chunks are word aligned, and the pad byte is not counted in `size`.
        at = body + size + (size & 1);
    }
    let (start, size) = data.ok_or_else(|| format!("{path} has no data chunk"))?;
    if channels == 0 || rate == 0.0 {
        return Err(format!("{path} has no format chunk"));
    }
    let width = (bits / 8) as usize;
    if width == 0 {
        return Err(format!("{path}: {bits} bits a sample"));
    }
    let frames = size / (width * channels);
    let mut out = vec![Vec::with_capacity(frames); channels];
    for f in 0..frames {
        for (c, channel) in out.iter_mut().enumerate() {
            let i = start + (f * channels + c) * width;
            let v = match (format, bits) {
                (1, 16) => i16::from_le_bytes([bytes[i], bytes[i + 1]]) as f64 / 32768.0,
                (1, 24) => {
                    // Sign extend the top byte into a 32 bit word.
                    let raw = i32::from_le_bytes([0, bytes[i], bytes[i + 1], bytes[i + 2]]) >> 8;
                    raw as f64 / 8_388_608.0
                }
                (1, 32) => u32le(&bytes, i) as i32 as f64 / 2_147_483_648.0,
                (3, 32) => {
                    f32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as f64
                }
                _ => return Err(format!("{path}: format {format}, {bits} bits")),
            };
            channel.push(v);
        }
    }
    Ok(Wav {
        rate,
        channels: out,
    })
}

/// 24 bit, which is what the file came in as and enough to hear everything
/// the plugin does. Clipped rather than wrapped: a wrap is a click.
fn write_wav(path: &str, rate: f64, channels: &[Vec<f64>]) -> std::io::Result<()> {
    let n = channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let ch = channels.len() as u16;
    let bytes = 3usize;
    let size = n * channels.len() * bytes;
    let mut out = Vec::with_capacity(size + 44);
    out.extend(b"RIFF");
    out.extend(((36 + size) as u32).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(ch.to_le_bytes());
    out.extend((rate as u32).to_le_bytes());
    out.extend((rate as u32 * ch as u32 * bytes as u32).to_le_bytes());
    out.extend((ch * bytes as u16).to_le_bytes());
    out.extend(24u16.to_le_bytes());
    out.extend(b"data");
    out.extend((size as u32).to_le_bytes());
    for f in 0..n {
        for channel in channels {
            let v = (channel[f].clamp(-1.0, 1.0) * 8_388_607.0) as i32;
            out.extend(&v.to_le_bytes()[0..3]);
        }
    }
    std::fs::File::create(path)?.write_all(&out)
}

// ---------------------------------------------------------------------------

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn db(x: f64) -> f64 {
    20.0 * x.max(1e-12).log10()
}

struct Run {
    times: Vec<f64>,
    /// The callbacks on which a control actually changed, by index.
    disturbed: Vec<usize>,
    out: Vec<f64>,
    /// Samples that finished at the ceiling because the budget lowered it.
    pinched: u64,
    health: voice::SolverHealth,
}

/// What the panel is set to on a given callback. `Plugin::process` builds one
/// of these once a block from the parameters and hands it to `Chain::apply`,
/// so anything that moves, moves here.
type Panel = dyn Fn(usize, usize) -> (Settings, bool);

/// One pass over the whole file, in blocks, timed per callback.
///
/// The settings are applied and the operating point hunted **inside** the
/// timed region, because that is where `Plugin::process` does it. Doing it
/// outside is how a harness reports that everything fits and a player still
/// hears a hole when they click a model.
///
/// `budget` runs the work budget the way `Plugin::process` does, reading the
/// clock every `stride` samples and bringing the Newton ceiling down for the
/// rest of a block that is running late. Without it this measures what a voice
/// costs; with it, what the plugin actually does.
fn run_budgeted(wav: &Wav, panel: &Panel, budget: Option<Budget>) -> Run {
    let frames = wav.channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let blocks = frames.div_ceil(BLOCK);
    let mut chain = Chain::new(wav.rate);
    // Built and settled before the first callback, as `initialize` does.
    let (first, _) = panel(0, blocks);
    chain.apply(&first);
    chain.settle();
    chain.find_operating_point();
    chain.set_pass_ceiling(32);

    let mut summing = Summing::new();
    let mut out = Vec::with_capacity(frames);
    let mut times = Vec::with_capacity(blocks);
    let mut disturbed = Vec::new();
    let ramp = 1.0 - (-1.0 / (0.05 * wav.rate)).exp();

    let mut at = 0usize;
    let mut block = 0usize;
    while at < frames {
        let end = (at + BLOCK).min(frames);
        let (settings, changed) = panel(block, blocks);
        let total = end - at;
        let deadline = total as f64 / wav.rate;
        let start = std::time::Instant::now();
        chain.apply(&settings);
        if chain.needs_operating_point() {
            chain.find_operating_point();
        }
        let mut ceiling = 32usize;
        chain.set_pass_ceiling(ceiling);
        for (done, f) in (at..end).enumerate() {
            if let Some(b) = budget {
                if b.armed && done > 0 && done % b.stride == 0 {
                    let want = b.ceiling(done, total, start.elapsed().as_secs_f64(), deadline);
                    if want != ceiling {
                        ceiling = want;
                        chain.set_pass_ceiling(ceiling);
                    }
                }
            }
            summing.start();
            for (c, channel) in wav.channels.iter().enumerate() {
                summing.add(c, channel[f]);
            }
            let input = summing.finish(ramp);
            out.push(std::hint::black_box(chain.process(input)));
        }
        times.push(start.elapsed().as_secs_f64() * 1e6);
        if changed {
            disturbed.push(block);
        }
        at = end;
        block += 1;
    }
    Run {
        times,
        disturbed,
        out,
        pinched: chain.pinched(),
        health: chain.solver_health(),
    }
}

/// The common case: no budget, so the numbers are what the voice costs.
fn run(wav: &Wav, panel: &Panel) -> Run {
    run_budgeted(wav, panel, None)
}

fn base(gain: Gain, drive: f64, iron: Iron) -> Settings {
    Settings {
        gain,
        drive,
        iron,
        tone: ToneSection::Scooping,
        cabinet: Cabinet::Stack,
        oversampling: 1,
        ..Settings::default()
    }
}

struct Spread {
    mean: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    worst: f64,
    over: f64,
}

fn spread(times: &[f64], deadline: f64) -> Spread {
    let mut sorted = times.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Spread {
        mean: times.iter().sum::<f64>() / times.len() as f64,
        p50: percentile(&sorted, 0.50),
        p95: percentile(&sorted, 0.95),
        p99: percentile(&sorted, 0.99),
        worst: sorted[sorted.len() - 1],
        over: 100.0 * times.iter().filter(|t| **t > deadline).count() as f64 / times.len() as f64,
    }
}

fn rms(x: &[f64]) -> f64 {
    (x.iter().map(|v| v * v).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

// ---------------------------------------------------------------------------
// the modes
// ---------------------------------------------------------------------------

/// What each voice costs on real material, with the drive swept underneath it
/// rather than held still. A drive control that moves invalidates the matrix
/// once a block, which is a cost the static harnesses never pay.
fn cost(wav: &Wav, deadline: f64) {
    println!("\n=== cost, drive swept 0 to 1 across the file, iron on steel ===\n");
    println!(
        "  {:<12}{:>8}{:>8}{:>8}{:>8}{:>8}{:>7}{:>8}{:>8}",
        "voice", "mean", "p50", "p95", "p99", "worst", "over", "passes", "unsett"
    );
    for gain in Gain::ALL {
        let r = run(wav, &move |block, blocks| {
            let d = block as f64 / (blocks.max(2) - 1) as f64;
            (base(gain, d, Iron::Steel), false)
        });
        let s = spread(&r.times, deadline);
        println!(
            "  {:<12}{:>7.0}u{:>7.0}u{:>7.0}u{:>7.0}u{:>7.0}u{:>6.1}%{:>8.2}{:>8}",
            gain.name(),
            s.mean,
            s.p50,
            s.p95,
            s.p99,
            s.worst,
            s.over,
            r.health.passes_per_solve(),
            r.health.unsettled,
        );
        if r.health.nonfinite > 0 {
            println!("    {} corrections were not numbers", r.health.nonfinite);
        }
    }
}

/// The one a player actually does: click through the models while the audio
/// runs, change the transformer, and turn the drive.
fn switch(wav: &Wav, deadline: f64) {
    // Every two seconds a different model, every seven tenths a different
    // transformer, and the drive turning the whole time.
    let per_voice = (2.0 * wav.rate / BLOCK as f64) as usize;
    let per_iron = (0.7 * wav.rate / BLOCK as f64) as usize;
    let irons = Iron::ALL;
    let r = run(wav, &move |block, blocks| {
        let v = (block / per_voice.max(1)) % Gain::ALL.len();
        let i = (block / per_iron.max(1)) % irons.len();
        let d = 0.15 + 0.85 * (block as f64 / (blocks.max(2) - 1) as f64);
        let changed = block > 0
            && (block.is_multiple_of(per_voice.max(1)) || block.is_multiple_of(per_iron.max(1)));
        (base(Gain::ALL[v], d, irons[i]), changed)
    });

    let s = spread(&r.times, deadline);
    println!("\n=== switching under audio ===\n");
    println!(
        "  A different model every 2 s, a different transformer every 0.7 s, and\n  \
         the drive turning throughout. {} callbacks, {} of them on a change.\n",
        r.times.len(),
        r.disturbed.len()
    );
    println!(
        "  overall     mean {:.0}u  p50 {:.0}u  p95 {:.0}u  p99 {:.0}u  worst {:.0}u  over {:.1} %",
        s.mean, s.p50, s.p95, s.p99, s.worst, s.over
    );

    // The callbacks a change landed on, against the ones it did not.
    let mut on = Vec::new();
    let mut off = Vec::new();
    let marked: std::collections::HashSet<usize> = r.disturbed.iter().copied().collect();
    for (i, t) in r.times.iter().enumerate() {
        if marked.contains(&i) {
            on.push(*t);
        } else {
            off.push(*t);
        }
    }
    if !on.is_empty() {
        let a = spread(&on, deadline);
        let b = spread(&off, deadline);
        println!(
            "  on a change mean {:.0}u  p50 {:.0}u  p95 {:.0}u  p99 {:.0}u  worst {:.0}u  over {:.1} %",
            a.mean, a.p50, a.p95, a.p99, a.worst, a.over
        );
        println!(
            "  settled     mean {:.0}u  p50 {:.0}u  p95 {:.0}u  p99 {:.0}u  worst {:.0}u  over {:.1} %",
            b.mean, b.p50, b.p95, b.p99, b.worst, b.over
        );
        println!(
            "\n  A change costs {:.1}x the mean callback and {:.0} us at its worst, \n  \
             against a deadline of {deadline:.0} us.",
            a.mean / b.mean.max(1e-9),
            a.worst
        );
    }

    // How long the expense lasts. A change that costs one callback is a
    // click; a change that costs fifty is a hole in the audio.
    let mut since = usize::MAX;
    let mut by_age: std::collections::BTreeMap<usize, (usize, usize)> = Default::default();
    for (i, t) in r.times.iter().enumerate() {
        if marked.contains(&i) {
            since = 0;
        } else if since != usize::MAX {
            since += 1;
        }
        let bucket = match since {
            usize::MAX => usize::MAX,
            0 => 0,
            1..=2 => 1,
            3..=8 => 3,
            9..=32 => 9,
            _ => 33,
        };
        let e = by_age.entry(bucket).or_default();
        e.0 += 1;
        if *t > deadline {
            e.1 += 1;
        }
    }
    println!("\n  callbacks by how long since a control moved:");
    for (age, (total, missed)) in by_age {
        let name = match age {
            0 => "the change itself",
            1 => "1-2 blocks after",
            3 => "3-8 blocks after",
            9 => "9-32 blocks after",
            33 => "33+ blocks after",
            _ => "before any change",
        };
        println!(
            "    {name:<22}{total:>7} callbacks, {:>5.1} % missed",
            100.0 * missed as f64 / total as f64
        );
    }

    // Which switch was the expensive one.
    let mut worst: Vec<(f64, usize)> = r.disturbed.iter().map(|&b| (r.times[b], b)).collect();
    worst.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    println!("\n  the five most expensive changes:");
    for (t, b) in worst.into_iter().take(5) {
        let v = (b / per_voice.max(1)) % Gain::ALL.len();
        let i = (b / per_iron.max(1)) % irons.len();
        println!(
            "    {:>7.0} us at {:>6.2} s  ->  {} on {}",
            t,
            b as f64 * BLOCK as f64 / wav.rate,
            Gain::ALL[v].name(),
            irons[i].name(),
        );
    }
    let peak = r.out.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    println!(
        "\n  output peak {:.1} dBFS, rms {:.1} dBFS, {} unsettled solves, {} not numbers",
        db(peak),
        db(rms(&r.out)),
        r.health.unsettled,
        r.health.nonfinite,
    );
}

/// The loudest `seconds` of the file, so a measurement is of playing rather
/// than of a gap between notes.
fn loudest(wav: &Wav, seconds: f64) -> Wav {
    let frames = wav.channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let want = ((seconds * wav.rate) as usize).min(frames).max(1);
    let step = (wav.rate as usize / 4).max(1);
    let mut best = (0usize, -1.0f64);
    let mut at = 0;
    while at + want <= frames {
        let e = rms(&wav.channels[0][at..at + want]);
        if e > best.1 {
            best = (at, e);
        }
        at += step;
    }
    Wav {
        rate: wav.rate,
        channels: wav
            .channels
            .iter()
            .map(|c| c[best.0..best.0 + want].to_vec())
            .collect(),
    }
}

/// Live playing: the controls still, the input level swept.
///
/// This is the harness the others were missing. `cost` turns the drive knob
/// across the whole file, which is a knob being turned for eighty seconds and
/// not what playing is; `switch` clicks through the models. Playing is a fixed
/// setting and a signal that gets louder and quieter, and the cost of a
/// nonlinear solve depends almost entirely on the second of those -- a circuit
/// is cheapest in silence and dearest at the top of a chord.
fn live(wav: &Wav, deadline: f64, seconds: f64, drive: f64) {
    // The budget reads the clock against the real block length, so a tighter
    // deadline than this file's rate implies is asked for by scaling here.
    let excerpt = loudest(wav, seconds);
    let base_rms = db(rms(&excerpt.channels[0]));
    println!(
        "\n=== live playing, drive {drive:.2}, controls still, {:.0} s of the loudest \
         stretch ===\n",
        excerpt.channels[0].len() as f64 / wav.rate
    );
    println!(
        "  The same passage at five levels. 0 dB is as recorded, rms {base_rms:.1} dBFS,\n           peak {:.1} dBFS. A guitar into a real amplifier arrives around 0 dB here.\n",
        db(excerpt.channels[0]
            .iter()
            .fold(0.0f64, |m, v| m.max(v.abs())))
    );
    println!(
        "  {:<25}{:>31}{:>32}",
        "", "-- no budget: what it costs --", "-- budget armed: what it does --"
    );
    println!(
        "  {:<12}{:>6}{:>8}{:>8}{:>6}{:>7}   {:>8}{:>8}{:>6}{:>8}",
        "voice", "in dB", "mean", "worst", "over", "passes", "mean", "worst", "over", "pinched"
    );
    for gain in Gain::ALL {
        for trim_db in [-24.0f64, -12.0, -6.0, 0.0] {
            let scale = 10f64.powf(trim_db / 20.0);
            let scaled = Wav {
                rate: excerpt.rate,
                channels: excerpt
                    .channels
                    .iter()
                    .map(|c| c.iter().map(|v| v * scale).collect())
                    .collect(),
            };
            let panel = move |_: usize, _: usize| (base(gain, drive, Iron::Steel), false);
            let bare = run(&scaled, &panel);
            let held = run_budgeted(&scaled, &panel, Some(Budget::new()));
            let a = spread(&bare.times, deadline);
            let b = spread(&held.times, deadline);
            println!(
                "  {:<12}{:>6.0}{:>7.0}u{:>7.0}u{:>6.1}%{:>7.2}   {:>7.0}u{:>7.0}u{:>6.1}%{:>8}",
                if trim_db == -24.0 { gain.name() } else { "" },
                trim_db,
                a.mean,
                a.worst,
                a.over,
                bare.health.passes_per_solve(),
                b.mean,
                b.worst,
                b.over,
                held.pinched,
            );
        }
    }
    println!("\n  'over' is the share of callbacks that missed the deadline.");
}

/// Whether every voice comes out at the same level at the same drive.
///
/// This is the make-up table's whole job: the drive control is meant to change
/// the character and not the loudness, and one model is meant to be swappable
/// for another without reaching for the fader.
fn level(wav: &Wav, seconds: f64) {
    let excerpt = loudest(wav, seconds);
    println!(
        "\n=== level match, {:.0} s of the loudest stretch ===\n",
        excerpt.channels[0].len() as f64 / wav.rate
    );
    println!(
        "  Input rms {:.1} dBFS. Output rms in dBFS, at the level as recorded and\n           again twelve decibels below it -- because a voice that compresses hard\n           holds its output where a clean one follows the input down, and that is\n           what a level table measured at one input level cannot show.\n",
        db(rms(&excerpt.channels[0]))
    );
    let drives = [0.0f64, 0.25, 0.5, 0.75, 1.0];
    print!("  {:<12}", "voice");
    for d in drives {
        print!("{:>9.2}", d);
    }
    println!();
    let mut table: Vec<(Gain, Vec<f64>)> = Vec::new();
    for gain in Gain::ALL {
        let mut row = Vec::new();
        let mut quiet = Vec::new();
        print!("  {:<12}", gain.name());
        for d in drives {
            let r = run(&excerpt, &move |_, _| (base(gain, d, Iron::Steel), false));
            let level = db(rms(&r.out));
            row.push(level);
            print!("{:>9.1}", level);
        }
        println!();
        // The same again, twelve decibels quieter in.
        let softer = Wav {
            rate: excerpt.rate,
            channels: excerpt
                .channels
                .iter()
                .map(|c| c.iter().map(|v| v * 0.251_188_6).collect())
                .collect(),
        };
        print!("  {:<12}", "  -12 dB in");
        for d in drives {
            let r = run(&softer, &move |_, _| (base(gain, d, Iron::Steel), false));
            let level = db(rms(&r.out));
            quiet.push(level);
            // How much of the twelve decibels came through. A clean voice
            // passes all of it; a voice at its ceiling passes none.
            print!("{:>9.1}", level - row[quiet.len() - 1] + 12.0);
        }
        println!("   <- dB of the 12 held back");
        table.push((gain, row));
    }
    println!("\n  spread across the catalogue, per drive position:");
    for (i, d) in drives.iter().enumerate() {
        let vals: Vec<(Gain, f64)> = table.iter().map(|(g, r)| (*g, r[i])).collect();
        let (hi_g, hi) = vals.iter().fold(
            (Gain::Clean, f64::MIN),
            |a, b| if b.1 > a.1 { *b } else { a },
        );
        let (lo_g, lo) = vals.iter().fold(
            (Gain::Clean, f64::MAX),
            |a, b| if b.1 < a.1 { *b } else { a },
        );
        println!(
            "    drive {d:.2}: {:>5.1} dB, {} at {:+.1} against {} at {:+.1}",
            hi - lo,
            hi_g.name(),
            hi,
            lo_g.name(),
            lo
        );
    }
}

/// What the transformer choice costs, and what it changes.
fn iron(wav: &Wav, deadline: f64, seconds: f64) {
    let frames = wav.channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let want = ((seconds * wav.rate) as usize).min(frames);
    let excerpt = Wav {
        rate: wav.rate,
        channels: wav.channels.iter().map(|c| c[..want].to_vec()).collect(),
    };
    println!("\n=== the transformer, at drive 0.8 ===\n");
    println!(
        "  {:<12}{:>10}{:>10}{:>10}{:>10}",
        "voice", "off", "steel", "nickel", "amorph"
    );
    for gain in [
        Gain::Peavey,
        Gain::Boogie,
        Gain::Twin,
        Gain::Neve,
        Gain::Muff,
    ] {
        print!("  {:<12}", gain.name());
        for i in Iron::ALL {
            let r = run(&excerpt, &move |_, _| (base(gain, 0.8, i), false));
            let s = spread(&r.times, deadline);
            print!("{:>9.0}u", s.mean);
        }
        println!();
        print!("  {:<12}", "  rms dBFS");
        for i in Iron::ALL {
            let r = run(&excerpt, &move |_, _| (base(gain, 0.8, i), false));
            print!("{:>10.1}", db(rms(&r.out)));
        }
        println!();
    }
}

// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: render <in.wav> [cost|switch|level|iron|write] ...");
        std::process::exit(2);
    };
    let wav = match read_wav(path) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let frames = wav.channels.iter().map(|c| c.len()).min().unwrap_or(0);
    let deadline = BLOCK as f64 / wav.rate * 1e6;
    let peak = wav
        .channels
        .iter()
        .flatten()
        .fold(0.0f64, |m, v| m.max(v.abs()));
    let mut all = Vec::new();
    for c in &wav.channels {
        all.extend_from_slice(c);
    }
    println!(
        "{path}\n  {:.0} Hz, {} channel{}, {:.1} s, peak {:.1} dBFS, rms {:.1} dBFS\n  \
         {BLOCK} samples a callback is {deadline:.0} us. No work budget: this is what \
         a voice costs, not what it was allowed.",
        wav.rate,
        wav.channels.len(),
        if wav.channels.len() == 1 { "" } else { "s" },
        frames as f64 / wav.rate,
        db(peak),
        db(rms(&all)),
    );

    match args.get(1).map(|s| s.as_str()).unwrap_or("cost") {
        "cost" => cost(&wav, deadline),
        "live" => live(
            &wav,
            deadline,
            args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8.0),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.7),
        ),
        "switch" => switch(&wav, deadline),
        "level" => level(
            &wav,
            args.get(2).and_then(|s| s.parse().ok()).unwrap_or(15.0),
        ),
        "iron" => iron(
            &wav,
            deadline,
            args.get(2).and_then(|s| s.parse().ok()).unwrap_or(15.0),
        ),
        "write" => {
            let want = args.get(2).map(|s| s.to_lowercase()).unwrap_or_default();
            let drive: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.8);
            let out = args.get(4).cloned().unwrap_or_else(|| "out.wav".into());
            let Some(gain) = Gain::ALL
                .into_iter()
                .find(|g| g.name().to_lowercase().replace(' ', "").contains(&want))
            else {
                eprintln!("no voice matches {want:?}");
                std::process::exit(1);
            };
            let r = run(&wav, &move |_, _| (base(gain, drive, Iron::Steel), false));
            let s = spread(&r.times, deadline);
            println!(
                "\n  {} at drive {drive:.2}: mean {:.0}u, worst {:.0}u, over {:.1} %, \
                 out {:.1} dBFS rms",
                gain.name(),
                s.mean,
                s.worst,
                s.over,
                db(rms(&r.out))
            );
            match write_wav(&out, wav.rate, &[r.out]) {
                Ok(()) => println!("  wrote {out}"),
                Err(e) => eprintln!("  {out}: {e}"),
            }
        }
        other => {
            eprintln!("unknown mode {other:?}");
            std::process::exit(2);
        }
    }
}
