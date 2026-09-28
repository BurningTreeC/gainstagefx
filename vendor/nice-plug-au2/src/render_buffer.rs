use super::{AudioBuffer, AudioBufferList};

impl AudioBuffer {
    /// Reads one buffer entry from an audio buffer list.
    ///
    /// # Safety
    /// `list` must be non-null and contain an entry at `index`.
    pub unsafe fn from_list(list: *const AudioBufferList, index: usize) -> Self {
        unsafe { *(&(*list).mBuffers as *const AudioBuffer).add(index) }
    }

    /// Reads one mutable buffer entry from an audio buffer list.
    ///
    /// # Safety
    /// `list` must be non-null, writable, and contain an entry at `index`.
    pub unsafe fn from_list_mut(list: *mut AudioBufferList, index: usize) -> Self {
        unsafe { *(&(*list).mBuffers as *const AudioBuffer).add(index) }
    }
}
