//! Panel colours and geometry.
//!
//! The whole layout is here as numbers rather than spread through the panel
//! code, because the one thing this panel has to get right is that it reads in
//! the order the signal travels, and that is a property of where things are.

use super::paint as vg;

/// Wide enough that the longest model names (Cali Oversized 4x12, Tube
/// Condenser 67) and the placement knobs sit without crowding.
pub const PANEL_W: f32 = 780.0;
pub const HEADER_H: f32 = 32.0;

/// The six sections, in signal order, with the height each needs.
///
/// Every one is full width and they stack downwards, so the panel is read the
/// way the signal goes through it and there is no second column to wonder
/// about. The number and name sit in a gutter down the left, which is what
/// makes the order legible at a glance rather than only after reading the
/// labels.
/// Sized to the controls in them and nothing else. An earlier set of these
/// was laid out around three and four line explanations sitting beside every
/// section, which made the window 720 by 734 -- a lot of screen for a plugin
/// with eleven controls on it, and permanently so, since panel text cannot be
/// dismissed once it has been read. The explanations are in the README, where
/// they can be read once.
pub const SECTIONS: [(&str, &str, f32); 6] = [
    // Trim and meter, then the pedal in front of the circuit.
    ("1", "INPUT", 124.0),
    // Wiring selections in two columns, plus the Twin input/bright switches
    // and the line describing the circuit.
    ("2", "CIRCUIT", 184.0),
    ("3", "DRIVE", 160.0),
    // Two rows: the stack's Bass/Middle/Treble, and beneath them the three
    // the Twin Reverb adds. Below rather than beside, because beside put
    // Speed and Intensity underneath the paragraph explaining the stack --
    // two things in one place, which is the layout fault that is hardest to
    // see in code and most obvious on screen.
    ("4", "TONE", 168.0),
    // Cabinet, speaker and microphones: dropdowns, placement knobs, dual-mic toggles.
    ("5", "CABINET", 204.0),
    ("6", "OUTPUT", 96.0),
];

/// Width of the numbered gutter down the left.
pub const GUTTER_W: f32 = 78.0;

/// A closed section is a strip this tall: its number, its name, and the
/// chevron that opens it. An open one keeps the same row at its top, so the
/// place to click to close a section is where it was clicked to open.
pub const CLOSED_H: f32 = 30.0;

/// Which sections are open, one bit each in signal order. The first one open
/// and the rest closed until the player says otherwise: the input is where a
/// session starts, and the rest can be opened as they are needed.
pub const FIRST_OPEN: u8 = 0b000001;

pub fn is_open(open: u8, index: usize) -> bool {
    open & (1 << index) != 0
}

/// How tall a section is drawn, open or closed.
pub fn section_height(open: u8, index: usize) -> f32 {
    if is_open(open, index) {
        SECTIONS[index].2
    } else {
        CLOSED_H
    }
}

/// Where a section starts, with the ones above it open or closed.
pub fn section_top(open: u8, index: usize) -> f32 {
    HEADER_H + (0..index).map(|i| section_height(open, i)).sum::<f32>()
}

/// The window's height for a set of open sections: it grows and shrinks to
/// fit them rather than leaving closed ones as empty panel.
pub fn window_height(open: u8) -> f32 {
    section_top(open, SECTIONS.len())
}

/// Every section open: the tallest the window gets.
pub const PANEL_H: f32 = {
    let mut total = 0.0;
    let mut i = 0;
    while i < SECTIONS.len() {
        total += SECTIONS[i].2;
        i += 1;
    }
    total
};

/// A knob sweeps this many degrees, zero at the lower left.
pub const SWEEP: f32 = 280.0;

pub const PANEL_TOP: u32 = 0x2f3336;
pub const PANEL_BOTTOM: u32 = 0x16191b;
/// The warm light a working control is lit by.
pub const GLOW: u32 = 0xff8a3c;

pub fn rgb(hex: u32) -> vg::Color {
    vg::Color::new(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
        1.0,
    )
}

pub fn rgba(hex: u32, alpha: f32) -> vg::Color {
    let mut c = rgb(hex);
    c.a = alpha;
    c
}

pub fn knob_angle(normalized: f32) -> f32 {
    (normalized - 0.5) * SWEEP
}

pub fn polar(cx: f32, cy: f32, radius: f32, degrees: f32) -> (f32, f32) {
    let a = degrees.to_radians();
    (cx + radius * a.sin(), cy - radius * a.cos())
}
