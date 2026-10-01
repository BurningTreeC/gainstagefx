//! The Oregon T (Sunn Model T, D-1029 A) against its own voltage chart.
//!
//! Sunn printed the chart's conditions on the drawing: 1 kHz into BOTH IN,
//! both volumes, bass, middle, treble and the master at 10, presence at 10, a
//! 4 ohm resistive load, and the input set for 24.5 V rms across it -- 150 W.
//! The stage is built at its 16 ohm tap, where the same 150 W is 49 V rms; the
//! readings below are referred to the chart's 4 ohm.
//! This prints the model's readings beside the chart's, no signal and at that
//! level, and the plate supply's two constants that put node A on the chart
//! (`power::OREGON_A_OPEN` and `power::OREGON_A_SOURCE`).
//!
//! `cargo run --release --example oregon_t_op`

use gainstagefx::circuits::oregon_t::{self, Input};
use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::netlist::Circuit;
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
const SOURCE: f64 = 10_000.0;

fn preamp() -> Circuit {
    oregon_t::tap_with(Input::Both, SOURCE, 500_000.0, "out").unwrap()
}

/// Every control at 10, as the chart was taken.
fn all_up(pre: &mut Simulation, pow: &mut Simulation) {
    for control in [
        oregon_t::VOLUME,
        oregon_t::NORMAL_VOLUME,
        oregon_t::TREBLE,
        oregon_t::BASS,
        oregon_t::MIDDLE,
        oregon_t::MASTER,
    ] {
        pre.set_control(control, 1.0);
    }
    pow.set_control(power::PRESENCE, 1.0);
}

struct Reading {
    rms: f64,
    a: f64,
    plate: f64,
    screen: f64,
    grid: f64,
    pi_p1: f64,
    pi_p2: f64,
    pi_k: f64,
    supply_amps: f64,
}

/// Runs the whole amplifier at `volts` peak of 1 kHz into BOTH IN -- or,
/// with `whole` false, the power stage alone with the sine at its input --
/// and averages the chart's nodes over the last half second.
fn run_with(spec: &PowerSpec, volts: f64, whole: bool) -> Reading {
    let pc = power::build(spec, SOURCE).unwrap();
    let at = |n: &str| pc.unknown_named(n).unwrap();
    let (ht, pl, os, og, p1, p2, k) = (
        at("ht"),
        at("ov1"),
        at("os1"),
        at("og1"),
        at("pi_p1"),
        at("pi_p2"),
        at("pi_k"),
    );
    let mut pre = Simulation::new(preamp(), RATE);
    let mut pow = Simulation::new(pc, RATE);
    all_up(&mut pre, &mut pow);
    pre.find_operating_point();
    pow.find_operating_point();
    let total = (RATE * 1.5) as usize;
    let window = (RATE * 0.5) as usize;
    let mut sums = [0.0f64; 8];
    for n in 0..total {
        let x = volts * (std::f64::consts::TAU * 1_000.0 * n as f64 / RATE).sin();
        let y = if whole {
            pow.process(pre.process(x))
        } else {
            pow.process(x)
        };
        if n >= total - window {
            let v = |i| pow.voltage_at(i);
            for (s, value) in
                sums.iter_mut()
                    .zip([y * y, v(ht), v(pl), v(os), v(og), v(p1), v(p2), v(k)])
            {
                *s += value;
            }
        }
    }
    let w = window as f64;
    let a = sums[1] / w;
    Reading {
        // At the 16 ohm tap; referred to the chart's 4 ohm.
        rms: (sums[0] / w).sqrt() / 2.0,
        a,
        plate: sums[2] / w,
        screen: sums[3] / w,
        grid: sums[4] / w,
        pi_p1: sums[5] / w,
        pi_p2: sums[6] / w,
        pi_k: sums[7] / w,
        supply_amps: (spec.plate_supply - a) / spec.supply_resistance,
    }
}

fn run(spec: &PowerSpec, volts: f64) -> Reading {
    run_with(spec, volts, true)
}

/// The 150 W column: the power stage driven by a sine at its own input to
/// 24.5 V rms across 4 ohm (referred), by bisection. The chart's column
/// describes the power stage delivering 150 W. Driven from the preamplifier
/// with every control at 10 the model gets there too (`whole_peak`), but only
/// at the top of a narrow window -- a little more input and the inverter's
/// grids charge their coupling capacitor and the output falls back -- which is
/// no place to bisect; the stage alone rises monotonically to its maximum.
fn at_150_watts(spec: &PowerSpec) -> (f64, Reading) {
    let (mut lo, mut hi) = (4.0f64, 24.0f64);
    let mut best = (hi, run_with(spec, hi, false));
    for _ in 0..12 {
        let mid = 0.5 * (lo + hi);
        let r = run_with(spec, mid, false);
        if r.rms < 24.5 {
            lo = mid;
        } else {
            hi = mid;
            best = (mid, r);
        }
    }
    best
}

