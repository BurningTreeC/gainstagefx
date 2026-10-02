//! The GainStageFx front panel.
//!
//! The panel is the signal path, drawn top to bottom, and that is the only
//! organising idea in it. Six numbered bands: what arrives, what does the
//! work, how hard it is worked, what is taken out afterwards, what it comes
//! out of, what leaves. An arrow at the foot of each band points into the
//! next.
//!
//! Each band opens and closes from its header row, and the window grows and
//! shrinks to fit what is open, so a player keeps in view the sections they
//! are working with and the order of the rest stays readable as six rows. A
//! fresh panel opens with the input alone; which are open is kept with the
//! session (`GainStageParams::open_sections`).
//!
//! This is deliberate and it is the one thing the previous version could not
//! be given afterwards. Its controls sat where they had been added, so the
//! panel recorded the order the plugin was built in rather than the order the
//! signal travels, and nothing on it said what happened before what. A layout
//! is a claim about how a thing works; that one made no claim at all.

mod dropdown;
mod fonts;
mod paint;
mod panel;
pub mod session;
mod sprites;
mod style;
mod widgets;

use fonts as vizia_assets;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use vizia_plug::vizia::prelude::*;
use vizia_plug::{create_vizia_editor, ViziaState, ViziaTheming};

use crate::params::{
    CabModel, Cabinet, Circuit, GainStageParams, MicModel, Oversampling, PedalModel, SpeakerModel,
    Switch, ToneStack,
};
use dropdown::{Choice, DropButton, Dropdowns};
use paint::PanelCanvas;
use panel::Faceplate;
use style::*;
use widgets::{Knob, Meter, Selector};

pub struct Panel {
    pub params: Arc<GainStageParams>,
    pub meters: Arc<crate::meters::Meters>,
    parameter_signal: Signal<Arc<GainStageParams>>,
    /// Which sections are open; see `style::is_open`.
    open: Signal<u8>,
}

pub enum PanelEvent {
    /// Open a closed section or close an open one.
    ToggleSection(usize),
}

impl Model for Panel {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|_: &vizia_plug::widgets::RawParamEvent, _| {
            self.parameter_signal.set(self.params.clone())
        });
        event.map(|panel: &PanelEvent, meta| match panel {
            PanelEvent::ToggleSection(index) => {
                let open = self.open.get_untracked() ^ (1 << index);
                // The stored set first: the host asks the editor its size in
                // answer to the resize, and `default_state` reads it there.
                self.params.open_sections.store(open, Ordering::Relaxed);
                self.open.set(open);
                cx.emit(WindowEvent::SetSize(WindowSize {
                    width: PANEL_W as u32,
                    height: window_height(open) as u32,
                }));
                meta.consume();
            }
        });
    }
}

/// The window's height as the panel stands now.
fn current_height(cx: &Context) -> f32 {
    window_height(cx.data::<Panel>().open.get_untracked())
}

pub const BASE_DPI: f64 = 1.5;

pub use style::{window_height, FIRST_OPEN};

/// The editor's state, sized to whichever sections `open` says are open.
pub fn default_state(open: Arc<AtomicU8>) -> Arc<ViziaState> {
    // The menu starts at 100%, which renders at the requested base DPI of 1.5.
    ViziaState::new_with_base_scale_factor(
        move || {
            (
                PANEL_W as u32,
                window_height(open.load(Ordering::Relaxed)) as u32,
            )
        },
        BASE_DPI,
    )
}

/// Updates the scale used by `Editor::size()` and saved in the host session.
/// Vizia's drawing scale is separate and must only change after the host has
/// accepted the resize. `PersistentField::set` copies the carrier's scale;
/// the original state's size function and open status stay intact.
pub fn remember_scale(state: &Arc<ViziaState>, scale: f64) {
    use nice_plug::params::persist::PersistentField;
    let carrier = ViziaState::new_with_default_scale_factor(|| (0, 0), scale);
    if let Ok(carrier) = Arc::try_unwrap(carrier) {
        PersistentField::set(state, carrier);
    }
}

/// Height of a label box, which is centred on its anchor point.
const LABEL_H: f32 = 16.0;

/// Picking one knob's parameter out of the set.
type ToKnob = fn(&Arc<GainStageParams>) -> &nice_plug::prelude::FloatParam;

pub fn create(
    params: Arc<GainStageParams>,
    meters: Arc<crate::meters::Meters>,
    editor_state: Arc<ViziaState>,
) -> Option<vizia_plug::ViziaEditor> {
    let state = editor_state.clone();
    create_vizia_editor(editor_state, ViziaTheming::None, move |cx, _gui| {
        build_panel(
            cx,
            params.clone(),
            meters.clone(),
            state.user_scale_factor(),
        );
    })
}

fn build_panel(
    cx: &mut Context,
    params: Arc<GainStageParams>,
    meters: Arc<crate::meters::Meters>,
    scale: f64,
) {
    // Vizia ships Roboto as TTF byte slices. Registering the faces from
    // memory makes them part of the plugin binary, so the GUI never
    // depends on a system-wide ttf-roboto installation.
    vizia_assets::register_roboto(cx);
    vizia_assets::register_roboto_bold(cx);
    // The only styling the panel takes from a sheet rather than from its
    // own drawing: the scroll bar, which vizia builds but cannot size or
    // colour without a theme.
    let _ = cx.add_stylesheet(session::SCROLLBAR);

    Panel {
        params: params.clone(),
        parameter_signal: Signal::new(params.clone()),
        meters: meters.clone(),
        open: Signal::new(params.open_sections.load(Ordering::Relaxed)),
    }
    .build(cx);

    session::Session::build_into(cx, params.clone(), scale);
    Dropdowns::build_into(cx, params.clone());

    {
        let open = cx.data::<Panel>().open;
        Binding::new(cx, open, move |cx| {
            Faceplate::new(cx, open.get());
        });
    }
    strip(cx);
    section(cx, 0, input);
    section(cx, 1, circuit);
    section(cx, 2, drive);
    section(cx, 3, tone);
    section(cx, 4, cabinet);
    section(cx, 5, output);
    // After the sections, so a header takes the click over the band it heads.
    gutter(cx);

    // Last, so they draw over the panel and take the clicks first. The
    // dialogs come after the menu: a question has to sit on top of
    // whatever asked it.
    dropdown::menu(cx);
    session::menu(cx);
    session::sizes(cx);
    session::dialogs(cx);
}

#[cfg(test)]
mod render_tests;

// ---------------------------------------------------------------------------
// Pieces every section is built from
// ---------------------------------------------------------------------------

/// Small print, centred on a point.
fn label(cx: &mut Context, text: &str, x: f32, y: f32, size: f32, width: f32, colour: u32) {
    Label::new(cx, text.to_owned())
        .position_type(PositionType::Absolute)
        .left(Pixels(x - width / 2.0))
        .top(Pixels(y - LABEL_H / 2.0))
        .width(Pixels(width))
        .height(Pixels(LABEL_H))
        .alignment(Alignment::Center)
        .text_align(TextAlign::Center)
        .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
        .font_size(size)
        .color(Color::rgb(
            ((colour >> 16) & 0xff) as u8,
            ((colour >> 8) & 0xff) as u8,
            (colour & 0xff) as u8,
        ))
        .hoverable(false);
}

/// A knob with its name under it and its value under that.
///
/// Both lines, always. A knob whose value can only be discovered by dragging
/// it is a knob you cannot set deliberately, and every one of these has a
/// number worth knowing.
fn knob<P, F>(cx: &mut Context, x: f32, y: f32, radius: f32, name: &str, to_param: F, read: P)
where
    F: Fn(&Arc<GainStageParams>) -> &nice_plug::prelude::FloatParam + Copy + 'static,
    P: Fn(&Arc<GainStageParams>) -> String + Clone + 'static,
{
    let parameter_signal = cx.data::<Panel>().parameter_signal;
    Knob::new(cx, parameter_signal, to_param, radius, true)
        .position_type(PositionType::Absolute)
        .left(Pixels(x - radius))
        .top(Pixels(y - radius));

    label(cx, name, x, y + radius + 10.0, 9.5, 100.0, 0x9aa6b0);

    Label::new(cx, parameter_signal.map(move |p| read(p)))
        .position_type(PositionType::Absolute)
        .left(Pixels(x - 50.0))
        .top(Pixels(y + radius + 21.0 - LABEL_H / 2.0))
        .width(Pixels(100.0))
        .height(Pixels(LABEL_H))
        .alignment(Alignment::Center)
        .text_align(TextAlign::Center)
        .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
        .font_size(9.5)
        .color(Color::rgb(0xff, 0xb2, 0x6a))
        .hoverable(false);
}

