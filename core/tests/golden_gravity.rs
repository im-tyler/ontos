use std::fs::File;
use std::path::Path;

use ontos_core::audio::{self, Excitation};
use ontos_core::gravity::{Action, GravityWorld};
use ontos_stream::{Record, StreamReader};

fn verify_golden_gravity(name: &str, seed: u64) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    verify_golden_gravity_at(&path, seed, None);
}

fn verify_golden_gravity_at(path: &Path, seed: u64, wav: Option<&Path>) {
    let name = path.file_name().unwrap().to_string_lossy();
    let input = File::open(path).expect("golden stream missing");
    let mut reader = StreamReader::new(input).expect("bad golden header");
    let body_count = reader
        .body_count()
        .expect("gravity stream must carry body_count");

    let mut world = GravityWorld::new(seed, body_count);
    let mut pending: Vec<(u8, Action)> = Vec::new();
    let mut collapse_records: Vec<Record> = Vec::new();
    let mut multipole_records: Vec<Record> = Vec::new();
    let mut radial_records: Vec<Record> = Vec::new();
    let mut contact_records: Vec<Record> = Vec::new();
    let mut pending_multipole = false;
    let mut pending_radial = false;
    let mut body_index = 0usize;
    let mut last_tick = 0u64;
    let mut masses = vec![0.0f64; body_count as usize];
    let mut collapse_mass = [0.0f64; 4];
    let mut stream_contacts: Vec<(u64, u32, u32, f64)> = Vec::new();

    while let Some(record) = reader.next_record().expect("golden parse failed") {
        match record {
            Record::Header { .. } => panic!("header record mid-stream"),
            Record::TickHeader { tick } => {
                world.multipole = pending_multipole;
                pending_multipole = false;
                world.radial = world.radial || pending_radial;
                pending_radial = false;
                world.contacts = world.contacts || !contact_records.is_empty();
                for &(region, action) in &pending {
                    world.schedule(world.tick + 1, region, action);
                }
                pending.clear();
                world.step();
                assert_eq!(tick, world.tick, "{name}: tick divergence at {tick}");
                for rec in collapse_records.drain(..) {
                    if let Record::RegionCollapsed {
                        tick,
                        region_x,
                        region_y,
                        body_count,
                        mass,
                        com_x,
                        com_y,
                        px,
                        py,
                        energy,
                    } = rec
                    {
                        assert_eq!(tick, world.tick, "{name}: RegionCollapsed tick");
                        let region = (region_y * 2 + region_x) as u8;
                        let tot = world
                            .collapsed_totals(region)
                            .expect("{name}: region collapsed at record");
                        assert_eq!(body_count, tot.count, "{name}: collapse count");
                        assert_eq!(mass.to_bits(), tot.mass.to_bits(), "{name}: collapse mass");
                        assert_eq!(
                            com_x.to_bits(),
                            tot.com_x.to_bits(),
                            "{name}: collapse com_x"
                        );
                        assert_eq!(
                            com_y.to_bits(),
                            tot.com_y.to_bits(),
                            "{name}: collapse com_y"
                        );
                        assert_eq!(px.to_bits(), tot.px.to_bits(), "{name}: collapse px");
                        assert_eq!(py.to_bits(), tot.py.to_bits(), "{name}: collapse py");
                        assert_eq!(
                            energy.to_bits(),
                            tot.energy.to_bits(),
                            "{name}: collapse energy"
                        );
                    }
                }
                for rec in multipole_records.drain(..) {
                    if let Record::RegionMultipole {
                        tick,
                        region_x,
                        region_y,
                        mx,
                        my,
                        qxx,
                        qxy,
                        qyy,
                    } = rec
                    {
                        assert_eq!(tick, world.tick, "{name}: RegionMultipole tick");
                        let region = (region_y * 2 + region_x) as u8;
                        let tot = world
                            .collapsed_totals(region)
                            .expect("{name}: region multipole at record");
                        assert_eq!(mx.to_bits(), tot.mx.to_bits(), "{name}: multipole mx");
                        assert_eq!(my.to_bits(), tot.my.to_bits(), "{name}: multipole my");
                        assert_eq!(qxx.to_bits(), tot.qxx.to_bits(), "{name}: multipole qxx");
                        assert_eq!(qxy.to_bits(), tot.qxy.to_bits(), "{name}: multipole qxy");
                        assert_eq!(qyy.to_bits(), tot.qyy.to_bits(), "{name}: multipole qyy");
                    }
                }
                for rec in radial_records.drain(..) {
                    if let Record::RegionRadial {
                        tick,
                        region_x,
                        region_y,
                        binding,
                    } = rec
                    {
                        assert_eq!(tick, world.tick, "{name}: RegionRadial tick");
                        let region = (region_y * 2 + region_x) as u8;
                        let tot = world
                            .collapsed_totals(region)
                            .expect("{name}: region radial at record");
                        assert_eq!(
                            binding.to_bits(),
                            tot.binding.to_bits(),
                            "{name}: radial binding"
                        );
                    }
                }
                body_index = 0;
                last_tick = tick;
                let local_contacts = std::mem::take(&mut world.last_contacts);
                assert_eq!(
                    contact_records.len(),
                    local_contacts.len(),
                    "{name}: contact count at tick {tick}"
                );
                for rec in contact_records.drain(..) {
                    if let Record::Contact {
                        tick,
                        body_a,
                        body_b,
                        jn,
                        cx,
                        cy,
                    } = rec
                    {
                        stream_contacts.push((tick, body_a, body_b, jn));
                        let local = local_contacts
                            .iter()
                            .find(|c| c.a == body_a && c.b == body_b)
                            .unwrap_or_else(|| {
                                panic!("{name}: contact ({body_a},{body_b}) not computed")
                            });
                        assert_eq!(tick, local.tick, "{name}: Contact tick");
                        assert_eq!(
                            jn.to_bits(),
                            local.jn.to_bits(),
                            "{name}: contact ({body_a},{body_b}) jn"
                        );
                        assert_eq!(
                            cx.to_bits(),
                            local.cx.to_bits(),
                            "{name}: contact ({body_a},{body_b}) cx"
                        );
                        assert_eq!(
                            cy.to_bits(),
                            local.cy.to_bits(),
                            "{name}: contact ({body_a},{body_b}) cy"
                        );
                    }
                }
            }
            Record::Snapshot { population } => {
                assert_eq!(
                    population,
                    world.bodies.len() as u64,
                    "{name}: population at tick {last_tick}"
                );
            }
            Record::CellFlipped { .. } => panic!("flip record in gravity stream"),
            Record::RegionLevel {
                region_x,
                region_y,
                level,
            } => {
                let action = match level {
                    0 => Action::Demote,
                    1 => Action::Promote,
                    _ => Action::Collapse,
                };
                pending.push(((region_y * 2 + region_x) as u8, action));
            }
            Record::RegionCollapsed { .. } => {
                if let Record::RegionCollapsed {
                    region_x,
                    region_y,
                    mass,
                    ..
                } = record
                {
                    collapse_mass[(region_y * 2 + region_x) as usize] = mass;
                }
                collapse_records.push(record);
            }
            Record::RegionMultipole { .. } => {
                multipole_records.push(record);
                pending_multipole = true;
            }
            Record::RegionRadial { .. } => {
                radial_records.push(record);
                pending_radial = true;
            }
            Record::ContactParams {
                restitution,
                friction,
                walls,
            } => {
                assert!(!world.contact_params, "{name}: duplicate ContactParams");
                world.contact_params = true;
                world.restitution = restitution;
                world.friction = friction;
                world.walls = walls == 1;
            }
            Record::Contact { .. } => contact_records.push(record),
            Record::RegionState {
                tick,
                region_x,
                region_y,
                level,
                population,
                hash,
            } => {
                let region = (region_y * 2 + region_x) as u8;
                let (w_level, w_pop, w_hash) = world.region_hash(region);
                assert_eq!(tick, world.tick, "{name}: RegionState tick");
                assert_eq!(level, w_level, "{name}: region level {region}");
                assert_eq!(population, w_pop, "{name}: region population {region}");
                assert_eq!(hash, w_hash, "{name}: region hash {region} tick {tick}");
            }
            Record::BodyState {
                tick,
                body_id,
                region,
                level,
                x,
                y,
                vx,
                vy,
                mass,
            } => {
                masses[body_id as usize] = mass;
                let (b, w_region, w_level) = world.emitted_state(body_index);
                assert_eq!(tick, world.tick, "{name}: BodyState tick");
                assert_eq!(body_id, b.id, "{name}: body id order");
                assert_eq!(region, w_region, "{name}: body {body_id} region");
                assert_eq!(level, w_level, "{name}: body {body_id} level");
                assert_eq!(
                    x.to_bits(),
                    b.x.to_bits(),
                    "{name}: body {body_id} x at tick {tick}"
                );
                assert_eq!(
                    y.to_bits(),
                    b.y.to_bits(),
                    "{name}: body {body_id} y at tick {tick}"
                );
                assert_eq!(
                    vx.to_bits(),
                    b.vx.to_bits(),
                    "{name}: body {body_id} vx at tick {tick}"
                );
                assert_eq!(
                    vy.to_bits(),
                    b.vy.to_bits(),
                    "{name}: body {body_id} vy at tick {tick}"
                );
                assert_eq!(
                    mass.to_bits(),
                    b.mass.to_bits(),
                    "{name}: body {body_id} mass"
                );
                body_index += 1;
            }
            Record::TotalsState {
                tick,
                fine_count,
                coarse_count,
                mass,
                px,
                py,
                energy,
            } => {
                let (w_fine, w_coarse, w_mass, w_px, w_py, w_energy) = world.totals();
                assert_eq!(tick, world.tick, "{name}: TotalsState tick");
                assert_eq!(fine_count, w_fine, "{name}: fine count at tick {tick}");
                assert_eq!(
                    coarse_count, w_coarse,
                    "{name}: coarse count at tick {tick}"
                );
                assert_eq!(
                    mass.to_bits(),
                    w_mass.to_bits(),
                    "{name}: mass at tick {tick}"
                );
                assert_eq!(px.to_bits(), w_px.to_bits(), "{name}: px at tick {tick}");
                assert_eq!(py.to_bits(), w_py.to_bits(), "{name}: py at tick {tick}");
                assert_eq!(
                    energy.to_bits(),
                    w_energy.to_bits(),
                    "{name}: energy at tick {tick}"
                );
            }
        }
    }
    assert_eq!(
        body_index,
        world.bodies.len(),
        "{name}: emitted all body records"
    );
    assert_eq!(last_tick, world.tick, "{name}: final tick");
    if let Some(wav_path) = wav {
        let excitations: Vec<Excitation> = stream_contacts
            .iter()
            .map(|&(tick, a, b, jn)| {
                let ma = masses[a as usize];
                let mu = if b >= ontos_core::gravity::WALL_BASE {
                    ma
                } else if b >= ontos_core::gravity::MONOPOLE_BASE {
                    let m = collapse_mass[(b - ontos_core::gravity::MONOPOLE_BASE) as usize];
                    (ma * m) / (ma + m)
                } else {
                    let mb = masses[b as usize];
                    (ma * mb) / (ma + mb)
                };
                Excitation { tick, mu, jn }
            })
            .collect();
        let pcm = audio::synthesize(&excitations, last_tick);
        let bytes = audio::wav_bytes(&pcm);
        let want = std::fs::read(wav_path).expect("golden wav missing");
        assert_eq!(bytes, want, "{name}: synthesized wav matches golden");
    }
}

