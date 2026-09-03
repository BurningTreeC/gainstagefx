//! What each tone control does, on its own and together.

use gainstagefx::circuits::tone;
use gainstagefx::dsp::ac;

fn main() {
    let hz = [82.0, 160.0, 320.0, 550.0, 1000.0, 2000.0, 4000.0, 8000.0];
    for henry in [0.35, 0.7, 1.2, 2.0] {
        let mut v = tone::WIDE;
        v.mid_henry = henry;
        let c = tone::build(&v, 38_000.0, 220_000.0).expect("builds");
        let flat = ac::solve(&c, &[1.0, 1.0, 1.0], 82.0).db();
        let scoop = ac::solve(&c, &[1.0, 0.0, 1.0], 82.0).db();
        let depth = ac::solve(&c, &[1.0, 0.0, 1.0], 550.0).db();
        println!(
            "  L = {henry:>4.2} H  notch {:>6.1} dB deep, and costs {:>5.1} dB at 82 Hz",
            depth - flat,
            scoop - flat
        );
    }
    println!();
    let circuit = tone::build(&tone::WIDE, 38_000.0, 220_000.0).expect("builds");

    print!("{:<26}", "wide voicing");
    for f in hz {
        print!("{f:>8.0}");
    }
    println!();
    for (label, c) in [
        ("all 10 (flat)", [1.0, 1.0, 1.0]),
        ("bass 10 mid 0  treb 10", [1.0, 0.0, 1.0]),
        ("bass 10 mid 5  treb 10", [1.0, 0.5, 1.0]),
        ("bass 0  mid 10 treb 10", [0.0, 1.0, 1.0]),
        ("bass 10 mid 10 treb 0", [1.0, 1.0, 0.0]),
        ("bass 5  mid 5  treb 5", [0.5, 0.5, 0.5]),
    ] {
        print!("{label:<26}");
        for f in hz {
            print!("{:>8.1}", ac::solve(&circuit, &c, f).db());
        }
        println!();
    }
}
