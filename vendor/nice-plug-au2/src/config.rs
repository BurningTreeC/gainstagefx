#[derive(Debug, Clone, Copy)]
pub struct Au2Config {
    pub category: Au2Category,
    pub manufacturer: [u8; 4],
    pub sub_type: [u8; 4],
    pub name: &'static str,
}

impl Au2Config {
    pub const fn new(
        category: Au2Category,
        manufacturer: [u8; 4],
        sub_type: [u8; 4],
        name: &'static str,
    ) -> Self {
        Self {
            category,
            manufacturer,
            sub_type,
            name,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Au2Category {
    Effect,
    MusicEffect,
    Generator,
    MidiEffect,
}

impl Au2Category {
    pub fn component_type(&self) -> u32 {
        match self {
            Au2Category::Effect => four_cc(*b"aufx"),
            Au2Category::MusicEffect => four_cc(*b"aumf"),
            Au2Category::Generator => four_cc(*b"aumu"),
            Au2Category::MidiEffect => four_cc(*b"aumi"),
        }
    }

    pub fn component_type_string(&self) -> &'static str {
        match self {
            Au2Category::Effect => "aufx",
            Au2Category::MusicEffect => "aumf",
            Au2Category::Generator => "aumu",
            Au2Category::MidiEffect => "aumi",
        }
    }
}

pub const fn four_cc(code: [u8; 4]) -> u32 {
    u32::from_be_bytes(code)
}

pub fn four_cc_string(code: [u8; 4]) -> String {
    String::from_utf8_lossy(&code).into_owned()
}
