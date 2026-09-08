# Ontos Stream Format and Phase-0 Rules Specification

Version 1. This document is the normative contract. An independent
implementation that follows it exactly must reproduce every hash in an
Ontos stream bit-for-bit. Nothing in the Rust code overrides this file.

## 1. Numbers and hashing

- All integers are unsigned, little-endian.
- All arithmetic on coordinates wraps (torus).
- The hash is FNV-1a 64-bit over bytes:

```
h = 0xcbf29ce484222325
for each byte b:  h = (h XOR b) * 0x100000001b3   (mod 2^64)
```

Known values: FNV("") = 0xcbf29ce484222325, FNV("a") = 0xaf63dc4c8601ec8c.

## 2. World layout

- Fine grid: 128 x 128 cells, world width W=128, height H=128.
- The world divides into 2 x 2 regions of 64 x 64 fine cells each.
  Region (rx, ry) for rx, ry in {0, 1} owns fine cells
  [rx*64, rx*64+64) x [ry*64, ry*64+64).
- Each region carries a level:
  - Fine: stores 64 x 64 cells.
  - Coarse: stores 32 x 32 cells; coarse cell (cx, cy) of region (rx, ry)
    covers the fine block [rx*64 + cx*2, rx*64 + cx*2 + 2) x
    [ry*64 + cy*2, ry*64 + cy*2 + 2).

## 3. Reads

- read(fx, fy) at FINE granularity (fx, fy wrapped mod 128): resolve the
  owning region. If Fine: that cell's value (0 or 1). If Coarse: the value
  of the coarse cell containing (fx, fy) — i.e. the coarse value is
  replicated across all 4 fine positions of its block.
- read_block(cx, cy) at COARSE granularity (cx, cy wrapped mod 64): the
  coarse position names a 2x2 fine block. Resolve the region containing the
  block's fine origin (cx*2, cy*2). If Coarse: that region's cell
  (cx mod 32, cy mod 32). If Fine: the OR of the block's 4 fine cells.

## 4. Neighbor counts

- For a cell in a FINE region at world fine coords (gx, gy): examine the 8
  offsets (dx, dy) in {-1,0,1}^2 \ {(0,0)}. Wrap (nx, ny) = (gx+dx mod 128,
  gy+dy mod 128). Skip neighbors with read(nx, ny) == 0. Each surviving
  neighbor contributes 1, EXCEPT that neighbors resolving to the same
  underlying cell are counted once: deduplicate by key
  - (0, nx, ny) if the neighbor's region is Fine
  - (1, nx div 2, ny div 2) if the neighbor's region is Coarse
  (div on the wrapped value).
- For a cell in a COARSE region at world coarse coords (gx, gy): examine
  the 8 offsets, wrap mod 64, count read_block(nx, ny) directly (no
  deduplication — distinct coarse positions are distinct blocks).

## 5. Tick rule

Standard game of life, B3/S23, evaluated simultaneously for every region
against the pre-tick state:

- alive cell survives iff neighbor count is 2 or 3
- dead cell becomes alive iff neighbor count is exactly 3

Each region applies the rule at its own granularity (fine cells count fine
neighbors per section 4; coarse cells count coarse neighbors per section 4).
All regions advance one tick per step. The tick counter starts at 0 and
increments after each step.

## 6. Level changes

- Demote (Fine -> Coarse): each coarse cell of the target region becomes
  the OR of its 4 fine cells.
- Promote (Coarse -> Fine): the region becomes 64 x 64 cells. For each
  alive coarse cell (cx, cy), exactly one of its 4 fine cells becomes
  alive, chosen by d = FNV-1a64(seed_le_8 || gx_le_4 || gy_le_4) mod 4,
  where gx = rx*64 + cx*2 and gy = ry*64 + cy*2 are the block's world fine
  origin coordinates and seed is the world seed (u64). The offsets are
  d=0: (0,0), d=1: (1,0), d=2: (0,1), d=3: (1,1), applied to the block
  origin. Dead coarse cells contribute no alive fine cells. Population is
  preserved exactly.

## 7. Region and world hashes

- Region hash for region (rx, ry): FNV-1a64 over the byte sequence
  [level_byte] || cells, where level_byte is 0 for Coarse / 1 for Fine and
  cells are the region's stored values in row-major order (64*64 bytes for
  Fine, 32*32 for Coarse).
- World hash: FNV-1a64 over tick_le_8 || (region hash le_8 for region
  (0,0), then (1,0), then (0,1), then (1,1)).

## 8. Seed pattern

The initial state (before any RegionLevel events) has every region Fine
and all cells dead except the r-pentomino at world center c = 64:

alive: (c, c), (c+1, c), (c, c+1), (c-1, c+1), (c, c+2)
      = (64,64), (65,64), (64,65), (63,65), (64,66)

Coordinates are world fine coordinates.

## 9. Stream binary format

Little-endian throughout. Records appear in order; unknown tag values must
be rejected.

Header (not a record):
- 4 bytes magic "ONTO"
- u32 format version = 1
- u32 world_w = 128
- u32 world_h = 128

Records, each a u8 tag followed by its payload:
- tag 1 TickHeader: u64 tick
- tag 2 Snapshot: u64 population (total across regions, live units)
- tag 3 CellFlipped: u64 tick, u32 x, u32 y (reserved: verifiers must parse
  and skip it; no state semantics are defined in version 1. The reference
  CLI does not emit it)
- tag 4 RegionLevel: u32 region_x, u32 region_y, u8 level (0 coarse, 1 fine)
- tag 5 RegionState: u64 tick, u32 region_x, u32 region_y, u8 level,
     u64 population, u64 region hash. The tick field repeats the TickHeader
     tick of the tick it belongs to; verifiers treat it as report-only in
     version 1 (the TickHeader is authoritative)

Emission contract of the reference CLI (`ontos`):
- Header, then one RegionLevel record per requested level change (in
  application order: all demotes, then all promotes), then for each tick:
  TickHeader, Snapshot, and RegionState for regions (0,0), (1,0), (0,1),
  (1,1) in that order. Level changes apply before the first tick.
- The format itself permits RegionLevel records at any point in the stream;
  they take effect when encountered, before the next tick. Verifiers apply
  them on encounter.

## 10. Verification procedure

