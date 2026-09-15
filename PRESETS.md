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
catalogue); Metal / Heavy and Blues (era presets). Further groups (Classic Rock,
Psychedelic / Lead, Alternative) are added when a preset exists for them. The panel
lists every group that has one.

## Era presets: what exists

Inspiration mappings are developer documentation, not UI text. Evidence labels:
**DOCUMENTED** (engineer, producer or artist statement in an interview or primary
publication), **WIDELY REPORTED** (consistent across gear-history sources, no
primary quote located), **PLAUSIBLE**, **APPROXIMATED** (component not available).

### Puppet Master '86 (`metal_1986_puppet`, group Metal / Heavy)

Target inspiration: Metallica, *Master of Puppets* (1986) rhythm tone.

| Stage | Rig | Evidence | Preset |
|---|---|---|---|
| Pedal | none on the core rhythm tone | WIDELY REPORTED | none |
| Preamp | Mesa/Boogie Mark IIC+ used as a preamp | WIDELY REPORTED (Ground Guitar, gearnews, Mixdown) | Cali IIC+, drive 0.70, graphic V |
| Power | modified Marshall JCM800 slaved from it | WIDELY REPORTED | Brit EL34 (2203 1981) |
| Cabinet / speaker | Marshall 4x12 | WIDELY REPORTED; speaker model not documented | Brit Closed 4x12, Matched (Brit T75) - PLAUSIBLE |
| Mic | not documented | - | Dynamic 57, close, slightly off centre, 10 deg - PLAUSIBLE |
| Studio | B&B/Aphex parametric EQ, desk EQ, triple-tracked rhythms | WIDELY REPORTED | not modeled (outside the chain) |

Sources: [Ground Guitar: Hetfield's Mark IIC+](https://www.groundguitar.com/james-hetfield-guitars-and-gear/james-hetfields-mesa-boogie-mark-iic/),
[gearnews: How to sound like Master of Puppets](https://www.gearnews.com/how-to-sound-like-metallica-master-of-puppets/),
[Mixdown](https://mixdownmag.com.au/features/how-to-get-the-metallica-master-of-puppets-tone/).

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
| Blackout '80, Appetite '87, Brown '78, Brown '84, Blizzard '80, Lead Airship II '69, Paranoia '70 | Brit 800 / plexi-era British preamp |
| The Great Wall '79 | Brit DR103 preamp and EL34 Hi-Headroom power |
| Experienced '67 | Round Fuzz pedal and a vintage British amp |
| Never Mind '91, Pumpkin Dream '93, Dirty Chains '92, Seattle Ten '91, Unknown Garden '94, Spiral '96, Machine Rage '92, Desert Deaf '02, Californicated '99, Blood Sugar '91 | Rig research, plus models such as Rodent, Cali Rectifier and Brit AC30 depending on the documented rig |

## Level

`tests/presets.rs::the_presets_are_level_matched` measures every factory preset through
`Preset::settings()`, including pedal, power stage and cabinet, and requires the
catalogue span to stay under 14 dB.
