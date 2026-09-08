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

Phase 0 and Phase 1 closed; Phase 2 (gravity epoch) core mechanics landed,
all simval-verified:

- Deterministic spine, promote/demote with population conservation, FNV
  state hashes, normative stream spec (version 1 = life, version 2 =
  gravity).
- Gravity epoch: leapfrog with exact-pair momentum ledger, Plummer-softened
  2D gravity, Chebyshev degree-8 ephemeris windows for demoted regions
  (W=32, least-squares fits in the deterministic op closure +,-,*,/,sqrt),
  automatic window re-fits, one-sided fine<-coarse forces with bounded
  ledger drift.
- Verified by three independent implementations of `docs/STREAM_SPEC.md`:
  the Rust sim, simval's Python reference (`python3 -m simval.ontos`), and
  a C++ spike (`light-system` `tools/ontos/ontos_stream_dump.cpp`). All
  three agree bit-for-bit on both versions, including windowed runs.
- CI proves bit-identical replay across macOS arm64 and Linux x86_64
  against committed golden-stream corpora (life + gravity), and
  cross-verifies freshly generated streams with the simval oracle on every
  push.

Phase 2 closed 2026-09-07 (REBOUND anchor < 1e-4; zoom policy verified
in-stream). Phase 3 viewer v1: light-system `ontos_view` plays back
gravity streams. Phase 4 v1 (section 19): collapse-to-monopole with
deterministic reconstruction on expansion — bit-verified by three
implementations, error-bounded by simval. Phase 4 v2 (section 20,
2026-09-07): multipole reconstruction — every collapse additionally
freezes the dipole (mass-weighted position sums) and quadrupole (second
central moments) in a RegionMultipole record; expansion synthesizes
positions that close on the dipole exactly by residual and match the
quadrupole to rounding via a deterministic Cholesky whitening/coloring
transform of the frozen jitter (measured: dipole 0.0 relative,
quadrupole ~1e-15; synthesized-set energy delta improves 2-20x over
section 19; post-expansion deviation stays region-scale, < 40 measured,
tolerance 64). Section 19 streams (no multipole record) still verify.
See [docs/DESIGN.md](docs/DESIGN.md).

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
