# cpal hardware smoke

This recipe records the guarded real-device smoke path for the cpal stream
adapter. The test enumerates cpal output sites, opens the first site through the
shared audio router, queues one silent PCM packet, and closes the stream.

Manual command:

```bash
SIM_CPAL_HARDWARE_SMOKE=1 \
  cargo test -p sim-lib-stream-cpal --features cpal-hardware \
  -- --nocapture cpal_real_site_smoke
```

Expected output:

```text
test tests::cpal_real_site_smoke ... ok
```

Without `SIM_CPAL_HARDWARE_SMOKE=1`, the test prints a skip message and returns
without opening host audio hardware.
