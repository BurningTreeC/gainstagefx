# Cabinet model status

Legacy Combo/Stack are five-reactance loaded networks, normalized by AC peak gain.
Combo: 110Hz/Q .9, 8.5kHz/Q .72, 14kHz pole. Stack: 85Hz/Q 1.35,
7kHz/Q .8, 11kHz pole. These are empirical filter targets, not measured drivers.
No explicit microphone coloration exists, but these are already voiced final
cabinet outputs. Preserve them as legacy options rather than adding a mic by default.

Geometry, enclosure compliance, front/rear cancellation, speaker arrays, baffle
modes and fractional propagation delays are not implemented. Future geometry must
be sourced independently and immutable; delay/filter state belongs per channel.
