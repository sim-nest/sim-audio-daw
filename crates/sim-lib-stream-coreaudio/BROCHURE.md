# sim-lib-stream-coreaudio

In one line: A stand-in for Apple's CoreAudio sound system that lets SIM audio run and be tested without a Mac or its frameworks.

## What it gives you

This crate models CoreAudio, the sound layer beneath macOS, entirely in Rust. It binds to none of Apple's frameworks and touches no real device; instead it serves steady, made-up PCM devices that behave the same on every run. It presents those devices in the shared stream-host shape SIM uses across backends, and bridges host callbacks into the audio graph so playback and capture flow through the normal path. This exists for native CoreAudio coverage in cases where the portable PortAudio path is not enough, while keeping the build free of Apple hardware and toolkits. It handles sound only; MIDI is left to a separate adapter so this crate stays focused on audio devices.

## Why you will be glad

- Your audio path can be exercised on any machine, with no Mac and no Apple frameworks.
- Made-up devices behave the same every run, keeping tests steady and repeatable.
- It stays focused on sound, leaving MIDI to its own adapter for a cleaner boundary.

## Where it fits

This is one of SIM's stream-host backends -- the crates that connect the audio graph to a real or modeled sound device. It covers the macOS path, sitting beside the ALSA, JACK, PipeWire, and PortAudio adapters. It gives SIM a CoreAudio-shaped surface to build and validate against without any Apple gear on hand.
