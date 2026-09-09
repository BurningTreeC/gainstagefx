//! Where the 5150's time actually goes, stage by stage, under the signal that
//! costs the most.
//!
//! `examples/powercost.rs` measured the two simulations at 19.5 % and 22.8 %
//! of realtime, 42 % between them. `examples/needit.rs` measured the whole
//! chain, with the tone stack and cabinet switched out, at 152 %. Those cannot
//! both be right about the same circuit, so something between them is either
//! expensive or signal-dependent, and the difference between the two harnesses
//! is the signal: a gentle 0.03 rad/sample sine against a 1757 Hz tone at a
//! quarter of full scale.
//!
//! A nonlinear solve is cheapest in silence and dearest near clipping. If the
//! pass count moves with the signal, the earlier "2.00 passes, so this is
//! matrix size and not convergence" reading was measured on the easy case.
//!
//! `cargo run --release --example wherecpu`

use gainstagefx::voice::{self, Cabinet, Chain, Gain, Tone as ToneSection};

const RATE: f64 = 48_000.0;

fn chain(gain: Gain, tone: ToneSection, cab: Cabinet) -> Chain {
    let mut c = Chain::new(RATE);
    c.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
    c.set_tone_section(tone);
    c.set_cabinet(cab);
    c.set_iron(voice::Iron::Off);
    c.set_oversampling(1);
    c.set_drive(0.8);
    c.settle();
    c.find_operating_point();
    c
}

/// One second of a given stimulus, returning realtime percentage and the
/// Newton passes the gain circuit averaged over it.
fn cost(gain: Gain, stim: impl Fn(usize) -> f64, tone: ToneSection, cab: Cabinet) -> (f64, f64) {
    let mut c = chain(gain, tone, cab);
    // Warm, so the operating-point hunt is not in the timing.
    for k in 0..1000 {
        c.process(stim(k));
    }
    let before = c.passes_per_sample();
    let n = RATE as usize;
    let start = std::time::Instant::now();
    for k in 0..n {
        std::hint::black_box(c.process(stim(k)));
    }
    let realtime = start.elapsed().as_secs_f64() * 100.0;
    let after = c.passes_per_sample();
    // `passes_per_sample` is cumulative, so this is close enough to the
    // second's own average once a thousand samples are already in it.
    let _ = before;
    (realtime, after)
}

fn main() {
    println!("One second of audio on one channel, at 48 kHz, drive 0.8.\n");

    println!("The two stimuli the earlier harnesses used:\n");
    println!(
        "  {:<12}{:<24}{:>12}{:>10}",
        "voice", "stimulus", "realtime", "passes"
    );
    for gain in [Gain::Peavey, Gain::Boogie, Gain::Neve, Gain::Muff] {
        for (name, f) in [
            (
                "gentle 0.2 @ 0.03 rad",
                Box::new(|k: usize| 0.2 * (k as f64 * 0.03).sin()) as Box<dyn Fn(usize) -> f64>,
            ),
            (
                "1757 Hz @ 0.25",
                Box::new(|k: usize| 0.25 * (k as f64 * 0.23).sin()),
            ),
            ("silence", Box::new(|_| 0.0)),
            (
                "full scale @ 0.23 rad",
                Box::new(|k: usize| 1.0 * (k as f64 * 0.23).sin()),
            ),
        ] {
            let (rt, passes) = cost(gain, &f, ToneSection::Off, Cabinet::Off);
            println!("  {:<12}{name:<24}{rt:>10.0} %{passes:>10.2}", gain.name());
        }
        println!();
    }

    println!("And what the rest of the chain adds, on the 5150 at 1757 Hz:\n");
    for (name, tone, cab) in [
        ("circuits only", ToneSection::Off, Cabinet::Off),
        ("+ tone stack", ToneSection::Scooping, Cabinet::Off),
        ("+ cabinet", ToneSection::Scooping, Cabinet::Stack),
    ] {
        let (rt, _) = cost(
            Gain::Peavey,
            |k: usize| 0.25 * (k as f64 * 0.23).sin(),
            tone,
            cab,
        );
        println!("  {name:<24}{rt:>10.0} %");
    }
}
