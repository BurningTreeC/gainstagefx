//! The preset strip, the menu behind it, and the two dialogs.
//!
//! The menu scrolls, and it has to. A shipped catalogue is a known size and
//! could be laid out to fit; saved presets are not, and a list that runs off
//! the bottom of the window puts everything past the fold permanently out of
//! reach. So the list is a scrolling view from the start rather than a grid
//! that fits today and stops fitting the first time somebody saves twenty
//! sounds of their own.
//!
//! Saved presets sit in their own section at the foot of the list rather than
//! mixed into the shipped groups. Which of them you can delete and which you
//! cannot is worth being able to see without clicking.

use super::fonts as vizia_assets;
use super::paint as vg;
use super::paint::PanelCanvas;
use nice_plug::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;
use vizia_plug::vizia::prelude::*;
use vizia_plug::widgets::RawParamEvent;

use super::style::*;
use crate::params::GainStageParams;
use crate::presets::{self, Stored, GROUPS, SAVED};

/// How tall the menu is allowed to get before it scrolls. Short enough to sit
/// inside the window at the size the panel opens at.
/// The preset list's own size.
///
/// It was 330 px tall when the catalogue was thirty presets in nine groups. It
/// is now sixty-seven in twelve, which is about seventeen hundred pixels of
/// list: at 330 the groups near the bottom -- Alternative, Metal / Heavy, Blues
/// -- were four screens down a scroll bar nobody was going to find. The panel
/// is 880 px tall below its header with every section open, so the list may
/// as well use it -- and with sections closed, whatever the window has left.
const MENU_H: f32 = crate::editor::style::PANEL_H - 48.0;
const MENU_W: f32 = 300.0;
const ROW_H: f32 = 22.0;
const HEADING_H: f32 = 24.0;

/// Where the preset button sits in the strip, which is also where the menu
/// hangs from.
pub const BUTTON_X: f32 = 118.0;
pub const BUTTON_W: f32 = 152.0;

/// The sizes the panel can be shown at.
///
/// A plugin that opens at one fixed size is a plugin that is too big on a
/// laptop and too small on a large display, and the panel is drawn rather than
/// pictured so it is sharp at any of them. 100% is drawn at `BASE_DPI`, so 50%
/// is three quarters of a physical pixel per point -- for a small screen, or a
/// panel kept open beside the arrangement.
pub const SCALES: [f64; 9] = [0.5, 0.65, 0.75, 0.9, 1.0, 1.25, 1.5, 1.75, 2.0];
/// Where 100% is in `SCALES`: what the wheel steps from when the current size
/// is not one of them.
const UNITY: usize = {
    let mut i = 0;
    while SCALES[i] != 1.0 {
        i += 1;
    }
    i
};
pub const SIZE_X: f32 = 386.0;
pub const SIZE_W: f32 = 46.0;
/// The size list's rows: shorter than the preset menu's, so that all nine fit
/// under the header of the smallest window, every section closed.
const SIZE_ROW_H: f32 = 19.0;
const SIZES_H: f32 = SIZE_ROW_H * SCALES.len() as f32 + 8.0;

/// Which question, if any, is on screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialog {
    None,
    /// Naming a preset before it is written.
    Save,
    /// Confirming that a save will replace one of yours. Shipped presets are
    /// deliberately not asked about: saving under one of their names writes a
    /// new file beside it and replaces nothing.
    Overwrite,
    Delete,
}

#[derive(Clone, PartialEq)]
pub(super) struct SessionView {
    open: bool,
    sizing: bool,
    scale: f64,
    dialog: Dialog,
    current: String,
    dirty: bool,
    pub(super) deletable: bool,
    draft: String,
    error: String,
    entries: Vec<Stored>,
}

pub struct Session {
    pub(super) view: Signal<SessionView>,
    pub open: bool,
    /// Whether the size list is showing.
    pub sizing: bool,
    /// How much larger than its own coordinates the panel is drawn.
    pub scale: f64,
    pub dialog: Dialog,
    /// The name shown in the strip.
    pub current: String,
    /// Whether the panel has been moved since that preset was loaded.
    pub dirty: bool,
    /// Whether the current preset is one of yours, and so can be deleted.
    pub deletable: bool,
    /// What is being typed into the save dialog.
    pub draft: String,
    pub error: String,
    pub entries: Vec<Stored>,
    params: Arc<GainStageParams>,
    /// The values the current preset was loaded with, which is what `dirty` is
    /// measured against.
    reference: BTreeMap<String, f32>,
}

