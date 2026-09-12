//! Running a circuit sample by sample.
//!
//! The same modified nodal analysis the AC solver uses, in real arithmetic and
//! stepped through time: reactances become companion models -- a conductance
//! and a current source standing in for what the part remembers -- and the
//! matrix is refactorised whenever something about it changes.
//!
//! Having both solvers is worth more than either alone. A linear network has
//! one right answer, and the two arrive at it by completely different routes:
//! one solves a complex system once per frequency, the other integrates
//! through time and has its spectrum taken. When they agree, both are working.
//! `the_two_solvers_agree` is the test that says so, and it is the closest
//! thing to an independent check either of them can have.

use super::device::{
    AnyDevice, Bipolar, Core, Device, Diode, Jfet, Linearisation, Mark, OpAmp, Pentode, Stamper,
    Triode,
};
use super::netlist::{Circuit, Part, GROUND};
use super::partition::{ReducedLinear, ReducedNonlinear};

/// Enable flush-to-zero and denormals-are-zero in the MXCSR register.
/// What one Newton pass achieved.
enum Pass {
    /// The answer has stopped moving and every device agrees it is done.
    Settled,
    /// A step was taken and it improved matters. There is more to do.
    Moved,
    /// No step of any permitted length improved the residual. Stopping here
    /// and keeping the last good answer is the deterministic fallback; going
    /// round again would only compute the same refusal.
    Stuck,
}

/// Called once at startup. Without this, denormal f64 values cause 10-100x
/// slower arithmetic on x86/x64, producing crackling when signals get small.
pub fn enable_ftz_daz() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let mut mxcsr: i32 = 0;
        std::arch::asm!("stmxcsr [{0}]", in(reg) &mut mxcsr, options(nostack, preserves_flags));
        mxcsr |= 0x8040;
        std::arch::asm!("ldmxcsr [{0}]", in(reg) &mxcsr, options(nostack, preserves_flags));
    }
}

/// How still the solve has to get before it is called settled, in volts.
const TOLERANCE: f64 = 1e-6;

/// The relative part of the convergence test. See `iterate`.
const RELATIVE: f64 = 1e-6;
/// How many passes a sample may take before the last good answer is used
/// instead. A circuit driven somewhere absurd should go quiet, not explode.
///
/// This was briefly eight, on the reasoning that the extrapolation from the
/// previous two samples converges in two or three passes and the rest are
/// rare enough not to matter. The first half is true and the second is not.
/// Measured on the 5150 lead channel with `examples/drivecheck.rs`: at eight
/// passes a 0.82 V input reads 190 % distortion, 3.3 V reads 107 % and 6.6 V
/// reads 428 %, and at thirty-two the same three read 49 %, 48 % and 52 %. A
/// hundred per cent distortion is not a quality setting, it is an unconverged
/// solve leaving the callback. The cost is real and belongs in the
/// formulation, not in the iteration count -- see
/// `docs/experiments/solver-iteration-bound.md`.
///
/// Sixty-four rather than thirty-two. The 32 figure was measured on steady
/// sine waves and is enough for those; a pick attack is a genuine
/// discontinuity at the input, and the grid limiter in `device.rs` walks a
/// valve grid about a volt a pass above cutoff. A pick that takes the grid
/// from -30 V to +20 V is fifty passes of walking before the solve can even
/// start checking, and 32 runs out partway through. The extra passes only
/// fire on samples that were already failing -- an ordinary sample settles
/// in two to three and a half. The hard ceiling itself bounds that tail; the
/// plugin no longer hides solver bursts behind an internal audio FIFO.
const MAX_ITERATIONS: usize = 64;

/// The fewest passes the work budget may ever leave a sample.
///
/// Twelve was chosen so it would be "comfortably above where they normally
/// finish" -- and that sentence is the bug. The work budget's entire job is to
/// bind on a block that is running late, and a floor above where the solve
/// normally finishes cannot bind on anything. Measured: with the budget armed
/// the 5150 went from 5.4 % of callbacks missed to 6.0 %, and the Twin from
/// 0.7 % to 4.1 %. It could not save a single pass, and it still paid for
/// reading the clock.
///
/// BUG-008 is real and is why a floor exists at all: a permanent ceiling of
/// eight gave the 5150's lead channel 190, 107 and 428 per cent distortion.
/// But that was measured before the grid limiter and the line search took that
/// same channel from 7.69 passes a sample to 3.71. The circuits now average
/// 2.0 to 3.3 on real playing, so four is where a solve that has genuinely
/// stopped converging gets cut off, not where an ordinary one does.
///
/// The floor and the budget's steps are two different numbers for two
/// different jobs: this one stops a *permanent* starvation from a caller bug,
/// and `Budget::ceiling` decides how hard to lean on the tail of one late
/// block. Conflating them is what made the guarantee do nothing.
///
/// Twelve stands, because the budget is off -- see `plugin::Budget`. Lowering
/// this to four so the budget could bite was tried and measured: it made the
/// misses worse, because a capped solve that fails to converge spends its
/// whole allowance and hands the next sample a worse place to start from.
const PASS_FLOOR: usize = 12;

/// How many times a Newton step may be halved before the solve gives up on it.
///
/// The default, and what every circuit uses unless it has been measured to
/// want otherwise -- see `Simulation::set_backtracks`.
///
/// The full step is tried first and almost always taken, so this is the depth
/// of a path the solver rarely walks. Six halvings reach a sixty-fourth, which
/// is far enough to step into a sliver a full step jumps over, and the cost of
/// reaching it is bounded and known.
/// How many plain Newton passes to take before the line search is worth its
/// cost.
///
/// A solve that has not converged by here is not going to on its own. Eight is
/// measured rather than chosen: the circuits here average between 3.6 and 3.9
/// passes, so a threshold of four fires on ordinary samples and charges every
/// circuit for a residual it did not need -- the 73P went from 36.1 % of
/// realtime to 41.4 % for nothing, its two unsettled solves a second unchanged.
/// Twelve is too late: the 5150's convergence failures come back.
///
/// At eight the 5150 keeps none, the Mark IIC+ and the Big Muff pay about a
/// point and a half, and the 73P pays two.
const FULL_STEPS: usize = 8;

/// How much a pass has to shrink the correction to count as converging.
///
/// Newton near the answer squares the error, so a healthy pass cuts `moved`
/// by orders of magnitude and a factor of two is a generous floor. A pass that
/// does not manage even that is not approaching the answer, it is oscillating
/// around it, and oscillation is exactly the case the line search exists for.
///
/// This is the test `FULL_STEPS` was standing in for. A count is a poor proxy:
/// tuned on the gain circuits, where a stalled solve is rare, it left the
/// 5150's power amplifier taking eight plain passes that got nowhere and then
/// converging on the ninth -- the first with the search on. A quarter of that
/// stage's samples landed on exactly nine passes, which is not a convergence
/// distribution, it is a wall.
const CONVERGING: f64 = 0.5;

/// How much bigger than the replayed pivot an entry below it may be before the
/// order is considered out of date.
///
/// Partial pivoting takes the largest, but it does not have to: any nonzero
/// pivot gives an exact LU, and the only thing at stake is how much the
/// entries grow during elimination. Circuit simulators therefore use a
/// threshold rather than the strict maximum -- KLU's default is a thousand --
/// and the value here is measured.
///
/// Taking the strict maximum makes the order look out of date constantly: the
/// 5150's power stage abandoned its plan on 34667 passes a second at a
/// threshold of one against 21643 at sixteen, and paid for a search each time.
/// Past sixteen there is nothing more to win -- what is left are real order
/// changes, tubes crossing between cutoff and conduction, which the other
/// guard catches -- so this is as loose as it has any reason to be.
///
/// The output is bit-identical at every threshold from one to a thousand. On
/// these matrices the pivot choice never changes the answer to double
/// precision; it only changes how often the search has to run.
const GROWTH: f64 = 16.0;

const MAX_BACKTRACKS: usize = 6;

/// The shortest step the line search will try, as a fraction of Newton's own.
const MIN_LAMBDA: f64 = 1.0 / 64.0;
/// The operating point is hunted once and can afford to be patient.
const DC_ITERATIONS: usize = 500;

/// What an inductor is stamped as in the direct-current matrix.
///
/// A tenth of a milliohm: a short by any measure that matters -- a hundred
/// milliamps through it drops nine microvolts -- while staying close enough to
/// the megohm grid leaks in the same matrix that the solve still converges.
/// It was a millionth of an ohm, and twelve orders of magnitude across one
/// matrix is more than the arithmetic has to give: the operating point came
/// out right and the residual never reached the tolerance, so a circuit
/// working at four hundred volts reported that it had not settled when it had.
///
/// It is also where an inductor's standing *current* comes from. See
/// `find_operating_point`.
const INDUCTOR_DC: f64 = 1e4;

/// A capacitor's companion, by the trapezoidal rule.
#[derive(Clone, Copy)]
struct Capacitor {
    a: usize,
    b: usize,
    conductance: f64,
    /// Current the companion source carries into the next step.
    history: f64,
    voltage: f64,
}

/// An inductor's, likewise.
#[derive(Clone, Copy)]
struct Inductor {
    a: usize,
    b: usize,
    conductance: f64,
    history: f64,
    current: f64,
}

