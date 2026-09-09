//! The GainStageFx front panel.
//!
//! The panel is the signal path, drawn top to bottom, and that is the only
//! organising idea in it. Six numbered bands: what arrives, what does the
//! work, how hard it is worked, what is taken out afterwards, what it comes
//! out of, what leaves. An arrow at the foot of each band points into the
//! next.
//!
//! This is deliberate and it is the one thing the previous version could not
//! be given afterwards. Its controls sat where they had been added, so the
//! panel recorded the order the plugin was built in rather than the order the
//! signal travels, and nothing on it said what happened before what. A layout
//! is a claim about how a thing works; that one made no claim at all.

mod panel;
pub mod session;
mod sprites;
mod style;
mod widgets;

use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::{
    Amplifier, Cabinet, Circuit, Diode, GainStageParams, Iron, Oversampling, ToneStack,
};
use panel::Faceplate;
use style::*;
use widgets::{Knob, Meter, Selector};

#[derive(Lens)]
pub struct Panel {
    pub params: Arc<GainStageParams>,
    pub meters: Arc<crate::meters::Meters>,
}

impl Model for Panel {}

pub fn default_state() -> Arc<ViziaState> {
    // The panel's coordinate system is already its intended 100% size.
    // Starting at 2.0 made a new session open at 200%, while the size menu
    // and persistent state define 1.0 as the default.
    ViziaState::new_with_default_scale_factor(|| (PANEL_W as u32, WINDOW_H as u32), 1.0)
}

/// Writes the chosen size into the state the host saves and reads the window
/// size from.
///
/// `cx.set_user_scale_factor` changes what vizia *draws* at, and that is all
/// it changes. Two other things depend on the figure, and both of them read it
/// out of `ViziaState` instead: `Editor::size`, which is what the host is told
/// to make the window, and the serialised editor state, which is what comes
/// back next session.
///
/// So it is written here, directly. `PersistentField::set` is the only door
/// into that field and it takes a whole `ViziaState`, so one is made to carry
/// the number in and is dropped on the way out. Its size function is never
/// asked anything -- `set` copies the scale and nothing else.
///
/// This on its own makes the size *persist*. It does not make the window
/// change size, and on its own it actively prevents it -- see `apply_scale`.
pub fn remember_scale(state: &Arc<ViziaState>, scale: f64) {
    use nih_plug::params::persist::PersistentField;
    let carrier = ViziaState::new_with_default_scale_factor(|| (0, 0), scale);
    if let Ok(carrier) = Arc::try_unwrap(carrier) {
        PersistentField::set(state, carrier);
    }
}

/// Chooses a window size: stores it, then asks the host for it.
///
/// The second half is the one that was missing, and the reason is worth
/// writing down because the first half is what hid it.
///
/// Only one thing in the stack actually asks a host to resize a window:
/// `GuiContext::request_resize`. nih-plug calls it from one place, its
/// `WindowModel`, when a `GeometryChanged` reaches the root -- and that
/// handler opens with a guard:
///
/// ```text
/// if logical_size == old_logical_size && scale_factor == old_user_scale_factor {
///     return;
/// }
/// ```
///
/// `logical_size` is the size *before* scaling. Our size function is a
/// constant, so that half never moves and the guard rests entirely on the
/// scale. `old_user_scale_factor` is read from `ViziaState` -- the very field
/// `remember_scale` writes. So storing the scale first, which is what the
/// panel did, made both halves equal by the time the event arrived: the
/// handler returned early, `request_resize` was never called, and the host was
/// never told. The panel redrew itself at the new scale inside a window that
/// stayed the old size, on every platform, which is exactly the reported bug.
///
/// Emitting `GuiContextEvent::Resize` does not rescue it either. That handler
/// sets the window to `inner_logical_size`, the unscaled size, which has not
/// moved -- so it asks for the size the window already is.
///
/// Rather than remove the store and depend on a `GeometryChanged` firing at
/// the right moment, the request is made directly. `Editor::size` is
/// `scaled_logical_size`, so once the scale is stored the host is asking for
/// the right window; the order below is therefore load-bearing. Doing it this
/// way also means the resize does not depend on vizia emitting anything, which
/// is what makes it behave the same under every host.
pub fn apply_scale(state: &Arc<ViziaState>, gui: &dyn nih_plug::prelude::GuiContext, scale: f64) {
    remember_scale(state, scale);
    gui.request_resize();
}

