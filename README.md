# sim-audio-daw

Build an audio workstation in Rust: shape sound with pure-Rust DSP, patch a live
signal graph that renders identically in tests and on real hardware, and host
CLAP, LV2, and VST3 plugins -- all as inspectable SIM data.

sim-audio-daw is a repository in the SIM constellation. SIM is an expandable Rust
runtime built around a small protocol kernel plus loadable libraries; this repo
holds the audio, plugin, and DAW libraries. The kernel does not own DAW or
plugin policy: audio graphs, plugin descriptors, live back-ends, and DAW
sessions are all library data surfaces loaded on top of the kernel.

## Example

Add the audio graph crate and render a block offline -- the same graph that later
drives a live host callback:

```bash
cargo add sim-lib-audio-graph-core
```

```rust
use sim_lib_audio_graph_core::{Graph, PrepareConfig, ProcessBlock, Processor};

#[derive(Default)]
struct CopyNode;

impl Processor for CopyNode {
    fn prepare(&mut self, _cfg: PrepareConfig) {}
    fn reset(&mut self) {}
    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let frames = block.frames as usize;
        for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
            output[..frames].copy_from_slice(&input[..frames]);
        }
    }
}

let mut graph = Graph::new();
graph.add_node("copy", Box::<CopyNode>::default(), 1, 1).unwrap();
graph.prepare(48_000, 4).unwrap();

// Deterministic offline render returns the processed buffers:
let output = graph.process_offline(&[vec![0.25, -0.5]], 2).unwrap();
assert_eq!(output, vec![vec![0.25, -0.5]]);
```

(from the crate-level doctest in
`crates/sim-lib-audio-graph-core/src/lib.rs:8`.)

## What this repo provides

A hardware-free, deterministic audio stack that can also bind to native sound
back-ends:

- A pure Rust processor graph that renders the same way in tests and live.
- F32 PCM as the audio currency between processors, hosts, and back-ends.
- The in-tree cpal stream adapter and modeled platform stream adapters for ALSA,
  ASIO, CoreAudio, JACK, PipeWire, and PortAudio.
- Plugin descriptor, export, and host-adapter surfaces for CLAP, LV2, and VST3.

Audio data and patches round-trip as SIM expressions, so graphs, plugin state,
and sessions are inspectable, replayable, and agent-readable data rather than
opaque host state.

## Crates

### Audio graph

- `sim-lib-audio-graph-core` -- pure Rust audio processor graph primitives. The
  core graph is hardware-free: callers implement `Processor`, prepare the graph
  with `PrepareConfig`, and process `ProcessBlock` values. Graphs render offline
  for deterministic tests and previews (`Graph::process_offline`), and patches
  round-trip as SIM expressions.
- `sim-lib-audio-graph-live` -- preallocated live runner that connects the
  processor protocol to host callback queues. It provides bounded control and
  audio queues, stream-clock-backed transport snapshots, and an allocation-free
  steady-state process path for mono and stereo graphs.
- `sim-lib-audio-dsp` -- reusable pure Rust DSP processors for the audio graph,
  including gain, filter, delay, dynamics, modulation, and oversampling.

### Plugins

- `sim-lib-plugin-core` -- common plugin descriptors, parameter descriptors,
  plugin state, and graph adapters. Descriptors carry stable parameter ids and
  normalized/plain parameter mapping; `PluginState` round-trips through SIM
  expressions.
- `sim-lib-plugin-clap` -- CLAP-shaped descriptor and adapter surface for audio
  graph processors.
- `sim-lib-plugin-lv2` -- LV2-shaped descriptor/port surface that lowers to the
  common plugin descriptor.
- `sim-lib-plugin-vst3` -- VST3-shaped export/bus descriptor surface that lowers
  to the common plugin descriptor.

- `sim-lib-plugin-wasm` -- WebAssembly plugin host support for the shared audio
  graph processor model.

### Stream-host adapters

The default workspace contains hardware-free modeled platform adapters plus the
cpal adapter's modeled lane:

- `sim-lib-stream-cpal` -- cpal stream-host adapter with a deterministic modeled
  lane, capability-gated hardware enumeration, and the modeled provider entry
  used by the loadable audio-provider contract.
- `sim-lib-stream-alsa` -- modeled Linux ALSA stream-host adapter.
- `sim-lib-stream-asio` -- modeled Windows ASIO stream-host adapter.
- `sim-lib-stream-coreaudio` -- modeled macOS CoreAudio stream-host adapter.
- `sim-lib-stream-jack` -- modeled JACK stream-host adapter.
- `sim-lib-stream-pipewire` -- modeled PipeWire stream-host adapter.
- `sim-lib-stream-portaudio` -- modeled PortAudio stream-host adapter.

The `sim-lib-stream-host` provider contract keeps platform-specific native
audio back-ends behind explicit capability and feature gates. The default
workspace stays hardware-free unless a caller explicitly enables the cpal
hardware feature.

`sim-lib-stream-jack-provider` is a loadable JACK placement provider. Its
default lane is modeled and FFI-free, and its `jack-hardware` feature keeps the
native JACK binding isolated in the provider cdylib. Hosts load the provider
through `LoaderRegistry::load_lib` under the `audio.provider.native` capability;
when the provider is absent, placement resolves to the modeled site.

## How the pieces fit

```text
processors (audio-dsp, plugin-*) --> audio-graph-core (Patch)
                                          |
                                          v
                                    audio-graph-live
                                          |
                                          v
                              stream-host adapters plus providers
```

The same `Patch` renders offline for deterministic tests, drives the live runner
against a host callback, and serializes into a session or a topology package.

## Feature families

Relevant root feature families across the constellation include
`audio-graph-core`, `audio-graph-live`, `audio-dsp`,
`plugin-core`, `plugin-clap`, `plugin-lv2`, `plugin-vst3`, `plugin-wasm`, and
the modeled stream-adapter families.

Source-level rustdoc is the primary API reference for these crates.

## Validation

These commands validate the default hardware-free lane:

```bash
cargo run -p xtask -- workspace-coverage --check
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo run -p xtask -- simdoc --check
```

`sim-lib-stream-jack-provider` is a loadable provider lane outside the root
workspace and is validated by manifest path. Native cpal hardware validation is
a named system-package gate; on Linux it requires `pkg-config` and
`libasound2-dev` before running `cargo clippy -p sim-lib-stream-cpal
--all-features --all-targets -- -D warnings` and `cargo test -p
sim-lib-stream-cpal --all-features`.

## Documentation Lanes

`cargo run -p xtask -- simdoc` builds the public documentation lanes:

- API docs: `target/doc/`
- Agent cards: `docs/agents/cards.jsonl` and `docs/agents/card-index.json`
- Human docs: `docs/humans/`
- Diagrams: `docs/diagrams/src/` and `docs/diagrams/generated/`

The same command writes split contract files under `docs/generated/`. Everything
under `docs/` is generated; do not hand-edit it.
