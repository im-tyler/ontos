use std::collections::BTreeMap;

use crate::fnv1a64;

pub const G: f64 = 1.0;
pub const EPS2: f64 = 1.0;
pub const DT: f64 = 1.0 / 1024.0;
pub const WINDOW: u64 = 32;
pub const DEGREE: usize = 8;
pub const SAMPLES: usize = 33;
pub const UNMANAGED: u8 = 255;

pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    pub fn draw(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub id: u32,
    pub mass: f64,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
}

impl Body {
    fn state_bytes(&self, level: u8) -> Vec<u8> {
        let mut v = Vec::with_capacity(46);
        v.extend_from_slice(&self.id.to_le_bytes());
        v.extend_from_slice(&self.x.to_le_bytes());
        v.extend_from_slice(&self.y.to_le_bytes());
        v.extend_from_slice(&self.vx.to_le_bytes());
        v.extend_from_slice(&self.vy.to_le_bytes());
        v.extend_from_slice(&self.mass.to_le_bytes());
        v.push(level);
        v
    }
}

#[derive(Clone)]
pub struct Fit {
    pub c: [[f64; 9]; 4],
    pub t0: u64,
}

pub struct GravityWorld {
    pub seed: u64,
    pub bodies: Vec<Body>,
    pub coarse: Vec<Option<Fit>>,
    pub body_region: Vec<u8>,
    pub region_coarse: [bool; 4],
    pub events: BTreeMap<u64, Vec<(u8, bool)>>,
    pub tick: u64,
    pub px: f64,
    pub py: f64,
}

pub fn initial_conditions(seed: u64, count: u32) -> Vec<Body> {
    let mut rng = SplitMix64::new(seed);
    let mut bodies = Vec::with_capacity(count as usize);
    for id in 0..count {
        let u0 = rng.draw();
        let u1 = rng.draw();
        let u2 = rng.draw();
        let u3 = rng.draw();
        let u4 = rng.draw();
        bodies.push(Body {
            id,
            mass: 0.5 + (u0 as f64) * 2.0f64.powi(-64) * 2.0,
            x: 32.0 + (u1 as f64) * 2.0f64.powi(-64) * 64.0,
            y: 32.0 + (u2 as f64) * 2.0f64.powi(-64) * 64.0,
            vx: ((u3 as f64) * 2.0f64.powi(-64) - 0.5) * 0.5,
            vy: ((u4 as f64) * 2.0f64.powi(-64) - 0.5) * 0.5,
        });
    }
    bodies
}

pub fn region_at(x: f64, y: f64) -> u8 {
    if x < 0.0 || x >= 128.0 || y < 0.0 || y >= 128.0 {
        return UNMANAGED;
    }
    let rx = (x / 64.0) as u8;
    let ry = (y / 64.0) as u8;
    ry * 2 + rx
}

impl GravityWorld {
    pub fn new(seed: u64, count: u32) -> Self {
        let bodies = initial_conditions(seed, count);
        let mut px = 0.0f64;
        let mut py = 0.0f64;
        for b in &bodies {
            px += b.mass * b.vx;
            py += b.mass * b.vy;
        }
        GravityWorld {
            seed,
            bodies,
            coarse: vec![None; count as usize],
            body_region: vec![UNMANAGED; count as usize],
            region_coarse: [false; 4],
            events: BTreeMap::new(),
            tick: 0,
            px,
            py,
        }
    }

    pub fn schedule(&mut self, tick: u64, region: u8, to_coarse: bool) {
        self.events
            .entry(tick)
            .or_default()
            .push((region, to_coarse));
    }

    fn eval_fit(fit: &Fit, t: u64) -> (f64, f64, f64, f64) {
        let s = -1.0 + ((t - fit.t0) as f64) / 32.0;
        (
            clenshaw(&fit.c[0], s),
            clenshaw(&fit.c[1], s),
            clenshaw(&fit.c[2], s),
            clenshaw(&fit.c[3], s),
        )
    }

    fn body_state_at(&self, i: usize, t: u64) -> Body {
        let mut b = self.bodies[i];
        if let Some(fit) = &self.coarse[i] {
            let (x, y, vx, vy) = Self::eval_fit(fit, t);
            b.x = x;
            b.y = y;
            b.vx = vx;
            b.vy = vy;
        }
        b
    }

