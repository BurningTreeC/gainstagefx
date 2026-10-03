//! Would more cores, each solving a later stretch of a block ahead of time,
//! shorten the power stage's critical path, and by how much?
//!
//! `cargo test --release --lib parallel_in_time -- --ignored --nocapture`
//!
//! The rig starts from `GSFX_PRESET` (default *American 800RB Clank*) and can
//! be changed by name as in `power_hard`: `GSFX_CIRCUIT`, `GSFX_POWER`,
//! `GSFX_PEDAL`, `GSFX_DRIVE`, `GSFX_MASTER`, `GSFX_OS` (1/2/4/8) and
//! `GSFX_GAIN_DB`; `GSFX_SECONDS` of the take (8), in `GSFX_BLOCK`-sample
//! callbacks (64). `GSFX_HARD_AT` is `SHADOW_HARD` (4).
//!
//! `SpecBlock` already does this with one helper: a shadow copy of the power
//! stage, taken at a block's start, solves the block's second half without
//! solving the first, and the real solve starts its hard second-half solves
//! from the shadow's answers. Here the block is cut into `K` segments instead,
//! with a shadow for each after the first, all started at the block's start
//! from the real state there -- `K - 1` helpers -- and, as a second variant,
//! each segment's shadow solved again from the end of the previous segment's
//! first sweep ("two sweeps", the first iteration of parareal without a coarse
//! solver; `2K - 3` helpers). And as a floor, each real solve started from the
//! answer it is about to reach, which no scheme can beat.
//!
//! Counted in Newton passes, so the answer does not depend on the machine: a
//! block's critical path is the real solves' passes plus whatever each waits
//! for its shadow's answer, every shadow running on its own core from the
//! block's start (or, in the second sweep, from when its predecessor's first
//! sweep ends). Printed against today's passes a block, for the mean and for
//! the heaviest 1 % and 0.1 % of blocks, which is the tail a callback misses
//! its deadline in. A starting point only changes where Newton begins: every
//! variant converges to the same test, and its output is compared with
//! today's to say how far that moved it.

use super::Simulation;
use crate::dsp::device::Linearisation;
use crate::params::{Circuit, Oversampling, PedalModel, PowerAmp};
use crate::presets::{Preset, PRESETS};
use crate::voice::Chain;
use nice_plug::prelude::Enum;

const RATE: f64 = 48_000.0;

fn take() -> Vec<f64> {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/01-260912_1044.wav"
    ))
    .expect("the take");
    let mut at = 12;
    let mut data = None;
    while at + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        if &bytes[at..at + 4] == b"data" {
            data = Some(&bytes[at + 8..(at + 8 + len).min(bytes.len())]);
        }
        at += 8 + len + (len & 1);
    }
    data.unwrap()
        .as_chunks::<3>()
        .0
        .iter()
        .map(|b| ((b[0] as i32 | (b[1] as i32) << 8 | (b[2] as i32) << 16) << 8 >> 8) as f64)
        .map(|v| v / 8_388_608.0)
        .collect()
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn by_name<T: Enum + Copy>(all: &[T], name: &str) -> T {
    *all.iter()
        .find(|x| T::variants()[x.to_index()] == name)
        .unwrap_or_else(|| panic!("no option named {name:?}"))
}

fn rig() -> Preset {
    let name = var("GSFX_PRESET").unwrap_or_else(|| "American 800RB Clank".into());
    let base = PRESETS.iter().find(|p| p.name == name).expect("preset");
    let mut rig = Preset { ..*base };
    if let Some(c) = var("GSFX_CIRCUIT") {
        rig.circuit = by_name(&Circuit::ALL, &c);
    }
    if let Some(p) = var("GSFX_POWER") {
        rig.power_amp = by_name(&PowerAmp::ALL, &p);
    }
    if let Some(p) = var("GSFX_PEDAL") {
        rig.pedal = by_name(&PedalModel::ALL, &p);
    }
    if let Some(d) = var("GSFX_DRIVE") {
        rig.drive = d.parse().expect("GSFX_DRIVE");
    }
    if let Some(m) = var("GSFX_MASTER") {
        rig.master = m.parse().expect("GSFX_MASTER");
    }
    if let Some(o) = var("GSFX_OS") {
        rig.oversampling = match o.as_str() {
            "1" => Oversampling::Off,
            "2" => Oversampling::Two,
            "4" => Oversampling::Four,
            _ => Oversampling::Eight,
        };
    }
    rig
}

