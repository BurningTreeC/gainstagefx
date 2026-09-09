//! What oversampling each voice actually runs at, whatever the control says.
use gainstagefx::voice::{self, Chain, Gain};
fn main() {
    println!(
        "  {:<12}{:>10}{:>12}",
        "voice", "asked for", "actually runs"
    );
    for gain in [
        Gain::Peavey,
        Gain::Boogie,
        Gain::Neve,
        Gain::Muff,
        Gain::Screamer,
        Gain::Crunch,
    ] {
        let mut c = Chain::new(48_000.0);
        c.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
        for asked in [1usize, 2, 4, 8] {
            c.set_oversampling(asked);
            if asked == 4 {
                println!(
                    "  {:<12}{:>10}x{:>11}x",
                    gain.name(),
                    asked,
                    c.effective_oversampling()
                );
            }
        }
    }
}
