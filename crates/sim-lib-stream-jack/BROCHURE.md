# sim-lib-stream-jack

In one line: A stand-in for the JACK audio routing system that lets SIM connect and be tested without a running JACK server.

## What it gives you

This crate models JACK, the pro-audio routing system that patches sound and MIDI between programs on Linux, entirely in Rust. It links to no library and needs no server; instead it serves a steady, made-up client with routable ports that behave the same every run. It models the JACK client, its audio and MIDI ports, sample-frame transport, and the callback bridge that drives an audio graph. Because none of this depends on a live JACK daemon, an audio project builds and validates anywhere, and a native adapter can later fill the same model from real JACK client and port registration.

## Why you will be glad

- Your routing setup can be exercised with no JACK server and no library installed.
- Audio and MIDI ports are modeled so you can test patching before real hardware exists.
- The made-up client behaves the same every run, keeping results steady and repeatable.

## Where it fits

This is one of SIM's stream-host backends -- the crates that connect the audio graph to a real or modeled sound device. It covers JACK's flexible routing world, sitting beside the ALSA, PipeWire, PortAudio, and cpal adapters. A companion provider crate carries the loadable native side, while this crate defines the JACK-shaped model SIM builds and tests against.
