use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use coreaudio_sys::*;

use crate::bridge::{NiceAu2BusConfig, NiceAu2BusInfo, NiceAu2BusType};
use crate::render::Au2ScheduledParameterEvent;

use super::selectors;

const MAX_CHANNELS: usize = 32;
const MAX_MIDI_EVENTS: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct MidiOutputCallback {
    pub callback: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const AudioTimeStamp,
            u32,
            *const MidiPacketList,
        ) -> OSStatus,
    >,
    pub user_data: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct MidiPacket {
    pub timestamp: u64,
    pub length: u16,
    pub data: [u8; 256],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct MidiPacketList {
    pub num_packets: u32,
    pub packet: MidiPacket,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct HostCallbacks {
    pub user_data: *mut c_void,
    pub beat_and_tempo: Option<unsafe extern "C" fn(*mut c_void, *mut f64, *mut f64) -> OSStatus>,
    pub musical_time: Option<
        unsafe extern "C" fn(*mut c_void, *mut u32, *mut f32, *mut u32, *mut f64) -> OSStatus,
    >,
    pub transport: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *mut u8,
            *mut u8,
            *mut f64,
            *mut u8,
            *mut f64,
            *mut f64,
        ) -> OSStatus,
    >,
    pub transport2: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *mut u8,
            *mut u8,
            *mut u8,
            *mut f64,
            *mut u8,
            *mut f64,
            *mut f64,
        ) -> OSStatus,
    >,
}

#[derive(Clone, Copy, Default)]
pub(super) struct PropertyListener {
    pub property: AudioUnitPropertyID,
    pub callback: AudioUnitPropertyListenerProc,
    pub user_data: *mut c_void,
}

#[derive(Clone, Copy, Default)]
pub(super) struct RenderNotify {
    pub callback: AURenderCallback,
    pub user_data: *mut c_void,
}

#[repr(C)]
pub(super) struct Component {
    pub interface: AudioComponentPlugInInterface,
    pub rust_instance: *mut crate::bridge::NiceInstanceHandle,
    pub component_instance: AudioComponentInstance,
    pub bus_config: NiceAu2BusConfig,
    pub input_format: AudioStreamBasicDescription,
    pub output_format: AudioStreamBasicDescription,
    pub sample_rate: f64,
    pub max_frames: u32,
    pub render_resources_allocated: bool,
    pub input_callback: AURenderCallbackStruct,
    pub input_connection: AudioUnitConnection,
    pub input_buffers: [Vec<f32>; MAX_CHANNELS],
    pub output_buffers: [Vec<f32>; MAX_CHANNELS],
    pub property_listeners: Vec<PropertyListener>,
    pub render_notifies: Vec<RenderNotify>,
    pub scheduled_events: [Au2ScheduledParameterEvent; 256],
    pub scheduled_event_count: AtomicU32,
    pub midi_events: UnsafeCell<[crate::render::Au2MidiEvent; MAX_MIDI_EVENTS]>,
    pub midi_event_count: AtomicU32,
    pub midi_output_callback: MidiOutputCallback,
    pub host_callbacks: HostCallbacks,
}

impl Component {
    pub fn allocate() -> *mut AudioComponentPlugInInterface {
        let component = Box::new(Self {
            interface: AudioComponentPlugInInterface {
                Open: Some(selectors::open),
                Close: Some(selectors::close),
                Lookup: Some(selectors::lookup),
                reserved: std::ptr::null_mut(),
            },
            rust_instance: std::ptr::null_mut(),
            component_instance: std::ptr::null_mut(),
            bus_config: NiceAu2BusConfig {
                input_bus_count: 0,
                output_bus_count: 0,
                input_buses: [NiceAu2BusInfo {
                    channel_count: 0,
                    bus_type: NiceAu2BusType::Main,
                }; 16],
                output_buses: [NiceAu2BusInfo {
                    channel_count: 0,
                    bus_type: NiceAu2BusType::Main,
                }; 16],
            },
            input_format: unsafe { std::mem::zeroed() },
            output_format: unsafe { std::mem::zeroed() },
            sample_rate: 44_100.0,
            max_frames: 1024,
            render_resources_allocated: false,
            input_callback: unsafe { std::mem::zeroed() },
            input_connection: unsafe { std::mem::zeroed() },
            input_buffers: std::array::from_fn(|_| Vec::new()),
            output_buffers: std::array::from_fn(|_| Vec::new()),
            property_listeners: Vec::new(),
            render_notifies: Vec::new(),
            scheduled_events: [Au2ScheduledParameterEvent::default(); 256],
            scheduled_event_count: AtomicU32::new(0),
            midi_events: UnsafeCell::new([crate::render::Au2MidiEvent::default(); MAX_MIDI_EVENTS]),
            midi_event_count: AtomicU32::new(0),
            midi_output_callback: MidiOutputCallback::default(),
            host_callbacks: HostCallbacks::default(),
        });
        Box::into_raw(component).cast()
    }

    pub fn queue_midi_event(&self, event: crate::render::Au2MidiEvent) -> OSStatus {
        let index = self.midi_event_count.fetch_add(1, Ordering::AcqRel) as usize;
        if index >= MAX_MIDI_EVENTS {
            self.midi_event_count
                .store(MAX_MIDI_EVENTS as u32, Ordering::Release);
            return kAudioUnitErr_TooManyFramesToProcess;
        }
        unsafe { (*self.midi_events.get())[index] = event };
        0
    }

    pub fn take_midi_events(&self) -> &[crate::render::Au2MidiEvent] {
        let count = self
            .midi_event_count
            .swap(0, Ordering::AcqRel)
            .min(MAX_MIDI_EVENTS as u32) as usize;
        unsafe { std::slice::from_raw_parts((*self.midi_events.get()).as_ptr(), count) }
    }

    pub unsafe fn from_self<'a>(this: *mut c_void) -> Option<&'a mut Self> {
        unsafe { this.cast::<Self>().as_mut() }
    }

    pub fn input_channels(&self) -> u32 {
        self.bus_config.input_buses[..self.bus_config.input_bus_count as usize]
            .iter()
            .map(|bus| bus.channel_count)
            .sum()
    }

    pub fn output_channels(&self) -> u32 {
        self.bus_config.output_buses[..self.bus_config.output_bus_count as usize]
            .iter()
            .map(|bus| bus.channel_count)
            .sum()
    }

    pub fn update_formats(&mut self) {
        self.input_format = make_format(self.sample_rate, self.input_channels());
        self.output_format = make_format(self.sample_rate, self.output_channels());
    }
}

fn make_format(sample_rate: f64, channels: u32) -> AudioStreamBasicDescription {
    AudioStreamBasicDescription {
        mSampleRate: sample_rate,
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagsNativeFloatPacked | kAudioFormatFlagIsNonInterleaved,
        mBytesPerPacket: 4,
        mFramesPerPacket: 1,
        mBytesPerFrame: 4,
        mChannelsPerFrame: channels,
        mBitsPerChannel: 32,
        mReserved: 0,
    }
}
