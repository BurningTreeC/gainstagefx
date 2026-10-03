//! The most expensive rig a player could actually dial in, found by measuring,
//! and what it does to the deadline.
//!
//! Every category the panel offers is swept in turn -- the circuit, the power
//! stage, the pedal, the wah, the cabinet, the speaker, microphone A and
//! microphone B -- holding the others at the choices made so far, and each
//! keeps its most expensive option: the one with the highest mean callback
//! time over the take, run serially so it is the chain's whole work. Costs
//! interact (an expensive pedal takes the oversampler to 1x, the speaker is
//! solved inside the power stage, a power stage costs more behind a hotter
//! preamp), so a second pass re-sweeps the circuit, power stage, pedal,
//! cabinet and speaker against the first pass's choices.
//!
//! Everything a player can turn up is up: oversampling requested at 8x (each
//! voice's own cap and the expensive-pedal rule then decide what runs), the
//! circuit's drive and the pedal's at full, both microphones on and blended.
//!
//! Then the rig found is played over the whole take as the plugin plays it --
//! blocks through a `StageWorker`, pipelined by the plugin's own
//! `PipelineGovernor` -- at the take's level and 6 dB hotter, back to back and
//! paced like a host. Callbacks over 930 us are the ones that drop out on the
//! owner's REAPER rig one cycle in three (`docs/realtime-multi-instance.md`).
//!
//! Wall-clock times: check the machine's load first and report it.
//!
//! `--exclude NAME` (repeatable) leaves an option out of its sweep, by the
//! name the panel gives it: how a component that dominates every comparison is
//! set aside so the rest of the catalogue gets a fair search.
//!
//! `--rig "PEDAL|CIRCUIT|POWER|CABINET|OVERSAMPLING"` skips the search and plays
//! that rig (names as the panel gives them, oversampling as 1/2/4/8), with
//! where each callback's time went, stage by stage, serially.
//!
//! ```text
//! cargo run --release --example stress
//! cargo run --release --example stress -- --rig "Round Fuzz|American 800RB|Matched|Legacy|2"
//! cargo run --release --example stress -- --seconds 6
//! cargo run --release --example stress -- --exclude "American 800RB" --exclude "American SS 800"
//! ```

use gainstagefx::params::{
    CabModel, Circuit, MicModel, Oversampling, PedalModel, PowerAmp, SpeakerModel, WahMode,
    WahModel,
};
use gainstagefx::plugin::PipelineGovernor;
use gainstagefx::presets::{Preset, PRESETS};
use gainstagefx::stage_worker::StageWorker;
use gainstagefx::voice::Chain;
use nice_plug::prelude::Enum;
use std::time::{Duration, Instant};

const RATE: f64 = 48_000.0;
const BLOCK: usize = 64;
const TAKE: &str = "tests/fixtures/01-260912_1044.wav";
const SHORT_CYCLE_US: f64 = 930.0;

/// The take, mono, at its recorded level: PCM 24 as the fixture is.
fn read_take() -> Vec<f64> {
    let bytes = std::fs::read(TAKE).expect("the fixture take");
    let u16at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32at = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let (mut channels, mut bits, mut data) = (1usize, 24u16, None);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let size = u32at(at + 4) as usize;
        match &bytes[at..at + 4] {
            b"fmt " => {
                channels = u16at(at + 10) as usize;
                bits = u16at(at + 22);
            }
            b"data" => data = Some((at + 8, size.min(bytes.len() - at - 8))),
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    assert_eq!(bits, 24, "the fixture is PCM 24");
    let (start, size) = data.expect("a data chunk");
    let frame = 3 * channels;
    (0..size / frame)
        .map(|f| {
            let b = &bytes[start + f * frame..start + f * frame + 3];
            (((b[0] as i32) | (b[1] as i32) << 8 | (b[2] as i32) << 16) << 8 >> 8) as f64
                / 8_388_608.0
        })
        .collect()
}

fn name<T: Enum>(x: T) -> &'static str {
    T::variants()[x.to_index()]
}

/// Mean and 99.9th-percentile callback time, serially, over `seconds` of the
/// take: the chain's whole work, nothing handed to another core.
fn cost(chain: &mut Chain, preset: &Preset, take: &[f64], seconds: f64) -> (f64, f64) {
    chain.apply(&preset.settings());
    chain.settle();
    chain.reset();
    chain.find_operating_point();
    let blocks = (seconds * RATE) as usize / BLOCK;
    let (mut input, mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK], vec![0.0; BLOCK]);
    let mut times = Vec::with_capacity(blocks);
    for b in 0..blocks {
        for (k, x) in input.iter_mut().enumerate() {
            *x = take[(b * BLOCK + k) % take.len()];
        }
        let started = Instant::now();
        chain.process_block(&input, &mut left, &mut right, true, None, false);
        times.push(started.elapsed().as_secs_f64() * 1e6);
    }
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    times.sort_by(f64::total_cmp);
    (mean, times[((times.len() - 1) as f64 * 0.999) as usize])
}

