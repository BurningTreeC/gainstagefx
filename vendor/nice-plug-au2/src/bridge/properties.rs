use std::ffi::{CStr, c_void};
use std::mem::{size_of, zeroed};
use std::ptr;

use coreaudio_sys::*;

use crate::bridge::{self, NiceAu2ParameterInfo};

use super::component::{Component, PropertyListener, RenderNotify};

const RUST_INSTANCE_PROPERTY: u32 = 0x4E41_7269;
const MIDI_OUTPUT_CALLBACK_INFO: u32 = 47;
const MIDI_OUTPUT_CALLBACK: u32 = 48;
const HOST_CALLBACKS: u32 = 27;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFDictionaryGetValue(dictionary: *const c_void, key: *const c_void) -> *const c_void;
    fn CFDataCreate(
        allocator: *const c_void,
        bytes: *const u8,
        length: isize,
    ) -> *const c_void;
    fn CFDataGetTypeID() -> usize;
    fn CFDataGetBytePtr(data: *const c_void) -> *const u8;
    fn CFDataGetLength(data: *const c_void) -> isize;
    fn CFGetTypeID(object: *const c_void) -> usize;
    fn CFNumberCreate(
        allocator: *const c_void,
        the_type: isize,
        value_ptr: *const c_void,
    ) -> *const c_void;

    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
}

fn property_callback_eq(
    a: AudioUnitPropertyListenerProc,
    b: AudioUnitPropertyListenerProc,
) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => std::ptr::fn_addr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

fn render_callback_eq(a: AURenderCallback, b: AURenderCallback) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => std::ptr::fn_addr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

fn valid_scope(property: u32, scope: u32) -> bool {
    match property {
        kAudioUnitProperty_StreamFormat => {
            scope == kAudioUnitScope_Input || scope == kAudioUnitScope_Output
        }
        kAudioUnitProperty_MakeConnection | kAudioUnitProperty_SetRenderCallback => {
            scope == kAudioUnitScope_Input
        }
        kAudioUnitProperty_ClassInfo
        | kAudioUnitProperty_ParameterList
        | kAudioUnitProperty_ParameterInfo
        | kAudioUnitProperty_CocoaUI
        | kAudioUnitProperty_Latency
        | kAudioUnitProperty_TailTime
        | kAudioUnitProperty_SupportedNumChannels
        | kAudioUnitProperty_PresentPreset => scope == kAudioUnitScope_Global,
        kAudioUnitProperty_ElementCount => scope <= kAudioUnitScope_Output,
        _ => true,
    }
}

pub unsafe extern "C" fn info(
    this: *mut c_void,
    property: u32,
    scope: u32,
    _element: u32,
    size: *mut u32,
    writable: *mut u8,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidProperty;
    };
    if !valid_scope(property, scope) {
        return kAudioUnitErr_InvalidScope;
    }
    let (bytes, can_write) = match property {
        kAudioUnitProperty_ClassInfo => (size_of::<*mut c_void>(), true),
        kAudioUnitProperty_StreamFormat => (size_of::<AudioStreamBasicDescription>(), true),
        kAudioUnitProperty_SampleRate => (size_of::<f64>(), true),
        kAudioUnitProperty_ElementCount | kAudioUnitProperty_MaximumFramesPerSlice => (
            size_of::<u32>(),
            property == kAudioUnitProperty_MaximumFramesPerSlice,
        ),
        kAudioUnitProperty_SupportedNumChannels => (size_of::<AUChannelInfo>(), false),
        kAudioUnitProperty_MakeConnection => (size_of::<AudioUnitConnection>(), true),
        kAudioUnitProperty_ParameterList => (
            bridge::nice_au2_get_parameter_count(component.rust_instance) as usize
                * size_of::<u32>(),
            false,
        ),
        kAudioUnitProperty_ParameterInfo => (size_of::<AudioUnitParameterInfo>(), false),
        kAudioUnitProperty_Latency | kAudioUnitProperty_TailTime => (size_of::<f64>(), false),
        kAudioUnitProperty_PresentPreset => (size_of::<AUPreset>(), true),
        kAudioUnitProperty_CocoaUI => (size_of::<AudioUnitCocoaViewInfo>(), false),
        MIDI_OUTPUT_CALLBACK_INFO => (0, false),
        MIDI_OUTPUT_CALLBACK => (size_of::<super::component::MidiOutputCallback>(), true),
        HOST_CALLBACKS => (size_of::<super::component::HostCallbacks>(), true),
        RUST_INSTANCE_PROPERTY => (size_of::<*mut c_void>(), false),
        kAudioUnitProperty_SetRenderCallback => (size_of::<AURenderCallbackStruct>(), true),
        _ => return kAudioUnitErr_InvalidProperty,
    };
    if !size.is_null() {
        unsafe { *size = bytes as u32 };
    }
    if !writable.is_null() {
        unsafe { *writable = can_write as u8 };
    }
    0
}

