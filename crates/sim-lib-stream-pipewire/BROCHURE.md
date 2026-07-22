# sim-lib-stream-pipewire

In one line: A stand-in for PipeWire, the modern Linux sound server, that lets SIM connect and be tested without a running daemon.

## What it gives you

This crate models PipeWire, the sound and media server behind current Linux desktops, entirely in Rust. It binds to no library and needs no running daemon; instead it serves steady, made-up nodes and ports that behave the same every run. It presents provider-reported PipeWire nodes and the visible SIM client ports, folds quantum, sample-rate, and latency details into the shared stream configuration, and bridges made-up process callbacks into the audio graph and its PCM queues. Because none of this leans on a live PipeWire session, an audio project builds and validates anywhere, while native adapters use the same model for real PipeWire registry events.

## Why you will be glad

- Your audio path can be exercised with no PipeWire daemon and no library installed.
- Buffer size, sample rate, and latency are carried into the shared stream configuration.
- Made-up nodes and ports behave the same every run, keeping tests steady and repeatable.

## Where it fits

This is one of SIM's stream-host backends -- the crates that connect the audio graph to a real or modeled sound device. It covers the modern Linux sound path, sitting beside the ALSA, JACK, PortAudio, and cpal adapters. It gives SIM a PipeWire-shaped surface to build and validate against with no daemon in the loop.