/// A row of choices, sized to the space it is given.
fn selector<P, F>(
    cx: &mut Context,
    x: f32,
    y: f32,
    width: f32,
    to_param: F,
    labels: Vec<&'static str>,
    enabled: bool,
) where
    F: Fn(&Arc<GainStageParams>) -> &P + Copy + 'static,
    P: nice_plug::prelude::Param + 'static,
{
    let parameter_signal = cx.data::<Panel>().parameter_signal;
    Selector::new(cx, parameter_signal, to_param, labels, enabled)
        .position_type(PositionType::Absolute)
        .left(Pixels(x))
        .top(Pixels(y))
        .width(Pixels(width))
        .height(Pixels(20.0));
}

/// Where a section's controls start, clear of the numbered gutter.
fn body_x() -> f32 {
    GUTTER_W + 14.0
}

fn body_w() -> f32 {
    PANEL_W - body_x() - 18.0
}

/// A section's controls, in a box that sits wherever the sections above it
/// leave it and is not there at all while the section is closed. Everything
/// inside is placed from the section's own top.
fn section(cx: &mut Context, index: usize, content: fn(&mut Context)) {
    let open = cx.data::<Panel>().open;
    VStack::new(cx, content)
        .position_type(PositionType::Absolute)
        .left(Pixels(0.0))
        .top(open.map(move |open| Pixels(section_top(*open, index))))
        .width(Pixels(PANEL_W))
        .height(Pixels(SECTIONS[index].2))
        .display(open.map(move |open| Display::from(is_open(*open, index))));
}

/// The numbering down the left, which is what makes the order legible before
/// anything else on the panel is read -- and the handle each section opens
/// and closes by.
///
/// Every section's number and name sit on one row at its top, the same row
/// open or closed, with a chevron saying which: so the place to click to
/// close a section is where it was clicked to open. A closed section is that
/// row and nothing else, and all of it opens it.
fn gutter(cx: &mut Context) {
    let open = cx.data::<Panel>().open;
    Binding::new(cx, open, move |cx| {
        let open = open.get();
        for (index, (number, name, _)) in SECTIONS.iter().enumerate() {
            let top = section_top(open, index);
            let mid = top + CLOSED_H / 2.0;
            let is_open = is_open(open, index);
            Fold::new(cx, index, is_open)
                .position_type(PositionType::Absolute)
                .left(Pixels(0.0))
                .top(Pixels(top))
                // The header row: the gutter's width while the controls are
                // beside it, the whole band while it is all there is.
                .width(Pixels(if is_open { GUTTER_W } else { PANEL_W }))
                .height(Pixels(CLOSED_H));
            Label::new(cx, *number)
                .position_type(PositionType::Absolute)
                .left(Pixels(19.0))
                .top(Pixels(mid - 14.0))
                .width(Pixels(16.0))
                .height(Pixels(28.0))
                .alignment(Alignment::Left)
                .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
                .font_size(20.0)
                .color(Color::rgba(
                    0xff,
                    0xff,
                    0xff,
                    if is_open { 0x30 } else { 0x24 },
                ))
                .hoverable(false);
            Label::new(cx, *name)
                .position_type(PositionType::Absolute)
                .left(Pixels(35.0))
                .top(Pixels(mid - LABEL_H / 2.0))
                .width(Pixels(GUTTER_W - 35.0))
                .height(Pixels(LABEL_H))
                .alignment(Alignment::Left)
                .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
                .font_size(9.5)
                .color(if is_open {
                    Color::rgb(0xb4, 0xbe, 0xc6)
                } else {
                    Color::rgb(0x8b, 0x95, 0x9d)
                })
                .hoverable(false);
        }
    });
}

/// The click target that opens or closes a section, and the chevron that
/// says which it will do.
struct Fold {
    index: usize,
    open: bool,
    hovered: bool,
}

impl Fold {
    fn new(cx: &mut Context, index: usize, open: bool) -> Handle<'_, Self> {
        Self {
            index,
            open,
            hovered: false,
        }
        .build(cx, |_| {})
        .cursor(CursorIcon::Hand)
    }
}