unsafe fn copy_out<T: Copy>(value: &T, output: *mut c_void, io_size: *mut u32) {
    let actual = size_of::<T>();
    let copied = if io_size.is_null() {
        actual
    } else {
        actual.min(unsafe { *io_size } as usize)
    };
    if !output.is_null() {
        unsafe {
            ptr::copy_nonoverlapping((value as *const T).cast::<u8>(), output.cast(), copied)
        };
    }
    if !io_size.is_null() {
        unsafe { *io_size = actual as u32 };
    }
}

pub unsafe extern "C" fn get(
    this: *mut c_void,
    property: u32,
    scope: u32,
    element: u32,
    output: *mut c_void,
    io_size: *mut u32,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidProperty;
    };
    if output.is_null() {
        return kAudioUnitErr_InvalidProperty;
    }
    if !valid_scope(property, scope) {
        return kAudioUnitErr_InvalidScope;
    }
    match property {
        kAudioUnitProperty_ClassInfo => {
            // AUv2 ClassInfo is a CFPropertyListRef dictionary. Besides our
            // canonical nice-plug state blob, auval requires the standard
            // component identity fields to be present and to match this AU.
            let mut state_size = 0u32;
            let state_ptr =
                bridge::nice_au2_save_state(component.rust_instance, &mut state_size);
            if state_ptr.is_null() {
                return kAudioUnitErr_InvalidPropertyValue;
            }

            let type_key = unsafe {
                CFStringCreateWithCString(
                    ptr::null(),
                    c"type".as_ptr(),
                    kCFStringEncodingUTF8,
                )
            };
            let subtype_key = unsafe {
                CFStringCreateWithCString(
                    ptr::null(),
                    c"subtype".as_ptr(),
                    kCFStringEncodingUTF8,
                )
            };
            let manufacturer_key = unsafe {
                CFStringCreateWithCString(
                    ptr::null(),
                    c"manufacturer".as_ptr(),
                    kCFStringEncodingUTF8,
                )
            };
            let version_key = unsafe {
                CFStringCreateWithCString(
                    ptr::null(),
                    c"version".as_ptr(),
                    kCFStringEncodingUTF8,
                )
            };
            let name_key = unsafe {
                CFStringCreateWithCString(
                    ptr::null(),
                    c"name".as_ptr(),
                    kCFStringEncodingUTF8,
                )
            };
            let state_key = unsafe {
                CFStringCreateWithCString(
                    ptr::null(),
                    c"nice-plug-state".as_ptr(),
                    kCFStringEncodingUTF8,
                )
            };

            if type_key.is_null()
                || subtype_key.is_null()
                || manufacturer_key.is_null()
                || version_key.is_null()
                || name_key.is_null()
                || state_key.is_null()
            {
                bridge::nice_au2_free_state(state_ptr);
                for object in [
                    type_key,
                    subtype_key,
                    manufacturer_key,
                    version_key,
                    name_key,
                    state_key,
                ] {
                    if !object.is_null() {
                        unsafe { CFRelease(object.cast()) };
                    }
                }
                return kAudioUnitErr_InvalidPropertyValue;
            }

            // AU ClassInfo stores the component identity as CFNumbers whose
            // values are the 32-bit AudioComponent FourCCs.
            const CF_NUMBER_SINT32_TYPE: isize = 3;
            let component_type = u32::from_be_bytes(*b"aufx") as i32;
            let component_subtype = u32::from_be_bytes(*b"GSfx") as i32;
            let component_manufacturer = u32::from_be_bytes(*b"BrTC") as i32;
            // AudioComponent version encoded as 0xMMMMmmdd. The component
            // currently advertises 0.0.1, so keep ClassInfo consistent.
            let component_version: i32 = 0x0000_0001;

            let type_value = unsafe {
                CFNumberCreate(
                    ptr::null(),
                    CF_NUMBER_SINT32_TYPE,
                    (&raw const component_type).cast(),
                )
            };
            let subtype_value = unsafe {
                CFNumberCreate(
                    ptr::null(),
                    CF_NUMBER_SINT32_TYPE,
                    (&raw const component_subtype).cast(),
                )
            };
            let manufacturer_value = unsafe {
                CFNumberCreate(
                    ptr::null(),
                    CF_NUMBER_SINT32_TYPE,
                    (&raw const component_manufacturer).cast(),
                )
            };
            let version_value = unsafe {
                CFNumberCreate(
                    ptr::null(),
                    CF_NUMBER_SINT32_TYPE,
                    (&raw const component_version).cast(),
                )
            };
            let name_value = unsafe {
                CFStringCreateWithCString(
                    ptr::null(),
                    c"GainStageFx".as_ptr(),
                    kCFStringEncodingUTF8,
                )
            };
            let state_value = unsafe {
                CFDataCreate(
                    ptr::null(),
                    state_ptr.cast_const(),
                    state_size as isize,
                )
            };
            bridge::nice_au2_free_state(state_ptr);

            if type_value.is_null()
                || subtype_value.is_null()
                || manufacturer_value.is_null()
                || version_value.is_null()
                || name_value.is_null()
                || state_value.is_null()
            {
                for object in [
                    type_key,
                    subtype_key,
                    manufacturer_key,
                    version_key,
                    name_key,
                    state_key,
                ] {
                    unsafe { CFRelease(object.cast()) };
                }
                for object in [
                    type_value,
                    subtype_value,
                    manufacturer_value,
                    version_value,
                    name_value.cast::<c_void>() as *const c_void,
                    state_value,
                ] {
                    if !object.is_null() {
                        unsafe { CFRelease(object) };
                    }
                }
                return kAudioUnitErr_InvalidPropertyValue;
            }

            let keys = [
                type_key.cast::<c_void>() as *const c_void,
                subtype_key.cast::<c_void>() as *const c_void,
                manufacturer_key.cast::<c_void>() as *const c_void,
                version_key.cast::<c_void>() as *const c_void,
                name_key.cast::<c_void>() as *const c_void,
                state_key.cast::<c_void>() as *const c_void,
            ];
            let values = [
                type_value,
                subtype_value,
                manufacturer_value,
                version_value,
                name_value.cast::<c_void>() as *const c_void,
                state_value,
            ];

            let dictionary = unsafe {
                CFDictionaryCreate(
                    ptr::null(),
                    keys.as_ptr(),
                    values.as_ptr(),
                    keys.len() as isize,
                    (&raw const kCFTypeDictionaryKeyCallBacks).cast(),
                    (&raw const kCFTypeDictionaryValueCallBacks).cast(),
                )
            };

            // The dictionary retains its keys and values through the CFType
            // callbacks. ClassInfo itself is returned retained to the host.
            for object in [
                type_key,
                subtype_key,
                manufacturer_key,
                version_key,
                name_key,
                state_key,
            ] {
                unsafe { CFRelease(object.cast()) };
            }
            for object in [
                type_value,
                subtype_value,
                manufacturer_value,
                version_value,
                name_value.cast::<c_void>() as *const c_void,
                state_value,
            ] {
                unsafe { CFRelease(object) };
            }

            if dictionary.is_null() {
                return kAudioUnitErr_InvalidPropertyValue;
            }
            unsafe { copy_out(&dictionary, output, io_size) };
        }
        kAudioUnitProperty_StreamFormat => unsafe {
            copy_out(
                if scope == kAudioUnitScope_Input {
                    &component.input_format
                } else {
                    &component.output_format
                },
                output,
                io_size,
            )
        },
        kAudioUnitProperty_SampleRate => unsafe {
            copy_out(&component.sample_rate, output, io_size)
        },
        kAudioUnitProperty_ElementCount => {
            let count = if scope == kAudioUnitScope_Input {
                component.bus_config.input_bus_count
            } else if scope == kAudioUnitScope_Output {
                component.bus_config.output_bus_count
            } else {
                1
            };
            unsafe { copy_out(&count, output, io_size) };
        }
        kAudioUnitProperty_MaximumFramesPerSlice => unsafe {
            copy_out(&component.max_frames, output, io_size)
        },
        kAudioUnitProperty_SupportedNumChannels => {
            let channels = AUChannelInfo {
                inChannels: component.input_channels() as i16,
                outChannels: component.output_channels() as i16,
            };
            unsafe { copy_out(&channels, output, io_size) };
        }
        kAudioUnitProperty_MakeConnection => unsafe {
            copy_out(&component.input_connection, output, io_size)
        },
        kAudioUnitProperty_ParameterList => {
            let count = bridge::nice_au2_get_parameter_count(component.rust_instance);
            let capacity = if io_size.is_null() {
                count
            } else {
                (unsafe { *io_size }) / 4
            };
            for index in 0..count.min(capacity) {
                let mut parameter: NiceAu2ParameterInfo = unsafe { zeroed() };
                if bridge::nice_au2_get_parameter_info(
                    component.rust_instance,
                    index,
                    &mut parameter,
                ) {
                    unsafe { *output.cast::<u32>().add(index as usize) = parameter.id };
                }
            }
            if !io_size.is_null() {
                unsafe { *io_size = count * 4 };
            }
        }
        kAudioUnitProperty_ParameterInfo => {
            let count = bridge::nice_au2_get_parameter_count(component.rust_instance);
            let mut parameter: NiceAu2ParameterInfo = unsafe { zeroed() };
            let found = (0..count).any(|index| {
                bridge::nice_au2_get_parameter_info(component.rust_instance, index, &mut parameter)
                    && parameter.id == element
            });
            if !found {
                return kAudioUnitErr_InvalidParameter;
            }
            let mut result: AudioUnitParameterInfo = unsafe { zeroed() };
            for (destination, source) in result.name.iter_mut().zip(parameter.name) {
                *destination = source;
            }
            result.unit = kAudioUnitParameterUnit_Generic;
            result.minValue = parameter.min_value;
            result.maxValue = parameter.max_value;
            result.defaultValue = parameter.default_value;
            result.flags = kAudioUnitParameterFlag_IsReadable | kAudioUnitParameterFlag_IsWritable;
            unsafe { copy_out(&result, output, io_size) };
        }
        kAudioUnitProperty_Latency | kAudioUnitProperty_TailTime => {
            let samples = if property == kAudioUnitProperty_Latency {
                bridge::nice_au2_get_latency_samples(component.rust_instance)
            } else {
                bridge::nice_au2_get_tail_samples(component.rust_instance)
            };
            let seconds = if component.sample_rate > 0.0 {
                samples as f64 / component.sample_rate
            } else {
                0.0
            };
            unsafe { copy_out(&seconds, output, io_size) };
        }
        RUST_INSTANCE_PROPERTY => unsafe {
            copy_out(&component.rust_instance.cast::<c_void>(), output, io_size)
        },
        kAudioUnitProperty_CocoaUI => return unsafe { cocoa_view_info(output, io_size) },
        MIDI_OUTPUT_CALLBACK_INFO => {
            if !io_size.is_null() {
                unsafe { *io_size = 0 };
            }
        }
        MIDI_OUTPUT_CALLBACK => unsafe {
            copy_out(&component.midi_output_callback, output, io_size)
        },
        HOST_CALLBACKS => unsafe { copy_out(&component.host_callbacks, output, io_size) },
        kAudioUnitProperty_PresentPreset => {
            let name = unsafe {
                CFStringCreateWithCString(ptr::null(), c"Default".as_ptr(), kCFStringEncodingUTF8)
            };
            if name.is_null() {
                return kAudioUnitErr_InvalidPropertyValue;
            }
            let preset = AUPreset {
                presetNumber: -1,
                presetName: name,
            };
            unsafe { copy_out(&preset, output, io_size) };
        }
        _ => return kAudioUnitErr_InvalidProperty,
    }
    0
}