/// A shadow's sweep over one segment: its answer and passes at each solve,
/// and when each answer is ready, in passes from the sweep's start.
struct Sweep {
    voltages: Vec<f64>,
    linearisations: Vec<Linearisation>,
    passes: Vec<u64>,
    ready: Vec<u64>,
}

impl Sweep {
    fn new(len: usize, unknowns: usize, devices: usize) -> Self {
        Self {
            voltages: vec![0.0; len * unknowns],
            linearisations: vec![Linearisation::default(); len * devices],
            passes: vec![0; len],
            ready: vec![0; len],
        }
    }

    /// Solve `inputs` on `sim` from where it stands, starting `offset`
    /// passes after the block's start.
    fn run(&mut self, sim: &mut Simulation, inputs: &[f64], offset: u64) {
        let (n, d) = (sim.unknowns(), sim.device_count());
        let mut at = offset;
        for (i, &x) in inputs.iter().enumerate() {
            let before = sim.newton_passes;
            sim.process(x);
            let p = sim.newton_passes - before;
            at += p;
            self.passes[i] = p;
            self.ready[i] = at;
            self.voltages[i * n..(i + 1) * n].copy_from_slice(sim.voltages());
            sim.linearisations_into(&mut self.linearisations[i * d..(i + 1) * d]);
        }
    }
}

/// A copy of `start` that solves as the chain's own power stage does,
/// half-step rescue included; a shadow, like `SpecBlock`'s, has none.
fn real_copy(start: &Simulation) -> Simulation {
    let mut sim = start.speculative_copy();
    sim.enable_half_step_rescue();
    sim
}

/// One scheme's real power stage, its shadows, and what it has counted.
struct Scheme {
    name: String,
    segments: usize,
    two_sweeps: bool,
    real: Simulation,
    first: Vec<Simulation>,
    second: Vec<Simulation>,
    first_sweeps: Vec<Sweep>,
    second_sweeps: Vec<Sweep>,
    critical: Vec<u64>,
    real_passes: u64,
    helper_passes: u64,
    waited: u64,
    output: Vec<f64>,
}

impl Scheme {
    fn new(
        name: String,
        start: &Simulation,
        segments: usize,
        two_sweeps: bool,
        block: usize,
    ) -> Self {
        let (n, d) = (start.unknowns(), start.device_count());
        let seg = block / segments;
        let copies = |count: usize| {
            (0..count)
                .map(|_| start.speculative_copy())
                .collect::<Vec<_>>()
        };
        let sweeps = |count: usize| {
            (0..count)
                .map(|_| Sweep::new(seg, n, d))
                .collect::<Vec<_>>()
        };
        Self {
            name,
            segments,
            two_sweeps,
            real: real_copy(start),
            first: copies(segments),
            second: copies(if two_sweeps { segments } else { 0 }),
            first_sweeps: sweeps(segments),
            second_sweeps: sweeps(if two_sweeps { segments } else { 0 }),
            critical: Vec::new(),
            real_passes: 0,
            helper_passes: 0,
            waited: 0,
            output: Vec::new(),
        }
    }

