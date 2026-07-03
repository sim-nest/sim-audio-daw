# sim-lib-plugin-wasm

In one line: A way to load audio plugins packaged as WebAssembly and run them as ordinary SIM effects.

## What it gives you

This crate lets SIM host audio plugins that ship as WebAssembly modules -- small, portable, sandboxed programs that run the same across machines. The default build reads a plugin's manifest and exposes its type surface, so you can inspect what a module offers without pulling in any runtime. Turning on the runtime feature adds a wasmtime-backed host that instantiates the module and runs it as a SIM plugin instance and audio graph node, taking its place in a signal chain like any built-in block. An optional feature also allows a capability-checked fallback that hands loading to an explicit host provider, keeping control over what is allowed to run.

## Why you will be glad

- You can run portable, sandboxed audio plugins that behave the same on any machine.
- The default build inspects a plugin's manifest without loading a heavier runtime.
- A hosted WebAssembly plugin plugs into the audio graph as an ordinary effect node.

## Where it fits

This is one of SIM's plugin-format adapters, built on the shared plugin core. Where the CLAP, LV2, and VST3 crates model established desktop formats, this one opens SIM to WebAssembly plugins and their portable, isolated packaging. It is how a module built elsewhere can become a working voice inside a SIM signal path.
