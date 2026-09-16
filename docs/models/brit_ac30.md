# Brit AC30 (Vox AC30/6 Top Boost) research log

Engineering identity: **Vox AC30/6 "Top Boost"** (AC30 Treble & Bass), the brilliant
channel through the Top Boost valve and its tone stack, into four cathode-biased EL84s
with no negative feedback.
- Stable ids: `amp_vox_ac30_tb` (`circuit`), `power_ac30_el84` (`power_amp`).
- Display: Brit AC30 / AC30 EL84.

## Eight-question checkpoint (2026-09-16, before code)

1. **Revision.** The integrated Top Boost AC30/6 (Treble & Bass), the circuit as the
   factory drew it from the late 1960s to 1974. Not the AC30/4, not the Normal/Bass/Treble
   models without Top Boost, not the 1961-63 add-on unit's own layout, not the solid-state
   AC30, not the modern reissues.
2. **Original schematic found?** Yes, two factory drawings that agree:
   - **Dallas Music Industries Ltd, "Vox AC30 (Treble & Bass) Circuit Diagram / Vox AC30
     Amplifier (Top Boost)", Sc/V/1313, 24 June 1974** — typeset, complete, legible.
   - **Vox Sound Limited, unnumbered AC30/6 Top Boost sheet, 12 February 1971** — hand
     drawn, same values.
   Both from [voxac30.org.uk's circuit diagram archive](https://www.voxac30.org.uk/vox_ac30_circuit_diagrams.html),
   which reproduces original JMI/VSL/Dallas documents.
3. **Best source.** The 1974 Dallas sheet for values; the 1971 VSL sheet as the
   cross-check.
4. **Cross-check and conflicts.**
   - The two drawings agree on every value the model uses, except:
     - output valve cathode resistor: **50 ohm** (Dallas) vs 47 ohm 6 W (VSL 1971);
     - its bypass: 250 uF 25 V (Dallas) vs 200 uF (VSL).
   - Rectifier: the 1971 sheet has a **valve rectifier** (V10) with a 10 H 100 mA 450 ohm
     choke; the 1974 sheet has **BY133 silicon diodes** with a 10-20 H 100 mA choke. JMI
     amplifiers used a GZ34. The model takes the valve-rectifier case.
   - [voxac30.org.uk](https://www.voxac30.org.uk/vox_ac30_top_boost_circuit.html): the Top
     Boost was "a 'lift' from a circuit adopted by Gibson in the mid 1950s for its
     Vanguard GA-77"; "treble and bass controls - 1M log potentiometers".
   - [Ampbooks' AC30 analysis](https://www.ampbooks.com/mobile/classic-circuits/vox-ac30/):
     "The phase inverter is a long tailed pair in its purest form", "The 47k ohm resistor
     creates a large 'tail'", "there is no negative feedback in the Vox AC30!", "The AC30
     output transformer is 4k ohm plate-to-plate", "The 47 ohm cathode resistor".
   - The cathode resistor is widely reported as 50 ohm or 82 ohm depending on year; the
     factory schematic's 10 V of bias at idle is what the model is checked against.
5. **Signal path and values (Dallas Sc/V/1313).**

```text
BRILLIANT inputs: two jacks, 68k each (R1, R2), 1M to ground (R3)
V1 ECC83 (both channels share one valve): R4 1k5 cathode with C1 2.5 uF; plates R5, R6 220k
brilliant plate -- C2 .047uF -- VOLUME 500k log, C41 100pF from its top to the wiper
wiper -> Top Boost ECC83:
  first half: R75 100k plate (from the node behind R76 10k with C42 32 uF), R77 1k5 cathode,
              unbypassed
  second half: cathode follower, grid on the first half's plate, plate on the same node,
              R78 56k cathode to ground
tone stack from the follower's cathode:
  C43 50pF -> TREBLE 1M log (top); wiper = output
  R79 100k -> slope node
  slope -- C44 .022uF -- treble bottom = BASS 1M log top (wired as a variable resistor)
  slope -- C45 .022uF -- the node the bass wiper returns to
  that node -- R80 10k -- ground   (the fixed resistor where a Fender stack has its Middle)
treble wiper -- R9 220k --+
NORMAL volume wiper -- R7 220k --+-- C5 .047uF -- phase inverter grid A (R8 1M leak)
phase inverter ECC83, long-tailed pair:
  R16 1k2 from the leak junction to the joined cathodes, R15 47k from that junction to ground
  grid B: R17 1M leak, C7 .047uF to the third channel's volume (at zero: AC ground)
  plates R18, R19 100k
outputs: C6 .15uF and C9 .15uF to the EL84 grids, R21 and R20 220k leaks to ground
CUT: VR4 250k log in series with C10 .0047uF, across the two inverter outputs
power: 4 x EL84, R22/R23/R27/R28 1k5 grid stoppers, R25/R26/R29/R30 100 ohm screen
       stoppers, shared cathode R24 50 ohm with C11 250 uF (10 V at idle on the factory
       sheet), no negative feedback
output transformer: 4k plate to plate, 16 / 8 / common secondary
supplies: valve rectifier, 10 H 450 ohm choke, 32 uF reservoirs; R11 22k and R10 22k
          droppers with C8 and C4 8 uF; R76 10k with C42 32 uF for the Top Boost stage
```

6. **To be modeled exactly.** The brilliant channel from the jack to the treble wiper: V1's
   stage, the volume with its bright capacitor, the Top Boost gain stage and cathode
   follower, the whole stack, the 220k mixers, and the supply droppers. In the power stage:
   the long-tailed pair with its 47k tail, the 100k plates, the .15 uF couplings, the 220k
   leaks, the Cut network, the 1k5 stoppers, the 100 ohm screen stoppers, four EL84s on a
   shared cathode resistor with its bypass, and **no feedback loop**.
7. **Approximated and estimated.**
   - Supplies: an ideal source behind the drawing's droppers. HT 330 V ESTIMATED (the
     drawings carry no voltages); the valve rectifier and choke become the supply's series
     resistance, so the sag is a resistance rather than a diode's curve.
   - The normal channel's triode is not built; its idle current is a resistor on the shared
     cathode, and its volume is at zero, so it passes no signal but still loads the mixer.
   - The output transformer's inductance, copper and core are ESTIMATED for a 4k a-a,
     30 W transformer; no winding data was found.
   - Cathode resistor 50 ohm with 250 uF (the Dallas values).
   - The Cut control rests mostly out; the panel has no knob for it yet.
   - Not modeled: the normal and vib/trem channels, the tremolo/vibrato, the second input
     jack of each channel.
8. **Why.** Transformer internals and supply voltages are not published, and a valve
   rectifier is not a part the solver has.

## Solver work this needs

`power.rs` had one shape of output stage: fixed bias from a negative supply, a
feedback loop to the tail, and a presence control in it. The AC30 has none of those. Added:
- `cathode_bias` / `cathode_bypass`: a shared cathode resistor with its capacitor, with the
  grid leaks returning to ground instead of to a bias supply;
- `feedback: 0.0`: no loop and no presence network;
- `pi_tail_lower: 0.0`: the tail resistor returns to ground, which is where a pair with no
  feedback puts it;
- `cut_pot` / `cut_cap`: a rheostat and capacitor across the inverter's two outputs.
