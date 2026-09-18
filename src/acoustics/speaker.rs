//! A loudspeaker as the load a power amplifier actually drives.
//!
//! Every power stage in this plugin used to end in a resistor. A speaker is not
//! one: its voice coil is a resistance and a lossy inductance, and behind it the
//! cone, its suspension and the air it pushes are a mass, a spring and a damper,
//! which the magnet reflects back into the electrical side as a parallel
//! resonant circuit. The impedance therefore peaks at the resonance, sits near
//! the coil resistance in the middle of the band and climbs with frequency. A
//! valve amplifier with modest feedback has a high output impedance, so that
//! curve reaches the output voltage, the current the tubes are asked for and
//! the feedback loop. A post-EQ cannot do that, and this module does not try.
//!
//! ```text
//!  spk --Re-- vc1 --(L1 || R1)-- vc2 --(L2 || R2)-- mot --+-- Res --+
//!                                                          +-- Lces -+
//!                                                          +-- Cmes -+-- gnd
//!                                                          +-- Lbox -- box -- Rbox -+
//! ```
//!
//! `Res = Bl^2 / Rms`, `Lces = Bl^2 Cms`, `Cmes = Mms / Bl^2`, `Lbox = Bl^2 Cab`:
//! the standard impedance-analogue of the driver's mechanical impedance, derived
//! from `F = Bl i` and `V_mot = Bl u`. Cone velocity is `u = V(mot) / Bl`.
//! `L1 || R1` and `L2 || R2` are the voice coil as two lossy inductances (the
//! eddy-current losses of the pole piece), fitted to Jensen's measured impedance
//! curves. A pure `Le` rises without limit; this levels off above the band,
//! which is what the measurements show and what keeps a feedback amplifier
//! driving it stable at every sample rate. See the research log.
//!
//! Every element is an adjustable netlist part, so one power netlist serves every
//! speaker and cabinet. See `docs/models/speakers.md` for sources and evidence
//! labels, and `CABINET_MODEL.md` for the enclosure terms.

use crate::dsp::complex::C;
use crate::dsp::netlist::{Adjust, Circuit, Fault, Netlist};
use crate::dsp::time::Simulation;

/// Density of air at 20 C, kg/m^3.
pub const RHO: f64 = 1.204;
/// Speed of sound at 20 C, m/s.
pub const SPEED_OF_SOUND: f64 = 343.0;

/// The node the driver's terminal is on, and its motional node.
pub const MOTIONAL: &str = "spk_mot";

/// One peaking section of the cone's breakup voicing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peak {
    pub hz: f64,
    pub gain_db: f64,
    pub q: f64,
}

/// What the cone does that a piston does not: breakup resonances and the fall
/// past them. EMPIRICALLY TUNED to the manufacturer's on-axis SPL plot, after the
/// lumped piston model's own response was removed from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breakup {
    pub peaks: [Peak; 3],
    pub lowpass_hz: f64,
    pub lowpass_q: f64,
}

/// Immutable description of one driver. Values are 8 ohm data; see the research log.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeakerProfile {
    /// Stable identifier. Never rename.
    pub id: &'static str,
    /// Product label.
    pub name: &'static str,
    /// Precise engineering reference.
    pub inspiration: &'static str,
    /// Voice coil DC resistance, ohms.
    pub re: f64,
    /// The coil's two lossy inductances, each with its loss resistance across it.
    pub l1: f64,
    pub r1: f64,
    pub l2: f64,
    pub r2: f64,
    /// Free-air resonance, Hz.
    pub fs: f64,
    pub qms: f64,
    /// Moving mass including air load, kg.
    pub mms: f64,
    /// Force factor, T m.
    pub bl: f64,
    /// Effective piston area, m^2.
    pub sd: f64,
    pub breakup: Breakup,
}

const fn pk(hz: f64, gain_db: f64, q: f64) -> Peak {
    Peak { hz, gain_db, q }
}

impl SpeakerProfile {
    /// Celestion Vintage 30. Re and Fs published; Bl, Mms, Qms and coil ESTIMATED.
    pub const BRIT_V30: SpeakerProfile = SpeakerProfile {
        id: "spk_celestion_v30",
        name: "Brit V30",
        inspiration: "Celestion Vintage 30, 8 ohm (current UK production)",
        re: 7.3,
        l1: 0.44804e-3,
        r1: 44.048,
        l2: 1.22136e-3,
        r2: 6.1927,
        fs: 75.0,
        qms: 9.73,
        mms: 28.0e-3,
        bl: 14.706,
        sd: 490.9e-4,
        breakup: Breakup {
            peaks: [
                pk(897.0, 4.17, 1.94),
                pk(2454.0, 10.62, 1.26),
                pk(3871.0, 11.82, 2.44),
            ],
            lowpass_hz: 5128.0,
            lowpass_q: 0.50,
        },
    };