pub enum SessionEvent {
    Toggle,
    ToggleSize,
    SetScale(f64),
    Close,
    Load(usize),
    /// Step through the whole list, which is what the wheel over the name
    /// does: the quickest way to hear what is in it.
    Step(i32),
    OpenSave,
    OpenDelete,
    Draft(String),
    Confirm,
    Cancel,
}

impl Session {
    pub fn build_into(cx: &mut Context, params: Arc<GainStageParams>, scale: f64) {
        let current = params
            .preset_name
            .lock()
            .map(|n| n.clone())
            .unwrap_or_else(|_| String::from(presets::NONE));
        let entries = presets::load_all(&*params);
        let reference = entries
            .iter()
            .find(|p| p.name == current)
            .map(|p| p.values.clone())
            .unwrap_or_default();
        let deletable = entries.iter().any(|p| !p.built_in && p.name == current);
        Self {
            view: Signal::new(SessionView {
                open: false,
                sizing: false,
                scale,
                dialog: Dialog::None,
                current: current.clone(),
                dirty: false,
                deletable,
                draft: String::new(),
                error: String::new(),
                entries: entries.clone(),
            }),
            open: false,
            sizing: false,
            scale,
            dialog: Dialog::None,
            current,
            dirty: false,
            deletable,
            draft: String::new(),
            error: String::new(),
            entries,
            params,
            reference,
        }
        .build(cx);
    }

    /// Rebuilt rather than patched, so a preset saved a moment ago is in the
    /// list and one deleted by hand outside the plugin is not.
    fn refresh(&mut self) {
        self.entries = presets::load_all(&*self.params);
        self.deletable = self
            .entries
            .iter()
            .any(|p| !p.built_in && p.name == self.current);
    }

    /// Push a preset out as ordinary parameter gestures, so the change is
    /// automatable and undoable like any other edit rather than a set of
    /// assignments the host never hears about.
    fn apply(&mut self, cx: &mut EventContext, index: usize) {
        let Some(preset) = self.entries.get(index).cloned() else {
            return;
        };
        for (id, ptr, _) in self.params.param_map() {
            let Some(&normalised) = preset.values.get(&id) else {
                continue;
            };
            cx.emit(RawParamEvent::BeginSetParameter(ptr));
            cx.emit(RawParamEvent::SetParameterNormalized(ptr, normalised));
            cx.emit(RawParamEvent::EndSetParameter(ptr));
        }
        self.current = preset.name.clone();
        self.reference = preset.values.clone();
        self.dirty = false;
        self.deletable = !preset.built_in;
        if let Ok(mut name) = self.params.preset_name.lock() {
            *name = preset.name;
        }
    }

    fn store(&mut self) {
        let name = self.draft.trim().to_string();
        let preset = presets::capture(&*self.params, &name);
        match presets::save(&preset) {
            Ok(_) => {
                self.current = name.clone();
                self.reference = preset.values.clone();
                self.dirty = false;
                self.dialog = Dialog::None;
                self.error.clear();
                if let Ok(mut stored) = self.params.preset_name.lock() {
                    *stored = name;
                }
                self.refresh();
            }
            Err(err) => {
                // Kept on screen with the reason, rather than closed as though
                // it had worked.
                self.dialog = Dialog::Save;
                self.error = err.to_string();
            }
        }
    }
}

