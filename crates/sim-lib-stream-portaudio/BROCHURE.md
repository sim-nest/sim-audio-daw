# sim-lib-stream-portaudio

In one line: A stand-in for PortAudio, the portable sound layer, that lets SIM play and be tested without a PortAudio install.

## What it gives you

This crate models PortAudio, a widely used portable sound layer that runs across many operating systems, entirely in Rust. It binds to no library and touches no real device; instead it serves a steady, made-up default output that behaves the same every run. It presents PortAudio devices in the shared stream-host shape SIM uses across backends, bridges host callbacks into the audio graph, and records the backend priority the sound bootstrap follows when choosing a device. Because there is nothing to install and no hardware to open, an audio project builds and validates anywhere, while a native adapter can later fill the same model from a real PortAudio installation.

## Why you will be glad

- Your audio path can be exercised with no PortAudio package and no sound hardware.
- A steady made-up default output keeps playback tests repeatable across every run.
- The backend priority the bootstrap uses is recorded, so device choice stays clear.

## Where it fits

This is one of SIM's stream-host backends -- the crates that connect the audio graph to a real or modeled sound device. It is the portable fallback that a plain Ubuntu sound bootstrap reaches for behind native PipeWire support, sitting beside the ALSA, JACK, and cpal adapters. It gives SIM a dependable, cross-platform sound surface to build and validate against.
