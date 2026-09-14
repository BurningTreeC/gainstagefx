# clap-wrapper compatibility patch

Upstream remains pinned to v0.16.0, commit
`1cca996e96f29ab2be7ae9f8cfe532bbc92e1dd6`.

`auv3-switch-scope.patch` adds a lexical scope to the containing app's
microphone-permission switch case. Xcode 16.4 rejects the original Objective-C++
ARC block because jumping to `default` enters the lifetime of a block strongly
capturing `self`. The braces do not change runtime behavior, DSP, plugin GUI,
parameter/state handling or signing.

`cmake/PatchClapWrapper.cmake` checks the source's SHA-256 before and after
applying the patch. It runs on every AUv3 configuration, so cached FetchContent
checkouts receive the fix as well. It accepts an already patched file and
fails on any unexpected source revision. On a future upstream update, review
whether the fix is included before changing the pin or removing this patch.
The patch modifies only the downloaded dependency inside the build directory.
