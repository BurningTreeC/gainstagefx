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
mod session;
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
    ViziaState::new(|| (PANEL_W as u32, WINDOW_H as u32))
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
    create_vizia_editor(editor_state, ViziaTheming::None, move |cx, _| {
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

        session::Session::build_into(cx, params.clone(), state.user_scale_factor());

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
    label(cx, "GAINSTAGEFX", 62.0, HEADER_H / 2.0, 10.5, 116.0, 0xe8eef4);

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
        session::Press::build_into(
            cx,
            "Delete",
            deletable.get(cx),
            false,
            || session::SessionEvent::OpenDelete,
        )
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
    selector(
        cx,
        left,
        row,
        width,
        |p| &p.oversampling,
        Oversampling::ALL.iter().map(|o| o.name()).collect(),
        true,
    );
    label(cx, "quality", left - 26.0, HEADER_H / 2.0, 9.5, 44.0, 0x7e8a96);
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
    label(cx, "topology", body_x() + 30.0, top + 18.0, 9.5, 76.0, 0x7e8a96);
    Selector::window(cx, Panel::params, |p| &p.circuit,
        names[..modelled].to_vec(), true, 0, names.len())
        .position_type(PositionType::SelfDirected)
        .left(Pixels(body_x() + 76.0))
        .top(Pixels(top + 8.0))
        .width(Pixels(body_w() - 76.0))
        .height(Pixels(20.0));

    label(cx, "modelled", body_x() + 30.0, top + 48.0, 9.5, 76.0, 0x7e8a96);
    Selector::window(cx, Panel::params, |p| &p.circuit,
        names[modelled..].to_vec(), true, modelled, names.len())
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
            row(cx, section_top(1) + 68.0, "clipping", |p| &p.diode,
                Diode::ALL.iter().map(|d| d.name()).collect(), live, 210.0);
        },
    );

    Binding::new(
        cx,
        Panel::params.map(|p| p.circuit.value().has_amplifier()),
        |cx, live| {
            let live = live.get(cx);
            row(cx, section_top(1) + 98.0, "amplifier", |p| &p.amplifier,
                Amplifier::ALL.iter().map(|a| a.name()).collect(), live, 210.0);
        },
    );

    // Iron applies to everything, which is the point of it being a control
    // rather than part of a circuit: a transformer belongs after a distortion
    // pedal exactly as much as after a console channel.
    row(cx, top + 128.0, "iron", |p| &p.iron,
        Iron::ALL.iter().map(|i| i.name()).collect(), true, 268.0);

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
        Circuit::Clean => "One valve stage barely working: a signal having been \
                           through something, not distortion.",
        Circuit::Crunch => "Two stages, the second driven by the first, so each \
                            amplifies the last one's distortion as well.",
        Circuit::HighGain => "Three stages run hard, all clipping on every note. \
                              Where the gain stops being a texture.",
        Circuit::Overdrive => "Diodes across the feedback resistor: they lower the \
                               gain, so it keeps following and cleans up.",
        Circuit::Distortion => "Diodes across the signal to ground: a ceiling. The \
                                wave is squared off, top to bottom of the band.",
        Circuit::Console => "A step-up transformer into a discrete stage, built \
                             not to run out of room.",
        Circuit::Studio => "An op-amp on a studio rail: nothing of its own \
                            anywhere in the band. Add iron to give it some.",
        Circuit::Screamer => "Ibanez TS808. Its gain leg leaves the bottom end \
                              alone, which is why one goes in front of an amp.",
        Circuit::Muff => "Big Muff Pi, 1973 Ram's Head. Four stages, and the \
                          tone control is the mid scoop.",
        Circuit::Boogie => "Mesa Mark IIC+ lead channel: four triodes, and its \
                            own tone stack on the tone knobs.",
        Circuit::Peavey => "Peavey EVH 5150 lead channel: six triodes, one of \
                            them run cold to square off the bottom.",
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// 3 Drive
// ---------------------------------------------------------------------------

