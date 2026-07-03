# JACK hardware smoke

This recipe records the guarded native JACK provider smoke path. The test
enumerates JACK provider sites through the loadable provider entry and returns
without opening host hardware unless `SIM_JACK_HARDWARE_SMOKE=1`.

Manual command:

```bash
SIM_JACK_HARDWARE_SMOKE=1 \
  cargo test -p sim-lib-stream-jack-provider --features jack-hardware \
  -- --nocapture jack_hardware_smoke
```

Expected output:

```text
test tests::jack_hardware_smoke ... ok
```
