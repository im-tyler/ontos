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
