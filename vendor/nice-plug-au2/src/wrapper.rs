pub use crate::config::{Au2Category, Au2Config};

use crate::{audio_setup, config, error, factory, instance, render};

use nice_plug_core::audio_setup::AuxiliaryBuffers;
use nice_plug_core::buffer::Buffer;
use nice_plug_core::context::PluginApi;
use nice_plug_core::context::gui::{AsyncExecutor, GuiContext, GuiContextInner};
use nice_plug_core::context::process::{ProcessContext, Transport};
use nice_plug_core::editor::{Editor, EditorHandle, ParentWindowHandle};
use nice_plug_core::midi::{NoteEvent, PluginNoteEvent};
use nice_plug_core::params::InternalParamMut;
use nice_plug_core::params::Params;
use nice_plug_core::params::internals::ParamPtr;
use nice_plug_core::plugin::Plugin;
use nice_plug_core::plugin::{ParamValue, PluginState, ProcessStatus};
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Weak};

pub trait Au2Plugin: Plugin + 'static {
    const AU2_CATEGORY: Au2Category;
    const AU2_MANUFACTURER: [u8; 4];
    const AU2_SUBTYPE: [u8; 4];
    const AU2_NAME: &'static str = Self::NAME;

    fn au2_config() -> Au2Config {
        Au2Config::new(
            Self::AU2_CATEGORY,
            Self::AU2_MANUFACTURER,
            Self::AU2_SUBTYPE,
            Self::AU2_NAME,
        )
    }
}

#[repr(C)]
pub struct Au2ExportedMetadata {
    pub name_ptr: *const u8,
    pub name_len: usize,
    pub component_type: u32,
    pub sub_type: u32,
    pub manufacturer: u32,
}

static REGISTERED_PLUGIN: OnceLock<fn() -> Box<dyn instance::NicePluginInstance>> = OnceLock::new();
static REGISTERED_CONFIG: OnceLock<Au2Config> = OnceLock::new();

pub fn register_au2_plugin<P: Au2Plugin + 'static>() {
    let config = P::au2_config();
    let _ = REGISTERED_CONFIG.set(config);
    let _ = REGISTERED_PLUGIN
        .set(|| Box::new(NiceAu2Processor::<P>::new()) as Box<dyn instance::NicePluginInstance>);
    factory::register_factory(
        || Box::new(NiceAu2Processor::<P>::new()) as Box<dyn instance::NicePluginInstance>,
        P::au2_config(),
    );

    log::info!(
        "Registered AU plugin: {} ({}/{})",
        P::AU2_NAME,
        config::four_cc_string(P::AU2_MANUFACTURER),
        config::four_cc_string(P::AU2_SUBTYPE)
    );
}

