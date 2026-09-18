//! Microphones as immutable data: on-axis response, polar pattern, off-axis
//! behaviour, proximity strength and transducer family. The processor that places
//! them in front of a cabinet is `acoustics::stage`. Sources, fits and evidence:
//! `docs/models/microphones.md`.

use super::speaker::Peak;

/// First-order directional pattern, `P(psi) = a + b cos(psi)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Omni,
    Cardioid,
    Supercardioid,
    Hypercardioid,
    Figure8,
}

impl Pattern {
    /// `(a, b)`. The gradient term keeps its sign, so a figure-8's rear lobe is
    /// inverted as it is on the hardware.
    pub const fn coefficients(self) -> (f64, f64) {
        match self {
            Pattern::Omni => (1.0, 0.0),
            Pattern::Cardioid => (0.5, 0.5),
            Pattern::Supercardioid => (0.366, 0.634),
            Pattern::Hypercardioid => (0.25, 0.75),
            Pattern::Figure8 => (0.0, 1.0),
        }
    }

    /// Signed sensitivity at incidence angle `psi` (radians).
    pub fn gain(self, psi: f64) -> f64 {
        let (a, b) = self.coefficients();
        a + b * psi.cos()
    }
}

/// What does the transducing, for the subtle nonlinearity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Dynamic,
    Ribbon,
    FetCondenser,
    TubeCondenser,
}

impl Family {
    /// Cubic soft-saturation coefficient at 0 dBFS. TUNED to be subtle: under 0.1 %
    /// third harmonic at nominal level for every family. Not a measurement.
    pub const fn cubic(self) -> f64 {
        match self {
            Family::Dynamic => 0.004,
            Family::Ribbon => 0.006,
            Family::FetCondenser => 0.002,
            Family::TubeCondenser => 0.005,
        }
    }

