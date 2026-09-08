//! One number, measured three ways, because a timing figure nobody can check
//! is not a measurement.
use gainstagefx::voice::{Cabinet, Chain, Gain, Settings, Tone as ToneSection};
const RATE: f64 = 48_000.0;
fn main() {
    for gain in [Gain::Peavey, Gain::Crunch] {
        let mut c = Chain::new(RATE);
        c.apply(&Settings {
            gain,
            drive: 0.8,
            tone: ToneSection::Scooping,
            cabinet: Cabinet::Stack,
            oversampling: 1,
            ..Settings::default()
        });
        c.settle();
        // Read *after* a sample has gone through, not before. Changing the
        // voice starts a crossfade, and an oversampling change made during one
        // is deferred to the fade's first sample -- so asking before any audio
        // has moved reports the factor the chain is about to leave, which is
        // how this example first claimed Crunch was running at four times when
        // it was running at one.
        c.process(0.0);
        // A plain second of audio, one channel, no drive sweeping at all.
        let n = RATE as usize;
        let start = std::time::Instant::now();
        let mut acc = 0.0f64;
        for k in 0..n {
            acc += c.process(0.1 * (k as f64 * 0.03).sin());
        }
        let secs = start.elapsed().as_secs_f64();
        println!(
            "{} at {}x: one second of audio on one channel took {:.3} s of CPU \
             -- {:.2} us a sample, {:.0}% of realtime",
            gain.name(),
            c.effective_oversampling(),
            secs,
            secs / n as f64 * 1e6,
            secs * 100.0
        );
        std::hint::black_box(acc);
    }
}