/// Height of a label box, which is centred on its anchor point.
const LABEL_H: f32 = 16.0;

/// Picking one knob's parameter out of the set.
type ToKnob = fn(&Arc<GainStageParams>) -> &nih_plug::prelude::FloatParam;

pub fn create(
    params: Arc<GainStageParams>,
    meters: Arc<crate::meters::Meters>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    let state = editor_state.clone();
    create_vizia_editor(editor_state, ViziaTheming::None, move |cx, gui| {
        assets::register_noto_sans_regular(cx);
        assets::register_noto_sans_bold(cx);
        // The only styling the panel takes from a sheet rather than from its
        // own drawing: the scroll bar, which vizia builds but cannot size or
        // colour without a theme.
        let _ = cx.add_stylesheet(session::SCROLLBAR);

        Panel {
            params: params.clone(),
            meters: meters.clone(),
        }
        .build(cx);

        session::Session::build_into(cx, params.clone(), state.user_scale_factor(), gui);

        Faceplate::new(cx);
        gutter(cx);
        strip(cx);
        input(cx);
        circuit(cx);
        drive(cx);
        tone(cx);
        cabinet(cx);
        output(cx);

        // Last, so they draw over the panel and take the clicks first. The
        // dialogs come after the menu: a question has to sit on top of
        // whatever asked it.
        session::menu(cx);
        session::sizes(cx);
        session::dialogs(cx);
    })
}

// ---------------------------------------------------------------------------
// Pieces every section is built from
// ---------------------------------------------------------------------------