Given a stream file, an independent verifier must be able to:
1. Parse the header and records.
2. Reconstruct the initial world from the seed (carried out-of-band by the
   harness; the stream itself does not embed the seed) and the RegionLevel
   events.
3. Step the world per sections 3-6, computing each region hash per section
   7.
4. Compare every RegionState record's level, population, and hash against
   the locally computed values. Any mismatch is a verification failure.

---

# Part II — Version 2: gravity epoch

Version 2 streams carry N-body gravity under the same region hierarchy:
Fine regions integrate, Coarse regions run a polynomial ephemeris. Version
1 (life) sections above are frozen; everything below is version 2 only.

## 11. Deterministic arithmetic

- All floats are IEEE 754 binary64, little-endian in the stream.
- The tick path may use ONLY +, -, *, /, and sqrt on f64. These are
  correctly rounded and bit-identical across conforming platforms.
  Fused operations (FMA / `mul_add` / `fma()`), x87, fast-math, and any
  libm function beyond sqrt are banned. Integer hashing stays FNV-1a64.
- Fixed dt = 2^-10. Fixed tick order everywhere it is specified. Two runs
  from the same seed produce bit-identical streams, cross-platform.

## 12. Constants and initial conditions

- G = 1.0, softening eps2 = 1.0, dt = 2^-10, window W = 32 ticks,
  ephemeris degree 8, samples 33.
- The world is the unbounded plane. The region grid is the same 2x2
  partition of the initial box [0,128) x [0,128); bodies outside the box
  belong to no region ("unmanaged") and are always Fine.
- Randomness: splitmix64. State s starts at the seed. Each draw advances
  s = (s + 0x9E3779B97F4A7C15) mod 2^64, then
  z = s; z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9 mod 2^64;
  z = (z ^ (z >> 27)) * 0x94D049BB133111EB mod 2^64; out = z ^ (z >> 31).
  Body i consumes five draws u0..u4 (all bodies draw in id order at init):
  mass = 0.5 + u0 * 2^-64 * 2.0;  x = 32.0 + u1 * 2^-64 * 64.0;
  y = 32.0 + u2 * 2^-64 * 64.0;  vx = (u3 * 2^-64 - 0.5) * 0.5;
  vy = (u4 * 2^-64 - 0.5) * 0.5.
- RegionLevel events before the first tick and mid-stream both apply at
  the tick boundary where they are encountered, before that tick's step.

## 13. Force law and integration (Fine)

Pair force on i from j (2D, Plummer softening):
  dx = xj - xi;  dy = yj - yi;  s2 = dx*dx + dy*dy + eps2;
  inv3 = 1.0 / (s2 * sqrt(s2));
  (ax_i, ay_i) += mj * dx * inv3, mj * dy * inv3  (times G = 1)
Accelerations are computed by iterating pairs (i, j), i < j, in
lexicographic order, computing the pair factor once and applying +f to i
and -f to j, so pair momentum exchange cancels exactly in floating point.

One tick (kick-drift-kick leapfrog), for Fine and unmanaged bodies only.
Accelerations are accumulated per body as two separate arrays: `ff` from
pairs where both bodies are Fine/unmanaged (symmetric +-f application in
pair order), and `fc` from pairs with exactly one Coarse body (only the
Fine body accumulates; Coarse positions are ephemeris evaluations for the
tick; pairs of two Coarse bodies are skipped):
  a_ff, a_fc = accel(x)
  v += a_ff * dt * 0.5; then v += a_fc * dt * 0.5
  x += v * dt
  a_ff, a_fc = accel(x)
  v += a_ff * dt * 0.5; then v += a_fc * dt * 0.5
The two kick phases are separate += operations in that order (the bit
pattern depends on it).

Momentum ledger: px, py start as sum of m*v over bodies in id order at
tick 0 and are updated ONLY by one-sided (fc) kicks: after each fc kick
phase, for each kicked body in id order,
  ledger += m * (a_fc * dt * 0.5).
Fine-fine pair exchanges conserve momentum exactly by axiom of the
ledger, not by floating-point cancellation. TotalsState carries the
ledger values.

## 14. Demotion (Fine -> Coarse) and the ephemeris window

Demote(region R) at tick t0 selects B = every body whose current position
lies in R's box [rx*64, rx*64+64) x [ry*64, ry*64+64). Unmanaged bodies
never demote.

For each body in B, from its state at t0, run an internal-only
pre-integration: 32 ticks of the section 13 leapfrog restricted to pairs
inside B (no external forces), recording position and velocity at each of
the 33 integer ticks t0 + k, k = 0..32.

Each recorded coordinate series y_k (x, y, vx, and vy separately) is fit
by weighted least squares onto Chebyshev polynomials of degree 8:
  s_k = -1.0 + k / 16.0            (k = 0..32)
  T_0(s) = 1;  T_1(s) = s;  T_{j+1}(s) = 2*s*T_j(s) - T_{j-1}(s)
  w_0 = w_32 = 0.5, other w_k = 1.0
  G[j][l] = sum_k w_k * T_j(s_k) * T_l(s_k)
  b[j]   = sum_k w_k * y_k * T_j(s_k)
The coefficients c are the unique solution of G c = b, computed by
Cholesky factorization in the fixed loop order:
  L[j][j] = sqrt(G[j][j] - sum_{k<j} L[j][k]^2)
  L[i][j] = (G[i][j] - sum_{k<j} L[i][k]*L[j][k]) / L[j][j]   (i > j)
followed by forward and back substitution in index order (G is positive
definite; no pivoting). Every operation is in the section 11 closure.
Evaluation at tick t in [t0, t0 + 32] uses s = -1.0 + (t - t0) / 16.0
and Clenshaw's recurrence; coefficients are stored per body per window.

During the window, the body's emitted position and velocity are the
polynomial evaluations; its region is R and its level is Coarse.

At t0 + 32 the region re-fits automatically: every body's state becomes
its polynomial evaluation at t0 + 32; bodies of B whose evaluated
position still lies in R's box enter a fresh pre-integration and window;
bodies that left the box become unmanaged Fine (state = evaluation).
Fine bodies that entered the box are not absorbed.

A RegionLevel promote event for R at tick t ends the window early:
bodies of B take polynomial evaluations at t, become Fine, and the
region level becomes Fine.

## 15. Hashes and totals (version 2)

- Body state bytes: id_le4 || x_le8 || y_le8 || vx_le8 || vy_le8 ||
  mass_le8 || level_u8 (level 0 coarse, 1 fine).
