# JACK provider load

This recipe records the loadable JACK provider path. The provider builds as a
cdylib-capable crate, the host grants `audio.provider.native`, and the provider
entry registers a JACK-shaped audio site through the loader-acquired provider
library.

Manual command:

```bash
sh bin/simctl meta-build
cargo build --manifest-path .meta-workspace/Cargo.toml \
  -p sim-lib-stream-jack-provider --lib
cargo test --manifest-path .meta-workspace/Cargo.toml \
  -p sim-lib-stream-jack-provider modeled_provider_loads_through_audio_provider_host
```

Expected list-style output:

```text
provider: audio/provider:jack
capability: audio.provider.native granted
loader: LoaderRegistry::load_lib
entry: sim_audio_provider_v1
site: audio/provider/jack-modeled
fallback: modeled
```
