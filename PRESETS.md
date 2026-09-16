# Presets

## Storage and compatibility

- Factory entries are `Preset` structs. `Preset::dials()` maps them to host parameter
  ids, and `Preset::settings()` gives the exact chain settings (used by level tests).
- Saved presets are JSON with normalized values plus `model_ids` (stable enum ids, which
  take precedence).
- `presets::migrate` fills parameters a legacy preset lacks with their legacy
  meaning: `power_amp = matched`, `cab_model = legacy`, `pedal = none`.
- Host state gets the same defaults in `GainStageFx::filter_state`.
- Existing enum ids and orders are never changed. New lists are separate parameters
  whose first entry is the legacy behaviour.

## Groups

Studio, Preamp, Crunch, High Gain, Overdrive, Distortion, Amplifier (the original
catalogue); Classic Rock, Psychedelic / Lead, Alternative, Metal / Heavy and Blues
(era presets). Further groups are added when a preset exists for them. The panel
lists every group that has one.

## Era presets: what exists

Inspiration mappings are developer documentation, not UI text. Evidence labels:
**DOCUMENTED** (engineer, producer or artist statement in an interview or primary
publication), **WIDELY REPORTED** (consistent across gear-history sources, no
primary quote located), **PLAUSIBLE**, **APPROXIMATED** (component not available).

### Puppet Master '86 (`metal_1986_puppet`, group Metal / Heavy)

Target inspiration: Metallica, *Master of Puppets* (1986) rhythm tone. Re-researched
2026-09-15 after the owner reported too little distortion.

| Stage | Rig | Evidence | Preset |
|---|---|---|---|
| Guitar | Jackson King V with Seymour Duncan Invader pickups (a hot passive set) | WIDELY REPORTED | input trim +3 dB - PLAUSIBLE |
| Pedal | none on the rhythm tone; "a lot of the distortion would come from the amp itself" | DOCUMENTED (Rasmussen interview) | none |
| Preamp | Mesa/Boogie Mark IIC+ lead channel, five-band EQ used | DOCUMENTED (Rasmussen) | Cali IIC+ (now with its V2B/V2A recovery stages), Lead Drive 0.80, Lead Master 0.55 |
| Knob settings | "Gain 9-10, Vol 8, Bass 8, Mid 3-4, Treble 8, Presence 7" / "Gain 7-8, Bass 6-7, Mid 3-4, Treble 6-7, Lead Drive 7-8, Lead Master 4-5"; graphic V (80 up, 240 down, 750 most down, 2.2k and 6.6k up) | CONFLICTING secondary sources, both attributed to the producer's session notes, which were not located. Mesa's IIC+ owner's manual: "As gain goes up, Bass should come down", BASS "at 3.0 or well below" with a high Volume 1, low end from the 80/240 Hz bands | Bass 0.20, Middle 0.35, Treble 0.75; graphic [0.65, 0.45, 0.25, 0.65, 0.70] (slider travel) - PLAUSIBLE, following the manual rather than the unlocated notes |
| Power | **the Boogie power stage**: "It is only on some solos we use the JCM 800 as poweramp. Most guitars are with the Boogie powerstage!" | DOCUMENTED (Flemming Rasmussen, 29 June 2018, quoted on Marshall Amp Forum) | Matched (Cali 6L6). *The earlier "IIC+ slaved into a JCM800" choice followed a widely repeated story the producer contradicts.* |
| Cabinet / speaker | Two 4x12s. The session notes are read as Marshall 4x12 (G12-65 vs G12T-75 debated); the interview says "their new cabinets" (Mesa) | CONFLICTING | Brit Closed 4x12, Matched (T75) - APPROXIMATED |
| Mics | "two Shure SM57s right in the cone"; room mics; "AKG Gold tubes, in a 45-degree angle out from each cabinet, three or four feet away" | DOCUMENTED (Rasmussen interview) | Dynamic 57 at 0.2 / 2.5 cm / 0 deg; Tube Condenser 67 at 1.0 m / 45 deg, blend 0.25 - APPROXIMATED (no AKG tube profile) |
| Studio | parametric EQ after the amp, double-tracked rhythms | DOCUMENTED | not modeled (outside the chain) |