/// The whole amplifier's largest output from BOTH IN at the chart's settings,
/// scanning the input upwards: past it the inverter's grids charge their
/// coupling capacitor and the output falls back.
fn whole_peak(spec: &PowerSpec) -> (f64, f64) {
    let mut best = (0.0, 0.0);
    let mut volts = 5e-3;
    while volts < 0.2 {
        let r = run(spec, volts);
        if r.rms > best.1 {
            best = (volts, r.rms);
        }
        volts *= 1.2;
    }
    best
}

/// The power stage alone, a 1 kHz sine at its input: output rms and the
/// inverter's average plates, so where it stops climbing can be seen apart
/// from the preamplifier.
fn power_alone(spec: &PowerSpec, volts: f64) -> (f64, f64, f64, f64) {
    let pc = power::build(spec, SOURCE).unwrap();
    let (p1, p2, ht) = (
        pc.unknown_named("pi_p1").unwrap(),
        pc.unknown_named("pi_p2").unwrap(),
        pc.unknown_named("ht").unwrap(),
    );
    let mut pow = Simulation::new(pc, RATE);
    pow.set_control(power::PRESENCE, 1.0);
    pow.find_operating_point();
    let total = (RATE * 1.0) as usize;
    let window = (RATE * 0.4) as usize;
    let mut s = [0.0f64; 4];
    for n in 0..total {
        let x = volts * (std::f64::consts::TAU * 1_000.0 * n as f64 / RATE).sin();
        let y = pow.process(x);
        if n >= total - window {
            s[0] += y * y;
            s[1] += pow.voltage_at(p1);
            s[2] += pow.voltage_at(p2);
            s[3] += pow.voltage_at(ht);
        }
    }
    let w = window as f64;
    ((s[0] / w).sqrt(), s[1] / w, s[2] / w, s[3] / w)
}

