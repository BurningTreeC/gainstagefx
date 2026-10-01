//! Passive reduced enclosure impedance, referred to one of N equally driven cones.
//!
//! The uniform cavity compliance, opening inertia and back-panel mobility share
//! one pressure. Higher rectangular modes add positive-real mechanical impedances
//! with source-position-dependent residues. Every term is realized as passive RLC
//! parts in the amplifier solve, so reflected pressure actually loads the cone.
//! This common-motion reduction assumes identical, in-phase drivers. Slanted
//! cabinets use a volume-equivalent rectangular cavity; joints and plywood
//! anisotropy require measurements beyond the current profiles.
//!
//! A cabinet of several identical sealed chambers (`CabinetProfile::compartments`)
//! is one chamber's modes and panels, counted as many times: with every cone in
//! phase every chamber is at the same pressure, so the shared compliance is the
//! whole volume against all the drivers, exactly as for one box, and only what
//! depends on a chamber's size -- its standing waves, its panels -- changes.

use super::cabinet::CabinetProfile;
use super::speaker::{SpeakerProfile, RHO, SPEED_OF_SOUND};
use std::f64::consts::{PI, TAU};

pub const MODE_COUNT: usize = 8;
/// Front, back, paired sides, paired top/bottom.
pub const PANEL_GROUPS: usize = 4;
/// Opening and six independently located panel radiators.
pub const RADIATORS: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelMode {
    pub area: f64,
    pub mass: f64,
    pub hz: f64,
    pub loss: f64,
    pub count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mode {
    pub hz: f64,
    /// Mechanical impedance K*s/(s*s + damping*s + omega*omega).
    pub stiffness: f64,
    pub damping: f64,
}

impl Mode {
    const EMPTY: Self = Self {
        hz: 1000.0,
        stiffness: 0.0,
        damping: 100.0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Enclosure {
    pub volume: f64,
    pub opening_area: f64,
    pub opening_length: f64,
    pub panels: [PanelMode; PANEL_GROUPS],
    pub modes: [Mode; MODE_COUNT],
}

/// Area average of cos(k dot x) over a circular piston, 2 J1(ka)/(ka).
pub fn piston_average(x: f64) -> f64 {
    // The enclosure's retained modes have ka < 8. The convergent Bessel series
    // avoids a platform-dependent special-function dependency.
    let mut sum = 1.0;
    let mut term = 1.0;
    for n in 1..40 {
        term *= -0.25 * x * x / (n * (n + 1)) as f64;
        sum += term;
        if term.abs() < 1e-15 {
            break;
        }
    }
    sum
}

impl Enclosure {
    pub fn new(cab: &CabinetProfile, speaker: &SpeakerProfile) -> Self {
        let (w, whole, _) = cab.internal();
        // One chamber: its inside height, its share of the air and of the drivers.
        // For a one-chamber cabinet each of these is exactly the whole.
        let n = cab.compartments.max(1);
        let h = cab.compartment_height();
        let volume = cab.volume();
        let chamber = volume / n as f64;
        let d = volume / (w * h * n as f64);
        // An open back, or a vented box's ports: either way an opening onto
        // the air outside, through an acoustic mass.
        let (opening_area, opening_length) = match cab.vent {
            Some(vent) => (vent.count as f64 * vent.area, vent.effective_length()),
            None => {
                let area = w * whole * cab.open_fraction.clamp(0.0, 1.0);
                // Two unflanged ends: total low-frequency end correction 1.22*r.
                (area, cab.wall + 1.22 * (area / PI).sqrt())
            }
        };
        let vent_area = cab.vent.map_or(0.0, |v| v.count as f64 * v.area);
        let chamber_opening = opening_area / n as f64;
        let centre = -whole / 2.0 + h / 2.0;
        let in_chamber: [bool; super::cabinet::MAX_DRIVERS] = std::array::from_fn(|i| {
            cab.driver_positions()
                .get(i)
                .is_some_and(|&(_, y)| (y - centre).abs() < h / 2.0)
        });
        let chamber_drivers = in_chamber.iter().filter(|&&b| b).count().max(1);
        let material = cab.material;
        let panel = |width: f64, height: f64, remaining: f64, count: usize| {
            let rigidity =
                material.young * cab.wall.powi(3) / (12.0 * (1.0 - material.poisson.powi(2)));
            PanelMode {
                area: 4.0 * width * height / (PI * PI) * remaining,
                mass: material.density * cab.wall * width * height / 4.0 * remaining.max(0.01),
                hz: PI / 2.0
                    * (rigidity / (material.density * cab.wall)).sqrt()
                    * (width.powi(-2) + height.powi(-2)),
                loss: material.loss_factor,
                count,
            }
        };
        // Perforated baffle and partial back use remaining modal area/mass.
        // Their detailed brace/cutout boundary conditions remain approximate.
        let front_fraction = (1.0
            - (chamber_drivers as f64 * speaker.sd + vent_area / n as f64) / (w * h))
            .clamp(0.05, 1.0);
        let panels = [
            panel(w, h, front_fraction, n),
            panel(w, h, 1.0 - cab.open_fraction, n),
            panel(d, h, 1.0, 2 * n),
            panel(w, d, 1.0, 2),
        ];
        let mut result = Self {
            volume,
            opening_area,
            opening_length,
            panels,
            modes: [Mode::EMPTY; MODE_COUNT],
        };
        let mut candidates = [Mode::EMPTY; 26];
        let mut count = 0;
        for nx in 0..=2 {
            for ny in 0..=2 {
                for nz in 0..=2 {
                    if nx + ny + nz == 0 {
                        continue;
                    }
                    let kx = PI * nx as f64 / w;
                    let ky = PI * ny as f64 / h;
                    let kz = PI * nz as f64 / d;
                    let omega = SPEED_OF_SOUND * (kx * kx + ky * ky + kz * kz).sqrt();
                    let aperture = piston_average(kx.hypot(ky) * speaker.radius());
                    let mean_shape = cab
                        .driver_positions()
                        .iter()
                        .zip(in_chamber)
                        .filter(|&(_, inside)| inside)
                        .map(|(&(x, y), _)| {
                            (kx * (x + w / 2.0)).cos()
                                * (ky * (y - centre + h / 2.0)).cos()
                                * aperture
                        })
                        .sum::<f64>()
                        / chamber_drivers as f64;
                    let norm =
                        chamber / 2f64.powi((nx > 0) as i32 + (ny > 0) as i32 + (nz > 0) as i32);
                    let stiffness = RHO
                        * SPEED_OF_SOUND.powi(2)
                        * speaker.sd.powi(2)
                        * chamber_drivers as f64
                        * mean_shape.powi(2)
                        / norm;
                    if stiffness < 1e-8 {
                        continue;
                    }
                    let surface = 2.0 * (w * h + w * d + h * d);
                    // Normal-incidence absorption of an equivalent elastic wall.
                    // Lining and the open area remove energy on each encounter.
                    let wall_mass = material.density * cab.wall;
                    let wall_omega = TAU * result.panels[1].hz;
                    let zr = wall_mass * wall_omega * material.loss_factor;
                    let zi = wall_mass * (omega - wall_omega.powi(2) / omega);
                    let zair = RHO * SPEED_OF_SOUND;
                    let absorption = 4.0 * zr * zair / ((zr + zair).powi(2) + zi * zi);
                    let lining = cab.lining_absorption.clamp(0.0, 0.99);
                    let absorbing_area = (surface - chamber_opening)
                        * (lining + (1.0 - lining) * absorption)
                        + chamber_opening;
                    let damping = SPEED_OF_SOUND * absorbing_area / (4.0 * chamber);
                    candidates[count] = Mode {
                        hz: omega / TAU,
                        stiffness,
                        damping: damping.max(1.0),
                    };
                    count += 1;
                }
            }
        }
        candidates[..count].sort_unstable_by(|a, b| a.hz.total_cmp(&b.hz));
        let retained = count.min(MODE_COUNT);
        result.modes[..retained].copy_from_slice(&candidates[..retained]);
        result
    }
}

/// Radiation through the opening and all six walls. The pressure solve is the
/// same passive network stamped into the power amp. Trapezoidal companion forms
/// reduce its coupled solve to one scalar division per sample, not a dense matrix.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Mobility {
    conductance: f64,
    history_gain: f64,
    stiffness: f64,
    flow: f64,
    displacement: f64,
}

impl Mobility {
    fn new(mass: f64, resistance: f64, stiffness: f64, half_step: f64) -> Self {
        Self {
            conductance: 1.0 / (mass / half_step + resistance + stiffness * half_step),
            history_gain: mass / half_step - resistance - stiffness * half_step,
            stiffness,
            ..Self::default()
        }
    }
    fn history(&self, pressure: f64) -> f64 {
        self.conductance
            * (pressure + self.history_gain * self.flow - 2.0 * self.stiffness * self.displacement)
    }
    fn update(&mut self, pressure: f64, history: f64, half_step: f64) -> f64 {
        let flow = self.conductance * pressure + history;
        self.displacement += half_step * (flow + self.flow);
        self.flow = flow;
        flow
    }
    fn reset(&mut self) {
        self.flow = 0.0;
        self.displacement = 0.0;
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CavityRadiation {
    mobilities: [Mobility; 1 + PANEL_GROUPS],
    compliance: f64,
    leakage: f64,
    half_step: f64,
    source_area: f64,
    pressure: f64,
    previous_input: f64,
}

impl CavityRadiation {
    pub fn configure(&mut self, rate: f64, cab: &CabinetProfile, speaker: &SpeakerProfile) {
        let e = Enclosure::new(cab, speaker);
        let values = super::speaker::LoadValues::new(speaker, &cab.mounting(), 1.0);
        let reference = speaker.bl.powi(2) / (cab.drivers as f64 * speaker.sd.powi(2));
        self.half_step = 0.5 / rate;
        self.compliance = e.volume / (RHO * SPEED_OF_SOUND.powi(2));
        self.leakage = values.rbox / reference;
        self.source_area = cab.drivers as f64 * speaker.sd;
        self.mobilities = [Mobility::default(); 1 + PANEL_GROUPS];
        if e.opening_area > 0.0 {
            self.mobilities[0] = Mobility::new(
                RHO * e.opening_length / e.opening_area,
                reference / values.opening_r,
                0.0,
                self.half_step,
            );
        }
        for (i, panel) in e.panels.iter().enumerate() {
            if panel.area > 0.0 {
                let mass = panel.mass / (panel.count as f64 * panel.area.powi(2));
                let omega = TAU * panel.hz;
                self.mobilities[i + 1] = Mobility::new(
                    mass,
                    panel.loss * omega * mass,
                    omega.powi(2) * mass,
                    self.half_step,
                );
            }
        }
        self.reset();
    }
    pub fn reset(&mut self) {
        for m in &mut self.mobilities {
            m.reset();
        }
        self.pressure = 0.0;
        self.previous_input = 0.0;
    }
    pub fn process(&mut self, acceleration: f64) -> [f64; RADIATORS] {
        if self.source_area == 0.0 {
            return [0.0; RADIATORS];
        }
        let history: [f64; 1 + PANEL_GROUPS] =
            std::array::from_fn(|i| self.mobilities[i].history(self.pressure));
        let source = -self.source_area * acceleration;
        let conductance = self.compliance / self.half_step;
        let denominator =
            conductance + self.leakage + self.mobilities.iter().map(|m| m.conductance).sum::<f64>();
        let past_flows = self
            .mobilities
            .iter()
            .zip(history)
            .map(|(m, h)| m.flow + h)
            .sum::<f64>();
        self.pressure =
            (source + self.previous_input + (conductance - self.leakage) * self.pressure
                - past_flows)
                / denominator;
        self.previous_input = source;
        let flow: [f64; 1 + PANEL_GROUPS] = std::array::from_fn(|i| {
            self.mobilities[i].update(self.pressure, history[i], self.half_step)
        });
        [
            flow[0],
            flow[1],
            flow[2],
            flow[3] / 2.0,
            flow[3] / 2.0,
            flow[4] / 2.0,
            flow[4] / 2.0,
        ]
    }
}
