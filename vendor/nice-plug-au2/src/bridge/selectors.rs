use std::ffi::c_void;
use std::sync::atomic::Ordering;

use coreaudio_sys::*;

use crate::bridge::{self, NiceAu2SampleFormat};

use super::component::Component;
use super::{properties, render};

unsafe extern "C" fn midi_event(
    this: *mut c_void,
    status: u32,
    data1: u32,
    data2: u32,
    offset: u32,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidParameter;
    };
    if status > 0xff || data1 > 0x7f || data2 > 0x7f {
        return kAudioUnitErr_InvalidParameter;
    }
    component.queue_midi_event(crate::render::Au2MidiEvent {
        status: status as u8,
        data1: data1 as u8,
        data2: data2 as u8,
        sample_offset: offset,
    })
}

pub unsafe extern "C" fn open(this: *mut c_void, instance: AudioComponentInstance) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_FailedInitialization;
    };
    if !bridge::nice_au2_ensure_factory_registered() {
        return kAudioUnitErr_FailedInitialization;
    }
    component.component_instance = instance;
    component.rust_instance = bridge::nice_au2_create_instance();
    if component.rust_instance.is_null()
        || !bridge::nice_au2_get_default_bus_config(&mut component.bus_config)
    {
        if !component.rust_instance.is_null() {
            bridge::nice_au2_destroy_instance(component.rust_instance);
            component.rust_instance = std::ptr::null_mut();
        }
        return kAudioUnitErr_FailedInitialization;
    }
    component.update_formats();
    0
}

pub unsafe extern "C" fn close(this: *mut c_void) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return 0;
    };
    if !component.rust_instance.is_null() {
        bridge::nice_au2_close_editor_for_rust_instance(component.rust_instance.cast());
        if component.render_resources_allocated {
            bridge::nice_au2_deallocate_render_resources(component.rust_instance);
        }
        bridge::nice_au2_destroy_instance(component.rust_instance);
    }
    unsafe { drop(Box::from_raw(component)) };
    0
}

unsafe extern "C" fn initialize(this: *mut c_void) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidProperty;
    };
    if component.render_resources_allocated {
        bridge::nice_au2_deallocate_render_resources(component.rust_instance);
    }
    let status = bridge::nice_au2_allocate_render_resources(
        component.rust_instance,
        component.sample_rate,
        component.max_frames,
        NiceAu2SampleFormat::Float32,
        &component.bus_config,
    );
    if status != 0 {
        return status;
    }
    component.render_resources_allocated = true;
    let input_channels = component.input_channels().min(32) as usize;
    let output_channels = component.output_channels().min(32) as usize;
    for buffer in component.input_buffers.iter_mut().take(input_channels) {
        buffer.resize(component.max_frames as usize, 0.0);
    }
    for buffer in component.output_buffers.iter_mut().take(output_channels) {
        buffer.resize(component.max_frames as usize, 0.0);
    }
    0
}

unsafe extern "C" fn uninitialize(this: *mut c_void) -> OSStatus {
    if let Some(component) = unsafe { Component::from_self(this) }
        && component.render_resources_allocated
    {
        bridge::nice_au2_deallocate_render_resources(component.rust_instance);
        component.render_resources_allocated = false;
    }
    0
}

unsafe extern "C" fn get_parameter(
    this: *mut c_void,
    id: u32,
    scope: u32,
    _element: u32,
    value: *mut f32,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidParameter;
    };
    if scope != kAudioUnitScope_Global {
        return kAudioUnitErr_InvalidScope;
    }
    if value.is_null() {
        return kAudioUnitErr_InvalidParameter;
    }
    unsafe { *value = bridge::nice_au2_get_parameter_value(component.rust_instance, id) };
    0
}

unsafe extern "C" fn set_parameter(
    this: *mut c_void,
    id: u32,
    scope: u32,
    _element: u32,
    value: f32,
    _offset: u32,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidParameter;
    };
    if scope != kAudioUnitScope_Global {
        return kAudioUnitErr_InvalidScope;
    }
    bridge::nice_au2_set_parameter_value(component.rust_instance, id, value);
    0
}

unsafe extern "C" fn reset(this: *mut c_void, _scope: u32, _element: u32) -> OSStatus {
    if let Some(component) = unsafe { Component::from_self(this) } {
        bridge::nice_au2_reset(component.rust_instance);
    }
    0
}

