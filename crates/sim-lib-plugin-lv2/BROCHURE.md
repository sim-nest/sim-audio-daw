# sim-lib-plugin-lv2

In one line: A way to shape a SIM effect so it looks and behaves like an LV2 plugin, and to run LV2-shaped effects inside SIM.

## What it gives you

This crate lets a SIM audio processor take on the LV2 plugin format, the open standard common on Linux. By default it works purely in Rust, with no external SDK and no compiled binary: it builds LV2-style descriptors, complete with the audio and control ports that describe how sound and parameters flow, and hosts a SIM processor shaped to match so it runs in-process. That keeps everything testable without an LV2 development package installed. On Linux, an optional host feature adds a provider seam for capability-checked loading through an explicit host, so you can reach toward outside LV2 content without adding an SDK binding to the core.

## Why you will be glad

- You can present an effect as an LV2 plugin without installing an LV2 toolkit.
- Ports for audio and control are laid out for you, matching how LV2 hosts expect them.
- The default path runs in-process and stays testable on any machine.

## Where it fits

This is one of SIM's plugin-format adapters, resting on the shared plugin core. It maps between the LV2 world and the SIM audio graph, so an effect can move in a format at home across Linux audio software. With the CLAP, VST3, and WebAssembly crates beside it, it broadens the plugin shapes SIM understands.