unsafe fn cocoa_view_info(output: *mut c_void, io_size: *mut u32) -> OSStatus {
    let mut info: libc::Dl_info = unsafe { zeroed() };
    if unsafe { libc::dladdr(get as *const () as *const c_void, &mut info) } == 0
        || info.dli_fname.is_null()
    {
        return kAudioUnitErr_InvalidProperty;
    }
    let Ok(path) = unsafe { CStr::from_ptr(info.dli_fname) }.to_str() else {
        return kAudioUnitErr_InvalidProperty;
    };
    let Some(bundle) = std::path::Path::new(path).ancestors().nth(3) else {
        return kAudioUnitErr_InvalidProperty;
    };
    let Some(bundle) = bundle
        .to_str()
        .and_then(|value| std::ffi::CString::new(value).ok())
    else {
        return kAudioUnitErr_InvalidProperty;
    };
    let path_string =
        unsafe { CFStringCreateWithCString(ptr::null(), bundle.as_ptr(), kCFStringEncodingUTF8) };
    if path_string.is_null() {
        return kAudioUnitErr_InvalidProperty;
    }
    let url = unsafe {
        CFURLCreateWithFileSystemPath(ptr::null(), path_string, kCFURLPOSIXPathStyle.into(), 1)
    };
    unsafe { CFRelease(path_string.cast()) };
    if url.is_null() {
        return kAudioUnitErr_InvalidProperty;
    }
    let class = unsafe {
        CFStringCreateWithCString(
            ptr::null(),
            c"NiceAu2CocoaViewFactory".as_ptr(),
            kCFStringEncodingUTF8,
        )
    };
    let mut result: AudioUnitCocoaViewInfo = unsafe { zeroed() };
    result.mCocoaAUViewBundleLocation = url;
    result.mCocoaAUViewClass[0] = class;
    unsafe { copy_out(&result, output, io_size) };
    0
}