pub struct Simulation {
    circuit: Circuit,
    rate: f64,
    n: usize,
    /// Everything that does not change from sample to sample.
    base: Vec<f64>,
    /// The same, as the circuit stands at DC: capacitors open and inductors
    /// shorted, rather than carrying the companion conductances they have when
    /// time is running.
    base_dc: Vec<f64>,
    /// The working matrix, and its factorisation.
    matrix: Vec<f64>,
    pivots: Vec<usize>,
    /// Per row, the column its last nonzero sits in. See `factorise`.
    /// The circuit's *structure*, worked out once from the netlist and the
    /// devices' declared footprints rather than rescanned from the values on
    /// every factorisation.
    ///
    /// `reach_template[row]` is the last column that row can hold a nonzero
    /// in; `depth_template[col]` is the last row that column can. The
    /// factorisation used to derive the first by scanning each row from the
    /// right for its last nonzero -- which walks the trailing zeros, so on a
    /// banded matrix it read most of the row to find an entry near the
    /// diagonal -- and it never had the second at all, so the pivot search
    /// read the whole column below the diagonal every time. Between them
    /// those two scans were about half of the factorisation, and the
    /// factorisation is half of the plugin. Measured on the four preamplifier
    /// circuits, taking the structure from the netlist instead is worth 30 to
    /// 37 per cent of the whole solve, and it is bit for bit identical: the
    /// entries skipped are the ones that were zero.
    /// The most Newton passes a sample may take, when a caller has asked for
    /// less than `MAX_ITERATIONS`.
    ///
    /// This is the deliberate last resort of layer 5a, and it is **not** the
    /// mistake BUG-008 records. That was `MAX_ITERATIONS` cut from 32 to 8
    /// permanently, so every sample at high drive came out unconverged and the
    /// 5150 measured 190 per cent distortion. This is a ceiling the host's own
    /// clock lowers for the tail of a block that is running out of time, put
    /// back at the top of the next one, floored so it can never reach the
    /// figure that broke it, and counted so a session doing it is visible
    /// rather than quietly worse.
    ///
    /// The trade it makes: a handful of less-converged samples against a block
    /// the host does not get at all.
    ceiling: usize,
    /// Samples finished at the ceiling rather than by converging.
    pinched: u64,
    watching: bool,
    violations: usize,
    reach_template: Vec<usize>,
    depth_template: Vec<usize>,
    reach: Vec<usize>,
    /// The working column bounds, refreshed from `depth_template` each call.
    depth: Vec<usize>,
    /// Per row, the column its L part starts in. The other end of the same
    /// idea, used by the forward substitution.
    first: Vec<usize>,
    /// Fixed topology mask used to derive `reach_template` and
    /// `depth_template` exactly once.
    ///
    /// The circuit's wiring never changes when a pot moves or the host sample
    /// rate changes. Keeping this storage in the simulation avoids allocating
    /// the old temporary `Vec<bool>` from `map_structure()` during a rebuild,
    /// and `structure_mapped` lets later rebuilds reuse the cached bounds
    /// outright.
    structure_pattern: Vec<bool>,
    structure_mapped: bool,
    /// The pivot row chosen for each column, learned once and replayed.
    ///
    /// This is KLU's refactorisation, and the reason it works here is that
    /// the entries which force a pivot come from the *linear* parts -- an
    /// inductor's or a transformer's current row, whose diagonal is zero or
    /// small -- and those do not change between Newton passes. What does
    /// change is the nonlinear device conductances, and those land on the
    /// conductance block, which a nodal matrix makes diagonally dominant by
    /// construction: the diagonal is the sum of the conductances at that node
    /// and the off-diagonals are the individual ones. So the order chosen the
    /// first time is still the order the search would choose the thousandth.
    ///
    /// Measured: removing the search outright left the 5150's power stage and
    /// the Mark IIC+'s both 19 % cheaper and their output bit-identical --
    /// identical because on these circuits the search never once picked
    /// anything but the diagonal.
    plan: Vec<usize>,
    /// Whether `plan` holds an order worth replaying. Cleared by `rebuild`,
    /// and by the guard below.
    planned: bool,
    /// How often a replayed pivot order had to be abandoned and searched
    /// again. Telemetry: a circuit that does this often is one the plan does
    /// not suit, and it should be paying for the search every time instead.
    replans: u64,
    rhs: Vec<f64>,
    /// Input and reactive contributions, constant throughout one Newton solve.
    /// Prepared before the solve; scratch rather than committed circuit state.
    fixed_rhs: Vec<f64>,
    /// Previous line-search residual within this sample. Reset before each
    /// transient solve; scratch, not physical state to copy on stereo wake-up.
    search_merit: f64,
    voltage: Vec<f64>,
    capacitors: Vec<Capacitor>,
    inductors: Vec<Inductor>,
    /// What each reactance was carrying, held across a rebuild. See `rebuild`.
    carried_c: Vec<(f64, f64)>,
    carried_l: Vec<f64>,
    /// The rate the device list was built at. A device outlives a rebuild
    /// unless the timestep it was built with has moved. See `rebuild`.
    device_rate: f64,
    /// Whether there is any audio in flight to protect.
    ///
    /// True from construction, a reset, or an explicit hunt, until the first
    /// sample goes through. It decides what a moved control means: with
    /// nothing in flight the circuit should simply settle where the control
    /// now puts it, and mid-signal it must not, because settling means solving
    /// the circuit with no signal in it and then setting every capacitor from
    /// that answer. See `process`.
    at_rest: bool,
    controls: Vec<f64>,
    /// The drive the input source injects for one volt in.
    source: Vec<f64>,
    /// What the rails inject, which does not depend on the signal.
    bias: Vec<f64>,
    /// Every device the solver linearises, stored inline rather than boxed.
    /// See `AnyDevice` for why.
    devices: Vec<AnyDevice>,
    /// Scratch for a Newton pass.
    work: Vec<f64>,
    guess: Vec<f64>,
    /// Scratch for the substitution: the permuted right-hand side that
    /// `substitute` writes and reads while it solves.
    ///
    /// Held rather than allocated where it is used. This was a heap
    /// allocation on the audio thread, once per Newton pass -- about one
    /// million a second in stereo at four times oversampling. Allocating
    /// there is not merely slow: `malloc` can take a lock, and a lock on
    /// the audio thread is a dropout.
    scratch: Vec<f64>,
    /// The extrapolated Newton starting point for this sample. Kept for
    /// diagnosis and for `jump`'s log. The failure branch below does *not*
    /// use this for its bound any more -- see `recent_move` for why.
    predicted: Vec<f64>,
    /// The per-node movement from `V[n-2]` to `V[n-1]`, taken *before* the
    /// predictor runs.
    ///
    /// This is the scale the failed-iterate bound is measured in, and it
    /// has to be taken before the predictor because the predictor
    /// overwrites `voltage` with the extrapolation and shifts `earlier`
    /// up one -- after which `|voltage - earlier|` is
    /// `|V_pred - V[n-1]|`, which happens to equal the movement by
    /// accident of the extrapolation's arithmetic and stops equalling it
    /// the moment the extrapolation is skipped.
    ///
    /// It is per node because the nodes in these circuits do not share a
    /// scale -- a power tube's plate swings hundreds of volts and the
    /// speaker node swings a few -- and the bound has to be meaningful
    /// on both.
    recent_move: Vec<f64>,
    /// Whether the previous sample's solve failed to converge.
    ///
    /// When it did, extrapolating from its answer is worse than not
    /// extrapolating at all. A failed sample's answer is a bounded guess,
    /// not a converged value, and `2 V[n-1] - V[n-2]` built from it
    /// extends a guess by doubling its distance from the sample before it.
    /// On the sample after a failure the correct starting point is
    /// `V[n-1]` directly -- the bounded guess the previous sample already
    /// produced -- and letting Newton walk from there, rather than from a
    /// number that has been exaggerated by the extrapolation.
    last_was_unsettled: bool,
    /// The answer before that, so the next sample can be started from where
    /// the last two were heading rather than from where the last one was.
    earlier: Vec<f64>,
    /// Whether the last stamp sits exactly at the point it was given, which
    /// is what says whether its residual may be believed.
    exact: bool,
    /// Whether the next solve may be started from an extrapolation. False if
    /// anything in the circuit switches rather than bends.
    predictable: bool,
    dirty: bool,
    /// How much work the solver is actually doing, which is the only way to
    /// tell a circuit that costs what it should from one iterating itself to
    /// a standstill.
    solves: u64,
    newton_passes: u64,
    unsettled: u64,
    rebuilds: u64,
    /// Where the devices are linearised for the stamp about to be taken.
    /// Either where the solve currently is, or a point the line search is
    /// trying out.
    point: Vec<f64>,
    /// The point being tried.
    trial: Vec<f64>,
    /// Every device's linearisation, saved so a rejected trial can undo it.
    saved: Vec<Linearisation>,
    /// How often the full Newton step was refused and a shorter one taken.
    backtrack_count: u64,
    /// Solves where no step of any length improved the residual.
    fallbacks: u64,
    /// Newton corrections that came back non-finite.
    nonfinite: u64,
    /// How far the last pass wanted to move, in convergence-test units. The
    /// solve loop watches it shrink; see `CONVERGING`.
    moved: f64,
    /// How short a step this circuit's line search will try. See
    /// `set_backtracks`.
    backtracks: usize,
    /// Zero right-hand side used while constructing/refactoring the cached
    /// linear partition. Stored here so a later rebuild never allocates a
    /// throwaway `vec![0.0; n]` on the audio thread.
    partition_zero_rhs: Vec<f64>,
    /// Whether the one allowed construction attempt for `linear_partition`
    /// has happened. If a topology cannot be reduced, later audio-thread
    /// rebuilds stay on the full factorised path instead of retrying an
    /// allocating constructor.
    partition_initialized: bool,
    /// Exact output-boundary reduction for linear circuits. Once created for
    /// a linear topology, its storage is refreshed in place rather than
    /// reconstructed.
    linear_partition: Option<ReducedLinear>,
    /// Fixed set of unknowns that contains every terminal touched by a
    /// nonlinear device, every MNA branch-current row, and the output node.
    ///
    /// Any unknown not in this set is therefore part of a purely linear block.
    /// Device stamps can change only the boundary-boundary block and boundary
    /// RHS, which is the structural proof that lets `ReducedNonlinear` eliminate
    /// the remaining unknowns exactly before every Newton factorisation.
    nonlinear_boundary: Vec<usize>,
    /// Construction of a nonlinear reduction is attempted only once. If the
    /// passive internal block is singular or the reduction would not be worth
    /// its recovery cost, the simulation permanently retains the original full
    /// MNA path rather than allocating/retrying from an audio-thread rebuild.
    nonlinear_partition_initialized: bool,
    /// Transient companion-model reduction used by normal audio samples.
    nonlinear_partition: Option<ReducedNonlinear>,
    /// DC companion-model reduction. This may legitimately be unavailable even
    /// when the transient reduction works (for example if opening capacitors at
    /// DC makes the chosen internal block singular); DC then uses the original
    /// full MNA solve while realtime audio still gets the reduced path.
    nonlinear_partition_dc: Option<ReducedNonlinear>,
}

impl Simulation {
    /// Resume an identically configured simulation at the source's current
    /// sample. This only copies into construction-time storage: no allocation,
    /// circuit rebuild, device construction or operating-point solve occurs.
    ///
    /// Controls/topology/rate must already agree. The destination may be dirty
    /// because it received controls while dormant, so copy the prepared numeric
    /// caches as well as the evolving physical and Newton state. Statistics
    /// remain local to each simulation and count only work it actually did.
    pub fn copy_runtime_state_from(&mut self, source: &Self) {
        debug_assert_eq!(self.n, source.n);
        debug_assert_eq!(self.circuit.output, source.circuit.output);
        debug_assert_eq!(self.circuit.parts.len(), source.circuit.parts.len());
        debug_assert_eq!(self.rate, source.rate);
        debug_assert_eq!(self.controls, source.controls);
        debug_assert_eq!(self.ceiling, source.ceiling);
        debug_assert_eq!(self.backtracks, source.backtracks);
        debug_assert_eq!(self.capacitors.len(), source.capacitors.len());
        debug_assert_eq!(self.inductors.len(), source.inductors.len());
        debug_assert_eq!(self.devices.len(), source.devices.len());

        self.base.copy_from_slice(&source.base);
        self.base_dc.copy_from_slice(&source.base_dc);
        self.source.copy_from_slice(&source.source);
        self.bias.copy_from_slice(&source.bias);
        self.matrix.copy_from_slice(&source.matrix);
        self.pivots.copy_from_slice(&source.pivots);
        self.reach.copy_from_slice(&source.reach);
        self.depth.copy_from_slice(&source.depth);
        self.first.copy_from_slice(&source.first);
        self.plan.copy_from_slice(&source.plan);
        self.planned = source.planned;
        self.voltage.copy_from_slice(&source.voltage);
        self.predicted.copy_from_slice(&source.predicted);
        self.earlier.copy_from_slice(&source.earlier);
        self.recent_move.copy_from_slice(&source.recent_move);
        self.last_was_unsettled = source.last_was_unsettled;
        self.exact = source.exact;
        self.predictable = source.predictable;
        self.moved = source.moved;
        self.capacitors.copy_from_slice(&source.capacitors);
        self.inductors.copy_from_slice(&source.inductors);
        for (dst, src) in self.devices.iter_mut().zip(&source.devices) {
            dst.copy_runtime_state_from(src);
        }
        self.device_rate = source.device_rate;
        self.at_rest = source.at_rest;
        self.dirty = source.dirty;

        match (&mut self.linear_partition, &source.linear_partition) {
            (Some(dst), Some(src)) => dst.copy_runtime_state_from(src),
            (None, None) => {}
            _ => panic!("runtime copy requires identical linear partitions"),
        }
        for (dst, src) in [
            (&mut self.nonlinear_partition, &source.nonlinear_partition),
            (
                &mut self.nonlinear_partition_dc,
                &source.nonlinear_partition_dc,
            ),
        ] {
            match (dst, src) {
                (Some(dst), Some(src)) => dst.copy_runtime_state_from(src),
                (None, None) => {}
                _ => panic!("runtime copy requires identical nonlinear partitions"),
            }
        }
        // Topology masks and partition selection are immutable after `new`.
        // Work/RHS/fixed_rhs/guess/trial/saved/carry buffers and search_merit
        // are overwritten before use; they contain no committed sample state.
    }