impl Model for Session {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|scale: &vizia_plug::vizia::UserScaleChanged, _| {
            self.scale = scale.0 / self.params.editor_state.base_scale_factor();
            crate::editor::remember_scale(&self.params.editor_state, self.scale);
        });
        // Any parameter movement can make the panel differ from the preset it
        // was loaded from, and moving a control back should make it match
        // again -- so this is compared rather than tracked with a flag.
        event.map(|_: &RawParamEvent, _| {
            self.dirty = !presets::matches(&*self.params, &self.reference);
        });

        event.map(|e: &SessionEvent, meta| {
            match e {
                SessionEvent::Toggle => {
                    self.sizing = false;
                    if self.open {
                        self.open = false;
                    } else {
                        self.refresh();
                        self.open = true;
                    }
                }
                SessionEvent::ToggleSize => {
                    self.open = false;
                    self.sizing = !self.sizing;
                }
                SessionEvent::SetScale(scale) => {
                    self.sizing = false;
                    cx.emit(WindowEvent::SetUserScale(
                        *scale * self.params.editor_state.base_scale_factor(),
                    ));
                }
                SessionEvent::Close => {
                    self.open = false;
                    self.sizing = false;
                }
                SessionEvent::Load(index) => {
                    self.open = false;
                    self.apply(cx, *index);
                }
                SessionEvent::Step(delta) => {
                    let at = self
                        .entries
                        .iter()
                        .position(|p| p.name == self.current)
                        .unwrap_or(0) as i32;
                    let last = self.entries.len().saturating_sub(1) as i32;
                    self.apply(cx, (at + delta).clamp(0, last) as usize);
                }
                SessionEvent::OpenSave => {
                    self.sizing = false;
                    self.open = false;
                    self.refresh();
                    self.draft = offered_name(&self.current);
                    self.error.clear();
                    self.dialog = Dialog::Save;
                }
                SessionEvent::OpenDelete => {
                    self.open = false;
                    self.error.clear();
                    if self.deletable {
                        self.dialog = Dialog::Delete;
                    }
                }
                SessionEvent::Draft(text) => self.draft = text.clone(),
                SessionEvent::Cancel => {
                    self.dialog = Dialog::None;
                    self.error.clear();
                }
                SessionEvent::Confirm => match self.dialog {
                    Dialog::Save => {
                        if self.draft.trim().is_empty() {
                            self.error = String::from("a preset needs a name");
                        } else if presets::name_taken(&self.draft, &self.entries) {
                            self.dialog = Dialog::Overwrite;
                        } else {
                            self.store();
                        }
                    }
                    Dialog::Overwrite => self.store(),
                    Dialog::Delete => match presets::delete(&self.current) {
                        Ok(()) => {
                            self.dialog = Dialog::None;
                            self.error.clear();
                            self.refresh();
                        }
                        Err(err) => self.error = err.to_string(),
                    },
                    Dialog::None => {}
                },
            }
            // The name box is the only thing in the panel that types, and it
            // exists exactly while the save dialog does. On Windows this is
            // what gets it the keyboard: a host's message loop sees every key
            // before the plugin, and some keep the letters for their own
            // shortcuts -- which is how a box that took Delete and the arrows
            // could not be typed into. See `vendor/baseview/src/win/text_input.rs`.
            // Every other platform ignores it. Repeating an unchanged state
            // does nothing, so it is simply kept in step here.
            cx.emit(vizia_plug::vizia::TextInputActive(
                self.dialog == Dialog::Save,
            ));
            meta.consume();
        });
        self.view.set_if_changed(SessionView {
            open: self.open,
            sizing: self.sizing,
            scale: self.scale,
            dialog: self.dialog,
            current: self.current.clone(),
            dirty: self.dirty,
            deletable: self.deletable,
            draft: self.draft.clone(),
            error: self.error.clone(),
            entries: self.entries.clone(),
        });
    }
}

/// What the save dialog offers as the name.
///
/// The name the sound already has, which is what somebody tweaking a sound and
/// saving it expects -- unless nothing has been loaded, when the strip's dash
/// is not a name and offering it only leaves something to delete first.
fn offered_name(current: &str) -> String {
    if current == presets::NONE {
        String::new()
    } else {
        current.to_string()
    }
}

// ---------------------------------------------------------------------------
// The strip
// ---------------------------------------------------------------------------

/// The name in the strip, with a caret and a dot when it has been edited.
pub struct PresetButton;

impl PresetButton {
    pub fn build_into(cx: &mut Context) -> Handle<'_, Self> {
        let session = cx.data::<Session>().view;
        Self.build(cx, |cx| {
            // A dot rather than an asterisk: it reads as a state, not as a
            // footnote pointing at something.
            Label::new(
                cx,
                session.map(|s: &SessionView| {
                    if s.dirty {
                        format!("{}  \u{2022}", s.current)
                    } else {
                        s.current.clone()
                    }
                }),
            )
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .padding_left(Pixels(10.0))
            .alignment(Alignment::Left)
            .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
            .font_size(11.0)
            .color(Color::rgb(0xff, 0xb2, 0x6a))
            .hoverable(false);
        })
    }
}