    /// Celestion G12M-25 Greenback. Re and Fs published; the rest ESTIMATED.
    pub const BRIT_GREEN_25: SpeakerProfile = SpeakerProfile {
        id: "spk_celestion_g12m25",
        name: "Brit Green 25",
        inspiration: "Celestion G12M-25 Greenback, 8 ohm (current Classic series)",
        re: 6.7,
        l1: 0.44804e-3,
        r1: 44.048,
        l2: 1.22136e-3,
        r2: 6.1927,
        fs: 75.0,
        qms: 9.73,
        mms: 28.0e-3,
        bl: 9.902,
        sd: 490.9e-4,
        breakup: Breakup {
            peaks: [
                pk(986.0, 4.51, 2.52),
                pk(2489.0, 12.0, 1.22),
                pk(3811.0, 12.0, 2.42),
            ],
            lowpass_hz: 5478.0,
            lowpass_q: 0.50,
        },
    };

    /// Celestion G12T-75. Re and Fs published; the rest ESTIMATED.
    pub const BRIT_T75: SpeakerProfile = SpeakerProfile {
        id: "spk_celestion_g12t75",
        name: "Brit T75",
        inspiration: "Celestion G12T-75, 8 ohm (current Classic series)",
        re: 6.77,
        l1: 0.44804e-3,
        r1: 44.048,
        l2: 1.22136e-3,
        r2: 6.1927,
        fs: 85.0,
        qms: 9.73,
        mms: 28.0e-3,
        bl: 10.874,
        sd: 490.9e-4,
        breakup: Breakup {
            peaks: [
                pk(1266.0, -5.07, 3.53),
                pk(2205.0, 12.0, 0.99),
                pk(3510.0, 9.25, 1.45),
            ],
            lowpass_hz: 7263.0,
            lowpass_q: 0.50,
        },
    };

    /// Jensen P12R reissue. PUBLISHED T/S; coil FITTED to the measured curve.
    pub const AMERICAN_VINTAGE_12: SpeakerProfile = SpeakerProfile {
        id: "spk_jensen_p12r",
        name: "American Vintage 12",
        inspiration: "Jensen P12R (Vintage Alnico reissue), 8 ohm",
        re: 6.05,
        l1: 0.2882e-3,
        r1: 36.81,
        l2: 0.8318e-3,
        r2: 4.278,
        fs: 80.0,
        qms: 13.07,
        mms: 25.2e-3,
        bl: 5.53,
        sd: 490.9e-4,
        breakup: Breakup {
            peaks: [
                pk(2011.0, 9.06, 0.88),
                pk(2011.0, 9.06, 0.88),
                pk(3734.0, 7.49, 1.64),
            ],
            lowpass_hz: 7304.0,
            lowpass_q: 1.22,
        },
    };

    /// Jensen P10R reissue. PUBLISHED T/S; coil FITTED.
    pub const AMERICAN_VINTAGE_10: SpeakerProfile = SpeakerProfile {
        id: "spk_jensen_p10r",
        name: "American Vintage 10",
        inspiration: "Jensen P10R (Vintage Alnico reissue), 8 ohm",
        re: 6.68,
        l1: 0.2985e-3,
        r1: 35.77,
        l2: 0.6741e-3,
        r2: 4.398,
        fs: 97.0,
        qms: 14.83,
        mms: 15.2e-3,
        bl: 5.83,
        sd: 330.1e-4,
        breakup: Breakup {
            peaks: [
                pk(1058.0, 5.48, 2.22),
                pk(2526.0, 11.32, 0.91),
                pk(4093.0, 12.0, 1.07),
            ],
            lowpass_hz: 7553.0,
            lowpass_q: 1.20,
        },
    };

    /// Jensen C12N reissue. PUBLISHED T/S; coil FITTED.
    pub const AMERICAN_CERAMIC: SpeakerProfile = SpeakerProfile {
        id: "spk_jensen_c12n",
        name: "American Ceramic",
        inspiration: "Jensen C12N (Vintage Ceramic reissue), 8 ohm",
        re: 6.05,
        l1: 0.4634e-3,
        r1: 46.77,
        l2: 1.4303e-3,
        r2: 6.671,
        fs: 113.0,
        qms: 7.52,
        mms: 29.9e-3,
        bl: 10.46,
        sd: 490.9e-4,
        breakup: Breakup {
            peaks: [
                pk(1108.0, 8.94, 1.81),
                pk(2092.0, 12.0, 2.12),
                pk(3497.0, 10.45, 1.27),
            ],
            lowpass_hz: 5829.0,
            lowpass_q: 2.13,
        },
    };

