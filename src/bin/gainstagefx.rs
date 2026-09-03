//! A standalone host, for trying the plugin without a DAW.

use nih_plug::prelude::*;

fn main() {
    nih_export_standalone::<gainstagefx::plugin::GainStageFx>();
}