impl View for PresetButton {
    fn element(&self) -> Option<&'static str> {
        Some("preset-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window: &WindowEvent, meta| match window {
            WindowEvent::MouseDown(MouseButton::Left) => {
                cx.emit(SessionEvent::Toggle);
                meta.consume();
            }
            WindowEvent::MouseScroll(_, y) => {
                cx.emit(SessionEvent::Step(-y.signum() as i32));
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();
        frame(canvas, b, scale, false);

        let (x, y) = (b.x + b.width() - 14.0 * scale, b.y + b.height() / 2.0);
        let mut caret = vg::Path::new();
        caret.move_to(x - 4.0 * scale, y - 2.0 * scale);
        caret.line_to(x, y + 2.5 * scale);
        caret.line_to(x + 4.0 * scale, y - 2.0 * scale);
        canvas.stroke_path(
            &caret,
            &vg::Paint::color(rgba(0xc9d2d8, 0.7)).with_line_width(1.4 * scale),
        );
    }
}

/// A small labelled button that emits one event.
pub struct Press {
    make: Box<dyn Fn() -> SessionEvent>,
    /// A greyed button still draws, so the strip does not change shape when
    /// there is nothing to delete.
    enabled: bool,
    strong: bool,
}

impl Press {
    pub fn build_into<'a>(
        cx: &'a mut Context,
        text: &'static str,
        enabled: bool,
        strong: bool,
        make: impl Fn() -> SessionEvent + 'static,
    ) -> Handle<'a, Self> {
        Self {
            make: Box::new(make),
            enabled,
            strong,
        }
        .build(cx, move |cx| {
            Label::new(cx, text)
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .alignment(Alignment::Center)
                .text_align(TextAlign::Center)
                .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
                .font_size(10.5)
                .color(if !enabled {
                    Color::rgba(0xff, 0xff, 0xff, 0x33)
                } else if strong {
                    Color::rgb(0xff, 0xb2, 0x6a)
                } else {
                    Color::rgb(0xc9, 0xd2, 0xd8)
                })
                .hoverable(false);
        })
    }
}

impl View for Press {
    fn element(&self) -> Option<&'static str> {
        Some("press")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        if !self.enabled {
            return;
        }
        event.map(|window: &WindowEvent, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window {
                cx.emit((self.make)());
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        frame(
            canvas,
            cx.bounds(),
            cx.scale_factor(),
            self.strong && self.enabled,
        );
    }
}

/// The box a button or field is drawn in.
fn frame(canvas: &Canvas, b: BoundingBox, scale: f32, lit: bool) {
    let mut path = vg::Path::new();
    path.rounded_rect(b.x, b.y, b.width(), b.height(), 3.0 * scale);
    canvas.fill_path(
        &path,
        &vg::Paint::color(if lit {
            rgba(GLOW, 0.18)
        } else {
            rgba(0x000000, 0.30)
        }),
    );
    canvas.stroke_path(
        &path,
        &vg::Paint::color(rgba(0xffffff, if lit { 0.20 } else { 0.12 })).with_line_width(scale),
    );
}

/// The size button: shows the scale it is at, and opens the list.
pub struct SizeButton;

impl SizeButton {
    pub fn build_into(cx: &mut Context) -> Handle<'_, Self> {
        let session = cx.data::<Session>().view;
        Self.build(cx, |cx| {
            Label::new(
                cx,
                session
                    .map(|s| s.scale)
                    .map(|s| format!("{:.0}%", s * 100.0)),
            )
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .alignment(Alignment::Center)
            .text_align(TextAlign::Center)
            .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
            .font_size(10.0)
            .color(Color::rgb(0xc9, 0xd2, 0xd8))
            .hoverable(false);
        })
    }
}

impl View for SizeButton {
    fn element(&self) -> Option<&'static str> {
        Some("size-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window: &WindowEvent, meta| match window {
            WindowEvent::MouseDown(MouseButton::Left) => {
                cx.emit(SessionEvent::ToggleSize);
                meta.consume();
            }
            // The wheel steps through the sizes, which is the quickest way to
            // find the one that suits the screen.
            WindowEvent::MouseScroll(_, y) => {
                let at = SCALES
                    .iter()
                    .position(|s| (*s - cx.data::<Session>().scale).abs() < 1e-6)
                    .unwrap_or(UNITY) as i64;
                let next = (at + y.signum() as i64).clamp(0, SCALES.len() as i64 - 1);
                cx.emit(SessionEvent::SetScale(SCALES[next as usize]));
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        frame(canvas, cx.bounds(), cx.scale_factor(), false);
    }
}

/// The sizes, as a short list under the button.
pub fn sizes(cx: &mut Context) {
    let session = cx.data::<Session>().view;
    {
        let sizing = session.map(|s| s.sizing);
        Binding::new(cx, sizing, move |cx| {
            if !sizing.get() {
                return;
            }
            Backdrop::new(cx, SessionEvent::Close);

            VStack::new(cx, |cx| {
                for scale in SCALES {
                    SizeRow::build_into(cx, scale);
                }
            })
            .position_type(PositionType::Absolute)
            .left(Pixels(SIZE_X - 8.0))
            .top(Pixels(HEADER_H - 2.0))
            .width(Pixels(72.0))
            .height(Pixels(SIZES_H))
            .padding_top(Pixels(4.0))
            .background_color(Color::rgb(0x1c, 0x20, 0x23))
            .border_color(Color::rgba(0xff, 0xff, 0xff, 0x22))
            .border_width(Pixels(1.0));
        });
    };
}

pub struct SizeRow {
    scale: f64,
}

impl SizeRow {
    pub fn build_into(cx: &mut Context, scale: f64) -> Handle<'_, Self> {
        let session = cx.data::<Session>().view;
        Self { scale }
            .build(cx, move |cx| {
                Label::new(cx, format!("{:.0}%", scale * 100.0))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .padding_left(Pixels(14.0))
                    .alignment(Alignment::Left)
                    .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
                    .font_size(10.5)
                    .color(session.map(|s| s.scale).map(move |current| {
                        if (*current - scale).abs() < 1e-6 {
                            Color::rgb(0xff, 0xb2, 0x6a)
                        } else {
                            Color::rgb(0xc9, 0xd2, 0xd8)
                        }
                    }))
                    .hoverable(false);
            })
            .width(Stretch(1.0))
            .height(Pixels(SIZE_ROW_H))
    }
}

