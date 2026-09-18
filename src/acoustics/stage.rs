//! The cabinet and microphones as one linear (almost) processor, per channel.
//!
//! Input is the cone's radiation proxy from the speaker model: `Re Mms / Bl^2`
//! times the rate of change of the motional voltage, which is the on-axis pressure
//! normalised to one volt at the terminals in the mass-controlled band. So this
//! stage never sees the terminal voltage, and the load's interaction with the
//! amplifier has already happened inside the power-stage solve.
//!
//! ```text
//! cone -> breakup voicing -> [closed box: standing waves, back panel] -> delay line
//!   delay line --(per driver, per mic: fractional delay, distance, piston
//!                 directivity, polar weight, off-axis loss)--> omni / gradient sums
//!   [open back: inverted rear wave per driver around the nearest edge]
//!   gradient -> + proximity (leaky integrator) ; sum -> baffle step -> mic response
//!            -> distance make-up -> subtle saturation
//!   mic A, mic B -> blend / polarity / alignment -> out
//! ```
//!
//! Everything that depends on geometry is recomputed at control rate and ramped
//! per sample. Nothing allocates. See `CABINET_MODEL.md` and `MICROPHONE_MODEL.md`.

use super::cabinet::CabinetProfile;
use super::filters::{Biquad, DelayLine, OnePole};
use super::mic::{MicPlacement, MicProfile, Pattern};
use super::speaker::{SpeakerProfile, SPEED_OF_SOUND};
use std::f64::consts::TAU;

/// Four drivers in front, and a rear wave for each of up to four drivers.
pub const MAX_PATHS: usize = 8;
/// Distance at which the geometric level is unity, m.
const REFERENCE_DISTANCE: f64 = 0.05;
/// Fraction of the geometric level loss (in dB) that is made up, so distant
/// placements stay usable. The relative level between the two microphones stays
/// physical when both are equally far; see `MICROPHONE_MODEL.md`.
const DISTANCE_MAKEUP: f64 = 0.5;
/// Effective radiating radius above breakup, as a fraction of the cone's. EMPIRICALLY
/// TUNED so the edge of a 12-inch cone at 2.5 cm is about 6 dB darker at 5 kHz than
/// its centre, which is the scale of difference manufacturers' placement guides describe.
const HF_RADIUS: f64 = 0.4;
/// Piston directivity -3 dB point: `2 J1(x)/x = 0.707` at `x = ka sin(theta) = 2.2`.
const PISTON_X: f64 = 2.2;
/// Samples over which control changes are ramped.
const RAMP: usize = 64;
/// Largest proximity boost the leaky integrator is allowed to give, as a ratio.
const PROXIMITY_CEILING: f64 = 8.0;

/// What a microphone slot holds.
#[derive(Clone, Copy, PartialEq)]
pub enum MicSlot {
    Off,
    /// A perfect omnidirectional pressure transducer at the placement: geometry only.
    Ideal,
    Profile(&'static MicProfile),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Ramped {
    now: f64,
    step: f64,
    target: f64,
}

impl Ramped {
    fn aim(&mut self, target: f64, snap: bool) {
        self.target = target;
        if snap {
            self.now = target;
            self.step = 0.0;
        } else {
            self.step = (target - self.now) / RAMP as f64;
        }
    }

