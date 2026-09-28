#![allow(clippy::not_unsafe_ptr_arg_deref)]

mod au2;
mod component;
mod editor;
mod properties;
mod render;
mod selectors;
mod view;

use std::ffi::{c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Arc, Mutex};

use crate::audio_setup::{BusType, CachedBusConfig, CachedBusInfo};
use crate::error::os_status;
use crate::factory;
use crate::instance::NicePluginInstance;
use crate::render::{
    Au2MidiEvent, Au2ScheduledParameterEvent, AudioBuffer, AudioBufferList, AudioTimeStamp,
};

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_close_editor_for_rust_instance(instance: *mut c_void) {
    editor::close_for_rust_instance(instance);
}

const MAX_BUSES: usize = 16;

const NICE_AU2_MAX_PARAM_NAME_LENGTH: usize = 128;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiceAu2BusType {
    Main = 0,
    Auxiliary = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NiceAu2BusInfo {
    pub channel_count: u32,
    pub bus_type: NiceAu2BusType,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NiceAu2BusConfig {
    pub input_bus_count: u32,
    pub output_bus_count: u32,
    pub input_buses: [NiceAu2BusInfo; MAX_BUSES],
    pub output_buses: [NiceAu2BusInfo; MAX_BUSES],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiceAu2SampleFormat {
    Float32 = 0,
}

#[repr(C)]
pub struct NiceAu2ParameterInfo {
    pub id: u32,
    pub name: [c_char; NICE_AU2_MAX_PARAM_NAME_LENGTH],
    pub units: [c_char; NICE_AU2_MAX_PARAM_NAME_LENGTH],
    pub unit_type: u32,
    pub min_value: f32,
    pub max_value: f32,
    pub default_value: f32,
    pub current_value: f32,
    pub step_count: i32,
    pub flags: u32,
    pub group_id: i32,
}

impl Default for NiceAu2ParameterInfo {
    fn default() -> Self {
        Self {
            id: 0,
            name: [0; NICE_AU2_MAX_PARAM_NAME_LENGTH],
            units: [0; NICE_AU2_MAX_PARAM_NAME_LENGTH],
            unit_type: 0,
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.0,
            current_value: 0.0,
            step_count: 0,
            flags: 0,
            group_id: 0,
        }
    }
}

pub struct NiceInstanceHandle {
    plugin: Arc<Mutex<Box<dyn NicePluginInstance>>>,
    frontend: Arc<dyn crate::instance::NicePluginFrontend>,
    sample_format: NiceAu2SampleFormat,
    sample_rate: f64,
    max_frames: u32,
    bus_config: Option<CachedBusConfig>,
}

unsafe impl Send for NiceInstanceHandle {}
unsafe impl Sync for NiceInstanceHandle {}

pub type NiceAu2InstanceHandle = *mut NiceInstanceHandle;

fn lock_plugin(
    handle: &NiceInstanceHandle,
) -> Result<std::sync::MutexGuard<'_, Box<dyn NicePluginInstance>>, i32> {
    handle
        .plugin
        .lock()
        .map_err(|_| os_status::K_AUDIO_UNIT_ERR_CANNOT_DO_IN_CURRENT_CONTEXT)
}

fn copy_str_to_char_array(s: &str, dest: &mut [c_char]) {
    let bytes = s.as_bytes();
    let copy_len = bytes.len().min(dest.len() - 1);
    for (i, &b) in bytes[..copy_len].iter().enumerate() {
        dest[i] = b as c_char;
    }
    if copy_len < dest.len() {
        dest[copy_len] = 0;
    } else {
        dest[dest.len() - 1] = 0;
    }
}

fn convert_bus_info_array(c_buses: &[NiceAu2BusInfo; MAX_BUSES], count: u32) -> Vec<CachedBusInfo> {
    let count = (count as usize).min(MAX_BUSES);
    let mut buses = Vec::with_capacity(count);

    for bus in c_buses.iter().take(count) {
        let bus_type = if bus.bus_type == NiceAu2BusType::Main {
            BusType::Main
        } else {
            BusType::Aux
        };
        buses.push(CachedBusInfo::new(bus.channel_count as usize, bus_type));
    }

    buses
}

fn bus_config_from_c(config: &NiceAu2BusConfig) -> CachedBusConfig {
    let input_buses = convert_bus_info_array(&config.input_buses, config.input_bus_count);
    let output_buses = convert_bus_info_array(&config.output_buses, config.output_bus_count);

    CachedBusConfig::new(input_buses, output_buses)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_ensure_factory_registered() -> bool {
    factory::is_registered()
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_get_component_description(desc: *mut u32) {
    if desc.is_null() {
        return;
    }
    let config = match factory::plugin_config() {
        Some(c) => c,
        None => return,
    };

    unsafe {
        *desc.add(0) = config.category.component_type();
        *desc.add(1) = crate::config::four_cc(config.sub_type);
        *desc.add(2) = crate::config::four_cc(config.manufacturer);
        *desc.add(3) = 0;
        *desc.add(4) = 0;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_get_default_bus_config(config: *mut NiceAu2BusConfig) -> bool {
    if config.is_null() {
        return false;
    }

    let Some(plugin) = factory::create_instance() else {
        return false;
    };

    let mut out = NiceAu2BusConfig {
        input_bus_count: 0,
        output_bus_count: 0,
        input_buses: [NiceAu2BusInfo {
            channel_count: 0,
            bus_type: NiceAu2BusType::Main,
        }; MAX_BUSES],
        output_buses: [NiceAu2BusInfo {
            channel_count: 0,
            bus_type: NiceAu2BusType::Main,
        }; MAX_BUSES],
    };

    let input_count = plugin.input_bus_count().min(MAX_BUSES);
    out.input_bus_count = input_count as u32;
    for index in 0..input_count {
        if let Some(info) = plugin.input_bus_info(index) {
            out.input_buses[index] = NiceAu2BusInfo {
                channel_count: info.channel_count as u32,
                bus_type: match info.bus_type {
                    BusType::Main => NiceAu2BusType::Main,
                    BusType::Aux => NiceAu2BusType::Auxiliary,
                },
            };
        }
    }

    let output_count = plugin.output_bus_count().min(MAX_BUSES);
    out.output_bus_count = output_count as u32;
    for index in 0..output_count {
        if let Some(info) = plugin.output_bus_info(index) {
            out.output_buses[index] = NiceAu2BusInfo {
                channel_count: info.channel_count as u32,
                bus_type: match info.bus_type {
                    BusType::Main => NiceAu2BusType::Main,
                    BusType::Aux => NiceAu2BusType::Auxiliary,
                },
            };
        }
    }

    unsafe {
        *config = out;
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_create_instance() -> NiceAu2InstanceHandle {
    let result = catch_unwind(|| {
        let plugin = factory::create_instance()?;
        let frontend = plugin.frontend();
        let handle = Box::new(NiceInstanceHandle {
            frontend,
            plugin: Arc::new(Mutex::new(plugin)),
            sample_format: NiceAu2SampleFormat::Float32,
            sample_rate: 44100.0,
            max_frames: 1024,
            bus_config: None,
        });
        Some(Box::into_raw(handle))
    });

    match result {
        Ok(Some(ptr)) => ptr,
        Ok(None) => ptr::null_mut(),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_destroy_instance(instance: NiceAu2InstanceHandle) {
    if instance.is_null() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        let _ = unsafe { Box::from_raw(instance) };
    }));
}

const MAX_SAMPLE_RATE: f64 = 384_000.0;
const MAX_FRAMES_PER_RENDER: u32 = 8192;

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_allocate_render_resources(
    instance: NiceAu2InstanceHandle,
    sample_rate: f64,
    max_frames: u32,
    sample_format: NiceAu2SampleFormat,
    bus_config: *const NiceAu2BusConfig,
) -> i32 {
    if instance.is_null() || bus_config.is_null() {
        return os_status::K_AUDIO_UNIT_ERR_INVALID_PARAMETER;
    }

    if sample_rate <= 0.0 || sample_rate > MAX_SAMPLE_RATE || !sample_rate.is_finite() {
        return os_status::K_AUDIO_UNIT_ERR_INVALID_PROPERTY_VALUE;
    }

    if max_frames == 0 || max_frames > MAX_FRAMES_PER_RENDER {
        return os_status::K_AUDIO_UNIT_ERR_INVALID_PROPERTY_VALUE;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &mut *instance };
        let c_bus_config = unsafe { &*bus_config };

        handle.sample_format = sample_format;
        handle.sample_rate = sample_rate;
        handle.max_frames = max_frames;

        let rust_bus_config = bus_config_from_c(c_bus_config);

        if let Err(_e) = rust_bus_config.validate() {
            return os_status::K_AUDIO_UNIT_ERR_FORMAT_NOT_SUPPORTED;
        }

        {
            let mut plugin = match lock_plugin(handle) {
                Ok(guard) => guard,
                Err(status) => return status,
            };

            let layout = plugin
                .output_bus_info(0)
                .or_else(|| plugin.input_bus_info(0));
            let nice_layout = match layout {
                Some(_) => crate::audio_setup::AudioIOLayout {
                    main_input_channels: std::num::NonZeroU32::new(
                        rust_bus_config.input_channel_count() as u32,
                    ),
                    main_output_channels: std::num::NonZeroU32::new(
                        rust_bus_config.output_channel_count() as u32,
                    ),
                    ..crate::audio_setup::AudioIOLayout::const_default()
                },
                None => crate::audio_setup::AudioIOLayout::const_default(),
            };
            let nice_buffer_config = crate::audio_setup::BufferConfig {
                sample_rate: sample_rate as f32,
                min_buffer_size: None,
                max_buffer_size: max_frames,
                process_mode: crate::audio_setup::ProcessMode::Realtime,
            };

            if let Err(e) = plugin.allocate_render_resources(&nice_layout, &nice_buffer_config) {
                log::error!("allocate_render_resources failed: {:?}", e);
                return os_status::K_AUDIO_UNIT_ERR_INVALID_PROPERTY_VALUE;
            }
        }

        handle.bus_config = Some(rust_bus_config);

        os_status::NO_ERR
    }));

    result.unwrap_or(os_status::K_AUDIO_UNIT_ERR_CANNOT_DO_IN_CURRENT_CONTEXT)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_deallocate_render_resources(instance: NiceAu2InstanceHandle) {
    if instance.is_null() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &mut *instance };

        match handle.plugin.try_lock() {
            Ok(mut plugin) => {
                plugin.deallocate_render_resources();
            }
            Err(_) => {
                log::warn!("nice_au2_deallocate_render_resources: plugin lock held");
                return;
            }
        }

        handle.bus_config = None;
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_is_prepared(instance: NiceAu2InstanceHandle) -> bool {
    if instance.is_null() {
        return false;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        let plugin = match lock_plugin(handle) {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        plugin.is_prepared()
    }));

    result.unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_render(
    instance: NiceAu2InstanceHandle,
    _action_flags: *mut u32,
    timestamp: *const AudioTimeStamp,
    frame_count: u32,
    _output_bus_number: isize,
    output_data: *mut AudioBufferList,
    events: *const Au2MidiEvent,
    event_count: u32,
    _pull_input_block: *const c_void,
    input_data: *const AudioBufferList,
    scheduled_events: *const Au2ScheduledParameterEvent,
    scheduled_event_count: u32,
    midi_output: &mut Vec<crate::render::Au2MidiEvent>,
    transport: *const crate::render::Au2TransportInfo,
) -> i32 {
    if instance.is_null() || output_data.is_null() {
        return os_status::K_AUDIO_UNIT_ERR_INVALID_PARAMETER;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };

        if frame_count > handle.max_frames {
            return os_status::K_AUDIO_UNIT_ERR_TOO_MANY_FRAMES_TO_PROCESS;
        }

        let mut plugin = match handle.plugin.try_lock() {
            Ok(guard) => guard,
            Err(_) => return os_status::K_AUDIO_UNIT_ERR_CANNOT_DO_IN_CURRENT_CONTEXT,
        };

        let output_buffer_list = unsafe { &*output_data };
        let num_buffers = output_buffer_list.mNumberBuffers as usize;
        if num_buffers > 32 {
            return os_status::K_AUDIO_UNIT_ERR_FORMAT_NOT_SUPPORTED;
        }

        let mut output_slices: [&mut [f32]; 32] = std::array::from_fn(|_| &mut [] as &mut [f32]);

        for (i, output_slice) in output_slices.iter_mut().enumerate().take(num_buffers) {
            let buffer = unsafe { AudioBuffer::from_list_mut(output_data, i) };
            if buffer.mData.is_null() {
                return os_status::K_AUDIO_UNIT_ERR_INVALID_PARAMETER;
            }
            let data = buffer.mData as *mut f32;
            let size = buffer.mDataByteSize as usize / std::mem::size_of::<f32>();
            if size < frame_count as usize {
                return os_status::K_AUDIO_UNIT_ERR_INVALID_PARAMETER;
            }
            let slice = unsafe { std::slice::from_raw_parts_mut(data, size) };
            *output_slice = &mut slice[..frame_count as usize];
        }

        let mut input_slices: [&[f32]; 32] = [&[]; 32];
        let mut input_count = 0;
        if !input_data.is_null() {
            let input_buffer_list = unsafe { &*input_data };
            if input_buffer_list.mNumberBuffers > 32 {
                return os_status::K_AUDIO_UNIT_ERR_FORMAT_NOT_SUPPORTED;
            }
            for i in 0..input_buffer_list.mNumberBuffers as usize {
                let buffer = unsafe { AudioBuffer::from_list(input_data, i) };
                if buffer.mData.is_null() {
                    continue;
                }
                let data = buffer.mData as *const f32;
                let size = buffer.mDataByteSize as usize / std::mem::size_of::<f32>();
                if size >= frame_count as usize {
                    input_slices[input_count] =
                        unsafe { std::slice::from_raw_parts(data, frame_count as usize) };
                    input_count += 1;
                }
            }
        }

        let parameter_event_count = if scheduled_events.is_null() {
            0
        } else {
            scheduled_event_count.min(256) as usize
        };
        let mut sorted_events = [Au2ScheduledParameterEvent::default(); 256];
        if !scheduled_events.is_null() && parameter_event_count > 0 {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    scheduled_events,
                    sorted_events.as_mut_ptr(),
                    parameter_event_count,
                );
            }
            sorted_events[..parameter_event_count]
                .sort_unstable_by_key(|event| event.sample_offset);
        }
        let parameter_events = &sorted_events[..parameter_event_count];
        let midi_events = if events.is_null() {
            &[][..]
        } else {
            unsafe {
                std::slice::from_raw_parts(
                    events.cast::<crate::render::Au2MidiEvent>(),
                    event_count as usize,
                )
            }
        };
        let _ = timestamp;
        let transport = unsafe { &*transport };
        match plugin.process_scheduled_with_events(
            &input_slices[..input_count],
            &mut output_slices[..num_buffers],
            frame_count as usize,
            parameter_events,
            crate::render::Au2ProcessEvents {
                midi_events,
                transport: *transport,
                midi_output,
            },
        ) {
            Ok(()) => os_status::NO_ERR,
            Err(e) => {
                log::error!("Render error: {:?}", e);
                os_status::K_AUDIO_UNIT_ERR_RENDER
            }
        }
    }));

    result.unwrap_or(os_status::K_AUDIO_UNIT_ERR_RENDER)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_reset(instance: NiceAu2InstanceHandle) {
    if instance.is_null() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &mut *instance };
        if let Ok(mut plugin) = lock_plugin(handle) {
            plugin.reset();
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_get_parameter_count(instance: NiceAu2InstanceHandle) -> u32 {
    if instance.is_null() {
        return 0;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        let plugin = match lock_plugin(handle) {
            Ok(guard) => guard,
            Err(_) => return 0u32,
        };
        plugin.parameter_count() as u32
    }));

    result.unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_get_latency_samples(instance: NiceAu2InstanceHandle) -> u32 {
    if instance.is_null() {
        return 0;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        lock_plugin(handle).map_or(0, |plugin| plugin.latency_samples())
    }))
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_get_tail_samples(instance: NiceAu2InstanceHandle) -> u32 {
    if instance.is_null() {
        return 0;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        lock_plugin(handle).map_or(0, |plugin| plugin.tail_samples())
    }))
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_get_parameter_info(
    instance: NiceAu2InstanceHandle,
    index: u32,
    out_info: *mut NiceAu2ParameterInfo,
) -> bool {
    if instance.is_null() || out_info.is_null() {
        return false;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        let plugin = match lock_plugin(handle) {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let Some(info) = plugin.parameter_info(index as usize) else {
            return false;
        };

        unsafe {
            (*out_info).id = info.id;
            copy_str_to_char_array(&info.name, &mut (*out_info).name);
            copy_str_to_char_array(&info.units, &mut (*out_info).units);
            (*out_info).unit_type = 0;
            (*out_info).min_value = info.min_value;
            (*out_info).max_value = info.max_value;
            (*out_info).default_value = info.default_value;
            (*out_info).current_value = info.current_value;
            (*out_info).step_count = info.step_count;
            (*out_info).flags = info.flags;
            (*out_info).group_id = info.group_id;
        }

        true
    }));

    result.unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_get_parameter_value(
    instance: NiceAu2InstanceHandle,
    param_id: u32,
) -> f32 {
    if instance.is_null() {
        return 0.0;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        let plugin = match lock_plugin(handle) {
            Ok(guard) => guard,
            Err(_) => return 0.0,
        };
        plugin.parameter_value(param_id).unwrap_or(0.0)
    }));

    result.unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_set_parameter_value(
    instance: NiceAu2InstanceHandle,
    param_id: u32,
    value: f32,
) {
    if instance.is_null() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        if let Ok(mut plugin) = lock_plugin(handle) {
            let _ = plugin.set_parameter_value(param_id, value);
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_get_editor_size(
    instance: NiceAu2InstanceHandle,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    if instance.is_null() || width.is_null() || height.is_null() {
        return false;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        let Some((w, h)) = handle.frontend.size() else {
            return false;
        };

        unsafe {
            *width = w;
            *height = h;
        }
        true
    }));

    result.unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_spawn_editor(
    instance: NiceAu2InstanceHandle,
    parent_ns_view: *mut c_void,
) -> *mut c_void {
    if instance.is_null() || parent_ns_view.is_null() {
        return ptr::null_mut();
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        handle.frontend.spawn(parent_ns_view)
    }));

    result.unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_destroy_editor(
    instance: NiceAu2InstanceHandle,
    editor_handle: *mut c_void,
) {
    if instance.is_null() || editor_handle.is_null() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        handle.frontend.destroy(editor_handle);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_flush_editor_notifications(instance: NiceAu2InstanceHandle) {
    if instance.is_null() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        handle.frontend.flush();
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_save_state(instance: NiceAu2InstanceHandle, size: *mut u32) -> *mut u8 {
    if instance.is_null() || size.is_null() {
        return ptr::null_mut();
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &*instance };
        let plugin = match lock_plugin(handle) {
            Ok(guard) => guard,
            Err(_) => return ptr::null_mut(),
        };

        let data = plugin.save_state();
        unsafe { *size = data.len() as u32 };

        let total_size = std::mem::size_of::<usize>() + data.len();
        let mut allocation = Vec::<u8>::with_capacity(total_size);
        allocation.extend_from_slice(&data.len().to_ne_bytes());
        allocation.extend_from_slice(&data);

        let ptr = unsafe { allocation.as_mut_ptr().add(std::mem::size_of::<usize>()) };
        std::mem::forget(allocation);
        ptr
    }));

    result.unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_load_state(
    instance: NiceAu2InstanceHandle,
    data: *const u8,
    size: u32,
) -> i32 {
    if instance.is_null() || data.is_null() {
        return os_status::K_AUDIO_UNIT_ERR_INVALID_PARAMETER;
    }

    let result = catch_unwind(AssertUnwindSafe(|| {
        let handle = unsafe { &mut *instance };
        let mut plugin = match lock_plugin(handle) {
            Ok(guard) => guard,
            Err(status) => return status,
        };

        let slice = unsafe { std::slice::from_raw_parts(data, size as usize) };
        match plugin.load_state(slice) {
            Ok(()) => os_status::NO_ERR,
            Err(_) => os_status::K_AUDIO_UNIT_ERR_INVALID_PARAMETER,
        }
    }));

    result.unwrap_or(os_status::K_AUDIO_UNIT_ERR_RENDER)
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_free_state(data: *mut u8) {
    if !data.is_null() {
        unsafe {
            let base = data.sub(std::mem::size_of::<usize>());
            let mut len_bytes = [0u8; std::mem::size_of::<usize>()];
            std::ptr::copy_nonoverlapping(base, len_bytes.as_mut_ptr(), len_bytes.len());
            let len = usize::from_ne_bytes(len_bytes);
            let total_size = std::mem::size_of::<usize>() + len;
            let _ = Vec::from_raw_parts(base, total_size, total_size);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn nice_au2_create_cocoa_view(
    audio_unit: objc2_audio_toolbox::AudioUnit,
) -> *mut objc2_app_kit::NSView {
    view::create_view(audio_unit)
}