    pub fn new(circuit: Circuit, rate: f64) -> Self {
        let n = circuit.unknowns();
        let initial_voltages = circuit.initial_voltages.clone();

        // The topology is immutable for the lifetime of a `Simulation`, so
        // size every rebuild-owned list once. `clear()` then retains exactly
        // the storage a later control/rate rebuild needs instead of growing a
        // carry/device buffer for the first time on the audio thread.
        let mut capacitor_count = 0usize;
        let mut inductor_count = 0usize;
        let mut device_count = 0usize;
        for part in &circuit.parts {
            match part {
                Part::Capacitor { .. } => capacitor_count += 1,
                Part::Inductor { .. } => inductor_count += 1,
                Part::Diode { .. }
                | Part::Triode { .. }
                | Part::Pentode { .. }
                | Part::Jfet { .. }
                | Part::Bipolar { .. }
                | Part::Core { .. }
                | Part::OpAmp { .. } => device_count += 1,
                _ => {}
            }
        }

        // Build the Schur boundary from topology, before any numeric stamp is
        // taken. Every nonlinear part terminal has to be on the boundary because
        // its Jacobian or equivalent-current source can move on every Newton
        // pass. Every branch-current unknown is included too: ideal transformers
        // are linear, but keeping their zero-diagonal MNA constraint rows out of
        // A_ii makes the passive internal block a plain nodal conductance block
        // and avoids an avoidable singular partition. The output is cheap to keep
        // on the boundary and makes diagnostics/direct reads straightforward.
        let mut nonlinear_boundary_mask = vec![false; n];
        for (index, part) in circuit.parts.iter().enumerate() {
            if !part.is_linear() {
                for node in part.touches() {
                    if node != GROUND && node < n {
                        nonlinear_boundary_mask[node] = true;
                    }
                }
            }
            if part.needs_branch() {
                let branch = circuit.branch_of(index);
                if branch < n {
                    nonlinear_boundary_mask[branch] = true;
                }
            }
        }
        if circuit.output < n {
            nonlinear_boundary_mask[circuit.output] = true;
        }
        let nonlinear_boundary: Vec<usize> = nonlinear_boundary_mask
            .iter()
            .enumerate()
            .filter_map(|(index, &selected)| if selected { Some(index) } else { None })
            .collect();

        // The middle, except where the circuit says otherwise: a front-panel
        // control the plugin does not expose still has to sit where a player
        // leaves it rather than at half its travel. See `Netlist::rest`.
        let mut controls = vec![0.5; circuit.controls.max(1)];
        for &(which, position) in &circuit.resting {
            if which < controls.len() {
                controls[which] = position;
            }
        }
        let mut sim = Self {
            circuit,
            rate,
            n,
            base: vec![0.0; n * n],
            base_dc: vec![0.0; n * n],
            matrix: vec![0.0; n * n],
            pivots: vec![0; n],
            ceiling: MAX_ITERATIONS,
            pinched: 0,
            watching: false,
            violations: 0,
            reach_template: vec![0; n],
            depth_template: vec![0; n],
            reach: vec![0; n],
            depth: vec![0; n],
            first: vec![0; n],
            structure_pattern: vec![false; n * n],
            structure_mapped: false,
            plan: vec![0; n],
            planned: false,
            replans: 0,
            rhs: vec![0.0; n],
            fixed_rhs: vec![0.0; n],
            search_merit: 0.0,
            voltage: vec![0.0; n],
            capacitors: Vec::with_capacity(capacitor_count),
            inductors: Vec::with_capacity(inductor_count),
            carried_c: Vec::with_capacity(capacitor_count),
            carried_l: Vec::with_capacity(inductor_count),
            device_rate: f64::NAN,
            at_rest: true,
            controls,
            source: vec![0.0; n],
            bias: vec![0.0; n],
            devices: Vec::with_capacity(device_count),
            work: vec![0.0; n * n],
            guess: vec![0.0; n],
            scratch: vec![0.0; n],
            predicted: vec![0.0; n],
            recent_move: vec![0.0; n],
            last_was_unsettled: false,
            earlier: vec![0.0; n],
            exact: true,
            predictable: false,
            dirty: true,
            solves: 0,
            newton_passes: 0,
            unsettled: 0,
            rebuilds: 0,
            point: vec![0.0; n],
            trial: vec![0.0; n],
            saved: Vec::with_capacity(device_count),
            backtrack_count: 0,
            fallbacks: 0,
            nonfinite: 0,
            moved: f64::INFINITY,
            backtracks: MAX_BACKTRACKS,
            partition_zero_rhs: vec![0.0; n],
            partition_initialized: false,
            linear_partition: None,
            nonlinear_boundary,
            nonlinear_partition_initialized: false,
            nonlinear_partition: None,
            nonlinear_partition_dc: None,
        };
        sim.rebuild();
        // Apply initial voltages from the circuit to help the DC solver
        // find the operating point for circuits with floating nodes.
        for &(node, volts) in &initial_voltages {
            if node != GROUND && node < sim.voltage.len() {
                sim.voltage[node] = volts;
            }
        }
        sim.find_operating_point();
        sim
    }

    /// Solves, Newton passes, solves that never settled, and matrix rebuilds.
    /// How big the matrix is: a row per node plus one per part that imposes a
    /// voltage. The factorisation costs the cube of this, so it is the first
    /// number to look at when a circuit turns out to be expensive.
    pub fn unknowns(&self) -> usize {
        self.n
    }

    /// Size of the realtime nonlinear solve after exact Schur reduction:
    /// `(boundary_unknowns, eliminated_internal_unknowns)`. `None` means this
    /// circuit stays on the original full-MNA path.
    pub fn nonlinear_reduction(&self) -> Option<(usize, usize)> {
        self.nonlinear_partition
            .as_ref()
            .map(|partition| (partition.boundary_len(), partition.internal_len()))
    }

    /// What the line search had to do: steps refused, solves where nothing
    /// of any length helped, and non-finite Newton corrections.
    ///
    /// Separate from `statistics` so that adding to it does not change a
    /// tuple half the harnesses already destructure.
    pub fn health(&self) -> (u64, u64, u64) {
        (self.backtrack_count, self.fallbacks, self.nonfinite)
    }

    /// How often a replayed pivot order had to be thrown away and searched
    /// again. A circuit that does this often is one the plan does not suit.
    pub fn replans(&self) -> u64 {
        self.replans
    }

    pub fn statistics(&self) -> (u64, u64, u64, u64) {
        (
            self.solves,
            self.newton_passes,
            self.unsettled,
            self.rebuilds,
        )
    }

    pub fn is_linear(&self) -> bool {
        self.devices.is_empty()
    }

    /// Snapshot the assembled matrix and source vector of a linear circuit.
    /// This is intended for exact partitioning experiments; nonlinear
    /// simulations must remain on the normal Newton path.
    pub fn linear_system(&self) -> Option<(Vec<f64>, Vec<f64>, usize)> {
        self.is_linear()
            .then(|| (self.base.clone(), self.source.clone(), self.circuit.output))
    }

    /// Snapshot one raw nonlinear Jacobian before factorisation. This is a
    /// diagnostic surface for partitioning experiments and is not used by
    /// the realtime solver.
    pub fn jacobian_snapshot(&mut self, input: f64) -> Option<(Vec<f64>, Vec<f64>, usize)> {
        if self.is_linear() {
            return None;
        }
        if self.dirty {
            self.rebuild();
        }
        self.point.copy_from_slice(&self.voltage);
        self.prepare_rhs(input, false);
        self.build(false, true);
        Some((self.work.clone(), self.rhs.clone(), self.circuit.output))
    }

    /// What a node is sitting at. Worth having: an operating point is the
    /// first thing to check when a stage sounds wrong, and it is a number the
    /// data sheet also has.
    pub fn voltage_at(&self, node: usize) -> f64 {
        self.voltage.get(node).copied().unwrap_or(0.0)
    }

    /// The operating point voltage vector. Used to share a DC solution between
    /// identical channels so the second one does not have to hunt it again.
    ///
    /// After the first sample this is the *running* state, not the operating
    /// point, so it is only meaningful to share it while `needs_operating_point`
    /// is still true. See `apply_operating_point`.
    pub fn operating_point(&self) -> &[f64] {
        &self.voltage
    }

    /// Whether this circuit still has its DC hunt in front of it: a control,
    /// sample rate, or deferred reset has made it dirty *and* there is no audio
    /// in flight to destroy.
    ///
    /// The one moment at which sharing a solution between identical channels
    /// is sound: before it, there is nothing new to share; after it, what
    /// would be shared is a running state and not an operating point.
    ///
    /// `at_rest` is the half that was missing, and leaving it out was audible.
    /// `dirty` means only that a pot has moved, and a pot moves whenever a
    /// player touches a knob -- so mid-playback this said yes once a block for
    /// as long as the knob was turning. `Plugin::process` takes that as
    /// permission to hunt channel 0's operating point and hand it to every
    /// other channel, and both halves of that are destructive with audio in
    /// flight: hunting solves the circuit *with no signal in it* and sets
    /// every capacitor from the answer, and sharing then replaces the other
    /// channels' running state with it. The audible result is the music with
    /// a block-rate transient laid over the top of it, worse the harder the
    /// circuit is driven, for exactly as long as a knob is held.
    ///
    /// `Simulation::process` has always drawn this distinction correctly --
    /// it rebuilds on `dirty` but only hunts `if self.at_rest`. This is the
    /// same distinction, which is why it is the same flag.
    pub fn needs_operating_point(&self) -> bool {
        self.dirty && self.at_rest
    }

    /// Apply an operating point from another identical simulation. Rebuilds
    /// the matrix (so component values are current), sets the node voltages,
    /// initialises the reactance state to steady state, and clears the dirty
    /// flag so the next `process` call skips the expensive DC hunt.
    ///
    /// This sets every capacitor's stored charge from the voltages it is
    /// handed, so it does not only seed a solve -- it *replaces* the running
    /// state. Calling it on a channel that is already running discards
    /// whatever that channel was doing. Measured with `examples/sharing.rs`:
    /// doing it once a block put the right channel of a TS808 three decibels
    /// from where it should have been, and made the output depend on the host
    /// buffer size. Call it only when `needs_operating_point` says there is
    /// nothing to discard.
    pub fn apply_operating_point(&mut self, voltage: &[f64]) {
        if self.dirty {
            self.rebuild();
        }
        self.voltage.copy_from_slice(voltage);
        for c in &mut self.capacitors {
            let v = across(&self.voltage, c.a, c.b);
            c.voltage = v;
            c.history = c.conductance * v;
        }
        for l in &mut self.inductors {
            l.current = INDUCTOR_DC * across(&self.voltage, l.a, l.b);
            l.history = -l.current;
        }
        self.predicted.copy_from_slice(&self.voltage);
        self.earlier.copy_from_slice(&self.voltage);
        self.last_was_unsettled = false;
        self.dirty = false;
    }

    pub fn controls(&self) -> usize {
        self.circuit.controls
    }

    /// Where the circuit rests a control that nobody turns, if it rests it at
    /// all. See `Netlist::rest`.
    pub fn resting_position(&self, which: usize) -> Option<f64> {
        self.circuit
            .resting
            .iter()
            .find(|&&(control, _)| control == which)
            .map(|&(_, position)| position)
    }

    pub fn set_control(&mut self, which: usize, position: f64) {
        if which < self.controls.len() && (self.controls[which] - position).abs() > 1e-12 {
            self.controls[which] = position.clamp(0.0, 1.0);
            self.dirty = true;
        }
    }

