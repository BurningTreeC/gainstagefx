//! Dropdowns: a button that shows the current choice, and one list at a time
//! hung over the panel from whichever button opened it.
//!
//! The lists are drawn last, as the preset menu is, rather than as vizia's own
//! popup. A popup belongs to the layout of the view that owns it, so it draws
//! under every section built after that one -- a cabinet list would open
//! beneath the output section it overlaps.
//!
//! Choosing writes the parameter as an ordinary gesture, so it is automatable
//! and undoable like a knob. The button still steps with the wheel, which is
//! the quickest way to hear what is in a list.

use nih_plug::prelude::{Param, ParamPtr};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::vizia_assets;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::RawParamEvent;
use std::sync::Arc;

use super::style::*;
use super::Panel;
use crate::params::{
    Amplifier, CabModel, Circuit, Diode, GainStageParams, Iron, Mains, MicModel, PedalModel,
    PowerAmp, SpeakerModel,
};

const ROW_H: f32 = 21.0;
/// Taller than any list today; a longer one scrolls rather than leaving the
/// window.
const MENU_MAX_H: f32 = 360.0;
const MENU_MIN_W: f32 = 170.0;
const MENU_PAD: f32 = 3.0;

/// Every list on the panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    Pedal,
    /// The first part of the circuit list: the generic topologies.
    Topology,
    /// The rest of it: circuits modelled from their schematics.
    Modelled,
    PowerAmp,
    Mains,
    Clipping,
    Amplifier,
    Iron,
    Cabinet,
    Speaker,
    MicA,
    MicB,
}

impl Data for Choice {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// The part of a parameter's list one dropdown offers.
struct Window {
    names: Vec<&'static str>,
    /// Where `names` starts in the parameter's own list, and how long that
    /// list is. Normalised values are made against the whole list.
    offset: usize,
    total: usize,
    /// The first absolute entry that may be chosen (mic A has no Off).
    first: usize,
}

impl Window {
    fn range(&self) -> std::ops::Range<usize> {
        self.offset.max(self.first)..self.offset + self.names.len()
    }

    fn index_of(&self, normalized: f32) -> usize {
        let last = self.total.max(1) - 1;
        ((normalized * last as f32).round() as usize).min(last)
    }

    fn normalized(&self, index: usize) -> f32 {
        if self.total > 1 {
            index as f32 / (self.total - 1) as f32
        } else {
            0.0
        }
    }

    fn name(&self, index: usize) -> Option<&'static str> {
        self.range().contains(&index).then(|| self.names[index - self.offset])
    }
}

fn topologies() -> usize {
    Circuit::ALL.iter().filter(|c| !c.is_modelled()).count()
}

impl Choice {
    fn window(self) -> Window {
        fn all<T: Copy>(list: &[T], name: fn(T) -> &'static str) -> Vec<&'static str> {
            list.iter().map(|&x| name(x)).collect()
        }
        let whole = |names: Vec<&'static str>, first: usize| Window {
            total: names.len(),
            names,
            offset: 0,
            first,
        };
        match self {
            Choice::Pedal => whole(all(&PedalModel::ALL, PedalModel::name), 0),
            Choice::Topology | Choice::Modelled => {
                let names = all(&Circuit::ALL, Circuit::name);
                let split = topologies();
                let total = names.len();
                if self == Choice::Topology {
                    Window { names: names[..split].to_vec(), offset: 0, total, first: 0 }
                } else {
                    Window { names: names[split..].to_vec(), offset: split, total, first: 0 }
                }
            }
            Choice::PowerAmp => whole(all(&PowerAmp::ALL, PowerAmp::name), 0),
            Choice::Mains => whole(all(&Mains::ALL, Mains::name), 0),
            Choice::Clipping => whole(all(&Diode::ALL, Diode::name), 0),
            Choice::Amplifier => whole(all(&Amplifier::ALL, Amplifier::name), 0),
            Choice::Iron => whole(all(&Iron::ALL, Iron::name), 0),
            Choice::Cabinet => whole(all(&CabModel::ALL, CabModel::name), 0),
            Choice::Speaker => whole(all(&SpeakerModel::ALL, SpeakerModel::name), 0),
            // Mic A always exists; its "Bypass" is an ideal omni.
            Choice::MicA => whole(all(&MicModel::ALL, MicModel::name), 1),
            Choice::MicB => whole(all(&MicModel::ALL, MicModel::name), 0),
        }
    }

    /// The parameter behind the list, and where it is set now.
    fn read(self, p: &GainStageParams) -> (ParamPtr, f32) {
        fn of<P: Param>(param: &P) -> (ParamPtr, f32) {
            (param.as_ptr(), param.unmodulated_normalized_value())
        }
        match self {
            Choice::Pedal => of(&p.pedal),
            Choice::Topology | Choice::Modelled => of(&p.circuit),
            Choice::PowerAmp => of(&p.power_amp),
            Choice::Mains => of(&p.mains),
            Choice::Clipping => of(&p.diode),
            Choice::Amplifier => of(&p.amplifier),
            Choice::Iron => of(&p.iron),
            Choice::Cabinet => of(&p.cab_model),
            Choice::Speaker => of(&p.speaker),
            Choice::MicA => of(&p.mic_a),
            Choice::MicB => of(&p.mic_b),
        }
    }

