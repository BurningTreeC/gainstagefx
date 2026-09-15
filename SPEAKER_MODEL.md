# Speaker model status

Audit: absent. power.rs ends in 8 ohms (Mark/5150), 4 ohms (Twin).
No electrical/mechanical/thermal speaker state exists. Legacy cabinet filters are
not a speaker load. The netlist infrastructure supports R/L/C and transformers,
so a passive electromechanical equivalent can be stamped into the power solve.

Planned linear relation: Z = Re + s Le + Bl^2 / (Rms + s Mms + 1/(s Cms)).
Cone velocity follows force Bl*i through the mechanical impedance. A sealed box
adds stiffness; do not double-count its compliance in the acoustic output stage.
Profile values require published-driver research before implementation. Missing
T/S values must be marked estimates; no guessed manufacturer data is installed.
Optional excursion/thermal nonlinearities remain unimplemented.