    fn box_of(region: u8) -> (f64, f64, f64, f64) {
        let rx = (region % 2) as f64 * 64.0;
        let ry = (region / 2) as f64 * 64.0;
        (rx, ry, rx + 64.0, ry + 64.0)
    }

    fn pre_integrate(bodies: &[Body]) -> Vec<[Body; SAMPLES]> {
        let mut samples: Vec<[Body; SAMPLES]> = bodies.iter().map(|b| [*b; SAMPLES]).collect();
        let mut cur: Vec<Body> = bodies.to_vec();
        for k in 1..SAMPLES {
            Self::leapfrog_restricted(&mut cur);
            for (row, b) in samples.iter_mut().zip(cur.iter()) {
                row[k] = *b;
            }
        }
        samples
    }

    fn leapfrog_restricted(bodies: &mut [Body]) {
        let mut ax = vec![0.0f64; bodies.len()];
        let mut ay = vec![0.0f64; bodies.len()];
        Self::accumulate_accel(bodies, &mut ax, &mut ay, None);
        let half = DT * 0.5;
        for i in 0..bodies.len() {
            bodies[i].vx += ax[i] * half;
            bodies[i].vy += ay[i] * half;
        }
        for b in bodies.iter_mut() {
            b.x += b.vx * DT;
            b.y += b.vy * DT;
        }
        let mut ax2 = vec![0.0f64; bodies.len()];
        let mut ay2 = vec![0.0f64; bodies.len()];
        Self::accumulate_accel(bodies, &mut ax2, &mut ay2, None);
        for i in 0..bodies.len() {
            bodies[i].vx += ax2[i] * half;
            bodies[i].vy += ay2[i] * half;
        }
    }

    fn accumulate_accel(
        bodies: &[Body],
        ax: &mut [f64],
        ay: &mut [f64],
        skip: Option<&dyn Fn(usize) -> bool>,
    ) {
        for i in 0..bodies.len() {
            for j in (i + 1)..bodies.len() {
                let dx = bodies[j].x - bodies[i].x;
                let dy = bodies[j].y - bodies[i].y;
                let s2 = dx * dx + dy * dy + EPS2;
                let inv3 = 1.0 / (s2 * s2.sqrt());
                let fx = G * inv3 * dx;
                let fy = G * inv3 * dy;
                let i_active = skip.is_none_or(|f| !f(i));
                let j_active = skip.is_none_or(|f| !f(j));
                if i_active {
                    ax[i] += bodies[j].mass * fx;
                    ay[i] += bodies[j].mass * fy;
                }
                if j_active {
                    ax[j] -= bodies[i].mass * fx;
                    ay[j] -= bodies[i].mass * fy;
                }
            }
        }
    }

    fn demote_region(&mut self, region: u8, t0: u64) {
        let (x0, y0, x1, y1) = Self::box_of(region);
        let members: Vec<usize> = (0..self.bodies.len())
            .filter(|&i| {
                let b = self.body_state_at(i, t0);
                b.x >= x0 && b.x < x1 && b.y >= y0 && b.y < y1
            })
            .collect();
        if members.is_empty() {
            self.region_coarse[region as usize] = true;
            return;
        }
        let subset: Vec<Body> = members.iter().map(|&i| self.body_state_at(i, t0)).collect();
        let samples = Self::pre_integrate(&subset);
        for (slot, &i) in members.iter().enumerate() {
            self.coarse[i] = Some(Fit {
                c: [
                    project(&samples[slot], |b| b.x),
                    project(&samples[slot], |b| b.y),
                    project(&samples[slot], |b| b.vx),
                    project(&samples[slot], |b| b.vy),
                ],
                t0,
            });
            self.body_region[i] = region;
        }
        self.region_coarse[region as usize] = true;
    }

    fn promote_region(&mut self, region: u8, t: u64) {
        for i in 0..self.bodies.len() {
            if self.coarse[i].is_some() && self.body_region[i] == region {
                let b = self.body_state_at(i, t);
                self.bodies[i] = b;
                self.coarse[i] = None;
                self.body_region[i] = UNMANAGED;
            }
        }
        self.region_coarse[region as usize] = false;
    }