/// Sweeps one category, keeps its most expensive option, and prints the top
/// three with their costs so the margin is visible.
#[allow(clippy::too_many_arguments)]
fn sweep<T: Copy + Enum>(
    label: &str,
    options: &[T],
    exclude: &[String],
    rig: &mut Preset,
    set: fn(&mut Preset, T),
    chain: &mut Chain,
    take: &[f64],
    seconds: f64,
) {
    let mut costs: Vec<(T, f64, f64)> = options
        .iter()
        .filter(|&&option| !exclude.iter().any(|e| e == name(option)))
        .map(|&option| {
            let mut candidate = Preset { ..*rig };
            set(&mut candidate, option);
            let (mean, p999) = cost(chain, &candidate, take, seconds);
            (option, mean, p999)
        })
        .collect();
    costs.sort_by(|a, b| b.1.total_cmp(&a.1));
    print!("  {label:<9}");
    for (option, mean, p999) in costs.iter().take(3) {
        print!("  {:<28}", format!("{} {mean:.0}/{p999:.0}", name(*option)));
    }
    println!();
    set(rig, costs[0].0);
}

/// The rig over the whole take as the plugin plays it.
fn play(chain: &mut Chain, rig: &Preset, take: &[f64], gain_db: f64, paced: bool) {
    chain.apply(&rig.settings());
    chain.settle();
    chain.reset();
    chain.find_operating_point();
    let worker = StageWorker::new();
    let mut governor = PipelineGovernor::new();
    let scale = 10f64.powf(gain_db / 20.0);
    let blocks = take.len() / BLOCK;
    let period = BLOCK as f64 / RATE;
    let (mut input, mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK], vec![0.0; BLOCK]);
    let mut times = Vec::with_capacity(blocks);
    let before = chain.solver_breakdown();
    let mut next = Instant::now();
    for b in 0..blocks {
        if paced {
            while Instant::now() < next {
                std::hint::spin_loop();
            }
            next += Duration::from_secs_f64(period);
        }
        for (k, x) in input.iter_mut().enumerate() {
            *x = take[b * BLOCK + k] * scale;
        }
        let started = Instant::now();
        chain.process_block(
            &input,
            &mut left,
            &mut right,
            true,
            Some(&worker),
            governor.pipelining(),
        );
        let used = started.elapsed().as_secs_f64();
        governor.update((used + chain.overlapped()) / period);
        times.push(used * 1e6);
    }
    let h = chain.solver_breakdown().saturating_delta(before);
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    let at = |p: f64| sorted[((sorted.len() - 1) as f64 * p).round() as usize];
    println!(
        "  {gain_db:+.0} dB {:<8} p50 {:5.0}  p99 {:5.0}  p99.9 {:5.0}  max {:5.0} us   over 930: {:4}  over 1333: {:4}   unsettled {} fallbacks {}",
        if paced { "paced" } else { "unpaced" },
        at(0.5),
        at(0.99),
        at(0.999),
        sorted[sorted.len() - 1],
        times.iter().filter(|&&t| t > SHORT_CYCLE_US).count(),
        times.iter().filter(|&&t| t > period * 1e6).count(),
        h.pedal.unsettled + h.gain.unsettled + h.power.unsettled + h.iron.unsettled,
        h.pedal.fallbacks + h.gain.fallbacks + h.power.fallbacks + h.iron.fallbacks,
    );
}

fn by_name<T: Enum + Copy>(all: &[T], wanted: &str) -> T {
    *all.iter()
        .find(|x| name(**x) == wanted)
        .unwrap_or_else(|| panic!("no option named {wanted:?}"))
}

/// The rig serially over the whole take, with each stage's share.
fn stages(chain: &mut Chain, rig: &Preset, take: &[f64]) {
    chain.apply(&rig.settings());
    chain.settle();
    chain.reset();
    chain.find_operating_point();
    chain.set_realtime_stage_timing(true);
    let before = chain.realtime_stage_timings();
    let health = chain.solver_breakdown();
    let blocks = take.len() / BLOCK;
    let (mut input, mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK], vec![0.0; BLOCK]);
    let started = Instant::now();
    for b in 0..blocks {
        input.copy_from_slice(&take[b * BLOCK..(b + 1) * BLOCK]);
        chain.process_block(&input, &mut left, &mut right, true, None, false);
    }
    let total = started.elapsed().as_secs_f64() * 1e6 / blocks as f64;
    let t = chain.realtime_stage_timings().delta(before);
    let h = chain.solver_breakdown().saturating_delta(health);
    let per = |ns: u64| ns as f64 / 1000.0 / blocks as f64;
    println!(
        "  serial, us a callback: total {total:.0}; pedal {:.0}, gain {:.0}, power {:.0}, iron {:.0}, tone {:.0}, cabinet {:.0}",
        per(t.pedal_ns),
        per(t.gain_ns),
        per(t.power_ns),
        per(t.iron_ns),
        per(t.tone_ns),
        per(t.cabinet_ns)
    );
    for (label, s) in [("pedal", h.pedal), ("gain", h.gain), ("power", h.power)] {
        if s.solves > 0 {
            println!(
                "  {label:<6} {:.2} passes a solve, {} solves, {} fallbacks, {} unsettled",
                s.passes as f64 / s.solves as f64,
                s.solves,
                s.fallbacks,
                s.unsettled
            );
        }
    }
    chain.set_realtime_stage_timing(false);
}