pub struct NiceAu2Processor<P: Plugin + 'static> {
    plugin: P,
    params: Arc<dyn Params>,
    frontend: Arc<Au2Frontend<P>>,
    params_by_id: Vec<(String, ParamPtr, String)>,
    params_by_hash: HashMap<u32, ParamPtr>,
    param_id_to_hash: HashMap<String, u32>,
    pending_editor_notifications: Arc<AtomicBool>,
    current_audio_io_layout: nice_plug_core::audio_setup::AudioIOLayout,
    current_buffer_config: Option<nice_plug_core::audio_setup::BufferConfig>,
    prepared: bool,
    sample_rate: f64,
    max_frames: u32,
    latency_samples: Arc<AtomicU32>,
    process_buffer: Buffer<'static>,
    aux_input_buffers: Vec<Buffer<'static>>,
    aux_output_buffers: Vec<Buffer<'static>>,
    aux_input_storage: Vec<Vec<f32>>,
    midi_events: Vec<crate::render::Au2MidiEvent>,
    midi_output: Vec<crate::render::Au2MidiEvent>,
    transport_info: crate::render::Au2TransportInfo,
    process_offset: usize,
}
impl<P: Plugin + 'static> NiceAu2Processor<P> {
    pub fn new() -> Self {
        let mut plugin = P::default();
        let params = plugin.params();
        let editor = plugin.editor(AsyncExecutor::new(
            Arc::new(|_task| {}),
            Arc::new(|_task| {}),
        ));
        let params_by_id = params.param_map();
        let mut params_by_hash = HashMap::with_capacity(params_by_id.len());
        let mut param_id_to_hash = HashMap::with_capacity(params_by_id.len());
        let mut param_id_by_ptr = HashMap::with_capacity(params_by_id.len());

        for (param_id, param_ptr, _) in &params_by_id {
            let hash = hash_param_id(param_id);
            params_by_hash.insert(hash, *param_ptr);
            param_id_to_hash.insert(param_id.clone(), hash);
            param_id_by_ptr.insert(*param_ptr, param_id.clone());
        }

        let pending_editor_notifications = Arc::new(AtomicBool::new(false));
        let frontend = Arc::new(Au2Frontend {
            editor: Mutex::new(editor),
            opened: Mutex::new(Weak::new()),
            context: Arc::new(Au2GuiContext {
                params: params.clone(),
                params_by_id: params_by_id.clone(),
                params_by_hash: params_by_hash.clone(),
                param_id_to_hash: param_id_to_hash.clone(),
                param_id_by_ptr: param_id_by_ptr.clone(),
                pending_editor_notifications: pending_editor_notifications.clone(),
            }),
        });
        Self {
            plugin,
            params,
            frontend,
            params_by_id,
            params_by_hash,
            param_id_to_hash,
            pending_editor_notifications,
            current_audio_io_layout: P::AUDIO_IO_LAYOUTS
                .first()
                .copied()
                .unwrap_or_else(nice_plug_core::audio_setup::AudioIOLayout::const_default),
            current_buffer_config: None,
            prepared: false,
            sample_rate: 44100.0,
            max_frames: 512,
            latency_samples: Arc::new(AtomicU32::new(0)),
            process_buffer: Buffer::default(),
            aux_input_buffers: Vec::new(),
            aux_output_buffers: Vec::new(),
            aux_input_storage: Vec::new(),
            midi_events: Vec::with_capacity(1024),
            midi_output: Vec::with_capacity(1024),
            transport_info: crate::render::Au2TransportInfo {
                sample_rate: 44100.0,
                sample_position: None,
                playing: None,
                recording: None,
                tempo: None,
                position_beats: None,
                time_signature: None,
                cycle_beats: None,
            },
            process_offset: 0,
        }
    }

    fn queue_editor_param_changed(&mut self, param_hash: u32, normalized_value: f32) {
        let _ = (param_hash, normalized_value);
        self.pending_editor_notifications
            .store(true, Ordering::Release);
    }

    fn queue_editor_values_changed(&mut self) {
        queue_editor_values_changed(&self.pending_editor_notifications);
    }

    fn process_range(
        &mut self,
        input_ptrs: &[*const f32],
        output_ptrs: &[*mut f32],
        start: usize,
        len: usize,
    ) -> error::PluginResult<()> {
        if len == 0 {
            return Ok(());
        }
        self.process_offset = start;
        let mut inputs: [&[f32]; 32] = [&[]; 32];
        let mut outputs: [&mut [f32]; 32] = std::array::from_fn(|_| &mut [] as &mut [f32]);
        for (index, ptr) in input_ptrs.iter().enumerate() {
            inputs[index] = unsafe { std::slice::from_raw_parts(ptr.add(start), len) };
        }
        for (index, ptr) in output_ptrs.iter().enumerate() {
            outputs[index] = unsafe { std::slice::from_raw_parts_mut(ptr.add(start), len) };
        }
        <Self as instance::NicePluginInstance>::process(
            self,
            &inputs[..input_ptrs.len()],
            &mut outputs[..output_ptrs.len()],
            len,
        )
    }
}

fn queue_editor_values_changed(pending: &Arc<AtomicBool>) {
    pending.store(true, Ordering::Release);
}

// The editor and its notification pump are independent of the DSP mutex. A
// host may open or repaint the editor while the audio thread is processing.
struct Au2Frontend<P: Plugin> {
    editor: Mutex<Option<P::Editor>>,
    opened: Mutex<Weak<Mutex<<P::Editor as Editor>::Handle>>>,
    context: Arc<Au2GuiContext>,
}
struct OpenEditor<H: EditorHandle> {
    _handle: Arc<Mutex<H>>,
    _window: H::Window,
}
impl<P: Plugin> instance::NicePluginFrontend for Au2Frontend<P> {
    fn size(&self) -> Option<(u32, u32)> {
        let editor = self.editor.lock().ok()?;
        let size = editor.as_ref()?.size();
        Some((size.width, size.height))
    }
    fn spawn(&self, parent: *mut std::ffi::c_void) -> *mut std::ffi::c_void {
        if parent.is_null() {
            return std::ptr::null_mut();
        }
        let Ok(editor) = self.editor.lock() else {
            return std::ptr::null_mut();
        };
        let Some(editor) = editor.as_ref() else {
            return std::ptr::null_mut();
        };
        let host = editor_host(parent);
        let Ok(spawned) = editor.spawn(
            Some(ParentWindowHandle::AppKitNsView(
                std::ptr::NonNull::new(parent).unwrap(),
            )),
            false,
            None,
            GuiContext::new(self.context.clone()),
            host,
        ) else {
            return std::ptr::null_mut();
        };
        if spawned.handle.show(&spawned.window).is_err() {
            return std::ptr::null_mut();
        }
        let handle = Arc::new(Mutex::new(spawned.handle));
        *self.opened.lock().unwrap() = Arc::downgrade(&handle);
        let view = OpenEditor {
            _handle: handle,
            _window: spawned.window,
        };
        Box::into_raw(Box::new(instance::NiceAu2EditorHandle {
            handle: Box::new(view),
        }))
        .cast()
    }
    fn destroy(&self, handle: *mut std::ffi::c_void) {
        if !handle.is_null() {
            // Host editor lifecycle calls, including window destruction, run
            // on the main thread. The native window is never sent to audio.
            unsafe {
                drop(Box::from_raw(
                    handle.cast::<instance::NiceAu2EditorHandle>(),
                ));
            }
        }
    }
    fn flush(&self) {
        if self
            .context
            .pending_editor_notifications
            .swap(false, Ordering::AcqRel)
        {
            if let Some(handle) = self.opened.lock().unwrap().upgrade() {
                handle.lock().unwrap().state_changed();
            }
        }
    }
}