impl View for SizeRow {
    fn element(&self) -> Option<&'static str> {
        Some("size-row")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        let scale = self.scale;
        event.map(|window: &WindowEvent, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window {
                cx.emit(SessionEvent::SetScale(scale));
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        let b = cx.bounds();
        let mut row = vg::Path::new();
        row.rect(b.x, b.y, b.width(), b.height());
        canvas.fill_path(&row, &vg::Paint::color(rgba(0xffffff, 0.02)));
    }
}

// ---------------------------------------------------------------------------
// The list
// ---------------------------------------------------------------------------

/// What the scroll bar looks like.
///
/// vizia builds one for a vertical `ScrollView` on its own, but the plugin
/// runs with no theme at all -- deliberately, so that nothing arrives looking
/// like a default toolkit -- and with no theme the bar has no width, no
/// colour and no thumb. It is there, and it is invisible, so the list scrolls
/// with no sign that it can.
/// The text caret is the same story, and the reason it is here rather than
/// set on the `Textbox` itself.
///
/// An unset colour property in vizia reads back as `rgba(0, 0, 0, 0)`, so with
/// no theme the caret was drawn on every frame in transparent black -- and the
/// selection highlight with it. That is exactly what was reported: clicking
/// into the middle of a name and pressing delete removed the right character,
/// because the box was editing and the caret was tracking; you simply could
/// not see it.
///
/// It goes in the sheet rather than on the view because vizia blinks the caret
/// by toggling a `caret` class on and off, and a rule is what that class has
/// to act on. Setting the colour inline would have made it visible and left it
/// staring, never blinking.
pub const SCROLLBAR: &str = r#"
scrollbar {
    width: 8px;
    background-color: #00000055;
}
scrollbar .thumb {
    width: 8px;
    background-color: #ffffff40;
    corner-radius: 4px;
}
textbox {
    caret-color: transparent;
    selection-color: #ff8a3c55;
}
textbox:checked.caret {
    caret-color: #e8eef4;
}
"#;

pub fn menu(cx: &mut Context) {
    let session = cx.data::<Session>().view;
    {
        let open = session.map(|s| s.open);
        Binding::new(cx, open, move |cx| {
            if !open.get() {
                return;
            }
            Backdrop::new(cx, SessionEvent::Close);
            let height = MENU_H.min(super::current_height(cx) - HEADER_H - 6.0);

            VStack::new(cx, |cx| {
                ScrollView::new(cx, move |cx| {
                    {
                        let entries = session.map(|s| s.entries.clone());
                        Binding::new(cx, entries, move |cx| {
                            let entries = entries.get();
                            VStack::new(cx, move |cx| {
                                for group in GROUPS.iter().copied().chain([SAVED]) {
                                    let rows: Vec<(usize, String)> = entries
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, p)| p.group == group)
                                        .map(|(i, p)| (i, p.name.clone()))
                                        .collect();
                                    // A heading with nothing under it is a promise the
                                    // list does not keep, so "Saved" only appears once
                                    // something has been saved.
                                    if rows.is_empty() {
                                        continue;
                                    }
                                    heading(cx, group);
                                    for (index, name) in rows {
                                        Row::build_into(cx, index, name);
                                    }
                                }
                            })
                            .width(Stretch(1.0))
                            .height(Auto);
                        });
                    };
                })
                // Down only. The rows are as wide as the list, so there is
                // nothing to scroll across -- and the bar vizia draws for it
                // anyway sat over the last preset.
                .show_horizontal_scrollbar(false)
                .width(Stretch(1.0))
                .height(Stretch(1.0));
            })
            // Hung under the button it belongs to, not merely near it.
            .position_type(PositionType::Absolute)
            .left(Pixels(BUTTON_X))
            .top(Pixels(HEADER_H - 2.0))
            .width(Pixels(MENU_W))
            .height(Pixels(height))
            .background_color(Color::rgb(0x1c, 0x20, 0x23))
            .border_color(Color::rgba(0xff, 0xff, 0xff, 0x22))
            .border_width(Pixels(1.0));
        });
    };
}