    fn block(&mut self, inputs: &[f64], hard_at: u64) {
        let seg = inputs.len() / self.segments;
        let (n, d) = (self.real.unknowns(), self.real.device_count());
        // First sweep: every segment after the first from the block's start.
        for k in 1..self.segments {
            self.first[k].follow(&self.real);
            self.first_sweeps[k].run(&mut self.first[k], &inputs[k * seg..(k + 1) * seg], 0);
            self.helper_passes += self.first_sweeps[k].passes.iter().sum::<u64>();
        }
        // Second sweep: segment k from where segment k - 1's first sweep
        // ended. Segment 1's would start from the real state after segment 0,
        // which is the real solve itself, so it starts at 2.
        if self.two_sweeps {
            for k in 2..self.segments {
                self.second[k].follow(&self.first[k - 1]);
                let offset = *self.first_sweeps[k - 1].ready.last().unwrap();
                self.second_sweeps[k].run(
                    &mut self.second[k],
                    &inputs[k * seg..(k + 1) * seg],
                    offset,
                );
                self.helper_passes += self.second_sweeps[k].passes.iter().sum::<u64>();
            }
        }
        let mut t = 0u64;
        for (j, &x) in inputs.iter().enumerate() {
            let (k, i) = (j / seg, j % seg);
            if k > 0 {
                let sweep = if self.two_sweeps && k >= 2 {
                    &self.second_sweeps[k]
                } else {
                    &self.first_sweeps[k]
                };
                // Waiting for the shadow's answer, whether or not it is used:
                // whether it is depends on the shadow's own passes.
                if sweep.ready[i] > t {
                    self.waited += sweep.ready[i] - t;
                    t = sweep.ready[i];
                }
                if sweep.passes[i] >= hard_at {
                    self.real.set_start_point(
                        &sweep.voltages[i * n..(i + 1) * n],
                        &sweep.linearisations[i * d..(i + 1) * d],
                    );
                }
            }
            let before = self.real.newton_passes;
            self.output.push(self.real.process(x));
            let p = self.real.newton_passes - before;
            t += p;
            self.real_passes += p;
        }
        self.critical.push(t);
    }
}

