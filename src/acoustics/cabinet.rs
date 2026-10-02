//! Cabinet geometry, as immutable data. What the geometry *does* lives in
//! `acoustics::stage`; what the box does to the driver's load lives in
//! `acoustics::speaker::Mounting`. Sources and evidence labels:
//! `docs/models/cabinets.md`.

use super::speaker::{Mounting, SpeakerProfile, SPEED_OF_SOUND};

/// Effective isotropic panel properties. These are engineering estimates, not
/// measurements of the particular production cabinets or their joints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelMaterial {
    pub young: f64,
    pub density: f64,
    pub poisson: f64,
    pub loss_factor: f64,
}

impl PanelMaterial {
    pub const PLYWOOD: Self = Self {
        young: 12.4e9,
        density: 680.0,
        poisson: 0.3,
        loss_factor: 0.04,
    };
    pub const MDF: Self = Self {
        young: 3.0e9,
        density: 750.0,
        poisson: 0.3,
        loss_factor: 0.06,
    };
}

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
    pub material: PanelMaterial,
    /// Effective broadband energy absorption of the interior lining (0..1).
    /// Estimated where construction/absorption measurements are unavailable.
    pub lining_absorption: f64,
    pub drivers: usize,
    /// Driver centres on the baffle, m, from its centre: +x right, +y up.
    /// Room for eight; the entries past `drivers` are unused.
    pub positions: [(f64, f64); MAX_DRIVERS],
    /// Identical sealed chambers stacked up the box, each with an equal share
    /// of the drivers and the air. One everywhere but the American 8x10, whose
    /// eight drivers sit two to a chamber. See `acoustics::enclosure`.
    pub compartments: usize,
    /// Fraction of the back that is open. Zero is sealed.
    pub open_fraction: f64,
    /// Volume lost to an angled top, as a fraction of the straight box.
    pub slant: f64,
    pub leakage_q: f64,
    /// Ports in the baffle, for a vented box. `None` is sealed (or open-backed,
    /// by `open_fraction`); a cabinet has one or the other.
    pub vent: Option<Vent>,
    /// The woofers' passive crossover, a third-order low-pass at this corner,
    /// for a cabinet built with one. `None` drives them full range.
    pub crossover_hz: Option<f64>,
    /// A horn tweeter behind the crossover's high-pass, for a cabinet with
    /// one. `None` is woofers alone.
    pub horn: Option<Horn>,
    /// What `Matched` resolves the speaker to. See the research log for which of
    /// these are factory complements and which are voicing choices.
    pub default_speaker: &'static SpeakerProfile,
}

/// Ports in a vented cabinet's baffle: `count` tubes of `area` each and
/// `length` deep, opening on the front about `position` (m from the baffle's
/// centre, +x right, +y up). Together they are one acoustic mass on the box,
/// `rho L / (count area)` with each tube's ends corrected, and they radiate
/// from where they are. See `acoustics::enclosure`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vent {
    pub count: usize,
    pub area: f64,
    pub length: f64,
    pub position: (f64, f64),
}

impl Vent {
    /// The tubes' effective length: the physical one plus an end correction
    /// for the outer end, flanged by the baffle (0.85 r), and the inner, free
    /// in the box (0.61 r).
    pub fn effective_length(&self) -> f64 {
        let r = (self.area / std::f64::consts::PI).sqrt();
        self.length + (0.85 + 0.61) * r
    }
}

/// A cabinet's horn tweeter: where its mouth is on the baffle (m from the
/// centre), how loud it is for 2.83 V at a metre on its axis, the band its
/// driver and flare pass, and how fast it falls off its axis (`cos^coverage`).
/// It is fed from the speaker's terminals through the crossover's high-pass,
/// third order at `CabinetProfile::crossover_hz`, and the cabinet's
/// attenuator; see `acoustics::stage` and `Chain`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Horn {
    pub position: (f64, f64),
    pub sensitivity_db: f64,
    pub low_hz: f64,
    pub high_hz: f64,
    pub coverage: f64,
}

/// The most drivers a cabinet here holds: the American 8x10's.
pub const MAX_DRIVERS: usize = 8;

/// Four positions, and the rest unused.
const fn four(p: [(f64, f64); 4]) -> [(f64, f64); MAX_DRIVERS] {
    [
        p[0],
        p[1],
        p[2],
        p[3],
        (0.0, 0.0),
        (0.0, 0.0),
        (0.0, 0.0),
        (0.0, 0.0),
    ]
}

const FOUR: [(f64, f64); MAX_DRIVERS] = four([
    (-0.170, 0.168),
    (0.170, 0.168),
    (-0.170, -0.168),
    (0.170, -0.168),
]);

