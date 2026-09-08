pub const REGION_FINE: u32 = 64;
pub const REGIONS_PER_AXIS: u32 = 2;
pub const COARSE_FACTOR: u32 = 2;

pub mod audio;
pub mod gravity;

pub const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
pub const FNV_PRIME: u64 = 0x100000001b3;

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Coarse,
    Fine,
}

impl Level {
    pub fn cell_count_per_axis(self) -> u32 {
        match self {
            Level::Coarse => REGION_FINE / COARSE_FACTOR,
            Level::Fine => REGION_FINE,
        }
    }
}

pub struct Region {
    pub level: Level,
    pub cells: Vec<u8>,
}

impl Region {
    fn new(level: Level) -> Self {
        let n = level.cell_count_per_axis();
        Region {
            level,
            cells: vec![0; (n * n) as usize],
        }
    }

    fn n(&self) -> u32 {
        self.level.cell_count_per_axis()
    }

    pub fn population(&self) -> u64 {
        self.cells.iter().map(|&c| c as u64).sum()
    }
}

pub struct World {
    pub seed: u64,
    pub regions: Vec<Region>,
    pub tick: u64,
}

impl World {
    pub fn new(seed: u64) -> Self {
        let count = (REGIONS_PER_AXIS * REGIONS_PER_AXIS) as usize;
        World {
            seed,
            regions: (0..count).map(|_| Region::new(Level::Fine)).collect(),
            tick: 0,
        }
    }

    fn region_index(rx: u32, ry: u32) -> usize {
        (ry * REGIONS_PER_AXIS + rx) as usize
    }

    pub fn set_level(&mut self, rx: u32, ry: u32, level: Level) {
        let current = &self.regions[Self::region_index(rx, ry)];
        if current.level == level {
            return;
        }
        let converted = match level {
            Level::Fine => Self::promote_region(current, rx, ry, self.seed),
            Level::Coarse => Self::demote_region(current),
        };
        self.regions[Self::region_index(rx, ry)] = converted;
    }

    pub fn promote(&mut self, rx: u32, ry: u32) {
        self.set_level(rx, ry, Level::Fine);
    }

    pub fn demote(&mut self, rx: u32, ry: u32) {
        self.set_level(rx, ry, Level::Coarse);
    }

    fn promote_region(region: &Region, rx: u32, ry: u32, seed: u64) -> Region {
        let mut fine = Region::new(Level::Fine);
        let cn = region.n();
        for cy in 0..cn {
            for cx in 0..cn {
                if region.cells[(cy * cn + cx) as usize] == 0 {
                    continue;
                }
                let gx = rx * REGION_FINE + cx * COARSE_FACTOR;
                let gy = ry * REGION_FINE + cy * COARSE_FACTOR;
                let pick = Self::expansion_pick(gx, gy, seed);
                let (dx, dy) = match pick {
                    0 => (0u32, 0u32),
                    1 => (1, 0),
                    2 => (0, 1),
                    _ => (1, 1),
                };
                let fx = cx * COARSE_FACTOR + dx;
                let fy = cy * COARSE_FACTOR + dy;
                fine.cells[(fy * REGION_FINE + fx) as usize] = 1;
            }
        }
        fine
    }

    fn demote_region(region: &Region) -> Region {
        let mut coarse = Region::new(Level::Coarse);
        let cn = coarse.n();
        for cy in 0..cn {
            for cx in 0..cn {
                let mut alive = 0;
                for dy in 0..COARSE_FACTOR {
                    for dx in 0..COARSE_FACTOR {
                        let fx = cx * COARSE_FACTOR + dx;
                        let fy = cy * COARSE_FACTOR + dy;
                        alive |= region.cells[(fy * REGION_FINE + fx) as usize];
                    }
                }
                coarse.cells[(cy * cn + cx) as usize] = alive;
            }
        }
        coarse
    }

