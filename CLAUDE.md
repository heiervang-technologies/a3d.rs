# a3d - Project Context

## What is this?

GPU-accelerated ASCII 3D rendering engine in Rust. Renders OBJ/STL models as real-time ASCII art in the terminal.

## Build & Run

```bash
cargo build --release
cargo run -- model.obj
cargo run -- model.obj --interactive --fps 60 --zoom 2.5
RUST_LOG=info cargo run -- model.obj   # debug logging
```

## Project Structure

- `src/main.rs` — render loop, CLI args (clap), CPU triangle rasterizer
- `src/gpu/` — wgpu context (Vulkan), compute pipeline (stub, M2 milestone)
- `src/model/` — OBJ (tobj) and STL (stl_io) loading, mesh normalization
- `src/render/` — camera (orbital, perspective), framebuffer (z-buffer + char grid), ASCII luminance mapping
- `src/terminal/` — crossterm raw mode display, keyboard input

## Key Design Decisions

- CPU rasterization is the current fallback; GPU compute pipeline (wgpu) is M2
- Barycentric coordinate interpolation for z-depth within triangles
- Aspect ratio correction: `width / (height * 1.8)` for terminal characters
- Golden ratio oscillation for smooth non-repeating auto-rotation
- Model normalization to [-1, 1] on load

## Conventions

- Rust 2024 edition
- No unsafe code
- `glam` for all vector/matrix math (SIMD-accelerated)
- `bytemuck` for GPU buffer data (Pod/Zeroable derives)

## Roadmap

Tracked in GitHub Issue #1. Current: M1 (core rendering). Next: M2 (GPU compute pipeline).

## Test Models

Sample OBJ files from upstream voxcii repo at `~/ht/forks/ht-voxcii/models/` (bunny, cow, teapot, dragon).
