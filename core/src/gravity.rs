use std::collections::{BTreeMap, BTreeSet};

use crate::fnv1a64;

pub const G: f64 = 1.0;
pub const EPS2: f64 = 1.0;
pub const DT: f64 = 1.0 / 1024.0;
pub const WINDOW: u64 = 32;
pub const DEGREE: usize = 8;
pub const SAMPLES: usize = 33;
pub const UNMANAGED: u8 = 255;
pub const CONTACT_R: f64 = 2.0;
pub const MONOPOLE_BASE: u32 = 0xFF00_0000;
pub const WALL_BASE: u32 = 0xFFFF_FF00;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Demote,
    Promote,
    Collapse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionMode {
    Fine,
    Coarse,
    Collapsed,
}

#[derive(Clone, Copy, Debug)]
pub struct CollapsedBody {
    pub jx: f64,
    pub jy: f64,
    pub sx: f64,
    pub sy: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct CollapseTotals {
    pub tick: u64,
    pub region: u8,
    pub count: u64,
    pub mass: f64,
    pub com_x: f64,
    pub com_y: f64,
    pub px: f64,
    pub py: f64,
    pub energy: f64,
    pub vcom_x: f64,
    pub vcom_y: f64,
    pub mx: f64,
    pub my: f64,
    pub qxx: f64,
    pub qxy: f64,
    pub qyy: f64,
    pub binding: f64,
    pub radial: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactEvent {
    pub tick: u64,
    pub a: u32,
    pub b: u32,
    pub jn: f64,
    pub cx: f64,
    pub cy: f64,
    pub vn: f64,
    pub vn_after: f64,
    pub mu: f64,
}

pub struct Observer {
    rng: SplitMix64,
    points: Vec<(f64, f64)>,
    pub policy_events: Vec<(u8, bool)>,
}

impl Observer {
    pub fn new(seed: u64, offset: u64) -> Self {
        let mut rng = SplitMix64::new(seed ^ offset);
        let mut points = Vec::new();
        for _ in 0..2 {
            let u0 = rng.draw();
            let u1 = rng.draw();
            points.push((
                16.0 + (u0 as f64) * 2.0f64.powi(-64) * 96.0,
                16.0 + (u1 as f64) * 2.0f64.powi(-64) * 96.0,
            ));
        }
        Observer {
            rng,
            points,
            policy_events: Vec::new(),
        }
    }

    fn focus(&mut self, t: u64) -> (f64, f64) {
        let k = ((t - 1) / 64) as usize;
        while self.points.len() < k + 2 {
            let u0 = self.rng.draw();
            let u1 = self.rng.draw();
            self.points.push((
                16.0 + (u0 as f64) * 2.0f64.powi(-64) * 96.0,
                16.0 + (u1 as f64) * 2.0f64.powi(-64) * 96.0,
            ));
        }
        let p0 = self.points[k];
        let p1 = self.points[k + 1];
        let f = ((t - (1 + 64 * k as u64)) as f64) / 64.0;
        (p0.0 + (p1.0 - p0.0) * f, p0.1 + (p1.1 - p0.1) * f)
    }
}

fn box_distance(fx: f64, fy: f64, region: u8) -> f64 {
    let x0 = (region % 2) as f64 * 64.0;
    let y0 = (region / 2) as f64 * 64.0;
    let cx = fx.max(x0).min(x0 + 64.0);
    let cy = fy.max(y0).min(y0 + 64.0);
    let dx = fx - cx;
    let dy = fy - cy;
    (dx * dx + dy * dy).sqrt()
}

pub struct GravityWorld {
    pub seed: u64,
    pub bodies: Vec<Body>,
    pub coarse: Vec<Option<Fit>>,
    pub collapsed: Vec<Option<CollapsedBody>>,
    pub body_region: Vec<u8>,
    pub region_mode: [RegionMode; 4],
    pub region_totals: [Option<CollapseTotals>; 4],
    pub region_multipole: [bool; 4],
    pub region_radial: [bool; 4],
    pub collapse_records: Vec<CollapseTotals>,
    pub events: BTreeMap<u64, Vec<(u8, Action)>>,
    pub tick: u64,
    pub px: f64,
    pub py: f64,
    pub observer: Option<Observer>,
    pub multipole: bool,
    pub radial: bool,
    pub last_expansion: Vec<Body>,
    pub contacts: bool,
    pub contact_params: bool,
    pub restitution: f64,
    pub friction: f64,
    pub walls: bool,
    pub touching: BTreeSet<(u32, u32)>,
    pub last_contacts: Vec<ContactEvent>,
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

// Test-only initial-condition profiles for corpus generation (see
// docs/DESIGN.md "corpus coverage"). Not part of the stream spec: a
// stream produced from these ICs verifies only when the verifier
// reconstructs the same profile (Rust corpus_world / simval
// --test-ic / ontos_stream_dump --test-ic). Each body consumes the same
// five SplitMix64 draws as initial_conditions, so masses match the spec
// ICs of the same seed.
//
// wallshot: body i targets wall i % 4 (x=0, x=128, y=0, y=128) — it
// starts within ~2 units of the wall, inbound at 2..5, so wall-hit
// Contact records fire within a few hundred ticks.
// coarsehit (count 8): bodies 0..3 are interceptors outside the
// region-3 box aimed straight at the cluster at 56..72; bodies 4..7
// are a slow target cluster around (86, 89) spread over y lanes
// 77 + 8*lane. Interceptors carry the smaller ids on purpose: the
// section 21/24 sweep is lexicographic with the fine body as the
// outer index, so a fine x ephemeris-coarse static pair only ever
// fires as (fine i, coarse j) with i < j. Same-lane |dy| <= 2 < min
// contact radius guarantees each interceptor a static contact once
// region 3 is demoted early; the 9-unit lane spacing keeps
// interceptors from contacting each other.
#[doc(hidden)]
pub fn corpus_initial_conditions(profile: &str, seed: u64, count: u32) -> Vec<Body> {
    let mut rng = SplitMix64::new(seed);
    let mut bodies = Vec::with_capacity(count as usize);
    for id in 0..count {
        let u0 = rng.draw();
        let u1 = rng.draw();
        let u2 = rng.draw();
        let u3 = rng.draw();
        let u4 = rng.draw();
        let mass = 0.5 + (u0 as f64) * 2.0f64.powi(-64) * 2.0;
        let along = 16.0 + (u1 as f64) * 2.0f64.powi(-64) * 96.0;
        let off = (u2 as f64) * 2.0f64.powi(-64) * 2.0;
        let speed = 2.0 + (u3 as f64) * 2.0f64.powi(-64) * 3.0;
        let drift = ((u4 as f64) * 2.0f64.powi(-64) - 0.5) * 0.5;
        let (x, y, vx, vy) = match profile {
            "wallshot" => match id % 4 {
                0 => (2.0 + off, along, 0.0 - speed, drift),
                1 => (124.0 + off, along, speed, drift),
                2 => (along, 2.0 + off, drift, 0.0 - speed),
                _ => (along, 124.0 + off, drift, speed),
            },
            "coarsehit" => {
                let lane = 77.0 + 8.0 * ((id % 4) as f64) + (u2 as f64) * 2.0f64.powi(-64) * 2.0;
                if id < 4 {
                    (
                        56.0 + (u1 as f64) * 2.0f64.powi(-64) * 4.0,
                        lane,
                        56.0 + (u3 as f64) * 2.0f64.powi(-64) * 16.0,
                        ((u4 as f64) * 2.0f64.powi(-64) - 0.5) * 0.5,
                    )
                } else {
                    (
                        84.0 + (u1 as f64) * 2.0f64.powi(-64) * 4.0,
                        lane,
                        ((u3 as f64) * 2.0f64.powi(-64) - 0.5) * 0.5,
                        ((u4 as f64) * 2.0f64.powi(-64) - 0.5) * 0.5,
                    )
                }
            }
            other => panic!("unknown corpus profile {other}"),
        };
        bodies.push(Body {
            id,
            mass,
            x,
            y,
            vx,
            vy,
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
        Self::from_bodies(seed, initial_conditions(seed, count))
    }

    // Test-only corpus constructor (see corpus_initial_conditions).
    #[doc(hidden)]
    pub fn corpus_world(profile: &str, seed: u64, count: u32) -> Self {
        Self::from_bodies(seed, corpus_initial_conditions(profile, seed, count))
    }

    fn from_bodies(seed: u64, bodies: Vec<Body>) -> Self {
        let count = bodies.len() as u32;
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
            collapsed: vec![None; count as usize],
            body_region: vec![UNMANAGED; count as usize],
            region_mode: [RegionMode::Fine; 4],
            region_totals: [None; 4],
            region_multipole: [false; 4],
            region_radial: [false; 4],
            collapse_records: Vec::new(),
            events: BTreeMap::new(),
            tick: 0,
            px,
            py,
            observer: None,
            multipole: true,
            radial: false,
            last_expansion: Vec::new(),
            contacts: false,
            contact_params: false,
            restitution: 0.0,
            friction: 0.0,
            walls: false,
            touching: BTreeSet::new(),
            last_contacts: Vec::new(),
        }
    }

    pub fn set_observer(&mut self, offset: u64) {
        self.observer = Some(Observer::new(self.seed, offset));
    }

    pub fn schedule(&mut self, tick: u64, region: u8, action: Action) {
        self.events.entry(tick).or_default().push((region, action));
    }

    pub fn collapsed_totals(&self, region: u8) -> Option<&CollapseTotals> {
        self.region_totals[region as usize].as_ref()
    }

    fn eval_fit(fit: &Fit, t: u64) -> (f64, f64, f64, f64) {
        let s = -1.0 + ((t - fit.t0) as f64) / 16.0;
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
                self.collapsed[i].is_none() && {
                    let b = self.body_state_at(i, t0);
                    b.x >= x0 && b.x < x1 && b.y >= y0 && b.y < y1
                }
            })
            .collect();
        if members.is_empty() {
            self.region_mode[region as usize] = RegionMode::Coarse;
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
        self.region_mode[region as usize] = RegionMode::Coarse;
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
        self.region_mode[region as usize] = RegionMode::Fine;
    }

    fn collapse_region(&mut self, region: u8, t0: u64) {
        let (x0, y0, x1, y1) = Self::box_of(region);
        let members: Vec<usize> = (0..self.bodies.len())
            .filter(|&i| {
                self.collapsed[i].is_none() && {
                    let b = self.body_state_at(i, t0);
                    b.x >= x0 && b.x < x1 && b.y >= y0 && b.y < y1
                }
            })
            .collect();
        if members.is_empty() {
            let totals = CollapseTotals {
                tick: t0,
                region,
                count: 0,
                mass: 0.0,
                com_x: 0.0,
                com_y: 0.0,
                px: 0.0,
                py: 0.0,
                energy: 0.0,
                vcom_x: 0.0,
                vcom_y: 0.0,
                mx: 0.0,
                my: 0.0,
                qxx: 0.0,
                qxy: 0.0,
                qyy: 0.0,
                binding: 0.0,
                radial: self.radial,
            };
            self.region_totals[region as usize] = Some(totals);
            self.region_multipole[region as usize] = self.multipole;
            self.region_radial[region as usize] = self.radial;
            self.collapse_records.push(totals);
            self.region_mode[region as usize] = RegionMode::Collapsed;
            return;
        }
        let states: Vec<Body> = members.iter().map(|&i| self.body_state_at(i, t0)).collect();
        let mut mass = 0.0f64;
        let mut mx = 0.0f64;
        let mut my = 0.0f64;
        let mut px = 0.0f64;
        let mut py = 0.0f64;
        let mut ke = 0.0f64;
        for b in &states {
            mass += b.mass;
            mx += b.mass * b.x;
            my += b.mass * b.y;
            px += b.mass * b.vx;
            py += b.mass * b.vy;
            ke += 0.5 * b.mass * (b.vx * b.vx + b.vy * b.vy);
        }
        let mut pe = 0.0f64;
        let mut binding = 0.0f64;
        for i in 0..states.len() {
            for j in (i + 1)..states.len() {
                let dx = states[j].x - states[i].x;
                let dy = states[j].y - states[i].y;
                let s2 = dx * dx + dy * dy + EPS2;
                pe -= states[i].mass * states[j].mass / s2.sqrt();
                binding += states[i].mass * states[j].mass / s2.sqrt();
            }
        }
        let com_x = mx / mass;
        let com_y = my / mass;
        let vcom_x = px / mass;
        let vcom_y = py / mass;
        let mut qxx = 0.0f64;
        let mut qxy = 0.0f64;
        let mut qyy = 0.0f64;
        for b in &states {
            let dx = b.x - com_x;
            let dy = b.y - com_y;
            qxx += b.mass * dx * dx;
            qxy += b.mass * dx * dy;
            qyy += b.mass * dy * dy;
        }
        let mut rng = SplitMix64::new(self.seed ^ (region as u64).wrapping_mul(0x9E3779B97F4A7C15));
        let mut jitter = Vec::with_capacity(members.len());
        for _ in 0..members.len() {
            let ux = rng.draw();
            let uy = rng.draw();
            jitter.push((
                ((ux as f64) * 2.0f64.powi(-64) - 0.5) * 8.0,
                ((uy as f64) * 2.0f64.powi(-64) - 0.5) * 8.0,
            ));
        }
        let mut spread = Vec::with_capacity(members.len());
        for _ in 0..members.len() {
            let ux = rng.draw();
            let uy = rng.draw();
            spread.push((
                ((ux as f64) * 2.0f64.powi(-64) - 0.5) * 0.1,
                ((uy as f64) * 2.0f64.powi(-64) - 0.5) * 0.1,
            ));
        }
        for (slot, &i) in members.iter().enumerate() {
            let (jx, jy) = jitter[slot];
            let (sx, sy) = spread[slot];
            self.collapsed[i] = Some(CollapsedBody { jx, jy, sx, sy });
            self.coarse[i] = None;
            self.body_region[i] = region;
            self.bodies[i].x = com_x + jx;
            self.bodies[i].y = com_y + jy;
            self.bodies[i].vx = vcom_x;
            self.bodies[i].vy = vcom_y;
        }
        let totals = CollapseTotals {
            tick: t0,
            region,
            count: members.len() as u64,
            mass,
            com_x,
            com_y,
            px,
            py,
            energy: ke + pe,
            vcom_x,
            vcom_y,
            mx,
            my,
            qxx,
            qxy,
            qyy,
            binding,
            radial: self.radial,
        };
        self.region_totals[region as usize] = Some(totals);
        self.region_multipole[region as usize] = self.multipole;
        self.region_radial[region as usize] = self.radial;
        self.collapse_records.push(totals);
        self.region_mode[region as usize] = RegionMode::Collapsed;
    }

    fn expand_region(&mut self, region: u8) {
        let members: Vec<usize> = (0..self.bodies.len())
            .filter(|&i| self.collapsed[i].is_some() && self.body_region[i] == region)
            .collect();
        let totals = self.region_totals[region as usize].take();
        let multipole = self.region_multipole[region as usize];
        let radial = self.region_radial[region as usize];
        self.region_multipole[region as usize] = false;
        self.region_radial[region as usize] = false;
        if !members.is_empty() {
            let t = totals.expect("collapsed region carries totals");
            let mut sigma = 1.0f64;
            if multipole {
                let mut base = self.expand_base(&members, &t);
                if radial {
                    let fl = self.radial_scale(&members, &t, &mut base);
                    sigma = self.solve_spread_sigma(&members, &t, t.energy + fl);
                }
                self.apply_dipole_residual(&members, &t, &base);
            }
            let mut sx = 0.0f64;
            let mut sy = 0.0f64;
            for &i in &members[..members.len() - 1] {
                let c = self.collapsed[i].take().expect("collapsed body");
                self.bodies[i].vx = t.vcom_x + sigma * c.sx;
                self.bodies[i].vy = t.vcom_y + sigma * c.sy;
                sx += self.bodies[i].mass * self.bodies[i].vx;
                sy += self.bodies[i].mass * self.bodies[i].vy;
            }
            let last = members[members.len() - 1];
            self.collapsed[last] = None;
            self.bodies[last].vx = (t.px - sx) / self.bodies[last].mass;
            self.bodies[last].vy = (t.py - sy) / self.bodies[last].mass;
            self.last_expansion = members.iter().map(|&i| self.bodies[i]).collect();
            for &i in &members {
                self.body_region[i] = UNMANAGED;
            }
        }
        self.region_mode[region as usize] = RegionMode::Fine;
    }

    fn expand_base(&self, members: &[usize], t: &CollapseTotals) -> Vec<(f64, f64)> {
        let mut base: Vec<(f64, f64)> = members
            .iter()
            .map(|&i| {
                let c = self.collapsed[i].expect("collapsed body");
                (c.jx, c.jy)
            })
            .collect();
        if members.len() >= 3 {
            let mut swx = 0.0f64;
            let mut swy = 0.0f64;
            for (slot, &i) in members.iter().enumerate() {
                let m = self.bodies[i].mass;
                swx += m * base[slot].0;
                swy += m * base[slot].1;
            }
            let wx = swx / t.mass;
            let wy = swy / t.mass;
            let mut dhat = Vec::with_capacity(members.len());
            let mut jxx = 0.0f64;
            let mut jxy = 0.0f64;
            let mut jyy = 0.0f64;
            for (slot, &i) in members.iter().enumerate() {
                let m = self.bodies[i].mass;
                let dx = base[slot].0 - wx;
                let dy = base[slot].1 - wy;
                jxx += m * dx * dx;
                jxy += m * dx * dy;
                jyy += m * dy * dy;
                dhat.push((dx, dy));
            }
            if jxx > 0.0 && t.qxx > 0.0 {
                let lj00 = jxx.sqrt();
                let lj10 = jxy / lj00;
                let jjd = jyy - lj10 * lj10;
                if jjd > 0.0 {
                    let lq00 = t.qxx.sqrt();
                    let lq10 = t.qxy / lq00;
                    let qqd = t.qyy - lq10 * lq10;
                    if qqd > 0.0 {
                        let lj11 = jjd.sqrt();
                        let lq11 = qqd.sqrt();
                        let u00 = 1.0 / lj00;
                        let u11 = 1.0 / lj11;
                        let u10 = -(lj10 / (lj00 * lj11));
                        let a00 = lq00 * u00;
                        let a11 = lq11 * u11;
                        let a10 = lq10 * u00 + lq11 * u10;
                        for slot in 0..members.len() {
                            let (dx, dy) = dhat[slot];
                            base[slot] = (a00 * dx, a10 * dx + a11 * dy);
                        }
                    }
                }
            }
        }
        base
    }

    fn apply_dipole_residual(
        &mut self,
        members: &[usize],
        t: &CollapseTotals,
        base: &[(f64, f64)],
    ) {
        let mut sum_mx = 0.0f64;
        let mut sum_my = 0.0f64;
        for (slot, &i) in members[..members.len() - 1].iter().enumerate() {
            self.bodies[i].x = t.com_x + base[slot].0;
            self.bodies[i].y = t.com_y + base[slot].1;
            sum_mx += self.bodies[i].mass * self.bodies[i].x;
            sum_my += self.bodies[i].mass * self.bodies[i].y;
        }
        let last = members[members.len() - 1];
        self.bodies[last].x = (t.mx - sum_mx) / self.bodies[last].mass;
        self.bodies[last].y = (t.my - sum_my) / self.bodies[last].mass;
    }

    fn radial_scale(&self, members: &[usize], t: &CollapseTotals, base: &mut [(f64, f64)]) -> f64 {
        if members.len() < 2 {
            return 0.0;
        }
        let mut pairs: Vec<(f64, f64)> =
            Vec::with_capacity(members.len() * (members.len() - 1) / 2);
        for a in 0..members.len() {
            for b in (a + 1)..members.len() {
                let w = self.bodies[members[a]].mass * self.bodies[members[b]].mass;
                let dx = base[b].0 - base[a].0;
                let dy = base[b].1 - base[a].1;
                pairs.push((w, dx * dx + dy * dy));
            }
        }
        let f = |lam: f64| -> f64 {
            let mut total = 0.0f64;
            for &(w, d2) in &pairs {
                total += w / (lam * lam * d2 + 1.0).sqrt();
            }
            total
        };
        let target = t.binding;
        let lam = if target >= f(0.0) {
            0.0
        } else {
            let mut hi = 1.0f64;
            let mut doublings = 0;
            while f(hi) > target && doublings < 64 {
                hi *= 2.0;
                doublings += 1;
            }
            let mut lo = 0.0f64;
            for _ in 0..128 {
                let mid = (lo + hi) * 0.5;
                if f(mid) >= target {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (lo + hi) * 0.5
        };
        for b in base.iter_mut() {
            b.0 *= lam;
            b.1 *= lam;
        }
        let mut swx = 0.0f64;
        let mut swy = 0.0f64;
        for (slot, &i) in members.iter().enumerate() {
            let m = self.bodies[i].mass;
            swx += m * base[slot].0;
            swy += m * base[slot].1;
        }
        let wx = swx / t.mass;
        let wy = swy / t.mass;
        for b in base.iter_mut() {
            b.0 -= wx;
            b.1 -= wy;
        }
        f(lam)
    }

    fn synth_velocities(
        &self,
        members: &[usize],
        t: &CollapseTotals,
        sigma: f64,
    ) -> Vec<(f64, f64)> {
        let mut out = Vec::with_capacity(members.len());
        let mut sx = 0.0f64;
        let mut sy = 0.0f64;
        for (slot, &i) in members.iter().enumerate() {
            if slot + 1 < members.len() {
                let c = self.collapsed[i].expect("collapsed body");
                let vx = t.vcom_x + sigma * c.sx;
                let vy = t.vcom_y + sigma * c.sy;
                sx += self.bodies[i].mass * vx;
                sy += self.bodies[i].mass * vy;
                out.push((vx, vy));
            } else {
                out.push((
                    (t.px - sx) / self.bodies[i].mass,
                    (t.py - sy) / self.bodies[i].mass,
                ));
            }
        }
        out
    }

    fn synth_ke(&self, members: &[usize], t: &CollapseTotals, sigma: f64) -> f64 {
        let vs = self.synth_velocities(members, t, sigma);
        let mut ke = 0.0f64;
        for (slot, &(vx, vy)) in vs.iter().enumerate() {
            let m = self.bodies[members[slot]].mass;
            ke += 0.5 * m * (vx * vx + vy * vy);
        }
        ke
    }

    fn solve_spread_sigma(&self, members: &[usize], t: &CollapseTotals, k_target: f64) -> f64 {
        let c0 = self.synth_ke(members, t, 0.0);
        let c1 = self.synth_ke(members, t, 1.0);
        let cm = self.synth_ke(members, t, -1.0);
        let a = ((c1 + cm) - (c0 + c0)) * 0.5;
        let b = (c1 - cm) * 0.5;
        let disc = b * b - 4.0 * a * (c0 - k_target);
        if a == 0.0 {
            0.0
        } else if disc < 0.0 {
            (0.0 - b) / (2.0 * a)
        } else {
            let sq = disc.sqrt();
            let r1 = ((0.0 - b) + sq) / (2.0 * a);
            let r2 = ((0.0 - b) - sq) / (2.0 * a);
            if r1 * r1 <= r2 * r2 {
                r1
            } else {
                r2
            }
        }
    }

    fn apply_event(&mut self, region: u8, action: Action) {
        let t = self.tick + 1;
        match action {
            Action::Demote => {
                if self.region_mode[region as usize] != RegionMode::Collapsed {
                    self.demote_region(region, t);
                }
            }
            Action::Promote => {
                if self.region_mode[region as usize] == RegionMode::Collapsed {
                    self.expand_region(region);
                } else {
                    self.promote_region(region, t);
                }
            }
            Action::Collapse => {
                if self.region_mode[region as usize] != RegionMode::Collapsed {
                    self.collapse_region(region, t);
                }
            }
        }
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
            self.region_mode[region as usize] = RegionMode::Fine;
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
            for &(region, action) in &events {
                self.apply_event(region, action);
            }
        }
        if entering >= 17 && entering % 16 == 1 && self.observer.is_some() {
            let (fx, fy) = self.observer.as_mut().expect("observer").focus(entering);
            let mut fired: Vec<(u8, bool)> = Vec::new();
            for region in 0..4u8 {
                let d = box_distance(fx, fy, region);
                let mode = self.region_mode[region as usize];
                if mode == RegionMode::Fine && d > 48.0 {
                    fired.push((region, true));
                } else if mode != RegionMode::Fine && d < 24.0 {
                    fired.push((region, false));
                }
            }
            for &(region, to_coarse) in &fired {
                self.apply_event(
                    region,
                    if to_coarse {
                        Action::Demote
                    } else {
                        Action::Promote
                    },
                );
            }
            self.observer
                .as_mut()
                .expect("observer")
                .policy_events
                .extend(fired);
        }
        for region in 0..4u8 {
            if self.region_mode[region as usize] == RegionMode::Coarse {
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
        let kind: Vec<u8> = (0..n)
            .map(|i| {
                if self.coarse[i].is_some() {
                    1
                } else if self.collapsed[i].is_some() {
                    2
                } else {
                    0
                }
            })
            .collect();
        let mut monopoles: Vec<(f64, f64, f64)> = Vec::new();
        for region in 0..4u8 {
            if let Some(tot) = &self.region_totals[region as usize] {
                if tot.count > 0 {
                    monopoles.push((tot.mass, tot.com_x, tot.com_y));
                }
            }
        }
        let half = DT * 0.5;
        let view: Vec<Body> = (0..n).map(|i| self.body_state_at(i, entering)).collect();
        let (ax_ff, ay_ff, ax_fc, ay_fc) = accel_split(&view, &kind, &monopoles);
        for i in 0..n {
            if kind[i] == 0 {
                self.bodies[i].vx += ax_ff[i] * half;
                self.bodies[i].vy += ay_ff[i] * half;
            }
        }
        for i in 0..n {
            if kind[i] == 0 {
                self.bodies[i].vx += ax_fc[i] * half;
                self.bodies[i].vy += ay_fc[i] * half;
                self.px += self.bodies[i].mass * (ax_fc[i] * half);
                self.py += self.bodies[i].mass * (ay_fc[i] * half);
            }
        }
        for (i, b) in self.bodies.iter_mut().enumerate() {
            if kind[i] == 0 {
                b.x += b.vx * DT;
                b.y += b.vy * DT;
            }
        }
        let view: Vec<Body> = (0..n).map(|i| self.body_state_at(i, entering)).collect();
        let (ax_ff, ay_ff, ax_fc, ay_fc) = accel_split(&view, &kind, &monopoles);
        for (i, b) in self.bodies.iter_mut().enumerate() {
            if kind[i] == 0 {
                b.vx += ax_ff[i] * half;
                b.vy += ay_ff[i] * half;
            }
        }
        for i in 0..n {
            if kind[i] == 0 {
                self.bodies[i].vx += ax_fc[i] * half;
                self.bodies[i].vy += ay_fc[i] * half;
                self.px += self.bodies[i].mass * (ax_fc[i] * half);
                self.py += self.bodies[i].mass * (ay_fc[i] * half);
            }
        }
        if self.contacts {
            self.contact_pass(entering, &kind);
        }
        self.tick = entering;
    }

    fn contact_pass(&mut self, entering: u64, kind: &[u8]) {
        let n = self.bodies.len();
        let radii: Vec<f64> = self
            .bodies
            .iter()
            .map(|b| CONTACT_R * b.mass.sqrt())
            .collect();
        let e = self.restitution;
        let fr = self.friction;
        let extended = self.contact_params;
        let mut next = BTreeSet::new();
        let mut events = Vec::new();
        for i in 0..n {
            if kind[i] != 0 {
                continue;
            }
            for j in (i + 1)..n {
                if kind[j] == 2 {
                    continue;
                }
                if kind[j] != 0 && !extended {
                    continue;
                }
                let sj = self.body_state_at(j, entering);
                let dx = sj.x - self.bodies[i].x;
                let dy = sj.y - self.bodies[i].y;
                let rs = radii[i] + radii[j];
                let d2 = dx * dx + dy * dy;
                if d2 >= rs * rs {
                    continue;
                }
                let pair = (i as u32, j as u32);
                next.insert(pair);
                let (nx, ny) = if d2 == 0.0 {
                    (1.0, 0.0)
                } else {
                    let dist = d2.sqrt();
                    (dx / dist, dy / dist)
                };
                let vrx = sj.vx - self.bodies[i].vx;
                let vry = sj.vy - self.bodies[i].vy;
                let vn = vrx * nx + vry * ny;
                if vn >= 0.0 {
                    continue;
                }
                let mi = self.bodies[i].mass;
                let mj = self.bodies[j].mass;
                let cx = (self.bodies[i].x + sj.x) * 0.5;
                let cy = (self.bodies[i].y + sj.y) * 0.5;
                let (jn, mu) = if kind[j] == 0 {
                    let inv = 1.0 / (mi + mj);
                    let t = vn * inv;
                    let s = (1.0 + e) * t;
                    let fi = s * mj;
                    let fj = s * mi;
                    self.bodies[i].vx += fi * nx;
                    self.bodies[i].vy += fi * ny;
                    self.bodies[j].vx -= fj * nx;
                    self.bodies[j].vy -= fj * ny;
                    let mu = (mi * mj) / (mi + mj);
                    let jn = ((0.0 - vn) * (1.0 + e)) * mu;
                    if fr > 0.0 {
                        let vt = (0.0 - vrx) * ny + vry * nx;
                        let mut q = vt * inv;
                        let qmax = (fr * jn) * inv;
                        if q > qmax {
                            q = qmax;
                        }
                        if q < 0.0 - qmax {
                            q = 0.0 - qmax;
                        }
                        let fti = q * mj;
                        let ftj = q * mi;
                        self.bodies[i].vx += fti * (0.0 - ny);
                        self.bodies[i].vy += fti * nx;
                        self.bodies[j].vx -= ftj * (0.0 - ny);
                        self.bodies[j].vy -= ftj * nx;
                    }
                    (jn, mu)
                } else {
                    let (_, jn) = self.static_impulse(i, nx, ny, vrx, vry);
                    (jn, (mi * mj) / (mi + mj))
                };
                if self.touching.contains(&pair) {
                    continue;
                }
                let (jvx, jvy) = if kind[j] == 0 {
                    (self.bodies[j].vx, self.bodies[j].vy)
                } else {
                    (sj.vx, sj.vy)
                };
                let vn_after = (jvx - self.bodies[i].vx) * nx + (jvy - self.bodies[i].vy) * ny;
                events.push(ContactEvent {
                    tick: entering,
                    a: i as u32,
                    b: j as u32,
                    jn,
                    cx,
                    cy,
                    vn,
                    vn_after,
                    mu,
                });
            }
        }
        if extended {
            for region in 0..4u8 {
                let tot = match self.region_mode[region as usize] {
                    RegionMode::Collapsed => match &self.region_totals[region as usize] {
                        Some(t) if t.count > 0 => *t,
                        _ => continue,
                    },
                    _ => continue,
                };
                let big_r = CONTACT_R * tot.mass.sqrt();
                for i in 0..n {
                    if kind[i] != 0 {
                        continue;
                    }
                    let dx = tot.com_x - self.bodies[i].x;
                    let dy = tot.com_y - self.bodies[i].y;
                    let rs = radii[i] + big_r;
                    let d2 = dx * dx + dy * dy;
                    if d2 >= rs * rs {
                        continue;
                    }
                    let pair = (i as u32, MONOPOLE_BASE + region as u32);
                    next.insert(pair);
                    let (nx, ny) = if d2 == 0.0 {
                        (1.0, 0.0)
                    } else {
                        let dist = d2.sqrt();
                        (dx / dist, dy / dist)
                    };
                    let vrx = tot.vcom_x - self.bodies[i].vx;
                    let vry = tot.vcom_y - self.bodies[i].vy;
                    let vn = vrx * nx + vry * ny;
                    if vn >= 0.0 {
                        continue;
                    }
                    let mi = self.bodies[i].mass;
                    let cx = (self.bodies[i].x + tot.com_x) * 0.5;
                    let cy = (self.bodies[i].y + tot.com_y) * 0.5;
                    let (_, jn) = self.static_impulse(i, nx, ny, vrx, vry);
                    let mu = (mi * tot.mass) / (mi + tot.mass);
                    if self.touching.contains(&pair) {
                        continue;
                    }
                    let vn_after = (tot.vcom_x - self.bodies[i].vx) * nx
                        + (tot.vcom_y - self.bodies[i].vy) * ny;
                    events.push(ContactEvent {
                        tick: entering,
                        a: i as u32,
                        b: MONOPOLE_BASE + region as u32,
                        jn,
                        cx,
                        cy,
                        vn,
                        vn_after,
                        mu,
                    });
                }
            }
        }
        if self.walls {
            for i in 0..n {
                if kind[i] != 0 {
                    continue;
                }
                for wall in 0..4u32 {
                    let (nx, ny, cx, cy) = match wall {
                        0 => {
                            if !(self.bodies[i].x - radii[i] < 0.0 && self.bodies[i].vx < 0.0) {
                                continue;
                            }
                            (
                                0.0 - 1.0,
                                0.0,
                                (self.bodies[i].x + 0.0) * 0.5,
                                self.bodies[i].y,
                            )
                        }
                        1 => {
                            if !(self.bodies[i].x + radii[i] > 128.0 && self.bodies[i].vx > 0.0) {
                                continue;
                            }
                            (1.0, 0.0, (self.bodies[i].x + 128.0) * 0.5, self.bodies[i].y)
                        }
                        2 => {
                            if !(self.bodies[i].y - radii[i] < 0.0 && self.bodies[i].vy < 0.0) {
                                continue;
                            }
                            (
                                0.0,
                                0.0 - 1.0,
                                self.bodies[i].x,
                                (self.bodies[i].y + 0.0) * 0.5,
                            )
                        }
                        _ => {
                            if !(self.bodies[i].y + radii[i] > 128.0 && self.bodies[i].vy > 0.0) {
                                continue;
                            }
                            (0.0, 1.0, self.bodies[i].x, (self.bodies[i].y + 128.0) * 0.5)
                        }
                    };
                    let pair = (i as u32, WALL_BASE + wall);
                    next.insert(pair);
                    let vrx = 0.0 - self.bodies[i].vx;
                    let vry = 0.0 - self.bodies[i].vy;
                    let vn = vrx * nx + vry * ny;
                    if vn >= 0.0 {
                        continue;
                    }
                    let (vn, jn) = self.static_impulse(i, nx, ny, vrx, vry);
                    let mi = self.bodies[i].mass;
                    if self.touching.contains(&pair) {
                        continue;
                    }
                    let vn_after = (0.0 - self.bodies[i].vx) * nx + (0.0 - self.bodies[i].vy) * ny;
                    events.push(ContactEvent {
                        tick: entering,
                        a: i as u32,
                        b: WALL_BASE + wall,
                        jn,
                        cx,
                        cy,
                        vn,
                        vn_after,
                        mu: mi,
                    });
                }
            }
        }
        self.touching = next;
        self.last_contacts.extend(events);
    }

    fn static_impulse(&mut self, i: usize, nx: f64, ny: f64, vrx: f64, vry: f64) -> (f64, f64) {
        let e = self.restitution;
        let fr = self.friction;
        let mi = self.bodies[i].mass;
        let vn = vrx * nx + vry * ny;
        let s = (1.0 + e) * vn;
        self.bodies[i].vx += s * nx;
        self.bodies[i].vy += s * ny;
        self.px += mi * (s * nx);
        self.py += mi * (s * ny);
        let jn = (0.0 - s) * mi;
        if fr > 0.0 {
            let vt = (0.0 - vrx) * ny + vry * nx;
            let mut jt = vt * mi;
            let jt_max = fr * jn;
            if jt > jt_max {
                jt = jt_max;
            }
            if jt < 0.0 - jt_max {
                jt = 0.0 - jt_max;
            }
            let w = jt / mi;
            self.bodies[i].vx += w * (0.0 - ny);
            self.bodies[i].vy += w * nx;
            self.px += jt * (0.0 - ny);
            self.py += jt * nx;
        }
        (vn, jn)
    }

    pub fn totals(&self) -> (u64, u64, f64, f64, f64, f64) {
        let n = self.bodies.len();
        let view: Vec<Body> = (0..n).map(|i| self.body_state_at(i, self.tick)).collect();
        let mut fine = 0u64;
        let mut coarse = 0u64;
        let mut mass = 0.0f64;
        let mut ke = 0.0f64;
        for b in &view {
            if self.coarse[b.id as usize].is_some() || self.collapsed[b.id as usize].is_some() {
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

    fn body_level(&self, i: usize) -> u8 {
        if self.collapsed[i].is_some() {
            2
        } else if self.coarse[i].is_some() {
            0
        } else {
            1
        }
    }

    pub fn body_state_bytes(&self, i: usize) -> Vec<u8> {
        let b = self.body_state_at(i, self.tick);
        b.state_bytes(self.body_level(i))
    }

    pub fn emitted_state(&self, i: usize) -> (Body, u8, u8) {
        let b = self.body_state_at(i, self.tick);
        let level = self.body_level(i);
        let region = if self.coarse[i].is_some() || self.collapsed[i].is_some() {
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
            if self.coarse[i].is_some() || self.collapsed[i].is_some() {
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
        let level = match self.region_mode[region as usize] {
            RegionMode::Coarse => 0u8,
            RegionMode::Fine => 1u8,
            RegionMode::Collapsed => 2u8,
        };
        let mut v = Vec::new();
        v.push(level);
        for &i in &members {
            v.extend_from_slice(&self.body_state_bytes(i));
        }
        (level, members.len() as u64, fnv1a64(&v))
    }
}

fn accel_split(
    view: &[Body],
    kind: &[u8],
    monopoles: &[(f64, f64, f64)],
) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
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
            match (kind[i], kind[j]) {
                (0, 0) => {
                    ax_ff[i] += view[j].mass * fx;
                    ay_ff[i] += view[j].mass * fy;
                    ax_ff[j] -= view[i].mass * fx;
                    ay_ff[j] -= view[i].mass * fy;
                }
                (0, 1) => {
                    ax_fc[i] += view[j].mass * fx;
                    ay_fc[i] += view[j].mass * fy;
                }
                (1, 0) => {
                    ax_fc[j] -= view[i].mass * fx;
                    ay_fc[j] -= view[i].mass * fy;
                }
                _ => {}
            }
        }
    }
    for &(m, cx, cy) in monopoles {
        for i in 0..n {
            if kind[i] != 0 {
                continue;
            }
            let dx = cx - view[i].x;
            let dy = cy - view[i].y;
            let s2 = dx * dx + dy * dy + EPS2;
            let inv3 = 1.0 / (s2 * s2.sqrt());
            let fx = G * inv3 * dx;
            let fy = G * inv3 * dy;
            ax_fc[i] += m * fx;
            ay_fc[i] += m * fy;
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
            w.schedule(5, 1, Action::Demote);
            w.schedule(70, 1, Action::Promote);
            w.schedule(40, 2, Action::Demote);
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
    fn collapse_expand_replay_bit_identical() {
        let run = || {
            let mut w = GravityWorld::new(11, 10);
            w.schedule(30, 3, Action::Collapse);
            w.schedule(120, 3, Action::Promote);
            w.schedule(50, 0, Action::Collapse);
            w.schedule(60, 0, Action::Promote);
            w.schedule(80, 2, Action::Demote);
            let mut hashes = Vec::new();
            for _ in 0..200 {
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
    fn collapse_all_bodies_freezes_ledger_exactly() {
        let mut world = None;
        for seed in 0..100_000u64 {
            let w = GravityWorld::new(seed, 4);
            let all_in = w
                .bodies
                .iter()
                .all(|b| b.x >= 64.0 && b.x < 128.0 && b.y >= 64.0 && b.y < 128.0);
            if all_in {
                world = Some(w);
                break;
            }
        }
        let mut w = world.expect("seed with all bodies in region 3");
        let (_, _, _, px0, py0, _) = w.totals();
        w.schedule(1, 3, Action::Collapse);
        for _ in 0..50 {
            w.step();
            let (_, _, _, px, py, _) = w.totals();
            assert_eq!((px, py), (px0, py0));
        }
        w.schedule(52, 3, Action::Promote);
        for _ in 0..50 {
            w.step();
            let (_, _, _, px, py, _) = w.totals();
            assert_eq!((px, py), (px0, py0));
        }
        assert!(w.collapsed.iter().all(|c| c.is_none()));
    }

    #[test]
    fn expansion_residual_momentum_matches_totals() {
        let mut world = None;
        for seed in 0..100_000u64 {
            let w = GravityWorld::new(seed, 5);
            let all_in = w
                .bodies
                .iter()
                .all(|b| b.x >= 64.0 && b.x < 128.0 && b.y >= 64.0 && b.y < 128.0);
            if all_in {
                world = Some(w);
                break;
            }
        }
        let mut w = world.expect("seed with all bodies in region 3");
        w.schedule(1, 3, Action::Collapse);
        w.schedule(60, 3, Action::Promote);
        for _ in 0..60 {
            w.step();
        }
        assert_eq!(w.collapse_records.len(), 1);
        let rec = w.collapse_records[0];
        let mut sx = 0.0f64;
        let mut sy = 0.0f64;
        for b in &w.bodies {
            sx += b.mass * b.vx;
            sy += b.mass * b.vy;
        }
        let ulp_x = (sx - rec.px).abs() / (rec.px.abs() * f64::EPSILON);
        let ulp_y = (sy - rec.py).abs() / (rec.py.abs() * f64::EPSILON);
        assert!(ulp_x <= 8.0, "sum m vx vs P: {ulp_x} ULP");
        assert!(ulp_y <= 8.0, "sum m vy vs P: {ulp_y} ULP");
    }

    #[test]
    fn collapsed_states_static_and_level_two() {
        let mut w = GravityWorld::new(13, 12);
        w.schedule(40, 0, Action::Collapse);
        let mut first: Option<Vec<(Body, u8, u8)>> = None;
        while w.tick < 80 {
            w.step();
            if w.tick >= 41 {
                let states: Vec<(Body, u8, u8)> = (0..w.bodies.len())
                    .map(|i| w.emitted_state(i))
                    .filter(|&(_, _, level)| level == 2)
                    .collect();
                assert!(!states.is_empty());
                assert!(states.iter().all(|&(_, region, _)| region == 0));
                if let Some(f) = &first {
                    assert_eq!(&states, f, "collapsed body states are static");
                } else {
                    first = Some(states);
                }
            }
        }
        let (level, _, _) = w.region_hash(0);
        assert_eq!(level, 2);
        w.schedule(w.tick + 1, 0, Action::Promote);
        w.step();
        let (level, _, _) = w.region_hash(0);
        assert_eq!(level, 1);
        for i in 0..w.bodies.len() {
            let (_, _, level) = w.emitted_state(i);
            assert_eq!(level, 1);
        }
    }

    #[test]
    fn collapse_empty_region_emits_zero_totals() {
        let mut w = GravityWorld::new(11, 10);
        let mut target = None;
        while w.tick < 40 {
            w.step();
            for region in 0..4u8 {
                let x0 = (region % 2) as f64 * 64.0;
                let y0 = (region / 2) as f64 * 64.0;
                let occupied = w
                    .bodies
                    .iter()
                    .any(|b| b.x >= x0 && b.x < x0 + 64.0 && b.y >= y0 && b.y < y0 + 64.0);
                if !occupied {
                    target = Some((w.tick + 1, region));
                    break;
                }
            }
            if target.is_some() {
                break;
            }
        }
        let (t, region) = target.expect("found an empty region box");
        w.schedule(t, region, Action::Collapse);
        for _ in 0..10 {
            w.step();
        }
        assert_eq!(w.collapse_records.len(), 1);
        let rec = w.collapse_records[0];
        assert_eq!(rec.tick, t);
        assert_eq!(rec.region, region);
        assert_eq!(rec.count, 0);
        assert_eq!(rec.mass, 0.0);
        assert_eq!(rec.com_x, 0.0);
        assert_eq!(rec.com_y, 0.0);
        assert_eq!(rec.px, 0.0);
        assert_eq!(rec.py, 0.0);
        assert_eq!(rec.energy, 0.0);
        let (level, pop, _) = w.region_hash(region);
        assert_eq!((level, pop), (2, 0));
        let (_, _, mass, _, _, energy) = w.totals();
        assert!(mass.is_finite() && energy.is_finite());
        w.schedule(w.tick + 1, region, Action::Promote);
        w.step();
        let (level, _, _) = w.region_hash(region);
        assert_eq!(level, 1);
    }

    #[test]
    fn demote_on_collapsed_region_is_noop() {
        let mut w = GravityWorld::new(11, 10);
        w.schedule(30, 3, Action::Collapse);
        w.schedule(60, 3, Action::Demote);
        while w.tick < 61 {
            w.step();
        }
        assert_eq!(w.region_mode[3], RegionMode::Collapsed);
        for i in 0..w.bodies.len() {
            if w.body_region[i] == 3 {
                assert!(w.collapsed[i].is_some());
                assert!(w.coarse[i].is_none());
            }
        }
    }

    #[test]
    fn collapse_monopole_ledger_drift_bounded() {
        let mut w = GravityWorld::new(11, 10);
        let (_, _, _, px0, py0, e0) = w.totals();
        w.schedule(30, 3, Action::Collapse);
        w.schedule(120, 3, Action::Promote);
        let mut e_mid = 0.0f64;
        for _ in 0..200 {
            w.step();
            if w.tick == 60 {
                e_mid = w.totals().5;
            }
        }
        let (_, _, _, px1, py1, e1) = w.totals();
        // Measured under multipole reconstruction (section 20): px drift
        // 2.2e-3, py 3.9e-4, (e1-e_mid)/e_mid 0.27, (e1-e0)/e0 0.10.
        assert!((px1 - px0).abs() < 5e-3, "px drift {}", (px1 - px0).abs());
        assert!((py1 - py0).abs() < 5e-3, "py drift {}", (py1 - py0).abs());
        assert!((e1 - e_mid).abs() / e_mid.abs() < 0.5);
        assert!((e1 - e0).abs() / e0.abs() < 0.5);
    }

    #[test]
    fn multipole_replay_bit_identical() {
        let run = || {
            let mut w = GravityWorld::new(17, 12);
            w.schedule(20, 0, Action::Collapse);
            w.schedule(90, 0, Action::Promote);
            w.schedule(91, 0, Action::Collapse);
            w.schedule(160, 0, Action::Promote);
            w.schedule(40, 2, Action::Collapse);
            w.schedule(120, 2, Action::Promote);
            w.schedule(60, 3, Action::Demote);
            let mut hashes = Vec::new();
            for _ in 0..200 {
                w.step();
                hashes.push(w.world_hash());
            }
            hashes
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn contact_replay_bit_identical() {
        let run = || {
            let mut w = GravityWorld::new(11, 32);
            w.contacts = true;
            w.schedule(80, 2, Action::Demote);
            w.schedule(200, 2, Action::Promote);
            w.schedule(100, 0, Action::Collapse);
            w.schedule(250, 0, Action::Promote);
            let mut hashes = Vec::new();
            for _ in 0..400 {
                w.step();
                hashes.push(w.world_hash());
                w.last_contacts.clear();
            }
            hashes
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn contact_ledger_exactly_conserved_all_fine() {
        let mut w = GravityWorld::new(11, 32);
        w.contacts = true;
        let (_, _, _, px0, py0, _) = w.totals();
        for _ in 0..300 {
            w.step();
            w.last_contacts.clear();
        }
        let (_, _, _, px1, py1, _) = w.totals();
        assert_eq!((px0, py0), (px1, py1));
    }

    #[test]
    fn contact_impulse_stops_approach_and_records_positive() {
        let mut w = GravityWorld::new(11, 32);
        w.contacts = true;
        let mut records = 0u64;
        let mut worst_vn_after = 0.0f64;
        for _ in 0..400 {
            w.step();
            for c in &w.last_contacts {
                assert!(c.jn > 0.0, "jn must be positive: {}", c.jn);
                worst_vn_after = worst_vn_after.max(c.vn_after.abs());
                records += 1;
            }
            w.last_contacts.clear();
        }
        assert!(records >= 3, "expected several contacts, got {records}");
        assert!(
            worst_vn_after < 1e-12,
            "resolving pairs close on zero approach: {worst_vn_after}"
        );
    }

    #[test]
    fn contact_physical_momentum_drifts_only_by_rounding() {
        let mut w = GravityWorld::new(11, 32);
        w.contacts = true;
        let sum_mv = |w: &GravityWorld| {
            let mut px = 0.0;
            let mut py = 0.0;
            for b in &w.bodies {
                px += b.mass * b.vx;
                py += b.mass * b.vy;
            }
            (px, py)
        };
        let (px0, py0) = sum_mv(&w);
        let ledger0 = (w.px, w.py);
        for _ in 0..200 {
            w.step();
            w.last_contacts.clear();
        }
        let (px1, py1) = sum_mv(&w);
        assert_eq!((w.px, w.py), ledger0, "ledger untouched by contacts");
        let scale = px0.abs().max(py0.abs()).max(1e-30);
        let drift = (px1 - px0).abs().max((py1 - py0).abs()) / scale;
        assert!(drift < 1e-12, "physical momentum drift {drift}");
    }

    #[test]
    fn contacts_off_matches_spec_history_bit_for_bit() {
        let off = {
            let mut w = GravityWorld::new(1, 8);
            let mut hashes = Vec::new();
            for _ in 0..300 {
                w.step();
                hashes.push(w.world_hash());
            }
            hashes
        };
        let on_no_contact = {
            let mut w = GravityWorld::new(1, 8);
            w.contacts = true;
            let mut hashes = Vec::new();
            for _ in 0..300 {
                w.step();
                hashes.push(w.world_hash());
                w.last_contacts.clear();
            }
            hashes
        };
        assert_eq!(off, on_no_contact);
    }

    #[test]
    fn multipole_dipole_and_quadrupole_close_on_totals() {
        let mut world = None;
        for seed in 0..100_000u64 {
            let w = GravityWorld::new(seed, 6);
            let all_in = w
                .bodies
                .iter()
                .all(|b| b.x >= 64.0 && b.x < 128.0 && b.y >= 64.0 && b.y < 128.0);
            if all_in {
                world = Some(w);
                break;
            }
        }
        let mut w = world.expect("seed with all bodies in region 3");
        w.schedule(1, 3, Action::Collapse);
        w.schedule(60, 3, Action::Promote);
        for _ in 0..60 {
            w.step();
        }
        assert_eq!(w.collapse_records.len(), 1);
        assert_eq!(w.last_expansion.len(), 6);
        let rec = w.collapse_records[0];
        let mut sx = 0.0f64;
        let mut sy = 0.0f64;
        for b in &w.last_expansion {
            sx += b.mass * b.x;
            sy += b.mass * b.y;
        }
        let dipole_scale = rec.mx.abs().max(rec.my.abs()).max(1e-30);
        let dipole = (sx - rec.mx).abs().max((sy - rec.my).abs()) / dipole_scale;
        assert!(dipole < 1e-12, "dipole residual {dipole}");
        let mut qxx = 0.0f64;
        let mut qxy = 0.0f64;
        let mut qyy = 0.0f64;
        for b in &w.last_expansion {
            let dx = b.x - rec.com_x;
            let dy = b.y - rec.com_y;
            qxx += b.mass * dx * dx;
            qxy += b.mass * dx * dy;
            qyy += b.mass * dy * dy;
        }
        let q_scale = rec.qxx.abs().max(rec.qyy.abs());
        let quad = (qxx - rec.qxx)
            .abs()
            .max((qxy - rec.qxy).abs())
            .max((qyy - rec.qyy).abs())
            / q_scale;
        assert!(quad < 1e-9, "quadrupole deviation {quad}");
    }

    #[test]
    fn multipole_single_body_reconstructs_exact_position() {
        let mut world = None;
        for seed in 0..100_000u64 {
            let w = GravityWorld::new(seed, 1);
            let b = &w.bodies[0];
            if b.x >= 0.0 && b.x < 64.0 && b.y >= 0.0 && b.y < 64.0 {
                world = Some(w);
                break;
            }
        }
        let mut w = world.expect("seed with the body in region 0");
        w.schedule(1, 0, Action::Collapse);
        w.schedule(10, 0, Action::Promote);
        for _ in 0..10 {
            w.step();
        }
        let rec = w.collapse_records[0];
        assert_eq!(rec.count, 1);
        let b = w.last_expansion[0];
        let expect_x = (rec.mx - 0.0) / b.mass;
        let expect_y = (rec.my - 0.0) / b.mass;
        let ulp_x = (b.x - expect_x).abs() / (expect_x.abs() * f64::EPSILON);
        let ulp_y = (b.y - expect_y).abs() / (expect_y.abs() * f64::EPSILON);
        assert!(ulp_x <= 4.0, "x ULP {ulp_x}");
        assert!(ulp_y <= 4.0, "y ULP {ulp_y}");
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

    #[test]
    fn radial_replay_bit_identical() {
        let run = || {
            let mut w = GravityWorld::new(17, 12);
            w.radial = true;
            w.schedule(20, 0, Action::Collapse);
            w.schedule(90, 0, Action::Promote);
            w.schedule(91, 0, Action::Collapse);
            w.schedule(160, 0, Action::Promote);
            w.schedule(40, 2, Action::Collapse);
            w.schedule(120, 2, Action::Promote);
            w.schedule(60, 3, Action::Demote);
            let mut hashes = Vec::new();
            for _ in 0..200 {
                w.step();
                hashes.push(w.world_hash());
            }
            hashes
        };
        assert_eq!(run(), run());
    }

    fn all_in_region_world(count: u32) -> GravityWorld {
        let mut world = None;
        for seed in 0..100_000u64 {
            let w = GravityWorld::new(seed, count);
            let all_in = w
                .bodies
                .iter()
                .all(|b| b.x >= 64.0 && b.x < 128.0 && b.y >= 64.0 && b.y < 128.0);
            if all_in {
                world = Some(w);
                break;
            }
        }
        world.expect("seed with all bodies in region 3")
    }

    #[test]
    fn radial_binding_and_energy_close_on_totals() {
        let mut w = all_in_region_world(6);
        w.radial = true;
        w.schedule(1, 3, Action::Collapse);
        w.schedule(60, 3, Action::Promote);
        for _ in 0..60 {
            w.step();
        }
        assert_eq!(w.collapse_records.len(), 1);
        assert_eq!(w.last_expansion.len(), 6);
        let rec = w.collapse_records[0];
        let exp = &w.last_expansion;
        let mut sx = 0.0f64;
        let mut sy = 0.0f64;
        for b in exp {
            sx += b.mass * b.x;
            sy += b.mass * b.y;
        }
        let dipole_scale = rec.mx.abs().max(rec.my.abs()).max(1e-30);
        let dipole = (sx - rec.mx).abs().max((sy - rec.my).abs()) / dipole_scale;
        assert!(dipole < 1e-12, "dipole residual {dipole}");
        let mut binding = 0.0f64;
        for i in 0..exp.len() {
            for j in (i + 1)..exp.len() {
                let dx = exp[j].x - exp[i].x;
                let dy = exp[j].y - exp[i].y;
                binding += exp[i].mass * exp[j].mass / (dx * dx + dy * dy + 1.0).sqrt();
            }
        }
        let binding_rel = (binding - rec.binding).abs() / rec.binding.abs().max(1e-30);
        assert!(binding_rel < 1e-9, "binding residual {binding_rel}");
        let mut ke = 0.0f64;
        for b in exp {
            ke += 0.5 * b.mass * (b.vx * b.vx + b.vy * b.vy);
        }
        let pe = 0.0f64 - binding;
        let energy_rel = ((ke + pe) - rec.energy).abs() / rec.energy.abs().max(1.0);
        assert!(energy_rel < 1e-9, "energy delta {energy_rel}");
        let mut qxx = 0.0f64;
        let mut qxy = 0.0f64;
        let mut qyy = 0.0f64;
        for b in exp {
            let dx = b.x - rec.com_x;
            let dy = b.y - rec.com_y;
            qxx += b.mass * dx * dx;
            qxy += b.mass * dx * dy;
            qyy += b.mass * dy * dy;
        }
        let q_scale = rec.qxx.abs().max(rec.qyy.abs()).max(1e-30);
        let quad = (qxx - rec.qxx)
            .abs()
            .max((qxy - rec.qxy).abs())
            .max((qyy - rec.qyy).abs())
            / q_scale;
        assert!(quad < 4.0, "quadrupole deviation {quad}");
    }

    #[test]
    fn radial_modes_differ_from_section20() {
        let schedule = |w: &mut GravityWorld| {
            w.schedule(20, 0, Action::Collapse);
            w.schedule(90, 0, Action::Promote);
        };
        let mut a = GravityWorld::new(17, 12);
        schedule(&mut a);
        let mut b = GravityWorld::new(17, 12);
        b.radial = true;
        schedule(&mut b);
        for _ in 0..150 {
            a.step();
            b.step();
        }
        assert_ne!(a.world_hash(), b.world_hash());
    }

    #[test]
    fn restitution_bounces_closes_on_minus_e_vn_and_keeps_ledger() {
        let mut w = GravityWorld::new(11, 32);
        w.contacts = true;
        w.contact_params = true;
        w.restitution = 0.5;
        w.friction = 0.25;
        let (_, _, _, px0, py0, _) = w.totals();
        let mut records = 0u64;
        let mut worst = 0.0f64;
        for _ in 0..400 {
            w.step();
            for c in &w.last_contacts {
                assert!(c.jn > 0.0);
                let expect = 0.0 - c.vn * 0.5;
                worst = worst.max((c.vn_after - expect).abs());
                records += 1;
            }
            w.last_contacts.clear();
        }
        assert!(records >= 3, "expected several contacts, got {records}");
        assert!(worst < 1e-12, "worst |vn_after + e*vn| {worst}");
        let (_, _, _, px1, py1, _) = w.totals();
        assert_eq!(
            (px0, py0),
            (px1, py1),
            "all-fine ledger exact under e/friction"
        );
    }

    #[test]
    fn wall_bounce_reflects_and_books_ledger() {
        let mut w = GravityWorld::new(3, 1);
        w.contacts = true;
        w.contact_params = true;
        w.walls = true;
        w.restitution = 0.5;
        w.friction = 0.25;
        w.bodies[0].x = -1.0;
        w.bodies[0].y = 50.0;
        w.bodies[0].vx = -0.5;
        w.bodies[0].vy = 0.25;
        let m = w.bodies[0].mass;
        let px0 = w.px;
        let py0 = w.py;
        w.step();
        let c = &w.last_contacts[0];
        assert_eq!(c.b, WALL_BASE, "x=0 wall pseudo id");
        assert_eq!(c.a, 0);
        assert!(c.jn > 0.0);
        let x_at_pass = -1.0 + (-0.5) * DT;
        assert!(
            (c.cx - ((x_at_pass + 0.0) * 0.5)).abs() < 1e-12,
            "contact point midpoint {}",
            c.cx
        );
        assert!(
            (c.cy - (50.0 + 0.25 * DT)).abs() < 1e-9,
            "wall contact carries the body y {}",
            c.cy
        );
        assert!(
            (w.bodies[0].vx - 0.25).abs() < 1e-12,
            "reflected vx {}",
            w.bodies[0].vx
        );
        assert!(
            (w.bodies[0].vy - 0.0625).abs() < 1e-12,
            "friction-damped vy {}",
            w.bodies[0].vy
        );
        let s = (1.0 + 0.5) * -0.5;
        let jt = 0.1875 * m;
        assert!(
            (w.px - (px0 + m * (s * (0.0 - 1.0)))).abs() < 1e-12,
            "ledger px {}",
            w.px - px0
        );
        assert!(
            (w.py - (py0 + jt * (0.0 - 1.0))).abs() < 1e-12,
            "ledger py books the tangential impulse {}",
            w.py - py0
        );
        assert!(c.vn_after > 0.0, "separating after bounce");
    }

    #[test]
    fn monopole_contact_one_sided_with_frozen_totals() {
        let mut world = None;
        for seed in 0..100_000u64 {
            let w = GravityWorld::new(seed, 5);
            let in3 = w
                .bodies
                .iter()
                .filter(|b| b.x >= 64.0 && b.x < 128.0 && b.y >= 64.0 && b.y < 128.0)
                .count();
            if in3 >= 3 && w.bodies[0].x < 64.0 {
                world = Some(w);
                break;
            }
        }
        let mut w = world.expect("seed with bodies inside and outside region 3");
        w.contacts = true;
        w.contact_params = true;
        w.restitution = 0.5;
        w.schedule(1, 3, Action::Collapse);
        w.step();
        let (com_x, com_y, vcom_x, vcom_y, mass) = {
            let t = w.region_totals[3].expect("collapsed");
            (t.com_x, t.com_y, t.vcom_x, t.vcom_y, t.mass)
        };
        w.bodies[0].x = com_x - 2.0;
        w.bodies[0].y = com_y;
        w.bodies[0].vx = vcom_x + 1.0;
        w.bodies[0].vy = vcom_y;
        let mi = w.bodies[0].mass;
        w.step();
        let events = std::mem::take(&mut w.last_contacts);
        assert_eq!(events.len(), 1, "one monopole contact");
        let c = &events[0];
        assert_eq!(c.b, MONOPOLE_BASE + 3, "region 3 pseudo id");
        assert_eq!(c.a, 0);
        assert!(c.jn > 0.0);
        let expected_jn = (0.0 - c.vn) * (1.0 + 0.5) * mi;
        assert!((c.jn - expected_jn).abs() < 1e-12, "one-sided jn {}", c.jn);
        assert!(
            (c.vn_after - (0.0 - c.vn * 0.5)).abs() < 1e-12,
            "bounce closes on -e*vn"
        );
        let t = w.region_totals[3].expect("still collapsed");
        assert_eq!(
            (t.com_x, t.com_y, t.vcom_x, t.vcom_y, t.mass),
            (com_x, com_y, vcom_x, vcom_y, mass)
        );
        let big_r = CONTACT_R * mass.sqrt();
        let r0 = CONTACT_R * mi.sqrt();
        let dx = w.bodies[0].x - com_x;
        let dy = w.bodies[0].y - com_y;
        assert!(
            dx * dx + dy * dy < (big_r + r0) * (big_r + r0),
            "still inside the disk"
        );
    }

    #[test]
    fn wallshot_corpus_hits_all_four_walls() {
        let mut w = GravityWorld::corpus_world("wallshot", 3, 16);
        w.contacts = true;
        w.contact_params = true;
        w.walls = true;
        w.restitution = 0.7;
        w.friction = 0.3;
        let mut per_wall = [0usize; 4];
        let mut worst = 0.0f64;
        for _ in 0..800 {
            w.step();
            for c in &w.last_contacts {
                assert!(c.jn > 0.0);
                let wall = (c.b - WALL_BASE) as usize;
                assert!(
                    wall < 4,
                    "wallshot corpus must stay wall-only, got {:08x}",
                    c.b
                );
                per_wall[wall] += 1;
                let expect = 0.0 - c.vn * 0.7;
                worst = worst.max((c.vn_after - expect).abs());
            }
            w.last_contacts.clear();
        }
        assert!(
            per_wall.iter().all(|&n| n >= 3),
            "per-wall hits {per_wall:?}"
        );
        assert!(worst < 1e-12, "worst |vn_after + e*vn| {worst}");
    }

    #[test]
    fn coarsehit_corpus_lands_static_contacts() {
        let mut w = GravityWorld::corpus_world("coarsehit", 11, 8);
        w.contacts = true;
        w.contact_params = true;
        w.restitution = 0.5;
        w.friction = 0.25;
        w.schedule(1, 3, Action::Demote);
        let mut statics = 0usize;
        for _ in 0..500 {
            w.step();
            for c in &w.last_contacts {
                assert!(c.jn > 0.0);
                let j = c.b as usize;
                if j < w.bodies.len() && w.coarse[j].is_some() {
                    statics += 1;
                    let expect = 0.0 - c.vn * 0.5;
                    assert!(
                        (c.vn_after - expect).abs() < 1e-12,
                        "static bounce closes on -e*vn"
                    );
                }
            }
            w.last_contacts.clear();
        }
        assert!(
            statics >= 4,
            "expected fine x coarse static contacts, got {statics}"
        );
    }

    #[test]
    #[should_panic(expected = "unknown corpus profile")]
    fn corpus_profile_unknown_rejected() {
        let _ = GravityWorld::corpus_world("nonsense", 1, 4);
    }

    #[test]
    fn corpus_profile_masses_match_spec_ics() {
        let spec = initial_conditions(5, 6);
        for profile in ["wallshot", "coarsehit"] {
            let corpus = corpus_initial_conditions(profile, 5, 6);
            for (a, b) in spec.iter().zip(corpus.iter()) {
                assert_eq!(a.id, b.id);
                assert_eq!(a.mass.to_bits(), b.mass.to_bits());
            }
        }
    }
}