    fn expansion_pick(gx: u32, gy: u32, seed: u64) -> u32 {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&seed.to_le_bytes());
        bytes[8..12].copy_from_slice(&gx.to_le_bytes());
        bytes[12..].copy_from_slice(&gy.to_le_bytes());
        (fnv1a64(&bytes) % 4) as u32
    }

    pub fn read(&self, fx: u32, fy: u32) -> u8 {
        let world = REGIONS_PER_AXIS * REGION_FINE;
        let fx = fx.rem_euclid(world);
        let fy = fy.rem_euclid(world);
        let rx = fx / REGION_FINE;
        let ry = fy / REGION_FINE;
        let region = &self.regions[Self::region_index(rx, ry)];
        match region.level {
            Level::Fine => {
                region.cells[((fy % REGION_FINE) * REGION_FINE + (fx % REGION_FINE)) as usize]
            }
            Level::Coarse => {
                let cx = (fx % REGION_FINE) / COARSE_FACTOR;
                let cy = (fy % REGION_FINE) / COARSE_FACTOR;
                let cn = region.n();
                region.cells[(cy * cn + cx) as usize]
            }
        }
    }

    pub fn read_block(&self, cx: u32, cy: u32) -> u8 {
        let coarse_world = REGIONS_PER_AXIS * REGION_FINE / COARSE_FACTOR;
        let cx = cx.rem_euclid(coarse_world);
        let cy = cy.rem_euclid(coarse_world);
        let rx = (cx * COARSE_FACTOR) / REGION_FINE;
        let ry = (cy * COARSE_FACTOR) / REGION_FINE;
        let region = &self.regions[Self::region_index(rx, ry)];
        match region.level {
            Level::Coarse => {
                let cn = region.n();
                region.cells[((cy % cn) * cn + (cx % cn)) as usize]
            }
            Level::Fine => {
                let base_fx = cx * COARSE_FACTOR;
                let base_fy = cy * COARSE_FACTOR;
                let mut alive = 0;
                for dy in 0..COARSE_FACTOR {
                    for dx in 0..COARSE_FACTOR {
                        let fx = base_fx + dx;
                        let fy = base_fy + dy;
                        alive |= self.read(fx, fy);
                    }
                }
                alive
            }
        }
    }

    pub fn set_fine(&mut self, fx: u32, fy: u32, alive: bool) {
        let rx = fx / REGION_FINE;
        let ry = fy / REGION_FINE;
        let region = &mut self.regions[Self::region_index(rx, ry)];
        let idx = match region.level {
            Level::Fine => ((fy % REGION_FINE) * REGION_FINE + (fx % REGION_FINE)) as usize,
            Level::Coarse => {
                let cn = region.n();
                (((fy % REGION_FINE) / COARSE_FACTOR) * cn + ((fx % REGION_FINE) / COARSE_FACTOR))
                    as usize
            }
        };
        region.cells[idx] = alive as u8;
    }

    pub fn seed_r_pentomino(&mut self) {
        let c = REGIONS_PER_AXIS * REGION_FINE / 2;
        let cells = [(0, 0), (1, 0), (0, 1), (-1, 1), (0, 2)];
        for &(dx, dy) in &cells {
            let x = (c as i32 + dx).rem_euclid((REGIONS_PER_AXIS * REGION_FINE) as i32) as u32;
            let y = (c as i32 + dy).rem_euclid((REGIONS_PER_AXIS * REGION_FINE) as i32) as u32;
            self.set_fine(x, y, true);
        }
    }

    pub fn population(&self) -> u64 {
        self.regions.iter().map(|r| r.population()).sum()
    }

    pub fn region(&self, rx: u32, ry: u32) -> &Region {
        &self.regions[Self::region_index(rx, ry)]
    }

    pub fn region_hash(&self, rx: u32, ry: u32) -> u64 {
        let region = &self.regions[Self::region_index(rx, ry)];
        let mut bytes = Vec::with_capacity(1 + region.cells.len());
        bytes.push(match region.level {
            Level::Coarse => 0,
            Level::Fine => 1,
        });
        bytes.extend_from_slice(&region.cells);
        fnv1a64(&bytes)
    }

    pub fn hash_state(&self) -> u64 {
        let mut bytes = Vec::with_capacity(8 + 8 * self.regions.len());
        bytes.extend_from_slice(&self.tick.to_le_bytes());
        for ry in 0..REGIONS_PER_AXIS {
            for rx in 0..REGIONS_PER_AXIS {
                bytes.extend_from_slice(&self.region_hash(rx, ry).to_le_bytes());
            }
        }
        fnv1a64(&bytes)
    }

    pub fn step(&mut self) {
        let mut next_regions = Vec::with_capacity(self.regions.len());
        for (i, region) in self.regions.iter().enumerate() {
            let rx = (i as u32) % REGIONS_PER_AXIS;
            let ry = (i as u32) / REGIONS_PER_AXIS;
            let mut next = Region::new(region.level);
            match region.level {
                Level::Fine => {
                    for fy in 0..REGION_FINE {
                        for fx in 0..REGION_FINE {
                            let gx = rx * REGION_FINE + fx;
                            let gy = ry * REGION_FINE + fy;
                            let n = self.fine_neighbors(gx, gy);
                            let alive = region.cells[(fy * REGION_FINE + fx) as usize] == 1;
                            next.cells[(fy * REGION_FINE + fx) as usize] =
                                (alive && (n == 2 || n == 3) || !alive && n == 3) as u8;
                        }
                    }
                }
                Level::Coarse => {
                    let cn = region.n();
                    for cy in 0..cn {
                        for cx in 0..cn {
                            let gx = rx * (REGION_FINE / COARSE_FACTOR) + cx;
                            let gy = ry * (REGION_FINE / COARSE_FACTOR) + cy;
                            let n = self.coarse_neighbors(gx, gy);
                            let alive = region.cells[(cy * cn + cx) as usize] == 1;
                            next.cells[(cy * cn + cx) as usize] =
                                (alive && (n == 2 || n == 3) || !alive && n == 3) as u8;
                        }
                    }
                }
            }
            next_regions.push(next);
        }
        self.regions = next_regions;
        self.tick += 1;
    }

    fn resolution_key(&self, fx: u32, fy: u32) -> (u8, u32, u32) {
        let rx = fx / REGION_FINE;
        let ry = fy / REGION_FINE;
        match self.regions[Self::region_index(rx, ry)].level {
            Level::Fine => (0, fx, fy),
            Level::Coarse => (1, fx / COARSE_FACTOR, fy / COARSE_FACTOR),
        }
    }

    fn fine_neighbors(&self, gx: u32, gy: u32) -> u8 {
        let world = REGIONS_PER_AXIS * REGION_FINE;
        let mut seen: [Option<(u8, u32, u32)>; 8] = [None; 8];
        let mut count = 0u8;
        let mut k = 0usize;
        for dy in [-1i32, 0, 1] {
            for dx in [-1i32, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = (gx as i32 + dx).rem_euclid(world as i32) as u32;
                let ny = (gy as i32 + dy).rem_euclid(world as i32) as u32;
                if self.read(nx, ny) == 0 {
                    continue;
                }
                let key = self.resolution_key(nx, ny);
                if seen[..k].contains(&Some(key)) {
                    continue;
                }
                seen[k] = Some(key);
                k += 1;
                count += 1;
            }
        }
        count
    }

    fn coarse_neighbors(&self, gx: u32, gy: u32) -> u8 {
        let mut n = 0;
        for dy in [-1i32, 0, 1] {
            for dx in [-1i32, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let w = (REGIONS_PER_AXIS * REGION_FINE / COARSE_FACTOR) as i32;
                let nx = (gx as i32 + dx).rem_euclid(w) as u32;
                let ny = (gy as i32 + dy).rem_euclid(w) as u32;
                n += self.read_block(nx, ny);
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_known_values() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }

    #[test]
    fn deterministic_replay() {
        let run = || {
            let mut w = World::new(42);
            w.seed_r_pentomino();
            w.demote(1, 0);
            w.demote(0, 1);
            let mut hashes = Vec::new();
            for _ in 0..200 {
                w.step();
                hashes.push(w.hash_state());
            }
            hashes
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn promote_preserves_population() {
        let mut w = World::new(7);
        w.seed_r_pentomino();
        w.demote(0, 0);
        let before = w.regions[World::region_index(0, 0)].population();
        w.promote(0, 0);
        let after = w.regions[World::region_index(0, 0)].population();
        assert_eq!(before, after);
    }

    #[test]
    fn demote_promote_roundtrip_identity() {
        let mut w = World::new(9);
        w.seed_r_pentomino();
        w.demote(0, 0);
        let coarse_before = w.regions[World::region_index(0, 0)].cells.clone();
        w.promote(0, 0);
        w.demote(0, 0);
        let coarse_after = w.regions[World::region_index(0, 0)].cells.clone();
        assert_eq!(coarse_before, coarse_after);
    }

    #[test]
    fn coarse_read_covers_block() {
        let mut w = World::new(1);
        w.set_fine(70, 66, true);
        w.demote(1, 1);
        assert_eq!(w.read(70, 66), 1);
        assert_eq!(w.read(71, 67), 1);
        assert_eq!(w.read(68, 64), 0);
    }

    #[test]
    fn fine_region_counts_coarse_neighbor() {
        let mut w = World::new(3);
        w.demote(1, 0);
        w.set_fine(64, 10, true);
        assert_eq!(w.read(64, 10), 1);
        assert_eq!(w.fine_neighbors(63, 10), 1);
        assert_eq!(w.fine_neighbors(60, 10), 0);
    }
}