impl CabinetProfile {
    pub const BRIT_1960: CabinetProfile = CabinetProfile {
        id: "cab_marshall_1960a",
        name: "Brit 1960 4x12",
        inspiration: "Marshall 1960A angled 4x12, G12T-75",
        width: 0.770,
        height: 0.755,
        depth: 0.365,
        wall: 0.0159,
        material: PanelMaterial::PLYWOOD,
        lining_absorption: 0.08,
        drivers: 4,
        compartments: 1,
        positions: FOUR,
        open_fraction: 0.0,
        slant: 0.08,
        leakage_q: 7.0,
        vent: None,
        crossover_hz: None,
        horn: None,
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
        material: PanelMaterial::PLYWOOD,
        lining_absorption: 0.08,
        drivers: 4,
        compartments: 1,
        positions: four([
            (-0.168, 0.180),
            (0.168, 0.180),
            (-0.168, -0.180),
            (0.168, -0.180),
        ]),
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        vent: None,
        crossover_hz: None,
        horn: None,
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
        vent: None,
        crossover_hz: None,
        horn: None,
        default_speaker: &SpeakerProfile::BRIT_GREEN_25,
        ..Self::BRIT_1960
    };

    pub const BRIT_V30: CabinetProfile = CabinetProfile {
        id: "cab_marshall_1960av",
        name: "Brit V30 4x12",
        inspiration: "Marshall 1960AV angled 4x12, Celestion G12 Vintage",
        vent: None,
        crossover_hz: None,
        horn: None,
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
        material: PanelMaterial::PLYWOOD,
        lining_absorption: 0.08,
        drivers: 4,
        compartments: 1,
        positions: four([
            (-0.172, 0.185),
            (0.172, 0.185),
            (-0.172, -0.185),
            (0.172, -0.185),
        ]),
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        vent: None,
        crossover_hz: None,
        horn: None,
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
        material: PanelMaterial::PLYWOOD,
        lining_absorption: 0.08,
        drivers: 2,
        compartments: 1,
        positions: four([(-0.151, -0.030), (0.151, -0.030), (0.0, 0.0), (0.0, 0.0)]),
        open_fraction: 0.40,
        slant: 0.0,
        leakage_q: 7.0,
        vent: None,
        crossover_hz: None,
        horn: None,
        default_speaker: &SpeakerProfile::AMERICAN_CERAMIC,
    };