#[test]
#[ignore = "diagnostic: what parallel-in-time speculation on more cores could buy"]
fn parallel_in_time() {
    let rig = rig();
    let gain_db: f64 = var("GSFX_GAIN_DB").map_or(0.0, |g| g.parse().expect("GSFX_GAIN_DB"));
    let seconds: f64 = var("GSFX_SECONDS").map_or(8.0, |s| s.parse().expect("GSFX_SECONDS"));
    let block: usize = var("GSFX_BLOCK").map_or(64, |b| b.parse().expect("GSFX_BLOCK"));
    let hard_at: u64 = var("GSFX_HARD_AT").map_or(4, |h| h.parse().expect("GSFX_HARD_AT"));
    let scale = 10f64.powf((rig.input_trim as f64 + gain_db) / 20.0);
    let take = take();
    let host: Vec<f64> = take[..((seconds * RATE) as usize).min(take.len())]
        .iter()
        .map(|x| x * scale)
        .collect();

    // The power stage's inputs do not depend on it: record them, a solve at a
    // time, from one serial run without speculation. The starting state is
    // taken after the first block, once the chain has installed its
    // oversampling.
    let mut chain = Chain::new(RATE);
    chain.apply(&rig.settings());
    chain.settle();
    chain.find_operating_point();
    chain.set_speculation(false);
    let (mut left, mut right) = (vec![0.0; block], vec![0.0; block]);
    chain.process_block(&host[..block], &mut left, &mut right, false, None, false);
    let factor = chain.effective_oversampling();
    let start = chain
        .test_active_power_mut()
        .expect("a power stage")
        .speculative_copy();
    let mut inputs = Vec::with_capacity(host.len() * factor);
    for chunk in host[block..].chunks_exact(block) {
        chain.process_block(chunk, &mut left, &mut right, false, None, false);
        inputs.extend_from_slice(&chain.test_mid()[..block * factor]);
    }
    let solves = block * factor;

    // Today, and the floor: each solve started from today's answer to it.
    let mut today = real_copy(&start);
    let mut floor = real_copy(&start);
    let (n, d) = (start.unknowns(), start.device_count());
    let mut lin = vec![Linearisation::default(); d];
    let (mut today_blocks, mut floor_blocks) = (Vec::new(), Vec::new());
    let mut today_output = Vec::with_capacity(inputs.len());
    let mut floor_output = Vec::with_capacity(inputs.len());
    for chunk in inputs.chunks_exact(solves) {
        let (t0, f0) = (today.newton_passes, floor.newton_passes);
        for &x in chunk {
            today_output.push(today.process(x));
            today.linearisations_into(&mut lin);
            floor.set_start_point(today.voltages(), &lin);
            floor_output.push(floor.process(x));
        }
        today_blocks.push(today.newton_passes - t0);
        floor_blocks.push(floor.newton_passes - f0);
    }
    {
        // The recording is right if replaying it lands where the chain did.
        let live = chain.test_active_power_mut().unwrap();
        assert_eq!(
            live.voltages(),
            today.voltages(),
            "replayed inputs reproduce the chain's power stage"
        );
        let _ = n;
    }

    let mut schemes = Vec::new();
    for k in [2usize, 4, 8, 16] {
        if solves.is_multiple_of(k) && solves / k >= 2 {
            schemes.push(Scheme::new(
                format!("{k} segments"),
                &start,
                k,
                false,
                solves,
            ));
            if k >= 4 {
                schemes.push(Scheme::new(
                    format!("{k} segments, two sweeps"),
                    &start,
                    k,
                    true,
                    solves,
                ));
            }
        }
    }
    for scheme in &mut schemes {
        for chunk in inputs.chunks_exact(solves) {
            scheme.block(chunk, hard_at);
        }
    }

    // The heaviest blocks today, and what each scheme makes of the same ones.
    let mut order: Vec<usize> = (0..today_blocks.len()).collect();
    order.sort_by(|&a, &b| today_blocks[b].cmp(&today_blocks[a]));
    let top = |fraction: f64| &order[..((order.len() as f64 * fraction).ceil() as usize).max(1)];
    let mean_over = |values: &[u64], which: &[usize]| {
        which.iter().map(|&i| values[i]).sum::<u64>() as f64 / which.len() as f64
    };
    let all: Vec<usize> = (0..today_blocks.len()).collect();
    let null = |out: &[f64]| {
        let (mut err, mut energy) = (0.0, 0.0);
        for (a, b) in today_output.iter().zip(out) {
            err += (a - b) * (a - b);
            energy += a * a;
        }
        10.0 * (err / energy.max(1e-300)).max(1e-300).log10()
    };
    println!(
        "{} + {} + {}, {}x, {:+} dB: {} blocks of {} power solves, {:.2} passes a solve today",
        PedalModel::variants()[rig.pedal.to_index()],
        rig.circuit.name(),
        rig.power_amp.name(),
        factor,
        gain_db,
        today_blocks.len(),
        solves,
        today_blocks.iter().sum::<u64>() as f64 / inputs.len() as f64
    );
    println!(
        "{:<28} {:>8} {:>8} {:>8} {:>8} {:>9} {:>9} {:>8}",
        "critical path a block", "mean", "top 1%", "top .1%", "max", "helpers", "waited", "null dB"
    );
    let base = (
        mean_over(&today_blocks, &all),
        mean_over(&today_blocks, top(0.01)),
        mean_over(&today_blocks, top(0.001)),
        *today_blocks.iter().max().unwrap() as f64,
    );
    println!(
        "{:<28} {:>8.1} {:>8.1} {:>8.1} {:>8.0}",
        "today", base.0, base.1, base.2, base.3
    );
    let row = |name: &str, blocks: &[u64], helpers: f64, waited: f64, out: &[f64]| {
        let r = (
            mean_over(blocks, &all) / base.0,
            mean_over(blocks, top(0.01)) / base.1,
            mean_over(blocks, top(0.001)) / base.2,
            *blocks.iter().max().unwrap() as f64 / base.3,
        );
        println!(
            "{:<28} {:>7.0}% {:>7.0}% {:>7.0}% {:>7.0}% {:>9.2} {:>9.2} {:>8.1}",
            name,
            100.0 * r.0,
            100.0 * r.1,
            100.0 * r.2,
            100.0 * r.3,
            helpers,
            waited,
            null(out)
        );
    };
    row(
        "floor (start at the answer)",
        &floor_blocks,
        0.0,
        0.0,
        &floor_output,
    );
    let total_today = today_blocks.iter().sum::<u64>() as f64;
    for s in &schemes {
        row(
            &s.name,
            &s.critical,
            s.helper_passes as f64 / total_today,
            s.waited as f64 / total_today,
            &s.output,
        );
    }
    println!("(helpers: their passes over today's; waited: real passes' worth spent waiting for a shadow)");
}