/// Small print, centred on a point.
fn label(cx: &mut Context, text: &str, x: f32, y: f32, size: f32, width: f32, colour: u32) {
    Label::new(cx, text)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x - width / 2.0))
        .top(Pixels(y - LABEL_H / 2.0))
        .width(Pixels(width))
        .height(Pixels(LABEL_H))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
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
    F: Fn(&Arc<GainStageParams>) -> &nih_plug::prelude::FloatParam + Copy + 'static,
    P: Fn(&Arc<GainStageParams>) -> String + Clone + 'static,
{
    Knob::new(cx, Panel::params, to_param, radius, true)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x - radius))
        .top(Pixels(y - radius));

    label(cx, name, x, y + radius + 10.0, 9.5, 100.0, 0x9aa6b0);

    Label::new(cx, Panel::params.map(move |p| read(p)))
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x - 50.0))
        .top(Pixels(y + radius + 21.0 - LABEL_H / 2.0))
        .width(Pixels(100.0))
        .height(Pixels(LABEL_H))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
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
    P: nih_plug::prelude::Param + 'static,
{
    Selector::new(cx, Panel::params, to_param, labels, enabled)
        .position_type(PositionType::SelfDirected)
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

/// The numbering down the left, which is what makes the order legible before
/// anything else on the panel is read.
fn gutter(cx: &mut Context) {
    for (index, (number, name, height)) in SECTIONS.iter().enumerate() {
        let mid = section_top(index) + height / 2.0;
        Label::new(cx, *number)
            .position_type(PositionType::SelfDirected)
            .left(Pixels(12.0))
            .top(Pixels(mid - 14.0))
            .width(Pixels(20.0))
            .height(Pixels(28.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_size(22.0)
            .color(Color::rgba(0xff, 0xff, 0xff, 0x24))
            .hoverable(false);
        label(cx, name, 52.0, mid, 9.5, 56.0, 0x8b959d);
    }
}

// ---------------------------------------------------------------------------
// The strip above the panel
// ---------------------------------------------------------------------------

fn strip(cx: &mut Context) {
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
        .position_type(PositionType::SelfDirected)
        .left(Pixels(session::BUTTON_X))
        .top(Pixels(row))
        .width(Pixels(session::BUTTON_W))
        .height(Pixels(20.0));

    session::Press::build_into(cx, "Save", true, false, || session::SessionEvent::OpenSave)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(276.0))
        .top(Pixels(row))
        .width(Pixels(46.0))
        .height(Pixels(20.0));

    // Only your own presets can be deleted, and the button says so by going
    // dim rather than by disappearing -- a strip that changes shape as the
    // selection moves is harder to aim at.
    Binding::new(cx, session::Session::deletable, |cx, deletable| {
        session::Press::build_into(cx, "Delete", deletable.get(cx), false, || {
            session::SessionEvent::OpenDelete
        })
        .position_type(PositionType::SelfDirected)
        .left(Pixels(326.0))
        .top(Pixels(HEADER_H / 2.0 - 10.0))
        .width(Pixels(52.0))
        .height(Pixels(20.0));
    });

    // How big the panel is drawn. Not part of the signal path, so it lives up
    // here with the rest of what is about the plugin rather than the sound.
    session::SizeButton::build_into(cx)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(session::SIZE_X))
        .top(Pixels(row))
        .width(Pixels(session::SIZE_W))
        .height(Pixels(20.0));

    let width = 112.0;
    let left = PANEL_W - 14.0 - width;
    // A circuit-modelled voice runs at the host rate whatever this is set to
    // -- `Chain::set_oversampling` pins it, because those circuits cannot yet
    // afford a higher one (§59.5). The row said otherwise, which is a control
    // claiming to do something it does not.
    //
    // Pinned to Off and dimmed rather than written to Off: the parameter is
    // left alone, so choosing a pedal again brings the setting back instead of
    // silently discarding it, and nothing here fights the host's automation.
    Binding::new(
        cx,
        Panel::params.map(|p| p.circuit.value().voice().is_modelled()),
        move |cx, modelled| {
            let labels: Vec<&'static str> = Oversampling::ALL.iter().map(|o| o.name()).collect();
            let handle = if modelled.get(cx) {
                Selector::pinned(cx, Panel::params, |p| &p.oversampling, labels, 0)
            } else {
                Selector::new(cx, Panel::params, |p| &p.oversampling, labels, true)
            };
            handle
                .position_type(PositionType::SelfDirected)
                .left(Pixels(left))
                .top(Pixels(row))
                .width(Pixels(width))
                .height(Pixels(20.0));
        },
    );
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
    let top = section_top(0);

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
    let meter_w = body_w() - 96.0;
    Meter::new(cx, Panel::meters)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(meter_x))
        .top(Pixels(top + 18.0))
        .width(Pixels(meter_w))
        .height(Pixels(14.0));

    label(
        cx,
        "arriving at the circuit, against the level it was voiced at",
        meter_x + meter_w / 2.0,
        top + 46.0,
        9.5,
        meter_w,
        0x7e8a96,
    );
}

// ---------------------------------------------------------------------------
// 2 Circuit
// ---------------------------------------------------------------------------

