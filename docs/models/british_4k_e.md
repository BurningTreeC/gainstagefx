# British 4K E (SSL SL 4000 E channel mic amplifier, 82E01) research log

Engineering identity: **Solid State Logic SL 4000 E channel amplifier card 82E01**, microphone
amplifier section (transformer input, the version before the transformerless 82E149).
Stable id `pre_ssl_4000e` (`circuit` parameter). Display: British 4K E.

Status: **IMPLEMENTED 2026-09-15** (`src/circuits/console_e.rs`), tested in
`tests/console_e.rs`.

## Eight-question checkpoint

1. **Revision.** 82E01 with the Jensen input transformer, as on SSL drawing T82001-71
   revision 16 (11/84, "REDRAWN"). Not the 1979 "early version" (82E01 old), and not the
   82E149 transformerless amplifier of later E and all G consoles (GroupDIY, "SSL 4K E mic
   input").
2. **Original schematic found?** Yes. The SSL drawings:
   - T82001-71 rev 16, "CHANNEL AMPLIFIER CARD", copyright SSL, via
     [ka-electronics](http://www.ka-electronics.com/images/SSL/ssl_82E01.pdf);
   - 82E01-710-8208-12 "Channel Amplifier Card", via
     [gyraf.dk classic schematics](https://gyraf.dk/schematics/schematics.html).
3. **Best source.** T82001-71 rev 16.
4. **Cross-check.** 82E01-710-8208-12 draws an earlier mic amplifier:
   - a single "TDA NB" (TDA1034NB, the 5534) with a 250 k log gain rheostat and R12 820 ohm;
   - R7 100 k secondary load, and the note "C2 NOT USED WITH JENSEN TRANSFORMER".
   Rev 16 replaces this with the two-op-amp arrangement below. The two drawings agree on
   the transformer input, R9, R10/R11, C3/C4 and the phantom feed. Transformer data:
   [Jensen JT-115K-E data sheet](https://www.jensen-transformers.com/wp-content/uploads/2014/08/jt-115K-e1.pdf)
   and the older
   [JE-115K-E sheet](http://www.technicalaudio.com/pdf/Jensen_Transformers/JE_RE_JT_Mic_Input_Transformers/Jensen_JE-115K-E_mic_input_xfmr.pdf).
5. **Signal path and values (rev 16).**

```text
MIC +8/-7 -- (SW1 pad out) -- Jensen primary (RED/BRN); R1/R2 6K81 to phantom node 2, C1 3u3
secondary (Y) -> T1 +IN; R7 150K to ground (C2 100pF, R8 link: "BBC ONLY")
T1 (5534): -IN (pin 19) -- R9 619R -- C3 100uF -- ground
           -IN -- R10 10K -- node (C4 100uF to ground) -- R11 10K -- T1 OUT
T1 OUT -- C54 220uF (R72 100K across) -- C55 220uF -- MIC GAIN wiper (pin 10); R73 1M to +ve
MIC GAIN 10K LIN: pin 19 (T1 -IN) ... pin 33
pin 33 -- R69 619R -- T9 -IN; T9 +IN -- R70 5K11 -- ground
T9 (5534): R71 10K7 || C56 100pF from OUT to -IN; OUT -- R46 100R -- channel
supplies: +/-20V via R14/R15 22R to +/-18.5V (C6/C7 10uF); "T2-T9 5534N"
```

   Gain, from the values: G = (1 + Ra / (619 || 10k)) x 10,700 / (10k - Ra + 619), with Ra
   the track between wiper and pin 19. That is 0 dB at Ra = 0 and 49.9 dB at Ra = 10 k.
   The Jensen adds 20 dB, for a total of **20 to 70 dB**. The model measures 18.8 to
   68.6 dB into its 150 ohm source.
6. **Exactly modeled.** Every resistor and capacitor in the mic path above, both gain
   stages, the phantom feed resistors (phantom off), and the Jensen's ratio, DC
   resistances and 150 k load.
7. **Approximated.**
   - 5534s are ideal op-amps with a ±16 V rail (5534 swing on ±18 V). GBW and slew are not
     modeled; the 22 pF parts are the 5534's external compensation.
   - SW1 pad out; phantom off; R73 omitted; BBC-only parts omitted.
   - One microphone leg is taken as ground (unbalanced model of a balanced input).
   - Jensen core and effective secondary capacitance are **ESTIMATED**, fitted to the data
     sheet:

     | Figure | Data sheet | Model |
     |---|---|---|
     | Voltage gain at the terminals | 19.75 dB | 19.8 dB |
     | −3 dB low | 2.5 Hz | −3.13 dB at 2.5 Hz |
     | −3 dB high | 90 kHz | −3.09 dB at 90 kHz |
     | THD, 20 Hz at −2.5 dBu | 1 % | 1.01 % |

     Constants: henry 9.7, knee 0.0109, sharpness 3, shunt 85 pF. The low-level 0.065 % at
     20 Hz / −20 dBu (hysteresis) is not reproduced; the model gives 0.018 %.
8. **Why.** No transistor-level 5534 model is warranted for a stage built to stay linear
   until it clips. Transformer internals are not published beyond the data sheet.
