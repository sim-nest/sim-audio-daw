## What this changes

<!-- One or two sentences on the change and why. -->

## Checklist

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo metadata --no-deps --format-version 1` passes
- [ ] `cargo run -p xtask -- workspace-coverage --check` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo doc --workspace --no-deps` passes
- [ ] `cargo run -p xtask -- simdoc --check` passes
- [ ] `sim-lib-stream-jack-provider` standalone fmt/test/clippy/doc checks pass
- [ ] Native cpal hardware checks pass when Linux `pkg-config` and `libasound2-dev` are available
- [ ] Tests added/updated for the behavior changed
- [ ] Source and Markdown are ASCII-only
- [ ] Commits are signed off (DCO: `git commit -s`)