fn editor_host(_parent: *mut std::ffi::c_void) -> Option<nice_plug_core::editor::HostMethods> {
    None
}

impl<P: Plugin + 'static> Default for NiceAu2Processor<P> {
    fn default() -> Self {
        Self::new()
    }
}

struct Au2GuiContext {
    params: Arc<dyn Params>,
    params_by_id: Vec<(String, ParamPtr, String)>,
    params_by_hash: HashMap<u32, ParamPtr>,
    param_id_to_hash: HashMap<String, u32>,
    param_id_by_ptr: HashMap<ParamPtr, String>,
    pending_editor_notifications: Arc<AtomicBool>,
}

impl Au2GuiContext {
    fn state(&self) -> PluginState {
        let params = self
            .params_by_id
            .iter()
            .map(|(id, param, _)| {
                let value = unsafe {
                    match param {
                        ParamPtr::FloatParam(_) => ParamValue::F32(param.unmodulated_plain_value()),
                        ParamPtr::IntParam(_) => {
                            ParamValue::I32(param.unmodulated_plain_value() as i32)
                        }
                        ParamPtr::BoolParam(_) => {
                            ParamValue::Bool(param.unmodulated_normalized_value() >= 0.5)
                        }
                        ParamPtr::EnumParam(_) => {
                            ParamValue::I32(param.unmodulated_plain_value() as i32)
                        }
                    }
                };
                (id.clone(), value)
            })
            .collect::<BTreeMap<_, _>>();

        PluginState {
            version: String::new(),
            params,
            fields: self.params.serialize_fields(),
        }
    }
}

impl GuiContextInner for Au2GuiContext {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }

    fn request_restart(&self) {
        queue_editor_values_changed(&self.pending_editor_notifications);
    }

    unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}

    unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, normalized: f32) {
        if !self.param_id_by_ptr.contains_key(&param) || !normalized.is_finite() {
            return;
        }
        unsafe {
            param._internal_set_normalized_value(normalized.clamp(0.0, 1.0));
        }
        queue_editor_values_changed(&self.pending_editor_notifications);
    }

    unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}

    fn get_state(&self) -> PluginState {
        self.state()
    }

    fn set_state(&self, state: PluginState) {
        for (param_id, value) in &state.params {
            let Some(hash) = self.param_id_to_hash.get(param_id) else {
                continue;
            };
            let Some(param) = self.params_by_hash.get(hash).copied() else {
                continue;
            };

            unsafe {
                match (param, value) {
                    (ParamPtr::FloatParam(p), ParamValue::F32(v)) => {
                        (*p)._internal_set_plain_value(*v);
                    }
                    (ParamPtr::IntParam(p), ParamValue::I32(v)) => {
                        (*p)._internal_set_plain_value(*v);
                    }
                    (ParamPtr::BoolParam(_), ParamValue::Bool(v)) => {
                        param._internal_set_normalized_value(if *v { 1.0 } else { 0.0 });
                    }
                    (ParamPtr::EnumParam(p), ParamValue::I32(v)) => {
                        (*p)._internal_set_plain_value(*v);
                    }
                    (ParamPtr::EnumParam(p), ParamValue::String(id)) => {
                        (*p).set_from_id(id);
                    }
                    _ => {}
                }
            }
        }

        self.params.deserialize_fields(&state.fields);
        queue_editor_values_changed(&self.pending_editor_notifications);
    }
}