    fn current(self, p: &GainStageParams) -> usize {
        self.window().index_of(self.read(p).1)
    }
}

/// Which list is open, and the button it hangs from, in panel coordinates.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Opened {
    choice: Choice,
    x: f32,
    top: f32,
    bottom: f32,
    width: f32,
}

impl Data for Opened {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Lens)]
pub struct Dropdowns {
    open: Option<Opened>,
    params: Arc<GainStageParams>,
}

pub enum DropEvent {
    Open(Opened),
    Close,
    /// An absolute index into the open list's parameter.
    Pick(usize),
}

impl Dropdowns {
    pub fn build_into(cx: &mut Context, params: Arc<GainStageParams>) {
        Self { open: None, params }.build(cx);
    }
}

impl Model for Dropdowns {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|e: &DropEvent, meta| {
            match *e {
                DropEvent::Open(opened) => self.open = Some(opened),
                DropEvent::Close => self.open = None,
                DropEvent::Pick(index) => {
                    if let Some(opened) = self.open.take() {
                        let window = opened.choice.window();
                        let (ptr, _) = opened.choice.read(&self.params);
                        let normalized = window.normalized(index);
                        cx.emit(RawParamEvent::BeginSetParameter(ptr));
                        cx.emit(RawParamEvent::SetParameterNormalized(ptr, normalized));
                        cx.emit(RawParamEvent::EndSetParameter(ptr));
                    }
                }
            }
            meta.consume();
        });
    }
}

// ---------------------------------------------------------------------------
// The button
// ---------------------------------------------------------------------------

pub struct DropButton {
    param: ParamWidgetBase,
    choice: Choice,
    enabled: bool,
}

impl DropButton {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        choice: Choice,
        enabled: bool,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param: ParamWidgetBase::new(cx, params, params_to_param),
            choice,
            enabled,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, data| {
                let value = data.make_lens(|param| param.modulated_normalized_value());
                let window = choice.window();
                let shown = value.map(move |v| window.name(window.index_of(*v)));
                // A circuit is a topology or a model, never both, so one of
                // the two circuit buttons always has nothing to show.
                Label::new(cx, shown.map(|name| name.unwrap_or("--").to_string()))
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_left(Pixels(9.0))
                    .child_right(Pixels(22.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .font_family(vec![FamilyOwned::Name(String::from(vizia_assets::ROBOTO))])
                    .font_size(11.0)
                    .color(shown.map(move |name| match (enabled, name.is_some()) {
                        (true, true) => Color::rgb(0xff, 0xb2, 0x6a),
                        (true, false) => Color::rgb(0x7e, 0x8a, 0x96),
                        (false, _) => Color::rgba(0xff, 0xff, 0xff, 0x33),
                    }))
                    .hoverable(false);
                Binding::new(cx, value, |cx, _| cx.needs_redraw());
            }),
        )
    }

    fn step(&self, cx: &mut EventContext, delta: i64) {
        let window = self.choice.window();
        let range = window.range();
        let current = window.index_of(self.param.unmodulated_normalized_value());
        let next = if range.contains(&current) {
            (current as i64 + delta).clamp(range.start as i64, range.end as i64 - 1) as usize
        } else {
            range.start
        };
        if next == current {
            return;
        }
        self.param.begin_set_parameter(cx);
        self.param.set_normalized_value(cx, window.normalized(next));
        self.param.end_set_parameter(cx);
    }
}

