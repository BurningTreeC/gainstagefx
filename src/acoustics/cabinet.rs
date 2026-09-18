//! Cabinet geometry, as immutable data. What the geometry *does* lives in
//! `acoustics::stage`; what the box does to the driver's load lives in
//! `acoustics::speaker::Mounting`. Sources and evidence labels:
//! `docs/models/cabinets.md`.

use super::speaker::{Mounting, SpeakerProfile, SPEED_OF_SOUND};

/// Birch plywood, for the panel resonance. ESTIMATED material constants.
const PLY_YOUNG: f64 = 12.4e9;
const PLY_DENSITY: f64 = 680.0;
const PLY_POISSON: f64 = 0.3;

/// Displacement of one driver's basket and magnet inside the box, m^3. ESTIMATED.
const DRIVER_DISPLACEMENT: f64 = 2.5e-3;
/// Fraction of the internal volume taken by bracing and cleats. ESTIMATED.
const BRACING: f64 = 0.03;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CabinetProfile {
    /// Stable identifier. Never rename.
    pub id: &'static str,
    pub name: &'static str,
    pub inspiration: &'static str,
    /// External dimensions, m.
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    /// Panel thickness, m.
    pub wall: f64,
    pub drivers: usize,
    /// Driver centres on the baffle, m, from its centre: +x right, +y up.
    pub positions: [(f64, f64); 4],
    /// Fraction of the back that is open. Zero is sealed.
    pub open_fraction: f64,
    /// Volume lost to an angled top, as a fraction of the straight box.
    pub slant: f64,
    pub leakage_q: f64,
    /// What `Matched` resolves the speaker to. See the research log for which of
    /// these are factory complements and which are voicing choices.
    pub default_speaker: &'static SpeakerProfile,
}

const FOUR: [(f64, f64); 4] = [
    (-0.170, 0.168),
    (0.170, 0.168),
    (-0.170, -0.168),
    (0.170, -0.168),
];

impl CabinetProfile {
    pub const BRIT_1960: CabinetProfile = CabinetProfile {
        id: "cab_marshall_1960a",
        name: "Brit 1960 4x12",
        inspiration: "Marshall 1960A angled 4x12, G12T-75",
        width: 0.770,
        height: 0.755,
        depth: 0.365,
        wall: 0.0159,
        drivers: 4,
        positions: FOUR,
        open_fraction: 0.0,
        slant: 0.08,
        leakage_q: 7.0,
        default_speaker: &SpeakerProfile::BRIT_T75,
    };

    pub const CALI_OVERSIZED: CabinetProfile = CabinetProfile {
        id: "cab_mesa_recto_standard",
        name: "Cali Oversized 4x12",
        inspiration: "Mesa/Boogie Rectifier Standard (oversized) 4x12, Vintage 30",
        width: 0.765,
        height: 0.836,
        depth: 0.362,
        wall: 0.019,
        drivers: 4,
        positions: [
            (-0.168, 0.180),
            (0.168, 0.180),
            (-0.168, -0.180),
            (0.168, -0.180),
        ],
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        default_speaker: &SpeakerProfile::BRIT_V30,
    };

    pub const BRIT_CLOSED: CabinetProfile = CabinetProfile {
        id: "cab_marshall_1960b",
        name: "Brit Closed 4x12",
        inspiration: "Marshall 1960B straight 4x12, G12T-75",
        slant: 0.0,
        ..Self::BRIT_1960
    };

    pub const BRIT_GREEN: CabinetProfile = CabinetProfile {
        id: "cab_marshall_1960ax",
        name: "Brit Green 4x12",
        inspiration: "Marshall 1960AX angled 4x12, G12M-25 Greenback",
        default_speaker: &SpeakerProfile::BRIT_GREEN_25,
        ..Self::BRIT_1960
    };

    pub const BRIT_V30: CabinetProfile = CabinetProfile {
        id: "cab_marshall_1960av",
        name: "Brit V30 4x12",
        inspiration: "Marshall 1960AV angled 4x12, Celestion G12 Vintage",
        default_speaker: &SpeakerProfile::BRIT_V30,
        ..Self::BRIT_1960
    };

    pub const OVERSIZED: CabinetProfile = CabinetProfile {
        id: "cab_generic_oversized_412",
        name: "Oversized 4x12",
        inspiration: "Generic oversized closed 4x12 (no hardware reference)",
        width: 0.800,
        height: 0.850,
        depth: 0.380,
        wall: 0.018,
        drivers: 4,
        positions: [
            (-0.172, 0.185),
            (0.172, 0.185),
            (-0.172, -0.185),
            (0.172, -0.185),
        ],
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        default_speaker: &SpeakerProfile::BRIT_T75,
    };

