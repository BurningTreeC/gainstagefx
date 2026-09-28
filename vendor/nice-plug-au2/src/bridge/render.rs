use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::Ordering;

use coreaudio_sys::*;

use crate::bridge;

use super::component::Component;

#[repr(C)]
struct BufferList32 {
    count: u32,
    buffers: [AudioBuffer; 32],
}

impl BufferList32 {
    fn new() -> Self {
        unsafe { std::mem::zeroed() }
    }
    fn as_mut_ptr(&mut self) -> *mut AudioBufferList {
        (self as *mut Self).cast()
    }
}

unsafe fn notify(
    component: &Component,
    flags: u32,
    timestamp: *const AudioTimeStamp,
    bus: u32,
    frames: u32,
    data: *mut AudioBufferList,
) -> OSStatus {
    for item in &component.render_notifies {
        if let Some(callback) = item.callback {
            let mut callback_flags = flags;
            let status = unsafe {
                callback(
                    item.user_data,
                    &mut callback_flags,
                    timestamp,
                    bus,
                    frames,
                    data,
                )
            };
            if status != 0 {
                return status;
            }
        }
    }
    0
}

pub unsafe extern "C" fn render(
    this: *mut c_void,
    flags: *mut u32,
    timestamp: *const AudioTimeStamp,
    output_bus: u32,
    frames: u32,
    io_data: *mut AudioBufferList,
) -> OSStatus {
    let Some(component) = (unsafe { Component::from_self(this) }) else {
        return kAudioUnitErr_InvalidProperty;
    };
    if frames > component.max_frames {
        return kAudioUnitErr_TooManyFramesToProcess;
    }

    let output_channels = component.output_channels().min(32) as usize;
    let mut output_storage = BufferList32::new();
    let output = if io_data.is_null() {
        output_storage.count = output_channels as u32;
        for (index, buffer) in component
            .output_buffers
            .iter_mut()
            .take(output_channels)
            .enumerate()
        {
            output_storage.buffers[index] = AudioBuffer {
                mNumberChannels: 1,
                mDataByteSize: frames * size_of::<f32>() as u32,
                mData: buffer.as_mut_ptr().cast(),
            };
        }
        output_storage.as_mut_ptr()
    } else {
        let list = unsafe { &mut *io_data };
        let buffers = unsafe {
            std::slice::from_raw_parts_mut(
                list.mBuffers.as_mut_ptr(),
                list.mNumberBuffers.min(output_channels as u32) as usize,
            )
        };
        for (index, buffer) in buffers.iter_mut().enumerate() {
            if buffer.mData.is_null() {
                buffer.mNumberChannels = 1;
                buffer.mDataByteSize = frames * size_of::<f32>() as u32;
                buffer.mData = component.output_buffers[index].as_mut_ptr().cast();
            }
        }
        io_data
    };

    let input_channels = component.input_channels().min(32) as usize;
    let has_callback = component.input_callback.inputProc.is_some();
    let has_connection = !component.input_connection.sourceAudioUnit.is_null();
    let mut input_storage = BufferList32::new();
    let input = if input_channels > 0 && (has_callback || has_connection) {
        input_storage.count = input_channels as u32;
        for (index, buffer) in component
            .input_buffers
            .iter_mut()
            .take(input_channels)
            .enumerate()
        {
            input_storage.buffers[index] = AudioBuffer {
                mNumberChannels: 1,
                mDataByteSize: frames * size_of::<f32>() as u32,
                mData: buffer.as_mut_ptr().cast(),
            };
        }
        let input = input_storage.as_mut_ptr();
        let status = if let Some(callback) = component.input_callback.inputProc {
            unsafe {
                callback(
                    component.input_callback.inputProcRefCon,
                    flags,
                    timestamp,
                    0,
                    frames,
                    input,
                )
            }
        } else {
            unsafe {
                AudioUnitRender(
                    component.input_connection.sourceAudioUnit,
                    flags,
                    timestamp,
                    component.input_connection.sourceOutputNumber,
                    frames,
                    input,
                )
            }
        };
        if status != 0 {
            return status;
        }
        input
    } else {
        std::ptr::null_mut()
    };

    let base_flags = if flags.is_null() {
        0
    } else {
        unsafe { *flags }
    };
    let status = unsafe {
        notify(
            component,
            base_flags | kAudioUnitRenderAction_PreRender,
            timestamp,
            output_bus,
            frames,
            output,
        )
    };
    if status != 0 {
        return status;
    }
    let event_count = component.scheduled_event_count.swap(0, Ordering::AcqRel);
    let midi_events = component.take_midi_events();
    let timestamp_value = if timestamp.is_null() {
        None
    } else {
        Some(unsafe { &*timestamp })
    };
    let mut transport = crate::render::Au2TransportInfo {
        sample_rate: component.sample_rate as f32,
        sample_position: timestamp_value
            .filter(|value| value.mFlags & kAudioTimeStampSampleTimeValid != 0)
            .map(|value| value.mSampleTime.round() as i64),
        playing: None,
        recording: None,
        tempo: None,
        position_beats: None,
        time_signature: None,
        cycle_beats: None,
    };
    let callbacks = component.host_callbacks;
    if let Some(callback) = callbacks.beat_and_tempo {
        let mut beat = 0.0;
        let mut tempo = 0.0;
        if unsafe { callback(callbacks.user_data, &mut beat, &mut tempo) } == 0 {
            transport.position_beats = Some(beat);
            transport.tempo = Some(tempo);
        }
    }
    if let Some(callback) = callbacks.musical_time {
        let mut delta = 0;
        let mut numerator = 0.0;
        let mut denominator = 0;
        let mut downbeat = 0.0;
        if unsafe {
            callback(
                callbacks.user_data,
                &mut delta,
                &mut numerator,
                &mut denominator,
                &mut downbeat,
            )
        } == 0
        {
            transport.time_signature = Some((numerator as i32, denominator as i32));
        }
    }
    if let Some(callback) = callbacks.transport2 {
        let mut playing = 0;
        let mut recording = 0;
        let mut changed = 0;
        let mut sample_position = transport.sample_position.unwrap_or_default() as f64;
        let mut cycling = 0;
        let mut cycle_start = 0.0;
        let mut cycle_end = 0.0;
        let status = unsafe {
            callback(
                callbacks.user_data,
                &mut playing,
                &mut recording,
                &mut changed,
                &mut sample_position,
                &mut cycling,
                &mut cycle_start,
                &mut cycle_end,
            )
        };
        if status == 0 {
            transport.playing = Some(playing != 0);
            transport.recording = callbacks.transport2.map(|_| recording != 0);
            transport.sample_position = Some(sample_position.round() as i64);
            transport.cycle_beats = (cycling != 0).then_some((cycle_start, cycle_end));
        }
    } else if let Some(callback) = callbacks.transport {
        let mut playing = 0;
        let mut changed = 0;
        let mut sample_position = transport.sample_position.unwrap_or_default() as f64;
        let mut cycling = 0;
        let mut cycle_start = 0.0;
        let mut cycle_end = 0.0;
        if unsafe {
            callback(
                callbacks.user_data,
                &mut playing,
                &mut changed,
                &mut sample_position,
                &mut cycling,
                &mut cycle_start,
                &mut cycle_end,
            )
        } == 0
        {
            transport.playing = Some(playing != 0);
            transport.sample_position = Some(sample_position.round() as i64);
            transport.cycle_beats = (cycling != 0).then_some((cycle_start, cycle_end));
        }
    }
    let mut midi_output = Vec::with_capacity(32);
    let mut status = bridge::nice_au2_render(
        component.rust_instance,
        flags,
        timestamp.cast(),
        frames,
        output_bus as isize,
        output.cast(),
        midi_events.as_ptr(),
        midi_events.len() as u32,
        std::ptr::null(),
        input.cast(),
        component.scheduled_events.as_ptr(),
        event_count,
        &mut midi_output,
        &transport,
    );
    if let Some(callback) = component.midi_output_callback.callback {
        for event in midi_output {
            let mut packet = super::component::MidiPacketList {
                num_packets: 1,
                packet: super::component::MidiPacket {
                    timestamp: timestamp_value.map_or(0, |value| value.mHostTime),
                    length: 3,
                    data: [0; 256],
                },
            };
            packet.packet.data[..3].copy_from_slice(&[event.status, event.data1, event.data2]);
            let callback_status = unsafe {
                callback(
                    component.midi_output_callback.user_data,
                    timestamp,
                    1,
                    &packet,
                )
            };
            if status == 0 && callback_status != 0 {
                status = callback_status;
            }
        }
    }
    let post_flags = base_flags
        | kAudioUnitRenderAction_PostRender
        | if status == 0 {
            0
        } else {
            kAudioUnitRenderAction_PostRenderError
        };
    let notify_status =
        unsafe { notify(component, post_flags, timestamp, output_bus, frames, output) };
    if status == 0 {
        status = notify_status;
    }
    status
}
