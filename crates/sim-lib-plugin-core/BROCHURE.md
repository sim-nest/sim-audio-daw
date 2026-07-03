# sim-lib-plugin-core

In one line: The shared language for describing an audio plugin -- its name, its knobs, and the settings you save and recall.

## What it gives you

This is the common ground every plugin format in SIM stands on. It describes a plugin in plain terms: what it is, how many channels it handles, and the list of parameters it exposes. Each parameter carries its range and a default, and it can translate between the plain value a musician reads and the normalized value a host expects. It also holds plugin state, so a patch of settings can be captured, written out, and loaded back exactly as it was. Because it adapts a plugin onto the audio graph node contract, a described plugin can take its place in a signal chain like any other block.

## Why you will be glad

- One consistent way to describe knobs, ranges, and defaults across every plugin format.
- Settings save and reload precisely, so a recalled patch matches what you dialed in.
- A described plugin drops straight into the audio graph as an ordinary node.

## Where it fits

This crate is the hub the plugin adapters share. The CLAP, LV2, VST3, and WebAssembly crates all speak in these descriptors and state records, and the graph hosts them through the adapter defined here. When SIM needs a plugin to look the same no matter its origin, this is where that shape is set.