    pub const AMERICAN_OPEN_212: CabinetProfile = CabinetProfile {
        id: "cab_fender_twin_open_212",
        name: "American Open 2x12",
        inspiration: "Fender Twin Reverb (AB763-style) open-back 2x12 combo cabinet",
        width: 0.664,
        height: 0.508,
        depth: 0.267,
        wall: 0.019,
        drivers: 2,
        positions: [(-0.151, -0.030), (0.151, -0.030), (0.0, 0.0), (0.0, 0.0)],
        open_fraction: 0.40,
        slant: 0.0,
        leakage_q: 7.0,
        default_speaker: &SpeakerProfile::AMERICAN_CERAMIC,
    };

    pub const AMERICAN_OPEN_112: CabinetProfile = CabinetProfile {
        id: "cab_fender_deluxe_open_112",
        name: "American Open 1x12",
        inspiration: "Fender '65 Deluxe Reverb open-back 1x12 combo cabinet",
        width: 0.622,
        height: 0.445,
        depth: 0.241,
        wall: 0.019,
        drivers: 1,
        positions: [(0.0, -0.020), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        open_fraction: 0.45,
        slant: 0.0,
        leakage_q: 7.0,
        default_speaker: &SpeakerProfile::AMERICAN_VINTAGE_12,
    };

    pub const CLOSED_112: CabinetProfile = CabinetProfile {
        id: "cab_marshall_1912",
        name: "Closed 1x12",
        inspiration: "Marshall 1912 closed-back 1x12",
        width: 0.500,
        height: 0.470,
        depth: 0.290,
        wall: 0.0159,
        drivers: 1,
        positions: [(0.0, 0.0); 4],
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        default_speaker: &SpeakerProfile::BRIT_V30,
    };

    pub const CLOSED_212: CabinetProfile = CabinetProfile {
        id: "cab_marshall_1936",
        name: "Closed 2x12",
        inspiration: "Marshall 1936 closed-back 2x12",
        width: 0.750,
        height: 0.600,
        depth: 0.310,
        wall: 0.0159,
        drivers: 2,
        positions: [(-0.167, 0.0), (0.167, 0.0), (0.0, 0.0), (0.0, 0.0)],
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        default_speaker: &SpeakerProfile::BRIT_T75,
    };

    pub const ALL: [&'static CabinetProfile; 10] = [
        &Self::BRIT_1960,
        &Self::CALI_OVERSIZED,
        &Self::BRIT_CLOSED,
        &Self::BRIT_GREEN,
        &Self::BRIT_V30,
        &Self::OVERSIZED,
        &Self::AMERICAN_OPEN_212,
        &Self::AMERICAN_OPEN_112,
        &Self::CLOSED_112,
        &Self::CLOSED_212,
    ];

    pub fn internal(&self) -> (f64, f64, f64) {
        (
            self.width - 2.0 * self.wall,
            self.height - 2.0 * self.wall,
            self.depth - 2.0 * self.wall,
        )
    }

    pub fn is_open(&self) -> bool {
        self.open_fraction > 0.0
    }

    /// Air volume behind all drivers, m^3.
    pub fn volume(&self) -> f64 {
        let (w, h, d) = self.internal();
        (w * h * d * (1.0 - self.slant) * (1.0 - BRACING)
            - DRIVER_DISPLACEMENT * self.drivers as f64)
            .max(0.005)
    }

    /// How the box loads each driver's cone. An open back has no air spring.
    pub fn mounting(&self) -> Mounting {
        Mounting {
            volume_per_driver: (!self.is_open())
                .then(|| self.volume() / self.drivers.max(1) as f64),
            leakage_q: self.leakage_q,
            drivers: self.drivers,
        }
    }

    /// Axial standing waves inside the box, `c / 2L`: depth, width, height.
    pub fn modes(&self) -> [f64; 3] {
        let (w, h, d) = self.internal();
        [
            SPEED_OF_SOUND / (2.0 * d),
            SPEED_OF_SOUND / (2.0 * w),
            SPEED_OF_SOUND / (2.0 * h),
        ]
    }

    /// Fundamental of the back panel as a simply supported plate.
    pub fn panel_hz(&self) -> f64 {
        let (w, h, _) = self.internal();
        let t = self.wall;
        let rigidity = PLY_YOUNG * t.powi(3) / (12.0 * (1.0 - PLY_POISSON * PLY_POISSON));
        let surface = PLY_DENSITY * t;
        std::f64::consts::FRAC_PI_2 * (rigidity / surface).sqrt() * (1.0 / (w * w) + 1.0 / (h * h))
    }

    /// Where a baffle of this width stops holding the radiation to a half space.
    pub fn baffle_step_hz(&self) -> f64 {
        SPEED_OF_SOUND / (std::f64::consts::PI * self.width)
    }

    pub fn driver_positions(&self) -> &[(f64, f64)] {
        &self.positions[..self.drivers.clamp(1, 4)]
    }
}
