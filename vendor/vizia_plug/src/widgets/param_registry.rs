//! Bridge between nice-plug's pull-based [`Param`] model and vizia's push-based
//! [`SyncSignal`] reactive graph.
//!
//! nice-plug exposes parameters through [`ParamPtr`] — stable opaque handles whose current
//! values are read on demand via unsafe accessors. vizia's new signal-based binding system
//! (vizia#619) requires observable values to be wrapped in [`SyncSignal`] so the reactive
//! graph can track dependencies and push updates to subscribers.
//!
//! [`ParamRegistry`] owns one [`SyncSignal<f32>`] per `(ParamPtr, axis)` pair (axes:
//! `Modulated`, `Unmodulated`). Widgets call
//! [`ParamRegistry::modulated`] / [`ParamRegistry::unmodulated`] on construction to obtain a
//! signal for the param value they care about; the registry lazily creates signals on first
//! access and reuses them on subsequent accesses.
//!
//! Parameter callbacks only set an atomic dirty flag. The editor's GUI idle callback
//! flushes current values into signals; no registry lock or reactive allocation is
//! performed on the audio thread.
//!
//! The type is cheaply `Clone` (it's an `Arc` internally), so the editor can keep its own
//! handle for flushing while also installing a clone as a vizia [`Model`] for widget lookup.

use std::collections::HashMap;
use std::sync::Arc;

use nice_plug_core::params::internals::ParamPtr;
use parking_lot::Mutex;
use vizia::prelude::*;

/// Which value of a parameter a signal tracks. nice-plug distinguishes between the raw
/// user/host-set value (*unmodulated*) and the value after any monophonic modulation has been
/// applied (*modulated*). Most widgets want modulated — it's what the user sees driving the
/// audio — but some (e.g. a slider that visualises both) want both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamAxis {
    /// `ParamPtr::modulated_normalized_value()`.
    Modulated,
    /// `ParamPtr::unmodulated_normalized_value()`.
    Unmodulated,
}

/// Shared, `Clone`-able handle to a set of param-tracking [`SyncSignal`]s. The same value
/// backs both the editor's flush path and the widget-facing lookup path — cloning a
/// `ParamRegistry` returns another handle to the same underlying signal map.
#[derive(Clone)]
pub struct ParamRegistry {
    inner: Arc<ParamRegistryInner>,
}

struct ParamRegistryInner {
    /// Lazily populated map of `(ParamPtr, axis)` → signal.
    ///
    /// Locked on the GUI thread during widget construction and idle flushing.
    /// Never acquire this lock from a host/audio parameter callback.
    signals: Mutex<HashMap<(ParamPtr, ParamAxis), SyncSignal<f32>>>,
}

impl ParamRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ParamRegistryInner {
                signals: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Drop all cached signals.
    ///
    /// Signals are tied to the editor's reactive runtime. When an editor instance closes, those
    /// runtime-owned signals become invalid and must not be reused after reopening.
    pub fn clear_signals(&self) {
        self.inner.signals.lock().clear();
    }

    /// Returns the signal tracking `param_ptr`'s value on the given `axis`, creating it
    /// (initialised from the current unsafe `ParamPtr` value) if it does not yet exist.
    pub fn signal(&self, param_ptr: ParamPtr, axis: ParamAxis) -> SyncSignal<f32> {
        let mut signals = self.inner.signals.lock();

        *signals.entry((param_ptr, axis)).or_insert_with(|| {
            // SAFETY: `param_ptr` was resolved from a valid `&impl Param` at widget
            // construction; it stays valid for the plugin's lifetime.
            let initial = unsafe {
                match axis {
                    ParamAxis::Modulated => param_ptr.modulated_normalized_value(),
                    ParamAxis::Unmodulated => param_ptr.unmodulated_normalized_value(),
                }
            };
            SyncSignal::new(initial)
        })
    }

    /// Shorthand for the common case: the modulated normalised value.
    pub fn modulated(&self, param_ptr: ParamPtr) -> SyncSignal<f32> {
        self.signal(param_ptr, ParamAxis::Modulated)
    }

    /// Shorthand for the unmodulated (user/host-set) value.
    pub fn unmodulated(&self, param_ptr: ParamPtr) -> SyncSignal<f32> {
        self.signal(param_ptr, ParamAxis::Unmodulated)
    }

    /// Re-read every registered parameter via unsafe `ParamPtr` and write the current value
    /// into its signal. Call only from the GUI thread after consuming the atomic
    /// parameter-change flag.
    pub fn flush_all(&self) {
        let signals = self.inner.signals.lock();

        for ((param_ptr, axis), signal) in signals.iter() {
            // SAFETY: see `signal()`.
            let current = unsafe {
                match axis {
                    ParamAxis::Modulated => param_ptr.modulated_normalized_value(),
                    ParamAxis::Unmodulated => param_ptr.unmodulated_normalized_value(),
                }
            };
            signal.set_if_changed(current);
        }
    }

    /// Re-read a single parameter via unsafe `ParamPtr` and write its current value into each
    /// of its registered axis signals (modulated + unmodulated), on the GUI thread.
    /// Axes with no registered signal are skipped.
    pub fn flush_one(&self, param_ptr: ParamPtr) {
        let signals = self.inner.signals.lock();

        for axis in [ParamAxis::Modulated, ParamAxis::Unmodulated] {
            if let Some(signal) = signals.get(&(param_ptr, axis)) {
                // SAFETY: see `signal()`.
                let current = unsafe {
                    match axis {
                        ParamAxis::Modulated => param_ptr.modulated_normalized_value(),
                        ParamAxis::Unmodulated => param_ptr.unmodulated_normalized_value(),
                    }
                };
                signal.set_if_changed(current);
            }
        }
    }
}

impl Default for ParamRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Model for ParamRegistry {}
