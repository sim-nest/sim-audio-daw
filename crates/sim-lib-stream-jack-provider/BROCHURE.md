# sim-lib-stream-jack-provider

In one line: A registrable JACK sound provider for SIM, modeled by default and hardware-gated when real JACK is available.

## What it gives you

This crate packages the JACK sound connection as a registrable provider that a host can add on demand. By default it is pure Rust and registers a steady, modeled JACK-shaped audio site through the shared provider registrar, so a project gains a JACK placement without linking to JACK or opening any hardware. The hardware feature enables the native JACK module for hosts that register the Rust provider entry directly. It is the seam that lets JACK support stay separate and swappable rather than baked in.

## Why you will be glad

- JACK support arrives as a registrable unit, added when wanted rather than always present.
- The default modeled site registers with no JACK library and no hardware attached.
- The hardware feature keeps real JACK behind the same provider registration path.

## Where it fits

This is the provider counterpart to the JACK stream-host model in SIM. Where the JACK adapter defines the shape, this crate delivers it as a registrable provider, both as a steady modeled site and, when built for it, as native JACK behind the shared host registration path. It is how SIM's shared audio-provider registrar comes to know about JACK.
