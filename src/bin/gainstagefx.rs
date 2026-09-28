//! A standalone host, for trying the plugin without a DAW.

use nice_plug::prelude::*;

fn main() {
    nice_export_standalone::<gainstagefx::plugin::GainStageFx>();
}