    fn refit_region(&mut self, region: u8, t: u64) {
        let (x0, y0, x1, y1) = Self::box_of(region);
        let members: Vec<usize> = (0..self.bodies.len())
            .filter(|&i| self.coarse[i].is_some() && self.body_region[i] == region)
            .collect();
        let subset: Vec<Body> = members.iter().map(|&i| self.body_state_at(i, t)).collect();
        let mut keep: Vec<usize> = Vec::new();
        for (slot, &i) in members.iter().enumerate() {
            self.bodies[i] = subset[slot];
            if subset[slot].x >= x0
                && subset[slot].x < x1
                && subset[slot].y >= y0
                && subset[slot].y < y1
            {
                keep.push(i);
            } else {
                self.coarse[i] = None;
                self.body_region[i] = UNMANAGED;
            }
        }
        if keep.is_empty() {
            self.region_coarse[region as usize] = false;
            return;
        }
        let subset: Vec<Body> = keep.iter().map(|&i| self.bodies[i]).collect();
        let samples = Self::pre_integrate(&subset);
        for (slot, &i) in keep.iter().enumerate() {
            self.coarse[i] = Some(Fit {
                c: [
                    project(&samples[slot], |b| b.x),
                    project(&samples[slot], |b| b.y),
                    project(&samples[slot], |b| b.vx),
                    project(&samples[slot], |b| b.vy),
                ],
                t0: t,
            });
            self.body_region[i] = region;
        }
    }

    pub fn step(&mut self) {
        let entering = self.tick + 1;
        if let Some(events) = self.events.remove(&entering) {
            for &(region, to_coarse) in &events {
                if to_coarse {
                    self.demote_region(region, entering);
                } else {
                    self.promote_region(region, entering);
                }
            }
        }
        for region in 0..4u8 {
            if self.region_coarse[region as usize] {
                let ended = (0..self.bodies.len()).any(|i| {
                    self.body_region[i] == region
                        && self.coarse[i]
                            .as_ref()
                            .is_some_and(|f| entering == f.t0 + WINDOW)
                });
                if ended {
                    self.refit_region(region, entering);
                }
            }
        }

        let n = self.bodies.len();
        let coarse: Vec<bool> = (0..n).map(|i| self.coarse[i].is_some()).collect();
        let half = DT * 0.5;
        let view: Vec<Body> = (0..n).map(|i| self.body_state_at(i, entering)).collect();
        let (ax_ff, ay_ff, ax_fc, ay_fc) = accel_split(&view, &coarse);
        for i in 0..n {
            if !coarse[i] {
                self.bodies[i].vx += ax_ff[i] * half;
                self.bodies[i].vy += ay_ff[i] * half;
            }
        }
        for i in 0..n {
            if !coarse[i] {
                self.bodies[i].vx += ax_fc[i] * half;
                self.bodies[i].vy += ay_fc[i] * half;
                self.px += self.bodies[i].mass * (ax_fc[i] * half);
                self.py += self.bodies[i].mass * (ay_fc[i] * half);
            }
        }
        for (i, b) in self.bodies.iter_mut().enumerate() {
            if !coarse[i] {
                b.x += b.vx * DT;
                b.y += b.vy * DT;
            }
        }
        let view: Vec<Body> = (0..n).map(|i| self.body_state_at(i, entering)).collect();
        let (ax_ff, ay_ff, ax_fc, ay_fc) = accel_split(&view, &coarse);
        for (i, b) in self.bodies.iter_mut().enumerate() {
            if !coarse[i] {
                b.vx += ax_ff[i] * half;
                b.vy += ay_ff[i] * half;
            }
        }
        for i in 0..n {
            if !coarse[i] {
                self.bodies[i].vx += ax_fc[i] * half;
                self.bodies[i].vy += ay_fc[i] * half;
                self.px += self.bodies[i].mass * (ax_fc[i] * half);
                self.py += self.bodies[i].mass * (ay_fc[i] * half);
            }
        }
        self.tick = entering;
    }