fn drive(cx: &mut Context) {
    let top = section_top(2);

    knob(
        cx,
        body_x() + 30.0,
        top + 24.0,
        21.0,
        "DRIVE",
        |p| &p.drive,
        |p| format!("{:.0} %", p.drive.value() * 100.0),
    );

    let x = body_x() + 90.0 + (body_w() - 100.0) / 2.0;
    label(cx, "All the way up is the sound the circuit is named for; down from", x, top + 26.0, 9.5, body_w() - 100.0, 0x86929c);
    label(cx, "there only cleans up. The level is held across the whole travel.", x, top + 42.0, 9.5, body_w() - 100.0, 0x86929c);
}

// ---------------------------------------------------------------------------
// 4 Tone
// ---------------------------------------------------------------------------

fn tone(cx: &mut Context) {
    let top = section_top(3);

    row(cx, top + 8.0, "stack", |p| &p.tone,
        ToneStack::ALL.iter().map(|t| t.name()).collect(), true, 230.0);

    let names: [(&str, ToKnob); 3] = [
        ("BASS", |p| &p.bass),
        ("MID", |p| &p.mid),
        ("TREBLE", |p| &p.treble),
    ];
    // Greyed when the stack is out of circuit, because then they reach
    // nothing at all -- and fourteen of the shipped presets switch it out. A
    // knob that turns and changes nothing is indistinguishable from a fault,
    // which is exactly how it was reported.
    Binding::new(
        cx,
        Panel::params.map(|p| p.tone.value() != ToneStack::Off),
        move |cx, live| {
            let live = live.get(cx);
            let top = section_top(3);
            for (i, (name, to_param)) in names.into_iter().enumerate() {
                let x = body_x() + 46.0 + i as f32 * 84.0;
                Knob::new(cx, Panel::params, to_param, 18.0, live)
                    .position_type(PositionType::SelfDirected)
                    .left(Pixels(x - 18.0))
                    .top(Pixels(top + 40.0));
                label(
                    cx,
                    name,
                    x,
                    top + 86.0,
                    9.5,
                    80.0,
                    if live { 0x9aa6b0 } else { 0x5a636b },
                );
            }
        },
    );

    // Kept to lines that fit the space rather than sentences that overflow
    // it: text wider than its box is simply clipped, with no warning.
    let x = body_x() + 340.0;
    label(cx, "A passive stack only ever cuts.", x, top + 44.0, 9.5, 240.0, 0x86929c);
    label(cx, "The scooping voicing has a resonant", x, top + 60.0, 9.5, 240.0, 0x86929c);
    label(cx, "leg, which dips the middle.", x, top + 76.0, 9.5, 240.0, 0x86929c);
}

// ---------------------------------------------------------------------------
// 5 Cabinet
// ---------------------------------------------------------------------------

fn cabinet(cx: &mut Context) {
    let top = section_top(4);

    row(cx, top + 12.0, "speaker", |p| &p.cabinet,
        Cabinet::ALL.iter().map(|c| c.name()).collect(), true, 230.0);

    // Two lines rather than one. A label wider than its box is not wrapped or
    // clipped to it -- it spills out over whatever is beside it, which here
    // was the selector it sits next to.
    let x = body_x() + 76.0 + 230.0 + (body_w() - 306.0) / 2.0;
    let w = body_w() - 306.0;
    label(cx, "Most of what a distorted amplifier", x, top + 14.0, 9.5, w, 0x86929c);
    label(cx, "sounds like. A preamp wants it off.", x, top + 30.0, 9.5, w, 0x86929c);
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

    let x = body_x() + 340.0;
    label(cx, "The dry signal is delayed to match, so", x, top + 26.0, 9.5, 250.0, 0x86929c);
    label(cx, "mixing the two is a mix and not a comb.", x, top + 42.0, 9.5, 250.0, 0x86929c);
}
