# Microphone model status

No existing microphone, polar, proximity, distance, angle, or placement processing.
No microphone profiles have yet passed the mandatory source checkpoint.

Planned separation: immutable profile + physical 3D placement + per-channel state.
General first-order polar magnitude |a+b cos(theta)|, with signed pressure retained
where needed for rear-lobe phase. Position must interact with speaker radiation;
distance controls fractional physical travel and array weighting, with safe bounded
attenuation. Profile filters run at control rate, not expensive per-sample updates.
Dual-mic composition must retain relative delays and allow deliberate polarity and
alignment choices. No dual-mic UI or processing is currently implemented.