impl<P: Plugin + 'static> instance::NicePluginInstance for NiceAu2Processor<P> {
    fn allocate_render_resources(
        &mut self,
        audio_io_layout: &audio_setup::AudioIOLayout,
        buffer_config: &audio_setup::BufferConfig,
    ) -> error::PluginResult<()> {
        use nice_plug_core::audio_setup::BufferConfig as NiceBufferConfig;

        let input_channels = audio_io_layout.main_input_channels.map_or(0, |v| v.get());
        let output_channels = audio_io_layout.main_output_channels.map_or(0, |v| v.get());
        let nice_layout = P::AUDIO_IO_LAYOUTS
            .iter()
            .find(|layout| {
                let declared_inputs = layout.main_input_channels.map_or(0, |v| v.get())
                    + layout.aux_input_ports.iter().map(|v| v.get()).sum::<u32>();
                let declared_outputs = layout.main_output_channels.map_or(0, |v| v.get())
                    + layout.aux_output_ports.iter().map(|v| v.get()).sum::<u32>();
                declared_inputs == input_channels && declared_outputs == output_channels
            })
            .copied()
            .ok_or_else(|| error::PluginError::InitializationFailed(format!(
                "unsupported AU channel layout: {input_channels} inputs, {output_channels} outputs"
            )))?;

        let nice_buffer_config = NiceBufferConfig {
            sample_rate: buffer_config.sample_rate,
            min_buffer_size: buffer_config.min_buffer_size,
            max_buffer_size: buffer_config.max_buffer_size,
            process_mode: match buffer_config.process_mode {
                audio_setup::ProcessMode::Realtime => {
                    nice_plug_core::audio_setup::ProcessMode::Realtime
                }
                audio_setup::ProcessMode::Buffered => {
                    nice_plug_core::audio_setup::ProcessMode::Buffered
                }
                audio_setup::ProcessMode::Offline => {
                    nice_plug_core::audio_setup::ProcessMode::Offline
                }
            },
        };

        fn make_init_context<P: Plugin>(
            latency_samples: Arc<AtomicU32>,
        ) -> impl nice_plug_core::context::activate::ActivateContext<P> {
            struct NiceInitContext(Arc<AtomicU32>);
            impl<P: Plugin> nice_plug_core::context::activate::ActivateContext<P> for NiceInitContext {
                fn plugin_api(&self) -> nice_plug_core::context::PluginApi {
                    nice_plug_core::context::PluginApi::Standalone
                }
                fn execute(&self, _task: P::BackgroundTask) {}
                fn set_latency_samples(&self, samples: u32) {
                    self.0.store(samples, Ordering::Relaxed);
                }
                fn set_current_voice_capacity(&self, _capacity: u32) {}
            }
            NiceInitContext(latency_samples)
        }

        let mut ctx = make_init_context::<P>(self.latency_samples.clone());
        if !self
            .plugin
            .activate(&nice_layout, &nice_buffer_config, &mut ctx)
        {
            return Err(error::PluginError::InitializationFailed(
                "Plugin activate returned false".to_string(),
            ));
        }

        self.plugin.reset();
        let output_channels = nice_layout
            .main_output_channels
            .map_or(0, |v| v.get() as usize)
            + nice_layout
                .aux_output_ports
                .iter()
                .map(|v| v.get() as usize)
                .sum::<usize>();
        unsafe {
            self.process_buffer
                .set_slices(0, |slices| slices.reserve(output_channels));
        }
        self.aux_input_buffers.clear();
        self.aux_output_buffers.clear();
        self.aux_input_storage.clear();
        for channels in nice_layout.aux_input_ports {
            let channel_count = channels.get() as usize;
            let mut buffer = Buffer::default();
            unsafe { buffer.set_slices(0, |slices| slices.reserve(channel_count)) };
            self.aux_input_buffers.push(buffer);
            for _ in 0..channel_count {
                self.aux_input_storage
                    .push(vec![0.0; buffer_config.max_buffer_size as usize]);
            }
        }
        for channels in nice_layout.aux_output_ports {
            let mut buffer = Buffer::default();
            unsafe {
                buffer.set_slices(0, |slices| slices.reserve(channels.get() as usize));
            }
            self.aux_output_buffers.push(buffer);
        }
        for (_, param, _) in &self.params_by_id {
            unsafe {
                param._internal_update_smoother(buffer_config.sample_rate, true);
            }
        }

        self.current_audio_io_layout = nice_layout;
        self.current_buffer_config = Some(nice_buffer_config);
        self.prepared = true;
        self.sample_rate = buffer_config.sample_rate as f64;
        self.max_frames = buffer_config.max_buffer_size;

        Ok(())
    }

    fn deallocate_render_resources(&mut self) {
        self.plugin.deactivate();
        self.prepared = false;
    }

    fn is_prepared(&self) -> bool {
        self.prepared
    }

    fn sample_rate(&self) -> Option<f64> {
        if self.prepared {
            Some(self.sample_rate)
        } else {
            None
        }
    }

    fn max_frames(&self) -> Option<u32> {
        if self.prepared {
            Some(self.max_frames)
        } else {
            None
        }
    }

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn save_state(&self) -> Vec<u8> {
        let params = self
            .params_by_id
            .iter()
            .map(|(id, param, _)| {
                let value = unsafe {
                    match param {
                        ParamPtr::FloatParam(_) => ParamValue::F32(param.unmodulated_plain_value()),
                        ParamPtr::IntParam(_) => {
                            ParamValue::I32(param.unmodulated_plain_value() as i32)
                        }
                        ParamPtr::BoolParam(_) => {
                            ParamValue::Bool(param.unmodulated_normalized_value() >= 0.5)
                        }
                        ParamPtr::EnumParam(_) => {
                            ParamValue::I32(param.unmodulated_plain_value() as i32)
                        }
                    }
                };
                (id.clone(), value)
            })
            .collect();

        let state = PluginState {
            version: P::VERSION.to_string(),
            params,
            fields: self.params.serialize_fields(),
        };

        serde_json::to_vec(&state).unwrap_or_default()
    }

    fn load_state(&mut self, data: &[u8]) -> error::PluginResult<()> {
        let mut state = serde_json::from_slice::<PluginState>(data)
            .map_err(|e| error::PluginError::StateError(e.to_string()))?;

        P::filter_state(&mut state);
        for (param_id, value) in &state.params {
            let Some(hash) = self.param_id_to_hash.get(param_id) else {
                continue;
            };
            let Some(param) = self.params_by_hash.get(hash).copied() else {
                continue;
            };

            unsafe {
                match (param, value) {
                    (ParamPtr::FloatParam(p), ParamValue::F32(v)) => {
                        (*p)._internal_set_plain_value(*v);
                    }
                    (ParamPtr::IntParam(p), ParamValue::I32(v)) => {
                        (*p)._internal_set_plain_value(*v);
                    }
                    (ParamPtr::BoolParam(p), ParamValue::Bool(v)) => {
                        (*p)._internal_set_plain_value(*v);
                    }
                    (ParamPtr::EnumParam(p), ParamValue::I32(v)) => {
                        (*p)._internal_set_plain_value(*v);
                    }
                    (ParamPtr::EnumParam(p), ParamValue::String(id)) => {
                        (*p).set_from_id(id);
                    }
                    _ => {}
                }

                if let Some(config) = self.current_buffer_config {
                    param._internal_update_smoother(config.sample_rate, true);
                }
            }
        }

        self.params.deserialize_fields(&state.fields);
        self.queue_editor_values_changed();
        Ok(())
    }

    fn reset(&mut self) {
        self.plugin.reset();
    }

    fn tail_samples(&self) -> u32 {
        0
    }

    fn latency_samples(&self) -> u32 {
        self.latency_samples.load(Ordering::Relaxed)
    }

    fn process(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],
        num_samples: usize,
    ) -> error::PluginResult<()> {
        if inputs.len() > 32 || outputs.len() > 32 {
            return Err(error::PluginError::ProcessingError(
                "AU supports at most 32 channels per direction".to_string(),
            ));
        }
        let main_inputs = self
            .current_audio_io_layout
            .main_input_channels
            .map_or(0, |v| v.get() as usize);
        let main_outputs = self
            .current_audio_io_layout
            .main_output_channels
            .map_or(0, |v| v.get() as usize);
        let expected_inputs = main_inputs
            + self
                .current_audio_io_layout
                .aux_input_ports
                .iter()
                .map(|v| v.get() as usize)
                .sum::<usize>();
        let expected_outputs = main_outputs
            + self
                .current_audio_io_layout
                .aux_output_ports
                .iter()
                .map(|v| v.get() as usize)
                .sum::<usize>();
        if inputs.len() != expected_inputs || outputs.len() != expected_outputs {
            return Err(error::PluginError::ProcessingError(format!(
                "AU buffer layout mismatch: got {}/{} input/output channels, expected {expected_inputs}/{expected_outputs}",
                inputs.len(),
                outputs.len()
            )));
        }

        (0..main_outputs).for_each(|channel| {
            if let Some(input) = inputs.get(channel).filter(|_| channel < main_inputs) {
                outputs[channel][..num_samples].copy_from_slice(&input[..num_samples]);
            } else {
                outputs[channel][..num_samples].fill(0.0);
            }
        });
        for output in outputs.iter_mut().skip(main_outputs) {
            output[..num_samples].fill(0.0);
        }

        let mut output_ptrs = [std::ptr::null_mut(); 32];
        for (index, output) in outputs.iter_mut().enumerate() {
            output_ptrs[index] = output.as_mut_ptr();
        }

        unsafe {
            self.process_buffer.set_slices(num_samples, |slices| {
                slices.clear();
                for ptr in output_ptrs.iter().take(main_outputs) {
                    slices.push(std::slice::from_raw_parts_mut(*ptr, num_samples));
                }
            });
        }

        let mut input_channel = main_inputs;
        let mut storage_channel = 0;
        for (bus_index, channels) in self
            .current_audio_io_layout
            .aux_input_ports
            .iter()
            .enumerate()
        {
            let channel_count = channels.get() as usize;
            for channel in 0..channel_count {
                self.aux_input_storage[storage_channel + channel][..num_samples]
                    .copy_from_slice(&inputs[input_channel + channel][..num_samples]);
            }
            let mut storage_ptrs = [std::ptr::null_mut(); 32];
            for (index, channel) in self.aux_input_storage
                [storage_channel..storage_channel + channel_count]
                .iter_mut()
                .enumerate()
            {
                storage_ptrs[index] = channel.as_mut_ptr();
            }
            unsafe {
                self.aux_input_buffers[bus_index].set_slices(num_samples, |slices| {
                    slices.clear();
                    for ptr in storage_ptrs.iter().take(channel_count) {
                        slices.push(std::slice::from_raw_parts_mut(*ptr, num_samples));
                    }
                });
            }
            input_channel += channel_count;
            storage_channel += channel_count;
        }

        let mut output_channel = main_outputs;
        for (bus_index, channels) in self
            .current_audio_io_layout
            .aux_output_ports
            .iter()
            .enumerate()
        {
            let channel_count = channels.get() as usize;
            unsafe {
                self.aux_output_buffers[bus_index].set_slices(num_samples, |slices| {
                    slices.clear();
                    for ptr in output_ptrs.iter().skip(output_channel).take(channel_count) {
                        slices.push(std::slice::from_raw_parts_mut(*ptr, num_samples));
                    }
                });
            }
            output_channel += channel_count;
        }

        // The buffers only borrow host/scratch memory for this process call. `Buffer` is
        // invariant in its lifetime, so shorten the preallocated buffers' erased lifetime here.
        let aux_inputs = unsafe {
            std::mem::transmute::<&mut [Buffer<'static>], &mut [Buffer<'_>]>(
                self.aux_input_buffers.as_mut_slice(),
            )
        };
        let aux_outputs = unsafe {
            std::mem::transmute::<&mut [Buffer<'static>], &mut [Buffer<'_>]>(
                self.aux_output_buffers.as_mut_slice(),
            )
        };
        let mut aux = AuxiliaryBuffers {
            inputs: aux_inputs,
            outputs: aux_outputs,
        };
        let mut context = Au2ProcessContext::<P>::new(
            self.sample_rate as f32,
            self.latency_samples.clone(),
            &self.midi_events,
            self.process_offset,
            num_samples,
            self.transport_info,
            &mut self.midi_output,
        );

        match self
            .plugin
            .process(&mut self.process_buffer, &mut aux, &mut context)
        {
            ProcessStatus::Error(message) => {
                Err(error::PluginError::ProcessingError(message.to_string()))
            }
            _ => Ok(()),
        }
    }

    fn process_f64(
        &mut self,
        _inputs: &[&[f64]],
        _outputs: &mut [&mut [f64]],
        _num_samples: usize,
    ) -> error::PluginResult<()> {
        Err(error::PluginError::ProcessingError(
            "f64 processing not supported".to_string(),
        ))
    }

    fn process_scheduled(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],
        num_samples: usize,
        events: &[render::Au2ScheduledParameterEvent],
    ) -> error::PluginResult<()> {
        if inputs.len() > 32 || outputs.len() > 32 {
            return Err(error::PluginError::ProcessingError(
                "AU supports at most 32 channels".to_string(),
            ));
        }
        let mut input_ptr_storage = [std::ptr::null(); 32];
        let mut output_ptr_storage = [std::ptr::null_mut(); 32];
        for (index, slice) in inputs.iter().enumerate() {
            input_ptr_storage[index] = slice.as_ptr();
        }
        for (index, slice) in outputs.iter_mut().enumerate() {
            output_ptr_storage[index] = slice.as_mut_ptr();
        }
        let input_ptrs = &input_ptr_storage[..inputs.len()];
        let output_ptrs = &output_ptr_storage[..outputs.len()];
        let mut cursor = 0;

        for event in events {
            let offset = (event.sample_offset as usize).min(num_samples);
            if offset > cursor {
                self.process_range(input_ptrs, output_ptrs, cursor, offset - cursor)?;
                cursor = offset;
            }
            if event.duration_samples == 0 {
                self.set_parameter_value(event.parameter_address, event.end_value)?;
                continue;
            }

            let ramp_end = offset
                .saturating_add(event.duration_samples as usize)
                .min(num_samples);
            while cursor < ramp_end {
                let position = (cursor - offset) as f32 / event.duration_samples as f32;
                let value = event.start_value + (event.end_value - event.start_value) * position;
                self.set_parameter_value(event.parameter_address, value)?;
                self.process_range(input_ptrs, output_ptrs, cursor, 1)?;
                cursor += 1;
            }
            self.set_parameter_value(event.parameter_address, event.end_value)?;
        }

        self.process_range(input_ptrs, output_ptrs, cursor, num_samples - cursor)
    }

    fn process_scheduled_with_events(
        &mut self,
        inputs: &[&[f32]],
        outputs: &mut [&mut [f32]],
        num_samples: usize,
        events: &[crate::render::Au2ScheduledParameterEvent],
        context: crate::render::Au2ProcessEvents<'_>,
    ) -> error::PluginResult<()> {
        if context.midi_events.len() > self.midi_events.capacity() {
            return Err(error::PluginError::ProcessingError(
                "AU MIDI input capacity exceeded".into(),
            ));
        }
        self.midi_events.clear();
        self.midi_events.extend_from_slice(context.midi_events);
        self.transport_info = context.transport;
        self.midi_output.clear();
        let result = self.process_scheduled(inputs, outputs, num_samples, events);
        if context.midi_output.capacity() - context.midi_output.len() >= self.midi_output.len() {
            context.midi_output.extend_from_slice(&self.midi_output);
        }
        result
    }

    fn apply_parameter_events(
        &mut self,
        immediate: &[render::Au2ParameterEvent],
        _ramps: &[render::Au2ParameterRampEvent],
    ) -> error::PluginResult<()> {
        for event in immediate {
            if let Some(param) = self.params_by_hash.get(&event.parameter_address).copied() {
                unsafe {
                    param._internal_set_normalized_value(event.value);
                    if let Some(config) = self.current_buffer_config {
                        param._internal_update_smoother(config.sample_rate, false);
                    }
                }
                self.queue_editor_param_changed(event.parameter_address, event.value);
            }
        }
        Ok(())
    }

    fn input_bus_count(&self) -> usize {
        P::AUDIO_IO_LAYOUTS
            .first()
            .map(|l| usize::from(l.main_input_channels.is_some()) + l.aux_input_ports.len())
            .unwrap_or(0)
    }

    fn output_bus_count(&self) -> usize {
        P::AUDIO_IO_LAYOUTS
            .first()
            .map(|l| usize::from(l.main_output_channels.is_some()) + l.aux_output_ports.len())
            .unwrap_or(0)
    }

    fn input_bus_info(&self, index: usize) -> Option<audio_setup::BusInfo> {
        let layout = P::AUDIO_IO_LAYOUTS.first()?;

        if let Some(channels) = layout.main_input_channels {
            if index == 0 {
                return Some(audio_setup::BusInfo {
                    name: layout.main_input_name(),
                    bus_type: audio_setup::BusType::Main,
                    channel_count: channels.get() as usize,
                });
            }
        }

        let aux_start = usize::from(layout.main_input_channels.is_some());
        let aux_index = index.checked_sub(aux_start)?;
        let channels = layout.aux_input_ports.get(aux_index)?;
        Some(audio_setup::BusInfo {
            name: layout
                .aux_input_name(aux_index)
                .unwrap_or_else(|| format!("Sidechain Input {}", aux_index + 1)),
            bus_type: audio_setup::BusType::Aux,
            channel_count: channels.get() as usize,
        })
    }

    fn output_bus_info(&self, index: usize) -> Option<audio_setup::BusInfo> {
        let layout = P::AUDIO_IO_LAYOUTS.first()?;

        if let Some(channels) = layout.main_output_channels {
            if index == 0 {
                return Some(audio_setup::BusInfo {
                    name: layout.main_output_name(),
                    bus_type: audio_setup::BusType::Main,
                    channel_count: channels.get() as usize,
                });
            }
        }

        let aux_start = usize::from(layout.main_output_channels.is_some());
        let aux_index = index.checked_sub(aux_start)?;
        let channels = layout.aux_output_ports.get(aux_index)?;
        Some(audio_setup::BusInfo {
            name: layout
                .aux_output_name(aux_index)
                .unwrap_or_else(|| format!("Auxiliary Output {}", aux_index + 1)),
            bus_type: audio_setup::BusType::Aux,
            channel_count: channels.get() as usize,
        })
    }

    fn parameter_count(&self) -> usize {
        self.params_by_id.len()
    }

    fn parameter_info(&self, index: usize) -> Option<instance::ParameterInfo> {
        let (id, param, group) = self.params_by_id.get(index)?;
        let hash = *self.param_id_to_hash.get(id)?;

        Some(instance::ParameterInfo {
            id: hash,
            name: unsafe { param.name().to_string() },
            units: unsafe { param.unit().to_string() },
            min_value: 0.0,
            max_value: 1.0,
            default_value: unsafe { param.default_normalized_value() },
            current_value: unsafe { param.unmodulated_normalized_value() },
            step_count: unsafe { param.step_count().map(|v| v as i32).unwrap_or(0) },
            flags: unsafe { param.flags().bits() },
            group_id: if group.is_empty() {
                -1
            } else {
                hash_param_id(group) as i32
            },
        })
    }

    fn parameter_value(&self, param_id: u32) -> Option<f32> {
        self.params_by_hash
            .get(&param_id)
            .map(|param| unsafe { param.unmodulated_normalized_value() })
    }

    fn set_parameter_value(&mut self, param_id: u32, value: f32) -> error::PluginResult<()> {
        let Some(param) = self.params_by_hash.get(&param_id).copied() else {
            return Err(error::PluginError::InvalidParameter(param_id.to_string()));
        };

        unsafe {
            param._internal_set_normalized_value(value);
            if let Some(config) = self.current_buffer_config {
                param._internal_update_smoother(config.sample_rate, false);
            }
        }
        self.queue_editor_param_changed(param_id, value);

        Ok(())
    }

    fn frontend(&self) -> Arc<dyn instance::NicePluginFrontend> {
        self.frontend.clone()
    }
}