fn heading(cx: &mut Context, text: &'static str) {
    Label::new(cx, text)
        .width(Stretch(1.0))
        .height(Pixels(HEADING_H))
        .padding_left(Pixels(10.0))
        .alignment(Alignment::Left)
        .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
        .font_size(9.5)
        .color(Color::rgb(0x7e, 0x8a, 0x96))
        .hoverable(false);
}

/// One name in the list.
pub struct Row {
    index: usize,
}

impl Row {
    pub fn build_into(cx: &mut Context, index: usize, name: String) -> Handle<'_, Self> {
        let session = cx.data::<Session>().view;
        let shown = name.clone();
        Self { index }
            .build(cx, move |cx| {
                Label::new(cx, shown.clone())
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .padding_left(Pixels(20.0))
                    .alignment(Alignment::Left)
                    .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
                    .font_size(11.0)
                    .color(session.map(|s| s.current.clone()).map(move |current| {
                        if *current == name {
                            Color::rgb(0xff, 0xb2, 0x6a)
                        } else {
                            Color::rgb(0xc9, 0xd2, 0xd8)
                        }
                    }))
                    .hoverable(false);
            })
            .width(Stretch(1.0))
            .height(Pixels(ROW_H))
    }
}

impl View for Row {
    fn element(&self) -> Option<&'static str> {
        Some("preset-row")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        let index = self.index;
        event.map(|window: &WindowEvent, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window {
                cx.emit(SessionEvent::Load(index));
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        let b = cx.bounds();
        let mut row = vg::Path::new();
        row.rect(b.x, b.y, b.width(), b.height());
        canvas.fill_path(&row, &vg::Paint::color(rgba(0xffffff, 0.02)));
    }
}

// ---------------------------------------------------------------------------
// The dialogs
// ---------------------------------------------------------------------------

const DIALOG_W: f32 = 320.0;
const DIALOG_H: f32 = 150.0;

pub fn dialogs(cx: &mut Context) {
    let session = cx.data::<Session>().view;
    {
        let which = session.map(|s| s.dialog);
        Binding::new(cx, which, move |cx| {
            let which = which.get();
            if which == Dialog::None {
                return;
            }
            // A question has to sit on top of whatever asked it, and clicking away
            // from it means no.
            Backdrop::new(cx, SessionEvent::Cancel);

            let left = (PANEL_W - DIALOG_W) / 2.0;
            let top = ((super::current_height(cx) - DIALOG_H) / 2.0).max(HEADER_H);

            VStack::new(cx, move |cx| {
                let title = match which {
                    Dialog::Save => "Save preset",
                    Dialog::Overwrite => "Replace it?",
                    Dialog::Delete => "Delete preset",
                    Dialog::None => "",
                };
                Label::new(cx, title)
                    .width(Stretch(1.0))
                    .height(Pixels(28.0))
                    .padding_left(Pixels(16.0))
                    .alignment(Alignment::Left)
                    .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
                    .font_size(11.5)
                    .color(Color::rgb(0xe8, 0xee, 0xf4))
                    .hoverable(false);

                match which {
                    Dialog::Save => {
                        Textbox::new(cx, session.map(|s| s.draft.clone()))
                            .position_type(PositionType::Absolute)
                            .left(Pixels(16.0))
                            .top(Pixels(36.0))
                            .width(Pixels(DIALOG_W - 32.0))
                            .height(Pixels(26.0))
                            .padding_left(Pixels(5.0))
                            .alignment(Alignment::Left)
                            .font_family(vec![FamilyOwned::Named(String::from(
                                vizia_assets::ROBOTO,
                            ))])
                            .font_size(11.0)
                            .color(Color::rgb(0xe8, 0xee, 0xf4))
                            .background_color(Color::rgba(0x00, 0x00, 0x00, 0x55))
                            .border_color(Color::rgba(0xff, 0xff, 0xff, 0x22))
                            .border_width(Pixels(1.0))
                            // Both, because a name typed and then clicked away
                            // from has still been typed. Committing only on Enter
                            // is how a save quietly writes the previous name.
                            .on_edit(|cx, text| cx.emit(SessionEvent::Draft(text)))
                            .on_submit(|cx, text, _| {
                                cx.emit(SessionEvent::Draft(text));
                                cx.emit(SessionEvent::Confirm);
                            })
                            // Focus it the moment the dialog opens.
                            //
                            // Without this the box is built unfocused, and a vizia
                            // `Textbox` only draws its caret and only accepts keys
                            // once it is in edit mode -- which it enters on
                            // `FocusIn`. So the dialog came up with no cursor in
                            // it. `save` rejects an empty name *before* it creates
                            // the directory, so the report that reached us was two
                            // things at once: no cursor, and no
                            // `GainStageFx\Presets` folder ever appearing.
                            //
                            // This is vizia's focus, inside the panel. Whether the
                            // keys reach the panel at all is the operating system's
                            // focus, and on Windows that is the job of the
                            // `set_text_input` call in `Session::event`.
                            .on_build(|cx| cx.focus());
                    }
                    Dialog::Overwrite => {
                        note(
                            cx,
                            session
                                .map(|s| s.draft.clone())
                                .map(|n| format!("You already have a preset called {n}.")),
                        );
                    }
                    Dialog::Delete => {
                        note(
                            cx,
                            session
                                .map(|s| s.current.clone())
                                .map(|n| format!("Delete {n}? This cannot be undone.")),
                        );
                    }
                    Dialog::None => {}
                }

                {
                    let error = session.map(|s| s.error.clone());
                    Binding::new(cx, error, move |cx| {
                        let error = error.get();
                        if error.is_empty() {
                            return;
                        }
                        Label::new(cx, error.clone())
                            .width(Stretch(1.0))
                            .height(Pixels(18.0))
                            .position_type(PositionType::Absolute)
                            .top(Pixels(70.0))
                            .left(Pixels(16.0))
                            .alignment(Alignment::Left)
                            .font_family(vec![FamilyOwned::Named(String::from(
                                vizia_assets::ROBOTO,
                            ))])
                            .font_size(10.0)
                            .color(Color::rgb(0xe8, 0x7a, 0x5a))
                            .hoverable(false);
                    });
                };

                HStack::new(cx, move |cx| {
                    Press::build_into(cx, "Cancel", true, false, || SessionEvent::Cancel)
                        .width(Pixels(84.0))
                        .height(Pixels(24.0));
                    let go = match which {
                        Dialog::Delete => "Delete",
                        Dialog::Overwrite => "Replace",
                        _ => "Save",
                    };
                    Press::build_into(cx, go, true, true, || SessionEvent::Confirm)
                        .width(Pixels(84.0))
                        .height(Pixels(24.0));
                })
                .position_type(PositionType::Absolute)
                .left(Pixels(DIALOG_W - 16.0 - 178.0))
                .top(Pixels(DIALOG_H - 14.0 - 24.0))
                .width(Pixels(178.0))
                .height(Pixels(24.0))
                .horizontal_gap(Pixels(10.0));
            })
            .position_type(PositionType::Absolute)
            .left(Pixels(left))
            .top(Pixels(top))
            .width(Pixels(DIALOG_W))
            .height(Pixels(DIALOG_H))
            .background_color(Color::rgb(0x22, 0x27, 0x2a))
            .border_color(Color::rgba(0xff, 0xff, 0xff, 0x2a))
            .border_width(Pixels(1.0));
        });
    };
}

fn note(cx: &mut Context, text: impl Res<String> + Clone + 'static) {
    Label::new(cx, text)
        .width(Stretch(1.0))
        .height(Pixels(30.0))
        .left(Pixels(16.0))
        .right(Pixels(16.0))
        .alignment(Alignment::Left)
        .font_family(vec![FamilyOwned::Named(String::from(vizia_assets::ROBOTO))])
        .font_size(10.5)
        .color(Color::rgb(0xa8, 0xb2, 0xba))
        .hoverable(false);
}

/// Catches a click anywhere outside whatever is on top.
pub struct Backdrop {
    on_click: Box<dyn Fn() -> SessionEvent>,
}

impl Backdrop {
    pub fn new(cx: &mut Context, close: SessionEvent) -> Handle<'_, Self> {
        let height = super::current_height(cx);
        // Captured as a maker rather than a value, because the view outlives
        // the one event it was built with.
        let which = match close {
            SessionEvent::Cancel => 1,
            _ => 0,
        };
        Self {
            on_click: Box::new(move || {
                if which == 1 {
                    SessionEvent::Cancel
                } else {
                    SessionEvent::Close
                }
            }),
        }
        .build(cx, |_| {})
        .position_type(PositionType::Absolute)
        .left(Pixels(0.0))
        .top(Pixels(0.0))
        .width(Pixels(PANEL_W))
        .height(Pixels(height))
    }
}

