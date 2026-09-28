use thiserror::Error;

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Processing error: {0}")]
    ProcessingError(String),

    #[error("State error: {0}")]
    StateError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Format not supported: {0}")]
    FormatNotSupported(String),
}

pub type PluginResult<T> = Result<T, PluginError>;

pub mod os_status {
    pub const NO_ERR: i32 = 0;

    pub const K_AUDIO_UNIT_ERR_INVALID_PARAMETER: i32 = -50;
    pub const K_AUDIO_UNIT_ERR_INVALID_PROPERTY_VALUE: i32 = -51;
    pub const K_AUDIO_UNIT_ERR_PROPERTY_NOT_SUPPORTED: i32 = -52;
    pub const K_AUDIO_UNIT_ERR_PROPERTY_IN_VARIATION: i32 = -53;
    pub const K_AUDIO_UNIT_ERR_INITIALIZED: i32 = -54;
    pub const K_AUDIO_UNIT_ERR_NOT_INITIALIZED: i32 = -55;
    pub const K_AUDIO_UNIT_ERR_ERR_HAS_DEPENDENT_ITEMS: i32 = -56;
    pub const K_AUDIO_UNIT_ERR_INVALID_PROPERTY_CHANGE: i32 = -57;
    pub const K_AUDIO_UNIT_ERR_INVALID_UNIT: i32 = -58;
    pub const K_AUDIO_UNIT_ERR_UNINITIALIZED: i32 = -59;
    pub const K_AUDIO_UNIT_ERR_ICON_NOT_LOADED: i32 = -60;
    pub const K_AUDIO_UNIT_ERR_CANNOT_DO_IN_CURRENT_CONTEXT: i32 = -61;
    pub const K_AUDIO_UNIT_ERR_RENDER: i32 = -62;
    pub const K_AUDIO_UNIT_ERR_TOO_MANY_FRAMES_TO_PROCESS: i32 = -63;
    pub const K_AUDIO_UNIT_ERR_INVALID_FILE: i32 = -64;
    pub const K_AUDIO_UNIT_ERR_UNKNOWN_FORMAT: i32 = -65;
    pub const K_AUDIO_UNIT_ERR_MANAGER_NOT_REGISTERED: i32 = -70;
    pub const K_AUDIO_UNIT_ERR_NOT_FOUND: i32 = -71;
    pub const K_AUDIO_UNIT_ERR_FORMAT_NOT_SUPPORTED: i32 = -66;
}
