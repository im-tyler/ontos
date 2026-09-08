use std::fs::File;
use std::path::PathBuf;

use ontos_core::audio::{self, Excitation};
use ontos_core::gravity::{Action, GravityWorld};
use ontos_core::{Level, World};
use ontos_stream::{Record, StreamWriter};

enum Mode {
    Life,
    Gravity,
}

fn parse_region(raw: &str) -> u32 {
    let v: u32 = raw
        .parse()
        .unwrap_or_else(|_| panic!("invalid region {raw}"));
    if v > 1 {
        panic!("region coordinate out of range: {v}");
    }
    v
}

fn level_byte(action: Action) -> u8 {
    match action {
        Action::Demote => 0,
        Action::Promote => 1,
        Action::Collapse => 2,
    }
}

fn main() {
    let mut ticks: u64 = 100;
    let mut seed: u64 = 42;
    let mut out: Option<PathBuf> = None;
    let mut demote: Vec<(u32, u32)> = Vec::new();
    let mut promote: Vec<(u32, u32)> = Vec::new();
    let mut mode = Mode::Life;
    let mut bodies: u32 = 8;
    let mut events: Vec<(u64, u8, Action)> = Vec::new();
    let mut observer_offset: Option<u64> = None;
    let mut contacts = false;
    let mut wav: Option<PathBuf> = None;
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--ticks" => {
                ticks = args[i + 1].parse().expect("invalid ticks");
                i += 2;
            }
            "--seed" => {
                seed = args[i + 1].parse().expect("invalid seed");
                i += 2;
            }
            "--out" => {
                out = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--bodies" => {
                bodies = args[i + 1].parse().expect("invalid bodies");
                i += 2;
            }
            "--mode" => {
                mode = match args[i + 1].as_str() {
                    "life" => Mode::Life,
                    "gravity" => Mode::Gravity,
                    other => panic!("unknown mode {other}"),
                };
                i += 2;
            }
            "--contacts" => {
                contacts = true;
                i += 1;
            }
            "--wav" => {
                wav = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--demote" => {
                let rx: u32 = parse_region(&args[i + 1]);
                let ry: u32 = parse_region(&args[i + 2]);
                demote.push((rx, ry));
                i += 3;
            }
            "--promote" => {
                let rx: u32 = parse_region(&args[i + 1]);
                let ry: u32 = parse_region(&args[i + 2]);
                promote.push((rx, ry));
                i += 3;
            }
            "--demote-at" => {
                let t: u64 = args[i + 1].parse().expect("invalid tick");
                let rx: u32 = parse_region(&args[i + 2]);
                let ry: u32 = parse_region(&args[i + 3]);
                events.push((t, (ry * 2 + rx) as u8, Action::Demote));
                i += 4;
            }
            "--promote-at" => {
                let t: u64 = args[i + 1].parse().expect("invalid tick");
                let rx: u32 = parse_region(&args[i + 2]);
                let ry: u32 = parse_region(&args[i + 3]);
                events.push((t, (ry * 2 + rx) as u8, Action::Promote));
                i += 4;
            }
            "--collapse-at" => {
                let t: u64 = args[i + 1].parse().expect("invalid tick");
                let rx: u32 = parse_region(&args[i + 2]);
                let ry: u32 = parse_region(&args[i + 3]);
                events.push((t, (ry * 2 + rx) as u8, Action::Collapse));
                i += 4;
            }
            "--expand-at" => {
                let t: u64 = args[i + 1].parse().expect("invalid tick");
                let rx: u32 = parse_region(&args[i + 2]);
                let ry: u32 = parse_region(&args[i + 3]);
                events.push((t, (ry * 2 + rx) as u8, Action::Promote));
                i += 4;
            }
            "--observer" => {
                observer_offset = Some(args[i + 1].parse().expect("invalid observer offset"));
                i += 2;
            }
            _ => {
                eprintln!(
                    "usage: ontos [--mode life|gravity] [--ticks N] [--seed S] [--bodies N] [--out FILE]\n\
                     life:    [--demote RX RY] [--promote RX RY]...\n\
                     gravity: [--demote-at T RX RY] [--promote-at T RX RY] [--collapse-at T RX RY]\n\
                              [--expand-at T RX RY] [--observer OFFSET] [--contacts]\n\
                              [--wav FILE]..."
                );
                std::process::exit(1);
            }
        }
    }

    match mode {
        Mode::Life => run_life(ticks, seed, out, demote, promote),
        Mode::Gravity => run_gravity(
            ticks,
            seed,
            out,
            bodies,
            events,
            observer_offset,
            contacts,
            wav,
        ),
    }
}

