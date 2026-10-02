# Modern Oversized 4x12 research log

The roadmap asked for a "modern oversized sealed high-gain 4x12", informed by real
cabinets with their sources documented, and not the Cali Oversized profile renamed.
Proposed stable id `cab_modern_oversized_4x12`.

Status: **RESEARCHED 2026-10-02, NOT BUILT: the premise did not hold.** The modern
high-gain makers' 4x12s are not oversized; they are the Marshall 1960's box, which the
plugin already has.

## What the makers publish

| Cabinet | H x W x D | Shell | Notes |
|---|---|---|---|
| Revv 4x12 (V30) | 30 x 30 x 14 in (762 x 762 x 356 mm), 85 lb | 18 mm Baltic birch | closed, 4 degree slanted baffle, fixed baffle |
| EVH 5150III 4x12 straight | 30 x 30 x 14 in, 88 lb | 7-ply 5/8 in birch/pine | closed, G12 EVH |
| Friedman 412 | 30 x 30 x 14 in, 86 lb | Baltic birch, tongue and groove | closed |
| Diezel 412FV | 29.75 x 29 x 14.25 in (756 x 737 x 362 mm), 100 lb | multi-ply Baltic birch | front-loaded, V30 |
| Bogner Uberkab 412 | 29.75 x 29.75 x 14 in, 92 lb | multi-ply birch | front-loaded, 2 V30 + 2 G12T-75 |
| *in the plugin:* Brit 1960 4x12 | 755 x 770 x 365 mm | 15.9 mm plywood | slanted (8 % of the volume) |
| *in the plugin:* Cali Oversized 4x12 | 836 x 765 x 362 mm | -- | Mesa Rectifier Standard, DOCUMENTED |
| *in the plugin:* Oversized 4x12 | 850 x 800 x 380 mm | 18 mm plywood | generic, no hardware claim |

(DOCUMENTED: the makers' product pages and dealers' copies of their specification
sheets, 2026-10-02.)

## Why nothing was built

Every modern maker above builds the 1960's footprint to within 3 % in each dimension.
Their interior volumes come out close to the Brit 1960's: the Revv's, with 18 mm walls
and its baffle slanted 4 degrees over its whole height, about 0.155 m^3 against the
1960's 0.164 m^3 (DERIVED from the dimensions; the slant takes 8 % of the Revv's box,
as the 1960's angled top takes 8 % of its own) -- 6 % less air, which moves a sealed
box's resonance by a few per cent at most. What differs is the shell -- Baltic birch
and 18 mm walls against 15.9 mm plywood, front-loaded baffles on two of them -- which
this model reaches only through the panels' thickness and material: a plate's modes
scale with its thickness, so 18 mm raises them some 13 % (DERIVED), a shift in the
panels' small contribution rather than in the box's response. A profile made from these numbers would be the **Brit V30
4x12** under another name, which is the thing the roadmap said not to do with the Cali
Oversized.

The oversized class itself -- a taller, deeper box than the 1960 -- is the Mesa
Rectifier Standard, which the plugin has (Cali Oversized, documented dimensions), and
the generic Oversized 4x12.

**What the modern chain uses:** the Brit V30 4x12 (the modern makers' footprint, with
the speaker most of them ship) or the Cali Oversized 4x12 (the one genuinely oversized
design). If a measured difference between a modern birch cabinet and a 1960 is found --
an impedance curve or a response measured on both -- that is the evidence a separate
profile would need.