fn stream_format_is_supported(
    component: &Component,
    scope: u32,
    format: &AudioStreamBasicDescription,
) -> bool {
    if format.mFormatID != kAudioFormatLinearPCM
        || format.mBitsPerChannel != 32
        || !format.mSampleRate.is_finite()
        || format.mSampleRate <= 0.0
    {
        return false;
    }

    // This AU advertises the concrete main-bus layout returned by nice-plug.
    // A host must not be allowed to set a different channel count and then
    // initialize successfully, otherwise kAudioUnitProperty_SupportedNumChannels
    // and the actual stream formats contradict each other.
    let required_channels = if scope == kAudioUnitScope_Input {
        component.input_channels()
    } else {
        component.output_channels()
    };

    format.mChannelsPerFrame == required_channels
}

pub unsafe extern "C" fn set(
    this: *mut c_void,
    property: u32,
    scope: u32,
    element: u32,
    input: *const c_void,
    bytes: u32,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidProperty;
    };
    if input.is_null() || !valid_scope(property, scope) {
        return kAudioUnitErr_InvalidProperty;
    }
    match property {
        kAudioUnitProperty_ClassInfo if bytes as usize >= size_of::<*mut c_void>() => {
            // The setter receives a pointer to the CFPropertyListRef, not the
            // dictionary object inline.
            let class_info = unsafe { *input.cast::<*const c_void>() };
            if class_info.is_null()
                || unsafe { CFGetTypeID(class_info) } != unsafe { CFDictionaryGetTypeID() }
            {
                return kAudioUnitErr_InvalidPropertyValue;
            }

            let key = unsafe {
                CFStringCreateWithCString(
                    ptr::null(),
                    c"nice-plug-state".as_ptr(),
                    kCFStringEncodingUTF8,
                )
            };
            if key.is_null() {
                return kAudioUnitErr_InvalidPropertyValue;
            }

            let data = unsafe { CFDictionaryGetValue(class_info, key.cast()) };
            unsafe { CFRelease(key.cast()) };
            if data.is_null()
                || unsafe { CFGetTypeID(data) } != unsafe { CFDataGetTypeID() }
            {
                return kAudioUnitErr_InvalidPropertyValue;
            }

            let length = unsafe { CFDataGetLength(data) };
            if length < 0 || length > u32::MAX as isize {
                return kAudioUnitErr_InvalidPropertyValue;
            }
            let state_ptr = unsafe { CFDataGetBytePtr(data) };
            if state_ptr.is_null() && length != 0 {
                return kAudioUnitErr_InvalidPropertyValue;
            }

            let status = bridge::nice_au2_load_state(
                component.rust_instance,
                state_ptr,
                length as u32,
            );
            if status != 0 {
                return status;
            }
        }
        kAudioUnitProperty_StreamFormat
            if bytes as usize >= size_of::<AudioStreamBasicDescription>() =>
        {
            let format = unsafe { *input.cast::<AudioStreamBasicDescription>() };
            if !stream_format_is_supported(component, scope, &format) {
                return kAudioUnitErr_FormatNotSupported;
            }

            component.sample_rate = format.mSampleRate;
            if scope == kAudioUnitScope_Input {
                component.input_format = format;
            } else {
                component.output_format = format;
            }
        }
        kAudioUnitProperty_SampleRate if bytes as usize >= size_of::<f64>() => {
            let sample_rate = unsafe { *input.cast::<f64>() };
            if !sample_rate.is_finite() || sample_rate <= 0.0 {
                return kAudioUnitErr_InvalidPropertyValue;
            }
            component.sample_rate = sample_rate;
            component.update_formats();
        }
        kAudioUnitProperty_MaximumFramesPerSlice if bytes as usize >= 4 => {
            component.max_frames = unsafe { *input.cast() }
        }
        kAudioUnitProperty_SetRenderCallback
            if bytes as usize >= size_of::<AURenderCallbackStruct>() =>
        {
            component.input_callback = unsafe { *input.cast() };
            component.input_connection = unsafe { zeroed() };
        }
        MIDI_OUTPUT_CALLBACK
            if bytes as usize >= size_of::<super::component::MidiOutputCallback>() =>
        {
            component.midi_output_callback = unsafe { *input.cast() };
        }
        HOST_CALLBACKS if bytes as usize >= size_of::<super::component::HostCallbacks>() => {
            component.host_callbacks = unsafe { *input.cast() };
        }
        kAudioUnitProperty_MakeConnection if bytes as usize >= size_of::<AudioUnitConnection>() => {
            component.input_connection = unsafe { *input.cast() };
            component.input_callback = unsafe { zeroed() };
        }
        kAudioUnitProperty_PresentPreset => return 0,
        _ => return kAudioUnitErr_InvalidProperty,
    }
    notify(component, property, scope, element);
    0
}

