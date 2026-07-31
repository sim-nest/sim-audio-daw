# sim-lib-audio-dsp

In one line: Realtime-safe sound sources, sample-rate conversion, and ready-made shaping blocks for an audio signal path.

## What it gives you

This is a stocked shelf of the signal tools an audio project reaches for again and again. It supplies PolyBLEP bandlimited oscillators and a fixed-state polyphase resampler alongside level and pan controls, click-free smoothing, one-pole, biquad, and state-variable filters, delay, chorus, flanger, vibrato, compression, gating, limiting, soft clipping, and waveshaping. Every callback component is plain math with its storage prepared in advance, so it behaves the same in a preview and a live graph.

## Why you will be glad

- You reach for a filter, delay, or compressor and it is already built and tested.
- Oscillator discontinuities and sample-rate changes have explicit anti-aliasing policy.
- Callback state is fixed before processing, with caller-owned resampler output storage.
- Level and knob changes glide instead of clicking, thanks to built-in smoothing.
- The same math runs in an offline preview and in a live stream, so nothing surprises you.

## Where it fits

These processors are the realtime signal layer of the SIM audio system. Other parts of the toolkit wire them into a graph, host them as plugins, or drive them from a sound card. Offline phase vocoding and loudness stay with the music renderer; this crate owns callback-safe generation, rate conversion, and shaping.
