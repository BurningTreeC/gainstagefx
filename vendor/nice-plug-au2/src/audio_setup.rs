pub use nice_plug_core::audio_setup::{AudioIOLayout, BufferConfig, PortNames, ProcessMode};

#[derive(Debug, Clone, PartialEq)]
pub enum BusType {
    Main,
    Aux,
}

#[derive(Debug, Clone)]
pub struct BusInfo {
    pub name: String,
    pub bus_type: BusType,
    pub channel_count: usize,
}

impl BusInfo {
    pub fn stereo(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bus_type: BusType::Main,
            channel_count: 2,
        }
    }

    pub fn mono(name: &str) -> Self {
        Self {
            name: name.to_string(),
            bus_type: BusType::Main,
            channel_count: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedBusInfo {
    pub channel_count: usize,
    pub bus_type: BusType,
}

impl CachedBusInfo {
    pub fn new(channel_count: usize, bus_type: BusType) -> Self {
        Self {
            channel_count,
            bus_type,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedBusConfig {
    pub input_buses: Vec<CachedBusInfo>,
    pub output_buses: Vec<CachedBusInfo>,
}

impl CachedBusConfig {
    pub fn new(input_buses: Vec<CachedBusInfo>, output_buses: Vec<CachedBusInfo>) -> Self {
        Self {
            input_buses,
            output_buses,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        fn validate_side(name: &str, buses: &[CachedBusInfo]) -> Result<(), String> {
            if buses.len() > 16 {
                return Err(format!("too many {name} buses"));
            }
            if buses
                .iter()
                .any(|bus| bus.channel_count == 0 || bus.channel_count > 32)
            {
                return Err(format!("invalid {name} bus channel count"));
            }
            if buses
                .iter()
                .filter(|bus| bus.bus_type == BusType::Main)
                .count()
                > 1
            {
                return Err(format!("more than one main {name} bus"));
            }
            if buses
                .first()
                .is_some_and(|bus| bus.bus_type != BusType::Main)
            {
                return Err(format!("the main {name} bus must be first"));
            }
            if buses.iter().skip(1).any(|bus| bus.bus_type != BusType::Aux) {
                return Err(format!("only the first {name} bus may be main"));
            }
            Ok(())
        }

        validate_side("input", &self.input_buses)?;
        validate_side("output", &self.output_buses)?;
        Ok(())
    }

    pub fn input_channel_count(&self) -> usize {
        self.input_buses.iter().map(|b| b.channel_count).sum()
    }

    pub fn output_channel_count(&self) -> usize {
        self.output_buses.iter().map(|b| b.channel_count).sum()
    }
}

pub fn from_nice_audio_io_layout(layout: &AudioIOLayout) -> CachedBusConfig {
    let mut input_buses = Vec::new();
    let mut output_buses = Vec::new();

    if let Some(channels) = layout.main_input_channels {
        input_buses.push(CachedBusInfo::new(channels.get() as usize, BusType::Main));
    }

    for ports in layout.aux_input_ports {
        input_buses.push(CachedBusInfo::new(ports.get() as usize, BusType::Aux));
    }

    if let Some(channels) = layout.main_output_channels {
        output_buses.push(CachedBusInfo::new(channels.get() as usize, BusType::Main));
    }

    for ports in layout.aux_output_ports {
        output_buses.push(CachedBusInfo::new(ports.get() as usize, BusType::Aux));
    }

    CachedBusConfig::new(input_buses, output_buses)
}