- Region hash: FNV-1a64 over level_u8 || body state bytes of the
  region's bodies sorted by id. Unmanaged bodies hash into no region.
- World hash: FNV-1a64 over tick_le8 || body state bytes of all bodies
  in id order.
- Energy (reporting only): KE = sum 0.5*m*(vx^2+vy^2);
  PE = sum over pairs i<j of -m_i*m_j / sqrt(s2) with s2 as section 13.
- Momentum: the section 13 ledger. Fine-fine pair exchange conserves it
  exactly by construction; one-sided (fine-coarse) kicks move it by the
  dropped reaction, so ledger drift is zero while all regions are Fine
  and bounded during coarse windows.

## 16. Stream format, version 2

Header: the 16-byte version 1 header with format version = 2, followed by
u32 body_count N. Records 1-5 keep their version 1 shapes (Snapshot's
population = number of bodies; RegionState's population = bodies in the
region). New records:

- tag 6 BodyState: u64 tick, u32 body_id, u8 region (0..3, or 255
  unmanaged), u8 level (0 coarse, 1 fine), f64 x, y, vx, vy, mass
- tag 7 TotalsState: u64 tick, u64 fine_count, u64 coarse_count,
  f64 mass, px, py, energy

Emission contract of the reference CLI in gravity mode:
- Header + body_count, then RegionLevel records (pre-tick changes first,
  in application order; `--demote-at/--promote-at` events are emitted at
  their tick, before that tick's records), then per tick: TickHeader,
  Snapshot, TotalsState, RegionState for regions (0,0), (1,0), (0,1),
  (1,1), then BodyState for every body in id order 0..N-1.
- tag 3 (CellFlipped) remains reserved and unused in version 2.

## 17. Verification procedure, version 2

1. Parse header + body_count, reconstruct initial conditions from the
   seed via splitmix64, apply RegionLevel events per section 12.
2. Step per sections 13-14, computing hashes per section 15.
3. Compare every RegionState (level, population, hash), every BodyState
   (region, level, x, y, vx, vy, mass), and every TotalsState field
   against locally computed values; any bit difference is a failure.
4. Bounded-error checks (against the verifier's own all-Fine reference
   run from the same seed): per-tick position deviation introduced by
   windows must stay within tolerance, and cumulative momentum and
   energy drift must stay within tolerance. Tolerances live in the
   verifier's check framework, not in this format spec.

## 18. Zoom policy (observer-driven level changes, version 2)

When the reference CLI runs gravity mode with `--observer <offset>` (u64),
a deterministic observer focus drives region levels automatically. The
generator is normative; verifiers reproduce it exactly:

- A dedicated splitmix64 instance seeded with seed XOR offset draws two
  values per control point, in order: P_0, P_1, ... with
  P_k = (16.0 + u0 * 2^-64 * 96.0, 16.0 + u1 * 2^-64 * 96.0).
  Control point k covers ticks [1 + 64k, 64 + 64k]; the focus during that
  span is the linear blend
  focus(t) = P_k + (P_{k+1} - P_k) * ((t - (1 + 64k)) / 64.0).
  Draws happen in tick order; a verifier stepping ticks in order consumes
  the same sequence.
- At each tick boundary entering t where t mod 16 == 1 and t >= 17, in
  region index order 0..3, let d = the Euclidean distance from focus(t)
  to the region's box (0 if inside):
  cx = min(max(fx, x0), x1); cy = min(max(fy, y0), y1);
  d = sqrt((fx-cx)*(fx-cx) + (fy-cy)*(fy-cy)).
  - a Fine region with d > 48.0 demotes (RegionLevel 0)
  - a Coarse region with d < 24.0 promotes (RegionLevel 1)
  Events apply at that boundary, in that order, before the tick. Between
  24.0 and 48.0 nothing changes (hysteresis).
- Event ordering at one boundary: zoom-policy events apply in the same
  phase as CLI-scheduled events (both, in stream order, before window
  re-fits), so a verifier that applies RegionLevel records on encounter
  reproduces the run bit-exactly without knowing the policy. The CLI emits
  one RegionLevel record per fired policy event, in region order,
  immediately before that tick's TickHeader.
- Run-dir metadata: ontos.json may carry "observer": <offset>. When
  present, verifiers must check that the stream's RegionLevel sequence
  equals the policy's expected sequence exactly.

## 19. Phase 4 experiment: collapse and reconstruction (version 2)

Collapse is a lossy coarse mode for regions: the region compresses to
TOTALS ONLY (monopole), individual bodies stop being tracked, and on
expansion the fine state is synthesized deterministically under the
totals. This section is experimental; simval bounds the reconstruction
error.

- RegionLevel level byte 2 means collapse (0 demote/ephemeris, 1
  promote/expand, 2 collapse). Level 2 is valid only in version 2.
- New record, emitted when a collapse is applied:
  tag 8 RegionCollapsed: u64 tick, u32 region_x, u32 region_y,
    u64 body_count N, f64 mass, f64 com_x, f64 com_y, f64 px, f64 py,
    f64 energy
  Totals at collapse (fixed summation order, bodies in id order):
  mass = sum m_i; com = sum m_i * pos / mass; px, py = sum m_i * v;
  energy = the section 15 formula over the region's bodies only.

- While a region is collapsed it acts as a SINGLE body of mass M at com
  for gravity: fine bodies accumulate acceleration from (M, com), one
  pair per fine body per collapsed region, in region index order after
  all individual-body pairs. The collapsed region receives no forces;
  com and v_com = (px/M, py/M) are frozen for the collapse's duration.
- Bodies of a collapsed region still emit BodyState every tick with
  level = 2: x = com_x + jx_i, y = com_y + jy_i, vx = v_com_x,
  vy = v_com_y, mass = m_i (their real masses — masses are never
  destroyed), where the jitter offsets come from a dedicated splitmix64
  seeded seed ^ (region * 0x9E3779B97F4A7C15 wrapped) drawing two values
  per body in id order at collapse time:
  jx_i = ((u * 2^-64) - 0.5) * 8.0, jy likewise. Jitter is drawn once
  and frozen; body states under collapse are static in time.

- Expansion (RegionLevel level 1 on a collapsed region at tick t):
  positions x_i = com_x + jx_i (the same frozen jitter), and velocities
  v_i = v_com + s_i for i < N-1 with spread s_i from two more
  splitmix64 draws per body in id order from the same generator state:
  s_i = ((u * 2^-64) - 0.5) * 0.1 per axis; the last body absorbs the
  exact residual: v_{N-1} = (P - sum_{i<N-1} m_i * v_i) / m_{N-1},
  component-wise, with the sum in id order. All bodies become Fine;
  integration resumes. The residual formula is part of the format: any
  conforming implementation must produce bit-identical velocities.

- Hashes: body state bytes under collapse use level byte 2 and the
  synthesized fields exactly as emitted. Region hash covers the region's
  bodies with their emitted (synthesized) states. The collapsed region
  contributes its synthesized body states to world/region hashes and
  totals like any other body.

- Momentum ledger: forces between a fine body and a collapsed region
  are one-sided (fine accumulates); the ledger updates follow section
  13 rules for those pairs (collapsed = coarse for ledger purposes).

- Verification: bit-match as usual, plus (check framework, not format):
  reconstruction error = deviation of the post-expansion continuation
  from an all-fine reference run, and energy non-conservation of the
  synthesized set vs the collapse record's energy field. Both are
  tolerance checks owned by the verifier.

Normative clarifications (implementation-consensus 2026-09-07):
- RegionCollapsed payload is 72 bytes plus the tag byte (8+4+4+8+48).
- Expansion spread draws: 2*(N-1) values, for bodies 0..N-2 in id order
  (the residual body draws nothing).
- TotalsState coarse_count includes collapsed bodies (fine + coarse = N).
- The jitter generator re-seeds at every collapse (identical membership
  yields identical jitter across collapse cycles).
- Collapsing an ephemeris-coarse region first materializes polynomial
  evaluations at the collapse tick, then selects in-box bodies.
- RegionLevel demote on a collapsed region is a no-op.
- Collapse of an empty region emits the zero-totals record; monopole
  skipped.
- RegionState and region-hash level byte are 2 for collapsed regions.
- Fine bodies entering a collapsed region's box hash in position-wise
  and are not absorbed (mirrors section 14).

CLI: `--collapse-at T RX RY` and `--expand-at T RX RY` schedule the
events; the CLI emits RegionLevel records (level 2 for collapse) and the
RegionCollapsed record at the collapse boundary, before the TickHeader.

## 20. Phase 4 v2: multipole reconstruction (version 2)

Section 19 reconstructs positions from the monopole only: com plus raw
jitter, with no constraint on the synthesized set's center of mass or
spread. This section tightens reconstruction beyond the monopole: the
synthesized set matches the collapsed set's dipole (mass-weighted
position sum) exactly by residual, and its quadrupole (mass-weighted
second central moments) to rounding, by a deterministic linear transform
of the frozen jitter. The gravity behavior of a collapsed region is
unchanged (still a monopole at com); only the expansion synthesis
changes. All arithmetic stays in the section 11 closure with fixed
summation orders; every conforming implementation produces
bit-identical synthesized states.

