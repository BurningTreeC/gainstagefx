# The DI: design

Status: **BUILT 2026-10-01** (`DrySource`, `dry_source` and `dry_route`; `tests/di.rs`
and the plugin's split test). **Decided** by the owner: **(a)**, the Mix knob's dry source made
selectable, **with (c)'s split output** as an option; no iron on the DI for now. Everything
else follows from the hardware and from how the chain is built.

Bass is recorded as an amplifier, a cabinet and a microphone **and** a DI, and the two are
blended or kept on separate tracks. The roadmap asks for the DI's tap to be chosen
deliberately -- raw input, after the pedal, or after the preamplifier -- documented, and
offered as a blend if that is useful. This is that choice.

## What a DI is, in the rigs this plugin builds

There is no one DI. There are three, and each is a piece of hardware this plugin models or
will:

| Where it taps | The hardware | What it carries |
|---|---|---|
| **Input** | a DI box between the bass and the amplifier (passive or active), its thru to the amp | the instrument, untouched |
| **After the pedal** | a DI pedal's balanced output -- the SansAmp Bass Driver DI's XLR, while its parallel jack feeds the amp the raw bass (`docs/models/bass_driver.md`) | the pedal's processed signal |
| **After the preamplifier** | an amplifier's own direct out -- the GK 800RB's, "after the effects loop", before the boost, the masters and the crossover (`docs/models/american_800rb.md`); the 1975 SVT has none | the preamplifier, EQ and all, nothing of the power stage |

None of them carries the power stage, the speaker or the microphone: that is what makes it
a DI.

## Where those points are in the chain

```text
input x drive volts -> [pedal -> hand-off] -> gain circuit -> graphic -> | power stage
       ^                       ^                                      ^  |  -> make-up -> ...
     INPUT                   PEDAL                               PREAMP     -> tone -> cabinet
```

- **Input** already exists: it is the dry path behind the Mix knob (`Chain::delayed_dry`) --
  the trimmed, gated input, delayed to line up with the processed output. Mix blends it
  linearly; two factory presets use it (0.45, 0.6).
- **Pedal** is the pedal's output after the hand-off, inside the oversampled half
  (`Front`).
- **Preamp** is the sample stream the front half hands the back half (`mid`), the gain
  circuit's output -- ahead of the power amplifier, as an amplifier's direct out is. A
  circuit that names its own direct-out node is read there instead: the 800RB's, at the
  return, ahead of the boost that ends the circuit here (`Gain::direct_out`).

## How a tap is made

1. **Capture.** The front half writes the tap's oversampled samples into a second buffer
   beside `mid`, preallocated to the same `MAX_OVERSAMPLING` length, so the back half --
   which may be running on the pipeline's worker -- receives it the way it receives `mid`.
2. **Decimate.** A second decimator, identical to the main one (the same FIR), brings it
   to the host rate. At 1x there is nothing to do.
3. **Align.** Through the same latency pad as the processed path. The DI and the power
   stage's output are then sample-aligned; the cabinet and microphone after it add only
   what they physically add -- the microphones' delays are measured from the nearest
   arrival (each microphone's own with Mic Align on, the nearer microphone's otherwise;
   `AcousticStage`), so the DI lines up with the first arrival, as an engineer aligning a
   DI to a close microphone would have it.
4. **Level.** Each tap is returned to the plugin's own units, so a DI at its rest is the
   level of the thing it replaces:
   - Input: as it is.
   - Pedal: the pedal's output volts divided by what the pedal is fed (`pedal_into`). A
     pedal's level at rest is unity through it, so the DI then equals the input.
   - Preamp: the circuit's output volts times the make-up for the drive, divided by what
     the circuit is fed, without the Master's lift -- the hardware's direct out is ahead of
     it. But the make-up was measured through the voice's whole path, its own power stage
     included (the SVT's 6550s are 40 dB of it: the first build read -39.7 dB), so the
     tap also takes the **Bypass** power selection's measured trim (`POWER_TRIM_DB`'s
     first column): the preamplifier at the level the whole amplifier has at the
     calibration drive, exactly as Power Amp = Bypass is levelled. Measured: within 0.5 dB
     of the input for the Clean, SVT, REDD.47 and V76 voices (`tests/di.rs`).
   - Preamp, for a circuit whose own direct out sits ahead of its last stage
     (`Gain::direct_out`): the 800RB's is "after the effects loop", and the boost after it
     is the circuit's last stage here. The tap reads that node, not the output, so the
     boost moves the amplifier and not the DI. The make-up cannot level it -- the boost
     clips at the calibration level and the direct out does not, so the make-up lifted it
     11 dB too far and turned it down as the volume came up -- so it is levelled as the
     pedal tap is, by what the circuit does to the signal on the way there: its measured
     small-signal gain to the node at the drive (`src/direct_out.rs`, written by
     `examples/directout.rs`). Measured: -0.3 dB re the input; the boost from 0.2 to 0.9
     moves the amplifier 8 dB and the DI 0.1 dB (`tests/di.rs`).
5. **Cost.** Nothing when the DI is not in use. In use, one decimator and one buffer copy
   a sample -- no solver work at all, since the tap reads what is computed anyway.
6. **Safety.** No allocation on the audio thread (`assert_no_heap` covers it), tested at
   every rate, and with the pipeline on and off to the bit.

## The two decisions

**1. How the DI reaches the output.**

- **(a) As the Mix knob's dry signal.** One new selector, *Dry from*: Input (today's
  behaviour, the default), Pedal, Preamp. Mix then blends the DI against the amplifier as
  it already blends the input. No existing session or preset changes. *Recommended*: it is
  what the Mix knob already is, and it adds one control, not three.
- **(b) A separate DI level** added to the output, beside Mix. More controls, and two ways
  to blend a clean signal in.
- **(c) Either of those, plus a split output:** the amplifier on the left, the DI on the
  right, for recording them to separate tracks and blending (or re-amping) later. The cost
  is true stereo: in split mode the instance runs mono, one chain, and the microphones'
  pans no longer apply.

**2. Whether the DI gets its own iron.** A passive DI box is a transformer; an active one
and an amplifier's direct out are op-amps. The Iron list already models output
transformers; reusing it for the DI is possible but is a separate choice. *Recommended*:
not now -- a clean DI first.