fn circuit(cx: &mut Context) {
    let top = section_top(1);

    // Four rows, in the order the signal meets them: what the topology is,
    // what part does the bending, what part does the amplifying, and what
    // iron it comes out through. Two of them apply to any given circuit and
    // two do not, and the ones that do not are greyed rather than hidden --
    // a panel that changes shape as the selection moves is harder to aim at,
    // and a control that vanishes is one you cannot see the state of.
    // Two rows for one control. The first seven entries are topologies -- a
    // valve cascade, a clipper, a channel -- and the rest are models of
    // particular circuits built from their schematics. Those are different
    // kinds of claim and deserve to look it, and ten segments on one row was
    // already too many before the rest of the models arrive.
    let names: Vec<&'static str> = Circuit::ALL.iter().map(|c| c.name()).collect();
    let modelled = Circuit::ALL.iter().filter(|c| !c.is_modelled()).count();
    label(
        cx,
        "topology",
        body_x() + 30.0,
        top + 18.0,
        9.5,
        76.0,
        0x7e8a96,
    );
    Selector::window(
        cx,
        Panel::params,
        |p| &p.circuit,
        names[..modelled].to_vec(),
        true,
        0,
        names.len(),
        None,
    )
    .position_type(PositionType::SelfDirected)
    .left(Pixels(body_x() + 76.0))
    .top(Pixels(top + 8.0))
    .width(Pixels(body_w() - 76.0))
    .height(Pixels(20.0));

    label(
        cx,
        "modelled",
        body_x() + 30.0,
        top + 48.0,
        9.5,
        76.0,
        0x7e8a96,
    );
    Selector::window(
        cx,
        Panel::params,
        |p| &p.circuit,
        names[modelled..].to_vec(),
        true,
        modelled,
        names.len(),
        None,
    )
    .position_type(PositionType::SelfDirected)
    .left(Pixels(body_x() + 76.0))
    .top(Pixels(top + 38.0))
    .width(Pixels(body_w() - 76.0))
    .height(Pixels(20.0));

    Binding::new(
        cx,
        Panel::params.map(|p| p.circuit.value().has_diodes()),
        |cx, live| {
            let live = live.get(cx);
            row(
                cx,
                section_top(1) + 68.0,
                "clipping",
                |p| &p.diode,
                Diode::ALL.iter().map(|d| d.name()).collect(),
                live,
                210.0,
            );
        },
    );

    Binding::new(
        cx,
        Panel::params.map(|p| p.circuit.value().has_amplifier()),
        |cx, live| {
            let live = live.get(cx);
            row(
                cx,
                section_top(1) + 98.0,
                "amplifier",
                |p| &p.amplifier,
                Amplifier::ALL.iter().map(|a| a.name()).collect(),
                live,
                210.0,
            );
        },
    );

    // Iron applies to everything, which is the point of it being a control
    // rather than part of a circuit: a transformer belongs after a distortion
    // pedal exactly as much as after a console channel.
    row(
        cx,
        top + 128.0,
        "iron",
        |p| &p.iron,
        Iron::ALL.iter().map(|i| i.name()).collect(),
        true,
        268.0,
    );

    // The one piece of prose that earns its space: it changes with the
    // selection, so it is telling you something you cannot see elsewhere.
    Label::new(cx, Panel::params.map(|p| describe(p.circuit.value())))
        .position_type(PositionType::SelfDirected)
        .left(Pixels(body_x()))
        .top(Pixels(top + 152.0))
        .width(Pixels(body_w()))
        .height(Pixels(22.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
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
    P: nih_plug::prelude::Param + 'static,
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
        Circuit::Screamer => {
            "Ibanez TS808. Its gain leg leaves the bottom end \
                              alone, which is why one goes in front of an amp."
        }
        Circuit::Muff => {
            "Big Muff Pi, 1973 Ram's Head. Four stages, and the \
                          tone control is the mid scoop."
        }
        Circuit::Boogie => {
            "Mesa Mark IIC+ lead channel: four triodes, and its \
                            own tone stack on the tone knobs."
        }
        Circuit::Peavey => {
            "Peavey EVH 5150 lead channel: six triodes, one of \
                            them run cold to square off the bottom."
        }
        Circuit::Neve => {
            "Neve 73P microphone preamplifier: two cascaded \
                          transistor stages with a step-up transformer."
        }
        Circuit::Twin => {
            "Fender Twin Reverb AB763: the clean one, with its own \
                          tone stack, a spring tank and a tremolo."
        }
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// 3 Drive
// ---------------------------------------------------------------------------

fn drive(cx: &mut Context) {
    let top = section_top(2);

    // Named after the pot it turns on the device selected, not after the
    // section. See `Gain::drive_name`.
    Knob::new(cx, Panel::params, |p| &p.drive, 21.0, true)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(body_x() + 30.0 - 21.0))
        .top(Pixels(top + 24.0 - 21.0));
    Label::new(
        cx,
        Panel::params.map(|p| String::from(p.circuit.value().voice().drive_name())),
    )
    .position_type(PositionType::SelfDirected)
    .left(Pixels(body_x() + 30.0 - 50.0))
    .top(Pixels(top + 24.0 + 21.0 + 10.0 - LABEL_H / 2.0))
    .width(Pixels(100.0))
    .height(Pixels(LABEL_H))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0))
    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
    .font_size(9.5)
    .color(Color::rgb(0x9a, 0xa6, 0xb0))
    .hoverable(false);
    Label::new(
        cx,
        Panel::params.map(|p| format!("{:.0} %", p.drive.value() * 100.0)),
    )
    .position_type(PositionType::SelfDirected)
    .left(Pixels(body_x() + 30.0 - 50.0))
    .top(Pixels(top + 24.0 + 21.0 + 21.0 - LABEL_H / 2.0))
    .width(Pixels(100.0))
    .height(Pixels(LABEL_H))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0))
    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
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
    Binding::new(
        cx,
        Panel::params.map(|p| p.circuit.value().voice().level_control().is_some()),
        move |cx, live| {
            let live = live.get(cx);
            Knob::new(cx, Panel::params, |p| &p.master, 21.0, live)
                .position_type(PositionType::SelfDirected)
                .left(Pixels(body_x() + 110.0 - 21.0))
                .top(Pixels(top + 24.0 - 21.0));
            label(
                cx,
                "MASTER",
                body_x() + 110.0,
                top + 24.0 + 21.0 + 10.0,
                9.5,
                100.0,
                if live { 0x9aa6b0 } else { 0x5a636b },
            );
        },
    );
    Label::new(
        cx,
        Panel::params.map(|p| {
            if p.circuit.value().voice().level_control().is_some() {
                format!("{:.0} %", p.master.value() * 100.0)
            } else {
                String::from("--")
            }
        }),
    )
    .position_type(PositionType::SelfDirected)
    .left(Pixels(body_x() + 110.0 - 50.0))
    .top(Pixels(top + 24.0 + 21.0 + 21.0 - LABEL_H / 2.0))
    .width(Pixels(100.0))
    .height(Pixels(LABEL_H))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0))
    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
    .font_size(9.5)
    .color(Color::rgb(0xff, 0x8a, 0x3c))
    .hoverable(false);

    let x = body_x() + 170.0 + (body_w() - 180.0) / 2.0;
    for (i, line) in [
        "All the way up on Drive is the sound the circuit is named for;",
        "down from there only cleans up, and the level is held across it.",
        "Master is the device's own level knob, and half way is where the",
        "voice was calibrated -- so it starts where the voicing put it.",
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
            body_w() - 180.0,
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
    Binding::new(
        cx,
        Panel::params.map(|p| p.circuit.value().voice().has_graphic()),
        move |cx, live| {
            let live = live.get(cx);
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
                Knob::fader(cx, Panel::params, to_param, 22.0, 58.0, live)
                    .position_type(PositionType::SelfDirected)
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
                "Only the Mark IIC+ has one."
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
        },
    );
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
}

