# sim-audio-daw

sim-audio-daw is a repository in the SIM constellation. SIM is an expandable Rust
runtime built around a small protocol kernel plus loadable libraries; this repo
holds the audio, plugin, and DAW libraries. The kernel does not own DAW or
plugin policy: audio graphs, plugin descriptors, live back-ends, and DAW
sessions are all library data surfaces loaded on top of the kernel.

## What this repo provides

A hardware-free, deterministic audio stack that can also bind to native sound
back-ends:

- A pure Rust processor graph that renders the same way in tests and live.
- F32 PCM as the audio currency between processors, hosts, and back-ends.
- The in-tree cpal stream adapter, plus provider-facing stream-host contracts
  that keep native audio back-ends outside the default workspace.
- Plugin descriptor, export, and host-adapter surfaces for CLAP, LV2, and VST3.
- A portable, headless DAW session model with offline rendering, expression
  round-trips, and a topology launch package.

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

### DAW session

- `sim-lib-daw-session` -- a headless DAW session surface. Sessions are portable
  data: an audio graph `Patch` plus tracks, buses, clips, transport,
  plugin-chain state, and recording metadata. The crate is hardware-free and
  renders deterministic offline buffers (`render_session_offline`) for tests,
  previews, and agent inspection. It exposes browse/help Cards, expression
  round-trips, and a topology package adapter
  (`daw_session_topology_package`) that emits a launch package consumable by the
  `sim-lib-topology` engine in the `sim-runtime` repo.

### Stream-host adapter

The default workspace contains one in-tree real audio adapter:

- `sim-lib-stream-cpal` -- cpal stream-host adapter with a deterministic modeled
  lane, capability-gated hardware enumeration, and the modeled provider entry
  used by the loadable audio-provider contract.

The `sim-lib-stream-host` provider contract keeps platform-specific native
audio back-ends outside the default workspace. The default workspace stays
hardware-free unless a caller explicitly grants the provider capability or cpal
hardware feature.

`sim-lib-stream-jack-provider` is a loadable JACK placement provider. Its
default lane is modeled and FFI-free, and its `jack-hardware` feature keeps the
native JACK binding isolated in the provider cdylib. Hosts load the provider
through `LoaderRegistry::load_lib` under the `audio.provider.native` capability;
when the provider is absent, placement resolves to the modeled site.

## How the pieces fit

```text
processors (audio-dsp, plugin-*) --> audio-graph-core (Patch) --> daw-session
                                          |                            |
                                          v                            v
                                    audio-graph-live              topology launch
                                          |                          package
                                          v
                                  stream-host adapter (cpal) plus
                                      loadable providers
```

The same `Patch` renders offline for deterministic tests, drives the live runner
against a host callback, and serializes into a session or a topology package.

## Feature families

Relevant root feature families across the constellation include
`audio-graph-core`, `audio-graph-live`, `audio-dsp`,
`plugin-core`, `plugin-clap`, `plugin-lv2`, `plugin-vst3`, `daw-session`, and
the `stream-cpal` back-end selector.

Source-level rustdoc is the primary API reference for these crates.

## Validation

These commands run in the constellation workspace; only `sim-kernel` builds from
a lone clone today (see `DEVELOPING.md` in `sim-sdk`).

```bash
cargo fmt --check && cargo test --workspace && cargo clippy --workspace -- -D warnings && cargo doc --workspace --no-deps
cargo run -p xtask -- simdoc --check
```

## Documentation Lanes

`cargo run -p xtask -- simdoc` builds the public documentation lanes:

- API docs: `target/doc/`
- Agent cards: `docs/agents/cards.jsonl` and `docs/agents/card-index.json`
- Human docs: `docs/humans/`
- Diagrams: `docs/diagrams/src/` and `docs/diagrams/generated/`

The same command writes split contract files under `docs/generated/`. Everything
under `docs/` is generated; do not hand-edit it.