fn main() {
    let mut seconds = 4.0;
    let mut exclude: Vec<String> = Vec::new();
    let mut fixed: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--exclude" => exclude.push(args.next().expect("--exclude takes a name")),
            "--rig" => fixed = Some(args.next().expect("--rig takes a rig")),
            "--seconds" => {
                seconds = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .expect("--seconds takes seconds")
            }
            other => panic!("unknown argument {other}"),
        }
    }
    let take = read_take();
    let base = PRESETS
        .iter()
        .find(|p| p.name == "Puppet Master '86")
        .expect("the starting preset");
    let mut rig = Preset {
        oversampling: Oversampling::Eight,
        drive: 1.0,
        pedal: PedalModel::None,
        pedal_drive: 1.0,
        wah: WahModel::Off,
        wah_mode: WahMode::Manual,
        wah_treadle: 0.5,
        cab_model: CabModel::Brit1960,
        speaker: SpeakerModel::Matched,
        mic_a: MicModel::Dynamic57,
        mic_b: MicModel::Off,
        mic_blend: 0.5,
        input_trim: 0.0,
        ..*base
    };
    let mut chain = Chain::new(RATE);
    if let Some(spec) = fixed {
        let parts: Vec<&str> = spec.split('|').collect();
        assert_eq!(
            parts.len(),
            5,
            "--rig \"PEDAL|CIRCUIT|POWER|CABINET|OVERSAMPLING\""
        );
        rig.pedal = by_name(&PedalModel::ALL, parts[0]);
        rig.circuit = by_name(&Circuit::ALL, parts[1]);
        rig.power_amp = by_name(&PowerAmp::ALL, parts[2]);
        rig.cab_model = by_name(&CabModel::ALL, parts[3]);
        rig.oversampling = match parts[4] {
            "1" => Oversampling::Off,
            "2" => Oversampling::Two,
            "4" => Oversampling::Four,
            _ => Oversampling::Eight,
        };
        println!("{spec}:");
        stages(&mut chain, &rig, &take);
        for (gain_db, paced) in [(0.0, false), (6.0, false)] {
            play(&mut chain, &rig, &take, gain_db, paced);
        }
        return;
    }
    println!(
        "mean / p99.9 callback us, serial, over {seconds} s of the take; the top three of each sweep:"
    );
    for pass in 1..=2 {
        println!("pass {pass}");
        sweep(
            "circuit",
            &Circuit::ALL,
            &exclude,
            &mut rig,
            |p, v| p.circuit = v,
            &mut chain,
            &take,
            seconds,
        );
        sweep(
            "power",
            &PowerAmp::ALL,
            &exclude,
            &mut rig,
            |p, v| p.power_amp = v,
            &mut chain,
            &take,
            seconds,
        );
        sweep(
            "pedal",
            &PedalModel::ALL,
            &exclude,
            &mut rig,
            |p, v| p.pedal = v,
            &mut chain,
            &take,
            seconds,
        );
        if pass == 1 {
            sweep(
                "wah",
                &WahModel::ALL,
                &exclude,
                &mut rig,
                |p, v| p.wah = v,
                &mut chain,
                &take,
                seconds,
            );
        }
        sweep(
            "cabinet",
            &CabModel::ALL,
            &exclude,
            &mut rig,
            |p, v| p.cab_model = v,
            &mut chain,
            &take,
            seconds,
        );
        sweep(
            "speaker",
            &SpeakerModel::ALL,
            &exclude,
            &mut rig,
            |p, v| p.speaker = v,
            &mut chain,
            &take,
            seconds,
        );
        if pass == 1 {
            sweep(
                "mic A",
                &MicModel::ALL,
                &exclude,
                &mut rig,
                |p, v| p.mic_a = v,
                &mut chain,
                &take,
                seconds,
            );
            sweep(
                "mic B",
                &MicModel::ALL,
                &exclude,
                &mut rig,
                |p, v| p.mic_b = v,
                &mut chain,
                &take,
                seconds,
            );
        }
    }
    chain.apply(&rig.settings());
    chain.settle();
    println!(
        "\nthe rig: {} + {} wah + {} + {} power, {} with {}, {} and {}; oversampling {}x as it runs",
        name(rig.pedal),
        name(rig.wah),
        name(rig.circuit),
        name(rig.power_amp),
        name(rig.cab_model),
        name(rig.speaker),
        name(rig.mic_a),
        name(rig.mic_b),
        chain.effective_oversampling(),
    );
    println!("played over the whole take as the plugin plays it:");
    for (gain_db, paced) in [(0.0, false), (6.0, false), (0.0, true), (6.0, true)] {
        play(&mut chain, &rig, &take, gain_db, paced);
    }
}