// Written out rather than derived: vizia's derive asks every field to be
// `Data` itself, and an array is not one.
impl Data for ToneKnobs {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl ToneKnobs {
    pub fn of(params: &Arc<GainStageParams>) -> Self {
        Self::for_state(params.circuit.value(), params.tone.value())
    }

    pub fn for_state(circuit: Circuit, stack: ToneStack) -> Self {
        let in_circuit = stack != ToneStack::Off;
        let own = circuit.own_tone_knobs();
        // The third knob is the pedal's own control only when the plugin's
        // stack is out of the way. With the stack in circuit the same knob
        // turns the stack's treble as well, and calling it TONE while its two
        // neighbours say BASS and MID would be naming it after the smaller
        // half of what it does.
        let sole = circuit.single_tone() && !in_circuit;
        Self {
            live: [
                in_circuit || own[0],
                in_circuit || own[1],
                in_circuit || own[2],
            ],
            names: ["BASS", "MID", if sole { "TONE" } else { "TREBLE" }],
            extras: circuit.has_reverb_and_tremolo(),
        }
    }
}

fn tone(cx: &mut Context) {
    let top = section_top(3);

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
    // reported, and why the greying went in. But a TS808 and a Big Muff each
    // have a tone control of their own on the drawing, wired to this knob in
    // `voice::set_tone_knobs`, and those presets switch the stack out too. So
    // the pedals' tone knobs were greyed out while still working, which is the
    // same fault the other way round.
    Binding::new(cx, Panel::params.map(ToneKnobs::of), move |cx, state| {
        let state = state.get(cx);
        let top = section_top(3);
        for (i, to_param) in knobs.into_iter().enumerate() {
            let live = state.live[i];
            let x = body_x() + 46.0 + i as f32 * 84.0;
            Knob::new(cx, Panel::params, to_param, 18.0, live)
                .position_type(PositionType::SelfDirected)
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
        // only the Twin does. A knob that turns and reaches nothing is
        // indistinguishable from a fault -- the same reason the three beside
        // them are greyed, and the same reason BUG-023 happened.
        let live = state.extras;
        for (i, to_param) in extras.into_iter().enumerate() {
            let x = body_x() + 46.0 + i as f32 * 84.0;
            Knob::new(cx, Panel::params, to_param, 18.0, live)
                .position_type(PositionType::SelfDirected)
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
    });

    // Kept to lines that fit the space rather than sentences that overflow
    // it: text wider than its box is simply clipped, with no warning.
    //
    // Two paragraphs, one a row: the top three knobs are the tone stack, the
    // bottom three are the Twin's reverb and tremolo, and the text sits beside
    // the row it is about.
    let x = body_x() + 340.0;
    for (i, line) in [
        "A passive stack only ever cuts. The",
        "scooping voicing has a resonant leg,",
        "which dips the middle. A circuit with",
        "tone controls of its own uses these.",
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
        "Below: the Twin Reverb's spring tank",
        "and its optical tremolo. Reverb is the",
        "recovery stage's own mix control;",
        "Speed and Intensity drive the bulb.",
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
    let top = section_top(4);

    row(
        cx,
        top + 12.0,
        "speaker",
        |p| &p.cabinet,
        Cabinet::ALL.iter().map(|c| c.name()).collect(),
        true,
        230.0,
    );

    // Two lines rather than one. A label wider than its box is not wrapped or
    // clipped to it -- it spills out over whatever is beside it, which here
    // was the selector it sits next to.
    let x = body_x() + 76.0 + 230.0 + (body_w() - 306.0) / 2.0;
    let w = body_w() - 306.0;
    label(
        cx,
        "Most of what a distorted amplifier",
        x,
        top + 14.0,
        9.5,
        w,
        0x86929c,
    );
    label(
        cx,
        "sounds like. A preamp wants it off.",
        x,
        top + 30.0,
        9.5,
        w,
        0x86929c,
    );
}

// ---------------------------------------------------------------------------
// 6 Output
// ---------------------------------------------------------------------------

fn output(cx: &mut Context) {
    let top = section_top(5);

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

    // The plugin in or out of circuit, bottom right where a footswitch would
    // be. `make_bypass` tells the host this is the bypass, so it can sit on the
    // DAW's own strip and be automated -- which is the point: dropping a
    // Screamer in for a solo from a controller.
    //
    // ON is the parameter being *false*. A bypass parameter reads "is it
    // bypassed", and a switch on a panel reads "is it on", so the row is
    // ordered to make those the same gesture.
    selector(
        cx,
        PANEL_W - 14.0 - 86.0,
        top + 40.0,
        86.0,
        |p| &p.bypass,
        vec!["ON", "OFF"],
        true,
    );
    label(
        cx,
        "plugin",
        PANEL_W - 14.0 - 43.0,
        top + 22.0,
        9.5,
        86.0,
        0x7e8a96,
    );
    let x = body_x() + 340.0;
    label(
        cx,
        "The dry signal is delayed to match, so",
        x,
        top + 26.0,
        9.5,
        250.0,
        0x86929c,
    );
    label(
        cx,
        "mixing the two is a mix and not a comb.",
        x,
        top + 42.0,
        9.5,
        250.0,
        0x86929c,
    );
}
