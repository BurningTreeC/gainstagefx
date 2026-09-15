# DSP implementation notes

Reuse modified nodal analysis: named validated netlists, trapezoidal capacitor /
inductor companions, separate DC solve, Newton iterations with voltage limiting,
line search, finite fallback, and nonlinear device advance. partition.rs caches
Schur reductions for linear internal nodes; runtime refactor workspaces are
preallocated. Device/core histories and reduction caches have explicit copy APIs.

See ARCHITECTURE.md for rate, oversampling, buffering, stereo and latency policy.
New routing must update process, DC sharing, diagnostic counters, master routing,
reset, oversampled rate sync and dormant-channel state copying together.

Regression policy: record deterministic baseline output before altering routing;
compare Matched against that output rather than merely comparing two calls through
the new code. Include impulse, sweep, stepped sine, seeded white/pink noise at
multiple amplitudes/rates. Existing guitar fixture enables later listening/DI tests.
Hardware models currently force 1x; do not silently alter their rate policy during
a compatibility refactor. New nonlinear paths need explicit alias/CPU evaluation.
