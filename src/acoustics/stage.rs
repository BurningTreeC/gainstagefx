//! Cone acceleration -> radiated pressure field -> microphones -> output calibration.
//!
//! All cones contribute coherent Rayleigh surface integrals to each microphone.
//! The cabinet load is in the amplifier solve; the same uniform cavity equations
//! provide opening and panel radiation here. Pressure is in Pa until the final
//! fixed output calibration. Placement changes crossfade pressure kernels without
//! allocating. A common propagation time is removed to preserve plugin latency.

use super::cabinet::{CabinetProfile, MAX_DRIVERS};
use super::diffraction::{self, ROUTES};
use super::enclosure::{CavityRadiation, RADIATORS};
use super::filters::{Biquad, DelayLine, DelayTap, OnePole};
use super::mic::{MicPlacement, MicProfile, Pattern};
use super::radiation::PressureField;
use super::speaker::{SpeakerProfile, RHO, SPEED_OF_SOUND};
use std::f64::consts::{PI, TAU};

const RAMP: usize = 64;
const REFERENCE_DISTANCE: f64 = 0.05;
pub const MAX_PATHS: usize = RADIATORS * ROUTES;

#[derive(Clone, Copy, PartialEq)]
pub enum MicSlot {
    Off,
    /// Ideal omnidirectional pressure transducer, including propagation geometry.
    Ideal,
    Profile(&'static MicProfile),
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
struct RearPath {
    source: usize,
    delay: Ramped,
    /// `delay.now`'s interpolation weights, cached while it holds still.
    tap: DelayTap,
    pressure: Ramped,
    velocity: Ramped,
    diffraction: OnePole,
}

impl RearPath {
    /// Whether this path can add anything to the capsule, now or later in
    /// the current ramp. A path with both gains at zero, heading to zero, and
    /// nothing left ringing in its diffraction pole adds `0.0` every sample,
    /// and goes on doing so until `geometry` aims it somewhere else.
    fn can_contribute(&self) -> bool {
        self.pressure.now != 0.0
            || self.pressure.target != 0.0
            || self.velocity.now != 0.0
            || self.velocity.target != 0.0
            || !self.diffraction.is_at_rest()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct BreakupPath {
    delay: Ramped,
    /// `delay.now`'s interpolation weights, cached while it holds still.
    tap: DelayTap,
    pressure: Ramped,
    velocity: Ramped,
    directivity: OnePole,
    beam: [Biquad; 2],
    far: bool,
}

/// The horn's path to one capsule: its delay and its gain, pascals for a
/// volt at the horn's driver, for where the capsule is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct HornPath {
    delay: Ramped,
    tap: DelayTap,
    gain: Ramped,
}

#[derive(Clone, Debug, PartialEq)]
struct MicChannel {
    slot: MicSlot,
    horn: HornPath,
    field: PressureField,
    front_panel: PressureField,
    breakup_paths: [BreakupPath; MAX_DRIVERS],
    driver_count: usize,
    rear: [RearPath; MAX_PATHS],
    /// The rear paths that can contribute, as indices into `rear` in their
    /// original order, so the sum over them is the same sum.
    ///
    /// Every panel has thirteen routes to each capsule and most of them are
    /// round an edge the capsule cannot see, so their weight is exactly zero;
    /// the front panel's own thirteen are zero by construction because it is
    /// surface-integrated in `front_panel`. On the Puppet Master's closed 4x12
    /// with two microphones, 24 of 91 per capsule carry any weight. Reading
    /// the other 67 was two Lagrange interpolations and a one-pole each, every
    /// sample, adding exactly zero: 12.5 % of that preset's CPU. Rebuilt by
    /// `geometry`, the only thing that aims a path.
    rear_live: [u8; MAX_PATHS],
    rear_live_len: usize,
    off_axis: OnePole,
    baffle: Biquad,
    eq: [Biquad; 6],
    eq_norm: f64,
    delays: (f64, f64),
}
impl MicChannel {
    fn new() -> Self {
        Self {
            slot: MicSlot::Off,
            horn: HornPath::default(),
            field: PressureField::default(),
            front_panel: PressureField::default(),
            breakup_paths: [BreakupPath::default(); MAX_DRIVERS],
            driver_count: 0,
            rear: [RearPath::default(); MAX_PATHS],
            rear_live: [0; MAX_PATHS],
            rear_live_len: 0,
            off_axis: OnePole::open(),
            baffle: Biquad::IDENTITY,
            eq: [Biquad::IDENTITY; 6],
            eq_norm: 1.0,
            delays: (0.0, 0.0),
        }
    }
    /// Re-index the rear paths that can contribute. A superset is always
    /// correct -- it only costs the reads -- so a path stays listed while its
    /// diffraction pole is still ringing, and leaves at the next re-aim.
    fn index_live_rear_paths(&mut self) {
        let mut len = 0;
        for (i, path) in self.rear.iter().enumerate() {
            if path.can_contribute() {
                self.rear_live[len] = i as u8;
                len += 1;
            }
        }
        self.rear_live_len = len;
    }

    fn reset(&mut self) {
        for path in &mut self.breakup_paths {
            path.directivity.reset();
            for filter in &mut path.beam {
                filter.reset();
            }
        }
        for path in &mut self.rear {
            path.diffraction.reset();
        }
        self.off_axis.reset();
        self.baffle.reset();
        for b in &mut self.eq {
            b.reset();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AcousticStage {
    rate: f64,
    cabinet: Option<&'static CabinetProfile>,
    speaker: &'static SpeakerProfile,
    acceleration: DelayLine,
    velocity: DelayLine,
    integrate: OnePole,
    breakup_acceleration: DelayLine,
    breakup_velocity: DelayLine,
    breakup_integrate: OnePole,
    piston_band: OnePole,
    cavity: CavityRadiation,
    rear_acceleration: [DelayLine; RADIATORS],
    rear_velocity: [DelayLine; RADIATORS],
    rear_integrate: [OnePole; RADIATORS],
    voicing: [Biquad; 4],
    /// The cabinet's passive crossover on the woofers, when it has one: a
    /// third-order Butterworth low-pass, a pole and a pair at Q 1. Applied to
    /// the cone's motion rather than stamped into the amplifier's load: the
    /// load the woofers present through the crossover's series inductor
    /// differs from the bare woofers only near and above the corner, kilohertz
    /// above where the power stage cares, and the series parts would cost
    /// every speaker-loaded stage the unknowns. See `CabinetProfile::crossover_hz`.
    crossover: (OnePole, Biquad),
    /// The cabinet's horn, when it has one: its driver's signal (volts, the
    /// crossover's high-pass and the attenuator already applied, by `Chain`)
    /// through the band the horn passes, and the line every capsule reads it
    /// back from at its own distance.
    horn_band: (Biquad, Biquad),
    horn_line: DelayLine,
    voicing_norm: f64,
    calibration: f64,
    mics: [MicChannel; 2],
    placements: [MicPlacement; 2],
    blend: Ramped,
    pan: [Ramped; 2],
    invert: bool,
    align: bool,
    ramp_left: usize,
}

fn off_axis_corner(off_db: f64, cos_psi: f64) -> f64 {
    let weight = if cos_psi >= 0.0 {
        1.0 - cos_psi * cos_psi
    } else {
        1.0
    };
    let loss = off_db * weight;
    if loss < 1e-3 {
        f64::INFINITY
    } else {
        10_000.0 / (10f64.powf(loss / 10.0) - 1.0).sqrt()
    }
}

impl AcousticStage {
    pub fn new(rate: f64) -> Self {
        let mut s = Self {
            rate,
            cabinet: None,
            speaker: &SpeakerProfile::BRIT_V30,
            acceleration: DelayLine::new(),
            velocity: DelayLine::new(),
            integrate: OnePole::open(),
            breakup_acceleration: DelayLine::new(),
            breakup_velocity: DelayLine::new(),
            breakup_integrate: OnePole::open(),
            piston_band: OnePole::open(),
            cavity: CavityRadiation::default(),
            rear_acceleration: std::array::from_fn(|_| DelayLine::new()),
            rear_velocity: std::array::from_fn(|_| DelayLine::new()),
            rear_integrate: [OnePole::open(); RADIATORS],
            voicing: [Biquad::IDENTITY; 4],
            crossover: (OnePole::open(), Biquad::IDENTITY),
            horn_band: (Biquad::IDENTITY, Biquad::IDENTITY),
            horn_line: DelayLine::new(),
            voicing_norm: 1.0,
            calibration: 1.0,
            mics: [MicChannel::new(), MicChannel::new()],
            placements: [MicPlacement::default(); 2],
            blend: Ramped::default(),
            pan: [Ramped::default(); 2],
            invert: false,
            align: false,
            ramp_left: 0,
        };
        s.mics[0].slot = MicSlot::Profile(&MicProfile::DYNAMIC_57);
        s.rebuild();
        s
    }

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

    #[allow(clippy::too_many_arguments)]
    pub fn set_placement(
        &mut self,
        a: MicPlacement,
        b: MicPlacement,
        blend: f64,
        pan_a: f64,
        pan_b: f64,
        invert: bool,
        align: bool,
    ) {
        let finite = |x: f64, default: f64, low: f64, high: f64| {
            if x.is_finite() {
                x.clamp(low, high)
            } else {
                default
            }
        };
        let (a, b) = (a.clamped(), b.clamped());
        let blend = finite(blend, 0.5, 0.0, 1.0);
        let (pan_a, pan_b) = (finite(pan_a, 0.0, -1.0, 1.0), finite(pan_b, 0.0, -1.0, 1.0));
        if self.placements != [a, b]
            || self.invert != invert
            || self.align != align
            || self.blend.target != blend
            || self.pan[0].target != pan_a
            || self.pan[1].target != pan_b
        {
            self.placements = [a, b];
            self.invert = invert;
            self.align = align;
            self.blend.aim(blend, false);
            self.pan[0].aim(pan_a, false);
            self.pan[1].aim(pan_b, false);
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
        self.acceleration.reset();
        self.velocity.reset();
        self.integrate.reset();
        self.breakup_acceleration.reset();
        self.breakup_velocity.reset();
        self.breakup_integrate.reset();
        self.piston_band.reset();
        self.cavity.reset();
        for line in self
            .rear_acceleration
            .iter_mut()
            .chain(self.rear_velocity.iter_mut())
        {
            line.reset();
        }
        for integrator in &mut self.rear_integrate {
            integrator.reset();
        }
        for bq in &mut self.voicing {
            bq.reset();
        }
        self.crossover.0.reset();
        self.crossover.1.reset();
        self.horn_band.0.reset();
        self.horn_band.1.reset();
        self.horn_line.reset();
        for mic in &mut self.mics {
            mic.reset();
        }
        self.geometry(true);
        self.blend.aim(self.blend.target, true);
        for pan in &mut self.pan {
            pan.aim(pan.target, true);
        }
    }

    pub fn copy_runtime_state_from(&mut self, source: &Self) {
        // Derived Clone replaces boxes; waking a stereo channel must reuse its
        // preallocated pressure kernels instead of touching the allocator.
        macro_rules! copy { ($($field:ident),*) => { $(self.$field.clone_from(&source.$field);)* }; }
        copy!(
            rate,
            cabinet,
            speaker,
            acceleration,
            velocity,
            integrate,
            cavity,
            rear_acceleration,
            rear_velocity,
            rear_integrate,
            voicing,
            voicing_norm,
            calibration,
            placements,
            blend,
            pan,
            invert,
            align,
            ramp_left,
            breakup_acceleration,
            breakup_velocity,
            breakup_integrate,
            piston_band
        );
        for (dest, src) in self.mics.iter_mut().zip(&source.mics) {
            dest.field.copy_runtime_state_from(&src.field);
            dest.front_panel.copy_runtime_state_from(&src.front_panel);
            dest.breakup_paths = src.breakup_paths;
            dest.driver_count = src.driver_count;
            dest.slot = src.slot;
            dest.rear = src.rear;
            dest.rear_live = src.rear_live;
            dest.rear_live_len = src.rear_live_len;
            dest.off_axis = src.off_axis;
            dest.baffle = src.baffle;
            dest.eq = src.eq;
            dest.eq_norm = src.eq_norm;
            dest.delays = src.delays;
        }
    }
    pub fn relative_delays(&self, mic: usize) -> (f64, f64) {
        self.mics[mic].delays
    }

    fn rebuild(&mut self) {
        let rate = self.rate;
        let b = self.speaker.breakup;
        for (filter, peak) in self.voicing.iter_mut().zip(b.peaks.iter()) {
            filter.set_peaking(rate, peak.hz, peak.gain_db, peak.q);
        }
        self.voicing[3] = Biquad::lowpass(rate, b.lowpass_hz, b.lowpass_q);
        self.voicing_norm = 1.0
            / self
                .voicing
                .iter()
                .map(|b| b.magnitude(rate, 600.0))
                .product::<f64>();
        // Fixed gain calibrated at 5 cm in the low-frequency piston limit. It
        // never follows microphone distance, and is outside the pressure model.
        let a = self.speaker.radius();
        let reference = RHO * ((a * a + REFERENCE_DISTANCE.powi(2)).sqrt() - REFERENCE_DISTANCE);
        self.calibration =
            self.speaker.re * self.speaker.mms / self.speaker.bl / reference * self.voicing_norm;
        // Regularise only DC: 0.5 Hz is below every modelled capsule's passband.
        self.integrate.set_leaky_integrator(rate, 0.5, 1.0 / PI);
        self.breakup_integrate
            .set_leaky_integrator(rate, 0.5, 1.0 / PI);
        self.piston_band
            .set_lowpass(rate, self.speaker.breakup_transition_hz());
        for integrator in &mut self.rear_integrate {
            integrator.set_leaky_integrator(rate, 0.5, 1.0 / PI);
        }
        self.horn_band = match self.cabinet.and_then(|cab| cab.horn) {
            Some(horn) => (
                Biquad::highpass(rate, horn.low_hz, std::f64::consts::FRAC_1_SQRT_2),
                Biquad::lowpass(rate, horn.high_hz, std::f64::consts::FRAC_1_SQRT_2),
            ),
            None => (Biquad::IDENTITY, Biquad::IDENTITY),
        };
        self.crossover = match self.cabinet.and_then(|cab| cab.crossover_hz) {
            Some(hz) => {
                let mut pole = OnePole::open();
                pole.set_lowpass(rate, hz);
                (pole, Biquad::lowpass(rate, hz, 1.0))
            }
            None => (OnePole::open(), Biquad::IDENTITY),
        };
        self.cavity = CavityRadiation::default();
        if let Some(cab) = self.cabinet {
            self.cavity.configure(rate, cab, self.speaker);
        }
        for mic in &mut self.mics {
            mic.eq = [Biquad::IDENTITY; 6];
            mic.eq_norm = 1.0;
            if let MicSlot::Profile(p) = mic.slot {
                mic.eq[0] = Biquad::highpass(rate, p.highpass.0, p.highpass.1);
                for (b, peak) in mic.eq[1..5].iter_mut().zip(p.peaks.iter()) {
                    b.set_peaking(rate, peak.hz, peak.gain_db, peak.q);
                }
                mic.eq[5] = Biquad::lowpass(rate, p.lowpass.0, p.lowpass.1);
                mic.eq_norm = 1.0
                    / mic
                        .eq
                        .iter()
                        .map(|b| b.magnitude(rate, 1000.0))
                        .product::<f64>();
            }
        }
        self.geometry(true);
    }

    fn geometry(&mut self, snap: bool) {
        let radius = self.speaker.radius();
        let single = [(0.0, 0.0)];
        let drivers = self
            .cabinet
            .map(|c| c.driver_positions())
            .unwrap_or(&single);
        let target = drivers[0];
        let outward = if target.0 > 0.0 { 1.0 } else { -1.0 };
        let positions: [[f64; 3]; 2] = std::array::from_fn(|m| {
            let depth = match self.mics[m].slot {
                MicSlot::Profile(p) => p.capsule_depth,
                _ => 0.0,
            };
            [
                target.0 + outward * self.placements[m].position * radius,
                target.1,
                self.placements[m].distance + depth,
            ]
        });
        let minima: [f64; 2] = std::array::from_fn(|m| {
            if self.mics[m].slot == MicSlot::Off {
                return f64::INFINITY;
            }
            drivers
                .iter()
                .map(|&(x, y)| {
                    PressureField::first_arrival(
                        radius,
                        [positions[m][0] - x, positions[m][1] - y, positions[m][2]],
                    )
                })
                .fold(f64::INFINITY, f64::min)
        });
        let common = minima[0].min(minima[1]);
        for (m, mic) in self.mics.iter_mut().enumerate() {
            mic.field.begin();
            mic.front_panel.begin();

            if mic.slot == MicSlot::Off {
                mic.field.commit(snap);
                mic.front_panel.commit(snap);

                continue;
            }
            let profile = match mic.slot {
                MicSlot::Profile(p) => Some(p),
                _ => None,
            };
            let pattern = profile
                .map(|p| p.pattern)
                .unwrap_or(Pattern::Omni)
                .coefficients();
            let proximity = profile.map(|p| p.proximity).unwrap_or(0.0);
            let at = positions[m];
            let tilt = self.placements[m].angle.to_radians();
            let axis = [-outward * tilt.sin(), 0.0, -tilt.cos()];
            let base = if self.align { minima[m] } else { common };
            // The horn: a point source at its mouth, on the baffle, falling
            // off its axis as cos^coverage; pascals for a volt at its driver
            // from its 2.83 V sensitivity, spread as 1 / r.
            let (horn_gain, horn_delay) = match self.cabinet.and_then(|cab| cab.horn) {
                Some(horn) => {
                    let to = [at[0] - horn.position.0, at[1] - horn.position.1, at[2]];
                    let r = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2])
                        .sqrt()
                        .max(0.01);
                    let facing = (to[2] / r).max(0.0).powf(horn.coverage);
                    let per_volt = 20e-6 * 10f64.powf(horn.sensitivity_db / 20.0) / 2.83;
                    (
                        per_volt * facing / r,
                        (r - base).max(0.0) / SPEED_OF_SOUND * self.rate,
                    )
                }
                None => (0.0, 0.0),
            };
            mic.horn.gain.aim(horn_gain, snap);
            mic.horn.delay.aim(horn_delay, snap);
            let mut longest = minima[m];
            mic.driver_count = drivers.len();
            for (i, &(x, y)) in drivers.iter().enumerate() {
                let receiver = [at[0] - x, at[1] - y, at[2]];
                mic.field
                    .add_piston(self.rate, radius, receiver, axis, pattern, proximity, base);
                let lateral = receiver[0].hypot(receiver[1]);
                let centre = lateral.hypot(receiver[2]);
                let far = lateral > radius;
                let length = if far { centre } else { receiver[2] };
                let cosine = if far {
                    (-receiver[0] * axis[0] - receiver[1] * axis[1] - receiver[2] * axis[2])
                        / length
                } else {
                    -axis[2]
                };
                let sin_theta = lateral / centre;
                let aperture = if far {
                    radius
                } else {
                    radius * self.speaker.breakup_radius_ratio()
                };
                let corner = if sin_theta > 1e-6 {
                    2.2 * SPEED_OF_SOUND / (TAU * aperture * sin_theta)
                } else {
                    f64::INFINITY
                };
                let path = &mut mic.breakup_paths[i];
                path.far = far;
                path.delay
                    .aim((length - base).max(0.0) / SPEED_OF_SOUND * self.rate, snap);
                // Quasistatic piston pressure, in Pa/(m/s²), with the measured
                // breakup-band placement/directivity approximation retained.
                let gain = RHO * ((radius * radius + length * length).sqrt() - length);
                path.pressure
                    .aim(gain * (pattern.0 + pattern.1 * cosine), snap);
                path.velocity.aim(
                    gain * pattern.1 * cosine * proximity * SPEED_OF_SOUND
                        / (length * length + 0.49 * radius * radius).sqrt(),
                    snap,
                );
                path.directivity
                    .set_lowpass(self.rate, if far { f64::INFINITY } else { corner });
                if far {
                    for (filter, q) in path.beam.iter_mut().zip([0.541_196_1, 1.306_563]) {
                        filter.set_lowpass(self.rate, corner, q);
                    }
                }
                longest = longest.max(at[2].hypot(receiver[0].hypot(receiver[1]) + radius));
            }
            mic.field.commit(snap);

            mic.off_axis.set_lowpass(
                self.rate,
                off_axis_corner(profile.map(|p| p.off_axis_db).unwrap_or(0.0), tilt.cos()),
            );
            // Integrate the flexing baffle over its remaining wood, not a
            // monopole in the centre of a cone cutout. The fundamental simply
            // supported mode weights each element by cos(pi*x/w)cos(pi*y/h).
            if let Some(cab) = self.cabinet {
                let (w, h, _) = cab.internal();
                let mut patches = [([0.0; 3], 0.0); 24 * 24];
                let mut total = 0.0;
                for (i, (point, weight)) in patches.iter_mut().enumerate() {
                    let x = w * ((i % 24) as f64 + 0.5) / 24.0 - w / 2.0;
                    let y = h * ((i / 24) as f64 + 0.5) / 24.0 - h / 2.0;
                    *point = [at[0] - x, at[1] - y, at[2]];
                    if drivers
                        .iter()
                        .all(|&(dx, dy)| (x - dx).hypot(y - dy) >= radius)
                    {
                        *weight = (PI * x / w).cos() * (PI * y / h).cos();
                        total += *weight;
                    }
                }
                for (point, weight) in patches {
                    if weight > 0.0 {
                        mic.front_panel.add_volume_element(
                            self.rate,
                            point,
                            axis,
                            pattern,
                            proximity,
                            base,
                            weight / total,
                        );
                    }
                }
            }
            mic.front_panel.commit(snap);
            for source in 0..RADIATORS {
                let routes = self.cabinet.map(|cab| diffraction::paths(cab, source, at));
                for (route, path) in mic.rear[source * ROUTES..(source + 1) * ROUTES]
                    .iter_mut()
                    .enumerate()
                {
                    path.source = source;
                    let ray = routes.map(|r| r[route]).unwrap_or_default();
                    let distance = ray.distance.max(0.005);
                    let incidence = ray
                        .arrival
                        .iter()
                        .zip(axis)
                        .map(|(x, a)| x * a)
                        .sum::<f64>();
                    // Front-panel radiation is already surface-integrated.
                    let gain = if source == 1 { 0.0 } else { ray.weight };
                    let pressure = gain * RHO / (TAU * distance);
                    path.delay.aim(
                        (distance - base).max(0.0) / SPEED_OF_SOUND * self.rate,
                        snap,
                    );
                    path.pressure
                        .aim(pressure * (pattern.0 + pattern.1 * incidence), snap);
                    path.velocity.aim(
                        pressure * pattern.1 * incidence * proximity * SPEED_OF_SOUND / distance,
                        snap,
                    );
                    let corner = if ray.around > 1e-6 {
                        SPEED_OF_SOUND / (PI * ray.around)
                    } else {
                        f64::INFINITY
                    };
                    path.diffraction.set_lowpass(self.rate, corner);
                    if gain > 0.0 {
                        longest = longest.max(distance);
                    }
                }
            }
            mic.index_live_rear_paths();
            mic.delays = (
                (minima[m] - base) / SPEED_OF_SOUND * self.rate,
                (longest - base) / SPEED_OF_SOUND * self.rate,
            );
            // Finite-baffle low-band transition, retained as a documented
            // reduced approximation until measured baffle diffraction is available.
            if let Some(cab) = self.cabinet {
                let far =
                    self.placements[m].distance / (self.placements[m].distance + cab.width / 2.0);
                mic.baffle
                    .set_low_shelf(self.rate, cab.baffle_step_hz(), -6.0 * far);
            } else {
                mic.baffle = Biquad::IDENTITY;
            }
        }
        self.ramp_left = if snap { 0 } else { RAMP };
    }

    /// Physical pressure at the two capsules, including their linear response.
    /// Input is the solved cone acceleration in m/s², shared by equal drivers.
    /// Unmeasured family-based saturation has deliberately been removed.
    pub fn pressure(&mut self, acceleration: f64) -> [f64; 2] {
        self.pressure_with_horn(acceleration, 0.0)
    }

    /// The same, with the horn's driver at `horn` volts as well.
    pub fn pressure_with_horn(&mut self, acceleration: f64, horn: f64) -> [f64; 2] {
        let horn = self.horn_band.1.process(self.horn_band.0.process(horn));
        self.horn_line.write(horn);
        let acceleration = self
            .crossover
            .1
            .process(self.crossover.0.process(acceleration));
        let rear = self.cavity.process(acceleration);
        for (i, q) in rear.into_iter().enumerate() {
            self.rear_acceleration[i].write(q);
            self.rear_velocity[i].write(self.rear_integrate[i].process(q));
        }
        let mut a = acceleration;
        for b in &mut self.voicing {
            a = b.process(a);
        }
        // Peaking sections and the breakup low-pass have unity DC gain. Keep
        // that here: front and rear volume acceleration must balance at low
        // frequencies. The listening-level trim belongs after both fields.
        // Above breakup the cone no longer moves as one rigid disc. Preserve
        // the fitted volume response with a continuous transition to the
        // empirical breakup directivity; do not invent full-disc HF nulls.
        let piston = self.piston_band.process(a);
        let breakup = a - piston;
        self.acceleration.write(piston);
        self.velocity.write(self.integrate.process(piston));
        self.breakup_acceleration.write(breakup);
        self.breakup_velocity
            .write(self.breakup_integrate.process(breakup));
        let ramping = self.ramp_left > 0;
        let last = self.ramp_left == 1;
        if ramping {
            self.ramp_left -= 1;
            self.blend.advance(last);
            for pan in &mut self.pan {
                pan.advance(last);
            }
        }
        std::array::from_fn(|m| {
            let mic = &mut self.mics[m];
            if mic.slot == MicSlot::Off {
                return 0.0;
            }
            let mut p = mic.field.process(&self.acceleration, &self.velocity);
            p += mic
                .front_panel
                .process(&self.rear_acceleration[1], &self.rear_velocity[1]);
            for path in &mut mic.breakup_paths[..mic.driver_count] {
                if ramping {
                    path.delay.advance(last);
                    path.pressure.advance(last);
                    path.velocity.advance(last);
                }
                let tap = path.tap.follow(path.delay.now);
                let mut high = path.pressure.now * self.breakup_acceleration.read_tap(&tap)
                    + path.velocity.now * self.breakup_velocity.read_tap(&tap);
                high = path.directivity.process(high);
                if path.far {
                    for filter in &mut path.beam {
                        high = filter.process(high);
                    }
                }
                p += high;
            }
            if ramping {
                // Every path's ramp moves, listed or not: the next `aim`
                // measures its step from where `now` has got to.
                for path in &mut mic.rear {
                    path.delay.advance(last);
                    path.pressure.advance(last);
                    path.velocity.advance(last);
                }
            }
            for &k in &mic.rear_live[..mic.rear_live_len] {
                let path = &mut mic.rear[k as usize];
                let i = path.source;
                let tap = path.tap.follow(path.delay.now);
                let q = path.pressure.now * self.rear_acceleration[i].read_tap(&tap)
                    + path.velocity.now * self.rear_velocity[i].read_tap(&tap);
                p += path.diffraction.process(q);
            }
            if ramping {
                mic.horn.delay.advance(last);
                mic.horn.gain.advance(last);
            }
            if mic.horn.gain.now != 0.0 || mic.horn.gain.target != 0.0 {
                let tap = mic.horn.tap.follow(mic.horn.delay.now);
                p += mic.horn.gain.now * self.horn_line.read_tap(&tap);
            }
            p = mic.off_axis.process(mic.baffle.process(p));
            for eq in &mut mic.eq {
                p = eq.process(p);
            }
            p * mic.eq_norm
        })
    }

    pub fn process(&mut self, acceleration: f64) -> f64 {
        self.process_with_horn(acceleration, 0.0)
    }

    /// `process`, with the horn's driver at `horn` volts as well.
    pub fn process_with_horn(&mut self, acceleration: f64, horn: f64) -> f64 {
        let [a, b] = self.pressure_with_horn(acceleration, horn);
        let b = if self.invert { -b } else { b };
        self.calibration
            * match (
                self.mics[0].slot != MicSlot::Off,
                self.mics[1].slot != MicSlot::Off,
            ) {
                (true, true) => (1.0 - self.blend.now) * a + self.blend.now * b,
                (true, false) => a,
                (false, true) => b,
                (false, false) => 0.0,
            }
    }

    pub fn process_stereo(&mut self, acceleration: f64) -> (f64, f64) {
        self.process_stereo_with_horn(acceleration, 0.0)
    }

    /// `process_stereo`, with the horn's driver at `horn` volts as well.
    pub fn process_stereo_with_horn(&mut self, acceleration: f64, horn: f64) -> (f64, f64) {
        let [a, b] = self.pressure_with_horn(acceleration, horn);
        let b = if self.invert { -b } else { b };
        let (la, ra) = pan_gains(self.pan[0].now);
        let (lb, rb) = pan_gains(self.pan[1].now);
        let (left, right) = match (
            self.mics[0].slot != MicSlot::Off,
            self.mics[1].slot != MicSlot::Off,
        ) {
            (true, true) => {
                let a = (1.0 - self.blend.now) * a;
                let b = self.blend.now * b;
                (a * la + b * lb, a * ra + b * rb)
            }
            (true, false) => (a * la, a * ra),
            (false, true) => (b * lb, b * rb),
            (false, false) => (0.0, 0.0),
        };
        (left * self.calibration, right * self.calibration)
    }
}

#[inline]
fn pan_gains(pan: f64) -> (f64, f64) {
    ((1.0 - pan).clamp(0.0, 1.0), (1.0 + pan).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acoustics::mic::{MicPlacement, MicProfile};
    use crate::acoustics::speaker::SpeakerProfile;

    /// Skipping a rear path is exact only while it adds exactly nothing: both
    /// gains zero now and at the end of any ramp, and nothing ringing in its
    /// diffraction pole. Every cabinet, both capsules, placements from on the
    /// cone to a metre off and angled, moved while signal is flowing so ramps
    /// and filters are live: every path left off the list must still be that
    /// silent, and the listed ones must stay in their original order so the
    /// sum over them is the same sum.
    #[test]
    fn every_unlisted_rear_path_stays_exactly_silent() {
        let placements = [
            (0.0, 0.025, 0.0),
            (0.35, 0.05, 0.0),
            (1.0, 0.3, 30.0),
            (0.5, 1.0, 45.0),
        ];
        for cabinet in CabinetProfile::ALL {
            let mut stage = AcousticStage::new(48_000.0);
            stage.configure(
                Some(cabinet),
                &SpeakerProfile::BRIT_V30,
                MicSlot::Profile(&MicProfile::DYNAMIC_57),
                MicSlot::Ideal,
            );
            let mut k = 0usize;
            for &(position, distance, angle) in &placements {
                let a = MicPlacement {
                    position,
                    distance,
                    angle,
                };
                let b = MicPlacement {
                    position: 1.0 - position,
                    distance: distance * 2.0,
                    angle: 0.0,
                };
                stage.set_placement(a, b, 0.5, 0.0, 0.0, false, false);
                for _ in 0..3 * RAMP {
                    stage.process((k as f64 * 0.07).sin() * 50.0);
                    k += 1;
                }
                for mic in &stage.mics {
                    let listed = &mic.rear_live[..mic.rear_live_len];
                    assert!(listed.windows(2).all(|w| w[0] < w[1]));
                    for (index, path) in mic.rear.iter().enumerate() {
                        if !listed.contains(&(index as u8)) {
                            assert!(
                                !path.can_contribute(),
                                "{} path {index} is unlisted but live",
                                cabinet.id
                            );
                        }
                    }
                }
            }
        }
    }
}
