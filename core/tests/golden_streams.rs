use std::fs::File;
use std::path::Path;

use ontos_core::{Level, World};
use ontos_stream::{Record, StreamReader};

fn verify_golden(name: &str, seed: u64) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    let input = File::open(&path).expect("golden stream missing");
    let mut reader = StreamReader::new(input).expect("bad golden header");
    assert_eq!(reader.header(), (128, 128));

    let mut world = World::new(seed);
    world.seed_r_pentomino();
    let mut ticks = 0u64;

    while let Some(record) = reader.next_record().expect("golden parse failed") {
        match record {
            Record::Header { .. } => panic!("header record mid-stream"),
            Record::TickHeader { tick } => {
                world.step();
                ticks += 1;
                assert_eq!(tick, world.tick, "{name}: tick divergence at {tick}");
            }
            Record::Snapshot { population } => {
                assert_eq!(
                    population,
                    world.population(),
                    "{name}: population at tick {ticks}"
                );
            }
            Record::CellFlipped { .. } => {}
            Record::BodyState { .. } => panic!("body record in life stream"),
            Record::TotalsState { .. } => panic!("totals record in life stream"),
            Record::RegionCollapsed { .. } => panic!("collapse record in life stream"),
            Record::RegionMultipole { .. } => panic!("multipole record in life stream"),
            Record::Contact { .. } => panic!("contact record in life stream"),
            Record::RegionLevel {
                region_x,
                region_y,
                level,
            } => {
                world.set_level(
                    region_x,
                    region_y,
                    if level == 0 {
                        Level::Coarse
                    } else {
                        Level::Fine
                    },
                );
            }
            Record::RegionState {
                tick,
                region_x,
                region_y,
                level,
                population,
                hash,
            } => {
                assert_eq!(tick, world.tick, "{name}: RegionState tick at {ticks}");
                let region = world.region(region_x, region_y);
                let expected_level = match region.level {
                    Level::Coarse => 0u8,
                    Level::Fine => 1u8,
                };
                assert_eq!(
                    level, expected_level,
                    "{name}: region level {region_x},{region_y}"
                );
                assert_eq!(
                    population,
                    region.population(),
                    "{name}: region pop {region_x},{region_y}"
                );
                assert_eq!(
                    hash,
                    world.region_hash(region_x, region_y),
                    "{name}: region hash {region_x},{region_y} tick {ticks}"
                );
            }
            Record::RegionRadial { .. }
            | Record::ContactParams { .. }
            | Record::RegionShells { .. } => {
                panic!("{name}: version 2 record in a life stream");
            }
        }
    }
}

#[test]
fn golden_r_pentomino() {
    verify_golden("r_pentomino.stream", 42);
}

#[test]
fn golden_promote_roundtrip() {
    verify_golden("promote_roundtrip.stream", 42);
}

#[test]
fn golden_mixed_levels() {
    verify_golden("mixed.stream", 7);
}

#[test]
fn golden_one_tick() {
    verify_golden("one_tick.stream", 0);
}
