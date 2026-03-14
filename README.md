# a3d

GPU-accelerated ASCII 3D rendering engine written in Rust.

Renders 3D models (OBJ, STL) as real-time ASCII art in your terminal, using wgpu compute shaders (Vulkan) for rasterization and glam for SIMD-accelerated math.

Inspired by [voxcii](https://github.com/ashish0kumar/voxcii), rebuilt from scratch in Rust for memory safety, performance, and extensibility.

## Features

- Real-time 3D rendering with ASCII shading
- Z-buffered triangle rasterization with barycentric interpolation
- Perspective projection with configurable camera
- OBJ format support (with MTL material colors)
- STL format support (binary and ASCII)
- Auto-rotation with golden ratio oscillation
- Interactive mode (arrow keys + zoom)
- Automatic terminal resize handling
- Aspect ratio correction for non-square terminal characters
- wgpu GPU context initialization (Vulkan backend)

## Requirements

- Rust 1.85+ (2024 edition)
- A Vulkan-capable GPU and driver
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

# Custom FPS and zoom
a3d model.obj --fps 60 --zoom 2.5
```

### CLI Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `<MODEL>` | | (required) | Path to OBJ or STL file |
| `--fps` | `-f` | 30 | Target frames per second |
| `--interactive` | `-i` | off | Manual rotation with arrow keys |
| `--zoom` | `-z` | 3.0 | Initial camera distance |

### Controls (interactive mode)

| Key | Action |
|-----|--------|
| Arrow keys | Rotate model |
| `+` / `=` | Zoom in |
| `-` | Zoom out |
| `q` / `Esc` | Quit |

### Debug logging

```bash
RUST_LOG=info a3d model.obj
```

## Architecture

```
src/
├── main.rs              # Render loop, CLI, CPU rasterizer
├── gpu/
│   ├── context.rs       # wgpu device/queue/adapter initialization
│   └── pipeline.rs      # GPU compute pipeline (stub)
├── model/
│   ├── loader.rs        # OBJ and STL file parsing
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
Model file (OBJ/STL)
    │
    ▼
Load & parse ──▶ Normalize to [-1, 1]
    │
    ▼
┌─────────── Render Loop (30 FPS) ───────────┐
│                                             │
│  Clear framebuffer (depth = ∞)              │
│       │                                     │
│       ▼                                     │
│  For each triangle:                         │
│    ├─ Compute face normal (cross product)   │
│    ├─ Directional lighting (dot product)    │
│    ├─ Map luminance → ASCII char            │
│    ├─ Project vertices (perspective)        │
│    └─ Rasterize with z-buffer               │
│       │                                     │
│       ▼                                     │
│  Render to terminal (crossterm)             │
│       │                                     │
│       ▼                                     │
│  Handle input / frame-rate limit            │
└─────────────────────────────────────────────┘
```

### ASCII Luminance Ramp

Surface brightness maps to characters from dark to bright:

```
 . , ' : ; ! + * = # $ @
◄─── dark              bright ───►
```

Luminance is computed as `dot(face_normal, light_direction) * 0.5 + 0.5`, giving a [0, 1] range that indexes into this 13-character ramp.

## Tech Stack

| Concern | Crate | Purpose |
|---------|-------|---------|
| GPU compute | `wgpu` | Vulkan compute shaders for rasterization |
| CPU math | `glam` | SIMD-accelerated vectors and matrices |
| Terminal | `crossterm` | Raw mode, cursor control, input events |
| OBJ loading | `tobj` | Wavefront OBJ + MTL parsing |
| STL loading | `stl_io` | Binary and ASCII STL parsing |
| CLI | `clap` | Argument parsing with derive macros |
| GPU data | `bytemuck` | Safe transmutes for GPU buffer data |
| Logging | `log` + `env_logger` | Debug and info logging |

## Roadmap

See [Issue #1](https://github.com/heiervang-technologies/a3d.rs/issues/1) for the full roadmap including:

- **M1:** Core rendering MVP (current)
- **M2:** GPU compute pipeline (wgpu shaders)
- **M3:** Advanced rendering (Phong lighting, shadows, color)
- **M4:** Scene graph and animation
- **M5:** Performance and polish
- **M6:** Extensions (WASM, export, plugins)

## License

MIT
