# Ontos — Design

## Thesis

Reality is one system; the only way to simulate it on finite hardware is to
drop degrees of freedom and keep the totals. Temperature and pressure are
nature's own compression of molecular motion. Ontos formalizes that: a single
world state where each region carries a resolution level, dynamics run at the
finest level the budget deserves, and conservation bookkeeping guards every
level boundary. The product is not a solver — it is a hierarchy manager.

The strongest honest claim: a universe statistically indistinguishable from
ours at every scale we can observe. Quantum randomness means we simulate *a*
universe, not *the* universe; replay from a different seed yields a different
one. Determinism is the sim's, not reality's.

## Architecture

Three pieces, one-way data flow:

```
simval ── verifies ──► ONTO ── state stream ──► light-system
```

- Ontos owns: universe state, physics, compression, the record stream format.
- simval owns: verification. Adapters and reference implementations of Ontos's
  rules live in this repo, never in simval. The auditor's books stay separate.
- light-system owns: presentation. Camera and listener are sibling views fed
  by the stream. Audio runs on its own clock, never inside the render pass.

## Verification: two tiers

- **Inside (gauges):** every tick, conservation asserts at every seam, bounds
  checks, monotonic tick. Continuous, cheap, in-process. The multiscale
  handoff is accounting; the accounting must run constantly.
- **Outside (audit):** simval replays record streams against independent
  references (Python, written from spec, not ported) and, from Phase 2,
  against REBOUND anchors for gravity. An in-sim verifier blesses its own
  corruption; the oracle cannot.

## Determinism contract

- Strict IEEE floats, fast-math banned
- Fixed dt, fixed tick order, no wall clock in tick
- Seeded explicit randomness only
- Bit-identical replay from identical seeds, cross-platform target

## Scale ladder

```
zoomed out ──► polynomial (Chebyshev ephemeris)   cheap, exact-enough
           ──► statistical / continuum            totals, flows
           ──► modal                              vibration, audio
zoomed in ──► full integration                    particles, contacts
```

Two open problems, in order of attack:

1. **Conservation at seams** — flux matching across level boundaries in real
   time. This is what killed KSP (the Kraken). Phase 2's deliverable.
2. **Zoom-in reconstruction** — coarse-to-fine destroys information; fine
   detail must be synthesized procedurally under constraints from coarse
   totals (boundary conditions + conserved quantities). Displacement mapping
   for physics. Phase 4, gated.

## Physics: own solvers, no external engine

No Jolt, no PhysX. Game physics engines solve the wrong problem:
game-shaped rigid bodies, single scale, wrong determinism story. Ontos grows
its own deterministic solvers, one per phenomenon, swapped by scale:

- Phase 0: none (game of life — rules, not physics)
- Phase 2: gravity (Chebyshev far field + integrator near field)
- Later: SPH/XPBD-informed solvers for continuum and contact, reading
  material from the godot-parity-archive cascade prototypes

The physics engine is not a dependency of this project. It is the product.

## Phases

- **Phase 0 — spine:** DONE 2026-09-06. Chunked hierarchical state, fixed-dt
  tick, record stream, determinism proven (CI replays bit-identical across
  macOS arm64 + Linux x86_64 against a committed golden corpus). Two
  resolution levels with totals conserved across the seam.
- **Phase 1 — oracle wiring:** DONE 2026-09-06. simval adapter + independent
  reference implementation of Phase 0 rules; both CIs cross-verify on every
  push. A third independent implementation (C++ stream dump, in light-system
  `tools/ontos/`) agrees bit-for-bit. Stream spec v1 frozen: changes require
  a version bump and a coordinated simval update.
- **Phase 2 — gravity epoch:** DONE 2026-09-07. Spec v2 (leapfrog +
  momentum ledger, Chebyshev ephemeris windows with automatic re-fits,
  one-sided seam forces), REBOUND anchoring in simval (independent
  integrator agrees < 1e-4 relative), and the zoom policy (section 18:
  deterministic observer focus drives promote/demote with hysteresis;
  policy events verified by simval's `ontos_zoom_policy` check).
  Window drift: position deviation < 5e-4, ledger drift < 1e-2, energy
  drift < 2e-4 across goldens. The eval-mapping fix (s in [-1,1] over
  the window, not [-1,0]) cut position drift 25x.
- **Phase 3 — viewer:** v1 DONE 2026-09-07. light-system `ontos_view`
  plays back v2 streams: instanced billboard bodies colored by
  region/level, region grid, playback controls, validation-clean, and a
  `--frames` headless smoke mode. `tools/ontos/ontos_stream_dump.cpp`
  remains the third bit-exact spec implementation. Audio (modal
  synthesis from contact events) waits for contact physics; bodies have
  no collisions yet.
- **Phase 4 — reconstruction:** v1 experiment DONE 2026-09-07 (spec
  section 19: collapse to totals-only monopole + deterministic
  reconstruction on expansion with exact momentum residual). Verified
  bit-exact by three implementations; simval bounds reconstruction error
  (post-expansion deviation ~10-12 on goldens, tolerance 64; ledger drift
  < 1e-2) and the orchestrator sweeps parameter grids with MAD-outlier
  detection. Open: collapse-on-coarse composition semantics, tighter
  reconstruction (constrained synthesis beyond monopole+residual).

## Non-goals

- No coupling to light-system internals (viewer is swappable; the
  meridian-extract lesson — the sophisticated path lost to stock Godot —
  stands)
- No Neutron coupling; standalone Rust
- No game-engine embedding
- Epoch fiction without ground truth (universe collisions) is cut; where
  verification data exists (LIGO, CMB, REBOUND), it is used
