//! Structural survey of every nonlinear Schur reduction in the plugin.
//!
//! `cargo run --release --example partition_survey`
//!
//! This is the measurement behind solver-architecture decisions: which reduced
//! boundary dimensions actually occur, how large the recovered internal block
//! is per model, and how dense the recovery/coupling matrices really are.
use gainstagefx::acoustics::speaker::{self, LoadValues, Mounting, SpeakerProfile};
use gainstagefx::circuits::{
    ac30, american312, bigmuff, brit800, clipper, console_e, distortion_plus, dr103, evh5150,
    heavy_metal, iron, jfet, markiic, metal_zone, neve, plexi, power, preamp, rectifier, rodent,
    round_fuzz, studio, ts808, tube610, twin, valve,
};
use gainstagefx::dsp::netlist::Circuit;
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 48_000.0;
const SOURCE: f64 = 10_000.0;
const LOAD: f64 = 470_000.0;

fn row(name: &str, circuit: Circuit) {
    let mut sim = Simulation::new(circuit, RATE);
    let cost = sim.reduced_elimination_cost();
    let full = sim.unknowns();
    match sim.nonlinear_structure() {
        Some(s) => {
            let response_dense = s.response_entries - s.response_zero_entries;
            let response_fill = if s.response_entries == 0 {
                0.0
            } else {
                100.0 * response_dense as f64 / s.response_entries as f64
            };
            let coupling_fill = if s.coupling_entries == 0 {
                0.0
            } else {
                100.0 * s.coupling_nonzero_entries as f64 / s.coupling_entries as f64
            };
            let span_fill = if s.response_entries == 0 {
                0.0
            } else {
                100.0 * s.response_span_entries as f64 / s.response_entries as f64
            };
            let (jfill, dense, sparse, scanned, book) = match cost {
                Some((b, structural, dense, sparse, scanned, book)) => (
                    100.0 * structural as f64 / (b * b) as f64,
                    dense,
                    sparse,
                    scanned,
                    book,
                ),
                None => (0.0, 0, 0, 0, 0),
            };
            let saved = if dense == 0 {
                0.0
            } else {
                100.0 * (dense - sparse.min(dense)) as f64 / dense as f64
            };
            println!(
                "{name:<22}{full:>5}{:>5}{:>6}{:>8}{:>8.1}%{:>8.1}%{:>7}{:>7}{:>7}{:>9.1}%{:>6}{:>9.1}%{:>7}{:>7}{:>8.1}%{:>7}{:>7}",
                s.boundary,
                s.internal,
                s.response_entries,
                response_fill,
                span_fill,
                s.response_runs,
                s.response_zero_columns,
                s.response_zero_rows,
                coupling_fill,
                s.active_rhs_nodes,
                jfill,
                dense,
                sparse,
                saved,
                scanned,
                book,
            );
        }
        None => println!("{name:<22}{full:>5}   -- no nonlinear reduction"),
    }
}

fn main() {
    println!(
        "{:<22}{:>5}{:>5}{:>6}{:>8}{:>9}{:>9}{:>7}{:>7}{:>7}{:>10}{:>6}{:>9}{:>7}{:>7}{:>8}{:>7}{:>7}",
        "model", "full", "b", "i", "i*b", "fill", "span", "runs", "0cols", "0rows", "cpl_fill", "rhs", "Jfill", "dense", "sparse", "saved", "scan", "book"
    );
    println!("-- gain / preamp / pedal circuits --");
    row("markiic", markiic::build(SOURCE, LOAD).unwrap());
    row("evh5150", evh5150::build(SOURCE, LOAD).unwrap());
    row("twin (preamp)", twin::build(SOURCE, LOAD).unwrap());
    row("brit800", brit800::build(SOURCE, LOAD).unwrap());
    row("plexi", plexi::build(SOURCE, LOAD).unwrap());
    row("ac30", ac30::build(SOURCE, LOAD).unwrap());
    row("dr103", dr103::build(SOURCE, LOAD).unwrap());
    row("american312", american312::build(SOURCE, LOAD).unwrap());
    row("tube610", tube610::build(SOURCE, LOAD).unwrap());
    row("neve", neve::build(SOURCE, LOAD).unwrap());
    row("console_e", console_e::build(SOURCE, LOAD).unwrap());
    row("rectifier", rectifier::build(SOURCE, LOAD).unwrap());
    row(
        "heavy_metal (HM-2)",
        heavy_metal::build(SOURCE, LOAD).unwrap(),
    );
    row(
        "metal_zone (MT-2)",
        metal_zone::build(SOURCE, LOAD).unwrap(),
    );
    row("ts808", ts808::build(SOURCE, LOAD).unwrap());
    row("rodent", rodent::build(SOURCE, LOAD).unwrap());
    row(
        "distortion_plus",
        distortion_plus::build(SOURCE, LOAD).unwrap(),
    );
    row("round_fuzz", round_fuzz::build(SOURCE, LOAD).unwrap());
    row(
        "bigmuff",
        bigmuff::build(&bigmuff::RAMS_HEAD, SOURCE, LOAD).unwrap(),
    );
    row(
        "clipper",
        clipper::build(&clipper::DISTORTION, SOURCE, LOAD).unwrap(),
    );
    row(
        "valve",
        valve::build(&valve::CLASSIC, SOURCE, LOAD).unwrap(),
    );
    row("jfet", jfet::build(&jfet::CLASSIC, SOURCE, LOAD).unwrap());
    row(
        "studio",
        studio::build(&studio::CONSOLE, SOURCE, LOAD).unwrap(),
    );
    row("iron", iron::build(&iron::OUTPUT, SOURCE, LOAD).unwrap());
    row(
        "preamp",
        preamp::build(&preamp::HIGH_GAIN, SOURCE, LOAD).unwrap(),
    );

    println!("-- power stages (resistive load) --");
    for (name, spec) in power_specs() {
        row(name, power::build(spec, SOURCE).unwrap());
    }

    println!("-- power stages (speaker load, the production path) --");
    for (name, spec) in power_specs() {
        let values = LoadValues::new(
            &SpeakerProfile::BRIT_V30,
            &Mounting::BAFFLE,
            power::speaker_scale(spec),
        );
        let (circuit, _slots) = power::build_with_speaker(spec, SOURCE, &values).unwrap();
        row(&format!("{name} + spk"), circuit);
    }

    println!("-- speaker alone --");
    let values = LoadValues::new(&SpeakerProfile::BRIT_V30, &Mounting::BAFFLE, 1.0);
    let (circuit, _) = speaker::voltage_driven(&values).unwrap();
    row("speaker (driven)", circuit);
}

fn power_specs() -> Vec<(&'static str, &'static power::PowerSpec)> {
    vec![
        ("MARKIIC", &power::PowerSpec::MARKIIC),
        ("TWIN", &power::PowerSpec::TWIN),
        ("EVH5150", &power::PowerSpec::EVH5150),
        ("BRIT_EL34", &power::PowerSpec::BRIT_EL34),
        ("PLEXI_EL34", &power::PowerSpec::PLEXI_EL34),
        ("AC30_EL84", &power::PowerSpec::AC30_EL84),
        ("DR103_EL34", &power::PowerSpec::DR103_EL34),
        ("RECTO_6L6", &power::PowerSpec::RECTO_6L6),
        ("RECTO_6L6_TUBE", &power::PowerSpec::RECTO_6L6_TUBE),
    ]
}
