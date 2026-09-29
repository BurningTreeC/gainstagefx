use std::sync::Arc;

use crate::audio_setup::{AudioIOLayout, BufferConfig, BusInfo};

#[path = "instance_types.rs"]
mod types;
use crate::error::PluginResult;
use nice_plug_core::params::Params;
pub use types::{NiceAu2EditorHandle, ParameterInfo};

pub trait NicePluginInstance: Send + 'static {
    fn allocate_render_resources(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
    ) -> PluginResult<()>;

    fn deallocate_render_resources(&mut self);

    fn is_prepared(&self) -> bool;

    fn sample_rate(&self) -> Option<f64>;

    fn max_frames(&self) -> Option<u32>;

    fn params(&self) -> Arc<dyn Params>;

    fn save_state(&self) -> Vec<u8>;

    fn load_state(&mut self, data: &[u8]) -> PluginResult<()>;

    fn reset(&mut self);

    fn tail_samples(&self) -> u32;

    fn latency_samples(&self) -> u32;

    fn process(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],
        num_samples: usize,
    ) -> PluginResult<()>;

    fn process_scheduled(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],
        num_samples: usize,
        events: &[crate::render::Au2ScheduledParameterEvent],
    ) -> PluginResult<()> {
        for event in events {
            self.set_parameter_value(event.parameter_address, event.end_value)?;
        }
        self.process(inputs, outputs, num_samples)
    }

    fn process_scheduled_with_events(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],
        num_samples: usize,
        events: &[crate::render::Au2ScheduledParameterEvent],
        _context: crate::render::Au2ProcessEvents<'_>,
    ) -> PluginResult<()> {
        self.process_scheduled(inputs, outputs, num_samples, events)
    }

    fn process_f64(
        &mut self,
        inputs: &[&[f64]],
        outputs: &mut [&mut [f64]],
        num_samples: usize,
    ) -> PluginResult<()>;

    fn apply_parameter_events(
        &mut self,
        immediate: &[crate::render::Au2ParameterEvent],
        ramps: &[crate::render::Au2ParameterRampEvent],
    ) -> PluginResult<()>;

    fn input_bus_count(&self) -> usize;

    fn output_bus_count(&self) -> usize;

    fn input_bus_info(&self, index: usize) -> Option<BusInfo>;

    fn output_bus_info(&self, index: usize) -> Option<BusInfo>;

    fn parameter_count(&self) -> usize;

    fn parameter_info(&self, index: usize) -> Option<ParameterInfo>;

    fn parameter_value(&self, param_id: u32) -> Option<f32>;

    fn set_parameter_value(&mut self, param_id: u32, value: f32) -> PluginResult<()>;

    fn frontend(&self) -> Arc<dyn NicePluginFrontend>;
}

pub trait NicePluginFrontend: Send + Sync {
    fn size(&self) -> Option<(u32, u32)>;
    fn spawn(&self, parent: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn destroy(&self, handle: *mut std::ffi::c_void);
    fn flush(&self);
}