    pub fn totals(&self) -> (u64, u64, f64, f64, f64, f64) {
        let n = self.bodies.len();
        let view: Vec<Body> = (0..n).map(|i| self.body_state_at(i, self.tick)).collect();
        let mut fine = 0u64;
        let mut coarse = 0u64;
        let mut mass = 0.0f64;
        let mut ke = 0.0f64;
        for b in &view {
            if self.coarse[b.id as usize].is_some() {
                coarse += 1;
            } else {
                fine += 1;
            }
            mass += b.mass;
            ke += 0.5 * b.mass * (b.vx * b.vx + b.vy * b.vy);
        }
        let mut pe = 0.0f64;
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = view[j].x - view[i].x;
                let dy = view[j].y - view[i].y;
                let s2 = dx * dx + dy * dy + EPS2;
                pe -= view[i].mass * view[j].mass / s2.sqrt();
            }
        }
        (fine, coarse, mass, self.px, self.py, ke + pe)
    }

    pub fn body_state_bytes(&self, i: usize) -> Vec<u8> {
        let b = self.body_state_at(i, self.tick);
        let level = if self.coarse[i].is_some() { 0u8 } else { 1u8 };
        b.state_bytes(level)
    }

    pub fn emitted_state(&self, i: usize) -> (Body, u8, u8) {
        let b = self.body_state_at(i, self.tick);
        let level = if self.coarse[i].is_some() { 0u8 } else { 1u8 };
        let region = if self.coarse[i].is_some() {
            self.body_region[i]
        } else {
            region_at(b.x, b.y)
        };
        (b, region, level)
    }

    pub fn world_hash(&self) -> u64 {
        let mut v = Vec::new();
        v.extend_from_slice(&self.tick.to_le_bytes());
        for i in 0..self.bodies.len() {
            v.extend_from_slice(&self.body_state_bytes(i));
        }
        fnv1a64(&v)
    }

    pub fn region_hash(&self, region: u8) -> (u8, u64, u64) {
        let n = self.bodies.len();
        let mut members: Vec<usize> = Vec::new();
        for i in 0..n {
            if self.coarse[i].is_some() {
                if self.body_region[i] == region {
                    members.push(i);
                }
            } else {
                let b = self.body_state_at(i, self.tick);
                if region_at(b.x, b.y) == region {
                    members.push(i);
                }
            }
        }
        let level = if self.region_coarse[region as usize] {
            0u8
        } else {
            1u8
        };
        let mut v = Vec::new();
        v.push(level);
        for &i in &members {
            v.extend_from_slice(&self.body_state_bytes(i));
        }
        (level, members.len() as u64, fnv1a64(&v))
    }
}

fn accel_split(view: &[Body], coarse: &[bool]) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = view.len();
    let mut ax_ff = vec![0.0f64; n];
    let mut ay_ff = vec![0.0f64; n];
    let mut ax_fc = vec![0.0f64; n];
    let mut ay_fc = vec![0.0f64; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let dx = view[j].x - view[i].x;
            let dy = view[j].y - view[i].y;
            let s2 = dx * dx + dy * dy + EPS2;
            let inv3 = 1.0 / (s2 * s2.sqrt());
            let fx = G * inv3 * dx;
            let fy = G * inv3 * dy;
            match (coarse[i], coarse[j]) {
                (false, false) => {
                    ax_ff[i] += view[j].mass * fx;
                    ay_ff[i] += view[j].mass * fy;
                    ax_ff[j] -= view[i].mass * fx;
                    ay_ff[j] -= view[i].mass * fy;
                }
                (false, true) => {
                    ax_fc[i] += view[j].mass * fx;
                    ay_fc[i] += view[j].mass * fy;
                }
                (true, false) => {
                    ax_fc[j] -= view[i].mass * fx;
                    ay_fc[j] -= view[i].mass * fy;
                }
                (true, true) => {}
            }
        }
    }
    (ax_ff, ay_ff, ax_fc, ay_fc)
}

fn clenshaw(c: &[f64; 9], s: f64) -> f64 {
    let mut b1 = 0.0f64;
    let mut b2 = 0.0f64;
    for j in (1..=DEGREE).rev() {
        let b0 = c[j] + 2.0 * s * b1 - b2;
        b2 = b1;
        b1 = b0;
    }
    c[0] + s * b1 - b2
}