    pub fn set_rate(&mut self, rate: f64) {
        if (self.rate - rate).abs() > 1e-9 {
            self.rate = rate;
            self.dirty = true;
        }
    }

    /// Stamps everything that does not depend on the solution, and factorises
    /// it once. A pot moving or the rate changing is what invalidates this.
    fn rebuild(&mut self) {
        self.rebuilds += 1;
        let n = self.n;
        self.base.iter_mut().for_each(|x| *x = 0.0);
        self.base_dc.iter_mut().for_each(|x| *x = 0.0);
        self.source.iter_mut().for_each(|x| *x = 0.0);
        self.bias.iter_mut().for_each(|x| *x = 0.0);
        // Carry every reactance's state across the rebuild.
        //
        // A rebuild happens when a control moves, and a control moving is a
        // resistance changing. Nothing else about the circuit changes: the
        // parts are the same parts in the same order, and every capacitor
        // still holds the charge it held. Rebuilding the lists from scratch
        // and leaving them at zero threw that charge away, and the only thing
        // standing between that and silence was a DC hunt from `process`,
        // which replaced the running audio with the operating point.
        //
        // Measured with `examples/knobmove.rs`: a knob nudged by one part in a
        // million at every block boundary -- a move that cannot change the
        // circuit -- put an error *louder than the signal* through the Mark
        // IIC+ and the 73P, and moved every other voice by four to fifteen
        // decibels. That is what a knob sounded like while it was turning.
        //
        // What is carried is the branch current, not the companion term,
        // because the companion term depends on the conductance and the
        // conductance moves with the sample rate.
        self.carried_c.clear();
        for c in &self.capacitors {
            self.carried_c
                .push((c.voltage, c.history - c.conductance * c.voltage));
        }
        self.carried_l.clear();
        for l in &self.inductors {
            self.carried_l.push(l.current);
        }
        // A device outlives a rebuild unless the timestep has moved.
        //
        // Nothing a control does reaches a device: a diode is a diode, and a
        // pot is a conductance in the matrix. But a device can have a state --
        // a saturating core integrates the voltage across it and remembers the
        // flux -- and rebuilding the list threw that away. It is the same
        // fault as the reactances above and it showed up in the same
        // measurement: with the capacitors carried across but the devices
        // still rebuilt, the 73P, whose input transformer has a core in it,
        // was still 40 dB from where it should have been while its knob moved,
        // against 100 dB or better for every voice without one.
        //
        // Only while there is signal in flight, though. At rest the circuit is
        // being established from nothing -- and a device with a state settles
        // to a different answer depending on where it starts, so a Console
        // channel re-settled around a core carrying the *previous* setting's
        // flux read two decibels off its own calibration.
        let keep_devices = !self.at_rest
            && !self.devices.is_empty()
            && (self.rate - self.device_rate).abs() < 1e-9;
        self.capacitors.clear();
        self.inductors.clear();
        if !keep_devices {
            self.devices.clear();
        }
        let step = 1.0 / self.rate;

        let mut base = std::mem::take(&mut self.base);
        let mut base_dc = std::mem::take(&mut self.base_dc);
        let mut source = std::mem::take(&mut self.source);
        let mut bias = std::mem::take(&mut self.bias);
        {
            // `into` says which of the two matrices a part belongs in. A
            // capacitor's companion conductance is 2C/T, which for a 22 uF
            // bypass at 96 kHz is four siemens -- a quarter of an ohm. Leaving
            // that in the matrix while hunting the operating point shorts the
            // cathode to ground and the stage comes up with no bias at all.

            for (index, part) in self.circuit.parts.iter().enumerate() {
                match *part {
                    Part::Resistor { a, b, ohms } => {
                        stamp_both(&mut base, &mut base_dc, n, a, b, 1.0 / ohms)
                    }
                    Part::Pot {
                        a,
                        wiper,
                        b,
                        ohms,
                        taper,
                        control,
                    } => {
                        let position = self.controls.get(control).copied().unwrap_or(0.5);
                        let f = taper.fraction(position).clamp(1e-4, 1.0 - 1e-4);
                        stamp_both(&mut base, &mut base_dc, n, wiper, b, 1.0 / (ohms * f));
                        stamp_both(
                            &mut base,
                            &mut base_dc,
                            n,
                            a,
                            wiper,
                            1.0 / (ohms * (1.0 - f)),
                        );
                    }
                    Part::Input { node, series } => {
                        let g = 1.0 / series;
                        stamp_both(&mut base, &mut base_dc, n, node, GROUND, g);
                        if node != GROUND {
                            source[node] += g;
                        }
                    }
                    Part::Capacitor { a, b, farads } => {
                        // Trapezoidal: 2C/T, with the history current carried
                        // between steps. Only into the running matrix -- at DC
                        // a capacitor is an open circuit.
                        let conductance = 2.0 * farads / step;
                        stamp_matrix(&mut base, n, a, b, conductance);
                        self.capacitors.push(Capacitor {
                            a,
                            b,
                            conductance,
                            history: 0.0,
                            voltage: 0.0,
                        });
                    }
                    Part::Inductor { a, b, henry } => {
                        // And at DC an inductor is a piece of wire.
                        //
                        // A tenth of a milliohm of wire, not a millionth. The
                        // figure was 1e6 siemens, which is a short by any
                        // measure that matters -- a hundred milliamps through
                        // it drops ninety nanovolts -- and it sat in the same
                        // matrix as a megohm grid leak at 4.5e-6. Twelve orders
                        // of magnitude across one matrix is more than the
                        // solve's arithmetic has to give, and in a circuit
                        // working at four hundred and fifty volts the residual
                        // never came down to the tolerance: the operating point
                        // was right and the solver would not say so. At 1e4 the
                        // same current drops nine microvolts, which is still
                        // nothing, and the spread is a hundred times narrower.
                        let conductance = step / (2.0 * henry);
                        stamp_matrix(&mut base, n, a, b, conductance);
                        stamp_matrix(&mut base_dc, n, a, b, INDUCTOR_DC);
                        self.inductors.push(Inductor {
                            a,
                            b,
                            conductance,
                            history: 0.0,
                            current: 0.0,
                        });
                    }
                    Part::Supply {
                        node,
                        series,
                        volts,
                    } => {
                        let g = 1.0 / series;
                        stamp_both(&mut base, &mut base_dc, n, node, GROUND, g);
                        if node != GROUND {
                            bias[node] += g * volts;
                        }
                    }
                    Part::Diode { a, k, spec } => {
                        if !keep_devices {
                            self.devices.push(AnyDevice::Diode(Diode::new(a, k, spec)));
                        }
                    }
                    Part::Triode { p, g, k, spec } => {
                        if !keep_devices {
                            self.devices
                                .push(AnyDevice::Triode(Triode::new(p, g, k, spec)));
                        }
                    }
                    Part::Pentode {
                        p,
                        g,
                        k,
                        s,
                        count,
                        spec,
                    } => {
                        if !keep_devices {
                            self.devices
                                .push(AnyDevice::Pentode(Pentode::new(p, g, k, s, count, spec)));
                        }
                    }
                    Part::Jfet {
                        d,
                        g,
                        s: source_pin,
                        spec,
                    } => {
                        if !keep_devices {
                            self.devices
                                .push(AnyDevice::Jfet(Jfet::new(d, g, source_pin, spec)));
                        }
                    }
                    Part::Bipolar { c, b, e, spec } => {
                        if !keep_devices {
                            self.devices
                                .push(AnyDevice::Bipolar(Bipolar::new(c, b, e, spec)));
                        }
                    }
                    Part::Core { a, b, spec } => {
                        // The only device that has to be told the rate: it
                        // integrates the voltage across it, so its answer
                        // depends on how long a sample lasts.
                        if !keep_devices {
                            let rate = self.rate;
                            self.devices
                                .push(AnyDevice::Core(Core::new(a, b, spec, rate)));
                        }
                    }
                    Part::OpAmp {
                        out,
                        plus,
                        minus,
                        reference,
                        rail,
                    } => {
                        let branch = self.circuit.branch_of(index);
                        if !keep_devices {
                            self.devices.push(AnyDevice::OpAmp(OpAmp::new(
                                out, plus, minus, reference, branch, rail,
                            )));
                        }
                    }
                    Part::Transformer {
                        p1,
                        p2,
                        s1,
                        s2,
                        ratio,
                    } => {
                        let branch = self.circuit.branch_of(index);
                        stamp_branch(&mut base, n, branch, p1, p2, s1, s2, ratio);
                        stamp_branch(&mut base_dc, n, branch, p1, p2, s1, s2, ratio);
                    }
                }
            }
        }
        // And put it back. `history = G v + i` for a capacitor and
        // `-(i + G v)` for an inductor, which is the shape the advance step
        // writes and the shape `inject` expects.
        if self.carried_c.len() == self.capacitors.len() {
            for (c, &(v, i)) in self.capacitors.iter_mut().zip(&self.carried_c) {
                c.voltage = v;
                c.history = c.conductance * v + i;
            }
        }
        if self.carried_l.len() == self.inductors.len() {
            for (l, &i) in self.inductors.iter_mut().zip(&self.carried_l) {
                let v = across(&self.voltage, l.a, l.b);
                l.current = i;
                l.history = -(i + l.conductance * v);
            }
        }

        self.device_rate = self.rate;
        self.predictable = !self.devices.iter().any(|d| d.switches());
        // Capacity was reserved from the immutable topology in `new`, so
        // this resize only changes the logical length. The line search may not
        // allocate on the audio thread.
        self.saved
            .resize(self.devices.len(), Linearisation::default());
        self.base = base;
        self.base_dc = base_dc;
        self.source = source;
        self.bias = bias;
        // Wiring is immutable. Compute the elimination bounds on the first
        // assembly only and reuse them for every later control/rate rebuild.
        self.map_structure();

        // A linear topology stays linear for this simulation's lifetime. Its
        // condensed structure is therefore allocated once and only its numeric
        // coefficients need to change when a pot or sample rate moves.
        if self.devices.is_empty() {
            let output = self.circuit.output;
            if let Some(partition) = self.linear_partition.as_mut() {
                let _ = partition.refresh(&self.base, &self.partition_zero_rhs, output);
            } else if !self.partition_initialized {
                // The first rebuild is construction-time work. If reduction is
                // impossible, remember that result and use the full LU path on
                // all later rebuilds rather than retrying an allocating
                // constructor from the audio thread.
                self.linear_partition =
                    ReducedLinear::new(&self.base, &self.partition_zero_rhs, output);
                self.partition_initialized = true;
            }
        } else {
            debug_assert!(self.linear_partition.is_none());

            // Nonlinear circuits get a different exact reduction. The boundary
            // contains every coefficient/RHS position a device can change, so
            // A_ii, A_ib and A_bi are purely linear and stay fixed between
            // rebuilds. Their Schur contribution is paid here once; a Newton
            // pass then factorises only the much smaller boundary matrix.
            if !self.nonlinear_partition_initialized {
                if nonlinear_reduction_worthwhile(n, self.nonlinear_boundary.len()) {
                    self.nonlinear_partition = ReducedNonlinear::new(
                        &self.base,
                        &self.partition_zero_rhs,
                        &self.nonlinear_boundary,
                    );
                    self.nonlinear_partition_dc = ReducedNonlinear::new(
                        &self.base_dc,
                        &self.partition_zero_rhs,
                        &self.nonlinear_boundary,
                    );
                }
                self.nonlinear_partition_initialized = true;
            } else {
                if let Some(partition) = self.nonlinear_partition.as_mut() {
                    let _ = partition.refresh(&self.base, &self.partition_zero_rhs);
                }
                if let Some(partition) = self.nonlinear_partition_dc.as_mut() {
                    let _ = partition.refresh(&self.base_dc, &self.partition_zero_rhs);
                }
            }
        }
        // Numeric values have changed even though the cached topology has not.
        // Conservatively relearn the pivot plan; factorisation will cache the
        // new order after this rebuild.
        self.planned = false;

        // A linear circuit is factorised once and reused. A circuit with a
        // device in it has to be refactorised every Newton pass, because the
        // device's own conductance is part of the matrix and moves with the
        // answer.
        self.matrix.copy_from_slice(&self.base);
        factorise(
            &mut self.matrix,
            &mut self.pivots,
            &mut self.reach,
            &mut self.depth,
            &mut self.first,
            &self.reach_template,
            &self.depth_template,
            &mut self.plan,
            false,
            n,
        );
        self.dirty = false;
    }

