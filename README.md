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

Scaffold. Phase 0 in progress: deterministic single-resolution game of life,
bit-identical replay from identical seeds. See [docs/DESIGN.md](docs/DESIGN.md).

## Determinism contract

- Strict IEEE floats, no fast-math reassociation
- Fixed dt, fixed tick order, no wall-clock reads inside the tick
- All randomness from explicit seeded generators, never ambient state
- Two runs from the same seed produce bit-identical trajectories

## Build

```
cargo test
```