#[macro_export]
macro_rules! nice_export_au2 {
    ($plugin_ty:ty) => {
        #[cfg(target_os = "macos")]
        #[doc(hidden)]
        #[used]
        #[unsafe(link_section = "__DATA,__mod_init_func")]
        static NICE_PLUG_AU2_INIT: extern "C" fn() = {
            extern "C" fn init() {
                $crate::register_au2_plugin::<$plugin_ty>();
            }
            init
        };

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub extern "C" fn nice_plug_au2_register() {
            $crate::register_au2_plugin::<$plugin_ty>();
        }

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub extern "C" fn nice_au2_register_plugin_entry() {
            $crate::register_au2_plugin::<$plugin_ty>();
        }

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub extern "C" fn nice_au2_metadata() -> $crate::Au2ExportedMetadata {
            let name = <$plugin_ty as $crate::Au2Plugin>::AU2_NAME.as_bytes();
            $crate::Au2ExportedMetadata {
                name_ptr: name.as_ptr(),
                name_len: name.len(),
                component_type: <$plugin_ty as $crate::Au2Plugin>::AU2_CATEGORY.component_type(),
                sub_type: $crate::config::four_cc(<$plugin_ty as $crate::Au2Plugin>::AU2_SUBTYPE),
                manufacturer: $crate::config::four_cc(
                    <$plugin_ty as $crate::Au2Plugin>::AU2_MANUFACTURER,
                ),
            }
        }
    };
}

