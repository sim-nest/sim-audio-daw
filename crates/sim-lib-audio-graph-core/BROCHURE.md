# sim-lib-audio-graph-core

In one line: The patch bay that connects your sound-shaping blocks into one signal flow and renders it start to finish.

## What it gives you

This is the wiring layer that turns separate processors into a working chain. You add nodes, say how many audio channels each one carries, connect them, and prepare the whole graph for a chosen sample rate and block size. Then you can render a stretch of sound offline, one block at a time, and get exactly the same result every run. There is no sound card involved and no timing luck: it is a clean, repeatable way to describe a signal path and hear what it produces. That makes it ideal for building effect chains, testing them, and previewing mixes before any hardware is in the picture.

## Why you will be glad

- You wire effects together once and the graph handles the order and buffers for you.
- Offline rendering is deterministic, so a chain sounds identical every single time.
- You can build and check a full signal path with no sound card attached.

## Where it fits

This crate is the backbone of the SIM audio system. The processors from the DSP kit become nodes here, plugin hosts plug into the same node contract, and the live runner takes a prepared graph and feeds it real audio. Anything that arranges sound in SIM starts from this graph.
