# sim-lib-stream-jack-provider

Loadable JACK audio placement provider for SIM stream hosts.

The default `model` feature is pure Rust and registers a deterministic modeled
JACK site through the shared audio-provider registrar. The `jack-hardware`
feature enables the native JACK module and the exported provider symbol used by
loadable cdylib hosts.
