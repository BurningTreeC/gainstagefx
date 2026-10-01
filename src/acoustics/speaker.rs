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

use super::cabinet::CabinetProfile;
use super::enclosure::{Enclosure, MODE_COUNT, PANEL_GROUPS};
use crate::dsp::complex::C;
use crate::dsp::netlist::{Adjust, Circuit, Fault, Netlist};
use crate::dsp::time::Simulation;
use std::f64::consts::TAU;

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

    /// Roland 30-103D, the JC-120's 12-inch driver. 30 cm and 8 ohm DOCUMENTED
    /// (Roland service notes, every edition from 1979 to 2000); no manufacturer
    /// data beyond that exists publicly. Re is the catalogue's median, and the
    /// coil, Mms and Qms are the priors the Celestions use; Fs is the
    /// catalogue's median and Bl gives its median Qts of 0.8 (all ESTIMATED:
    /// the measurement below cannot see them). The breakup voicing is FITTED,
    /// by `examples/jazz_speaker_fit.rs`, to a published near-field
    /// measurement of a JC-120 through its return input -- the amplifier's own
    /// power stage into these speakers in this cabinet -- reproduced inside the
    /// model at the measurement's placement. See `docs/models/speakers.md`.
    pub const JAZZ_12: SpeakerProfile = SpeakerProfile {
        id: "spk_roland_30_103d",
        name: "Jazz 12",
        inspiration: "Roland 30-103D, 8 ohm (JC-120, 1975 onward)",
        re: 6.68,
        l1: 0.44804e-3,
        r1: 44.048,
        l2: 1.22136e-3,
        r2: 6.1927,
        fs: 85.0,
        qms: 9.73,
        mms: 28.0e-3,
        bl: 10.72,
        sd: 490.9e-4,
        // 1.296 dB rms against the measurement from 90 Hz to 12 kHz,
        // refitted by `examples/jazz_speaker_fit.rs` after the acoustic overhaul.
        breakup: Breakup {
            peaks: [
                pk(545.0, 5.90, 2.09),
                pk(3733.0, 7.83, 3.62),
                pk(10328.0, 18.0, 1.90),
            ],
            lowpass_hz: 4791.0,
            lowpass_q: 0.50,
        },
    };

    /// Celestion G12K-85, 8 ohm: the high-power ceramic twelve of Peavey's
    /// late-80s 4x12s. Built from Celestion's data for the **G12K-100**, its
    /// Legacy-series successor -- the same 1.75 in coil and 50 oz magnet, Fs
    /// 85 Hz, Re 7 ohm (PUBLISHED) -- because no G12K-85 sheet was found and
    /// the two are WIDELY REPORTED to be one design re-rated. Mms, Qms and the
    /// coil are the Celestion priors; Bl is solved from the plot's level and
    /// the breakup voicing fitted to it, 1.98 dB rms from 300 Hz to 7 kHz, by
    /// `tools/speaker_fit/fit_speakers.py` exactly as for the G12T-75. The
    /// result has a stronger motor than the G12T-75 (Bl 14.2 against 10.9),
    /// which is what Celestion's own description -- the T-75 with a larger
    /// magnet -- predicts. See `docs/models/speakers.md`.
    pub const BRIT_K85: SpeakerProfile = SpeakerProfile {
        id: "spk_celestion_g12k85",
        name: "Brit K85",
        inspiration: "Celestion G12K-85, 8 ohm (data: its G12K-100 successor)",
        re: 7.0,
        l1: 0.44804e-3,
        r1: 44.048,
        l2: 1.22136e-3,
        r2: 6.1927,
        fs: 85.0,
        qms: 9.73,
        mms: 28.0e-3,
        bl: 14.161,
        sd: 490.9e-4,
        breakup: Breakup {
            peaks: [
                pk(2469.0, 11.85, 1.14),
                pk(3577.0, 8.80, 1.94),
                pk(5195.0, 7.03, 5.73),
            ],
            lowpass_hz: 6631.0,
            lowpass_q: 0.50,
        },
    };

    /// Eminence Legend B810, the 32 ohm ten Eminence built after the 1970s
    /// SVT's and sells as the 8x10's replacement: the American 8x10's driver,
    /// a stand-in for Ampeg's unpublished "custom 10" Eminence" (PLAUSIBLE).
    /// Every T/S value PUBLISHED (Fs 52.07 Hz, Re 27.5, Qms 13.91, Qes 0.68,
    /// Bl 17.7, Mms 24 g, Sd 350 cm2) and referred to 8 ohm like every profile
    /// here -- impedances a quarter, Bl a half -- since eight in parallel are
    /// 4 ohm. The coil is FITTED to the sheet's impedance curve and the breakup
    /// voicing to its response, 1.19 dB rms from 300 Hz to 7 kHz, both read by
    /// colour off Eminence's plot (`tools/speaker_fit/fit_b810.py`). See
    /// `docs/models/speakers.md`.
    pub const AMERICAN_BASS_10: SpeakerProfile = SpeakerProfile {
        id: "spk_eminence_legend_b810",
        name: "American Bass 10",
        inspiration: "Eminence Legend B810, 32 ohm (referred to 8)",
        re: 6.875,
        l1: 0.59374e-3,
        r1: 64.237,
        l2: 1.04726e-3,
        r2: 4.9425,
        fs: 52.07,
        qms: 13.91,
        mms: 24.0e-3,
        bl: 8.85,
        sd: 350.1e-4,
        breakup: Breakup {
            peaks: [
                pk(1133.0, 5.32, 1.83),
                pk(2545.0, 8.84, 4.00),
                pk(4941.0, 7.28, 5.30),
            ],
            lowpass_hz: 4155.0,
            lowpass_q: 2.50,
        },
    };

    /// Gallien-Krueger P10/200, the 410RBH's cast-frame ten, an Eminence
    /// special for GK (part 082-0000-A): 32 ohm and 200 W DOCUMENTED (GK's
    /// parts listing), Fs 43 Hz SECONDARY (reseller listings). Nothing else is
    /// published, so the rest is ESTIMATED inside a 200 W cast-frame ten's
    /// bounds: Re 27 ohm (the Legend B810's 27.5 for the same nominal), Mms
    /// 32 g, Qms 6, and Bl for a Qes of 0.45 -- a vented-box driver -- which
    /// with that Fs gives Vas about 74 l. The coil and the breakup voicing are
    /// the B810's, fitted to Eminence's plots of a 32 ohm ten of the same
    /// class: priors, not measurements of this part. Referred to 8 ohm like
    /// every profile here (impedances a quarter, Bl a half): four in parallel.
    /// See `docs/models/american_410.md`.
    pub const CAST_BASS_10: SpeakerProfile = SpeakerProfile {
        id: "spk_gk_p10_200",
        name: "Cast Bass 10",
        inspiration: "Gallien-Krueger P10/200, 32 ohm (referred to 8), Eminence-built",
        re: 6.75,
        l1: 0.59374e-3,
        r1: 64.237,
        l2: 1.04726e-3,
        r2: 4.9425,
        fs: 43.0,
        qms: 6.0,
        mms: 32.0e-3,
        bl: 11.4,
        sd: 350.1e-4,
        breakup: Breakup {
            peaks: [
                pk(1133.0, 5.32, 1.83),
                pk(2545.0, 8.84, 4.00),
                pk(4941.0, 7.28, 5.30),
            ],
            lowpass_hz: 4155.0,
            lowpass_q: 2.50,
        },
    };

    /// Every implemented driver, in the order stable ids were assigned.
    pub const ALL: [&'static SpeakerProfile; 11] = [
        &Self::BRIT_V30,
        &Self::BRIT_GREEN_25,
        &Self::BRIT_T75,
        &Self::AMERICAN_VINTAGE_12,
        &Self::AMERICAN_VINTAGE_10,
        &Self::AMERICAN_CERAMIC,
        &Self::AMERICAN_ALNICO,
        &Self::JAZZ_12,
        &Self::BRIT_K85,
        &Self::AMERICAN_BASS_10,
        &Self::CAST_BASS_10,
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

    /// Effective central radiator above breakup. Retained from the earlier
    /// placement model: estimated from placement guides, not cone-velocity data.
    /// Keeping this explicit avoids applying a rigid full-disc model outside
    /// its valid band and mistaking its deep zeros for measured guitar tone.
    pub fn breakup_radius_ratio(&self) -> f64 {
        0.4
    }

    /// First fitted breakup feature supplies the transition band. The spatial
    /// response needs measured complex cone motion for a more specific model.
    pub fn breakup_transition_hz(&self) -> f64 {
        self.breakup
            .peaks
            .iter()
            .map(|p| p.hz)
            .fold(f64::INFINITY, f64::min)
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
    /// Geometry for the passive cavity, opening and flexible-wall model.
    pub cabinet: Option<CabinetProfile>,
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
        cabinet: None,
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
    pub cavity_modes: [SeriesRlc; MODE_COUNT],
    /// Equivalent impedances of opening and panel acoustic mobilities, in
    /// series with Lbox. Their common branch current represents cavity pressure.
    pub opening_r: f64,
    pub opening_c: f64,
    pub panels: [ParallelRlc; PANEL_GROUPS],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesRlc {
    pub r: f64,
    pub l: f64,
    pub c: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParallelRlc {
    pub r: f64,
    pub l: f64,
    pub c: f64,
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
        let mut result = Self {
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
            // N identical drivers share terminal power. The equivalent
            // impedance is unchanged; each cone's velocity is 1/sqrt(N).
            bl: profile.bl * (k * drivers).sqrt(),
            cavity_modes: [SeriesRlc {
                r: 1e12,
                l: 1.0,
                c: 1e-9,
            }; MODE_COUNT],
            opening_r: 1e-6,
            opening_c: 1e-9,
            panels: [ParallelRlc {
                r: 1e-6,
                l: 1.0,
                c: 1e-9,
            }; PANEL_GROUPS],
        };
        if let Some(cab) = mounting.cabinet {
            let e = Enclosure::new(&cab, profile);
            // Duality: Z_e = Bl²/(N Sd²) * Y_acoustic. All positive elements
            // give a passive load, including partially open cabinets.
            let reference = k * bl2 / (drivers * profile.sd.powi(2));
            result.lbox = reference * e.volume / (RHO * SPEED_OF_SOUND.powi(2));
            if e.opening_area > 0.0 {
                let mass = RHO * e.opening_length / e.opening_area;
                // Low-frequency opening radiation resistance evaluated at its
                // Helmholtz resonance; the lumped opening is a low-band model.
                let omega2 =
                    SPEED_OF_SOUND.powi(2) * e.opening_area / (e.volume * e.opening_length);
                let resistance = RHO * omega2 / (TAU * SPEED_OF_SOUND);
                result.opening_r = reference / resistance.max(1e-6);
                result.opening_c = mass / reference;
            }
            for (values, panel) in result.panels.iter_mut().zip(e.panels) {
                if panel.area > 0.0 {
                    let omega = TAU * panel.hz;
                    let transform = reference * panel.area.powi(2) * panel.count as f64;
                    *values = ParallelRlc {
                        r: transform / (panel.loss * omega * panel.mass),
                        l: transform / (omega.powi(2) * panel.mass),
                        c: panel.mass / transform,
                    };
                }
            }
            for (values, mode) in result.cavity_modes.iter_mut().zip(e.modes) {
                if mode.stiffness > 0.0 {
                    let l = k * bl2 / mode.stiffness;
                    *values = SeriesRlc {
                        r: l * mode.damping,
                        l,
                        c: 1.0 / ((TAU * mode.hz).powi(2) * l),
                    };
                }
            }
        }
        result
    }

    /// The electrical impedance at the terminals, analytically.
    pub fn impedance(&self, hz: f64) -> C {
        let s = C::new(0.0, std::f64::consts::TAU * hz);
        let lossy = |l: f64, r: f64| {
            let sl = s * C::real(l);
            sl * C::real(r) / (C::real(r) + sl)
        };
        let coil = C::real(self.re) + lossy(self.l1, self.r1) + lossy(self.l2, self.r2);
        let panel = self.panels.iter().fold(C::ZERO, |sum, panel| {
            sum + (C::real(1.0 / panel.r) + (s * C::real(panel.l)).recip() + s * C::real(panel.c))
                .recip()
        });
        let opening = (C::real(1.0 / self.opening_r) + s * C::real(self.opening_c)).recip();
        let mut admittance = C::real(1.0 / self.res)
            + (s * C::real(self.lces)).recip()
            + s * C::real(self.cmes)
            + (s * C::real(self.lbox) + C::real(self.rbox) + opening + panel).recip();
        for mode in self.cavity_modes {
            admittance = admittance
                + (C::real(mode.r) + s * C::real(mode.l) + (s * C::real(mode.c)).recip()).recip();
        }
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
    opening: [usize; 2],
    panels: [[usize; 3]; PANEL_GROUPS],
    cavity_modes: [[usize; 3]; MODE_COUNT],
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
        for (slots, values) in self.cavity_modes.iter().zip(v.cavity_modes) {
            for (&slot, value) in slots.iter().zip([values.r, values.l, values.c]) {
                sim.set_value(slot, value);
            }
        }
        for (&slot, value) in self.opening.iter().zip([v.opening_r, v.opening_c]) {
            sim.set_value(slot, value);
        }
        for (slots, values) in self.panels.iter().zip(v.panels) {
            for (&slot, value) in slots.iter().zip([values.r, values.l, values.c]) {
                sim.set_value(slot, value);
            }
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
        rbox: net.adjustable("spk_box", "spk_open", Adjust::Resistor, v.rbox),
        opening: [
            net.adjustable("spk_open", "spk_panel", Adjust::Resistor, v.opening_r),
            net.adjustable("spk_open", "spk_panel", Adjust::Capacitor, v.opening_c),
        ],
        panels: std::array::from_fn(|i| {
            let a = if i == 0 {
                "spk_panel".to_owned()
            } else {
                format!("spk_panel_{i}")
            };
            let b = if i + 1 == PANEL_GROUPS {
                "gnd".to_owned()
            } else {
                format!("spk_panel_{}", i + 1)
            };
            let panel = v.panels[i];
            [
                net.adjustable(&a, &b, Adjust::Resistor, panel.r),
                net.adjustable(&a, &b, Adjust::Inductor, panel.l),
                net.adjustable(&a, &b, Adjust::Capacitor, panel.c),
            ]
        }),
        cavity_modes: std::array::from_fn(|i| {
            let a = format!("spk_mode_{i}_a");
            let b = format!("spk_mode_{i}_b");
            let mode = v.cavity_modes[i];
            [
                net.adjustable(MOTIONAL, &a, Adjust::Resistor, mode.r),
                net.adjustable(&a, &b, Adjust::Inductor, mode.l),
                net.adjustable(&b, "gnd", Adjust::Capacitor, mode.c),
            ]
        }),
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