- New record, emitted immediately after every RegionCollapsed record at
  the same boundary:
  tag 9 RegionMultipole: u64 tick, u32 region_x, u32 region_y,
    f64 mx, f64 my, f64 qxx, f64 qxy, f64 qyy
  Payload is 56 bytes plus the tag byte (8+4+4+40). For an empty
  collapse (N=0) all five floats are zero. The reference CLI emits this
  record for every collapse; streams whose collapses carry no
  RegionMultipole record are section 19 streams and reconstruct per
  section 19 (a verifier selects the expansion mode per collapse cycle
  from the presence of this record).

- Totals at collapse (all sums left-to-right over the section 19 member
  list in id order, from the same materialized states that produced the
  RegionCollapsed record):
  mx = sum m_i * x_i;  my = sum m_i * y_i
  (these are the exact accumulators that produced com = mx / mass)
  with com_x = mx / mass, com_y = my / mass:
  dx_i = x_i - com_x;  dy_i = y_i - com_y
  qxx = sum m_i * dx_i * dx_i;  qxy = sum m_i * dx_i * dy_i;
  qyy = sum m_i * dy_i * dy_i

- Expansion of a collapse cycle that carries a RegionMultipole record:
  the draw sequence is identical to section 19 (jitter 2 draws per body
  in id order, then spread 2 draws per body for bodies 0..N-2; the
  residual body draws nothing). Velocities are synthesized exactly as
  section 19 (v_i = v_com + s_i for i < N-1; the last body absorbs the
  momentum residual). Positions are synthesized per this section.

  Dipole residual (always applied, any N >= 1): given base
  displacements (bx_i, by_i) from the transform step below (or the raw
  jitter when the transform is skipped), positions are
  x_i = com_x + bx_i, y_i = com_y + by_i for i < N-1;
  Sx = sum_{i<N-1} m_i * x_i (left-to-right, id order); same Sy;
  x_{N-1} = (mx - Sx) / m_{N-1};  y_{N-1} = (my - Sy) / m_{N-1}.
  This mirrors the section 19 momentum residual: the synthesized set's
  mass-weighted position sum closes on (mx, my) to rounding.

  Quadrupole transform (applied only when N >= 3):
  - Recenter the raw jitter: wx = (sum_i m_i * jx_i) / mass and
    wy = (sum_i m_i * jy_i) / mass (sums over all members, id order,
    mass = the RegionCollapsed mass field); dhat_x_i = jx_i - wx,
    dhat_y_i = jy_i - wy.
  - Jitter second moments (id order): jxx = sum m_i * dhat_x_i^2,
    jxy = sum m_i * dhat_x_i * dhat_y_i, jyy = sum m_i * dhat_y_i^2.
  - Cholesky guards, evaluated in this order; if any fails the
    transform is skipped (base displacements = raw jitter):
    lj00 = sqrt(jxx) requires jxx > 0.0;
    lj10 = jxy / lj00;  jjd = jyy - lj10 * lj10 requires jjd > 0.0;
    lq00 = sqrt(qxx) requires qxx > 0.0;
    lq10 = qxy / lq00;  qqd = qyy - lq10 * lq10 requires qqd > 0.0.
  - Inverse factor entries: u00 = 1.0 / lj00; u11 = 1.0 / lj11 where
    lj11 = sqrt(jjd); u10 = -(lj10 / (lj00 * lj11)).
  - Transform A = L_Q * L_J^{-1} (both lower-triangular, so A is
    lower-triangular):
    a00 = lq00 * u00;  a11 = lq11 * u11 where lq11 = sqrt(qqd);
    a10 = lq10 * u00 + lq11 * u10.
  - Base displacements: bx_i = a00 * dhat_x_i;
    by_i = a10 * dhat_x_i + a11 * dhat_y_i.
    In exact arithmetic sum m_i * (bx_i, by_i)(bx_i, by_i)^T equals the
    recorded quadrupole (A J A^T = Q); in floating point it matches to
    rounding.

  Body states while collapsed (before expansion) are unchanged from
  section 19: com plus the raw frozen jitter. The transform exists only
  at the expansion boundary.

