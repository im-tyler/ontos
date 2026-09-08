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
- Phase 5: contact v1 (impulsive inelastic pairwise resolution, section 21)
- Later: SPH/XPBD-informed solvers for continuum, reading
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
  synthesis from contact events) landed with contact physics (sections
  21-22); the v1 output path is a deterministic offline WAV render.
- **Phase 5 — contact + audio:** v1 DONE 2026-09-07 (spec sections 21-22).
  Contact: impulsive, perfectly inelastic, frictionless body-body
  resolution between fine bodies — one pinned lexicographic pass per
  tick after the second kick, no iteration count, no tolerance, every
  operand order pinned; radii are a pure function of mass. Contact
  records (tag 10) mark contact beginnings only; resting contact
  re-resolves silently each tick. The momentum ledger is untouched by
  contact impulses (fine-fine axiom; measured physical drift < 1e-11
  relative). Audio: a pure function of the stream (section 22) —
  contact impulses excite three damped resonators per event (65536 Hz
  = 64 samples per 2^-10 tick, mono PCM16 WAV; recurrence-based
  synthesis in the +,-,*,/ closure — no libm transcendentals — so
  consumers are bit-identical cross-platform; FNV-1a64 hash over the
  PCM bytes). Verified bit-exact by three implementations (Rust CLI
  `--wav`, simval `python3 -m simval.ontos_audio`, light-system
  `ontos_stream_dump`/`ontos_view --wav`). v2 DONE 2026-09-08 (spec
  section 24): restitution and Coulomb-clamped friction parameters
  travel in a ContactParams record (tag 12; absence keeps section 21
  bit-identical); fine bodies resolve one-sided against frozen
  contactants — ephemeris-coarse bodies (polynomial-evaluated
  positions), collapsed-region monopoles (a disk of mass M at com,
  radius by the same mass law), and the walls of a bounded world at
  the managed extent — with the static impulses booking the ledger
  exactly like fc kicks (fine-fine pairs still conserve it by axiom;
  measured: all-fine runs keep the ledger bit-exact for any e and
  friction). Static contactants ride Contact records as pseudo body
  ids (monopoles 0xFF000000+region, walls 0xFFFFFF00+wall) and feed
  the section 22 audio with their own reduced-mass rule. Corpus
  coverage DONE 2026-09-08: wall-hit and fine x ephemeris-coarse
  contacts are geometrically unreachable under spec initial conditions
  (bodies start >= 32 from every wall at |v| <= 0.25, so a first wall
  hit needs ~120k ticks and coarse clusters sit mid-world), so a
  test-only corpus constructor closes the gap without touching the
  spec: GravityWorld::corpus_world / corpus_initial_conditions
  (doc-hidden; CLI --test-ic) derives near-wall inbound bodies
  (wallshot) and lane-aimed interceptors against an early-demoted
  coarse cluster (coarsehit) from the same five SplitMix64 draws per
  body as the spec ICs — masses match the spec ICs of the same seed,
  positions and velocities do not. The streams carry no profile
  marker: verification requires passing the same profile (simval
  --test-ic, ontos_stream_dump --test-ic). Goldens g_wallshot (16
  wall-hit records, 4 per wall, e=0.7 friction=0.3) and g_coarsehit
  (4 fine x coarse static contacts vs a demoted region, e=0.5
  friction=0.25) execute the wall and static branches in all three
  implementations, WAV hashes included. Open: spatialized/realtime
  audio.
- **Phase 4 — reconstruction:** v1 experiment DONE 2026-09-07 (spec
  section 19: collapse to totals-only monopole + deterministic
  reconstruction on expansion with exact momentum residual). v2 DONE
  2026-09-07 (spec section 20: multipole reconstruction — the collapse
  record set gains RegionMultipole (dipole accumulators + quadrupole
  tensor); expansion closes the synthesized set's mass-weighted
  position sum on the dipole exactly by residual, and matches the
  quadrupole tensor to rounding by Cholesky whitening/coloring of the
  frozen jitter; gravity during collapse stays monopole). Verified
  bit-exact by three implementations; simval bounds the invariants
  (dipole 0.0 relative, quadrupole ~1e-15, tolerance 1e-9;
  post-expansion deviation stays region-scale, < 40 across seeds vs
  tolerance 64; ledger drift < 1e-2) and the synthesized-set energy
  delta improves 2-20x over section 19. Section 19 streams remain
  valid input (mode selected per collapse cycle by record presence).
  v3 DONE 2026-09-08 (spec section 23: radial-shape synthesis): the
  observation driving it is that no finite set of moments pins pair
  distances — potential energy is dominated by the closest pairs — so
  the collapse now records its binding (the exact internal potential
  statistic, RegionRadial tag 11) and the expansion closes on it
  directly: a pinned 128-iteration bisection solves a strictly
  monotone radial scale on the section 20 displacements, a
  closed-form three-point quadratic solves the velocity-spread scale
  that closes the synthesized kinetic energy on the recorded total,
  and a post-scale recentering keeps the dipole residual exact. The
  synthesized-set energy delta drops from O(1) (0.14-0.57 measured on
  the multipole corpus) to rounding (~1e-14); the quadrupole closes
  only to lambda^2 of the record (measured <= 1.9, bounded 4.0) —
  the honest, documented trade. Opt-in via `--radial`; verified
  bit-exact by three implementations (goldens g_radial, simval
  examples/ontos_gravity/radial, C++ dump) with the new
  `ontos_radial_shape` check closing binding/energy at 1e-9. Open:
  collapse-on-coarse composition semantics; per-shell radial detail
  beyond the single binding scalar (the pair-distance distribution is
  now pinned only in aggregate).

## Non-goals

- No coupling to light-system internals (viewer is swappable; the
  meridian-extract lesson — the sophisticated path lost to stock Godot —
  stands)
- No Neutron coupling; standalone Rust
- No game-engine embedding
- Epoch fiction without ground truth (universe collisions) is cut; where
  verification data exists (LIGO, CMB, REBOUND), it is used