#[test]
fn golden_gravity_all_fine() {
    verify_golden_gravity("g_all_fine.stream", 42);
}

#[test]
fn golden_gravity_window() {
    verify_golden_gravity("g_window.stream", 42);
}

#[test]
fn golden_gravity_refit() {
    verify_golden_gravity("g_refit.stream", 7);
}

#[test]
fn golden_gravity_multi() {
    verify_golden_gravity("g_multi.stream", 3);
}

#[test]
fn golden_gravity_observer() {
    verify_golden_gravity("g_observer.stream", 5);
}

#[test]
fn golden_gravity_collapse() {
    verify_golden_gravity("g_collapse.stream", 11);
}

#[test]
fn golden_gravity_collapse_observer() {
    verify_golden_gravity("g_collapse_observer.stream", 13);
}

#[test]
fn golden_gravity_multipole() {
    verify_golden_gravity("g_multipole.stream", 17);
}

#[test]
fn golden_gravity_radial() {
    verify_golden_gravity("g_radial.stream", 17);
}

#[test]
fn golden_gravity_walls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    verify_golden_gravity_at(
        &root.join("g_walls.stream"),
        22,
        Some(&root.join("g_walls.wav")),
    );
}

#[test]
fn golden_gravity_restitution() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    verify_golden_gravity_at(
        &root.join("g_restitution.stream"),
        11,
        Some(&root.join("g_restitution.wav")),
    );
}

#[test]
fn golden_gravity_contact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    verify_golden_gravity_at(
        &root.join("g_contact.stream"),
        11,
        Some(&root.join("g_contact.wav")),
    );
}
