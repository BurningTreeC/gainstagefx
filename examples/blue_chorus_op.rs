//! The Blue Chorus (Boss CE-2) against its service notes and the measurements
//! of it, and the fit of its clock law.
//!
//! The oscillator's period across RATE (0.32 to 3.6 Hz from R32, C19 and the
//! Schmitt's 33 k / 47 k, DERIVED; ElectroSmash: "3.5 Hz"); Q4's emitter at
//! rest and its swing at the middle of DEPTH, which `CLOCK_SLOPE` is fitted to
//! so that the clock spans the 100-128 kHz measured on a real one; and the
//! pedal's level and response with the delay line in its loop, as the slot
//! runs it.
//!
//! `cargo run --release --example blue_chorus_op`

use gainstagefx::circuits::blue_chorus::{self as ce2, tap};
use gainstagefx::dsp::bbd::Brigade;
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 48_000.0;

struct Pedal {
    sim: Simulation,
    line: Brigade,
    send: usize,
    control: usize,
    tri: usize,
    returned: f64,
}

impl Pedal {
    fn new(rate: f64, depth: f64) -> Self {
        let c = tap(10_000.0, 470_000.0, ce2::OUTPUT).unwrap();
        let send = c.unknown_named(ce2::BBD_IN).unwrap();
        let control = c.unknown_named(ce2::CLOCK_CONTROL).unwrap();
        let tri = c.unknown_named("tri").unwrap();
        let mut sim = Simulation::new(c, RATE);
        sim.set_control(ce2::RATE, rate);
        sim.set_control(ce2::DEPTH, depth);
        sim.find_operating_point();
        Self {
            sim,
            line: Brigade::new(ce2::LONGEST, RATE),
            send,
            control,
            tri,
            returned: 0.0,
        }
    }

    fn process(&mut self, x: f64) -> f64 {
        self.sim.set_aux_input(ce2::BBD_RETURN, self.returned);
        let y = self.sim.process(x);
        let delay = ce2::delay_seconds(self.sim.voltage_at(self.control)) * RATE;
        self.returned = self
            .line
            .process(self.sim.voltage_at(self.send), delay, RATE);
        y
    }
}

fn main() {
    println!("the oscillator against RATE (depth at the middle):");
    let mut middle_swing = (0.0, 0.0);
    for rate in [0.0, 0.5, 1.0] {
        let mut p = Pedal::new(rate, 0.5);
        let n = (RATE * 12.0) as usize;
        let (mut tri, mut cv) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for _ in 0..n {
            p.process(0.0);
            tri.push(p.sim.voltage_at(p.tri));
            cv.push(p.sim.voltage_at(p.control));
        }
        let tail = &tri[(RATE * 2.0) as usize..];
        let mid = 0.5
            * (tail.iter().cloned().fold(f64::MAX, f64::min)
                + tail.iter().cloned().fold(f64::MIN, f64::max));
        let rises: Vec<usize> = tail
            .windows(2)
            .enumerate()
            .filter(|(_, w)| w[0] < mid && w[1] >= mid)
            .map(|(i, _)| i)
            .collect();
        let period = if rises.len() >= 2 {
            (rises[rises.len() - 1] - rises[0]) as f64 / (rises.len() - 1) as f64 / RATE
        } else {
            f64::NAN
        };
        let ctail = &cv[(RATE * 2.0) as usize..];
        let (lo, hi) = (
            ctail.iter().cloned().fold(f64::MAX, f64::min),
            ctail.iter().cloned().fold(f64::MIN, f64::max),
        );
        println!(
            "  rate {rate:.1}: {:.3} Hz; triangle {:+.2} .. {:+.2} V; Q4 emitter {lo:+.3} .. {hi:+.3} V",
            1.0 / period,
            tail.iter().cloned().fold(f64::MAX, f64::min),
            tail.iter().cloned().fold(f64::MIN, f64::max)
        );
        if rate == 0.5 {
            middle_swing = (lo, hi);
        }
    }

    // --- the clock: at rest, and the slope for 28 kHz across the middle swing --------
    let mut p = Pedal::new(0.5, 0.0);
    for _ in 0..(RATE as usize) {
        p.process(0.0);
    }
    let rest = p.sim.voltage_at(p.control);
    let slope = 28_000.0 / (middle_swing.1 - middle_swing.0);
    println!(
        "\nclock: Q4's emitter at rest {rest:+.4} V (CONTROL_REST {}); middle-depth swing {:.3} V gives CLOCK_SLOPE {slope:.0} Hz/V ({})",
        ce2::CONTROL_REST,
        middle_swing.1 - middle_swing.0,
        ce2::CLOCK_SLOPE
    );
    println!(
        "  delays as built: rest {:.2} ms, middle depth {:.2} .. {:.2} ms",
        ce2::delay_seconds(rest) * 1e3,
        ce2::delay_seconds(middle_swing.1) * 1e3,
        ce2::delay_seconds(middle_swing.0) * 1e3
    );

    // --- level, depth down: dry and a fixed delay ------------------------------------
    println!("\nlevel through the pedal, depth down, re the input:");
    for hz in [100.0, 300.0, 1_000.0, 3_000.0, 6_000.0, 10_000.0] {
        let mut p = Pedal::new(0.5, 0.0);
        let n = (RATE * 0.6) as usize;
        let (mut si, mut so) = (0.0, 0.0);
        for k in 0..n {
            let x = 0.05 * (std::f64::consts::TAU * hz * k as f64 / RATE).sin();
            let y = p.process(x);
            if k > n / 2 {
                si += x * x;
                so += y * y;
            }
        }
        print!(" {hz:.0} {:+.1}", 10.0 * (so / si).log10());
    }
    println!();
}