    /// Work out which matrix positions can ever be nonzero, and turn that into
    /// the two bounds the factorisation needs.
    ///
    /// Everything the linear network stamps is already sitting in `base` and
    /// `base_dc`, so their nonzeros are read straight off. What is not there
    /// is the nonlinear devices, whose entries appear only once they are
    /// stamped and, for an op-amp, differ between its linear state and its
    /// railed one -- so each device declares its own footprint over all of its
    /// states. See `Mark`.
    ///
    /// The result is deliberately an **upper bound**. A conductance that
    /// happens to come out at exactly zero leaves a position marked that holds
    /// nothing, and the elimination then runs one column further and subtracts
    /// `factor * 0.0` from it. That changes no value, so the arithmetic stays
    /// bit for bit what a value-driven scan produced; it only declines to
    /// discover, a hundred thousand times a second, a shape that was decided
    /// when the parts were wired together.
    fn map_structure(&mut self) {
        if self.structure_mapped {
            return;
        }

        let n = self.n;
        for marked in &mut self.structure_pattern {
            *marked = false;
        }
        for (slot, marked) in self.base.iter().zip(self.structure_pattern.iter_mut()) {
            *marked |= *slot != 0.0;
        }
        for (slot, marked) in self.base_dc.iter().zip(self.structure_pattern.iter_mut()) {
            *marked |= *slot != 0.0;
        }
        {
            let mut mark = Mark {
                pattern: &mut self.structure_pattern,
                n,
            };
            for device in &self.devices {
                device.footprint(&mut mark);
            }
        }
        // The diagonal is always in play: the factorisation pivots on it, and
        // a row whose reach fell short of its own diagonal would bound the
        // elimination to nothing.
        let pattern = &self.structure_pattern;
        let reach_template = &mut self.reach_template;
        let depth_template = &mut self.depth_template;
        for row in 0..n {
            reach_template[row] = (row..n)
                .rev()
                .find(|&k| pattern[row * n + k])
                .unwrap_or(row)
                .max(row);
            depth_template[row] = (row..n)
                .rev()
                .find(|&k| pattern[k * n + row])
                .unwrap_or(row)
                .max(row);
        }
        self.structure_mapped = true;
    }

    /// Check every stamp against the declared structure, and count what falls
    /// outside it.
    ///
    /// Off by default and never on in the audio path: this is `O(n^2)` a pass
    /// and exists so `tests/devices.rs` can hold the devices to their word.
    pub fn watch_structure(&mut self) {
        self.watching = true;
        self.violations = 0;
    }

    /// Stamps seen outside the declared structure since `watch_structure`.
    pub fn violations(&self) -> usize {
        self.violations
    }

    /// How short a step this circuit's line search may try before giving up
    /// and taking the full Newton step.
    ///
    /// Per circuit, because the right answer is per circuit and both halves of
    /// that are measured. The Twin Reverb's power amplifier spirals at six:
    /// a step shortened to a sixty-fourth barely moves the solve, so the next
    /// pass stalls too and searches again, and the stage spends 3713 solves a
    /// second running out of passes and 120,182 backtracks a second getting
    /// there. Stopping at a sixteenth breaks the spiral -- in the plugin, 3.6 %
    /// of callbacks missed becomes 0.1 %, the worst falls from 7509 us to 1447,
    /// and against a high-accuracy reference **the Twin does not move at all**:
    /// -205.4, -199.3 and -189.8 dB at three drives, identical either way.
    ///
    /// The 5150 is the reason this is not simply lowered for everyone. The same
    /// change takes its answer from 209.6 dB below that reference to **45.7 dB
    /// below**, which is audible. Its solve genuinely needs the short steps,
    /// and cutting them off leaves it somewhere it had not finished with --
    /// §58.4's fourth point, in a new place.
    ///
    /// So this is a numerical parameter tuned per matrix, the way a tolerance
    /// or a preconditioner is, and not a property of any circuit. Anything
    /// changed here has to be measured both ways: the deadline *and* the
    /// distance from an accurate reference.
    pub fn set_backtracks(&mut self, most: usize) {
        self.backtracks = most.clamp(1, MAX_BACKTRACKS);
    }

    /// Cap the Newton passes a sample may take. See `ceiling`.
    ///
    /// Clamped to `PASS_FLOOR` at the bottom, so no caller -- and no bug in a
    /// caller -- can starve the solve past the point BUG-008 measured.
    pub fn set_pass_ceiling(&mut self, passes: usize) {
        self.ceiling = passes.clamp(PASS_FLOOR, MAX_ITERATIONS);
    }

    /// How many samples finished at the ceiling rather than by converging.
    pub fn pinched(&self) -> u64 {
        self.pinched
    }

    /// The two structural bounds `map_structure` worked out.
    ///
    /// Exposed so `tests/devices.rs` can check that nothing ever stamps
    /// outside it. A footprint that is wrong is not a crash: it is a matrix
    /// entry the elimination declines to touch, an answer that is quietly not
    /// the answer, and no test would notice without this.
    pub fn structure(&self) -> (Vec<usize>, Vec<usize>) {
        (self.reach_template.clone(), self.depth_template.clone())
    }

    /// Where the circuit sits with no signal on it.
    ///
    /// Hunted before any audio arrives, with the reactances held at their
    /// steady state, so the first sample starts from the operating point
    /// rather than charging up through it. Without this a valve stage spends
    /// its first tenth of a second climbing to its own bias, and whatever is
    /// listening hears that as a thump.
    pub fn find_operating_point(&mut self) -> bool {
        // The matrix has to be the one the controls currently describe. Asking
        // for an operating point after moving a control, without rebuilding
        // first, solves the circuit as it used to be -- and the answer looks
        // perfectly converged, because it is: it is the converged answer to
        // the wrong question. What gives it away is Kirchhoff's law, which the
        // returned voltages then quietly fail.
        if self.dirty {
            self.rebuild();
        }
        if self.devices.is_empty() {
            return true;
        }
        let mut settled = false;
        self.prepare_rhs(0.0, true);
        // At rest a capacitor carries no current and an inductor no voltage,
        // which is what the zeroed histories already say.
        for pass in 0..DC_ITERATIONS {
            match self.iterate(true, pass >= FULL_STEPS) {
                Pass::Settled => {
                    settled = true;
                    break;
                }
                Pass::Moved => {}
                Pass::Stuck => break,
            }
        }
        // The reactances take up whatever the operating point implies, so they
        // do not then charge through it.
        for c in &mut self.capacitors {
            let v = across(&self.voltage, c.a, c.b);
            c.voltage = v;
            c.history = c.conductance * v;
        }
        // An inductor's standing current, which is not readable the way a
        // capacitor's charge is.
        //
        // At rest a capacitor holds a voltage and no current, so its state is
        // right there in the node voltages. An inductor is the other way
        // about: it holds a current and drops no voltage, so the node voltages
        // say nothing about it and this left every inductor starting from
        // zero. Nothing noticed for a long time, because the only inductors in
        // the catalogue were in tone stacks and cabinets, where none of them
        // carries any standing current.
        //
        // The 73P's output transformer does. Its primary is T8's collector
        // load, so it carries the whole of that transistor's thirty-seven
        // milliamps -- and starting from zero, the amplifier spent a fifth of
        // a second climbing to its own operating point and put a thump of 1.5
        // out through the speaker on the way. The guitar amplifiers' output
        // chokes carry their plate current the same way.
        //
        // The direct-current matrix stamps an inductor as a conductance, so
        // the current is the small drop across it times that conductance.
        for l in &mut self.inductors {
            l.current = INDUCTOR_DC * across(&self.voltage, l.a, l.b);
            l.history = -l.current;
        }
        // The predictor state, reset to the operating point.
        //
        // Without this the first sample after a hunt runs the extrapolator
        // `2 * voltage - earlier` with `earlier` holding whatever it held
        // from before -- a previous circuit, a previous voice, a previous
        // session's running state. On a valve voice that means a first
        // Newton correction from four hundred volts at the plate to a value
        // some previous state suggested, which is a worse start than no
        // prediction at all. `apply_operating_point` already resets both
        // vectors; this is the same reset, in the hunt that does not go
        // through it.
        //
        // `last_was_unsettled` is cleared alongside, so the first sample
        // after a reset starts from a clean predictor state and is not
        // treated as following a failure.
        self.predicted.copy_from_slice(&self.voltage);
        self.earlier.copy_from_slice(&self.voltage);
        self.last_was_unsettled = false;
        self.at_rest = true;
        settled
    }

    /// Cache the input, supply and reactive-history contributions once per
    /// solve. Newton trials change device stamps, never these contributions.
    fn prepare_rhs(&mut self, input: f64, dc: bool) {
        let n = self.n;
        for k in 0..n {
            self.fixed_rhs[k] = self.source[k] * input + self.bias[k];
        }
        if !dc {
            for c in &self.capacitors {
                inject(&mut self.fixed_rhs, c.a, c.b, c.history);
            }
            for l in &self.inductors {
                inject(&mut self.fixed_rhs, l.a, l.b, l.history);
            }
        }
    }

    /// Stamp at `self.point`, starting from the constant matrix and RHS. A
    /// solve must call `prepare_rhs` before its first stamp.
    fn build(&mut self, dc: bool, limiting: bool) {
        let n = self.n;
        self.work
            .copy_from_slice(if dc { &self.base_dc } else { &self.base });
        self.rhs.copy_from_slice(&self.fixed_rhs);

        let mut stamper = Stamper {
            matrix: &mut self.work,
            rhs: &mut self.rhs,
            n,
            limiting,
            junction_held: false,
        };
        for device in &mut self.devices {
            device.stamp(&mut stamper, &self.point);
        }
        // Whether this stamp actually sits at the point it was given. See
        // `Stamper::junction_held`.
        self.exact = !stamper.junction_held;
        if self.watching {
            for row in 0..n {
                for col in 0..n {
                    if self.work[row * n + col] != 0.0
                        && (col > self.reach_template[row] || row > self.depth_template[col])
                    {
                        self.violations += 1;
                    }
                }
            }
        }
    }

    /// How badly `x` fails to satisfy the circuit as currently stamped.
    ///
    /// The companion models are built so that solving `A x = b` yields the
    /// next Newton iterate, which makes `A(x) x - b(x)` the residual of the
    /// nonlinear system itself: it is zero exactly at the solution and
    /// nowhere else. Summed and squared, that is the merit function -- one
    /// number saying how wrong a candidate is, which is what lets the line
    /// search compare two of them.
    ///
    /// Summing the squares rather than taking the largest matters: individual
    /// rows cancel each other, and a step that halves one row while doubling
    /// another is not progress.
    fn merit(&self, x: &[f64]) -> f64 {
        let n = self.n;
        let mut total = 0.0;
        for row in 0..n {
            let mut sum = -self.rhs[row];
            for col in 0..=self.reach_template[row] {
                let a = self.work[row * n + col];
                if a != 0.0 {
                    sum += a * x[col];
                }
            }
            total += sum * sum;
        }
        total
    }