    /// Jensen P12N reissue. PUBLISHED T/S; coil FITTED.
    pub const AMERICAN_ALNICO: SpeakerProfile = SpeakerProfile {
        id: "spk_jensen_p12n",
        name: "American Alnico",
        inspiration: "Jensen P12N (Vintage Alnico reissue), 8 ohm",
        re: 6.03,
        l1: 0.4327e-3,
        r1: 41.33,
        l2: 1.0125e-3,
        r2: 5.714,
        fs: 90.0,
        qms: 4.36,
        mms: 30.9e-3,
        bl: 10.62,
        sd: 490.9e-4,
        breakup: Breakup {
            peaks: [
                pk(1006.0, 8.69, 2.44),
                pk(2436.0, 10.7, 1.71),
                pk(3632.0, 12.0, 1.45),
            ],
            lowpass_hz: 6156.0,
            lowpass_q: 1.54,
        },
    };

    /// Every implemented driver, in the order stable ids were assigned.
    pub const ALL: [&'static SpeakerProfile; 7] = [
        &Self::BRIT_V30,
        &Self::BRIT_GREEN_25,
        &Self::BRIT_T75,
        &Self::AMERICAN_VINTAGE_12,
        &Self::AMERICAN_VINTAGE_10,
        &Self::AMERICAN_CERAMIC,
        &Self::AMERICAN_ALNICO,
    ];

    /// The data sets are 8 ohm parts.
    pub const NOMINAL_OHMS: f64 = 8.0;

    pub fn cms(&self) -> f64 {
        let w = std::f64::consts::TAU * self.fs;
        1.0 / (w * w * self.mms)
    }

    pub fn rms(&self) -> f64 {
        std::f64::consts::TAU * self.fs * self.mms / self.qms
    }

    pub fn qes(&self) -> f64 {
        std::f64::consts::TAU * self.fs * self.mms * self.re / (self.bl * self.bl)
    }

    /// Equivalent piston radius, m.
    pub fn radius(&self) -> f64 {
        (self.sd / std::f64::consts::PI).sqrt()
    }

    /// Low-frequency radiation mass of one side of a baffled piston, `8 rho a^3 / 3`.
    pub fn side_air_mass(&self) -> f64 {
        8.0 * RHO * self.radius().powi(3) / 3.0
    }
}

/// How a cabinet loads each of its drivers. PHYSICS DERIVED approximations; see
/// `CABINET_MODEL.md`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mounting {
    /// Sealed air volume behind each driver, m^3. `None` is an open back or a baffle.
    pub volume_per_driver: Option<f64>,
    /// Q of the box's leakage and absorption, at the loaded resonance.
    pub leakage_q: f64,
    /// Drivers radiating side by side. Mutual coupling adds air mass to each.
    pub drivers: usize,
}

impl Mounting {
    /// One driver on an open baffle: the free-air parameters as published.
    pub const BAFFLE: Mounting = Mounting {
        volume_per_driver: None,
        leakage_q: 7.0,
        drivers: 1,
    };
}

/// Inductance standing for "no box": its companion conductance is negligible
/// against the motional branch at every rate this plugin runs at.
const OPEN_BOX_HENRY: f64 = 1.0e4;

/// The element values actually stamped, already referred to the amplifier's tap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoadValues {
    pub re: f64,
    pub l1: f64,
    pub r1: f64,
    pub l2: f64,
    pub r2: f64,
    pub res: f64,
    pub lces: f64,
    pub cmes: f64,
    pub lbox: f64,
    pub rbox: f64,
    /// Bl referred to the tap, so `u = V(mot) / bl`.
    pub bl: f64,
}

