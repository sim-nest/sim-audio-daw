# sim-lib-plugin-wasm

`sim-lib-plugin-wasm` loads WebAssembly audio plugins as SIM plugin instances
and audio graph processors.

The crate is feature gated. The default build exposes the manifest parser and
type surface without a WebAssembly runtime dependency. Enabling `wasm-plugin`
adds the wasmtime-backed processor host. Enabling `clap-host` adds a
capability-checked CLAP fallback loader that delegates instantiation to an
explicit host provider.
