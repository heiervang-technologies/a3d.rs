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
- `src/gpu/` — wgpu context (Vulkan), 3-pass compute pipeline (transform → depth → shade) in `raster.wgsl`
- `src/model/` — OBJ (tobj) and STL (stl_io) loading, mesh normalization
- `src/render/` — camera (orbital, perspective), framebuffer (z-buffer + char grid), ASCII luminance mapping
- `src/terminal/` — crossterm raw mode display, keyboard input

## Key Design Decisions

- GPU compute pipeline (wgpu) renders by default; CPU rasterization is the fallback when no adapter is available
- Plane-equation (triangle-normal) z-depth interpolation within triangles
- Aspect ratio correction: `width / (height * 1.8)` for terminal characters
- Golden ratio oscillation for smooth non-repeating auto-rotation
- Model normalization to [-1, 1] on load

## Conventions

- Rust 2024 edition
- No unsafe code
- `glam` for all vector/matrix math (SIMD-accelerated)
- `bytemuck` for GPU buffer data (Pod/Zeroable derives)

## Roadmap

Tracked in GitHub Issue #1. M1 (core rendering) and M2 (GPU compute pipeline) are complete. Next: M3 (advanced rendering — Phong lighting, shadows, color).

## Test Models

The bundled sample mesh is `models/dog.stl` (the author's own Meshy-generated
model, MIT-licensed with the repo). See `models/README.md`. Tests and the demo
default to it; the engine works with any OBJ/STL file.