    /// Asymmetric (second-order) coefficient, only the tube family has one.
    pub const fn quadratic(self) -> f64 {
        match self {
            Family::TubeCondenser => 0.004,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MicProfile {
    /// Stable identifier. Never rename.
    pub id: &'static str,
    pub name: &'static str,
    pub inspiration: &'static str,
    pub pattern: Pattern,
    pub family: Family,
    /// On-axis response, fitted: second-order high-pass, four peaks, second-order low-pass.
    pub highpass: (f64, f64),
    pub peaks: [Peak; 4],
    pub lowpass: (f64, f64),
    /// Extra loss at 90 degrees incidence and 10 kHz, dB: how much darker than the
    /// ideal pattern the capsule gets off axis.
    pub off_axis_db: f64,
    /// Proximity strength relative to an ideal point-source gradient transducer.
    pub proximity: f64,
    /// Distance from the front of the grille to the diaphragm, m.
    pub capsule_depth: f64,
}

const fn pk(hz: f64, gain_db: f64, q: f64) -> Peak {
    Peak { hz, gain_db, q }
}

impl MicProfile {
    pub const DYNAMIC_57: MicProfile = MicProfile {
        id: "mic_shure_sm57",
        name: "Dynamic 57",
        inspiration: "Shure SM57",
        pattern: Pattern::Cardioid,
        family: Family::Dynamic,
        highpass: (64.0, 0.460),
        peaks: [
            pk(490.0, -1.35, 1.63),
            pk(4302.0, 2.40, 1.19),
            pk(5620.0, 2.00, 2.81),
            pk(11129.0, 1.76, 3.75),
        ],
        lowpass: (8789.0, 1.269),
        off_axis_db: 4.0,
        proximity: 0.18,
        capsule_depth: 0.012,
    };

    pub const DYNAMIC_421: MicProfile = MicProfile {
        id: "mic_sennheiser_md421",
        name: "Dynamic 421",
        inspiration: "Sennheiser MD 421 II (bass switch M)",
        pattern: Pattern::Cardioid,
        family: Family::Dynamic,
        highpass: (19.2, 0.343),
        peaks: [
            pk(168.0, -2.88, 0.40),
            pk(3714.0, 2.19, 0.61),
            pk(4920.0, 1.67, 1.48),
            pk(9542.0, -0.80, 3.55),
        ],
        lowpass: (10800.0, 1.169),
        off_axis_db: 5.0,
        proximity: 0.25,
        capsule_depth: 0.020,
    };

    pub const DYNAMIC_906: MicProfile = MicProfile {
        id: "mic_sennheiser_e906",
        name: "Dynamic 906",
        inspiration: "Sennheiser e 906 (flat presence position)",
        pattern: Pattern::Supercardioid,
        family: Family::Dynamic,
        highpass: (67.6, 0.461),
        peaks: [
            pk(968.0, -0.26, 3.46),
            pk(3536.0, 2.52, 1.02),
            pk(10385.0, 2.37, 1.72),
            pk(10385.0, 2.37, 1.72),
        ],
        lowpass: (6254.0, 1.114),
        off_axis_db: 3.0,
        proximity: 0.25,
        capsule_depth: 0.010,
    };

    pub const DYNAMIC_409: MicProfile = MicProfile {
        id: "mic_sennheiser_md409",
        name: "Dynamic 409",
        inspiration: "Sennheiser MD 409 U3 (1986 data sheet)",
        pattern: Pattern::Cardioid,
        family: Family::Dynamic,
        highpass: (47.5, 0.300),
        peaks: [
            pk(103.0, -1.06, 1.13),
            pk(3748.0, 0.61, 3.15),
            pk(12094.0, 4.34, 1.84),
            pk(12106.0, 4.33, 1.85),
        ],
        lowpass: (7901.0, 0.777),
        off_axis_db: 4.0,
        proximity: 0.22,
        capsule_depth: 0.010,
    };

    pub const RIBBON_121: MicProfile = MicProfile {
        id: "mic_royer_r121",
        name: "Ribbon 121",
        inspiration: "Royer R-121",
        pattern: Pattern::Figure8,
        family: Family::Ribbon,
        highpass: (17.5, 0.528),
        peaks: [
            pk(60.0, 1.85, 1.12),
            pk(3435.0, 0.52, 0.54),
            pk(3568.0, 0.57, 0.55),
            pk(14960.0, -1.07, 2.28),
        ],
        lowpass: (39870.0, 0.454),
        off_axis_db: 0.5,
        proximity: 1.0,
        capsule_depth: 0.012,
    };

    pub const RIBBON_160: MicProfile = MicProfile {
        id: "mic_beyer_m160",
        name: "Ribbon 160",
        inspiration: "beyerdynamic M 160",
        pattern: Pattern::Hypercardioid,
        family: Family::Ribbon,
        highpass: (81.6, 0.495),
        peaks: [
            pk(183.0, 2.63, 1.06),
            pk(4086.0, 1.53, 0.60),
            pk(6787.0, 1.17, 1.90),
            pk(12352.0, -0.84, 2.84),
        ],
        lowpass: (17406.0, 0.768),
        off_axis_db: 2.0,
        proximity: 1.2,
        capsule_depth: 0.015,
    };

    pub const RIBBON_38: MicProfile = MicProfile {
        id: "mic_coles_4038",
        name: "Ribbon 38",
        inspiration: "Coles 4038",
        pattern: Pattern::Figure8,
        family: Family::Ribbon,
        highpass: (27.6, 0.798),
        peaks: [
            pk(125.0, 0.37, 0.40),
            pk(3178.0, 0.40, 0.68),
            pk(9646.0, -0.19, 8.00),
            pk(12057.0, 0.37, 6.23),
        ],
        lowpass: (16653.0, 0.723),
        off_axis_db: 1.0,
        proximity: 1.0,
        capsule_depth: 0.030,
    };

    pub const CONDENSER_87: MicProfile = MicProfile {
        id: "mic_neumann_u87ai",
        name: "Condenser 87",
        inspiration: "Neumann U 87 Ai, cardioid",
        pattern: Pattern::Cardioid,
        family: Family::FetCondenser,
        highpass: (7.6, 0.300),
        peaks: [
            pk(69.0, 0.39, 1.14),
            pk(4783.0, -0.38, 1.45),
            pk(9215.0, 0.76, 2.24),
            pk(9216.0, 0.76, 2.24),
        ],
        lowpass: (14432.0, 0.861),
        off_axis_db: 6.0,
        proximity: 0.5,
        capsule_depth: 0.030,
    };

    pub const CONDENSER_414: MicProfile = MicProfile {
        id: "mic_akg_c414",
        name: "Condenser 414",
        inspiration: "AKG C414 (XLS-like flat curve, APPROXIMATED)",
        pattern: Pattern::Cardioid,
        family: Family::FetCondenser,
        highpass: (16.5, 0.655),
        peaks: [
            pk(60.0, 0.15, 0.97),
            pk(2741.0, 0.18, 1.00),
            pk(11414.0, 0.26, 3.60),
            pk(11856.0, 0.29, 5.78),
        ],
        lowpass: (15475.0, 1.018),
        off_axis_db: 4.0,
        proximity: 0.5,
        capsule_depth: 0.025,
    };

    pub const TUBE_CONDENSER_67: MicProfile = MicProfile {
        id: "mic_neumann_u67",
        name: "Tube Condenser 67",
        inspiration: "Neumann U 67 (APPROXIMATED response)",
        pattern: Pattern::Cardioid,
        family: Family::TubeCondenser,
        highpass: (8.6, 0.300),
        peaks: [
            pk(113.0, 0.23, 2.09),
            pk(5840.0, 0.39, 0.51),
            pk(8516.0, 0.46, 3.49),
            pk(15183.0, 0.38, 7.56),
        ],
        lowpass: (12711.0, 1.156),
        off_axis_db: 6.0,
        proximity: 0.5,
        capsule_depth: 0.030,
    };

    pub const FET_CONDENSER_47: MicProfile = MicProfile {
        id: "mic_neumann_u47fet",
        name: "FET Condenser 47",
        inspiration: "Neumann U 47 fet (APPROXIMATED response)",
        pattern: Pattern::Cardioid,
        family: Family::FetCondenser,
        highpass: (32.8, 0.663),
        peaks: [
            pk(121.0, 0.53, 0.40),
            pk(7621.0, -0.72, 1.21),
            pk(13464.0, 1.10, 2.99),
            pk(15111.0, 1.03, 3.63),
        ],
        lowpass: (10637.0, 1.274),
        off_axis_db: 6.0,
        proximity: 0.5,
        capsule_depth: 0.030,
    };

    pub const ALL: [&'static MicProfile; 11] = [
        &Self::DYNAMIC_57,
        &Self::DYNAMIC_421,
        &Self::DYNAMIC_906,
        &Self::DYNAMIC_409,
        &Self::RIBBON_121,
        &Self::RIBBON_160,
        &Self::RIBBON_38,
        &Self::CONDENSER_87,
        &Self::CONDENSER_414,
        &Self::TUBE_CONDENSER_67,
        &Self::FET_CONDENSER_47,
    ];

    /// The one-pole corner giving `off_axis_db` of extra loss at 10 kHz and 90 degrees.
    pub fn off_axis_corner_at_90(&self) -> f64 {
        let ratio = 10f64.powf(self.off_axis_db.max(0.01) / 10.0) - 1.0;
        10_000.0 / ratio.sqrt()
    }
}

/// Where a microphone is, as the panel describes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MicPlacement {
    /// 0 is the dust cap, 1 the cone's edge.
    pub position: f64,
    /// From the grille cloth to the front of the microphone, m.
    pub distance: f64,
    /// Tilt away from the cone's axis, degrees. 0 is on axis.
    pub angle: f64,
}

impl MicPlacement {
    pub const MIN_DISTANCE: f64 = 0.01;
    pub const MAX_DISTANCE: f64 = 1.0;

    pub fn clamped(self) -> Self {
        Self {
            position: if self.position.is_finite() {
                self.position.clamp(0.0, 1.0)
            } else {
                0.0
            },
            distance: if self.distance.is_finite() {
                self.distance.clamp(Self::MIN_DISTANCE, Self::MAX_DISTANCE)
            } else {
                Self::MIN_DISTANCE
            },
            angle: if self.angle.is_finite() {
                self.angle.clamp(0.0, 90.0)
            } else {
                0.0
            },
        }
    }
}

impl Default for MicPlacement {
    fn default() -> Self {
        Self {
            position: 0.3,
            distance: 0.025,
            angle: 0.0,
        }
    }
}
