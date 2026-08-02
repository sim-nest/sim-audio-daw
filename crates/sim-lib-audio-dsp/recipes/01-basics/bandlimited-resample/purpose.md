# Realtime bandlimited source and rate conversion

Select `OscillatorPolicy` and `BandlimitPolicy` before preparing a
`BandlimitedOscillator`. Construct `PolyphaseResampler` with fixed phase, tap,
channel, and per-call input bounds, ask it for the required caller-owned output
size, and keep the coefficient bank and history ring unchanged in callbacks.

The checked Rust specimen measures folded-harmonic suppression, impulse gain,
passband response, downsampling alias rejection, fail-closed buffer bounds, and
steady-state allocation capacity. Offline phase-vocoder and loudness policy
remain with `sim-lib-sound-render`.
