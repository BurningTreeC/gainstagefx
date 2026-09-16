# Brit Plexi (Marshall 1959 Super Lead 100 W, EL34, c. 1969-70) research log

Engineering identity: **Marshall JMP 1959 Super Lead**, the non-master 100 W head with
four EL34s, in the late-plexi / early-aluminium-panel circuit.
- Stable ids: `amp_marshall_1959` (`circuit`), `power_1959_el34` (`power_amp`).
- Display: Brit Plexi / Brit Plexi EL34.

## Eight-question checkpoint (2026-09-15, before code)

1. **Revision.** The 1959 Super Lead as Unicord (Marshall's US distributor) drew it:
   "MARSHALL 1959", drawing 70-6-11, issue B, July 1970, 3 x ECC83 + 4 x EL34.
   - This is the plexi-era circuit players mean: bright channel on a 2.7 k / .68 uF
     cathode with a .0022 uF coupling cap and a 5000 pF bright cap; 470 k mixers with
     500 pF across the bright one; V2a bypassed.
   - Not the 1967 KT66 units, not the late-70s/80s 1959 (see 4), not the 1959HW/SLP
     reissues, not the 1959T tremolo.
2. **Original schematic found?** Yes, three distributor/manufacturer drawings:
   - Unicord 70-6-11 (July 1970), via [Dr.Tube, Marshall JMP](https://www.drtube.com/marshall-jmp/)
     (`/schematics/marshall/1959u.gif`).
   - Marshall "100 WATT AMPLIFIER TYPE #2 C.1967", hand-drawn with node voltages
     (`/schematics/marshall/100w-67.gif`, same page). KT66 era.
   - Marshall "1959 STD Preamp" and "1959 STD Output Stage & PSU", Jim Marshall Products
     Ltd, issue 4, 18-5-88 / 10-5-88
     ([schematicheaven](https://schematicheaven.net/marshallamps/jcm800_superlead_100w_1959.pdf)).
     This is the only one that marks **pot laws**.
3. **Best source.** Unicord 70-6-11 for values; the 1988 Marshall sheet for the pot laws;
   the 1967 sheet for supply voltages.
4. **Cross-check.**
   - [Rob Robinette, How the Plexi and JCM800 work](https://robrobinette.com/How_the_Marshall_JCM800_Works.htm):
     - bright channel "2.7k cathode resistor" with ".68uF bypass cap" and ".0022uF
       coupling cap";
     - normal channel 820 ohm with "330uF" (Unicord: 250 uF);
     - "470pF Bright Cap" across the mixer (Unicord: 500 pF);
     - the Low inputs "cut in half" by the two 68 k resistors; High input 1 M.
   - What the 1988 sheet changed (recorded, not used):
     - bright coupling C5 .022 uF;
     - bright cap C7 220 pF;
     - mixer cap C6 470 pF;
     - V2a R12 820 ohm **with no bypass capacitor**;
     - C4 100 pF on V1b;
     - Treble 220k lin, Middle 22k lin;
     - NFB R24 100 k, Presence 22 k, tail R25 4k7 (the 2203 output stage).
   - Unicord vs 1967: 20 k/1 W (Unicord) vs 8.2 k from the HT to the inverter node;
     otherwise the same topology.
5. **Signal path and values (Unicord 70-6-11).**

```text
HIGH input, bright ("High Treble") channel, nothing in LOW:
  jack: 1M to gnd; 68k || 68k (LOW jack switch parallels them) -> V1b grid
  V1b ECC83: 100k plate from V1 node; 2.7k || .68uF cathode
  plate -- .0022uF -- VOL I 1M log (top); .005uF top->wiper (bright cap)
  wiper -- 470k || 500pF -- V2a grid
  normal channel VOL II at 0: its 470k mixer from V2a grid to ground
  V2a ECC83: 820 (1K) || .68uF cathode; 100k plate from V2 node
  V2b cathode follower: grid on V2a plate, plate on V2 node, 100k cathode to gnd
  stack: 500pF to TREBLE 250k lin top; 33k slope; .022uF slope->treble bottom/bass top;
         BASS 1M log rheostat; MIDDLE 25k lin, .022uF slope->middle wiper
  treble wiper -- .022uF -- phase inverter
supplies: HT -- 20k/1W -- PI node (82k/100k) -- 10k/1W -- V2 node (50uF) -- 10k/1W -- V1 node (50uF)
power: ECC83 LTP 82k/100k, 470 cathode, 10k tail, 1M leaks, .1uF second grid, 47pF plates
       .022uF -> 220k bias leaks, 5.6k stoppers, 4 x EL34, 1k screens
       NFB 47k (100k) from the 16 ohm tap to the tail; PRESENCE 5k, .1uF on its wiper
       bias: 27k(22k)/15k/8uF + 47k/27k set pot; 4 x A10D10 bridge; 2 x 100uF series
```

6. **To be modeled exactly.**
   - The bright channel from the jack to the treble wiper, every part above, including
     the normal channel's mixer loading and the direct-coupled cathode follower.
   - The power stage with the Unicord inverter, coupling, leak, stopper, screen and
     feedback/presence values, and no master.
7. **Approximated / estimated (planned).**
   - Supplies:
     - ideal 375 V at the inverter node (1967 sheet), then the drawing's 10 k droppers
       and 50 uF reservoirs, ESTIMATED;
     - plate 460 V, screen 455 V (1967 sheet);
     - bias ESTIMATED for about 70 % plate dissipation at idle.
   - V1a (normal channel) not built. Its idle current is a resistor on the V1 node, and
     its volume is at 0, so it passes no signal.
   - The output transformer uses the Brit EL34 (1.7 k CT) data, APPROXIMATED: no 1969
     Drake winding data was found.
   - Presence: the 5 k track as the tail's shunt to ground plus a 5 k rheostat and .1 uF,
     the builder's topology rather than the wiper-to-capacitor drawing, APPROXIMATED.
   - Feedback from 16 ohm through 47 k is built as 23.5 k from the 4 ohm secondary
     (the same current).
   - Tapers from the 1988 sheet (PLAUSIBLE for 1970: same pots and parts supplier
     practice).
   - Not modeled: channel jumpering, the LOW inputs, the variac/low-mains operation some
     records used.
8. **Why.** The builder's single tap and single presence topology; missing transformer
   data; one channel keeps the cost at the Brit 800's.