- Hashes and totals: unchanged; the synthesized expansion states enter
  body/region/world hashes and totals exactly as emitted.

- Verification: bit-match the RegionMultipole fields against locally
  computed values, plus (check framework, not format): the synthesized
  set's mass-weighted position sum closes on (mx, my), and its second
  central moments (computed with the same dx_i = x_i - com_x formulas
  against the record's com) close on (qxx, qxy, qyy). Tolerances live in
  the verifier's check framework. Post-expansion deviation vs an
  all-fine reference remains a verifier-owned bound (section 19).

## 21. Contact dynamics (version 2, additive)

Bodies interact by contact in addition to gravity: fine bodies that
overlap receive an impulsive, perfectly inelastic, frictionless
velocity resolution. Contact is an opt-in mode selected by record
presence, exactly like section 19/20 mode selection: a stream in
contact mode carries tag 10 Contact records; a verifier that encounters
none steps exactly as before (contact trajectories are bit-identical to
gravity-only trajectories up to the first contact). Contact mode is
sticky: from the first Contact record to the end of the stream, the
contact pass of this section runs every tick. Streams with no Contact
records are indistinguishable from section 13-20 streams and verify
against implementations that know nothing of this section (except that
they must reject unknown tag 10 if they predate it).

- Body radius: r_i = CONTACT_R * sqrt(m_i) with CONTACT_R = 2.0. The
  radius is a pure function of mass (never stored, recomputed where
  needed). Masses are in [0.5, 2.5] by section 12, so radii lie in
  [sqrt(0.5)*2, sqrt(2.5)*2].
- The contact pass runs at the end of each tick, after the second fc
  kick and before the tick counter advances. It iterates pairs
  (i, j), i < j, in lexicographic body-id order, over bodies that are
  fine at that moment (ephemeris-coarse and collapsed bodies never
  contact; a pair with a non-fine member is skipped). Pairs are
  processed and applied immediately in that single fixed order — there
  is no iteration, no convergence loop, and no solver tolerance.
- Detection: dx = x_j - x_i;  dy = y_j - y_i;  d2 = dx*dx + dy*dy;
  rs = r_i + r_j. The pair overlaps iff d2 < rs * rs.
- Normal: if d2 == 0.0 the normal is pinned to (nx, ny) = (1.0, 0.0);
  otherwise dist = sqrt(d2), nx = dx / dist, ny = dy / dist.
- Relative normal velocity: vn = (vx_j - vx_i) * nx + (vy_j - vy_i) * ny.
  The pair resolves iff it overlaps AND vn < 0.0 (approaching).
- Impulse (perfectly inelastic, frictionless, equal-and-opposite):
  inv = 1.0 / (m_i + m_j);  t = vn * inv;
  f_i = t * m_j;  f_j = t * m_i;
  vx_i += f_i * nx;  vy_i += f_i * ny;  vx_j -= f_j * nx;  vy_j -= f_j * ny.
  (vn < 0 makes f_i, f_j negative: body i is pushed along -n, body j
  along +n — apart. The equal-and-opposite pair products m_i * f_i and
  m_j * f_j share the t operand, so the exchange conserves momentum to
  rounding.)
  Positions are not modified: with dt = 2^-10 and vn of order unity the
  per-contact penetration is below 1e-3 and is left to the next tick's
  impulse (no position correction, no Baumgarte). The impulse magnitude
  recorded for consumers is jn = -vn * mu with mu = (m_i * m_j) /
  (m_i + m_j) (positive; note mu recomputed with this exact op order).
- Momentum ledger: contact impulses never update px, py. Like fine-fine
  gravity exchanges, contacts between fine bodies conserve the ledger by
  axiom; the physical sum of m*v changes only by rounding (a few ULP
  per contact).
- Touching set: at the end of each pass, the set of overlapping
  fine pairs (whether or not an impulse fired) replaces the previous
  set. A Contact record is emitted iff an impulse fired AND the pair
  was NOT in the previous tick's set (a contact beginning). Resting
  contact (pair stays overlapping) re-fires gravity-built approach
  velocity every tick; those impulses apply silently and emit nothing.
  Pairs leave the set by separating, or by either member leaving the
  fine level (demote/collapse drops its pairs from the set; a body that
  returns to fine and still overlaps begins a fresh contact).
- New record:
  tag 10 Contact: u64 tick, u32 body_a, u32 body_b (a < b),
    f64 jn, f64 cx, f64 cy
  Payload is 40 bytes plus the tag byte (8+4+4+8+8+8). cx, cy = the
  contact midpoint (x_i + x_j) * 0.5, (y_i + y_j) * 0.5 from the
  positions at detection (post-drift, pre-impulse — positions are not
  changed by the impulse). jn is the impulse magnitude above. The tick
  field is the tick whose pass produced the event.
- Emission contract: Contact records for tick t are written immediately
  before that tick's TickHeader (after any RegionCollapsed/
  RegionMultipole records of the same boundary), in generation order —
  lexicographic (a, b) within the tick. A verifier applies them on
  encounter and enables the sticky contact mode at the TickHeader of the
  first one; the emitter's records for tick t must bit-match the
  verifier's own pass at tick t.
- Hashes and totals: unchanged in shape. BodyState records for tick t
  carry the post-impulse velocities; body/region/world hashes and
  TotalsState are computed from the emitted states as before. The
  touching set is replay state, not hashed state; verifiers reconstruct
  it by running the pass.