fn main() {
    if std::env::args().any(|a| a == "--map") {
        // Where the heavy-overdrive solve fails: presence against drive, at
        // two rates.
        let spec = PowerSpec::OREGON_6550;
        for rate in [48_000.0, 96_000.0] {
            println!("at {rate} Hz, unsettled solves in 0.3 s (rows presence, columns mV peak):");
            let levels = [0.02, 0.05, 0.1, 0.3, 1.0];
            print!("  {:>8}", "");
            for l in levels {
                print!(" {:>7.0}", l * 1e3);
            }
            println!();
            for presence in [0.0, 0.5, 0.75, 0.9, 1.0] {
                print!("  {presence:>8.2}");
                for level in levels {
                    let pc = power::build(&spec, SOURCE).unwrap();
                    let mut pre = Simulation::new(preamp(), rate);
                    let mut pow = Simulation::new(pc, rate);
                    all_up(&mut pre, &mut pow);
                    pow.set_control(power::PRESENCE, presence);
                    pre.find_operating_point();
                    pow.find_operating_point();
                    for n in 0..(rate * 0.3) as usize {
                        let x = level * (std::f64::consts::TAU * 1_000.0 * n as f64 / rate).sin();
                        pow.process(pre.process(x));
                    }
                    print!(" {:>7}", pow.statistics().2);
                }
                println!();
            }
        }
        return;
    }
    if std::env::args().any(|a| a == "--variants") {
        // Which part of the stage the heavy-overdrive solve fails on.
        let base = PowerSpec::OREGON_6550;
        let variants = [
            ("as built", base),
            (
                "screens on a stiff supply, no tap",
                PowerSpec {
                    screen_tap: 0.0,
                    screen_supply: 505.0,
                    screen_resistance: 50.0,
                    screen_reservoir: 50e-6,
                    ..base
                },
            ),
            (
                "no plate resistors",
                PowerSpec {
                    plate_resistor: 0.0,
                    ..base
                },
            ),
            (
                "tap 20 %",
                PowerSpec {
                    screen_tap: 0.2,
                    ..base
                },
            ),
            (
                "EL34s",
                PowerSpec {
                    tube: gainstagefx::dsp::netlist::PentodeSpec::EL34,
                    ..base
                },
            ),
            (
                "no loop",
                PowerSpec {
                    feedback: 0.0,
                    ..base
                },
            ),
            (
                "half the loop (22 k)",
                PowerSpec {
                    feedback: 22_000.0,
                    ..base
                },
            ),
            (
                "6550 with the EL34's knee (kvb 24)",
                PowerSpec {
                    tube: gainstagefx::dsp::netlist::PentodeSpec {
                        kvb: 24.0,
                        ..gainstagefx::dsp::netlist::PentodeSpec::T6550
                    },
                    ..base
                },
            ),
            ("no presence (0)", base),
        ];
        for (name, spec) in variants {
            let pc = power::build(&spec, SOURCE).unwrap();
            let mut pre = Simulation::new(preamp(), RATE);
            let mut pow = Simulation::new(pc, RATE);
            all_up(&mut pre, &mut pow);
            if name.starts_with("no presence") {
                pow.set_control(power::PRESENCE, 0.0);
            }
            pre.find_operating_point();
            pow.find_operating_point();
            let mut sum2 = 0.0;
            let n_total = (RATE * 0.3) as usize;
            for n in 0..n_total {
                let x = 0.1 * (std::f64::consts::TAU * 1_000.0 * n as f64 / RATE).sin();
                let y = pow.process(pre.process(x));
                sum2 += y * y;
            }
            let (_, passes, unsettled, _) = pow.statistics();
            println!(
                "  {name:<36} unsettled {unsettled:>6} of {n_total}, passes {passes}, rms {:.1}",
                (sum2 / n_total as f64).sqrt()
            );
            if std::env::args().any(|a| a == "--nodes") {
                let c = power::build(&spec, SOURCE).unwrap();
                for n in [
                    "in", "pi_a", "pi_p1", "pi_p2", "pi_k", "tail", "on1", "on2", "og1", "og2",
                    "ht", "pl_a", "pl_b", "ov1", "ov2", "tap_a", "tap_b", "os1", "os2", "sec",
                    "spk", "pres",
                ] {
                    if let Some(i) = c.unknown_named(n) {
                        print!(" {n}={:.1}", pow.voltage_at(i));
                    }
                }
                println!();
            }
        }
        return;
    }
    if std::env::args().any(|a| a == "--spectrum") {
        // Where the energy is at a heavy overdrive: on the 1 kHz harmonics, or
        // somewhere else.
        let spec = PowerSpec::OREGON_6550;
        for volts in [0.02, 0.05, 0.1, 0.3] {
            let pc = power::build(&spec, SOURCE).unwrap();
            let mut pre = Simulation::new(preamp(), RATE);
            let mut pow = Simulation::new(pc, RATE);
            all_up(&mut pre, &mut pow);
            pre.find_operating_point();
            pow.find_operating_point();
            let total = (RATE * 1.0) as usize;
            let window = 9_600; // 100 ms, a whole number of 1 kHz cycles
            let mut ys = Vec::with_capacity(window);
            let mut pre_out = Vec::with_capacity(window);
            let block = (RATE * 0.05) as usize;
            let mut course = String::new();
            let mut acc = 0.0;
            for n in 0..total {
                let x = volts * (std::f64::consts::TAU * 1_000.0 * n as f64 / RATE).sin();
                let p = pre.process(x);
                let y = pow.process(p);
                acc += y;
                if (n + 1) % block == 0 {
                    let (_, _, unsettled, _) = pow.statistics();
                    course.push_str(&format!(" {:.1}/{unsettled}", acc / block as f64));
                    acc = 0.0;
                }
                if n >= total - window {
                    ys.push(y);
                    pre_out.push(p);
                }
            }
            println!("    mean output and unsettled solves by 50 ms:{course}");
            println!(
                "    power health (backtracks, fallbacks, nonfinite) {:?}",
                pow.health()
            );
            let total_e: f64 = ys.iter().map(|y| y * y).sum::<f64>() / window as f64;
            let mut harm = 0.0;
            for h in 1..=48 {
                let w = std::f64::consts::TAU * 1_000.0 * h as f64 / RATE;
                let (mut re, mut im) = (0.0, 0.0);
                for (n, y) in ys.iter().enumerate() {
                    re += y * (w * n as f64).cos();
                    im += y * (w * n as f64).sin();
                }
                let a = 2.0 * (re * re + im * im).sqrt() / window as f64;
                harm += a * a / 2.0;
            }
            let mean = ys.iter().sum::<f64>() / window as f64;
            let pmax = pre_out.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            let ymax = ys.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            println!(
                "  {:>6.1} mV: total {:.2} V rms, on 1 kHz harmonics {:.2} V rms, dc {:.2} V, peak out {:.1} V, preamp peak {:.1} V",
                volts * 1e3,
                total_e.sqrt(),
                harm.sqrt(),
                mean,
                ymax,
                pmax
            );
        }
        return;
    }
    if std::env::args().any(|a| a == "--sweep") {
        let spec = PowerSpec::OREGON_6550;
        println!("power stage alone, 1 kHz sine at its input:");
        for volts in [1.0, 2.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0] {
            let (rms, p1, p2, a) = power_alone(&spec, volts);
            let rms = rms / 2.0; // the 16 ohm tap, referred to 4 ohm
            println!(
                "  {volts:>5.1} V peak: {rms:6.2} V rms ({:5.1} W), PI plates {p1:5.1} / {p2:5.1}, A {a:5.1}",
                rms * rms / 4.0
            );
        }
        println!("whole amplifier from BOTH IN, every control at 10:");
        for volts in [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.3] {
            let r = run(&spec, volts);
            println!(
                "  {:>6.1} mV peak: {:6.2} V rms ({:5.1} W), PI plates {:5.1} / {:5.1}, A {:5.1}",
                volts * 1e3,
                r.rms,
                r.rms * r.rms / 4.0,
                r.pi_p1,
                r.pi_p2,
                r.a
            );
        }
        return;
    }
    // --- the preamplifier, no signal --------------------------------------------
    let c = preamp();
    let names = ["c", "v1a_p", "v1b_p", "v1_k", "v2_p", "v2_k", "cf"];
    let idx: Vec<_> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut sim = Simulation::new(c, RATE);
    sim.find_operating_point();
    let chart = [351.0, 227.0, 232.0, 1.71, 203.0, 1.3, 205.0];
    println!("preamplifier, no signal       model    chart");
    for ((n, i), want) in names.iter().zip(idx).zip(chart) {
        println!("  {n:<8} {:>17.2} {want:>8.2}", sim.voltage_at(i));
    }

    // --- the power stage, no signal ---------------------------------------------
    let spec = PowerSpec::OREGON_6550;
    let idle = run(&spec, 0.0);
    println!("\npower stage, no signal        model    chart");
    println!("  A        {:>17.1} {:>8}", idle.a, 519);
    println!("  plate    {:>17.1} {:>8}", idle.plate, 507);
    println!("  screen   {:>17.1} {:>8}", idle.screen, 504);
    println!("  grid     {:>17.1} {:>8}", idle.grid, -53);
    println!("  PI plate {:>17.1} {:>8}", idle.pi_p1, 271);
    println!("  PI other {:>17.1} {:>8}", idle.pi_p2, 251);
    println!("  PI k     {:>17.1} {:>8}", idle.pi_k, 38);
    println!(
        "  supply   {:>15.0} mA, {:.1} mA a valve",
        idle.supply_amps * 1e3,
        idle.supply_amps * 1e3 / 4.0
    );

    // --- 150 W ----------------------------------------------------------------------
    let (peak_in, peak_out) = whole_peak(&spec);
    println!(
        "\nwhole amplifier from BOTH IN, every control at 10: largest output {:.2} V rms \
         into 4 ohm ({:.0} W) at {:.1} mV peak; the drawing rates 150 W",
        peak_out,
        peak_out * peak_out / 4.0,
        peak_in * 1e3
    );
    let (volts, full) = at_150_watts(&spec);
    println!(
        "\npower stage at 24.5 V rms into 4 ohm ({:.2} V peak of sine at its input)",
        volts
    );
    println!("                               model    chart");
    println!("  output   {:>15.2} rms {:>8}", full.rms, 24.5);
    println!("  A        {:>17.1} {:>8}", full.a, 481);
    println!("  plate    {:>17.1} {:>8}", full.plate, 472);
    println!("  screen   {:>17.1} {:>8}", full.screen, 459);
    println!("  grid     {:>17.1} {:>8}", full.grid, -55);
    println!("  PI plate {:>17.1} {:>8}", full.pi_p1, 271);
    println!("  PI other {:>17.1} {:>8}", full.pi_p2, 251);
    println!("  PI k     {:>17.1} {:>8}", full.pi_k, 30);
    println!("  supply   {:>15.0} mA", full.supply_amps * 1e3);

    // The two constants that put A on the chart at both readings, with these
    // valves drawing what they drew: V0 - R I0 = 519, V0 - R I1 = 481.
    let (i0, i1) = (idle.supply_amps, full.supply_amps);
    let r = (519.0 - 481.0) / (i1 - i0);
    let v0 = 519.0 + r * i0;
    println!(
        "\nfitted supply: OREGON_A_OPEN {v0:.1} V, OREGON_A_SOURCE {r:.1} ohm \
         (now {:.1} V, {:.1} ohm)",
        power::OREGON_A_OPEN,
        power::OREGON_A_SOURCE
    );
}
