# sim-lib-stream-jack-provider

In one line: A loadable add-on that registers a JACK sound connection for SIM, either modeled by default or wired to real JACK.

## What it gives you

This crate packages the JACK sound connection as a loadable provider -- a piece a host can pick up and register on demand. By default it is pure Rust and registers a steady, modeled JACK-shaped audio site through the shared provider registrar, so a project gains a JACK placement without linking to JACK or opening any hardware. Turning on the hardware feature enables the native JACK module and exports the symbol that a loadable plug-in library needs, so an outside host can bring the real thing into play. It is the seam that lets JACK support be added as a separate, swappable unit rather than baked in.

## Why you will be glad

- JACK support arrives as a loadable unit, added when wanted rather than always present.
- The default modeled site registers with no JACK library and no hardware attached.
- The hardware feature exports the exact symbol a loadable host needs for real JACK.

## Where it fits

This is the loadable counterpart to the JACK stream-host model in SIM. Where the JACK adapter defines the shape, this crate delivers it as a registrable provider, both as a steady modeled site and, when built for it, as native JACK behind an exported symbol. It is how SIM's shared audio-provider registrar comes to know about JACK.
