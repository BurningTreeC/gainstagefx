//! The cabinet's response, straight out of the AC solver.
use gainstagefx::circuits::cabinet;
use gainstagefx::dsp::ac;

fn main() {
    let hz = [40.0, 60.0, 82.0, 110.0, 250.0, 400.0, 1000.0, 2000.0, 3000.0,
              4000.0, 6000.0, 8000.0, 12000.0];
    print!("{:<10}", "");
    for f in hz {
        print!("{f:>7.0}");
    }
    println!();
    for (name, v) in [("1x12", cabinet::COMBO), ("4x12", cabinet::STACK)] {
        let c = cabinet::build(&v, 600.0).expect("builds");
        let reference = ac::solve(&c, &[], 400.0).db();
        print!("{name:<10}");
        for f in hz {
            print!("{:>7.1}", ac::solve(&c, &[], f).db() - reference);
        }
        println!();
    }
    println!("\n(dB relative to 400 Hz)");
}