    #[inline]
    fn advance(&mut self, last: bool) {
        if last {
            self.now = self.target;
            self.step = 0.0;
        } else {
            self.now += self.step;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Path {
    delay: Ramped,
    omni: Ramped,
    gradient: Ramped,
    directivity: OnePole,
    off_axis: OnePole,
    diffraction: OnePole,
    /// Absolute delay before the common minimum was removed, samples.
    absolute: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MicChannel {
    slot: MicSlot,
    paths: [Path; MAX_PATHS],
    count: usize,
    proximity: OnePole,
    proximity_on: bool,
    baffle: Biquad,
    eq: [Biquad; 6],
    eq_norm: f64,
    trim: Ramped,
    cubic: f64,
    quadratic: f64,
    dc: OnePole,
}

impl MicChannel {
    fn new() -> Self {
        Self {
            slot: MicSlot::Off,
            paths: [Path::default(); MAX_PATHS],
            count: 0,
            proximity: OnePole::open(),
            proximity_on: false,
            baffle: Biquad::IDENTITY,
            eq: [Biquad::IDENTITY; 6],
            eq_norm: 1.0,
            trim: Ramped::default(),
            cubic: 0.0,
            quadratic: 0.0,
            dc: OnePole::open(),
        }
    }

    fn reset(&mut self) {
        for path in &mut self.paths {
            path.directivity.reset();
            path.off_axis.reset();
            path.diffraction.reset();
        }
        self.proximity.reset();
        self.baffle.reset();
        for bq in &mut self.eq {
            bq.reset();
        }
        self.dc.reset();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AcousticStage {
    rate: f64,
    cabinet: Option<&'static CabinetProfile>,
    speaker: &'static SpeakerProfile,
    line: DelayLine,
    voicing: [Biquad; 4],
    voicing_norm: f64,
    enclosure: [Biquad; 4],
    enclosure_on: bool,
    mics: [MicChannel; 2],
    placements: [MicPlacement; 2],
    blend: Ramped,
    invert: bool,
    align: bool,
    ramp_left: usize,
}

impl std::fmt::Debug for MicSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MicSlot::Off => write!(f, "Off"),
            MicSlot::Ideal => write!(f, "Ideal"),
            MicSlot::Profile(p) => write!(f, "{}", p.id),
        }
    }
}

/// One-pole corner for a capsule's extra high-frequency loss at incidence `psi`: the
/// profile's loss at 90 degrees and 10 kHz, scaled in dB by `sin^2 psi` up to 90 degrees
/// and held beyond. Consistent with the SM57's published polars (about half the 90 degree
/// extra loss at 45 degrees). Zero loss is a wire.
fn off_axis_corner(off_db: f64, cos_psi: f64) -> f64 {
    let weight = if cos_psi >= 0.0 {
        1.0 - cos_psi * cos_psi
    } else {
        1.0
    };
    let loss = off_db * weight;
    if loss < 1e-3 {
        return f64::INFINITY;
    }
    10_000.0 / (10f64.powf(loss / 10.0) - 1.0).sqrt()
}

impl AcousticStage {
    pub fn new(rate: f64) -> Self {
        let mut stage = Self {
            rate,
            cabinet: None,
            speaker: &SpeakerProfile::BRIT_V30,
            line: DelayLine::new(),
            voicing: [Biquad::IDENTITY; 4],
            voicing_norm: 1.0,
            enclosure: [Biquad::IDENTITY; 4],
            enclosure_on: false,
            mics: [MicChannel::new(), MicChannel::new()],
            placements: [MicPlacement::default(); 2],
            blend: Ramped::default(),
            invert: false,
            align: false,
            ramp_left: 0,
        };
        stage.mics[0].slot = MicSlot::Profile(&MicProfile::DYNAMIC_57);
        stage.rebuild();
        stage
    }

    /// Select what is in front of the power stage. Changing anything resets state,
    /// which the chain covers with its switch fade.
    pub fn configure(
        &mut self,
        cabinet: Option<&'static CabinetProfile>,
        speaker: &'static SpeakerProfile,
        mic_a: MicSlot,
        mic_b: MicSlot,
    ) {
        if self.cabinet != cabinet
            || self.speaker != speaker
            || self.mics[0].slot != mic_a
            || self.mics[1].slot != mic_b
        {
            self.cabinet = cabinet;
            self.speaker = speaker;
            self.mics[0].slot = mic_a;
            self.mics[1].slot = mic_b;
            self.rebuild();
            self.reset();
        }
    }

    /// Placement and the dual-microphone controls. Ramped; call once a block.
    pub fn set_placement(
        &mut self,
        a: MicPlacement,
        b: MicPlacement,
        blend: f64,
        invert: bool,
        align: bool,
    ) {
        let (a, b) = (a.clamped(), b.clamped());
        let blend = if blend.is_finite() {
            blend.clamp(0.0, 1.0)
        } else {
            0.5
        };
        if self.placements != [a, b]
            || self.invert != invert
            || self.align != align
            || self.blend.target != blend
        {
            self.placements = [a, b];
            self.invert = invert;
            self.align = align;
            self.blend.aim(blend, false);
            self.geometry(false);
        }
    }

    pub fn set_rate(&mut self, rate: f64) {
        if (self.rate - rate).abs() > 1e-9 {
            self.rate = rate;
            self.rebuild();
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.line.reset();
        for bq in self.voicing.iter_mut().chain(self.enclosure.iter_mut()) {
            bq.reset();
        }
        for mic in &mut self.mics {
            mic.reset();
        }
        self.geometry(true);
        self.blend.aim(self.blend.target, true);
    }

    pub fn copy_runtime_state_from(&mut self, source: &Self) {
        self.clone_from(source);
    }

    /// The shortest and longest relative propagation delay of a microphone, samples.
    pub fn relative_delays(&self, mic: usize) -> (f64, f64) {
        let paths = &self.mics[mic].paths[..self.mics[mic].count];
        let shortest = paths
            .iter()
            .map(|p| p.delay.target)
            .fold(f64::INFINITY, f64::min);
        let longest = paths.iter().map(|p| p.delay.target).fold(0.0, f64::max);
        (shortest, longest)
    }

    fn rebuild(&mut self) {
        let rate = self.rate;
        let b = self.speaker.breakup;
        for (bq, peak) in self.voicing.iter_mut().zip(b.peaks.iter()) {
            bq.set_peaking(rate, peak.hz, peak.gain_db, peak.q);
        }
        self.voicing[3] = Biquad::lowpass(rate, b.lowpass_hz, b.lowpass_q);
        let at = 600.0;
        self.voicing_norm = 1.0
            / self
                .voicing
                .iter()
                .map(|bq| bq.magnitude(rate, at))
                .product::<f64>();

        self.enclosure_on = false;
        if let Some(cab) = self.cabinet.filter(|c| !c.is_open()) {
            // Standing waves reflect on to the back of the cone: small dips at the
            // axial modes, deepest along the shortest dimension. The back panel's
            // fundamental radiates a little of its own. Levels TUNED small.
            let [depth, width, height] = cab.modes();
            self.enclosure[0].set_peaking(rate, depth, -2.0, 4.0);
            self.enclosure[1].set_peaking(rate, width, -1.0, 5.0);
            self.enclosure[2].set_peaking(rate, height, -1.0, 5.0);
            self.enclosure[3].set_peaking(rate, cab.panel_hz(), 1.0, 3.0);
            self.enclosure_on = true;
        }

        for mic in &mut self.mics {
            mic.eq = [Biquad::IDENTITY; 6];
            mic.eq_norm = 1.0;
            mic.cubic = 0.0;
            mic.quadratic = 0.0;
            if let MicSlot::Profile(p) = mic.slot {
                mic.eq[0] = Biquad::highpass(rate, p.highpass.0, p.highpass.1);
                for (bq, peak) in mic.eq[1..5].iter_mut().zip(p.peaks.iter()) {
                    bq.set_peaking(rate, peak.hz, peak.gain_db, peak.q);
                }
                mic.eq[5] = Biquad::lowpass(rate, p.lowpass.0, p.lowpass.1);
                mic.eq_norm = 1.0
                    / mic
                        .eq
                        .iter()
                        .map(|bq| bq.magnitude(rate, 1_000.0))
                        .product::<f64>();
                mic.cubic = p.family.cubic();
                mic.quadratic = p.family.quadratic();
            }
            mic.dc.set_lowpass(rate, 5.0);
        }
        self.geometry(true);
    }

    /// Recompute every path from the placements. `snap` skips the ramp.
    fn geometry(&mut self, snap: bool) {
        let rate = self.rate;
        let c = SPEED_OF_SOUND;
        let cabinet = self.cabinet;
        let placements = self.placements;
        let voicing_norm = self.voicing_norm;
        let align = self.align;
        let radius = self.speaker.radius();
        let hf_radius = HF_RADIUS * radius;
        let single = [(0.0, 0.0)];
        let drivers: &[(f64, f64)] = cabinet.map(|cab| cab.driver_positions()).unwrap_or(&single);
        let target = drivers[0];
        let outward = if target.0 > 0.0 { 1.0 } else { -1.0 };
        let geometric = |length: f64| {
            ((radius * radius + REFERENCE_DISTANCE * REFERENCE_DISTANCE)
                / (radius * radius + length * length))
                .sqrt()
        };

        let mut minimum = [f64::INFINITY; 2];
        for (m, mic) in self.mics.iter_mut().enumerate() {
            let placement = placements[m];
            let profile = match mic.slot {
                MicSlot::Off => {
                    mic.count = 0;
                    continue;
                }
                MicSlot::Ideal => None,
                MicSlot::Profile(p) => Some(p),
            };
            let (pa, pb) = profile
                .map(|p| p.pattern)
                .unwrap_or(Pattern::Omni)
                .coefficients();
            let depth = profile.map(|p| p.capsule_depth).unwrap_or(0.0);
            let at = (
                target.0 + outward * placement.position * radius,
                target.1,
                placement.distance + depth,
            );
            let tilt = placement.angle.to_radians();
            let axis = (-outward * tilt.sin(), 0.0, -tilt.cos());
            let off_db = profile.map(|p| p.off_axis_db).unwrap_or(0.0);

            let mut count = 0;
            let mut nearest = f64::INFINITY;
            let push = |path: &mut Path,
                        delay: f64,
                        omni: f64,
                        gradient: f64,
                        directivity: f64,
                        off: f64,
                        diffraction: f64| {
                path.absolute = delay;
                path.omni.aim(omni, snap);
                path.gradient.aim(gradient, snap);
                path.directivity.set_lowpass(rate, directivity);
                path.off_axis.set_lowpass(rate, off);
                path.diffraction.set_lowpass(rate, diffraction);
            };
            for &(x, y) in drivers {
                // The whole cone moves together at low frequencies, so the level,
                // arrival time and incidence come from the nearest point of the cone.
                // Above breakup only the middle radiates, so the directivity is
                // taken from the centre.
                let (dx, dy) = (at.0 - x, at.1 - y);
                let lateral = dx.hypot(dy);
                let pull = if lateral > radius {
                    radius / lateral
                } else {
                    1.0
                };
                let v = (x + dx * pull - at.0, y + dy * pull - at.1, -at.2);
                let length = (v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt().max(0.005);
                nearest = nearest.min(length);
                let to_centre = (lateral * lateral + at.2 * at.2).sqrt().max(0.005);
                let sin_theta = (lateral / to_centre).clamp(0.0, 1.0);
                let cos_psi =
                    ((v.0 * axis.0 + v.1 * axis.1 + v.2 * axis.2) / length).clamp(-1.0, 1.0);
                let directivity = if sin_theta > 1e-6 {
                    PISTON_X * c / (TAU * hf_radius * sin_theta)
                } else {
                    f64::INFINITY
                };
                let off = off_axis_corner(off_db, cos_psi);
                let g = geometric(length);
                push(
                    &mut mic.paths[count],
                    length / c * rate,
                    pa * g,
                    pb * g * cos_psi,
                    directivity,
                    off,
                    f64::INFINITY,
                );
                count += 1;
            }
            if let Some(cab) = cabinet.filter(|cab| cab.is_open()) {
                // The back of each cone radiates in antiphase out of the open back
                // and reaches the front round the nearest edge of the cabinet.
                let (w, h) = (cab.width / 2.0, cab.height / 2.0);
                let (_, _, inside) = cab.internal();
                for &(x, y) in drivers {
                    let edges = [
                        (x + w, (-w, y)),
                        (w - x, (w, y)),
                        (h - y, (x, h)),
                        (y + h, (x, -h)),
                    ];
                    let (edge, point) =
                        edges
                            .iter()
                            .copied()
                            .fold((f64::INFINITY, (0.0, 0.0)), |best, e| {
                                if e.0 < best.0 {
                                    e
                                } else {
                                    best
                                }
                            });
                    let v = (point.0 - at.0, point.1 - at.1, -at.2);
                    let front = (v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt().max(0.005);
                    let around = inside + edge;
                    let length = around + front;
                    let cos_psi =
                        ((v.0 * axis.0 + v.1 * axis.1 + v.2 * axis.2) / front).clamp(-1.0, 1.0);
                    let off = off_axis_corner(off_db, cos_psi);
                    // At low frequencies the box air and the opening pass nearly the
                    // whole rear volume velocity: a dipole. A partly closed back still
                    // passes most of it. APPROXIMATED: gain min(1, 2 x open fraction).
                    let g = -(2.0 * cab.open_fraction).min(1.0) * geometric(length);
                    // Diffraction round the edge loses the top once the wavelength is
                    // shorter than the way round. ESTIMATED corner c / (pi L): a lower one
                    // puts a low-pass phase lag on the bass the dipole is meant to cancel.
                    let diffraction = c / (std::f64::consts::PI * around);
                    push(
                        &mut mic.paths[count],
                        length / c * rate,
                        pa * g,
                        pb * g * cos_psi,
                        f64::INFINITY,
                        off,
                        diffraction,
                    );
                    count += 1;
                }
            }
            mic.count = count;
            minimum[m] = mic.paths[..count]
                .iter()
                .map(|p| p.absolute)
                .fold(f64::INFINITY, f64::min);

            // Proximity: the gradient term's near-field rise, from a source no smaller
            // than the cone that radiates it.
            let proximity = profile.map(|p| p.proximity).unwrap_or(0.0) * pb;
            mic.proximity_on = proximity > 0.0;
            if mic.proximity_on {
                let r = (nearest * nearest + (0.7 * radius).powi(2)).sqrt();
                let wp = profile.map(|p| p.proximity).unwrap_or(0.0) * c / r;
                let fp = wp / TAU;
                mic.proximity
                    .set_leaky_integrator(rate, fp / PROXIMITY_CEILING, PROXIMITY_CEILING);
            }

            // Baffle step: close to the baffle the cone radiates into a half space;
            // at a distance comparable to the baffle it does not.
            match cabinet {
                Some(cab) => {
                    let far = placement.distance / (placement.distance + cab.width / 2.0);
                    mic.baffle
                        .set_low_shelf(rate, cab.baffle_step_hz(), -6.0 * far);
                }
                None => mic.baffle = Biquad::IDENTITY,
            }

            let makeup = (1.0 / geometric(nearest)).powf(DISTANCE_MAKEUP);
            mic.trim.aim(makeup * mic.eq_norm * voicing_norm, snap);
        }

        let common = minimum[0].min(minimum[1]);
        for (m, mic) in self.mics.iter_mut().enumerate() {
            let base = if align { minimum[m] } else { common };
            for path in &mut mic.paths[..mic.count] {
                path.delay.aim(path.absolute - base, snap);
            }
        }
        self.ramp_left = if snap { 0 } else { RAMP };
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let mut s = x;
        for bq in &mut self.voicing {
            s = bq.process(s);
        }
        if self.enclosure_on {
            for bq in &mut self.enclosure {
                s = bq.process(s);
            }
        }
        self.line.write(s);

        let ramping = self.ramp_left > 0;
        let last = self.ramp_left == 1;
        if ramping {
            self.ramp_left -= 1;
            self.blend.advance(last);
        }

        let mut out = [0.0; 2];
        for (m, mic) in self.mics.iter_mut().enumerate() {
            if mic.count == 0 {
                continue;
            }
            let (mut omni, mut gradient) = (0.0, 0.0);
            for path in &mut mic.paths[..mic.count] {
                if ramping {
                    path.delay.advance(last);
                    path.omni.advance(last);
                    path.gradient.advance(last);
                }
                let mut y = self.line.read(path.delay.now);
                y = path.directivity.process(y);
                y = path.off_axis.process(y);
                y = path.diffraction.process(y);
                omni += path.omni.now * y;
                gradient += path.gradient.now * y;
            }
            if mic.proximity_on {
                gradient += mic.proximity.process(gradient);
            }
            let mut v = mic.baffle.process(omni + gradient);
            for bq in &mut mic.eq {
                v = bq.process(v);
            }
            if ramping {
                mic.trim.advance(last);
            }
            v *= mic.trim.now;
            if mic.cubic > 0.0 {
                v /= (1.0 + 2.0 * mic.cubic * v * v).sqrt();
            }
            if mic.quadratic > 0.0 {
                let square = v * v;
                v += mic.quadratic * (square - mic.dc.process(square));
            }
            out[m] = v;
        }

        let a_on = self.mics[0].count > 0;
        let b_on = self.mics[1].count > 0;
        let b = if self.invert { -out[1] } else { out[1] };
        match (a_on, b_on) {
            (true, true) => (1.0 - self.blend.now) * out[0] + self.blend.now * b,
            (true, false) => out[0],
            (false, true) => b,
            (false, false) => 0.0,
        }
    }
}
