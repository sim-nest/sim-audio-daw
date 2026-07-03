# sim-lib-stream-alsa

In one line: A stand-in for the low-level Linux ALSA sound system that lets SIM audio run and be tested without real hardware.

## What it gives you

This crate models ALSA, the direct sound layer beneath most Linux audio, entirely in Rust. It does not touch a real sound card; instead it serves steady, made-up PCM devices that behave predictably every run. It understands the device names Linux users know -- `default`, `hw:` and `plughw:` -- reports what those devices can do, hands back a plan for opening them, and bridges playback into the audio graph while recording captured sound as stream packets. Because there is no real driver and no library to install, an audio project can be built and checked anywhere, and a native adapter can later fill the same model from actual hardware.

## Why you will be glad

- Your audio path can be exercised on Linux with no sound card and no ALSA package.
- Familiar device names like `default` and `hw:` are recognized just as you would type them.
- The made-up devices behave the same on every run, so tests stay steady and repeatable.

## Where it fits

This is one of SIM's stream-host backends -- the crates that connect the audio graph to a real or modeled sound device. It covers the direct Linux PCM path, sitting beside the JACK, PipeWire, PortAudio, and cpal adapters. It gives SIM a dependable ALSA-shaped surface to run and validate audio against.
