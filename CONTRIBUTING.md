# Contributing

Thanks for your interest in SIM. This repo is one crate group in a constellation
of repos that build together; contributions of all sizes are welcome.

## Building and testing

This repo is self-contained and builds against the published SIM crates on
crates.io -- no extra tooling or sibling checkouts are required for the default,
hardware-free lane:

- Clone this repo and run `cargo build` and `cargo test --workspace`.
- Cross-repo dependencies resolve from crates.io; dependencies within this repo
  resolve locally.
- The loadable `sim-lib-stream-jack-provider` crate is intentionally outside the
  root workspace and is checked by manifest path.
- Native cpal hardware checks on Linux require `pkg-config` and
  `libasound2-dev`; CI installs them before running that named system-package
  gate.

## What a pull request must pass

Every PR runs these gates in CI, and they must be green before merge:

- `cargo fmt --all --check`
- `cargo metadata --no-deps --format-version 1`
- `cargo run -p xtask -- workspace-coverage --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo doc --workspace --no-deps`
- `cargo run -p xtask -- simdoc --check`
- `cargo fmt --manifest-path crates/sim-lib-stream-jack-provider/Cargo.toml --check`
- `cargo test --manifest-path crates/sim-lib-stream-jack-provider/Cargo.toml`
- `cargo clippy --manifest-path crates/sim-lib-stream-jack-provider/Cargo.toml --all-targets -- -D warnings`
- `cargo doc --manifest-path crates/sim-lib-stream-jack-provider/Cargo.toml --no-deps`
- `cargo clippy -p sim-lib-stream-cpal --all-features --all-targets -- -D warnings`
  and `cargo test -p sim-lib-stream-cpal --all-features` after the Linux native
  audio packages are installed

Please keep source and Markdown ASCII-only, and add or update tests for behavior
you change. Public APIs carry `#![deny(missing_docs)]`; document new public items.

## Sign your work (DCO)

We use the Developer Certificate of Origin, not a CLA. Add a `Signed-off-by` line
to each commit certifying you wrote the change or have the right to submit it:

```
git commit -s -m "your message"
```

This adds `Signed-off-by: Your Name <you@example.com>`. That is all we need; there
is no copyright-assignment agreement to sign.

## License

By contributing you agree that your contributions are licensed under the
repository's MPL-2.0 license (see `LICENSE`).

## Filing issues

Use the issue templates. A small reproducible example beats a long description.
Security-sensitive reports go through `SECURITY.md`, not public issues.
