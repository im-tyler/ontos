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
