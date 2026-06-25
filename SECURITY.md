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
model parsing is delegated to the `tobj` and `stl_io` crates. Reports of panics
or unbounded allocation on crafted model files are welcome.
