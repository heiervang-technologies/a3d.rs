<!-- Thanks for contributing! Keep PRs focused and small where possible. -->

## What & why

<!-- What does this change and why? Link any related issue (e.g. Closes #123). -->

## Checklist

- [ ] `cargo fmt --all` is clean
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test --all-features -- --test-threads=1` passes
- [ ] Tests added/updated for behavior changes
- [ ] Snapshots regenerated (`cargo test --test gen_snapshots -- --ignored`) if rendering output changed