fn run_life(
    ticks: u64,
    seed: u64,
    out: Option<PathBuf>,
    demote: Vec<(u32, u32)>,
    promote: Vec<(u32, u32)>,
) {
    let mut world = World::new(seed);
    world.seed_r_pentomino();
    let mut writer = out.map(|path| {
        StreamWriter::new(
            File::create(&path).expect("failed to create stream"),
            128,
            128,
        )
        .expect("failed to write stream header")
    });
    for &(rx, ry) in &demote {
        world.set_level(rx, ry, Level::Coarse);
        if let Some(w) = writer.as_mut() {
            w.write(&Record::RegionLevel {
                region_x: rx,
                region_y: ry,
                level: 0,
            })
            .expect("stream write failed");
        }
    }
    for &(rx, ry) in &promote {
        world.set_level(rx, ry, Level::Fine);
        if let Some(w) = writer.as_mut() {
            w.write(&Record::RegionLevel {
                region_x: rx,
                region_y: ry,
                level: 1,
            })
            .expect("stream write failed");
        }
    }

    for _ in 0..ticks {
        world.step();
        if let Some(w) = writer.as_mut() {
            w.write(&Record::TickHeader { tick: world.tick })
                .expect("stream write failed");
            w.write(&Record::Snapshot {
                population: world.population(),
            })
            .expect("stream write failed");
            for ry in 0..ontos_core::REGIONS_PER_AXIS {
                for rx in 0..ontos_core::REGIONS_PER_AXIS {
                    let region = world.region(rx, ry);
                    w.write(&Record::RegionState {
                        tick: world.tick,
                        region_x: rx,
                        region_y: ry,
                        level: match region.level {
                            ontos_core::Level::Coarse => 0u8,
                            ontos_core::Level::Fine => 1u8,
                        },
                        population: region.population(),
                        hash: world.region_hash(rx, ry),
                    })
                    .expect("stream write failed");
                }
            }
        }
    }
    if let Some(mut w) = writer {
        w.flush().expect("flush failed");
    }
    println!(
        "seed={} ticks={} population={} hash={:016x}",
        seed,
        world.tick,
        world.population(),
        world.hash_state()
    );
}