Sources:
- [How Flemming Rasmussen produced Metallica's Master Of Puppets (MusicRadar via Yahoo)](https://www.yahoo.com/entertainment/articles/flemming-rasmussen-produced-metallicas-master-180000815.html)
- [Master Of Puppets Was All Boogie, Baby (Marshall Amp Forum)](https://marshallforum.com/threads/master-of-puppets-was-all-boogie-baby.105049/)
- [Deciphering Flemming Rasmussen's Notes (Mesa Boogie Amp Forum)](https://boogieforum.com/threads/deciphering-flemming-rasmussens-notes.50309/)
- [Guitar Chalk: Master of Puppets amp settings](https://www.guitarchalk.com/master-of-puppets-metallica-amp-settings/)
- [Boost Guitar Pedals: Hetfield's gear on Master of Puppets](https://www.boostguitarpedals.co.uk/blogs/gear-of-the-gods/james-hetfields-gear-on-master-of-puppets-1986)
- [gearnews: How to sound like Master of Puppets](https://www.gearnews.com/how-to-sound-like-metallica-master-of-puppets/)

**Why it lacked distortion.** Two causes:
- The Cali IIC+ model stopped at `LEAD OUTPUT` and left out V2B and V2A, which both
  drawings have. At a guitar's level the preamp made 39 %; corrected, it makes 61 %. See
  [docs/models/cali_iic_plus.md](docs/models/cali_iic_plus.md).
- The preset sat on the wrong power stage.

**Why it had too much low end** (re-voiced 2026-09-15 after the owner reported it).
- The 80 Hz slider sat at 0.85, about +9 dB, which put about 6 dB more energy in the
  63 Hz octave (relative to 1 kHz) than any other modelled-amplifier preset
  (`examples/octaves.rs`, a struck E5 chord through the whole chain).
- The Bass pot was modelled linear. Both the SLOCLONE trace ("250k A") and Mesa's
  replacement part (A250K, "MK II/III Bass, Treble, Master") say audio taper. On a linear
  track, Bass 0.3 already had all but 1.5 dB of the available bass in circuit.
- The Bass knob is in front of four clipping stages, so at this gain it changes the
  output's octave balance by well under a decibel. The slider after the gain stages is
  what sets the low end, as the manual says.
- Now: Bass 0.20, a moderate V ([0.65, 0.45, 0.25, 0.65, 0.70]), and the 57 at 2.5 cm
  instead of 1.5 cm. The low octaves are 5.5 dB lower relative to 1-4 kHz; the
  low/high balance (-22 dB) sits between Brit Lead and Brit Crunch.

### Blackout '80 (group Classic Rock)

Target inspiration: AC/DC, *Back in Black* (recorded April 1980, Compass Point, Nassau;
engineer Tony Platt, producer Mutt Lange).

| Stage | Rig | Evidence | Preset |
|---|---|---|---|
| Guitar | Gibson SG ("we'll need a Gibson SG and we'll need Angus") | DOCUMENTED (Platt, Sound On Sound) | nominal input |
| Pedal | none on the rhythm tracks. Angus's solos went through a radio transmitter that "gave the guitar ... quite an edge" | DOCUMENTED (Platt) | none; the transmitter's filtering is not modeled |
| Preamp / power | "a number of different Marshall heads and cabinets ... Sometimes it would be a 50 Watt head ... sometimes it would be a 100 Watt head"; "we only ever turned up the amplifiers as far as we needed to" | DOCUMENTED (Platt) | Brit Plexi (1959 Super Lead 100 W), Volume 0.55 (not dimed), Matched EL34 - APPROXIMATED (heads' years and revisions per song not documented) |
| Knob settings | not documented | - | Treble 0.6, Middle 0.7, Bass 0.4 - PLAUSIBLE |
| Cabinet / speaker | Marshall 4x12 | DOCUMENTED (make and size only) | Brit 1960 4x12, Matched - APPROXIMATED (speaker type not documented) |
| Mics | "a couple of Neumann U67 microphones or a U67 and U87 - sometimes cardioid, sometimes figure of eight, sometimes omni, placed on different speakers of the cabinet" | DOCUMENTED (Platt) | Tube Condenser 67 on one speaker (0.35, 10 cm) and Condenser 87 on another (0.6, 15 cm), blend 0.5 - distances and patterns PLAUSIBLE (cardioid only) |
| Room | ambience recorded "on tape right from the start" | DOCUMENTED | not modeled |

Sources: [Classic Tracks: AC/DC 'Back In Black' (Sound On Sound)](https://www.soundonsound.com/techniques/classic-tracks-acdc-back-black).

### Experienced '67 (group Psychedelic / Lead)

Target inspiration: The Jimi Hendrix Experience, *Are You Experienced* (October 1966 -
April 1967; De Lane Lea, CBS and Olympic, London; Eddie Kramer engineered most of it).

| Stage | Rig | Evidence | Preset |
|---|---|---|---|
| Guitar | Fender Stratocasters | WIDELY REPORTED (Guitar.com rig feature) | nominal input |
| Pedal | Arbiter Fuzz Face on "Love or Confusion"; Octavia on "Purple Haze"/"Fire" | WIDELY REPORTED (Guitar.com, citing Roger Mayer for the Octavia) | Round Fuzz (germanium Fuzz Face), Fuzz 0.85, Level 0.6. No Octavia model |
| Preamp / power | "at least two Super 100 Marshall Heads and four 4x12 cabinets" bought in autumn 1966 | WIDELY REPORTED | Brit Plexi, Volume 0.7 - APPROXIMATED: the 1966-67 Super 100 (JTM45/100) ran KT66 valves and earlier values; the model is the 1970 EL34 1959 |
| Cabinet / speaker | Marshall 4x12 | WIDELY REPORTED | Brit Green 4x12 (25 W greenbacks) - PLAUSIBLE for 1966 Marshall cabinets |
| Mics | "a ribbon microphone pointed straight at the cone usually a Beyer M160", often combined with a U67 | DOCUMENTED (Eddie Kramer's own post) / WIDELY REPORTED (the U67) | Ribbon 160 at 5 cm on axis, Tube Condenser 67 at 50 cm, blend 0.35 - distances PLAUSIBLE |
| Studio | "a strong commitment to the EQ and compression in the control room" | DOCUMENTED (Kramer) | not modeled |

Sources: [The gear used on Are You Experienced (Guitar.com)](https://guitar.com/features/artist-rigs/the-gear-used-on-are-you-experienced-by-the-jimi-hendrix-experience/),
[Eddie Kramer on the M160 (his Facebook page)](https://m.facebook.com/EddieKramerProducer/photos/a.304749109593232/609331622468311/),
[Are You Experienced (Wikipedia)](https://en.wikipedia.org/wiki/Are_You_Experienced).

### The Great Wall '79 (group Psychedelic / Lead)

Target inspiration: Pink Floyd, *The Wall* (1979), David Gilmour's lead sound.

| Stage | Rig | Evidence | Preset |
|---|---|---|---|
| Guitar | the black Stratocaster, "a 1969 black alder body ... Fender single coils and a DiMarzio FS-1 bridge pickup"; a Les Paul Goldtop on one solo | WIDELY REPORTED (Guitar.com rig feature) | nominal input |
| Pedal | a Pete Cornish board with an Electro-Harmonix Big Muff, a Fuzz Face, an Electric Mistress and a Dynacomp; the Big Muff used "with sustain considerably lower than most users expect" | WIDELY REPORTED | Ram Fuzz, Sustain 0.35, Tone 0.55, Level 0.75. The Mistress and the Dynacomp are not modeled |
| Preamp / power | "an Alembic F-2B tube preamp and a 100-watt Hiwatt DR103, equipped with four Mullard EL34 power tubes" | WIDELY REPORTED | Brit DR103, Volume 0.45, Master 0.5, Matched EL34. **The Alembic preamp in front of it is not modeled**, and the 1970s DR103 differs from the Issue 4 drawing the model follows (see [brit_dr103.md](docs/models/brit_dr103.md)) |
| Cabinet / speaker | "WEM Super Starfinder 200s with Fane Crescendo speakers", plus a Yamaha RA-200 rotary cabinet | WIDELY REPORTED | Brit Closed 4x12, Matched - APPROXIMATED (no WEM cabinet or Fane profile; the rotary cabinet is not modeled) |
| Mics | not documented in the sources located | - | Dynamic 421 close and Condenser 87 at half a metre, blend 0.3 - PLAUSIBLE |

Sources: [The guitar gear used by David Gilmour to record The Wall (Guitar.com)](https://guitar.com/features/artist-rigs/the-gear-used-by-david-gilmour-pink-floyd-the-wall/),
[gilmourish.com, The Wall 1979-81](https://www.gilmourish.com/?page_id=20).

### Pumpkin Dream '93 (group Alternative)

Target inspiration: The Smashing Pumpkins, *Siamese Dream* (recorded December 1992 -
March 1993, Triclops Studios; produced by Butch Vig and Billy Corgan).

| Stage | Rig | Evidence | Preset |
|---|---|---|---|
| Guitar | the "Bat Strat", a Stratocaster with Lace Sensors | WIDELY REPORTED | nominal input |
| Pedal | an Electro-Harmonix Big Muff Pi, "the late 70s V4 'Op Amp' version" | WIDELY REPORTED | Ram Fuzz - **APPROXIMATED**: the model is the 1973 Ram's Head, four transistors; the op-amp Muff is a different circuit and is not modeled |
| Preamp / power | "All of the Siamese Dream rhythm sound was done by plugging an EHX Big Muff Pi into the low gain input of a 1984 JCM800 (2203) head set with the master volume all the way, and used the preamp gain to make adjustments"; the amp was retubed with KT88s | WIDELY REPORTED, consistent across rig features | Brit 800, Preamp 0.35, Master 1.0, Matched EL34 - APPROXIMATED: the model has the drawing's EL34s and its HIGH input, not the low one, so the pedal's Level stands in for that 6 dB |
| Cabinet / speaker | a Marshall 4x12 ("Mars") | WIDELY REPORTED | Brit 1960 4x12, Matched |
| Other effects | Mu-Tron Bi-Phase, Electro-Harmonix Electric Mistress | WIDELY REPORTED | not modeled |
| Studio | many layered guitar takes ("we wanted to go way, way over the top", Butch Vig) | DOCUMENTED (Vig, Tape Op via Rolling Stone) | one track; the layering is the player's to do |

Sources: [The gear of the Smashing Pumpkins (Reverb)](https://reverb.com/news/the-gear-of-the-smashing-pumpkins),
[Billy Corgan's gear (GuitarGangsters)](https://guitargangsters.net/billy-corgan-guitars-gear/),
[Smashing Pumpkins recorded with Butch Vig again (Rolling Stone)](https://www.rollingstone.com/music/music-news/smashing-pumpkins-butch-vig-first-time-siamese-dream-1235566401/).

### Brown '78 and Brown '84 (group Classic Rock)

Target inspiration: Van Halen, *Van Halen* (1978) and *1984*, Eddie Van Halen's rhythm
and lead sound. Engineer Donn Landee, producer Ted Templeman.

| Stage | Rig | Evidence | Preset |
|---|---|---|---|
| Guitar | the Frankenstrat, a PAF humbucker from a 1960s ES-335 in a Strat body, one volume knob | WIDELY REPORTED | nominal input |
| Pedal | none in the amplifier's path (MXR Phase 90, Flanger and an Echoplex on some tracks) | WIDELY REPORTED | none |
| Preamp / power | a 1967/68 Marshall 1959 Super Lead, "pretty much stock", used on the first six records; "the only way the amp sounded good was with everything all the way up" | WIDELY REPORTED; Eddie's own 1978 claims about modifications were later retracted | Brit Plexi, Volume 1.0, Matched EL34 |
| **Mains** | "he was using the Variac [variable transformer] to run his Marshall at 85 volts instead of 120. That was one of the key reasons he got that sound ... He'd have it set to between 80 and 85 volts and he would rarely change it" | DOCUMENTED (Donn Landee, Tape Op interview) | **Mains 70 %**, which is 84 V of 120. The whole amplifier's supplies come down together; see `Chain::set_mains` |
| Cabinet / speaker | Marshall 4x12s; a combination cabinet with Celestions and JBLs is reported for the first album | WIDELY REPORTED / CONFLICTING | Brit Green 4x12, Matched (25 W greenbacks) - APPROXIMATED, no JBL profile |
| Mics | "If the mic was right on the speaker, it was likely an SM56 or 57"; one cabinet, always | DOCUMENTED (Landee) | Dynamic 57 close, with a condenser (1978) or ribbon (1984) further out |
| Not modeled | the dummy-load resistor box reported on the first album, the Echoplex preamp, the phaser and flanger | - | - |

Sources: [Donn Landee interview (Tape Op)](https://tapeop.com/interviews/159/donn-landee-1),
[The gear used by Eddie Van Halen on Van Halen I (Guitar.com)](https://guitar.com/features/artist-rigs/the-gear-used-by-eddie-van-halen-on-van-halen-i/),
[Eddie Van Halen's 1967/68 Marshall 1959 (Ground Guitar)](https://www.groundguitar.com/eddie-van-halen-gear/eddie-van-halens-1967-68-marshall-1959-super-lead/).

### Blizzard '80 (group Classic Rock)

Target inspiration: Ozzy Osbourne, *Blizzard of Ozz* (recorded March-April 1980, Ridge
Farm; engineer Max Norman), Randy Rhoads's rhythm and lead sound.

| Stage | Rig | Evidence | Preset |
|---|---|---|---|
| Guitar | a Les Paul Custom, later the polka-dot Flying V | WIDELY REPORTED | nominal input |
| Pedal | "The crucial effects in the pedalboard were an MXR Distortion +, which Rhoads almost always kept on, and an MXR Ten Band Graphic Equalizer" | WIDELY REPORTED | Yellow Dist, Distortion 0.6, Output 0.7. The ten-band EQ is not modeled |
| Preamp / power | "a stock mid-'70s Marshall JMP model 1959 Super Lead MKII 100-watt head that was rented for the sessions" | WIDELY REPORTED | Brit Plexi, Volume 0.85, Matched EL34 |
| **Mains** | "Similar to Eddie Van Halen, he would turn the voltage down to 90 or 92" volts | WIDELY REPORTED | **Mains 80 %**, which is 96 V of 120 -- the nearest setting |
| Cabinet / speaker | "a pair of 4x12 cabinets loaded with Altec 417-8H speakers" | WIDELY REPORTED | Brit Closed 4x12, Matched - APPROXIMATED (no Altec profile) |
| Studio | a short delay and slight detune across the two sides (AMS DMX 15-80S) | WIDELY REPORTED (Max Norman) | not modeled |

Sources: [The gear used by Randy Rhoads on Blizzard of Ozz (Guitar.com)](https://guitar.com/features/artist-rigs/gear-used-by-randy-rhoads-blizzard-of-ozz-ozzy-osbourne/),
[Behind the scenes of Blizzard of Ozz (Guitar World)](https://www.guitarworld.com/features/ozzy-osbourne-randy-rhoads-recording-of-blizzard-of-ozz).

### Machine Rage '92 (group Alternative)

Target inspiration: Rage Against the Machine, *Rage Against the Machine* (1992), Tom
Morello's rhythm sound. Producer Garth Richardson.

| Stage | Rig | Evidence | Preset |
|---|---|---|---|
| Guitar | "Arm The Homeless", a 1986 Performance Guitar build with EMG pickups; a 1982 Telecaster for drop-D tracks | WIDELY REPORTED | nominal input |
| Preamp / power | a Marshall **2205** head: "It's identical. It's unchanged, for every RATM album, every Audioslave song, every show" (Morello) | WIDELY REPORTED, and the quote is his | Brit 800 - **APPROXIMATED**: the model is the 100 W single-channel 2203, and the 2205 is the two-channel 50 W amplifier of the same family and year. Its extra preamp stage is not modeled |
| Pedals | a DOD EQ as a boost, Whammy, Cry Baby, DOD delay, flanger | WIDELY REPORTED | none. The EQ-as-boost is not modeled, and nothing else here is on the rhythm sound |
| Cabinet / speaker | a Peavey 4x12 | WIDELY REPORTED | Oversized 4x12, Matched - APPROXIMATED (no Peavey cabinet profile) |
| Studio | tracked live with the amplifiers in back rooms and a PA in the live room (Garth Richardson) | DOCUMENTED | not modeled |

Sources: [The gear used on Rage Against The Machine's debut (Guitar.com)](https://guitar.com/features/artist-rigs/the-gear-used-on-rage-against-the-machine-self-titled-debut/),
[All-Star Gear: Tom Morello's JCM800 (MusicRadar)](https://www.musicradar.com/news/guitars/all-star-gear-tom-morellos-arm-the-homeless-guitar-and-marshall-jcm800-amp-537090).

### Texas Storm '83 (`blues_1983_texas_storm`, group Blues)

Target inspiration: Stevie Ray Vaughan, *Texas Flood* (recorded November 1982,
released 1983).

| Stage | Rig | Evidence | Preset |
|---|---|---|---|
| Pedal | Ibanez Tube Screamer (the only effect) | DOCUMENTED (engineer Richard Mullen via Jas Obrecht / Line 6; Wikipedia summary) | Green 808, low drive, high level |
| Amps | 1964 blackface Fender Vibroverb (1x15 Altec) and Dumble Steel String Singer (Dumble 4x12, EV speakers) | DOCUMENTED | American Twin (same AB763 blackface family as the Vibroverb), Matched power |
| Cabinet / speaker | Vibroverb open-back 1x15 with Altec | DOCUMENTED | American Open 1x12, American Ceramic - APPROXIMATED (no 15-inch driver or Altec profile) |
| Mics | two SM57s (one per amp) | DOCUMENTED | Dynamic 57, close. The Dumble path is not represented - APPROXIMATED |

Sources: [Jas Obrecht: SRV's Texas Flood sessions (Line 6 blog)](https://blog.line6.com/2023/05/19/jas-obrecht-stevie-ray-vaughans-texas-flood-sessions/),
[Texas Flood (Wikipedia)](https://en.wikipedia.org/wiki/Texas_Flood).

## Era presets: waiting for components

These are not added under approximate aliases. Each needs the listed model first,
plus its own rig research at that time.

| Preset | Blocked on |
|---|---|
| Appetite '87 | Rig research. Commonly said to be a rented, modified 1959T; the stock Brit Plexi is not that amplifier |
| Dirty Chains '92 | Three amplifiers at once, none of them modeled: "Lows: Bogner Fish preamp/VHT amp ... mids: Bogner Ecstasy; highs: Rockman Pocket amp direct" (Dave Jerden), split three ways and recombined |
| Unknown Garden '94 | A Sunn Model T for much of it, which is not modeled |
| Spiral '96 | A modified mid-70s Marshall Super Bass with its channels jumpered, alongside a Rectifier; the Super Bass is not modeled and the record is the two together |
| Never Mind '91, Seattle Ten '91, Desert Deaf '02, Californicated '99, Blood Sugar '91 | Rig research; the documented amplifiers (a Mesa Studio .22 preamp, a Marshall Major, an Ampeg VT) are not modeled |
| Lead Airship II '69 | Rig research (the amplifier used on the record is disputed) |
| Paranoia '70 | A Laney Supergroup preamp and a treble booster |

## Catalogue update (2026-09-15)

Every shipped guitar preset now comes out of a physical cabinet, speaker and microphone
setup. The legacy baked Combo/Stack filter is kept for old sessions, but no shipped
preset uses it.

- **Topologies** (Crunch, High Gain, Overdrive and Distortion circuits, Ram Fuzz) drive
  the speaker directly. They have no power stage, and adding one would run it inside the
  2x/4x oversampler at several times the cost.
- **Modelled amplifiers** drive the speaker from their own power stage, with Matched
  power.
- **Green Overdrive** and **Screamer Boost** put the corrected Green 808 in the pedal slot:
  - Green Overdrive: in front of the American Twin;
  - Screamer Boost: as a boost into the Brit 800.
- **Cali IIC+** presets were re-voiced for the recovery stages. Their Master knob is now
  the Lead Master.
- **American Twin** presets lost their Steel iron. On the physical path the Iron control
  sits in front of the power stage, which an AB763 does not have.
- **New:** Brit Crunch and Brit Lead (Brit 800, Amplifier group).
- Studio, Preamp and 73P presets are unchanged.
- **New microphone preamplifiers** (Preamp group): American Console Pre / Pushed
  (American 312), British Desk Pre / Driven (British 4K E), Valve Console Pre / Pushed
  (Tube 610).

Level (220 Hz, `tests/presets.rs`): the catalogue spans 10.8 dB (Blackout '80 loudest at
+3.7 dB, Parallel Warmth quietest at -7.1), with nine output trims set to keep it there.
`examples/presetlevel.rs` prints the whole list, which is how those trims are chosen.

## Level

`tests/presets.rs::the_presets_are_level_matched` measures every factory preset through
`Preset::settings()`, including pedal, power stage and cabinet, and requires the
catalogue span to stay under 14 dB.
