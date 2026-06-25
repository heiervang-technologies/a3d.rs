# Contributing to a3d

Thanks for your interest in improving a3d! Contributions of all kinds are
welcome — bug reports, fixes, features, docs, and test models.

## Getting started

```bash
git clone https://github.com/heiervang-technologies/a3d.rs.git
cd a3d.rs
cargo build
cargo run -- models/dog.stl
```

A Vulkan-capable GPU is optional: a3d falls back to the CPU rasterizer when no
adapter is available, so it builds and runs (and the test suite passes) on
machines without a GPU.

## Before opening a pull request

CI runs these on every PR and they must pass. Run them locally first:

```bash
cargo fmt --all                                          # format
cargo clippy --all-targets --all-features -- -D warnings # lint (warnings are errors)
cargo test                                               # unit + snapshot + GPU-vs-CPU
```

If you make an intentional change to rendering output, regenerate the CPU
snapshots and review the diff:

```bash
cargo test --test gen_snapshots -- --ignored
```

## Conventions

- **Rust 2024 edition**, MSRV 1.85.
- **No `unsafe`** — both crate roots are `#![forbid(unsafe_code)]`; keep it that way.
- **Document public items** — the library is `#![warn(missing_docs)]` and CI's
  `clippy -D warnings` fails on any undocumented public API.
- Use **`glam`** for vector/matrix math and **`bytemuck`** for GPU buffer data.
- **Conventional commit** messages (`feat:`, `fix:`, `refactor:`, `test:`, `chore:`, `docs:`).
- Keep PRs focused; add or update tests for behavior changes.

## Reporting bugs

Open an issue with your OS, GPU/driver (or `--cpu`), the model file (or a link),
and the exact command. A screenshot or recording helps for visual glitches.

## License

By contributing, you agree that your contributions are licensed under the
project's [MIT License](LICENSE).