    /// One Newton pass, with a bounded backtracking line search.
    ///
    /// `search` turns the line search on. With it off this is a plain full
    /// Newton step, which is what it should be while the solve is behaving:
    /// judging a step means stamping the circuit where it lands, and a stamp
    /// costs about what the factorisation does, so a pass that consults the
    /// residual costs roughly twice one that does not. Measured over a whole
    /// second the search buys about a quarter fewer passes, which does not
    /// pay for doubling them.
    ///
    /// What it does buy is the tail, and the tail is what misses deadlines.
    /// So the caller runs plain Newton first and turns this on only for a
    /// solve that is still going after `FULL_STEPS` -- one that is no longer
    /// converging but oscillating, where a shorter step is the only way out
    /// and the extra stamp is worth paying for because the alternative is
    /// thirty-two passes and a held sample.
    fn iterate(&mut self, dc: bool, search: bool) -> Pass {
        let n = self.n;

        // Linearise where the solve is now.
        //
        // Re-centred every pass rather than inherited from the step that got
        // us here. A stamp taken during the last pass's trial had its devices
        // limited toward *that* pass's starting point, so it describes
        // somewhere slightly other than where the solve now is, and a
        // residual measured against it is not the residual of anything.
        // Carrying it forward to save the stamp cost the 73P forty-seven
        // thousand failed solves a second.
        self.point.copy_from_slice(&self.voltage);
        self.build(dc, !search);
        // A search needs a residual it can believe at both ends. If a junction
        // had to be held here, this one cannot be believed and the pass takes
        // the plain full step instead.
        let search = search && self.exact;
        let here = if search {
            self.merit(&self.voltage)
        } else {
            0.0
        };
        // A two-point nonmonotone search lets Newton cross a short increase
        // in residual instead of getting trapped taking tiny steps around a
        // power tube's conduction boundary. It must still improve on the
        // larger of this and the previous search residual. The reference
        // rolls forward, is reset every sample, and never changes the full
        // correction/device convergence test below. DC keeps its old search.
        let reference = if dc { here } else { here.max(self.search_merit) };
        if search {
            self.search_merit = here;
        }

        // Save the linearisation before the search disturbs it. See
        // `Linearisation`: a rejected trial must leave nothing behind.
        //
        // Only when the search can actually run. `linearisation` is a call
        // per device, and on the common path -- no stall, fewer than
        // `FULL_STEPS` passes -- the search never fires, so the save is pure
        // overhead. Gating it on `search` costs nothing on the passes that
        // need it and returns the whole cost on the passes that do not,
        // which is most of them: the catalogue's voices average two to three
        // and a half passes on real playing, and the search is only turned on
        // at eight or on the first pass that fails to shrink its correction.
        //
        // Safe because `relinearise` is only ever called from inside the
        // search block, and that block is only reached when `search` is true
        // here -- so `self.saved` is always populated on the passes that can
        // read it.
        if search {
            for (device, saved) in self.devices.iter().zip(self.saved.iter_mut()) {
                *saved = device.linearisation();
            }
        }

        // Solve the Newton linearisation. For a reducible nonlinear circuit,
        // every changing device stamp lives in the Schur boundary and the
        // passive internal unknowns are eliminated algebraically. This is the
        // same MNA system and yields the full voltage vector after recovery; it
        // merely avoids refactorising the passive nodes on every Newton pass.
        let reduced = if dc {
            if let Some(partition) = self.nonlinear_partition_dc.as_mut() {
                partition.solve_into(&self.work, &self.rhs, &mut self.guess)
            } else {
                false
            }
        } else if let Some(partition) = self.nonlinear_partition.as_mut() {
            partition.solve_into(&self.work, &self.rhs, &mut self.guess)
        } else {
            false
        };

        if !reduced {
            self.guess.copy_from_slice(&self.rhs);
            // Learn the pivot order on the first pass after a rebuild and replay
            // it afterwards. See `Simulation::plan`.
            let sound = factorise(
                &mut self.work,
                &mut self.pivots,
                &mut self.reach,
                &mut self.depth,
                &mut self.first,
                &self.reach_template,
                &self.depth_template,
                &mut self.plan,
                self.planned,
                n,
            );
            if !sound {
                self.replans += 1;
            }
            self.planned = sound;
            substitute(
                &self.work,
                &self.pivots,
                &self.reach,
                &self.first,
                &mut self.guess,
                n,
                &mut self.scratch,
            );
        }
        if !self.guess.iter().all(|v| v.is_finite()) {
            // A correction that is not a number cannot be shortened into one.
            self.nonfinite += 1;
            return Pass::Stuck;
        }

        // How far the solution wants to move, measured against the scale it is
        // moving *at*.
        //
        // This was an absolute figure: a millionth of a volt, everywhere. On a
        // grid sitting at a tenth of a volt that is a reasonable demand. On a
        // plate sitting at four hundred and fifty it asks for nine significant
        // digits, which is nine orders below anything the circuit does and
        // about six below what the arithmetic can even carry meaningfully --
        // so the solve kept iterating long after the answer had stopped
        // changing in any physical sense.
        //
        // The form is the one every circuit simulator uses: a relative part
        // for the nodes that are large and an absolute floor for the nodes
        // that are small. SPICE calls them RELTOL and VNTOL and ships defaults
        // of 1e-3 and 1e-6; this is a thousand times stricter on the relative
        // part than SPICE and keeps the same absolute floor, because the
        // absolute floor is what a small-signal node needs and there is no
        // reason to relax it.
        //
        // Measured on the *full* Newton correction, never on a shortened one.
        // The test asks whether the answer has stopped moving, and a step that
        // the line search deliberately made small is small for that reason and
        // not because the circuit has settled. Measuring the shortened step
        // instead let a solve declare itself converged at whatever value it
        // happened to be holding: damped to a sixty-fourth, the 73P reported
        // itself done after a single pass. See `R-013` in the regression log.
        let mut moved: f64 = 0.0;
        for k in 0..n {
            let step = (self.guess[k] - self.voltage[k]).abs();
            let scale = TOLERANCE + RELATIVE * self.voltage[k].abs();
            moved = moved.max(step / scale);
        }
        // Kept so the caller can see whether this pass made progress, and turn
        // the line search on the moment one does not. See `CONVERGING`.
        self.moved = moved;

        // Converged? Then stop here, before the line search, and take the
        // full step -- which is by definition a tiny one.
        //
        // This test has to come first, and getting that wrong is subtle. A
        // converged solve has a residual sitting at round-off, and a residual
        // at round-off cannot get any smaller: every trial the line search
        // offers is rejected for failing to improve on a number that is
        // already as small as arithmetic allows, and the search reports
        // failure at precisely the moment it has succeeded. Measured with the
        // test in the wrong place, the transformer core -- five unknowns, one
        // device, converging in two passes flat -- reported 7854 failed
        // solves a second.
        if moved < 1.0 && self.devices.iter().all(|d| d.settled(TOLERANCE)) {
            self.voltage.copy_from_slice(&self.guess);
            return Pass::Settled;
        }

        // Not there yet, so the step has to earn its place. The full one
        // first, and almost always the one taken: Newton converges
        // quadratically near the answer and a shortened step throws that
        // away, so the common path must not pay for the search at all -- one
        // trial, accepted, and done.
        if !search {
            self.voltage.copy_from_slice(&self.guess);
            return Pass::Moved;
        }

        let mut lambda = 1.0;
        let mut taken = false;
        let mut best_lambda = 1.0;
        let mut best_merit = f64::INFINITY;
        for _ in 0..self.backtracks {
            for k in 0..n {
                self.trial[k] = self.voltage[k] + lambda * (self.guess[k] - self.voltage[k]);
            }
            if self.trial.iter().all(|v| v.is_finite()) {
                // Re-linearise where the step lands and ask whether the
                // circuit is any closer to satisfying itself there.
                self.point.copy_from_slice(&self.trial);
                self.build(dc, false);
                if !self.exact {
                    // A junction was held where this step lands, so the
                    // residual there is somewhere else's. Stop searching and
                    // let the full step stand rather than decide on it.
                    break;
                }
                let there = self.merit(&self.trial);
                if there.is_finite() && there < best_merit {
                    best_merit = there;
                    best_lambda = lambda;
                }
                if there.is_finite() && there < reference {
                    taken = true;
                    break;
                }
            }
            // Refused. Put every device back where it was, so the next,
            // shorter trial is measured from the same place this one was.
            for (device, saved) in self.devices.iter_mut().zip(self.saved.iter()) {
                device.relinearise(*saved);
            }
            self.backtrack_count += 1;
            lambda *= 0.5;
            if lambda < MIN_LAMBDA {
                break;
            }
        }

        if !taken {
            // Nothing of any length improved the residual. That is not a
            // reason to stop: abandoning the solve here leaves it at a point
            // it had not finished with, and measured against an accurate
            // reference solve that was *worse* than the plain bounded Newton
            // this replaced -- the 5150 went from 48.8 dB below the reference
            // to 31.7 dB below it, which is a solver getting less right, not
            // more.
            //
            // Use the best finite trial that was actually measured, even
            // though it did not beat the reference. Unconditionally taking
            // the full jump here discarded useful search results and made
            // the 5150 oscillate through its iteration allowance on attacks.
            // If no finite exact merit was measured, retain the full step.
            self.fallbacks += 1;
            for (device, saved) in self.devices.iter_mut().zip(self.saved.iter()) {
                device.relinearise(*saved);
            }
            if dc || best_lambda == 1.0 {
                self.voltage.copy_from_slice(&self.guess);
            } else {
                for k in 0..n {
                    self.voltage[k] += best_lambda * (self.guess[k] - self.voltage[k]);
                }
            }
            return Pass::Moved;
        }

        self.voltage.copy_from_slice(&self.trial);
        Pass::Moved
    }