impl View for DropButton {
    fn element(&self) -> Option<&'static str> {
        Some("drop-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        if !self.enabled {
            return;
        }
        event.map(|window: &WindowEvent, meta| match window {
            WindowEvent::MouseDown(MouseButton::Left) => {
                // Bounds are in device pixels; the overlay is placed in the
                // panel's own coordinates.
                let b = cx.bounds();
                let s = cx.scale_factor();
                cx.emit(DropEvent::Open(Opened {
                    choice: self.choice,
                    x: b.x / s,
                    top: b.y / s,
                    bottom: (b.y + b.h) / s,
                    width: b.w / s,
                }));
                meta.consume();
            }
            WindowEvent::MouseScroll(_, y) if *y != 0.0 => {
                self.step(cx, -(y.signum() as i64));
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();
        let mut cell = vg::Path::new();
        cell.rounded_rect(b.x + 0.5 * scale, b.y, b.w - scale, b.h, 3.0 * scale);
        canvas.fill_path(
            &cell,
            &vg::Paint::color(rgba(0x000000, if self.enabled { 0.30 } else { 0.12 })),
        );
        canvas.stroke_path(
            &cell,
            &vg::Paint::color(rgba(0xffffff, if self.enabled { 0.14 } else { 0.05 }))
                .with_line_width(scale),
        );

        let (x, y) = (b.x + b.w - 12.0 * scale, b.y + b.h / 2.0);
        let mut caret = vg::Path::new();
        caret.move_to(x - 4.0 * scale, y - 2.0 * scale);
        caret.line_to(x, y + 2.5 * scale);
        caret.line_to(x + 4.0 * scale, y - 2.0 * scale);
        canvas.stroke_path(
            &caret,
            &vg::Paint::color(rgba(0xc9d2d8, if self.enabled { 0.7 } else { 0.2 }))
                .with_line_width(1.4 * scale),
        );
    }
}

// ---------------------------------------------------------------------------
// The list
// ---------------------------------------------------------------------------

/// Draws the open list, if there is one. Built after the panel sections so it
/// sits on top of them and takes the clicks first.
pub fn menu(cx: &mut Context) {
    Binding::new(cx, Dropdowns::open, |cx, open| {
        let Some(opened) = open.get(cx) else {
            return;
        };
        Catch::build_into(cx);

        let window = opened.choice.window();
        let rows: Vec<(usize, &'static str)> =
            window.range().map(|i| (i, window.names[i - window.offset])).collect();
        // Padding above and below, and the border.
        let natural = rows.len() as f32 * ROW_H + 2.0 * MENU_PAD + 2.0;
        let scrolls = natural > MENU_MAX_H;
        let height = natural.min(MENU_MAX_H);
        let width = opened.width.max(MENU_MIN_W);
        let left = opened.x.min(PANEL_W - width - 4.0).max(4.0);
        // Below the button when it fits, above it when it does not, and
        // pinned to the foot of the window if neither does.
        let top = if opened.bottom + 1.0 + height <= WINDOW_H - 4.0 {
            opened.bottom + 1.0
        } else if opened.top - 1.0 - height >= HEADER_H {
            opened.top - 1.0 - height
        } else {
            (WINDOW_H - 4.0 - height).max(HEADER_H)
        };
        let choice = opened.choice;

        let list = move |cx: &mut Context| {
            VStack::new(cx, move |cx| {
                for (index, name) in rows {
                    Row::build_into(cx, choice, index, name);
                }
            })
            .width(Stretch(1.0))
            .height(Auto)
            .child_top(Pixels(MENU_PAD))
            .child_bottom(Pixels(MENU_PAD));
        };
        VStack::new(cx, move |cx| {
            // The scroll bar's track draws whether or not there is anything
            // to scroll, so a list that fits does without one.
            if scrolls {
                ScrollView::new(cx, 0.0, 0.0, false, true, list)
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));
            } else {
                list(cx);
            }
        })
        .position_type(PositionType::SelfDirected)
        .left(Pixels(left))
        .top(Pixels(top))
        .width(Pixels(width))
        .height(Pixels(height))
        .background_color(Color::rgb(0x1c, 0x20, 0x23))
        .border_color(Color::rgba(0xff, 0xff, 0xff, 0x22))
        .border_width(Pixels(1.0));
    });
}

/// One entry in the open list.
struct Row {
    index: usize,
    hovered: bool,
}

impl Row {
    fn build_into(cx: &mut Context, choice: Choice, index: usize, name: &'static str) {
        Self { index, hovered: false }
            .build(cx, move |cx| {
                Label::new(cx, name)
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_left(Pixels(10.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .font_family(vec![FamilyOwned::Name(String::from(vizia_assets::ROBOTO))])
                    .font_size(11.0)
                    .color(Panel::params.map(move |p| {
                        if choice.current(p) == index {
                            Color::rgb(0xff, 0xb2, 0x6a)
                        } else {
                            Color::rgb(0xc9, 0xd2, 0xd8)
                        }
                    }))
                    .hoverable(false);
            })
            .width(Stretch(1.0))
            .height(Pixels(ROW_H));
    }
}

impl View for Row {
    fn element(&self) -> Option<&'static str> {
        Some("drop-row")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window: &WindowEvent, meta| match window {
            WindowEvent::MouseDown(MouseButton::Left) => {
                cx.emit(DropEvent::Pick(self.index));
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

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        if !self.hovered {
            return;
        }
        let b = cx.bounds();
        let mut row = vg::Path::new();
        row.rect(b.x, b.y, b.w, b.h);
        canvas.fill_path(&row, &vg::Paint::color(rgba(GLOW, 0.14)));
    }
}

/// Catches a click anywhere outside the open list and closes it.
struct Catch;

impl Catch {
    fn build_into(cx: &mut Context) {
        Self.build(cx, |_| {})
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(0.0))
            .width(Pixels(PANEL_W))
            .height(Pixels(WINDOW_H));
    }
}

impl View for Catch {
    fn element(&self) -> Option<&'static str> {
        Some("drop-backdrop")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window: &WindowEvent, meta| {
            if let WindowEvent::MouseDown(_) = window {
                cx.emit(DropEvent::Close);
                meta.consume();
            }
        });
    }
}