fn project(samples: &[Body; SAMPLES], get: impl Fn(&Body) -> f64) -> [f64; 9] {
    let mut y = [0.0f64; SAMPLES];
    for (k, b) in samples.iter().enumerate() {
        y[k] = get(b);
    }
    let mut t = [[0.0f64; SAMPLES]; DEGREE + 1];
    let mut w = [1.0f64; SAMPLES];
    w[0] = 0.5;
    w[SAMPLES - 1] = 0.5;
    #[allow(clippy::needless_range_loop)]
    for k in 0..SAMPLES {
        let s = -1.0 + (k as f64) / 16.0;
        t[0][k] = 1.0;
        t[1][k] = s;
        for j in 2..=DEGREE {
            t[j][k] = 2.0 * s * t[j - 1][k] - t[j - 2][k];
        }
    }
    let mut g = [[0.0f64; DEGREE + 1]; DEGREE + 1];
    for j in 0..=DEGREE {
        for l in 0..=DEGREE {
            let mut sum = 0.0f64;
            for k in 0..SAMPLES {
                sum += w[k] * t[j][k] * t[l][k];
            }
            g[j][l] = sum;
        }
    }
    let mut b = [0.0f64; DEGREE + 1];
    for j in 0..=DEGREE {
        let mut sum = 0.0f64;
        for k in 0..SAMPLES {
            sum += w[k] * y[k] * t[j][k];
        }
        b[j] = sum;
    }
    cholesky_solve(&g, &b)
}

#[allow(clippy::needless_range_loop)]
fn cholesky_solve(g: &[[f64; 9]; 9], b: &[f64; 9]) -> [f64; 9] {
    let n = DEGREE + 1;
    let mut l = [[0.0f64; 9]; 9];
    for i in 0..n {
        for j in 0..=i {
            let mut sum = g[i][j];
            for k in 0..j {
                sum -= l[i][k] * l[j][k];
            }
            if i == j {
                l[i][j] = sum.sqrt();
            } else {
                l[i][j] = sum / l[j][j];
            }
        }
    }
    let mut z = [0.0f64; 9];
    for i in 0..n {
        let mut sum = b[i];
        for k in 0..i {
            sum -= l[i][k] * z[k];
        }
        z[i] = sum / l[i][i];
    }
    let mut c = [0.0f64; 9];
    for i in (0..n).rev() {
        let mut sum = z[i];
        for k in (i + 1)..n {
            sum -= l[k][i] * c[k];
        }
        c[i] = sum / l[i][i];
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_reference() {
        let mut rng = SplitMix64::new(42);
        let a = rng.draw();
        let b = rng.draw();
        assert_ne!(a, 0);
        assert_ne!(b, a);
    }

    #[test]
    fn gravity_replay_bit_identical() {
        let run = || {
            let mut w = GravityWorld::new(7, 12);
            w.schedule(5, 1, true);
            w.schedule(70, 1, false);
            w.schedule(40, 2, true);
            let mut hashes = Vec::new();
            for _ in 0..120 {
                w.step();
                hashes.push(w.world_hash());
            }
            hashes
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn momentum_exactly_conserved_all_fine() {
        let mut w = GravityWorld::new(3, 10);
        let (_, _, _, px0, py0, _) = w.totals();
        for _ in 0..200 {
            w.step();
        }
        let (_, _, _, px1, py1, _) = w.totals();
        assert_eq!(px0, px1);
        assert_eq!(py0, py1);
    }

    #[test]
    fn energy_drift_bounded_all_fine() {
        let mut w = GravityWorld::new(11, 8);
        let e0 = w.totals().5;
        for _ in 0..500 {
            w.step();
        }
        let e1 = w.totals().5;
        assert!((e1 - e0).abs() / e0.abs() < 1e-6);
    }

    #[test]
    fn chebyshev_roundtrip_error_small() {
        let mut samples = [Body {
            id: 0,
            mass: 1.0,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
        }; SAMPLES];
        for (k, s) in samples.iter_mut().enumerate() {
            s.x = (k as f64) * 0.1 * (k as f64) * 0.05 + 1.0;
        }
        let c = project(&samples, |b| b.x);
        for (k, sample) in samples.iter().enumerate() {
            let s = -1.0 + (k as f64) / 16.0;
            let approx = clenshaw(&c, s);
            assert!((approx - sample.x).abs() < 1e-9);
        }
    }
}
