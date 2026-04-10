# Project: litelink

Lightweight LiveLink Face → VRChat OSC bridge in Rust.

## Test runner

Use `cargo nextest run` for tests, not `cargo test`.

## Commit style

Single-line conventional commits: `feat: ...`, `fix: ...`, `chore: ...`
No Co-Authored-By. No multi-line bodies.

## Architecture

- `src/livelink.rs` — UDP packet parser (LiveLink Face protocol)
- `src/mapping.rs` — ARKit blendshape → VRChat OSC parameter mapping
- `src/osc.rs` — OSC bundle construction + change detection sender
- `src/state.rs` — Shared tracking state (RwLock + atomic connected flag)
- `src/gui.rs` — Optional egui status window (feature-gated behind `gui`)
- `src/main.rs` — CLI, thread orchestration, signal handling