- Verification: bit-match as usual (Contact records and every affected
  BodyState/TotalsState/hash), plus (check framework, not format):
  jn > 0.0 for every emitted record; the relative normal speed of a
  pair measured immediately after its own impulse closes on 0.0 within
  rounding of the impulse arithmetic (later impulses in the same
  single pass may perturb other pairs — the next tick's pass resolves
  those); and the ledger is exactly invariant in an all-fine contact
  run. Position/energy drift against a contact-free reference is not a
  meaningful check for contact streams (contacts are dissipative by
  design); verifiers scope their reference-drift checks to contact-free
  runs.

CLI: `--contacts` enables contact mode for gravity runs; with it, the
output line gains `contacts=N` (records emitted). Without the flag the
CLI emits no Contact records and produces bit-identical output to
section 13-20 runs.

## 22. Modal audio (pure function of the stream)

Audio is a PURE FUNCTION of the record stream: contact events excite
damped resonant modes, synthesized offline so that any conforming
consumer produces bit-identical samples. Nothing about the simulation
depends on audio; a stream with no Contact records synthesizes silence
(the all-zero PCM block).

- Format: mono, 16-bit signed little-endian PCM, sample rate 65536 Hz.
  One tick spans exactly 64 samples (65536 = 1024 * 64: audio time is
  the sim's fixed dt scaled 2^-6 — no resampling anywhere). The
  container is the canonical 44-byte RIFF/WAVE header:
  "RIFF", u32 36 + data_size, "WAVE", "fmt ", u32 16, u16 1 (PCM),
  u16 1 (channels), u32 65536, u32 131072 (byte rate), u16 2 (block
  align), u16 16 (bits), "data", u32 data_size, then the PCM bytes.
- Excitation: a Contact record at tick t excites at sample index
  e = (t + 1) * 64 (the block after the tick that produced it).
  m_a, m_b are the masses of body_a, body_b in that tick's BodyState
  records (mass is constant per body; the tick's records are the
  normative source). mu = (m_a * m_b) / (m_a + m_b) — the same operand
  order as section 21.
- Each contact excites three modes k = 0, 1, 2 with pinned per-mode
  constants:
  partial coefficients C = [1.0, 4.0, 9.0]
  per-sample decay RHO = [0.9990, 0.9985, 0.9980]
  excitation gain AMP = [0.5, 0.3, 0.2]
  base OMEGA0 = 0.0004448824124529259
  (OMEGA0 = (2*pi*220/65536)^2; mode 1 of a reduced-mass-1 contact
  rings at 220 Hz; across the mass range modes span roughly 196-1320
  Hz. These are pinned constants of the format, not derivations.)
  omega_k = OMEGA0 * C[k] / mu
  a_k = (2.0 - omega_k) * RHO[k];  b_k = RHO[k] * RHO[k]
  Ring samples (second-order resonator recurrence, zero initial
  velocity):
  s_0 = AMP[k] * jn;  s_1 = a_k * s_0;
  s_n = a_k * s_{n-1} - b_k * s_{n-2}   for n >= 2
  Ring length L = 16384 samples.
- Mixing: an f64 accumulator buffer of N = (T + 260) * 64 samples,
  where T is the last TickHeader tick in the stream (260 blocks > L/64
  so the last ring decays fully). For each Contact record in stream
  order, for k = 0, 1, 2, for n = 0..L-1: buf[e + n] += s_n. This
  accumulation order is normative.
- Quantization: for each sample, v = min(max(buf[n], -1.0), 1.0);
  pcm[n] = floor(v * 32767.0 + 0.5) as a signed 16-bit integer (floor,
  not truncation — half-up rounding toward +infinity), serialized
  little-endian.
- Audio hash: FNV-1a64 (section 1) over the PCM data bytes only (the
  data chunk payload, not the RIFF header).
- Determinism: the synthesis uses only +, -, *, /, floor, and integer
  conversion on f64 — no libm transcendentals — so conforming
  implementations are bit-identical cross-platform, matching the
  section 11 replay guarantee.

CLI: `--contacts --wav FILE` additionally writes the synthesized WAV
and prints `audio=<fnv1a64-of-pcm>` (16 hex digits) on the output line.

## 23. Phase 4 v3: radial-shape synthesis (version 2, additive)

Section 20 matches the synthesized set's dipole and quadrupole, but a
tensor match does not pin pair distances and the synthesized-set energy
delta stays O(1): potential energy is dominated by the closest pairs,
which no finite set of moments controls. This section pins the
pair-distance structure directly — the collapse records its binding
(the exact internal potential statistic) and the expansion closes the
synthesis on it with a deterministic radial scale, plus a
velocity-spread scale that closes the synthesized kinetic energy on
the recorded total. The synthesized-set energy delta drops to
rounding. The trade is honest and bounded: the radial scale multiplies
the section 20 displacements, so the quadrupole closes only to
lambda^2 of the record (reported and bounded by the verifier, not
exact). No octupole term is added: moments beyond the quadrupole do
not constrain pair distances, the binding statistic does.

- New record, emitted immediately after every RegionMultipole record
  at the same boundary when the run is in radial mode:
  tag 11 RegionRadial: u64 tick, u32 region_x, u32 region_y,
    f64 binding
  Payload is 24 bytes plus the tag byte (8+4+4+8). A collapse cycle
  that carries a RegionRadial record expands per this section (the
  section 20 synthesis runs first, then the radial closure); a cycle
  without it expands per section 20 (or section 19 when the collapse
  carries no RegionMultipole record). For an empty collapse (N=0)
  binding = 0.0.
- binding = sum over member pairs i < j in id order of
  m_i * m_j / sqrt(d2_ij + eps2), accumulated in the same double loop
  and operand order as the section 19 energy's potential term (eps2 =
  1.0), so it is the exact negation of that accumulator.