    /// WIDELY REPORTED: a blackface Deluxe Reverb of this period shipped with a
    /// Jensen C12N or an Oxford 12K5 depending on date and order, so `Matched`
    /// resolves to the same ceramic Jensen the Twin's cabinet does rather than
    /// to the alnico P12R it used to. The '65 reissue's C12K is a later part
    /// and is deliberately not substituted for either.
    /// See `docs/models/american_deluxe.md`.
    pub const AMERICAN_OPEN_112: CabinetProfile = CabinetProfile {
        id: "cab_fender_deluxe_open_112",
        name: "American Open 1x12",
        inspiration: "Fender '65 Deluxe Reverb open-back 1x12 combo cabinet",
        width: 0.622,
        height: 0.445,
        depth: 0.241,
        wall: 0.019,
        material: PanelMaterial::PLYWOOD,
        lining_absorption: 0.08,
        drivers: 1,
        compartments: 1,
        positions: four([(0.0, -0.020), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)]),
        open_fraction: 0.45,
        slant: 0.0,
        leakage_q: 7.0,
        vent: None,
        crossover_hz: None,
        horn: None,
        default_speaker: &SpeakerProfile::AMERICAN_CERAMIC,
    };

    pub const CLOSED_112: CabinetProfile = CabinetProfile {
        id: "cab_marshall_1912",
        name: "Closed 1x12",
        inspiration: "Marshall 1912 closed-back 1x12",
        width: 0.500,
        height: 0.470,
        depth: 0.290,
        wall: 0.0159,
        material: PanelMaterial::PLYWOOD,
        lining_absorption: 0.08,
        drivers: 1,
        compartments: 1,
        positions: [(0.0, 0.0); MAX_DRIVERS],
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        vent: None,
        crossover_hz: None,
        horn: None,
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
        material: PanelMaterial::PLYWOOD,
        lining_absorption: 0.08,
        drivers: 2,
        compartments: 1,
        positions: four([(-0.167, 0.0), (0.167, 0.0), (0.0, 0.0), (0.0, 0.0)]),
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        vent: None,
        crossover_hz: None,
        horn: None,
        default_speaker: &SpeakerProfile::BRIT_T75,
    };

    /// The Roland JC-120's own cabinet: a shallow open-back 2x12 combo.
    ///
    /// DOCUMENTED, Roland's JC-120/JC-160 service notes (fifth edition): 750 W
    /// x 540 H x 270 D mm without casters, two 30 cm speakers (30-103D, part
    /// 041-019), one backboard (089-070). The 2000 JC-120UT/JT notes give
    /// 760 x 622 x 280 mm, which is the same box standing on its casters.
    /// Open back: WIDELY REPORTED by owners, including one who documented
    /// closing it; how much of it the backboard covers is ESTIMATED, as for the
    /// Fender combos. Panel thickness ESTIMATED (a stapled MDF-backed box, per a
    /// 1982 repair write-up). Driver centres DERIVED as for the other cabinets --
    /// equal gaps across the internal width from a 283 mm cutout -- and lowered
    /// by half the control strip across the top of the front, ESTIMATED.
    /// See `docs/models/cabinets.md`.
    pub const JAZZ_OPEN_212: CabinetProfile = CabinetProfile {
        id: "cab_roland_jc120_212",
        name: "Jazz Open 2x12",
        inspiration: "Roland JC-120 Jazz Chorus open-back 2x12 combo cabinet",
        width: 0.750,
        height: 0.540,
        depth: 0.270,
        wall: 0.018,
        material: PanelMaterial::MDF,
        lining_absorption: 0.08,
        drivers: 2,
        compartments: 1,
        positions: four([(-0.166, -0.040), (0.166, -0.040), (0.0, 0.0), (0.0, 0.0)]),
        open_fraction: 0.40,
        slant: 0.0,
        leakage_q: 7.0,
        vent: None,
        crossover_hz: None,
        horn: None,
        default_speaker: &SpeakerProfile::JAZZ_12,
    };

    /// A late-80s Peavey 4x12: the 412M, straight-fronted and closed-backed,
    /// as it shipped with Celestion G12K-85s. The cabinet on the *Machine Rage*
    /// records is "a 1987 Peavey 4x12 cabinet with Celestion G12K-85 speakers"
    /// (WIDELY REPORTED); which Peavey model is not stated, and the 412M and
    /// its slant 412MS are the ones of those years that carried G12K-85s
    /// (PLAUSIBLE). Closed back DOCUMENTED (Peavey's 412M/412MS sheet).
    /// 30.125 W x 32.125 H x 14.25 D inches is an owner's measurement of a
    /// 412M (PLAUSIBLE), and Peavey's published 4x12 shells of later years are
    /// the same size to within an eighth of an inch. Panel thickness ESTIMATED
    /// (the sheet's "7 ply plywood", a 3/4 in board). Driver centres DERIVED as
    /// for the other 4x12s: equal gaps round a 283 mm cutout. See
    /// `docs/models/cabinets.md`.
    pub const AMERICAN_CLOSED_412: CabinetProfile = CabinetProfile {
        id: "cab_peavey_412m",
        name: "American Closed 4x12",
        inspiration: "Peavey 412M straight closed-back 4x12 (late 1980s), G12K-85",
        width: 0.765,
        height: 0.816,
        depth: 0.362,
        wall: 0.019,
        material: PanelMaterial::PLYWOOD,
        lining_absorption: 0.08,
        drivers: 4,
        compartments: 1,
        positions: four([
            (-0.168, 0.177),
            (0.168, 0.177),
            (-0.168, -0.177),
            (0.168, -0.177),
        ]),
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        vent: None,
        crossover_hz: None,
        horn: None,
        default_speaker: &SpeakerProfile::BRIT_K85,
    };

    /// The Ampeg SVT-810E: eight tens in four sealed chambers of two, the SVT's
    /// own cabinet. 660.4 W x 1219 H x 406.4 D mm, DOCUMENTED (Ampeg's owner's
    /// manual). The chambers are WIDELY REPORTED -- "4 sealed 16 ohm 2x10 cabs
    /// stuck together" -- and agree with the drawing's "32 ohm speakers (8) all
    /// in parallel" (Ampeg D 591719): two 32 ohm drivers to a chamber are
    /// 16 ohm, all eight 4 ohm. Panels and dividers 19 mm, ESTIMATED. Driver
    /// centres DERIVED: equal gaps across the 622 mm inside round the B810's
    /// 232 mm cutout, and the four 281 mm chambers' centres up the box. See
    /// `docs/models/cabinets.md`.
    pub const AMERICAN_810: CabinetProfile = CabinetProfile {
        id: "cab_ampeg_svt_810e",
        name: "American 8x10",
        inspiration: "Ampeg SVT-810E sealed 8x10, four chambers of two, 32 ohm tens",
        width: 0.6604,
        height: 1.219,
        depth: 0.4064,
        wall: 0.019,
        material: PanelMaterial::PLYWOOD,
        lining_absorption: 0.08,
        drivers: 8,
        compartments: 4,
        positions: [
            (-0.142, 0.450),
            (0.142, 0.450),
            (-0.142, 0.150),
            (0.142, 0.150),
            (-0.142, -0.150),
            (0.142, -0.150),
            (-0.142, -0.450),
            (0.142, -0.450),
        ],
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        vent: None,
        crossover_hz: None,
        horn: None,
        default_speaker: &SpeakerProfile::AMERICAN_BASS_10,
    };

    /// Gallien-Krueger 410RBH, the Eminence-built RBH of GK's owner's manual
    /// (1999): 24 W x 27.5 H x 18 D in, 3/4 in birch with dado joints, four
    /// P10/200 tens, two front vents and an 18 dB/oct passive crossover, all
    /// DOCUMENTED; one chamber (none is described). ESTIMATED: the layout --
    /// the horn's space at the top, the tens in a square below it, the vents
    /// along the bottom; the vents' size and depth, two 3 in tubes 52 mm deep
    /// tuning the box to 42 Hz, about the drivers' own resonance -- with the
    /// estimated drivers the flattest of the tunings that reach lowest
    /// (`examples/american_410_op.rs`); the crossover's corner, 3 kHz; and the
    /// horn -- the P508, of which nothing is published -- 102 dB for 2.83 V at
    /// a metre, passing 2 to 13 kHz (the manual's "usable response" ends at
    /// 13), falling off its axis as cos^2 (-6 dB at 45 degrees, a 90 degree
    /// horn). Its level is the cabinet's attenuator, the panel's Horn knob.
    pub const AMERICAN_410: CabinetProfile = CabinetProfile {
        id: "cab_gk_410rbh",
        name: "American 4x10",
        inspiration: "Gallien-Krueger 410RBH (RBH series), four 32 ohm tens, two front vents",
        width: 0.6096,
        height: 0.6985,
        depth: 0.4572,
        wall: 0.019,
        material: PanelMaterial::PLYWOOD,
        lining_absorption: 0.08,
        drivers: 4,
        compartments: 1,
        positions: four([
            (-0.145, 0.105),
            (0.145, 0.105),
            (-0.145, -0.155),
            (0.145, -0.155),
        ]),
        open_fraction: 0.0,
        slant: 0.0,
        leakage_q: 7.0,
        vent: Some(Vent {
            count: 2,
            area: 45.6e-4,
            length: 0.052,
            position: (0.0, -0.305),
        }),
        crossover_hz: Some(3_000.0),
        horn: Some(Horn {
            position: (0.0, 0.27),
            sensitivity_db: 102.0,
            low_hz: 2_000.0,
            high_hz: 13_000.0,
            coverage: 2.0,
        }),
        default_speaker: &SpeakerProfile::CAST_BASS_10,
    };

    pub const ALL: [&'static CabinetProfile; 14] = [
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
        &Self::JAZZ_OPEN_212,
        &Self::AMERICAN_CLOSED_412,
        &Self::AMERICAN_810,
        &Self::AMERICAN_410,
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

    /// Air volume behind all drivers, m^3, every chamber together.
    pub fn volume(&self) -> f64 {
        let (w, h, d) = self.internal();
        let dividers = (self.compartments.max(1) - 1) as f64 * w * d * self.wall;
        (w * h * d * (1.0 - self.slant) * (1.0 - BRACING)
            - DRIVER_DISPLACEMENT * self.drivers as f64
            - dividers)
            .max(0.005)
    }

    /// One chamber's inside height, m: the inside less the dividers, shared.
    pub fn compartment_height(&self) -> f64 {
        let (_, h, _) = self.internal();
        let n = self.compartments.max(1) as f64;
        (h - (n - 1.0) * self.wall) / n
    }

    /// The cavity and opening load every driver continuously, including open backs.
    pub fn mounting(&self) -> Mounting {
        Mounting {
            cabinet: Some(*self),
            volume_per_driver: Some(self.volume() / self.drivers.max(1) as f64),
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
        let rigidity = self.material.young * t.powi(3)
            / (12.0 * (1.0 - self.material.poisson * self.material.poisson));
        let surface = self.material.density * t;
        std::f64::consts::FRAC_PI_2 * (rigidity / surface).sqrt() * (1.0 / (w * w) + 1.0 / (h * h))
    }

    /// Where a baffle of this width stops holding the radiation to a half space.
    pub fn baffle_step_hz(&self) -> f64 {
        SPEED_OF_SOUND / (std::f64::consts::PI * self.width)
    }

    pub fn driver_positions(&self) -> &[(f64, f64)] {
        &self.positions[..self.drivers.clamp(1, MAX_DRIVERS)]
    }
}
