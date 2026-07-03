# sim-lib-stream-cpal

In one line: A cross-platform sound-card connection for SIM, with a hardware-free default lane and an optional path to real devices.

## What it gives you

This crate connects SIM audio to cpal, a cross-platform layer that speaks to whatever sound system a machine happens to have. By default it runs a modeled audio site through a shared fake backend, so a project builds and validates with no real sound hardware and no surprises between runs. Turning on the hardware feature adds the native cpal boundary that reaches an actual device on Windows, macOS, or Linux. That native edge is the one place the crate uses low-level buffer handling, and it documents each such step so the risky part stays small, contained, and easy to review.

## Why you will be glad

- One adapter reaches sound cards across Windows, macOS, and Linux through a single layer.
- The default lane needs no real hardware, so builds and tests stay steady everywhere.
- The native, low-level portion is confined to one place and clearly documented.

## Where it fits

This is one of SIM's stream-host backends -- the crates that connect the audio graph to a real or modeled sound device. Where the ALSA, CoreAudio, and ASIO crates each model one platform, cpal offers a single cross-platform route to real hardware. It gives SIM a portable way to reach a machine's sound output when a device is actually present.
