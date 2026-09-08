use std::fs::File;
use std::path::Path;

use ontos_core::gravity::{Action, GravityWorld};
use ontos_stream::{Record, StreamReader};

fn verify_golden_gravity(name: &str, seed: u64) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    let input = File::open(&path).expect("golden stream missing");
    let mut reader = StreamReader::new(input).expect("bad golden header");
    let body_count = reader
        .body_count()
        .expect("gravity stream must carry body_count");

    let mut world = GravityWorld::new(seed, body_count);
    let mut pending: Vec<(u8, Action)> = Vec::new();
    let mut collapse_records: Vec<Record> = Vec::new();
    let mut multipole_records: Vec<Record> = Vec::new();
    let mut pending_multipole = false;
    let mut body_index = 0usize;
    let mut last_tick = 0u64;

    while let Some(record) = reader.next_record().expect("golden parse failed") {
        match record {
            Record::Header { .. } => panic!("header record mid-stream"),
            Record::TickHeader { tick } => {
                world.multipole = pending_multipole;
                pending_multipole = false;
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
                body_index = 0;
                last_tick = tick;
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
            Record::RegionCollapsed { .. } => collapse_records.push(record),
            Record::RegionMultipole { .. } => {
                multipole_records.push(record);
                pending_multipole = true;
            }
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
