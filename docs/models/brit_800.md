# Brit 800 (Marshall JCM800 2203 preamp) research log

Engineering identity: **Marshall JCM800 Lead Series 2203, 100 W master volume**, preamp
section per the original Marshall drawing dated 24-4-81 ("MASTER VOLUME PREAMP, used on
2103, 2203, 2104, 2204"). Proposed stable id `amp_jcm800_2203`. Display: Brit 800.
Matched power stage: Brit EL34 (`power_2203_el34`, see [brit_el34.md](brit_el34.md)).

## Eight-question checkpoint (2026-09-15, before implementation)

1. **Revision.** 1981 2203 export drawings; HIGH input with the LOW jack empty.
   This is not the 2204 (50 W), not the standard-lead 1959/1987 preamp on the upper half of
   the same sheet, and not 1988+ board revisions.
2. **Original schematic found?** Yes. Marshall manufacturer drawings:
   [preamp](https://www.drtube.com/schematics/marshall/jcm800pr.gif) and
   [power/PSU](https://www.drtube.com/schematics/marshall/jcm800pw.gif), plus the
   project-local `docs/schematics/jcm_800_2203.pdf`.
3. **Best source.** The 1981 drawings above.
4. **Cross-check.**
   - The [1988 manufacturer 2203 drawing](https://el34world.com/charts/Schematics/files/Marshall/Marshall_jcm800_lead_mstvol_100w_2203.pdf),
     already used for the power section, is to be cross-checked for the preamp values
     before code. The middle-pot value on the 1981 sheet is hard to read (see 5).
   - Supply voltages are not printed on the preamp sheet. The power sheet gives B+ 470 V
     and screen 468 V, "average only".
   - A measured 100 W JCM800 reported on the Marshall Amp Forum gives V1 rail 286 V and
     V2 rail 293 V, with 470 V B+. That is SECONDARY and marks the preamp rails ESTIMATED.
5. **Signal path and values, 1981 drawing.**

```text
HIGH jack -- 1M to ground -- 68k -- V1b grid (ECC83)
  V1b: plate 100k to node 5; cathode 2k7 bypassed .68 uF
  plate -- .022 uF -- (LOW jack switch contacts) -- 470k || 470 pF -- Preamp Volume 1M log (top)
           .001 uF from volume top to wiper (bright); wiper -> V1a grid
V1a: plate 100k to node 5; cathode 10k, unbypassed
  plate -- .022 uF -- 470k || 470 pF -- V2a grid; 470k grid to ground
V2a: plate 100k to node 6; cathode 820 ohm, unbypassed
  plate direct -> V2b grid (cathode follower)
V2b: plate to node 6; cathode 100k to ground
  tone stack from the V2b cathode:
    470 pF -> Treble 220k lin (top) ; wiper -> Master Volume 1M log (top) -> power stage
    33k slope -> node X ; X -- .022 uF -> Treble bottom / Bass 1M lin top (bass wired as rheostat)
    Bass bottom -> Middle pot top ; Middle bottom to ground ; X -- .022 uF -> Middle wiper
supplies: node 6 -- 10k 1W -- node 5, each with 50 uF ; node 6 fed from PI node A via 10k 1W
```

   Middle pot: the drawing reads "2?K LIN". The common 2203 value is 25 k, and 22 k also
   appears in secondary lists. **Unresolved until the 1988 drawing is checked.**
6. **Exactly modeled (planned).**
   - All four triode sections, the coupling and divider networks, bright caps and
     cathode networks.
   - The cathode follower driving the passive Marshall stack, in one nonlinear netlist.
   - The handoff at the treble wiper to the power stage's own master pot, which matches
     how the Brit EL34 spec begins.
7. **Approximated (planned).**
   - Rails as ideal sources behind their droppers: 286 / 293 V from the secondary
     measurement.
   - LOW jack and jack-switch contact resistance omitted.
   - The master pot's load on the stack is seen through the existing fixed-impedance
     handoff between netlists, as for the other amplifiers.
8. **Why.** The rail voltages are not on the drawing. Separate preamp and power netlists are
   the project's existing cost structure (see `voice::build_power`).

Status: research checkpoint only; not yet implemented.
