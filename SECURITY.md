# Security Policy

## Supported versions

a3d is pre-1.0. Security fixes are applied to the latest `main` and the most
recent release only.

## Reporting a vulnerability

Please report security issues privately rather than opening a public issue:

- Use GitHub's **[Report a vulnerability](https://github.com/heiervang-technologies/a3d.rs/security/advisories/new)**
  (Security → Advisories) to open a private advisory.

We aim to acknowledge reports within a few days.

## Scope notes

a3d is an offline terminal renderer. It parses untrusted model files (OBJ/STL),
so the most likely concerns are crashes or excessive resource use on malformed
input rather than remote attacks. The crate is `#![forbid(unsafe_code)]`, and
model parsing is delegated to the `tobj` and `stl_io` crates.

`load_model` surfaces malformed, missing, empty, and non-finite-coordinate
files as clean errors rather than panicking, and a crafted binary-STL triangle
count does not trigger unbounded allocation (the parser reads lazily). The
underlying `tobj`/`stl_io` parsers are not exhaustively audited, so reports of
any remaining panic, hang, or unbounded allocation reachable from a crafted
model file are still welcome.