    /// One sample in, one out.
    pub fn process(&mut self, input: f64) -> f64 {
        self.solves += 1;
        // A control has moved. What that means depends on whether there is
        // anything to lose.
        //
        // With nothing in flight -- a circuit just built, just reset, or set
        // up before playback starts -- settle it where the control now puts
        // it. That is what makes a plugin start at its operating point instead
        // of climbing to it, and it is what every measurement harness here
        // relies on to mean "this circuit, at this setting".
        //
        // Mid-signal it must not. Hunting the operating point solves the
        // circuit *with no signal in it* and then sets every capacitor from
        // that answer, which is to say it deletes the audio in flight. Done
        // once a block for as long as a knob is turning -- see
        // `examples/knobmove.rs` and `tests/knobs.rs`.
        if self.dirty {
            self.rebuild();
            if self.at_rest {
                self.find_operating_point();
            }
            self.dirty = false;
        }
        self.at_rest = false;
        let n = self.n;

        // Whether this sample's solve failed to converge. It gates the
        // reactance advance at the end of this function; see the advance
        // block for why that gate is necessary.
        let mut failed = false;

        if self.devices.is_empty() {
            // Nothing bends, so the matrix from `rebuild` still stands and one
            // substitution is the whole solve.
            for k in 0..n {
                self.rhs[k] = self.source[k] * input + self.bias[k];
            }
            for c in &self.capacitors {
                inject(&mut self.rhs, c.a, c.b, c.history);
            }
            for l in &mut self.inductors {
                inject(&mut self.rhs, l.a, l.b, l.history);
            }
            let partitioned = if let Some(partition) = &self.linear_partition {
                partition.solve_into(
                    &self.rhs,
                    &mut self.voltage,
                    &mut self.scratch,
                    &mut self.guess,
                )
            } else {
                false
            };
            if !partitioned {
                substitute(
                    &self.matrix,
                    &self.pivots,
                    &self.reach,
                    &self.first,
                    &mut self.rhs,
                    n,
                    &mut self.scratch,
                );
                self.voltage.copy_from_slice(&self.rhs);
            }
        } else {
            self.prepare_rhs(input, false);
            // Take the recent movement before the predictor runs.
            //
            // `|V[n-1] - V[n-2]|` is the per-node scale the failure
            // branch's bound is measured in. Taken before the predictor
            // because the predictor overwrites `voltage` with the
            // extrapolation and shifts `earlier` up one -- after which
            // `|voltage - earlier|` is `|V_pred - V[n-1]|`, which happens
            // to equal the movement by accident of the extrapolation's
            // arithmetic and stops equalling it the moment the
            // extrapolation is skipped.
            for k in 0..n {
                self.recent_move[k] = (self.voltage[k] - self.earlier[k]).abs();
            }

            // Start from where the last two samples were heading, not from
            // where the last one was. Audio is smooth over a sample, so a
            // straight line through the last two lands much closer to the
            // answer than the last one does, and Newton then has less to do:
            // measured, the passes drop from 2.9 a sample to the floor of 2.
            //
            // Only where nothing in the circuit switches. Carried through an
            // op-amp deciding which rail it is against, the prediction put its
            // output at -90 V against a 3.5 V rail and left it there: two
            // passes is enough for a device that bends and not for one that
            // steps, and a settled wrong state still reports itself settled.
            //
            // And only when the previous sample converged. A failed solve's
            // answer is a bounded guess, and extending a bounded guess by
            // doubling its distance from the sample before it produces a
            // number that is further from the truth than the guess was. On
            // the sample after a failure the correct starting point is
            // `V[n-1]` directly -- the bounded guess the previous sample
            // already produced -- and letting Newton walk from there.
            if self.predictable && !self.last_was_unsettled {
                for k in 0..n {
                    let predicted = 2.0 * self.voltage[k] - self.earlier[k];
                    self.earlier[k] = self.voltage[k];
                    self.voltage[k] = predicted;
                }
            } else {
                self.earlier.copy_from_slice(&self.voltage);
            }
            // Save the starting point for the record; the failure branch
            // below uses `recent_move` for its scale, not this.
            self.predicted.copy_from_slice(&self.voltage);

            let mut settled = false;
            let ceiling = self.ceiling.clamp(PASS_FLOOR, MAX_ITERATIONS);
            // No history at the start of a sample: the first two passes are
            // plain whatever the last sample did.
            self.moved = f64::INFINITY;
            self.search_merit = 0.0;
            let mut before = f64::INFINITY;
            for pass in 0..ceiling {
                self.newton_passes += 1;
                // Plain Newton while it is converging, and the line search the
                // moment it is not.
                //
                // "Is not" is measured rather than counted: a pass that failed
                // to shrink the correction by `CONVERGING` was not approaching
                // the answer, and no number of further plain passes will
                // change that -- they will oscillate at the same price and
                // arrive at `FULL_STEPS` having learnt nothing. `FULL_STEPS`
                // stays as the backstop for a solve that shrinks a little
                // every pass and still gets nowhere.
                let stalled = self.moved > before * CONVERGING;
                before = self.moved;
                match self.iterate(false, stalled || pass >= FULL_STEPS) {
                    Pass::Settled => {
                        settled = true;
                        break;
                    }
                    Pass::Moved => {}
                    // Nothing helped, and running the same pass again would
                    // only find that out again at the same price.
                    Pass::Stuck => break,
                }
            }
            if !settled {
                self.unsettled += 1;
                if ceiling < MAX_ITERATIONS {
                    self.pinched += 1;
                }
                // Bound the failed iterate and use it, rather than throwing
                // it away.
                //
                // The failed iterate is the Newton loop's last answer, and
                // it is a linear response to the *current* input. It
                // therefore follows the signal; on a pick attack that is
                // exactly what is wanted, because the pick is a genuine
                // jump and the failed iterate's scale reflects it.
                //
                // What the iterate cannot be trusted for is *magnitude*: a
                // Newton solve that ran out of passes has no convergence
                // guarantee, and its answer can be a large overshoot on any
                // node. Keeping it unbounded is what produced the +2.3 dB
                // spike on the Twin before the reversion was added.
                //
                // So: keep the iterate, and cap how far any node may move
                // from the last settled value. The cap has to be
                // *signal-scaled*, because the nodes in these circuits do
                // not share a scale. A power tube's plate sits at four
                // hundred and fifty volts and swings by hundreds; a
                // speaker node sits at a few volts and swings by a few. One
                // absolute cap cannot admit the pick on the plate and
                // reject the click on the speaker at the same time.
                //
                // `recent_move[k]` is how much node `k` moved between the
                // two samples before this one, taken above before the
                // predictor overwrote anything. `STEP_MULTIPLE` times that
                // admits a continuation of the signal with room to overshoot
                // it somewhat, and rejects a Newton overshoot that is many
                // times the signal's own scale. `STEP_CEILING` is a
                // last-resort absolute cap so a numerical accident cannot
                // escape by way of a large recent move.
                //
                // STEP_MULTIPLE was 4 until a pick attack was reported as
                // "distorted and stuttering". A pick is a genuine jump --
                // the grid has to travel from cutoff to hard clipping in a
                // few samples -- and 4 times the previous sample's movement
                // is not enough of a jump. 16 admits the transient while
                // still bounding the answer: a real Newton overshoot is
                // many orders larger than sixteen times the signal, and the
                // absolute cap catches those.
                //
                // A non-predictable circuit -- one with an op-amp in it --
                // does not extrapolate, so `recent_move` is stale on it and
                // it takes `STEP_CEILING` alone. The 5150 and the Twin do
                // not have op-amps in the signal path, so this branch does
                // not fire on them; it is here for the pedal voices.
                const STEP_MULTIPLE: f64 = 16.0;
                const STEP_CEILING: f64 = 1000.0;
                for k in 0..n {
                    let base = self.earlier[k];
                    let iter = self.voltage[k] - base;
                    let bound = if self.predictable && !self.last_was_unsettled {
                        (self.recent_move[k] * STEP_MULTIPLE).min(STEP_CEILING)
                    } else {
                        STEP_CEILING
                    };
                    if iter.abs() > bound {
                        self.voltage[k] = base + iter.signum() * bound;
                    }
                }
                failed = true;
                self.last_was_unsettled = true;
            } else {
                self.last_was_unsettled = false;
            }
        }

        // Advance what each reactance remembers -- but only when this sample
        // actually converged.
        //
        // The advance is what changes the state: a capacitor's stored charge,
        // an inductor's current, the transformer core's flux. It has to
        // integrate the sample's *answer*, and a failed sample's answer is
        // not the answer -- it is a bounded guess. Advancing from a bounded
        // guess lets the state move by up to `STEP_MULTIPLE` times the
        // signal's own per-sample movement, and doing that for a run of
        // consecutive failed samples walks the state far enough from any
        // plausible operating point that the circuit stops returning to it.
        // On the Twin Reverb, whose power stage is the largest solver in the
        // catalogue and the one whose transformer core integrates the most,
        // that is what produced a loud pulsating that continued after the
        // input stopped: the core had walked into a limit cycle of its own.
        //
        // So on a failed sample the state is left exactly where the last
        // converged sample left it, and only the *output* -- which reads
        // `voltage`, not the state -- follows the bounded failed iterate.
        // That is what lets the sound follow the pick even while the solver
        // is struggling: the sample goes out, the state does not commit, and
        // the next sample's solve starts from the same operating point the
        // last one trusted.
        //
        // The convention throughout is that a branch's current leaving node
        // `a` is `G * v - history`, so `history` is exactly what gets injected
        // into the right hand side. Both companions are written to that shape
        // so one `inject` serves them, and getting the shape wrong is easy: a
        // capacitor whose recurrence carries the wrong term reads 24 dB down
        // on a network that should be flat, and an inductor's diverges to NaN
        // within a few samples.
        if !failed {
            for c in &mut self.capacitors {
                let v = across(&self.voltage, c.a, c.b);
                // Trapezoidal: i = 2C/T (v - v_prev) - i_prev, which rearranges to
                // i = G v - (G v_prev + i_prev).
                let current = c.conductance * v - c.history;
                c.history = c.conductance * v + current;
                c.voltage = v;
            }
            for l in &mut self.inductors {
                let v = across(&self.voltage, l.a, l.b);
                // i = i_prev + T/2L (v + v_prev) = G v + (i_prev + G v_prev), so
                // the injected term is the negative of that bracket.
                let current = l.conductance * v - l.history;
                l.history = -(current + l.conductance * v);
                l.current = current;
            }
            // And what each device remembers. Most of them remember nothing --
            // a diode's current depends on its voltage now and on nothing
            // else -- so this went uncalled for a long time without any of
            // them noticing. A saturating core is the first device here with a
            // state, and it integrates the voltage across it: left unadvanced
            // its flux never accumulated past a single sample, so it drew a
            // hundred-thousandth of the current it should have and iron in the
            // signal path did nothing whatsoever.
            for device in &mut self.devices {
                device.advance();
            }
        }

        self.voltage[self.circuit.output]
    }

    /// Clears every dynamic state and marks the circuit as resting, but does
    /// not hunt the operating point yet.
    ///
    /// This is the realtime-safe reset primitive for a caller that is about to
    /// apply more controls. `Chain::apply` can switch a voice and then move its
    /// Drive, Master, EQ and other controls in the same host block. Settling
    /// inside the reset would solve an old/intermediate setting and force a
    /// second DC hunt after the final controls were installed.
    ///
    /// The simulation is therefore left `dirty && at_rest`. The caller can
    /// apply every pending control, then call `find_operating_point()` once
    /// (and share that DC solution with identical stereo channels). `process()`
    /// retains the safe fallback: if a caller does not prepare it explicitly,
    /// the first sample rebuilds and settles before audio is advanced.
    ///
    /// Marking the circuit dirty also resets stateful nonlinear devices
    /// correctly. A transformer core owns flux history that `Device::advance()`
    /// commits; advancing it is not a reset. The next rebuild occurs with
    /// `at_rest == true`, so `rebuild()` reconstructs the device list from its
    /// pristine constructors without allocating, using the capacity reserved
    /// in `new`.
    pub fn reset_deferred(&mut self) {
        self.at_rest = true;

        for c in &mut self.capacitors {
            c.history = 0.0;
            c.voltage = 0.0;
        }
        for l in &mut self.inductors {
            l.history = 0.0;
            l.current = 0.0;
        }

        for value in &mut self.voltage {
            *value = 0.0;
        }
        for value in &mut self.predicted {
            *value = 0.0;
        }
        for value in &mut self.earlier {
            *value = 0.0;
        }
        for value in &mut self.recent_move {
            *value = 0.0;
        }
        for value in &mut self.point {
            *value = 0.0;
        }
        for value in &mut self.trial {
            *value = 0.0;
        }
        for value in &mut self.guess {
            *value = 0.0;
        }
        self.last_was_unsettled = false;
        self.exact = true;
        self.moved = f64::INFINITY;

        // Force one rebuild after the caller has finished applying the new
        // setting. With `at_rest` true that rebuild also gives stateful devices
        // a genuine fresh state. Do not rebuild or DC-hunt here.
        self.dirty = true;
    }

    /// Reset and establish the operating point immediately.
    ///
    /// This preserves the original public `reset()` contract for preparation,
    /// tests and any caller that explicitly wants a fully settled simulation
    /// before returning. Realtime voice/model switching uses `reset_deferred`
    /// so it can apply the complete new setting before paying for this hunt.
    pub fn reset(&mut self) {
        self.reset_deferred();
        self.find_operating_point();
    }
}