unsafe extern "C" fn schedule_parameters(
    this: *mut c_void,
    events: *const AudioUnitParameterEvent,
    count: u32,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidParameter;
    };
    if events.is_null() && count != 0 {
        return kAudioUnitErr_InvalidParameter;
    }
    let queued = component.scheduled_event_count.load(Ordering::Acquire);
    if count > 256 - queued {
        return kAudioUnitErr_TooManyFramesToProcess;
    }
    for index in 0..count {
        let event = unsafe { &*events.add(index as usize) };
        if event.scope != kAudioUnitScope_Global {
            return kAudioUnitErr_InvalidScope;
        }
        let output = &mut component.scheduled_events[(queued + index) as usize];
        output.parameter_address = event.parameter;
        unsafe {
            if event.eventType == kParameterEvent_Immediate {
                output.sample_offset = event.eventValues.immediate.bufferOffset;
                output.duration_samples = 0;
                output.start_value = event.eventValues.immediate.value;
                output.end_value = event.eventValues.immediate.value;
            } else if event.eventType == kParameterEvent_Ramped {
                output.sample_offset = event.eventValues.ramp.startBufferOffset.max(0) as u32;
                output.duration_samples = event.eventValues.ramp.durationInFrames;
                output.start_value = event.eventValues.ramp.startValue;
                output.end_value = event.eventValues.ramp.endValue;
            } else {
                return kAudioUnitErr_InvalidParameter;
            }
        }
    }
    component
        .scheduled_event_count
        .store(queued + count, Ordering::Release);
    0
}

fn method<T>(function: T) -> AudioComponentMethod
where
    T: Copy,
{
    unsafe { Some(std::mem::transmute_copy(&function)) }
}

pub unsafe extern "C" fn lookup(selector: i16) -> AudioComponentMethod {
    match selector as u32 {
        kAudioUnitInitializeSelect => {
            method(initialize as unsafe extern "C" fn(*mut c_void) -> OSStatus)
        }
        kAudioUnitUninitializeSelect => {
            method(uninitialize as unsafe extern "C" fn(*mut c_void) -> OSStatus)
        }
        kAudioUnitGetPropertyInfoSelect => method(
            properties::info
                as unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut u32, *mut u8) -> OSStatus,
        ),
        kAudioUnitGetPropertySelect => method(
            properties::get
                as unsafe extern "C" fn(
                    *mut c_void,
                    u32,
                    u32,
                    u32,
                    *mut c_void,
                    *mut u32,
                ) -> OSStatus,
        ),
        kAudioUnitSetPropertySelect => method(
            properties::set
                as unsafe extern "C" fn(*mut c_void, u32, u32, u32, *const c_void, u32) -> OSStatus,
        ),
        kAudioUnitAddPropertyListenerSelect => method(
            properties::add_listener
                as unsafe extern "C" fn(
                    *mut c_void,
                    u32,
                    AudioUnitPropertyListenerProc,
                    *mut c_void,
                ) -> OSStatus,
        ),
        kAudioUnitRemovePropertyListenerSelect => method(
            properties::remove_listener
                as unsafe extern "C" fn(
                    *mut c_void,
                    u32,
                    AudioUnitPropertyListenerProc,
                ) -> OSStatus,
        ),
        kAudioUnitRemovePropertyListenerWithUserDataSelect => method(
            properties::remove_listener_with_data
                as unsafe extern "C" fn(
                    *mut c_void,
                    u32,
                    AudioUnitPropertyListenerProc,
                    *mut c_void,
                ) -> OSStatus,
        ),
        kAudioUnitAddRenderNotifySelect => method(
            properties::add_render_notify
                as unsafe extern "C" fn(*mut c_void, AURenderCallback, *mut c_void) -> OSStatus,
        ),
        kAudioUnitRemoveRenderNotifySelect => method(
            properties::remove_render_notify
                as unsafe extern "C" fn(*mut c_void, AURenderCallback, *mut c_void) -> OSStatus,
        ),
        kAudioUnitGetParameterSelect => method(
            get_parameter as unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut f32) -> OSStatus,
        ),
        kAudioUnitSetParameterSelect => method(
            set_parameter as unsafe extern "C" fn(*mut c_void, u32, u32, u32, f32, u32) -> OSStatus,
        ),
        kAudioUnitScheduleParametersSelect => method(
            schedule_parameters
                as unsafe extern "C" fn(
                    *mut c_void,
                    *const AudioUnitParameterEvent,
                    u32,
                ) -> OSStatus,
        ),
        kAudioUnitRenderSelect => method(
            render::render
                as unsafe extern "C" fn(
                    *mut c_void,
                    *mut u32,
                    *const AudioTimeStamp,
                    u32,
                    u32,
                    *mut AudioBufferList,
                ) -> OSStatus,
        ),
        kAudioUnitResetSelect => {
            method(reset as unsafe extern "C" fn(*mut c_void, u32, u32) -> OSStatus)
        }
        0x0101 => {
            method(midi_event as unsafe extern "C" fn(*mut c_void, u32, u32, u32, u32) -> OSStatus)
        }
        _ => None,
    }
}
