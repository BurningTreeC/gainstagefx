//! The response of a network, printed. One solve per frequency, no audio.

use gainstagefx::circuits::stack;
use gainstagefx::dsp::ac;

fn main() {
    let hz = [82.0, 160.0, 320.0, 640.0, 1000.0, 1600.0, 3200.0, 6400.0];
    let circuit = stack::build(&stack::BRITISH, 38_000.0, 1_000_000.0).expect("builds");

    print!("{:<26}", "British stack");
    for f in hz {
        print!("{f:>8.0}");
    }
    println!();
    for (label, c) in [
        ("bass 5  mid 5  treb 5", [0.5, 0.5, 0.5]),
        ("bass 10 mid 0  treb 10", [1.0, 0.0, 1.0]),
        ("bass 5  mid 10 treb 5", [0.5, 1.0, 0.5]),
        ("bass 0  mid 5  treb 10", [0.0, 0.5, 1.0]),
    ] {
        print!("{label:<26}");
        for f in hz {
            print!("{:>8.1}", ac::solve(&circuit, &c, f).db());
        }
        println!();
    }
}
