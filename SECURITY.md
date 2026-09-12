# Security Policy

## Supported versions

a3d is pre-1.0 and has not published a release yet. Security fixes are applied
to the latest `main`; once releases exist, the most recent release will also be
supported.

## Reporting a vulnerability

Please report security issues privately rather than opening a public issue:

- Use GitHub's **[Report a vulnerability](https://github.com/heiervang-technologies/a3d.rs/security/advisories/new)**
  (Security → Advisories) to open a private advisory.

We aim to acknowledge reports within a few days.

## Scope notes

a3d is an offline terminal renderer. It parses untrusted model files (OBJ/STL/GLB),
so the most likely concerns are crashes or excessive resource use on malformed
input rather than remote attacks. The crate is `#![forbid(unsafe_code)]`, and
model parsing is delegated to the `tobj`, `stl_io`, and `gltf` crates.

`load_model` surfaces malformed, missing, empty, and non-finite-coordinate
files as clean errors rather than panicking. GLB scene traversal is iterative
and rejects cycles/repeated nodes before expanding geometry; declared binary
buffers and consumed accessors must be readable. Accessor types, counts,
strides, and byte ranges are checked before reader construction, including
sparse values and ordered, in-range sparse indices. Each accessor has a
256 MiB decoded budget (at least 16 bytes per element); this limits sparse
expansion, but is not a total file or scene memory limit. A crafted binary-STL triangle
count does not trigger unbounded allocation (the parser reads lazily). The
underlying model parsers are not exhaustively audited, so reports of
any remaining panic, hang, or unbounded allocation reachable from a crafted
model file are still welcome.