fn hash_param_id(id: &str) -> u32 {
    let mut hash: u32 = 0;
    for byte in id.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    hash & !(1 << 31)
}

struct Au2ProcessContext<'a, P: Plugin> {
    transport: Transport,
    latency_samples: Arc<AtomicU32>,
    events: &'a [crate::render::Au2MidiEvent],
    process_offset: usize,
    process_length: usize,
    event_index: usize,
    midi_output: *mut Vec<crate::render::Au2MidiEvent>,
    _marker: std::marker::PhantomData<P>,
}

impl<'a, P: Plugin> Au2ProcessContext<'a, P> {
    fn new(
        sample_rate: f32,
        latency_samples: Arc<AtomicU32>,
        events: &'a [crate::render::Au2MidiEvent],
        process_offset: usize,
        process_length: usize,
        info: crate::render::Au2TransportInfo,
        midi_output: &mut Vec<crate::render::Au2MidiEvent>,
    ) -> Self {
        let mut transport = Transport::new(sample_rate);
        transport.pos_samples = info.sample_position;
        transport.pos_seconds = info
            .sample_position
            .map(|value| value as f64 / sample_rate as f64);
        transport.playing = info.playing.unwrap_or(false);
        transport.recording = info.recording.unwrap_or(false);
        transport.tempo = info.tempo;
        transport.pos_beats = info.position_beats;
        if let Some((numerator, denominator)) = info.time_signature {
            transport.time_sig_numerator = Some(numerator);
            transport.time_sig_denominator = Some(denominator);
        }
        transport.loop_range_beats = info.cycle_beats;
        Self {
            transport,
            latency_samples,
            events,
            process_offset,
            process_length,
            event_index: 0,
            midi_output,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<P: Plugin> ProcessContext<P> for Au2ProcessContext<'_, P> {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }

    fn execute_background(&self, _task: P::BackgroundTask) {}

    fn execute_gui(&self, _task: P::BackgroundTask) {}

    fn transport(&self) -> &Transport {
        &self.transport
    }

    fn next_event(&mut self) -> Option<PluginNoteEvent<P>> {
        while let Some(event) = self.events.get(self.event_index) {
            self.event_index += 1;
            let at = event.sample_offset as usize;
            if (self.process_offset..self.process_offset + self.process_length).contains(&at) {
                if let Ok(note) = NoteEvent::from_midi(
                    (at - self.process_offset) as u32,
                    &[event.status, event.data1, event.data2],
                ) {
                    return Some(note);
                }
            }
        }
        None
    }

    fn try_send_event(
        &mut self,
        event: PluginNoteEvent<P>,
    ) -> Result<
        (),
        (
            PluginNoteEvent<P>,
            nice_plug_core::context::process::SendEventError,
        ),
    > {
        use nice_plug_core::context::process::SendEventError;
        let Some(nice_plug_core::midi::MidiResult::Basic(bytes)) = event.as_midi() else {
            return Err((
                event,
                SendEventError::InvalidEvent {
                    midi_output_config: P::MIDI_OUTPUT,
                },
            ));
        };
        let output = unsafe { &mut *self.midi_output };
        if output.len() == output.capacity() {
            return Err((event, SendEventError::HostBufferFull));
        }
        output.push(crate::render::Au2MidiEvent {
            status: bytes[0],
            data1: bytes[1],
            data2: bytes[2],
            sample_offset: event.timing(),
        });
        Ok(())
    }
    fn request_restart(&self) {}

    fn set_latency_samples(&self, samples: u32) {
        self.latency_samples.store(samples, Ordering::Relaxed);
    }

    fn set_current_voice_capacity(&self, _capacity: u32) {}
}
