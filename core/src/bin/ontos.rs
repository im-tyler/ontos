use std::fs::File;
use std::path::PathBuf;

use ontos_core::{Level, World};
use ontos_stream::{Record, StreamWriter};

fn main() {
    let mut ticks: u64 = 100;
    let mut seed: u64 = 42;
    let mut out: Option<PathBuf> = None;
    let mut demote: Vec<(u32, u32)> = Vec::new();
    let mut promote: Vec<(u32, u32)> = Vec::new();
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
            "--demote" => {
                let rx: u32 = args[i + 1].parse().expect("invalid region x");
                let ry: u32 = args[i + 2].parse().expect("invalid region y");
                demote.push((rx, ry));
                i += 3;
            }
            "--promote" => {
                let rx: u32 = args[i + 1].parse().expect("invalid region x");
                let ry: u32 = args[i + 2].parse().expect("invalid region y");
                promote.push((rx, ry));
                i += 3;
            }
            _ => {
                eprintln!(
                    "usage: ontos [--ticks N] [--seed S] [--out FILE] [--demote RX RY] [--promote RX RY]..."
                );
                std::process::exit(1);
            }
        }
    }

    let mut world = World::new(seed);
    world.seed_r_pentomino();
    let mut writer = out.map(|path| {
        StreamWriter::new(File::create(&path).expect("failed to create stream"), 128, 128)
            .expect("failed to write stream header")
    });
    for &(rx, ry) in &demote {
        world.set_level(rx, ry, Level::Coarse);
        if let Some(w) = writer.as_mut() {
            w.write(&Record::RegionLevel { region_x: rx, region_y: ry, level: 0 })
                .expect("stream write failed");
        }
    }
    for &(rx, ry) in &promote {
        world.set_level(rx, ry, Level::Fine);
        if let Some(w) = writer.as_mut() {
            w.write(&Record::RegionLevel { region_x: rx, region_y: ry, level: 1 })
                .expect("stream write failed");
        }
    }

    for _ in 0..ticks {
        world.step();
        if let Some(w) = writer.as_mut() {
            w.write(&Record::TickHeader { tick: world.tick })
                .expect("stream write failed");
            w.write(&Record::Snapshot { population: world.population() })
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
