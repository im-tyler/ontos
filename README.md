# Ontos

One deterministic universe simulation, all scales, one state.

Ontos simulates reality as a single state store evolving under scale-conditional
physics: every region runs the finest dynamics its budget deserves, and totals
(mass, energy, momentum) are conserved across every level boundary. Zoom in, a
region promotes; zoom out, it demotes. There are no separate sims and no
loading screens between scales.

```
simval ── verifies ──► ONTO ── state stream ──► light-system (viewer: see + hear)
(external oracle)      (this repo)             (draws frames, plays audio)
```

- **simval** (separate repo) is the external oracle. It replays Ontos's record
  streams against independently written references. It never learns Ontos
  exists; Ontos owns its adapter, dependency is one-way.
- **light-system** (separate repo) is the viewer app: camera samples the
  radiance field, listener samples the pressure field. Both are consumers of
  the stream; the sim never knows who is watching.

## Status

Phase 0 closed and Phase 1 (oracle wiring) done, both simval-verified:

- Deterministic spine, promote/demote with population conservation, FNV
  state hashes, normative stream spec (version 1).
- Verified by three independent implementations of `docs/STREAM_SPEC.md`:
  the Rust sim, simval's Python reference (`python3 -m simval.ontos`),
  and a C++ spike (`light-system` `tools/ontos/ontos_stream_dump.cpp`).
  All three agree bit-for-bit.
- CI proves bit-identical replay across macOS arm64 and Linux x86_64
  against a committed golden-stream corpus, and cross-verifies freshly
  generated streams with the simval oracle on every push.

Next: Phase 2, the gravity epoch — see [docs/DESIGN.md](docs/DESIGN.md).

## Determinism contract

- Strict IEEE floats, no fast-math reassociation
- Fixed dt, fixed tick order, no wall-clock reads inside the tick
- All randomness from explicit seeded generators, never ambient state
- Two runs from the same seed produce bit-identical trajectories

## Build

```
cargo test
```

Golden-stream regression corpus lives in `core/tests/golden/`; the CLI is
`cargo run --bin ontos -- --ticks N --seed S [--demote RX RY] [--promote RX RY]
[--out FILE]`.
