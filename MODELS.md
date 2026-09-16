# Model inventory and evidence policy

Before implementation: 13 selectable Gain entries / 21 gain variants.

| Existing circuit | Implementation | Evidence status / target name |
| --- | --- | --- |
| Clean/Crunch/High Gain | 1/2/3 cascaded triode netlists | Generic topology, no specific amp claim |
| Overdrive/Distortion | op-amp feedback/shunt diode circuits, silicon/germanium/LED | Generic circuits, not TS9/RAT |
| Console/Studio | transformer/discrete or op-amp, valve/JFET/op-amp alternatives | Generic circuits, not API 312/UA610 |
| TS808 | 9V, midpoint bias, BJT buffers, op-amp feedback diodes, tone/level/output network | Existing hardware model; Green 808 display target |
| Big Muff | four BJTs, two feedback diode stages, passive tone, volume | Ram's Head selected; Triangle/Supa definitions also exist |
| Mark IIC+ | four triodes, own stack and graphic; separate 60W power | Cali IIC+ |
| 5150 | six triodes; substitute external tone; separate power | Revision identification needs recheck (ULTRA-labelled source); American 5150 |
| DIYRE 73P v1.1 | input transformer, PRE1/PRE2 BJTs, separate output driver/transformer | British 73; NOT a literal original 1073 |
| Twin AB763 | channel/stack, power, spring and optical tremolo approximations | American Twin |
| Iron | nickel/steel/amorphous nonlinear flux cores with winding network | Physics-derived and empirically tuned |
| Cabinet Combo/Stack | loaded RLC highpass/lowpass/extra pole | Empirical response approximation; no geometry/load/mic |

Other modeled components: Koren triodes; screen-dependent pentodes; Shockley
diodes; bipolar and JFET devices; dynamic op-amp; ideal transformers; nonlinear
integrated-flux cores; RC/RL companion models; passive Wide/Scooping tone circuits.
See ARCHITECTURE.md for actual routing rather than relying on module prose.

Added since the audit: independent selection of the three existing guitar power
circuits plus a 1981 2203-inspired Brit EL34 circuit, Matched and Bypass. Brit EL34
reuses the MNA builder and pentode fit with documented component data and estimates;
see [research checkpoint](docs/models/brit_el34.md). No Brit 800 preamp was added.
The five requested display-name changes for existing hardware models are applied.

Added since: an independent pedal slot (Green 808, Big Muff, Green 9); the catalogue
Green 808 corrected to the three-source TS-808 values; physical speakers with a
reactive load coupled into the power stage, geometry-based cabinets, and eleven
microphone profiles with placement and dual mic. See docs/MODEL_INVENTORY.md.

Added 2026-09-15: the Brit 800 preamp (1981 2203 drawing, Matched = Brit EL34); the
Cali IIC+ corrected to include its lead return, V2B, Lead Master and V2A.

Added later on 2026-09-15: the American 312, British 4K E and Tube 610 microphone
preamplifiers, and two pedals, the Rodent (Pro Co RAT, LM308 with its gain-bandwidth and
slew) and the Round Fuzz (germanium Fuzz Face, PNP).

Also added: the Brit Plexi (1959 Super Lead bright channel, Unicord 1970 drawing) with its
own Brit Plexi EL34 power stage.

Added 2026-09-16: the Brit AC30 (Vox AC30/6 Top Boost, cathode-biased EL84s with no
feedback loop) and the Brit DR103 (Hiwatt Custom 100, direct-coupled inverter), each with
its own power stage.

Added 2026-09-16: the Cali Rectifier (Dual Rectifier Rev F, red channel) and its 6L6
power stage. Every amplifier on the target list now exists; what is still missing is a
valve-rectifier device and a mains-voltage (variac) supply. Never expose an unavailable family
under an approximate alias.

Each hardware change requires a per-device log in docs/models with revision,
active online schematic/service/component search, cross-check, actual values,
DSP mapping, unresolved evidence and the eight-question checkpoint. Label claims
CIRCUIT DERIVED, PHYSICS DERIVED, PUBLISHED-PARAMETER DERIVED, EMPIRICALLY TUNED,
or APPROXIMATED. Existing code comments are evidence of implementation, not proof
that every claimed component agrees with its original drawing.