impl View for Backdrop {
    fn element(&self) -> Option<&'static str> {
        Some("backdrop")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window: &WindowEvent, meta| {
            if let WindowEvent::MouseDown(_) = window {
                cx.emit((self.on_click)());
                meta.consume();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{offered_name, SCALES, SCROLLBAR, SIZES_H, UNITY};
    use crate::editor::style::{window_height, HEADER_H};
    use crate::presets;

    /// The size list hangs from the header and has to fit the window at its
    /// shortest -- every section closed -- or the largest sizes are cut off
    /// below its edge, where they cannot be clicked. And the wheel's fallback
    /// is 100%, wherever the list puts it.
    #[test]
    fn every_size_fits_the_smallest_window() {
        assert!(HEADER_H - 2.0 + SIZES_H <= window_height(0));
        assert_eq!(SCALES[UNITY], 1.0);
        assert!(SCALES.contains(&0.5) && SCALES.contains(&0.65));
        assert!(SCALES.windows(2).all(|w| w[0] < w[1]));
    }

    /// Reported as "a little * or - in the input": saving from scratch offered
    /// the strip's dash as the name.
    #[test]
    fn saving_from_scratch_offers_no_name() {
        assert_eq!(offered_name(presets::NONE), "");
        assert_eq!(offered_name("Console Channel"), "Console Channel");
    }

    /// The stylesheet has to be well formed, because nothing will say so if it
    /// is not.
    ///
    /// `Context::add_stylesheet` returns `Ok(())` whatever happens, and the
    /// parse behind it is `if let Ok(stylesheet) = StyleSheet::parse(..)` --
    /// so one missing semicolon does not break one rule, it silently discards
    /// **the whole sheet**. Both things in it are invisible when they are
    /// missing rather than wrong: a scrollbar with no width, and a caret drawn
    /// in transparent black. Nobody would see which had gone.
    ///
    /// vizia imports its own parser privately, so this cannot call it. What it
    /// can do is the structure, which is what a typo actually breaks.
    #[test]
    fn the_stylesheet_is_well_formed() {
        let mut depth = 0i32;
        for (line, text) in SCROLLBAR.lines().enumerate() {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            if text.ends_with('{') {
                depth += 1;
                continue;
            }
            if text == "}" {
                depth -= 1;
                assert!(
                    depth >= 0,
                    "line {} closes a rule that never opened",
                    line + 1
                );
                continue;
            }
            assert!(
                depth > 0,
                "line {} is a declaration outside any rule",
                line + 1
            );
            assert!(
                text.ends_with(';'),
                "line {} has no semicolon, which discards the whole sheet: {text}",
                line + 1
            );
            assert!(
                text.matches(':').count() >= 1,
                "line {} is not a declaration: {text}",
                line + 1
            );
        }
        assert_eq!(depth, 0, "a rule is left open");
    }

    /// The caret is the one rule whose absence looks exactly like the bug it
    /// was added for, so it is named here rather than only described.
    ///
    /// An unset colour property in vizia reads back as `rgba(0, 0, 0, 0)`, and
    /// this editor runs with `ViziaTheming::None` -- so with no rule the caret
    /// is drawn every frame in transparent black. Typing works, clicking
    /// works, delete lands where you clicked, and there is no cursor.
    #[test]
    fn the_caret_has_a_colour_to_be_drawn_in() {
        assert!(
            SCROLLBAR.contains("textbox:checked.caret"),
            "vizia blinks the caret by toggling a `caret` class, so the colour \
             has to hang off that selector or it will not blink"
        );
        let visible = SCROLLBAR
            .lines()
            .filter(|line| line.trim().starts_with("caret-color:"))
            .any(|line| !line.contains("transparent"));
        assert!(
            visible,
            "every caret-color in the sheet is transparent, which is the bug"
        );
    }
}