- Expansion of a radial cycle. Draw sequence, section 19 velocity
  residual, and section 20 dipole residual are unchanged. Positions:
  1. Compute the section 20 base displacements (bx_i, by_i): the
     Cholesky-transformed recentered jitter (N >= 3, guards passed),
     otherwise the raw jitter. The dipole residual has NOT applied yet.
  2. Pair data, pairs p in lexicographic member order:
     W_p = m_i * m_j;  d2_p = (bx_j - bx_i)^2 + (by_j - by_i)^2.
     F(lambda) = sum_p W_p / sqrt(lambda * lambda * d2_p + 1.0),
     summed left-to-right in pair order.
  3. lambda: if the member count N < 2, lambda = 1.0. Else if
     binding >= F(0.0), lambda = 0.0. Else: hi = 1.0; while
     F(hi) > binding, hi = hi * 2.0, at most 64 doublings (pinned
     cap, not adaptive). Then lo = 0.0 and exactly 128 iterations:
     mid = (lo + hi) * 0.5; if F(mid) >= binding then lo = mid else
     hi = mid. lambda = (lo + hi) * 0.5, evaluated once after the
     loop. F is strictly decreasing on lambda >= 0 (every term is),
     the bracket F(lo) >= binding >= F(hi) is maintained, and 128
     halvings of a bracket of width <= 2^64 close past double
     precision. All evaluations are in the section 11 closure.
  4. Scale, recenter, then close: bx_i = lambda * bx_i and by_i =
     lambda * by_i for every member. Recenter the scaled displacements
     (restores the zero mass-weighted mean the dipole residual
     assumes): swx = sum_i m_i * bx_i over all members in id order
     (over the scaled values), wx = swx / mass (the RegionCollapsed
     mass field), bx_i = bx_i - wx, and likewise y. For transformed
     cycles the scaled mean is already ~0 and the shift is
     rounding-only; for untransformed cycles (N = 2, or skipped
     guards) the raw jitter's mean is scaled by lambda and must be
     removed. The recentering is a rigid translation: pair distances
     and the F(lambda) evaluation are unchanged by it. The section 20
     dipole residual then applies unchanged (x_i = com + bx_i for
     i < N-1; the last body absorbs (mx - Sx) / m_{N-1}, likewise y).
  Velocities: let ke(sigma) be the section 19 velocity synthesis with
  the spread draws multiplied by sigma (v_i = v_com + sigma * s_i for
  i < N-1; the last body absorbs the momentum residual computed from
  those velocities, same formulas), followed by the section 15 kinetic
  sum over all members in id order. ke(1.0) is bit-identical to the
  section 19/20 synthesized kinetic energy. With
    c0 = ke(0.0);  c1 = ke(1.0);  cm = ke(-1.0);
    a = ((c1 + cm) - (c0 + c0)) * 0.5;  b = (c1 - cm) * 0.5;
    K = energy + F(lambda)     (the RegionCollapsed energy field plus
                                the converged binding evaluation; in
                                exact arithmetic the synthesized
                                potential is -F(lambda));
    disc = b * b - 4.0 * a * (c0 - K);
  the spread scale is
    sigma = 0.0                          if a == 0.0
    sigma = (0.0 - b) / (2.0 * a)        if disc < 0.0 (unreachable
                                          target: the parabola vertex)
    else r1 = ((0.0 - b) + sqrt(disc)) / (2.0 * a),
         r2 = ((0.0 - b) - sqrt(disc)) / (2.0 * a),
         sigma = r1 if r1 * r1 <= r2 * r2, else r2.
  The final velocities are ke(sigma)'s synthesis: v_i = v_com +
  sigma * s_i (i < N-1), last body residual. a > 0 for N >= 2 (the
  parabola opens upward); the smaller-magnitude root is chosen
  deterministically.
- Momentum: unchanged — the residual body closes total momentum on
  (px, py) exactly as section 19; sigma commutes with the residual.
- Hashes and totals: synthesized states enter body/region/world
  hashes and totals exactly as emitted, unchanged.
- Verification: bit-match the RegionRadial binding field against the
  locally computed value, plus (check framework, not format): the
  synthesized set's internal potential (the binding formula over the
  synthesized positions) closes on the record (relative <= 1e-9);
  the synthesized-set total energy closes on the RegionCollapsed
  energy (relative <= 1e-9; measured at rounding); dipole closure is
  unchanged (<= 1e-12); the quadrupole deviation |Q_syn - Q_rec| /
  |Q_rec| is reported and bounded (measured <= 0.6 across seeds for
  transformed cycles, verifier bound 4.0; cycles that skipped the
  transform report 0). Tolerances live in the verifier's check
  framework.

CLI: `--radial` enables radial mode for gravity runs; collapses then
emit RegionRadial records (immediately after RegionMultipole) and
expansions run this section. Without the flag, output is
bit-identical to section 20 runs.

## 24. Contact extensions: restitution, friction, static contact, walls (version 2, additive)

Section 21 resolves fine-fine contacts perfectly inelastically and
frictionlessly, and non-fine bodies never contact. This section adds
restitution and Coulomb-clamped friction to the pinned impulse pass,
lets fine bodies resolve against frozen contactants — ephemeris-coarse
bodies, collapsed-region monopoles, and the walls of a bounded world —
and extends the momentum ledger rule to those one-sided impulses. The
pass stays single-sweep, lexicographic, operand-pinned: no iteration,
no solver tolerance. The parameters travel in the stream.

- New record, emitted at most once, immediately after the version 2
  header (before the first TickHeader):
  tag 12 ContactParams: f64 restitution, f64 friction, u8 walls
  Payload is 17 bytes plus the tag byte (8+8+1). restitution must lie
  in [0, 1], friction in [0, +inf), walls is 0 (unbounded plane, the
  section 21 world) or 1 (walls at the managed extent: the four
  segments x=0, x=128, y=0, y=128). A stream without this record is a
  section 21 stream and every contact behavior is bit-identical to
  section 21 (e = 0, friction = 0, no static contact, no walls). A
  verifier rejects the record if it appears twice, after the first
  TickHeader, or with out-of-range values. Parameters are sticky for
  the whole stream.

- Fine-fine pairs (the section 21 sweep, order unchanged):
  normal impulse: t = vn * inv;  s = (1.0 + e) * t;
  f_i = s * m_j;  f_j = s * m_i; applied exactly as section 21
  (v_i += f_i * n, v_j -= f_j * n). The post-impulse relative normal
  velocity is -e * vn (to rounding).
  friction (tangent (tx, ty) = (0.0 - ny, nx)):
  vt = (0.0 - vrx) * ny + vry * nx;
  q = vt * inv;  jn = ((0.0 - vn) * (1.0 + e)) * mu;
  qmax = (friction * jn) * inv;
  if q > qmax then q = qmax; if q < 0.0 - qmax then q = 0.0 - qmax;
  ft_i = q * m_j;  ft_j = q * m_i;
  vx_i += ft_i * (0.0 - ny);  vy_i += ft_i * nx;
  vx_j -= ft_j * (0.0 - ny);  vy_j -= ft_j * nx;
  (in exact arithmetic q = vt * inv zeroes the tangential relative
  velocity; the clamp is the Coulomb cone |impulse| <= friction * jn.)
  The recorded impulse magnitude is jn (friction is observable only
  through the trajectory). The friction impulse is applied only when
  friction > 0.0 — with friction = 0 the formulas degenerate to
  signed-zero additions, and skipping them keeps section 21 runs
  bit-identical. The normal and tangential pair products share their
  scalar operands, so the exchange conserves momentum to rounding;
  the ledger is untouched (fine-fine axiom, section 21).