impl LoadValues {
    /// `scale` multiplies every impedance: `PowerSpec::speaker / 8` behind a power
    /// stage, one for a driver on its own. See "Referencing" in the research log.
    pub fn new(profile: &SpeakerProfile, mounting: &Mounting, scale: f64) -> Self {
        let k = scale.max(1e-6);
        let bl2 = profile.bl * profile.bl;
        let drivers = mounting.drivers.max(1) as f64;
        // Closely packed drivers share radiation mass: the array radiates like one
        // piston of N times the area, whose air mass split N ways is sqrt(N) of one.
        let mms = profile.mms + profile.side_air_mass() * (drivers.sqrt() - 1.0);
        let cms = profile.cms();
        let (lbox, rbox) = match mounting.volume_per_driver {
            Some(volume) if volume > 0.0 => {
                let cab =
                    volume / (RHO * SPEED_OF_SOUND * SPEED_OF_SOUND * profile.sd * profile.sd);
                let lbox = k * bl2 * cab;
                // Series loss giving the stated Q at the loaded resonance.
                let ctotal = cms * cab / (cms + cab);
                let wc = 1.0 / (mms * ctotal).sqrt();
                (lbox, wc * lbox / mounting.leakage_q.max(0.5))
            }
            _ => (OPEN_BOX_HENRY, 1.0),
        };
        Self {
            re: k * profile.re,
            l1: k * profile.l1,
            r1: k * profile.r1,
            l2: k * profile.l2,
            r2: k * profile.r2,
            res: k * bl2 / profile.rms(),
            lces: k * bl2 * cms,
            cmes: mms / (k * bl2),
            lbox,
            rbox,
            bl: profile.bl * k.sqrt(),
        }
    }

    /// The electrical impedance at the terminals, analytically.
    pub fn impedance(&self, hz: f64) -> C {
        let s = C::new(0.0, std::f64::consts::TAU * hz);
        let lossy = |l: f64, r: f64| {
            let sl = s * C::real(l);
            sl * C::real(r) / (C::real(r) + sl)
        };
        let coil = C::real(self.re) + lossy(self.l1, self.r1) + lossy(self.l2, self.r2);
        let admittance = C::real(1.0 / self.res)
            + (s * C::real(self.lces)).recip()
            + s * C::real(self.cmes)
            + (s * C::real(self.lbox) + C::real(self.rbox)).recip();
        coil + admittance.recip()
    }

    /// Multiply `dV(mot)/dt` by this to get the on-axis pressure normalised to one
    /// volt at the terminals in the mass-controlled band: `Re Mms / Bl^2`.
    pub fn pressure_scale(&self) -> f64 {
        self.re * self.cmes
    }
}

/// Where each element's value lives in a simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadSlots {
    re: usize,
    l1: usize,
    r1: usize,
    l2: usize,
    r2: usize,
    res: usize,
    lces: usize,
    cmes: usize,
    lbox: usize,
    rbox: usize,
}

impl LoadSlots {
    /// Install new element values. Marks the simulation for one rebuild; reactive
    /// state is carried across it.
    pub fn apply(&self, sim: &mut Simulation, v: &LoadValues) {
        for (slot, value) in [
            (self.re, v.re),
            (self.l1, v.l1),
            (self.r1, v.r1),
            (self.l2, v.l2),
            (self.r2, v.r2),
            (self.res, v.res),
            (self.lces, v.lces),
            (self.cmes, v.cmes),
            (self.lbox, v.lbox),
            (self.rbox, v.rbox),
        ] {
            sim.set_value(slot, value);
        }
    }
}

/// Stamp the driver between `terminal` and ground.
pub fn stamp(net: &mut Netlist, terminal: &str, v: &LoadValues) -> LoadSlots {
    LoadSlots {
        re: net.adjustable(terminal, "spk_vc1", Adjust::Resistor, v.re),
        l1: net.adjustable("spk_vc1", "spk_vc2", Adjust::Inductor, v.l1),
        r1: net.adjustable("spk_vc1", "spk_vc2", Adjust::Resistor, v.r1),
        l2: net.adjustable("spk_vc2", MOTIONAL, Adjust::Inductor, v.l2),
        r2: net.adjustable("spk_vc2", MOTIONAL, Adjust::Resistor, v.r2),
        res: net.adjustable(MOTIONAL, "gnd", Adjust::Resistor, v.res),
        lces: net.adjustable(MOTIONAL, "gnd", Adjust::Inductor, v.lces),
        cmes: net.adjustable(MOTIONAL, "gnd", Adjust::Capacitor, v.cmes),
        lbox: net.adjustable(MOTIONAL, "spk_box", Adjust::Inductor, v.lbox),
        rbox: net.adjustable("spk_box", "gnd", Adjust::Resistor, v.rbox),
    }
}

/// Source impedance of the ideal amplifier used when no power stage is selected.
pub const VOLTAGE_DRIVE_OHMS: f64 = 0.01;

/// The driver alone behind a near-ideal voltage source: what a speaker sees from a
/// chain with no power amplifier in it. Output is the motional node.
pub fn voltage_driven(v: &LoadValues) -> Result<(Circuit, LoadSlots), Fault> {
    let mut net = Netlist::new("speaker, voltage driven");
    net.input("spk", VOLTAGE_DRIVE_OHMS);
    let slots = stamp(&mut net, "spk", v);
    Ok((net.build(MOTIONAL)?, slots))
}
