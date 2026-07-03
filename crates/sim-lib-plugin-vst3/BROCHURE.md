# sim-lib-plugin-vst3

In one line: A way to describe a SIM effect in the shape of a VST3 plugin, worked out entirely in Rust with no native binary.

## What it gives you

This crate lets a SIM audio processor be laid out in the VST3 plugin format, the standard many studios rely on. It works as a modeled tier: pure Rust, no external SDK, no compiled `.vst3` file. From a SIM effect it produces VST3-style descriptors, including the audio buses that carry sound in and out, and it translates those into the shared plugin description SIM uses everywhere else. Native hosting and binary export are not part of this crate; a scope report spells out plainly what an outside native provider would need. What you get today is a clean, testable way to express an effect in VST3 terms without any SDK or platform hosting in the loop.

## Why you will be glad

- You can shape an effect in VST3 terms without a Steinberg SDK or a compiled binary.
- Audio buses and parameters are described for you and map onto SIM's shared plugin form.
- A scope report states honestly what native hosting would require, so nothing is hidden.

## Where it fits

This is one of SIM's plugin-format adapters, built on the shared plugin core. It bridges the VST3 vocabulary and the SIM audio graph, letting an effect be expressed in the format studios know best. Beside the CLAP, LV2, and WebAssembly crates, it rounds out the plugin shapes SIM can describe.
