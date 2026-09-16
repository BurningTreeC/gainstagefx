//! Shared measurement helpers for the microphone preamplifier tests.
#![allow(dead_code)]
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::{Circuit, CoreSpec, Netlist};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{Chain, Gain, Settings, Tone as Stack, NOMINAL_DBFS};

pub fn measure(c: &Circuit, rate: f64, controls: &[(usize, f64)], hz: f64, peak: f64) -> measure::Measured {
    let mut sim = Simulation::new(c.clone(), rate);
    for &(k, v) in controls {
        sim.set_control(k, v);
    }
    assert!(sim.find_operating_point());
    let t = Tone::near(rate, 16_384, hz, peak);
    measure::run(t, (rate * 0.5) as usize, |x| sim.process(x))
}

/// A transformer on its own, the way a data sheet tests it.
#[allow(clippy::too_many_arguments)]
pub fn transformer(source: f64, primary_r: f64, core: CoreSpec, ratio: f64, secondary_r: f64, shunt: f64, load: f64) -> Circuit {
    let mut n = Netlist::new("transformer under test");
    n.input("in", source)
        .resistor("in", "p", primary_r)
        .core("p", "gnd", core)
        .transformer("p", "gnd", "s", "gnd", ratio)
        .resistor("s", "o", secondary_r)
        .capacitor("o", "gnd", shunt)
        .resistor("o", "gnd", load);
    n.build("o").unwrap()
}

/// Volts peak for a level in dBu.
pub fn dbu(level: f64) -> f64 {
    0.7746 * 10f64.powf(level / 20.0) * 2f64.sqrt()
}

/// The voice in a whole chain: allocation-free and finite at every rate.
pub fn realtime_safe(gain: Gain, alloc: impl Fn(&mut dyn FnMut())) {
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        chain.apply(&Settings { gain, drive: 0.9, tone: Stack::Off, oversampling: 1, ..Settings::default() });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let before = chain.solver_breakdown();
        alloc(&mut || {
            for k in 0..4_096 {
                let x = amplitude * (k as f64 * std::f64::consts::TAU * 220.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{gain:?} at {rate}");
            }
        });
        assert!(chain.solver_breakdown().saturating_delta(before).gain.solves > 0);
    }
}