- Static contactants (one-sided impulses). A fine body i resolved
  against a frozen contactant — an ephemeris-coarse body, a
  collapsed-region monopole, or a wall — changes only its own
  velocity; the contactant's state never changes (polynomials and
  collapse totals stay frozen; walls are immutable). With the
  contactant's velocity v_c (polynomial evaluation, v_com, or zero)
  and the normal n pointing from the body toward the contactant:
  vn = (vcx - vx_i) * nx + (vcy - vy_i) * ny;
  vt = (0.0 - (vcx - vx_i)) * ny + (vcy - vy_i) * nx;
  s = (1.0 + e) * vn;
  vx_i += s * nx;  vy_i += s * ny;
  jn = (0.0 - s) * m_i;
  jt = vt * m_i;  jt_max = friction * jn; clamp as above;
  w = jt / m_i;
  vx_i += w * (0.0 - ny);  vy_i += w * nx;
  Momentum ledger: after each one-sided application, immediately and
  in that order, the ledger books the intended impulse exactly:
    px += m_i * (s * nx);  py += m_i * (s * ny);
    px += jt * (0.0 - ny);  py += jt * nx;
  (jt = 0 exactly when friction = 0 — the tangential booking is
  skipped together with the application. The frozen contactant drops
  the reaction; the ledger books it, mirroring the section 13 fc-kick
  rule.) In an all-fine unbounded run the ledger stays exactly
  invariant for any e and friction.

- Coarse bodies as contactants: the pair (i fine, j ephemeris-coarse)
  joins the section 21 lexicographic sweep (section 21 skipped it).
  j's position and velocity are its polynomial evaluations at the
  tick; the resolution is one-sided on i. Coarse-coarse pairs are
  skipped (no movable member). Collapsed member bodies never
  participate individually; their region contacts as a monopole.

- Collapsed-region monopoles as contactants: after all body pairs,
  for each collapsed region r = 0..3 with body_count > 0, in region
  index order, for each fine body i in id order: the monopole is a
  disk of mass M (the RegionCollapsed mass field) at com with radius
  R = CONTACT_R * sqrt(M) (the section 21 radius law applied to M)
  and velocity v_com. Detection: dx = com_x - x_i; dy = com_y - y_i;
  rs = r_i + R; overlap iff dx * dx + dy * dy < rs * rs (normal
  pinned to (1, 0) at d2 == 0); resolve iff overlapping and
  vn < 0 (vn per the static formulas with v_c = v_com). Contact
  point: cx = (x_i + com_x) * 0.5; cy = (y_i + com_y) * 0.5.

- Walls: when walls = 1, after the monopole phase, for each fine body
  i in id order, walls are examined in the pinned order x=0, x=128,
  y=0, y=128 (a corner body resolves against two walls sequentially):
  x=0: overlap iff x_i - r_i < 0.0, approaching iff vx_i < 0.0,
       n = (-1, 0), contact point ((x_i + 0.0) * 0.5, y_i)
  x=128: overlap iff x_i + r_i > 128.0, approaching iff vx_i > 0.0,
       n = (1, 0), contact point ((x_i + 128.0) * 0.5, y_i)
  y=0: overlap iff y_i - r_i < 0.0, approaching iff vy_i < 0.0,
       n = (0, -1), contact point (x_i, (y_i + 0.0) * 0.5)
  y=128: overlap iff y_i + r_i > 128.0, approaching iff vy_i > 0.0,
       n = (0, 1), contact point (x_i, (y_i + 128.0) * 0.5)
  vn = vx_i * nx + vy_i * ny (v_c = 0); the static one-sided formulas
  apply with the pinned tangent rule.

- Records: Contact records are emitted for every phase in generation
  order — body pairs lexicographic (including fine x coarse static
  pairs, in the same sweep), then monopole contacts in region order
  (body id order within a region), then wall contacts in body id
  order (wall order within a body). Static contactants are named by
  pseudo ids in body_b:
  collapsed-region monopole of region r: body_b = 0xFF000000 + r
  walls x=0, x=128, y=0, y=128: body_b = 0xFFFFFF00 + 0..3
  body_a is always the fine body (real ids are small, so a < b
  holds). jn is the applied normal impulse magnitude (jn > 0); cx,
  cy the contact point above; the tick field is the tick whose pass
  produced the event. The touching-set rule of section 21 extends
  verbatim with keys (body_a, body_b) including pseudo ids: pairs
  leave the set by separating, by the fine body leaving the fine
  level, or by the region leaving collapse (its pseudo pairs drop).

- Audio (section 22 amendment): for a Contact record whose body_b is
  a pseudo id, mu = m_a for wall contacts (a wall is infinitely
  massive) and (m_a * M) / (m_a + M) for a monopole contact, with M
  the region's RegionCollapsed mass (operand order as section 22).
  Body-pair contacts are unchanged.

- Emission contract: Contact records for tick t are written
  immediately before that tick's TickHeader in generation order;
  ContactParams is written once, before the first TickHeader. All
  other records unchanged.

- Verification: bit-match as usual (ContactParams, Contact records,
  and every affected BodyState/TotalsState/hash), plus (check
  framework, not format): jn > 0 for every record; the relative
  normal speed of a pair measured immediately after its own impulse
  closes on -e * vn within rounding of the impulse arithmetic (later
  impulses in the same pass may perturb other pairs; the next tick's
  pass resolves those); the ledger is exactly invariant in an
  all-fine unbounded contact run for any e and friction; in runs
  with static contactants the ledger tracks the applied one-sided
  impulses exactly and the cumulative drift is reported.

CLI: `--restitution E`, `--friction F` (each requiring --contacts)
and `--walls` (implies --contacts) emit the ContactParams record with
the given values; with none of them, --contacts runs stay
bit-identical to section 21. The output line's contacts=N counts
every Contact record, pseudo-id contacts included.
