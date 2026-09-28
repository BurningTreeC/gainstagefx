#![allow(non_snake_case)]

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioTimeStamp {
    pub mFlags: u32,
    pub mSampleTime: f64,
    pub mHostTime: u64,
    pub mOffset: u64,
    pub mDuration: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioBuffer {
    pub mNumberChannels: u32,
    pub mDataByteSize: u32,
    pub mData: *mut std::ffi::c_void,
}

#[path = "render_buffer.rs"]
mod buffer;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioBufferList {
    pub mNumberBuffers: u32,
    pub mBuffers: AudioBuffer,
}

#[derive(Debug, Clone, Copy)]
pub enum AURenderEventType {
    Parameter,
    MIDISysEx,
    MIDI,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AURenderEvent {
    pub event_type: AURenderEventType,
    pub sample_offset: u32,
    pub flag: u32,
    pub reserved: u64,
    pub next: *const AURenderEvent,
    pub parameter: Au2ParameterEvent,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Au2ParameterEvent {
    pub parameter_address: u32,
    pub reserved: u32,
    pub sample_offset: u32,
    pub value: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Au2ParameterRampEvent {
    pub parameter_address: u32,
    pub reserved: u32,
    pub sample_offset: u32,
    pub duration_samples: u32,
    pub start_value: f32,
    pub end_value: f32,
}

/// A host parameter change queued for a specific sample in the next render block.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Au2ScheduledParameterEvent {
    pub parameter_address: u32,
    pub sample_offset: u32,
    pub duration_samples: u32,
    pub start_value: f32,
    pub end_value: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Au2MidiEvent {
    pub status: u8,
    pub data1: u8,
    pub data2: u8,
    pub sample_offset: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Au2TransportInfo {
    pub sample_rate: f32,
    pub sample_position: Option<i64>,
    pub playing: Option<bool>,
    pub recording: Option<bool>,
    pub tempo: Option<f64>,
    pub position_beats: Option<f64>,
    pub time_signature: Option<(i32, i32)>,
    pub cycle_beats: Option<(f64, f64)>,
}

pub struct Au2ProcessEvents<'a> {
    pub midi_events: &'a [Au2MidiEvent],
    pub transport: Au2TransportInfo,
    pub midi_output: &'a mut Vec<Au2MidiEvent>,
}
