# sim-lib-audio-graph-live

In one line: The engine that runs your effect chain in real time, feeding a sound card without stutters or dropouts.

## What it gives you

This is the piece that takes a prepared signal path and runs it live, inside the tight timing window a sound card gives you. It sets up its memory ahead of time so the steady playback path never stops to allocate, which is what keeps audio free of clicks and gaps. Control changes and audio blocks travel through bounded queues, so a slow moment in one part cannot flood the others. It also carries a transport snapshot tied to the stream clock, so playback position stays honest. In short, it is the bridge between a designed chain and the moment-to-moment callback that actually makes sound.

## Why you will be glad

- Your effect chain runs live with the memory work done up front, so playback stays smooth.
- Control and audio traffic move through sized queues that will not overrun each other.
- Transport position tracks the real stream clock, keeping timing trustworthy while you play.

## Where it fits

This crate sits between the audio graph and the sound-card backends. The graph core describes the chain, a stream host provides the callback, and this runner marries the two for mono and stereo playback. When a SIM project moves from offline preview to real-time sound, this is the layer that carries it there.
