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

Still not implemented: American 312, Tube 610, Green 9, Rodent, Round Fuzz, Brit 800
preamp, Cali Rectifier, DR103, AC30, EL34 Hi-Headroom and EL84 power families,
physical speakers/cabinets and all microphones. Never expose an unavailable family
under an approximate alias.

Each hardware change requires a per-device log in docs/models with revision,
active online schematic/service/component search, cross-check, actual values,
DSP mapping, unresolved evidence and the eight-question checkpoint. Label claims
CIRCUIT DERIVED, PHYSICS DERIVED, PUBLISHED-PARAMETER DERIVED, EMPIRICALLY TUNED,
or APPROXIMATED. Existing code comments are evidence of implementation, not proof
that every claimed component agrees with its original drawing.
