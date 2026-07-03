# sim-lib-plugin-clap

In one line: A way to shape a SIM effect so it looks and behaves like a CLAP plugin, and to run CLAP-shaped effects inside SIM.

## What it gives you

This crate lets a SIM audio processor wear the CLAP plugin format. By default it works entirely in Rust, with no external SDK and no compiled binary: it builds CLAP-style descriptors that spell out an effect's identity and parameters, and it hosts a SIM processor shaped to that descriptor so it runs in-process. That keeps the whole thing testable on any machine, with no plugin scanning or native hosting required. An optional host feature adds a provider seam for capability-checked loading through an explicit host, so you can extend toward outside CLAP content without pulling an SDK binding into the core.

## Why you will be glad

- You can describe an effect as a CLAP plugin without touching a native SDK.
- The default path runs and tests in-process, so no plugin scanner or hardware is needed.
- An optional provider seam leaves room to load outside CLAP content under explicit control.

## Where it fits

This is one of SIM's plugin-format adapters, built on the shared plugin core. It translates between the CLAP world and the SIM audio graph, so an effect can travel in a format many hosts already understand. Alongside the LV2, VST3, and WebAssembly crates, it widens the range of plugin shapes SIM can speak.