impl View for Fold {
    fn element(&self) -> Option<&'static str> {
        Some("section-fold")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window: &WindowEvent, meta| match window {
            WindowEvent::MouseDown(MouseButton::Left) => {
                cx.emit(PanelEvent::ToggleSection(self.index));
                meta.consume();
            }
            WindowEvent::MouseEnter => {
                self.hovered = true;
                cx.needs_redraw();
            }
            WindowEvent::MouseLeave => {
                self.hovered = false;
                cx.needs_redraw();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        let b = cx.bounds();
        let s = cx.scale_factor();
        if self.hovered {
            // The row it acts on, lit.
            let mut row = paint::Path::new();
            row.rect(b.x, b.y, b.width(), b.height());
            canvas.fill_path(&row, &paint::Paint::color(rgba(GLOW, 0.06)));
        }
        let (x, y) = (b.x + 10.0 * s, b.y + CLOSED_H / 2.0 * s);
        let mut chevron = paint::Path::new();
        if self.open {
            chevron.move_to(x - 3.5 * s, y - 1.5 * s);
            chevron.line_to(x, y + 2.0 * s);
            chevron.line_to(x + 3.5 * s, y - 1.5 * s);
        } else {
            chevron.move_to(x - 1.5 * s, y - 3.5 * s);
            chevron.line_to(x + 2.0 * s, y);
            chevron.line_to(x - 1.5 * s, y + 3.5 * s);
        }
        let alpha = if self.hovered { 0.55 } else { 0.30 };
        canvas.stroke_path(
            &chevron,
            &paint::Paint::color(rgba(0xffffff, alpha)).with_line_width(1.4 * s),
        );
    }
}

// ---------------------------------------------------------------------------
// The strip above the panel
// ---------------------------------------------------------------------------

fn strip(cx: &mut Context) {
    let parameter_signal = cx.data::<Panel>().parameter_signal;
    // The name, and the version under it. Stacked inside the same 32 pixel
    // strip rather than given room of their own: a version is something you
    // go and look for when reporting a fault, not something to read every
    // session, so it gets the smaller half of a header that already exists.
    label(cx, "GAINSTAGEFX", 62.0, 10.0, 10.5, 116.0, 0xe8eef4);
    label(
        cx,
        concat!("v", env!("CARGO_PKG_VERSION")),
        62.0,
        22.0,
        8.0,
        116.0,
        0x7d878f,
    );

    let row = HEADER_H / 2.0 - 10.0;
    session::PresetButton::build_into(cx)
        .position_type(PositionType::Absolute)
        .left(Pixels(session::BUTTON_X))
        .top(Pixels(row))
        .width(Pixels(session::BUTTON_W))
        .height(Pixels(20.0));

    session::Press::build_into(cx, "Save", true, false, || session::SessionEvent::OpenSave)
        .position_type(PositionType::Absolute)
        .left(Pixels(276.0))
        .top(Pixels(row))
        .width(Pixels(46.0))
        .height(Pixels(20.0));

    // Only your own presets can be deleted, and the button says so by going
    // dim rather than by disappearing -- a strip that changes shape as the
    // selection moves is harder to aim at.
    {
        let deletable = cx.data::<session::Session>().view.map(|s| s.deletable);
        Binding::new(cx, deletable, move |cx| {
            session::Press::build_into(cx, "Delete", deletable.get(), false, || {
                session::SessionEvent::OpenDelete
            })
            .position_type(PositionType::Absolute)
            .left(Pixels(326.0))
            .top(Pixels(HEADER_H / 2.0 - 10.0))
            .width(Pixels(52.0))
            .height(Pixels(20.0));
        });
    };

    // How big the panel is drawn. Not part of the signal path, so it lives up
    // here with the rest of what is about the plugin rather than the sound.
    session::SizeButton::build_into(cx)
        .position_type(PositionType::Absolute)
        .left(Pixels(session::SIZE_X))
        .top(Pixels(row))
        .width(Pixels(session::SIZE_W))
        .height(Pixels(20.0));

    let width = 112.0;
    let left = PANEL_W - 14.0 - width;
    // A circuit-modelled voice follows this control only as far as
    // `voice::MODELLED_MAX_OVERSAMPLING`: past that those circuits cost more
    // than the time there is. So the row offers what it can deliver -- the
    // factors up to the cap, live, because they are what takes the fold-back
    // out of a high-gain amplifier -- and lights the one in use when a stored
    // setting is higher than the cap.
    //
    // The parameter itself is left alone rather than written down to the cap,
    // so choosing a pedal again brings the setting back instead of silently
    // discarding it, and nothing here fights the host's automation.
    {
        let modelled = parameter_signal.map(|p| p.circuit.value().voice().is_modelled());
        Binding::new(cx, modelled, move |cx| {
            let labels: Vec<&'static str> = Oversampling::ALL.iter().map(|o| o.name()).collect();
            let handle = if modelled.get() {
                let total = labels.len();
                let cap = Oversampling::ALL
                    .iter()
                    .position(|o| o.factor() >= crate::voice::MODELLED_MAX_OVERSAMPLING)
                    .unwrap_or(0);
                let offered = labels[..=cap].to_vec();
                Selector::capped(
                    cx,
                    parameter_signal,
                    |p| &p.oversampling,
                    offered,
                    total,
                    cap,
                )
            } else {
                Selector::new(cx, parameter_signal, |p| &p.oversampling, labels, true)
            };
            handle
                .position_type(PositionType::Absolute)
                .left(Pixels(left))
                .top(Pixels(row))
                .width(Pixels(width))
                .height(Pixels(20.0));
        });
    };
    label(
        cx,
        "quality",
        left - 26.0,
        HEADER_H / 2.0,
        9.5,
        44.0,
        0x7e8a96,
    );
}

// ---------------------------------------------------------------------------
// 1 Input
// ---------------------------------------------------------------------------

fn input(cx: &mut Context) {
    let parameter_signal = cx.data::<Panel>().parameter_signal;
    let top = 0.0;

    knob(
        cx,
        body_x() + 30.0,
        top + 24.0,
        19.0,
        "TRIM",
        |p| &p.input_trim,
        |p| format!("{:+.1} dB", p.input_trim.value()),
    );

    // The meter reads against the level the circuits were voiced at, so its
    // zero is the only place on the panel where every other control means what
    // its label says.
    let meter_x = body_x() + 88.0;
    let meter_w = body_w() - 300.0;
    let meters = cx.data::<Panel>().meters.clone();
    Meter::new(cx, meters)
        .position_type(PositionType::Absolute)
        .left(Pixels(meter_x))
        .top(Pixels(top + 18.0))
        .width(Pixels(meter_w))
        .height(Pixels(14.0));

    label(
        cx,
        "input level, before noise reduction",
        meter_x + meter_w / 2.0,
        top + 46.0,
        9.5,
        meter_w,
        0x7e8a96,
    );

    let noise_x = body_x() + body_w() - 195.0;
    selector(
        cx,
        noise_x,
        top + 14.0,
        110.0,
        |p| &p.noise_reduction,
        vec!["Off", "On"],
        true,
    );
    label(
        cx,
        "NOISE REDUCTION",
        noise_x + 55.0,
        top + 44.0,
        9.0,
        126.0,
        0x7e8a96,
    );
    knob(
        cx,
        body_x() + body_w() - 35.0,
        top + 24.0,
        16.0,
        "THRESHOLD",
        |p| &p.noise_threshold,
        |p| format!("{:.0} dBFS", p.noise_threshold.value()),
    );

    // The pedal, between the guitar and the circuit. Its knobs are its own, so
    // a pedal in front of an amplifier keeps both sets of controls.
    label(
        cx,
        "pedal",
        body_x() + 30.0,
        top + 84.0,
        9.5,
        76.0,
        0x7e8a96,
    );
    DropButton::new(cx, parameter_signal, |p| &p.pedal, Choice::Pedal, true)
        .position_type(PositionType::Absolute)
        .left(Pixels(body_x() + 76.0))
        .top(Pixels(top + 74.0))
        .width(Pixels(200.0))
        .height(Pixels(20.0));
    // A pedal's knobs are the ones that pedal has. Most have three; the Heavy
    // Metal has four and the Metal Zone six, and a Metal Zone with three of its
    // controls missing is not that pedal. Each tone knob is labelled with what
    // the box calls it -- tone, filter, colour, middle -- rather than all of
    // them being flattened to "tone". See `voice::PEDAL_TONES`.
    {
        let pedal = parameter_signal.map(|p| p.pedal.value());
        Binding::new(cx, pedal, move |cx| {
            let pedal = pedal.get();
            let live = pedal != PedalModel::None;
            let labels = pedal.voice().tone_labels();
            // Drive and level named as the box names them, as the tone knobs
            // are. See `Pedal::drive_and_level_labels`.
            let (drive_label, level_label) = pedal.voice().drive_and_level_labels();
            let shown: Vec<(usize, &'static str)> = labels
                .iter()
                .enumerate()
                .filter_map(|(index, label)| label.map(|label| (index, label)))
                .collect();
            let x0 = body_x() + 330.0;
            let y = top + 82.0;
            // Three knobs keep the spacing they have always had; a wider row
            // closes up to stay inside the panel.
            let count = shown.len().max(1) + 2;
            let step = if count <= 3 {
                86.0
            } else {
                370.0 / (count - 1) as f32
            };
            // A pedal with no drive control (the Treble Boost, whose one pot
            // is its level) greys the knob rather than leaving one that turns
            // nothing.
            placement_knob(
                cx,
                x0,
                y,
                11.0,
                drive_label,
                |p| &p.pedal_drive,
                live && pedal.voice().has_drive(),
                percent,
            );
            if shown.is_empty() {
                // A fuzz with no tone control greys its Tone knob rather than
                // leaving one that turns nothing.
                placement_knob(
                    cx,
                    x0 + step,
                    y,
                    11.0,
                    "tone",
                    |p| &p.pedal_tone,
                    false,
                    percent,
                );
            }
            for (slot, &(index, label)) in shown.iter().enumerate() {
                let x = x0 + step * (slot + 1) as f32;
                match index {
                    0 => placement_knob(cx, x, y, 11.0, label, |p| &p.pedal_tone, live, percent),
                    1 => placement_knob(cx, x, y, 11.0, label, |p| &p.pedal_tone_b, live, percent),
                    2 => placement_knob(cx, x, y, 11.0, label, |p| &p.pedal_tone_c, live, percent),
                    3 => placement_knob(cx, x, y, 11.0, label, |p| &p.pedal_tone_d, live, percent),
                    _ => placement_knob(cx, x, y, 11.0, label, |p| &p.pedal_tone_e, live, percent),
                }
            }
            let last = x0 + step * (count - 1) as f32;
            // And one with no level control (the Clean Boost, whose one knob is
            // its gain) greys this one.
            placement_knob(
                cx,
                last,
                y,
                11.0,
                level_label,
                |p| &p.pedal_level,
                live && pedal.voice().has_level(),
                percent,
            );
        });
    };

    // The wah, ahead of the pedal: which one, its treadle -- the knob to
    // automate, or to link an expression pedal's controller to; in Auto, where
    // it rests -- and in Auto how hard it has to be played to open it.
    label(cx, "wah", body_x() + 30.0, top + 138.0, 9.5, 76.0, 0x7e8a96);
    selector(
        cx,
        body_x() + 76.0,
        top + 128.0,
        200.0,
        |p| &p.wah,
        vec!["Off", "Black", "Chrome"],
        true,
    );
    {
        let state = parameter_signal.map(|p| (p.wah.value(), p.wah_mode.value()));
        Binding::new(cx, state, move |cx| {
            let (wah, mode) = state.get();
            let live = wah != crate::params::WahModel::Off;
            let x0 = body_x() + 330.0;
            let y = top + 136.0;
            placement_knob(
                cx,
                x0,
                y,
                11.0,
                "treadle",
                |p| &p.wah_treadle,
                live,
                percent,
            );
            selector(
                cx,
                x0 + 50.0,
                y - 10.0,
                130.0,
                |p| &p.wah_mode,
                vec!["Manual", "Auto"],
                live,
            );
            placement_knob(
                cx,
                x0 + 230.0,
                y,
                11.0,
                "sense",
                |p| &p.wah_sense,
                live && mode == crate::params::WahMode::Auto,
                percent,
            );
        });
    };
}

// ---------------------------------------------------------------------------
// 2 Circuit
// ---------------------------------------------------------------------------

fn circuit(cx: &mut Context) {
    let parameter_signal = cx.data::<Panel>().parameter_signal;
    let top = 0.0;

    // Six lists in two columns, read left to right in the order the signal
    // meets them: what the circuit is, what part bends and what part
    // amplifies, then the iron and the power stage it comes out through.
    //
    // The circuit list is one parameter behind two buttons. The first seven
    // entries are topologies -- a valve cascade, a clipper, a channel -- and
    // the rest are models of particular circuits built from their schematics.
    // Those are different kinds of claim and deserve to look it; the button
    // that does not hold the current circuit shows a dash.
    //
    // Clipping and amplifier apply to some circuits and not others. The ones
    // that do not are greyed rather than hidden: a panel that changes shape as
    // the selection moves is harder to aim at.
    let grid = Grid::new(top);
    grid.dropdown(cx, 0, 0, "topology", |p| &p.circuit, Choice::Topology, true);
    grid.dropdown(cx, 1, 0, "modelled", |p| &p.circuit, Choice::Modelled, true);

    {
        let live = parameter_signal.map(|p| p.circuit.value().has_diodes());
        Binding::new(cx, live, move |cx| {
            let live = live.get();
            grid.dropdown(cx, 0, 1, "clipping", |p| &p.diode, Choice::Clipping, live);
        });
    };
    {
        let live = parameter_signal.map(|p| p.circuit.value().has_amplifier());
        Binding::new(cx, live, move |cx| {
            let live = live.get();
            grid.dropdown(
                cx,
                1,
                1,
                "amplifier",
                |p| &p.amplifier,
                Choice::Amplifier,
                live,
            );
        });
    };

    // Iron applies to everything, which is the point of it being a control
    // rather than part of a circuit: a transformer belongs after a distortion
    // pedal exactly as much as after a console channel.
    grid.dropdown(cx, 0, 2, "iron", |p| &p.iron, Choice::Iron, true);
    grid.dropdown(
        cx,
        1,
        2,
        "power amp",
        |p| &p.power_amp,
        Choice::PowerAmp,
        true,
    );

    // What the amplifier is plugged into. A variac is how a rig was wired
    // rather than something anybody sweeps, so it sits with the selections and
    // not on a knob: every supply in the circuit and its power stage comes down
    // together, which is what a valve amplifier on low mains does.
    grid.dropdown(cx, 0, 3, "mains", |p| &p.mains, Choice::Mains, true);

    // Switched input jacks are part of the amplifier's wiring, not an
    // arbitrary gain trim, so the two selectors carry the values the circuit
    // in use actually switches. Both stay visible and go grey where the
    // amplifier has not got them, so the panel does not change shape when
    // circuits are changed -- and they are separate: the American Deluxe has
    // the jacks but its Bright capacitor is soldered in, so the first is live
    // there and the second is not.
    {
        let circuit = parameter_signal.map(|p| p.circuit.value());
        Binding::new(cx, circuit, move |cx| {
            let voice = circuit.get().voice();
            let jacks = voice.input_jacks();
            let bright = voice.bright_switch();
            grid.caption(cx, 1, 3, "sensitivity", jacks.is_some());
            selector(
                cx,
                Grid::right(),
                grid.y(3),
                Grid::right_w(),
                |p| &p.twin_low_input,
                vec![
                    jacks.map_or("High 1 (1 MΩ)", |j| j.high_label),
                    jacks.map_or("Low 2 (-6 dB)", |j| j.low_label),
                ],
                jacks.is_some(),
            );
            grid.caption(cx, 1, 4, "bright", bright.is_some());
            selector(
                cx,
                Grid::right(),
                grid.y(4),
                Grid::right_w(),
                |p| &p.twin_bright,
                vec!["Off", bright.map_or("On (120 pF)", |b| b.on_label)],
                bright.is_some(),
            );
        });
    };

    // A circuit's own three-position switches, named as the circuit names its
    // positions -- the SVT's BASS CUT / OFF / ULTRA LO beside its ULTRA HI, and
    // its 220 Hz / 800 Hz / 3 kHz beneath -- and grey where the circuit has
    // none, for the same reason as the two above.
    {
        let circuit = parameter_signal.map(|p| p.circuit.value());
        Binding::new(cx, circuit, move |cx| {
            let voice = circuit.get().voice();
            // Each row shows the circuit's own positions out of the parameter's
            // four, so a two- or three-position switch is that many segments.
            // A value past a circuit's last position plays as its last
            // (`CircuitSwitch::at`), and the row lights its last to match.
            let low = voice.low_switch();
            grid.caption(cx, 0, 4, "low switch", low.is_some());
            match low {
                Some(s) => Selector::capped(
                    cx,
                    parameter_signal,
                    |p| &p.low_switch,
                    s.labels.to_vec(),
                    Switch::ALL.len(),
                    s.labels.len() - 1,
                ),
                None => Selector::window(
                    cx,
                    parameter_signal,
                    |p| &p.low_switch,
                    vec!["Left", "Centre", "Right"],
                    false,
                    0,
                    Switch::ALL.len(),
                    None,
                ),
            }
            .position_type(PositionType::Absolute)
            .left(Pixels(Grid::left()))
            .top(Pixels(grid.y(4)))
            .width(Pixels(Grid::LEFT_W))
            .height(Pixels(20.0));
            let mid = voice.mid_switch();
            grid.caption(cx, 0, 5, "mid switch", mid.is_some());
            match mid {
                Some(s) => Selector::capped(
                    cx,
                    parameter_signal,
                    |p| &p.mid_switch,
                    s.labels.to_vec(),
                    Switch::ALL.len(),
                    s.labels.len() - 1,
                ),
                None => Selector::window(
                    cx,
                    parameter_signal,
                    |p| &p.mid_switch,
                    vec!["Left", "Centre", "Right"],
                    false,
                    0,
                    Switch::ALL.len(),
                    None,
                ),
            }
            .position_type(PositionType::Absolute)
            .left(Pixels(Grid::left()))
            .top(Pixels(grid.y(5)))
            .width(Pixels(Grid::LEFT_W))
            .height(Pixels(20.0));
        });
    };

    // The one piece of prose that earns its space: it changes with the
    // selection, so it is telling you something you cannot see elsewhere.
    Label::new(cx, parameter_signal.map(|p| describe(p.circuit.value())))
        .position_type(PositionType::Absolute)
        .left(Pixels(body_x()))
        .top(Pixels(top + 182.0))
        .width(Pixels(body_w()))
        .height(Pixels(22.0))
        .alignment(Alignment::Left)
        .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
        .font_size(9.5)
        .color(Color::rgb(0x86, 0x92, 0x9c))
        .hoverable(false);
}

/// A named selection row: the caption in the gutter, the choices beside it.
fn row<P, F>(
    cx: &mut Context,
    y: f32,
    name: &str,
    to_param: F,
    labels: Vec<&'static str>,
    enabled: bool,
    width: f32,
) where
    F: Fn(&Arc<GainStageParams>) -> &P + Copy + 'static,
    P: nice_plug::prelude::Param + 'static,
{
    label(
        cx,
        name,
        body_x() + 30.0,
        y + 10.0,
        9.5,
        76.0,
        if enabled { 0x7e8a96 } else { 0x5a636b },
    );
    selector(cx, body_x() + 76.0, y, width, to_param, labels, enabled);
}

/// Two columns of captioned dropdowns, the layout the circuit and cabinet
/// sections share so their lists line up down the panel.
#[derive(Clone, Copy)]
struct Grid {
    top: f32,
}

impl Grid {
    const LEFT_W: f32 = 230.0;
    const ROW: f32 = 28.0;

    fn new(top: f32) -> Self {
        Self { top }
    }

    fn left() -> f32 {
        body_x() + 76.0
    }

    fn right() -> f32 {
        Self::left() + Self::LEFT_W + 64.0
    }

    fn right_w() -> f32 {
        body_x() + body_w() - Self::right()
    }

    /// The top of a row's controls.
    fn y(&self, row: usize) -> f32 {
        self.top + 10.0 + row as f32 * Self::ROW
    }

    /// A caption in front of column `column` of row `row`.
    fn caption(&self, cx: &mut Context, column: usize, row: usize, name: &str, live: bool) {
        let colour = if live { 0x7e8a96 } else { 0x5a636b };
        let y = self.y(row) + 10.0;
        if column == 0 {
            label(cx, name, body_x() + 30.0, y, 9.5, 76.0, colour);
        } else {
            label(cx, name, Self::right() - 32.0, y, 9.5, 60.0, colour);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dropdown<P, F>(
        &self,
        cx: &mut Context,
        column: usize,
        row: usize,
        name: &str,
        to_param: F,
        choice: Choice,
        live: bool,
    ) where
        F: Fn(&Arc<GainStageParams>) -> &P + Copy + 'static,
        P: nice_plug::prelude::Param + 'static,
    {
        let parameter_signal = cx.data::<Panel>().parameter_signal;
        self.caption(cx, column, row, name, live);
        let (x, w) = if column == 0 {
            (Self::left(), Self::LEFT_W)
        } else {
            (Self::right(), Self::right_w())
        };
        DropButton::new(cx, parameter_signal, to_param, choice, live)
            .position_type(PositionType::Absolute)
            .left(Pixels(x))
            .top(Pixels(self.y(row)))
            .width(Pixels(w))
            .height(Pixels(20.0));
    }
}

/// One line saying what the selected circuit actually is. It changes with the
/// selection, so it is a single line rather than a dozen pieces of permanent
/// small print nobody reads.
///
/// Public so a test can check each one fits the row it is drawn on: a label
/// wider than its box is not wrapped or clipped, it spills across whatever is
/// beside it.
pub fn describe(circuit: Circuit) -> String {
    match circuit {
        Circuit::Clean => {
            "One valve stage barely working: a signal having been \
                           through something, not distortion."
        }
        Circuit::Crunch => {
            "Two stages, the second driven by the first, so each \
                            amplifies the last one's distortion as well."
        }
        Circuit::HighGain => {
            "Three stages run hard, all clipping on every note. \
                              Where the gain stops being a texture."
        }
        Circuit::Overdrive => {
            "Diodes across the feedback resistor: they lower the \
                               gain, so it keeps following and cleans up."
        }
        Circuit::Distortion => {
            "Diodes across the signal to ground: a ceiling. The \
                                wave is squared off, top to bottom of the band."
        }
        Circuit::Console => {
            "A step-up transformer into a discrete stage, built \
                             not to run out of room."
        }
        Circuit::Studio => {
            "An op-amp on a studio rail: nothing of its own \
                            anywhere in the band. Add iron to give it some."
        }
        // The modelled circuits say what they were modelled after in generic
        // terms. The hardware each was researched from is named in the
        // developer documents (docs/MODEL_INVENTORY.md), not on the panel.
        Circuit::Screamer => {
            "Modeled after a late-70s green overdrive pedal. It leaves the bass \
                              alone, so it sits in front of an amp."
        }
        Circuit::Muff => {
            "Modeled after a 1973 four-transistor fuzz: two clipping stages, \
                          and the tone control is a mid scoop."
        }
        Circuit::Boogie => {
            "Modeled after an early-80s Californian lead channel: six triodes, \
                            its own stack and a five-band graphic."
        }
        Circuit::Peavey => {
            "Modeled after a 90s American high-gain lead channel: six triodes, \
                            one run cold to square off the bottom."
        }
        Circuit::Neve => {
            "Modeled after a British class-A console mic preamp: two transistor \
                          stages and a step-up transformer."
        }
        Circuit::Twin => {
            "Modeled after a 60s American blackface clean amp, with its own \
                          stack, spring reverb and tremolo."
        }
        Circuit::Brit800 => {
            "Modeled after an early-80s British 100 W master-volume lead amp: \
                             four triodes into its own stack."
        }
        Circuit::American312 => {
            "Modeled after an American console mic preamp card: input iron, one \
                                 discrete op-amp, output iron."
        }
        Circuit::ConsoleE => {
            "Modeled after a British 80s console's mic input: a 1:10 transformer \
                              and two op-amps around one pot."
        }
        Circuit::Tube610 => {
            "Modeled after a 60s American valve console channel: four triodes in \
                              two feedback loops, iron each end."
        }
        Circuit::Plexi => {
            "Modeled after a late-60s British 100 W lead amp with no master: \
                            three triodes, then the power valves."
        }
        Circuit::AC30 => {
            "Modeled after a 60s British 30 W combo: a top-boost valve, no \
                          middle knob, and self-biased power valves."
        }
        Circuit::DR103 => {
            "Modeled after a British 100 W head built for headroom: five \
                           triodes, a master volume and a tight loop."
        }
        Circuit::Recto => {
            "Modeled after a 90s American two-channel head: five triodes, one \
                          run cold, and a choice of rectifier."
        }
        Circuit::Green9 => {
            "The green overdrive with the later pedal's output \
                          resistors: the same circuit, a little quieter."
        }
        Circuit::Rat => {
            "A hard-clipping distortion whose slow op-amp runs out of \
                          gain-bandwidth before it runs out of gain."
        }
        Circuit::FuzzFace => {
            "Two germanium transistors and a feedback resistor, which \
                          is the whole pedal."
        }
        Circuit::DistPlus => {
            "One slow op-amp and a pair of germanium diodes to ground: \
                          the simplest distortion here."
        }
        Circuit::Hm2 => {
            "A gated distortion: two germanium diodes in series with \
                          the signal hold quiet playing back entirely."
        }
        Circuit::Mt2 => {
            "Two gain stages and seven filters, with a three band \
                          equaliser whose middle sweeps."
        }
        Circuit::Deluxe => {
            "A 60s American blackface circuit at a quarter of the Twin's \
                          power: two 6V6 behind a valve rectifier."
        }
        Circuit::Jazz120 => {
            "The clean channel of a 70s Japanese solid-state amp: two \
                          JFET stages, no valves, no sag."
        }
        Circuit::DeluxeNormal => {
            "The same blackface amp through its plain channel: no bright \
                          capacitor, no reverb, no tremolo."
        }
        Circuit::Brit2205 => {
            "Modeled after an 80s British 50 W two-channel head's boost \
                          channel: a diode-biased stage and a clipper."
        }
        Circuit::Ds1 => {
            "A transistor clips first and a diode pair clips what is \
                          left, then a scoop: bright, tight distortion."
        }
        Circuit::TrebleBoost => {
            "One germanium transistor behind a 5 nF capacitor: a \
                          treble boost to push a valve amp into breakup."
        }
        Circuit::PlexiBass => {
            "Modeled after a late-60s British 100 W bass head: the lead \
                          amp's sibling, without its bass cut."
        }
        Circuit::Brum100 => {
            "Modeled after a late-60s British 100 W head from the Midlands: \
                          600 V on four EL34s, and no master."
        }
        Circuit::OregonT => {
            "Modeled after an early-70s American 150 W head: four 6550s in \
                          ultra-linear behind a tweed-style preamp."
        }
        Circuit::GoldDrive => {
            "Clipped and clean summed: germanium diodes on one path, two clean \
                          ones beside it, and a treble shelf."
        }
        Circuit::BritDrive => {
            "Two op-amp stages into red LEDs, then a bass, middle and treble \
                          stack that all move each other."
        }
        Circuit::ModernPurple => {
            "Modeled after a modern high-gain pedal: six op-amps, diode and LED clipping, active mid, Aggression."
        }
        Circuit::Modern33 => {
            "Modeled after a modern metal boost: a transistor and an op-amp on 33 V, 22 dB, the lows held back."
        }
        Circuit::BlueChorus => {
            "Modeled after a late-70s blue chorus: a bucket-brigade delay swept by an oscillator, mixed with the dry."
        }
        Circuit::OrangePhase => {
            "Modeled after a 70s orange phaser: four JFET stages swept by an oscillator, one knob, script or block."
        }
        Circuit::American800RB => {
            "Modeled after an 80s solid-state bass head: op-amp preamp, four-band EQ, a boost, 300 W."
        }
        Circuit::BassDriver => {
            "A bass preamp and DI in a pedal: drive into a zener pair, two filters, a dry blend, an active EQ."
        }
        Circuit::German76 => {
            "Modeled after a late-50s German broadcast valve amplifier: four \
                          valves, two transformers, stepped gain."
        }
        Circuit::British47 => {
            "Modeled after a late-50s British studio valve amplifier: two valves, \
                          two transformers, its gain a switch."
        }
        Circuit::AmericanSvt => {
            "Modeled after a 300 W American bass head: six 6550s on cathode \
                          followers, a midrange in a feedback loop."
        }
        Circuit::CleanBoost => {
            "One op-amp and one knob: up to 26 dB, flat, until a hot pickup \
                          meets its 9 V rail."
        }
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// 3 Drive
// ---------------------------------------------------------------------------

fn drive(cx: &mut Context) {
    let parameter_signal = cx.data::<Panel>().parameter_signal;
    let top = 0.0;

    // Named after the pot it turns on the device selected, not after the
    // section. See `Gain::drive_name`.
    Knob::new(cx, parameter_signal, |p| &p.drive, 21.0, true)
        .position_type(PositionType::Absolute)
        .left(Pixels(body_x() + 30.0 - 21.0))
        .top(Pixels(top + 24.0 - 21.0));
    Label::new(
        cx,
        parameter_signal.map(|p| String::from(p.circuit.value().voice().drive_name())),
    )
    .position_type(PositionType::Absolute)
    .left(Pixels(body_x() + 30.0 - 50.0))
    .top(Pixels(top + 24.0 + 21.0 + 10.0 - LABEL_H / 2.0))
    .width(Pixels(100.0))
    .height(Pixels(LABEL_H))
    .alignment(Alignment::Center)
    .text_align(TextAlign::Center)
    .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
    .font_size(9.5)
    .color(Color::rgb(0x9a, 0xa6, 0xb0))
    .hoverable(false);
    Label::new(
        cx,
        parameter_signal.map(|p| format!("{:.0} %", p.drive.value() * 100.0)),
    )
    .position_type(PositionType::Absolute)
    .left(Pixels(body_x() + 30.0 - 50.0))
    .top(Pixels(top + 24.0 + 21.0 + 21.0 - LABEL_H / 2.0))
    .width(Pixels(100.0))
    .height(Pixels(LABEL_H))
    .alignment(Alignment::Center)
    .text_align(TextAlign::Center)
    .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
    .font_size(9.5)
    .color(Color::rgb(0xff, 0xb2, 0x6a))
    .hoverable(false);

    // The circuit's own level control, beside the Drive it answers to: gain in
    // front, level behind, which is how both amplifiers with a master are
    // played and how a pedal's two knobs are laid out.
    //
    // Greyed where the drawing has no such control -- and the Twin Reverb is
    // one of those. An AB763 has no master volume; its channel Volume is
    // already the Drive knob. See `Gain::level_control`.
    // Named as the drive is: MASTER on an amplifier, and a pedal's own name
    // for it -- LEVEL, VOLUME, OUTPUT -- when a pedal is the circuit. See
    // `Gain::level_name`.
    {
        let live =
            parameter_signal.map(|p| (p.master_enabled(), p.circuit.value().voice().level_name()));
        Binding::new(cx, live, move |cx| {
            let (live, name) = live.get();
            Knob::new(cx, parameter_signal, |p| &p.master, 21.0, live)
                .position_type(PositionType::Absolute)
                .left(Pixels(body_x() + 110.0 - 21.0))
                .top(Pixels(top + 24.0 - 21.0));
            label(
                cx,
                name,
                body_x() + 110.0,
                top + 24.0 + 21.0 + 10.0,
                9.5,
                100.0,
                if live { 0x9aa6b0 } else { 0x5a636b },
            );
        });
    };
    Label::new(
        cx,
        parameter_signal.map(|p| {
            if p.master_enabled() {
                format!("{:.0} %", p.master.value() * 100.0)
            } else {
                String::from("--")
            }
        }),
    )
    .position_type(PositionType::Absolute)
    .left(Pixels(body_x() + 110.0 - 50.0))
    .top(Pixels(top + 24.0 + 21.0 + 21.0 - LABEL_H / 2.0))
    .width(Pixels(100.0))
    .height(Pixels(LABEL_H))
    .alignment(Alignment::Center)
    .text_align(TextAlign::Center)
    .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
    .font_size(9.5)
    .color(Color::rgb(0xff, 0x8a, 0x3c))
    .hoverable(false);

    // The power stage's presence -- the AC30's cut -- named after what the
    // stage in the path has in that slot, and greyed where it has nothing:
    // the AB763s, the transistor stages and Bypass. The DR103's is its
    // driver's (`power::DriverSpec`); the Cali Rectifier's is in its
    // preamplifier (`Gain::own_presence`). See `Chain::set_presence`.
    {
        let name = parameter_signal.map(|p| p.presence_name());
        Binding::new(cx, name, move |cx| {
            let name = name.get();
            let live = name.is_some();
            Knob::new(cx, parameter_signal, |p| &p.presence, 21.0, live)
                .position_type(PositionType::Absolute)
                .left(Pixels(body_x() + 190.0 - 21.0))
                .top(Pixels(top + 24.0 - 21.0));
            label(
                cx,
                name.unwrap_or("PRESENCE"),
                body_x() + 190.0,
                top + 24.0 + 21.0 + 10.0,
                9.5,
                100.0,
                if live { 0x9aa6b0 } else { 0x5a636b },
            );
        });
    };
    Label::new(
        cx,
        parameter_signal.map(|p| {
            if p.presence_name().is_some() {
                format!("{:.0} %", p.presence.value() * 100.0)
            } else {
                String::from("--")
            }
        }),
    )
    .position_type(PositionType::Absolute)
    .left(Pixels(body_x() + 190.0 - 50.0))
    .top(Pixels(top + 24.0 + 21.0 + 21.0 - LABEL_H / 2.0))
    .width(Pixels(100.0))
    .height(Pixels(LABEL_H))
    .alignment(Alignment::Center)
    .text_align(TextAlign::Center)
    .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
    .font_size(9.5)
    .color(Color::rgb(0xff, 0x8a, 0x3c))
    .hoverable(false);

    let x = body_x() + 250.0 + (body_w() - 260.0) / 2.0;
    for (i, line) in [
        "All the way up on Drive is the sound the circuit is named for;",
        "down from there only cleans up, and the level is held across it.",
        "Master (a pedal's Level) and Presence start at half, where the voice",
        "was calibrated; Presence is the power stage's where it has one.",
    ]
    .into_iter()
    .enumerate()
    {
        label(
            cx,
            line,
            x,
            top + 10.0 + i as f32 * 16.0,
            9.5,
            body_w() - 260.0,
            0x86929c,
        );
    }

    // The Mark IIC+'s five band graphic equaliser.
    //
    // Faders, because that is what the amplifier has, and because the point of
    // a graphic equaliser is that the shape of the curve is the shape of the
    // row -- five knobs would say the same thing and show none of it. They sit
    // in this section rather than in the tone one because on the amplifier
    // they are late in the preamplifier and *before* the power stage, and the
    // panel reads in signal order (§9.8).
    const BANDS: [(ToKnob, &str); 5] = [
        (|p| &p.eq60, "60"),
        (|p| &p.eq240, "240"),
        (|p| &p.eq750, "750"),
        (|p| &p.eq2200, "2.2k"),
        (|p| &p.eq6600, "6.6k"),
    ];
    {
        let live = parameter_signal.map(|p| p.circuit.value().voice().has_graphic());
        Binding::new(cx, live, move |cx| {
            let live = live.get();
            let base = top + 74.0;
            label(
                cx,
                "graphic",
                body_x() + 30.0,
                base + 30.0,
                9.5,
                76.0,
                if live { 0x7e8a96 } else { 0x5a636b },
            );
            for (i, (to_param, name)) in BANDS.into_iter().enumerate() {
                let x = body_x() + 96.0 + i as f32 * 46.0;
                Knob::fader(cx, parameter_signal, to_param, 22.0, 58.0, live)
                    .position_type(PositionType::Absolute)
                    .left(Pixels(x - 11.0))
                    .top(Pixels(base));
                label(
                    cx,
                    name,
                    x,
                    base + 68.0,
                    9.0,
                    46.0,
                    if live { 0x9aa6b0 } else { 0x5a636b },
                );
            }
            let note = if live {
                "Five sliders, late in the preamp and before the power stage."
            } else {
                "Only the Cali IIC+ has one."
            };
            label(
                cx,
                note,
                body_x() + 340.0 + 120.0,
                base + 30.0,
                9.5,
                260.0,
                0x86929c,
            );
        });
    };
}

// ---------------------------------------------------------------------------
// 4 Tone
// ---------------------------------------------------------------------------

/// What the three tone knobs are, for the panel as it stands: which of them
/// reach anything, and what to call them.
///
/// A type rather than a pair of bindings because vizia rebuilds on one lens at
/// a time, and both the stack selector and the circuit selector change the
/// answer. Public so a test can ask the same question the panel does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ToneKnobs {
    /// Bass, middle, treble: whether each reaches a control.
    pub live: [bool; 3],
    /// What the panel writes under each.
    pub names: [&'static str; 3],
    /// Whether the Twin's reverb and tremolo knobs reach anything.
    pub extras: bool,
    /// A fourth tone knob, for a circuit with a control of its own past bass,
    /// middle and treble, and what it is called. Only the Metal Zone has one.
    pub sweep: Option<&'static str>,
    /// The Heavy Metal's dedicated Colour Mix pair. These are deliberately
    /// separate from the generic Bass/Treble stack controls.
    pub colour_mix: Option<[&'static str; 2]>,
    /// A pedal circuit's single tone control, and what its box calls it: a
    /// knob of its own in the fourth column. See `Gain::own_single_tone`.
    pub single: Option<&'static str>,
    /// Whether the Chorus knob reaches anything. Only the Jazz 120 has a
    /// bucket brigade; see `Gain::has_chorus`.
    pub chorus: bool,
}

// Written out rather than derived: vizia's derive asks every field to be
// `Data` itself, and an array is not one.

impl ToneKnobs {
    pub fn of(params: &Arc<GainStageParams>) -> Self {
        Self::for_state(params.circuit.value(), params.tone.value())
    }

    pub fn for_state(circuit: Circuit, stack: ToneStack) -> Self {
        let in_circuit = stack != ToneStack::Off;
        let own = circuit.own_tone_knobs();
        Self {
            live: [
                in_circuit || own[0],
                in_circuit || own[1],
                in_circuit || own[2],
            ],
            names: ["BASS", "MID", "TREBLE"],
            extras: circuit.has_reverb_and_tremolo(),
            sweep: circuit.voice().own_sweep().map(|(_, name)| name),
            colour_mix: circuit
                .voice()
                .own_colour_mix()
                .map(|((_, low), (_, high))| [low, high]),
            single: circuit.voice().own_single_tone().map(|(_, name, _)| name),
            chorus: circuit.voice().has_chorus(),
        }
    }
}

fn tone(cx: &mut Context) {
    let parameter_signal = cx.data::<Panel>().parameter_signal;
    let top = 0.0;

    row(
        cx,
        top + 8.0,
        "stack",
        |p| &p.tone,
        ToneStack::ALL.iter().map(|t| t.name()).collect(),
        true,
        230.0,
    );

    let knobs: [ToKnob; 3] = [|p| &p.bass, |p| &p.mid, |p| &p.treble];
    // The Twin Reverb's own three, on a second row directly beneath the
    // stack's and aligned to the same columns. They belong to section 4
    // because they are what is done to the sound after it is made.
    //
    // They were beside rather than below, to keep the window the height it
    // was. That was the wrong trade: the row is only wide enough for six if
    // the paragraph beside it is not there, and it is -- so Speed and
    // Intensity sat on top of the text explaining the stack. The section is
    // now tall enough for the row, which makes the window sixty-four pixels
    // taller for every circuit, and that is the price.
    let extras: [ToKnob; 3] = [|p| &p.reverb, |p| &p.speed, |p| &p.intensity];
    const EXTRA_NAMES: [&str; 3] = ["REVERB", "SPEED", "INTENSITY"];
    // A knob is live when it reaches something, and there are two ways it can:
    // the plugin's own stack when that is in circuit, or the selected
    // circuit's own tone controls where it has any.
    //
    // Greying on the stack alone was wrong in both directions. Fourteen of the
    // shipped presets switch the stack out, and a knob that turns and changes
    // nothing is indistinguishable from a fault -- which is how it was
    // reported, and why the greying went in. But an amplifier's own stack is
    // wired to these knobs in `voice::set_tone_knobs`, and its presets switch
    // the plugin's stack out too, so greying on the stack alone greyed knobs
    // that were working -- the same fault the other way round. (A pedal's
    // single tone control rode on the third knob for the same reason until
    // 2026-09-30; it has its own now, in the fourth column.)
    {
        let state = parameter_signal.map(ToneKnobs::of);
        Binding::new(cx, state, move |cx| {
            let state = state.get();
            let top = 0.0;
            // The fourth knob, where the circuit has a control of its own past the
            // three. It draws in the row's fourth column, which is clear of the
            // paragraph beside it, and only for the circuit that has one.
            if let Some(name) = state.sweep {
                let x = body_x() + 46.0 + 3.0 * 84.0;
                Knob::new(cx, parameter_signal, |p| &p.tone_sweep, 18.0, true)
                    .position_type(PositionType::Absolute)
                    .left(Pixels(x - 18.0))
                    .top(Pixels(top + 40.0));
                label(cx, name, x, top + 86.0, 9.5, 80.0, 0x9aa6b0);
            }
            // A pedal's own tone control, selected as the circuit: its own knob
            // and its own name, as it has in the pedal slot, rather than a ride
            // on the stack's Treble. Same column as the Metal Zone's sweep; no
            // circuit has both.
            if let Some(name) = state.single {
                let x = body_x() + 46.0 + 3.0 * 84.0;
                Knob::new(cx, parameter_signal, |p| &p.circuit_tone, 18.0, true)
                    .position_type(PositionType::Absolute)
                    .left(Pixels(x - 18.0))
                    .top(Pixels(top + 40.0));
                label(cx, name, x, top + 86.0, 9.5, 80.0, 0x9aa6b0);
            }
            if let Some(names) = state.colour_mix {
                let controls: [ToKnob; 2] = [|p| &p.hm2_colour_lo, |p| &p.hm2_colour_hi];
                for (i, (to_param, name)) in controls.into_iter().zip(names).enumerate() {
                    let x = body_x() + 46.0 + (3 + i) as f32 * 84.0;
                    Knob::new(cx, parameter_signal, to_param, 18.0, true)
                        .position_type(PositionType::Absolute)
                        .left(Pixels(x - 18.0))
                        .top(Pixels(top + 40.0));
                    label(cx, name, x, top + 86.0, 9.5, 80.0, 0x9aa6b0);
                }
            }
            for (i, to_param) in knobs.into_iter().enumerate() {
                let live = state.live[i];
                let x = body_x() + 46.0 + i as f32 * 84.0;
                Knob::new(cx, parameter_signal, to_param, 18.0, live)
                    .position_type(PositionType::Absolute)
                    .left(Pixels(x - 18.0))
                    .top(Pixels(top + 40.0));
                label(
                    cx,
                    state.names[i],
                    x,
                    top + 86.0,
                    9.5,
                    80.0,
                    if live { 0x9aa6b0 } else { 0x5a636b },
                );
            }
            // Greyed unless the selected circuit has a tank and a tremolo, which
            // only the blackface amplifiers' vibrato channels do. A knob that turns and reaches nothing is
            // indistinguishable from a fault -- the same reason the three beside
            // them are greyed, and the same reason BUG-023 happened.
            let live = state.extras;
            for (i, to_param) in extras.into_iter().enumerate() {
                let x = body_x() + 46.0 + i as f32 * 84.0;
                Knob::new(cx, parameter_signal, to_param, 18.0, live)
                    .position_type(PositionType::Absolute)
                    .left(Pixels(x - 18.0))
                    .top(Pixels(top + 104.0));
                label(
                    cx,
                    EXTRA_NAMES[i],
                    x,
                    top + 150.0,
                    9.5,
                    80.0,
                    if live { 0x9aa6b0 } else { 0x5a636b },
                );
            }
            // The Jazz 120's chorus, in the fourth column of the same row: it is
            // the amplifier's own effect and belongs beside the amplifier's own
            // reverb and tremolo rather than in a section of its own. Drawn for
            // every circuit and greyed where it reaches nothing, which is what
            // the three beside it do.
            {
                let live = state.chorus;
                let x = body_x() + 46.0 + 3.0 * 84.0;
                Knob::new(cx, parameter_signal, |p| &p.chorus, 18.0, live)
                    .position_type(PositionType::Absolute)
                    .left(Pixels(x - 18.0))
                    .top(Pixels(top + 104.0));
                label(
                    cx,
                    "CHORUS",
                    x,
                    top + 150.0,
                    9.5,
                    80.0,
                    if live { 0x9aa6b0 } else { 0x5a636b },
                );
            }
        });
    };

    // Kept to lines that fit the space rather than sentences that overflow
    // it: text wider than its box is simply clipped, with no warning.
    //
    // Two paragraphs, one a row: the top three knobs are the tone stack, the
    // bottom three are the Twin's reverb and tremolo, and the text sits beside
    // the row it is about.
    // Circuit-specific controls occupy columns four and five: MT-2 uses the
    // fourth for Mid Freq and HM-2 uses both for Colour Low/High. Keep the
    // explanatory copy to their right instead of letting controls sit on it.
    let x = body_x() + 555.0;
    for (i, line) in [
        "A passive stack only ever cuts. The",
        "scooping voicing dips the middle. An",
        "amplifier's own stack uses these three;",
        "a pedal's own controls sit beside them.",
    ]
    .into_iter()
    .enumerate()
    {
        label(
            cx,
            line,
            x,
            top + 40.0 + i as f32 * 16.0,
            9.5,
            240.0,
            0x86929c,
        );
    }
    for (i, line) in [
        "Below: the blackface amps' spring tank",
        "and optical tremolo. Chorus is the",
        "Jazz 120's bucket brigade, which sends",
        "one of its two speakers the delay.",
    ]
    .into_iter()
    .enumerate()
    {
        label(
            cx,
            line,
            x,
            top + 104.0 + i as f32 * 16.0,
            9.5,
            240.0,
            0x86929c,
        );
    }
}

// ---------------------------------------------------------------------------
// 5 Cabinet
// ---------------------------------------------------------------------------

fn cabinet(cx: &mut Context) {
    let parameter_signal = cx.data::<Panel>().parameter_signal;
    let top = 0.0;
    let grid = Grid::new(top);
    let left = Grid::left();
    let right = Grid::right();
    let rest = Grid::right_w();
    let dim = |live: bool| if live { 0x7e8a96 } else { 0x5a636b };

    // --- which cabinet, and the legacy filter it replaces -------------------
    // Legacy is what every old session is: the resistor load and the baked
    // Combo/Stack response. The two are never stacked, so the legacy row only
    // applies while Legacy is chosen.
    grid.dropdown(cx, 0, 0, "cabinet", |p| &p.cab_model, Choice::Cabinet, true);
    {
        let legacy = parameter_signal.map(|p| p.cab_model.value() == CabModel::Legacy);
        Binding::new(cx, legacy, move |cx| {
            let live = legacy.get();
            grid.caption(cx, 1, 0, "legacy", live);
            selector(
                cx,
                right,
                grid.y(0),
                rest,
                |p| &p.cabinet,
                Cabinet::ALL.iter().map(|c| c.name()).collect(),
                live,
            );
        });
    };

    // --- the driver ---------------------------------------------------------
    {
        let live = parameter_signal.map(|p| p.cab_model.value() != CabModel::Legacy);
        Binding::new(cx, live, move |cx| {
            let live = live.get();
            grid.dropdown(cx, 0, 1, "speaker", |p| &p.speaker, Choice::Speaker, live);
        });
    };
    // What the selection physically is, rather than what it is named after.
    Label::new(cx, parameter_signal.map(|p| cabinet_summary(p)))
        .position_type(PositionType::Absolute)
        .left(Pixels(right - 58.0))
        .top(Pixels(grid.y(1)))
        .width(Pixels(rest + 58.0))
        .height(Pixels(20.0))
        .alignment(Alignment::Left)
        .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
        .font_size(9.5)
        .color(Color::rgb(0x86, 0x92, 0x9c))
        .hoverable(false);

    // --- the microphones ----------------------------------------------------
    {
        let live = parameter_signal.map(|p| p.physical_cabinet());
        Binding::new(cx, live, move |cx| {
            let live = live.get();
            // Mic A always exists; Bypass is an ideal omni at the placement.
            grid.dropdown(cx, 0, 2, "mic A", |p| &p.mic_a, Choice::MicA, live);
            grid.dropdown(cx, 1, 2, "mic B", |p| &p.mic_b, Choice::MicB, live);
        });
    };

    // Placement: three for A, three for B and the blend, one row; and the
    // horn's attenuator at its end, live for a cabinet with a horn.
    {
        let flags = parameter_signal.map(|p| {
            let horn = matches!(
                p.cab_model.value().voice(),
                crate::voice::CabinetChoice::Model(cab) if cab.horn.is_some()
            );
            u8::from(p.physical_cabinet())
                | (u8::from(p.mic_b.value() != MicModel::Off) << 1)
                | (u8::from(horn) << 2)
        });
        Binding::new(cx, flags, move |cx| {
            let flags = flags.get();
            let a = flags & 1 != 0;
            let b = a && flags & 2 != 0;
            let horn = a && flags & 4 != 0;
            let step = body_w() / 10.0;
            let x = |i: usize| body_x() + step * (i as f32 + 0.5);
            let y = top + 116.0;
            let r = 15.0;
            placement_knob(
                cx,
                x(0),
                y,
                r,
                "A position",
                |p| &p.mic_a_position,
                a,
                percent,
            );
            placement_knob(
                cx,
                x(1),
                y,
                r,
                "A distance",
                |p| &p.mic_a_distance,
                a,
                centimetres,
            );
            placement_knob(cx, x(2), y, r, "A angle", |p| &p.mic_a_angle, a, degrees);
            // Mic A's pan is live whenever mic A is, so a single microphone can
            // still be placed off centre.
            placement_knob(cx, x(3), y, r, "A pan", |p| &p.mic_a_pan, a, pan_position);
            placement_knob(
                cx,
                x(4),
                y,
                r,
                "B position",
                |p| &p.mic_b_position,
                b,
                percent,
            );
            placement_knob(
                cx,
                x(5),
                y,
                r,
                "B distance",
                |p| &p.mic_b_distance,
                b,
                centimetres,
            );
            placement_knob(cx, x(6), y, r, "B angle", |p| &p.mic_b_angle, b, degrees);
            placement_knob(cx, x(7), y, r, "B pan", |p| &p.mic_b_pan, b, pan_position);
            placement_knob(cx, x(8), y, r, "blend", |p| &p.mic_blend, b, percent);
            placement_knob(cx, x(9), y, r, "horn", |p| &p.cab_horn, horn, percent);

            let row_y = top + 170.0;
            label(
                cx,
                "B polarity",
                body_x() + 30.0,
                row_y + 10.0,
                9.5,
                76.0,
                dim(b),
            );
            selector(
                cx,
                left,
                row_y,
                140.0,
                |p| &p.mic_b_invert,
                vec!["Normal", "Invert"],
                b,
            );
            label(cx, "time", left + 180.0, row_y + 10.0, 9.5, 56.0, dim(b));
            selector(
                cx,
                left + 208.0,
                row_y,
                160.0,
                |p| &p.mic_align,
                vec!["Physical", "Aligned"],
                b,
            );
        });
    };
}

fn percent(v: f32) -> String {
    format!("{:.0} %", v * 100.0)
}

fn centimetres(v: f32) -> String {
    format!("{:.1} cm", v * 100.0)
}

fn degrees(v: f32) -> String {
    format!("{:.0} deg", v)
}

/// Console shorthand for a pan position: centre, or how far to one side.
fn pan_position(v: f32) -> String {
    let position = (v * 100.0).round() as i32;
    match position {
        0 => "C".to_string(),
        p if p < 0 => format!("L {}", -p),
        p => format!("R {p}"),
    }
}

/// A knob for a microphone placement, dimmed when it does not apply.
#[allow(clippy::too_many_arguments)]
fn placement_knob<F>(
    cx: &mut Context,
    x: f32,
    y: f32,
    radius: f32,
    name: &str,
    to_param: F,
    live: bool,
    format: fn(f32) -> String,
) where
    F: Fn(&Arc<GainStageParams>) -> &nice_plug::prelude::FloatParam + Copy + 'static,
{
    let parameter_signal = cx.data::<Panel>().parameter_signal;
    Knob::new(cx, parameter_signal, to_param, radius, live)
        .position_type(PositionType::Absolute)
        .left(Pixels(x - radius))
        .top(Pixels(y - radius));
    label(
        cx,
        name,
        x,
        y + radius + 10.0,
        9.5,
        76.0,
        if live { 0x9aa6b0 } else { 0x5a636b },
    );
    Label::new(
        cx,
        parameter_signal.map(move |p| {
            if live {
                format(to_param(p).value())
            } else {
                String::from("--")
            }
        }),
    )
    .position_type(PositionType::Absolute)
    .left(Pixels(x - 38.0))
    .top(Pixels(y + radius + 21.0 - LABEL_H / 2.0))
    .width(Pixels(76.0))
    .height(Pixels(LABEL_H))
    .alignment(Alignment::Center)
    .text_align(TextAlign::Center)
    .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
    .font_size(9.5)
    .color(Color::rgb(0xff, 0xb2, 0x6a))
    .hoverable(false);
}

/// The selected cabinet and driver, described physically. The product names
/// of the hardware they were researched from stay in the developer documents.
pub fn cabinet_summary(p: &GainStageParams) -> String {
    use crate::voice::{CabinetChoice, SpeakerChoice};
    let cabinet = p.cab_model.value().voice();
    let speaker = p.speaker.value();
    match cabinet {
        CabinetChoice::Legacy => String::from("resistor load, baked cabinet filter"),
        CabinetChoice::Bypass if speaker == SpeakerModel::Bypass => String::from("power stage DI"),
        CabinetChoice::Bypass => String::from("driver on an open baffle"),
        CabinetChoice::Model(_) if speaker == SpeakerModel::Bypass => {
            String::from("power stage DI")
        }
        CabinetChoice::Model(cab) => {
            let driver = match speaker.voice() {
                SpeakerChoice::Model(profile) => profile.name,
                _ => cab.default_speaker.name,
            };
            format!(
                "{} back, {}x12, {:.0} L, {}",
                if cab.is_open() { "open" } else { "closed" },
                cab.drivers,
                cab.volume() * 1000.0,
                driver
            )
        }
    }
}

// ---------------------------------------------------------------------------
// 6 Output
// ---------------------------------------------------------------------------

fn output(cx: &mut Context) {
    let top = 0.0;

    knob(
        cx,
        body_x() + 30.0,
        top + 24.0,
        19.0,
        "MIX",
        |p| &p.mix,
        |p| format!("{:.0} %", p.mix.value() * 100.0),
    );
    knob(
        cx,
        body_x() + 120.0,
        top + 24.0,
        19.0,
        "LEVEL",
        |p| &p.output_trim,
        |p| format!("{:+.1} dB", p.output_trim.value()),
    );

    // Where the dry signal comes from and where it goes: the input, the
    // pedal's output or the circuit's -- a DI box, a DI pedal, an amplifier's
    // direct out -- blended by Mix or split onto the right channel. See
    // `docs/DI.md`.
    let x = body_x() + 180.0;
    label(cx, "dry from", x + 100.0, top + 10.0, 9.5, 200.0, 0x9aa6b0);
    selector(
        cx,
        x,
        top + 18.0,
        200.0,
        |p| &p.dry_source,
        vec!["Input", "Pedal", "Preamp"],
        true,
    );
    label(cx, "dry route", x + 100.0, top + 50.0, 9.5, 200.0, 0x9aa6b0);
    selector(
        cx,
        x,
        top + 58.0,
        200.0,
        |p| &p.dry_route,
        vec!["Mix", "Split L/R"],
        true,
    );

    let x = body_x() + 530.0;
    label(
        cx,
        "The dry signal -- input, pedal or preamp,",
        x,
        top + 26.0,
        9.5,
        250.0,
        0x86929c,
    );
    label(
        cx,
        "a DI -- is delayed to match the amplifier.",
        x,
        top + 42.0,
        9.5,
        250.0,
        0x86929c,
    );
    label(
        cx,
        "Split puts the amp left and the DI right.",
        x,
        top + 58.0,
        9.5,
        250.0,
        0x86929c,
    );
}