fn notify(component: &Component, property: u32, scope: u32, element: u32) {
    for listener in &component.property_listeners {
        if listener.property == property {
            if let Some(callback) = listener.callback {
                unsafe {
                    callback(
                        listener.user_data,
                        component.component_instance.cast(),
                        property,
                        scope,
                        element,
                    )
                };
            }
        }
    }
}

pub unsafe extern "C" fn add_listener(
    this: *mut c_void,
    property: u32,
    callback: AudioUnitPropertyListenerProc,
    user_data: *mut c_void,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidPropertyValue;
    };
    if callback.is_none() {
        return kAudioUnitErr_InvalidPropertyValue;
    }
    if !component.property_listeners.iter().any(|item| {
        item.property == property
            && property_callback_eq(item.callback, callback)
            && item.user_data == user_data
    }) {
        if component.property_listeners.len() >= 32 {
            return kAudioUnitErr_TooManyFramesToProcess;
        }
        component.property_listeners.push(PropertyListener {
            property,
            callback,
            user_data,
        });
    }
    0
}

pub unsafe extern "C" fn remove_listener(
    this: *mut c_void,
    property: u32,
    callback: AudioUnitPropertyListenerProc,
) -> OSStatus {
    unsafe { remove_listener_with_data(this, property, callback, ptr::null_mut()) }
}

pub unsafe extern "C" fn remove_listener_with_data(
    this: *mut c_void,
    property: u32,
    callback: AudioUnitPropertyListenerProc,
    user_data: *mut c_void,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidPropertyValue;
    };
    component.property_listeners.retain(|item| {
        !(item.property == property
            && property_callback_eq(item.callback, callback)
            && (user_data.is_null() || item.user_data == user_data))
    });
    0
}

pub unsafe extern "C" fn add_render_notify(
    this: *mut c_void,
    callback: AURenderCallback,
    user_data: *mut c_void,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidPropertyValue;
    };
    if callback.is_none() {
        return kAudioUnitErr_InvalidPropertyValue;
    }
    if !component
        .render_notifies
        .iter()
        .any(|item| render_callback_eq(item.callback, callback) && item.user_data == user_data)
    {
        component.render_notifies.push(RenderNotify {
            callback,
            user_data,
        });
    }
    0
}

pub unsafe extern "C" fn remove_render_notify(
    this: *mut c_void,
    callback: AURenderCallback,
    user_data: *mut c_void,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidPropertyValue;
    };
    component.render_notifies.retain(|item| {
        !(render_callback_eq(item.callback, callback) && item.user_data == user_data)
    });
    0
}