#[allow(clippy::too_many_arguments)]
fn run_gravity(
    ticks: u64,
    seed: u64,
    out: Option<PathBuf>,
    bodies: u32,
    mut events: Vec<(u64, u8, Action)>,
    observer_offset: Option<u64>,
    contacts: bool,
    wav: Option<PathBuf>,
) {
    events.sort();
    events.dedup();
    let mut world = GravityWorld::new(seed, bodies);
    world.contacts = contacts;
    for &(t, region, action) in &events {
        world.schedule(t, region, action);
    }
    if let Some(offset) = observer_offset {
        world.set_observer(offset);
    }
    let mut writer = out.map(|path| {
        StreamWriter::new_gravity(
            File::create(&path).expect("failed to create stream"),
            128,
            128,
            bodies,
        )
        .expect("failed to write stream header")
    });
    let mut all_contacts: Vec<(u64, u32, u32, f64, f64, f64)> = Vec::new();
    for _ in 0..ticks {
        let entering = world.tick + 1;
        if let Some(t) = world.events.get(&entering) {
            let t = t.clone();
            for &(region, action) in &t {
                let (rx, ry) = ((region % 2) as u32, (region / 2) as u32);
                if let Some(w) = writer.as_mut() {
                    w.write(&Record::RegionLevel {
                        region_x: rx,
                        region_y: ry,
                        level: level_byte(action),
                    })
                    .expect("stream write failed");
                }
            }
        }
        world.step();
        let tick_contacts = std::mem::take(&mut world.last_contacts);
        all_contacts.extend(
            tick_contacts
                .iter()
                .map(|c| (c.tick, c.a, c.b, c.jn, c.cx, c.cy)),
        );
        if let Some(w) = writer.as_mut() {
            if let Some(observer) = world.observer.as_ref() {
                for &(region, to_coarse) in &observer.policy_events {
                    let (rx, ry) = ((region % 2) as u32, (region / 2) as u32);
                    w.write(&Record::RegionLevel {
                        region_x: rx,
                        region_y: ry,
                        level: if to_coarse { 0 } else { 1 },
                    })
                    .expect("stream write failed");
                }
            }
            if let Some(observer) = world.observer.as_mut() {
                observer.policy_events.clear();
            }
            for tot in std::mem::take(&mut world.collapse_records) {
                w.write(&Record::RegionCollapsed {
                    tick: tot.tick,
                    region_x: (tot.region % 2) as u32,
                    region_y: (tot.region / 2) as u32,
                    body_count: tot.count,
                    mass: tot.mass,
                    com_x: tot.com_x,
                    com_y: tot.com_y,
                    px: tot.px,
                    py: tot.py,
                    energy: tot.energy,
                })
                .expect("stream write failed");
                w.write(&Record::RegionMultipole {
                    tick: tot.tick,
                    region_x: (tot.region % 2) as u32,
                    region_y: (tot.region / 2) as u32,
                    mx: tot.mx,
                    my: tot.my,
                    qxx: tot.qxx,
                    qxy: tot.qxy,
                    qyy: tot.qyy,
                })
                .expect("stream write failed");
            }
            for c in &tick_contacts {
                w.write(&Record::Contact {
                    tick: c.tick,
                    body_a: c.a,
                    body_b: c.b,
                    jn: c.jn,
                    cx: c.cx,
                    cy: c.cy,
                })
                .expect("stream write failed");
            }
            w.write(&Record::TickHeader { tick: world.tick })
                .expect("stream write failed");
            w.write(&Record::Snapshot {
                population: world.bodies.len() as u64,
            })
            .expect("stream write failed");
            let (fine, coarse, mass, px, py, energy) = world.totals();
            w.write(&Record::TotalsState {
                tick: world.tick,
                fine_count: fine,
                coarse_count: coarse,
                mass,
                px,
                py,
                energy,
            })
            .expect("stream write failed");
            for region in 0..4u8 {
                let (level, population, hash) = world.region_hash(region);
                w.write(&Record::RegionState {
                    tick: world.tick,
                    region_x: (region % 2) as u32,
                    region_y: (region / 2) as u32,
                    level,
                    population,
                    hash,
                })
                .expect("stream write failed");
            }
            for i in 0..world.bodies.len() {
                w.write(&body_state_record(&world, i))
                    .expect("stream write failed");
            }
        }
    }
    if let Some(mut w) = writer {
        w.flush().expect("flush failed");
    }
    let (.., px, py, energy) = world.totals();
    let mut line = format!(
        "seed={} ticks={} bodies={} hash={:016x} px={:e} py={:e} E={:e} contacts={}",
        seed,
        world.tick,
        world.bodies.len(),
        world.world_hash(),
        px,
        py,
        energy,
        all_contacts.len()
    );
    if wav.is_some() || contacts {
        let excitations: Vec<Excitation> = all_contacts
            .iter()
            .map(|&(tick, a, b, jn, _, _)| Excitation {
                tick,
                mass_a: world.bodies[a as usize].mass,
                mass_b: world.bodies[b as usize].mass,
                jn,
            })
            .collect();
        let pcm = audio::synthesize(&excitations, world.tick);
        if let Some(path) = &wav {
            std::fs::write(path, audio::wav_bytes(&pcm)).expect("failed to write wav");
        }
        line.push_str(&format!(" audio={:016x}", audio::pcm_hash(&pcm)));
    }
    println!("{line}");
}

fn body_state_record(world: &GravityWorld, i: usize) -> Record {
    let (b, region, level) = world.emitted_state(i);
    Record::BodyState {
        tick: world.tick,
        body_id: b.id,
        region,
        level,
        x: b.x,
        y: b.y,
        vx: b.vx,
        vy: b.vy,
        mass: b.mass,
    }
}
