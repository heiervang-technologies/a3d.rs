# a3d

[![Rust CI](https://github.com/heiervang-technologies/a3d.rs/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/heiervang-technologies/a3d.rs/actions/workflows/rust-ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

GPU-accelerated ASCII 3D rendering engine written in Rust.

Renders 3D models (OBJ, STL, GLB) as real-time ASCII art in your terminal, using
wgpu compute shaders or a CPU rasterizer and glam for vector math.

Inspired by [voxcii](https://github.com/ashish0kumar/voxcii), rebuilt from scratch in Rust for memory safety, performance, and extensibility.

![a3d rendering a spinning dog model as real-time ASCII 3D](assets/demo.gif)

> Recorded with [VHS](https://github.com/charmbracelet/vhs) from `assets/demo.tape` — `a3d models/dog.stl --color --fg 00ddff`.

## Features

- Real-time 3D rendering with ASCII shading
- Z-buffered scanline triangle rasterization with plane-equation depth interpolation
- Orthographic projection with terminal aspect-ratio correction
- OBJ format support (with MTL material colors)
- STL format support (binary and ASCII)
- GLB format support (geometry, node transforms, vertex/material colors)
- Auto-rotation with golden ratio oscillation
- Interactive mode (arrow keys + zoom)
- Automatic terminal resize handling
- Aspect ratio correction for non-square terminal characters
- wgpu GPU rendering with automatic CPU fallback

## Requirements

- Rust 1.85+ (2024 edition)
- For GPU rendering, a wgpu adapter with 64-bit atomic min/max; without one,
  a3d transparently falls back to the CPU renderer
- A terminal emulator

### Arch Linux

```bash
sudo pacman -S vulkan-icd-loader vulkan-headers
# For NVIDIA:
sudo pacman -S nvidia nvidia-utils
# For AMD:
sudo pacman -S vulkan-radeon
```

## Installation

```bash
git clone https://github.com/heiervang-technologies/a3d.rs.git
cd a3d.rs
cargo build --release
```

The binary will be at `target/release/a3d`.

## Usage

```bash
# Auto-rotating model (default)
a3d model.obj

# Interactive mode
a3d model.obj --interactive

# With ANSI true color
a3d model.obj --color

# Custom FPS and zoom
a3d model.obj --fps 60 --zoom 2.5

# Render one terminal-independent text frame (useful for scripts/status bars)
a3d model.glb --frame 12x10 --time 4.5
```

### CLI Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `<MODEL>` | | (required) | Path to OBJ, STL, or GLB file |
| `--fps` | `-f` | 30 | Target frames per second |
| `--interactive` | `-i` | off | Manual rotation with arrow keys |
| `--zoom` | `-z` | 1.0 | Initial zoom level, from 0.1 to 10 |
| `--color` | `-c` | off | Enable ANSI 24-bit true color output |
| `--gpu` | | auto | Force GPU rendering (error if no adapter) |
| `--cpu` | | auto | Force CPU rendering |
| `--fg` | | model | Foreground color as hex, e.g. `ff6600` (implies `--color`) |
| `--bg` | | none | Background color as hex, e.g. `1a1a2e` (implies `--color`) |
| `--frame` | | off | Render one plain-text frame at `WIDTHxHEIGHT`, then exit |
| `--time` | | 0 | Animation time in seconds for `--frame` |

By default a3d briefly benchmarks both available renderers against the loaded
model and current terminal size, then uses the faster one. `--cpu` and `--gpu`
bypass the benchmark and force a backend. In automatic mode, GPU setup,
allocation, resize, or readback failures switch to CPU rendering. With `--gpu`,
these failures exit with an error after restoring the terminal.

Single-frame mode always uses the CPU renderer and writes only the requested
ASCII frame to standard output. It does not initialize a terminal or GPU, so it
can be embedded in status bars and other non-interactive scripts.

### Controls (interactive mode)

| Key | Action |
|-----|--------|
| Arrow keys / `hjkl` | Rotate model |
| Scroll wheel | Zoom in/out |
| `+` / `=` | Zoom in |
| `-` | Zoom out |
| `c` | Toggle color on/off |
| `q` / `Esc` / `Ctrl-C` | Quit |

### Debug logging

```bash
RUST_LOG=info a3d model.obj
```

## Development

```bash
cargo test --all-features -- --test-threads=1
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
```

- **`gpu_compare`** checks the sample scene with a tolerance of less than 0.5%
  differing ASCII cells (1% for the zoom sweep). Synthetic colored/overlapping
  triangles require exact ASCII output and color/luminance differences at most
  `1e-5`. GPU tests skip when no suitable adapter is present.
- **`gpu_failures`** checks resource-limit errors and deliberate device loss,
  including CPU rendering after a GPU error.
- **Regenerate the CPU snapshots** after an intentional rendering change:
  ```bash
  cargo test --test gen_snapshots -- --ignored
  ```
- **Re-record the demo GIF** (requires [VHS](https://github.com/charmbracelet/vhs)
  + ffmpeg; see the optimization note in `assets/demo.tape`):
  ```bash
  vhs assets/demo.tape
  ```

CI runs `fmt --check`, `clippy -D warnings`, rustdoc, the full test suite, and a
separate Rust 1.85 MSRV check on every pull request.

## Architecture

```
src/
├── main.rs              # CLI and interactive render loop
├── lib.rs               # Shared CPU/GPU entry points + CPU rasterizer
├── gpu/
│   ├── context.rs       # wgpu device/queue/adapter initialization
│   ├── pipeline.rs      # GPU pipeline + synchronous framebuffer readback
│   └── raster.wgsl      # Compute shader entry points
├── model/
│   ├── loader.rs        # OBJ, STL, and GLB file parsing
│   └── mesh.rs          # Vertex/Mesh types, normalization
├── render/
│   ├── ascii.rs         # ASCII luminance ramp mapping
│   ├── camera.rs        # Orbital camera with perspective projection
│   └── framebuffer.rs   # Depth buffer + character grid
└── terminal/
    └── display.rs       # crossterm terminal rendering + input
```

### Rendering Pipeline

```
Model file (OBJ/STL/GLB)
    │
    ▼
Load & parse ──▶ Center + fit to unit sphere
    │
    ▼
┌────────────── Render Loop ──────────────────┐
│ CPU: transform/light/rasterize triangles     │
│                  or                         │
│ GPU: transform → atomic depth → shade        │
│      → synchronous framebuffer readback      │
│                    │                         │
│                    ▼                         │
│          ANSI terminal output                │
│                    │                         │
│                    ▼                         │
│          input + frame-rate limit            │
└──────────────────────────────────────────────┘
```

### CPU/GPU output contract

Both backends use flat shading and the first vertex's color for each triangle.
Floating-point rounding and GPU depth quantization mean arbitrary scenes are
not guaranteed byte-identical. Characters, colors, and luminances are the
shared outputs; GPU rendering leaves `Framebuffer::depth` at infinity because
it does not read back depth. Use CPU rendering when CPU-side depth compositing
is required.

`RasterPipeline::new`, `RasterPipeline::resize`, and `render_frame_gpu` return
`Result` so library callers can handle GPU failures or render on CPU. GPU render
errors clear the framebuffer. A resize limit error preserves the pipeline;
a GPU allocation/device error requires discarding it and building a new one.

### ASCII Luminance Ramp

Surface brightness maps to characters from dark to bright:

```
 . , ' : ; ! + * = # $ @
◄─── dark              bright ───►
```

Luminance is computed as `dot(-face_normal, light_direction) * 0.5 + 0.5`, giving a [0, 1] range that indexes into this 12-character ramp.

## Tech Stack

| Concern | Crate | Purpose |
|---------|-------|---------|
| GPU compute | `wgpu` | Portable compute shaders and device access |
| CPU math | `glam` | SIMD-accelerated vectors and matrices |
| Terminal | `crossterm` | Raw mode, cursor control, input events |
| OBJ loading | `tobj` | Wavefront OBJ + MTL parsing |
| STL loading | `stl_io` | Binary and ASCII STL parsing |
| GLB loading | `gltf` | Binary glTF geometry and scene parsing |
| CLI | `clap` | Argument parsing with derive macros |
| GPU data | `bytemuck` | Safe transmutes for GPU buffer data |
| Logging | `log` + `env_logger` | Debug and info logging |

## Roadmap

See [Issue #1](https://github.com/heiervang-technologies/a3d.rs/issues/1) for the full roadmap including:

- **M1:** Core rendering MVP ✅
- **M2:** GPU compute pipeline (wgpu shaders) ✅
- **M3:** Advanced rendering (Phong lighting, shadows, color) — current
- **M4:** Scene graph and animation
- **M5:** Performance and polish
- **M6:** Extensions (WASM, export, plugins)

## Acknowledgements

- Inspired by [voxcii](https://github.com/ashish0kumar/voxcii) by ashish0kumar.
- The bundled sample mesh `models/dog.stl` is the author's own model. See
  [`models/README.md`](models/README.md) for details.

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) and our
[Code of Conduct](CODE_OF_CONDUCT.md). Security issues: please follow
[SECURITY.md](SECURITY.md).

## License

a3d is licensed under the [MIT License](LICENSE). This covers both the source
code and the bundled sample model (`models/dog.stl`), which is the author's own
work — see [`models/README.md`](models/README.md).
