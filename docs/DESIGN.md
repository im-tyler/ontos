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

- **Phase 0 — spine:** chunked hierarchical state, fixed-dt tick, record
  stream, determinism proven (two runs bit-identical). Two resolution levels
  with totals conserved across the seam.
- **Phase 1 — oracle wiring:** simval adapter + independent reference
  implementation of Phase 0 rules. Divergence = bug found before physics
  exists.
- **Phase 2 — gravity epoch:** Chebyshev ephemeris far, integrator near,
  invisible-zoom handoff. simval verifies against REBOUND.
- **Phase 3 — viewer:** light-system consumes the stream (draws); contact
  events drive modal synthesis into an audio callback on its own thread.
  Can pull earlier once the stream format stabilizes.
- **Phase 4 — reconstruction:** fine-detail synthesis under coarse
  constraints. Timeboxed experiments; simval bounds the error. Gate: Phase
  2's seam must be boring first.

## Non-goals

- No coupling to light-system internals (viewer is swappable; the
  meridian-extract lesson — the sophisticated path lost to stock Godot —
  stands)
- No Neutron coupling; standalone Rust
- No game-engine embedding
- Epoch fiction without ground truth (universe collisions) is cut; where
  verification data exists (LIGO, CMB, REBOUND), it is used
