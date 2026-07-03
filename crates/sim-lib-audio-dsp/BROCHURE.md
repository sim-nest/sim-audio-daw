# sim-lib-audio-dsp

In one line: A kit of ready-made sound-shaping blocks -- gain, filters, delay, and dynamics -- that you drop into a signal path.

## What it gives you

This is a stocked shelf of the tone-shaping tools an audio project reaches for again and again. You get level and pan controls, gentle smoothing so knob moves never click, one-pole and biquad and state-variable filters for carving tone, a delay line with comb and all-pass building blocks, and modulation colours like chorus, flanger, and vibrato. On the dynamics side there is a compressor, gate, limiter, soft clipper, and waveshaper. Every block is plain math with no hardware attached, so it sounds the same whether you preview it or run it live.

## Why you will be glad

- You reach for a filter, delay, or compressor and it is already built and tested.
- Level and knob changes glide instead of clicking, thanks to built-in smoothing.
- The same math runs in an offline preview and in a live stream, so nothing surprises you.

## Where it fits

These processors are the raw voices of the SIM audio system. Other parts of the toolkit wire them into a graph, host them as plugins, or drive them from a sound card; this crate simply supplies the sound-shaping stages themselves. When a SIM project needs to color, filter, or tame audio, it draws on the blocks kept here.