/// An ideal transformer's rows: the primary is `ratio` times the secondary in
/// volts, and one over that in amps.
#[allow(clippy::too_many_arguments)]
fn stamp_branch(
    m: &mut [f64],
    n: usize,
    branch: usize,
    p1: usize,
    p2: usize,
    s1: usize,
    s2: usize,
    ratio: f64,
) {
    for (node, sign) in [(p1, 1.0), (p2, -1.0), (s1, -ratio), (s2, ratio)] {
        if node != GROUND {
            m[node * n + branch] += sign;
            m[branch * n + node] += sign;
        }
    }
}

/// Adds a conductance between two nodes.
fn stamp_matrix(m: &mut [f64], n: usize, a: usize, b: usize, g: f64) {
    if a != GROUND {
        m[a * n + a] += g;
    }
    if b != GROUND {
        m[b * n + b] += g;
    }
    if a != GROUND && b != GROUND {
        m[a * n + b] -= g;
        m[b * n + a] -= g;
    }
}

/// Into both the running matrix and the one the operating point is solved on.
/// Only the reactances differ between them.
fn stamp_both(base: &mut [f64], dc: &mut [f64], n: usize, a: usize, b: usize, g: f64) {
    stamp_matrix(base, n, a, b, g);
    stamp_matrix(dc, n, a, b, g);
}

fn across(v: &[f64], a: usize, b: usize) -> f64 {
    let va = if a == GROUND { 0.0 } else { v[a] };
    let vb = if b == GROUND { 0.0 } else { v[b] };
    va - vb
}

fn inject(rhs: &mut [f64], a: usize, b: usize, current: f64) {
    if a != GROUND {
        rhs[a] += current;
    }
    if b != GROUND {
        rhs[b] -= current;
    }
}

/// LU with partial pivoting, in place.
///
/// Pivoting is not optional here. The diagonal carries zeros wherever a node's
/// own admittance cancels, and an unpivoted elimination divides by whatever
/// happens to be sitting there.
#[allow(clippy::too_many_arguments)]
/// The largest entry at or below the diagonal in `col`, recorded in `plan`.
fn find_pivot(m: &[f64], plan: &mut [usize], col: usize, bottom: usize, n: usize) -> usize {
    let mut best = col;
    let mut magnitude = m[col * n + col].abs();
    for row in (col + 1)..=bottom {
        let candidate = m[row * n + col].abs();
        if candidate > magnitude {
            best = row;
            magnitude = candidate;
        }
    }
    plan[col] = best;
    best
}

/// Exchange two rows, and the per-row bookkeeping that travels with them.
fn swap_rows(
    m: &mut [f64],
    pivots: &mut [usize],
    reach: &mut [usize],
    first: &mut [usize],
    col: usize,
    best: usize,
    n: usize,
) {
    let width = reach[col].max(reach[best]).saturating_add(1).min(n);
    for k in 0..width {
        m.swap(col * n + k, best * n + k);
    }
    pivots.swap(col, best);
    reach.swap(col, best);
    first.swap(col, best);
}

/// Whether exact nonlinear condensation is likely to beat the already sparse
/// full LU. Tiny circuits are deliberately left alone: forming/recovering a
/// Schur system has fixed O(n²) work and is meant for the large valve models,
/// not a two-diode pedal. This is only a performance gate; either path solves
/// the same equations.
fn nonlinear_reduction_worthwhile(n: usize, boundary: usize) -> bool {
    if n < 16 || boundary == 0 || boundary >= n {
        return false;
    }
    let internal = n - boundary;
    if internal < 4 {
        return false;
    }

    let n = n as f64;
    let b = boundary as f64;
    let i = internal as f64;
    // Reduced boundary LU + full internal recovery + RHS reduction/coupling.
    // Keep a conservative margin because the original LU exploits sparsity.
    let full = n * n * n;
    let reduced = b * b * b + i * i + 2.0 * b * i + b * b;
    reduced < 0.70 * full
}

fn factorise(
    m: &mut [f64],
    pivots: &mut [usize],
    reach: &mut [usize],
    depth: &mut [usize],
    first: &mut [usize],
    reach_template: &[usize],
    depth_template: &[usize],
    plan: &mut [usize],
    planned: bool,
    n: usize,
) -> bool {
    // How far right each row goes, and how far down each column does.
    //
    // A circuit matrix is nearly all zeros -- a part only ever stamps the nodes
    // it is connected to, so a row typically holds three or four entries out of
    // thirty -- and the elimination's inner loop was running the full width of
    // the matrix regardless, multiplying by zero for most of it. So each row
    // carries the column index of its last nonzero and the inner loop stops
    // there; each column carries the row index of its last, and the pivot
    // search stops there.
    //
    // Both come from the netlist, not from the values. They used to be
    // rediscovered on every call by scanning: the row scan walked in from the
    // right, which on a banded matrix means reading the whole tail of zeros to
    // find an entry sitting near the diagonal, and there was no column bound at
    // all, so the pivot search read every row below the diagonal. Measured on
    // the Boogie preamplifier that was 325 strided column reads and about 230
    // row reads a factorisation, against 59 actual eliminations -- the search
    // for the shape cost four times the arithmetic done on it.
    //
    // The shape is fixed when the parts are wired together. `map_structure`
    // works it out once. See `reach_template`.
    reach.copy_from_slice(reach_template);
    depth.copy_from_slice(depth_template);

    for (k, p) in pivots.iter_mut().enumerate() {
        *p = k;
    }
    // Where each row's L part starts. A row with nothing below the diagonal
    // keeps its own index, which makes the forward substitution's loop empty.
    for (k, f) in first.iter_mut().enumerate() {
        *f = k;
    }
    // Whether the order being replayed is still the order a search would have
    // chosen, and whether this call has had to give up on it partway.
    //
    // Giving up partway is exact, and that is the point. The elimination is
    // sequential: at column `col` every column to its left is finished, and
    // each of those was confirmed to have used the largest entry available.
    // So the matrix here is precisely what a searching factorisation would
    // have produced, and switching to a search from this column on completes a
    // proper partial-pivoted LU. No copy of the matrix is needed and nothing
    // has to be done twice -- which matters, because keeping a copy to retry
    // from cost more than the search it was there to avoid: the 5150's power
    // stage went from 64.9 % of realtime to 76.5 %.
    let mut sound = true;
    let mut searching = !planned;
    for col in 0..n {
        // Nothing below this column's last possible entry can be the pivot.
        let bottom = depth[col];
        let mut best = if searching {
            find_pivot(m, plan, col, bottom, n)
        } else {
            // Replayed. Whether it was the right choice is checked twice
            // below: for a pivot that has gone to nothing, before anything is
            // eliminated with it, and for a pivot that is merely no longer the
            // largest, out of loads the elimination performs anyway.
            plan[col]
        };
        if best != col {
            swap_rows(m, pivots, reach, first, col, best, n);
        }
        // The swap may have carried the old pivot row's entries down to row
        // `best`, so every column that row reaches into can now hold a nonzero
        // that far down. `best <= depth[col]`, so bounding the widening by
        // this column's own depth below covers it.

        let mut pivot = m[col * n + col];
        if !searching && pivot.abs() < 1e-30 {
            // The plan named a row that no longer holds anything here.
            //
            // This is the check that was missing, and leaving it out did not
            // fail quietly by a decibel: the `continue` below steps over the
            // whole elimination of this column, so the factorisation is of a
            // different matrix from the one that was stamped, and the
            // Distortion voice fell to 162 dB below the rest of the catalogue
            // -- silence. `tests/presets.rs::the_presets_are_level_matched`
            // is what caught it.
            //
            // The other guard cannot catch this, because it lives inside the
            // elimination loop this skips.
            searching = true;
            sound = false;
            best = find_pivot(m, plan, col, bottom, n);
            if best != col {
                swap_rows(m, pivots, reach, first, col, best, n);
            }
            pivot = m[col * n + col];
        }
        if pivot.abs() < 1e-30 {
            // Genuinely nothing to eliminate with: the matrix is singular in
            // this column.
            continue;
        }
        // Nothing to the right of the pivot row's reach can be changed by it,
        // and nothing below this column's own last entry has anything in this
        // column to eliminate. The second half of that was missing: the loop
        // ran to `n` and read `m[row * n + col]` for every row below the
        // diagonal just to find a zero and skip it -- and that read is
        // strided across rows, so on a twenty-eight unknown circuit it was
        // about four hundred loads a factorisation, each on its own cache
        // line. Bounding it costs nothing and skips only entries that were
        // zero, so the arithmetic is unchanged.
        let stop = reach[col];
        let ceiling = pivot.abs();
        for row in (col + 1)..=bottom {
            // Tested before the division, not after it.
            //
            // Most rows have nothing below the pivot at all, and eliminating
            // with a zero multiplier subtracts nothing from every remaining
            // column -- but the old form divided first and asked afterwards,
            // so a thirty unknown circuit paid about nine hundred divisions a
            // factorisation to compute `0.0 / pivot`, store it back over the
            // zero it came from, and then skip. A division is twenty odd
            // cycles where the loads either side are one, so those were a
            // sizeable share of the factorisation on their own.
            let entry = m[row * n + col];
            if entry == 0.0 {
                continue;
            }
            // Partial pivoting's own test, applied to a load already made: was
            // the pivot the largest in the column? If a replayed order says no,
            // the factorisation stands -- it is still an exact LU of this
            // matrix, only a less well conditioned one -- and the caller
            // searches again next time.
            if !searching && entry.abs() > ceiling * GROWTH {
                // Not the largest in the column any more. Unlike the check
                // above this is not a correctness problem -- the result is
                // still an exact LU of this matrix, only a less well
                // conditioned one -- so the pass stands and the next
                // factorisation searches and learns the order again.
                sound = false;
            }
            let factor = entry / pivot;
            m[row * n + col] = factor;
            // Where this row's L part starts, for the forward substitution.
            if col < first[row] {
                first[row] = col;
            }
            for k in (col + 1)..=stop {
                m[row * n + k] -= factor * m[col * n + k];
            }
            // The fill-in this just created reaches as far as the pivot row did.
            if stop > reach[row] {
                reach[row] = stop;
            }
        }
        // And it goes as far down as this column's own entries did, which is
        // what keeps the pivot search's bound honest for the columns to come.
        for k in (col + 1)..=stop {
            if depth[k] < bottom {
                depth[k] = bottom;
            }
        }
    }
    sound
}

/// Solves with the factorisation `factorise` left behind.
///
/// Both triangles are walked between the bounds that function recorded rather
/// than across the full width: `first[row]` is where the row's L part starts
/// and `reach[row]` is where its U part ends, fill-in included. Everything
/// outside them is an exact zero, so skipping it subtracts nothing and the
/// arithmetic is what it was. It is worth doing because a substitution is
/// otherwise two full triangles -- about nine hundred multiply-adds on a
/// thirty unknown circuit, of which a circuit matrix means perhaps a tenth
/// are not multiplying by zero.
fn substitute(
    m: &[f64],
    pivots: &[usize],
    reach: &[usize],
    first: &[usize],
    rhs: &mut [f64],
    n: usize,
    permuted: &mut [f64],
) {
    for (row, &from) in pivots.iter().enumerate() {
        permuted[row] = rhs[from];
    }
    for row in 1..n {
        let mut sum = permuted[row];
        for k in first[row]..row {
            sum -= m[row * n + k] * permuted[k];
        }
        permuted[row] = sum;
    }
    for row in (0..n).rev() {
        let mut sum = permuted[row];
        for k in (row + 1)..=reach[row] {
            sum -= m[row * n + k] * permuted[k];
        }
        let pivot = m[row * n + row];
        permuted[row] = if pivot.abs() < 1e-30 {
            0.0
        } else {
            sum / pivot
        };
    }
    rhs.copy_from_slice(&permuted[..n]);
}
