//! Headless world-tuning binary (R13 / D14).
//!
//! Writes the map image and world metrics without opening Godot:
//!
//! ```text
//! cargo run -p sim-core --bin worldsim -- --seed 1 --years 2000 --out run.png
//! cargo run -p sim-core --bin worldsim -- --years 2000 --overlay biome --out map.png
//! ```
//!
//! Tuning metrics: settled tiles and population over time, culture count,
//! tiles and people per culture, and pairwise centroid distances. They are the
//! numbers that decide whether migration and drift produce the Milestone 1
//! "3+ visibly distinct peoples" gate.
//!
//! PNG encoding is hand-rolled with uncompressed deflate blocks: a dev-dependency
//! is not visible to `[[bin]]` targets, and a regular `image` dependency would
//! be linked into the shipped dylib. The PNG format is frozen, so this is a
//! stable ~60 lines.

use std::path::Path;
use std::process::ExitCode;

use sim_core::World;
use sim_core::culture;
use sim_core::world::{GRID_HEIGHT, GRID_WIDTH, Overlay};
use sim_core::worldgen::Biome;

fn main() -> ExitCode {
    let mut seed = 1_u64;
    let mut years = 0_u32;
    let mut trace = 0_u32;
    let mut out = None::<String>;
    let mut overlay = Overlay::Culture;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next();
        match arg.as_str() {
            "--seed" => seed = value().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--years" => years = value().and_then(|v| v.parse().ok()).unwrap_or(years),
            "--trace" => trace = value().and_then(|v| v.parse().ok()).unwrap_or(trace),
            "--out" => out = value(),
            "--overlay" => {
                overlay = match value().as_deref() {
                    Some("biome") => Overlay::Biome,
                    Some("culture") | None => Overlay::Culture,
                    Some(other) => {
                        eprintln!("unknown overlay: {other} (biome|culture)");
                        return ExitCode::FAILURE;
                    }
                }
            }
            other => {
                eprintln!("unknown argument: {other}");
                return ExitCode::FAILURE;
            }
        }
    }

    let mut world = World::new(seed);
    if trace > 0 {
        println!(
            "{:>6}  {:>7}  {:>9}  {:>8}  {:>4}",
            "year", "settled", "people", "largest", "cultures"
        );
        while world.tick() < years as u64 {
            let chunk = trace.min(years - world.tick() as u32);
            world.step(chunk);
            print_progress(&world);
        }
    } else {
        world.step(years);
    }

    print_biomes(&world);
    print_cultures(&world);

    if let Some(out) = out {
        let rgba = world.paint_overlay(overlay);
        if let Err(err) = write_png(
            Path::new(&out),
            GRID_WIDTH as u32,
            GRID_HEIGHT as u32,
            &rgba,
        ) {
            eprintln!("failed to write {out}: {err}");
            return ExitCode::FAILURE;
        }
        println!("wrote {out}");
    }
    ExitCode::SUCCESS
}

fn settled(world: &World) -> usize {
    world.population.iter().filter(|&&p| p >= 1.0).count()
}

fn people(world: &World) -> f32 {
    world.population.iter().sum()
}

fn print_progress(world: &World) {
    let largest = world.cultures().iter().map(|c| c.tiles).max().unwrap_or(0);
    println!(
        "{:>6}  {:>7}  {:>9.0}  {:>8}  {:>4}",
        world.tick(),
        settled(world),
        people(world),
        largest,
        world.cultures().len()
    );
}

fn print_biomes(world: &World) {
    let tiles = world.biome.len() as f32;
    let land = world.biome.iter().filter(|b| **b != Biome::Ocean).count();
    let capacity: f32 = world.carrying_capacity.iter().sum();
    println!(
        "seed {}, year {}: land {:.1}%, capacity {:.0}",
        world.seed(),
        world.tick(),
        100.0 * land as f32 / tiles,
        capacity
    );
    for biome in [
        Biome::Ocean,
        Biome::Beach,
        Biome::Grassland,
        Biome::Forest,
        Biome::Desert,
        Biome::Mountain,
        Biome::Ice,
    ] {
        let count = world.biome.iter().filter(|b| **b == biome).count();
        println!(
            "  {biome:?}: {count} ({:.1}%)",
            100.0 * count as f32 / tiles
        );
    }
}

fn print_cultures(world: &World) {
    let orphans: Vec<usize> = (0..world.population.len())
        .filter(|&i| {
            world.population[i] >= 1.0 && world.culture_id[i] == sim_core::culture::NO_CULTURE
        })
        .collect();
    if !orphans.is_empty() {
        println!("orphaned settlements: {} e.g. {orphans:?}", orphans.len());
        for &i in orphans.iter().take(5) {
            let (x, y) = (i % GRID_WIDTH, i / GRID_WIDTH);
            println!(
                "  tile {i} ({x},{y}) pop {:.2} biome {:?}",
                world.population[i], world.biome[i]
            );
        }
    }
    let cultures = world.cultures();
    println!("cultures: {}", cultures.len());
    for culture in cultures {
        println!(
            "  #{} parent {:?} {:?} tiles {:>4} people {:>7.0}",
            culture.id, culture.parent_id, culture.favor, culture.tiles, culture.population
        );
        let parts = culture::components(world, culture.id);
        let sizes: Vec<usize> = parts.iter().map(|part| part.len()).collect();
        let mut furthest = 0.0_f32;
        for part in &parts {
            let mean = culture::weighted_mean(world, part);
            furthest = furthest.max(culture::distance(&mean, &culture.centroid));
        }
        println!("     components {sizes:?}, furthest {furthest:.3} from centroid");

        if let Some(main) = parts.iter().max_by_key(|part| part.len()) {
            let body = culture::weighted_mean(world, main);
            let mut report: Vec<(f32, usize)> = parts
                .iter()
                .filter(|part| part.len() >= 30)
                .map(|part| {
                    (
                        culture::distance(&culture::weighted_mean(world, part), &body),
                        part.len(),
                    )
                })
                .collect();
            report.sort_by(|a, b| b.0.total_cmp(&a.0));
            let top: Vec<String> = report
                .iter()
                .take(4)
                .map(|(d, n)| format!("{n} tiles @ {d:.3}"))
                .collect();
            println!("     vs body: {}", top.join(", "));
        }
    }
    for (i, a) in cultures.iter().enumerate() {
        for b in &cultures[i + 1..] {
            let d = culture::distance(&a.centroid, &b.centroid);
            if d > 0.0 {
                println!("  centroid distance #{} <-> #{}: {d:.3}", a.id, b.id);
            }
        }
    }
}

/// Writes an 8-bit RGBA PNG using zlib "stored" (uncompressed) deflate blocks.
fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    // IHDR: width, height, bit depth 8, color type 6 (RGBA), deflate, no filter, no interlace.
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_chunk(&mut out, b"IHDR", &ihdr);

    // Raw image data: one filter byte (None) per scanline.
    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * stride..(y + 1) * stride]);
    }

    // zlib stream: header, then stored deflate blocks, then Adler-32.
    let mut zlib = Vec::new();
    zlib.extend_from_slice(&[0x78, 0x01]);
    let mut offset = 0;
    loop {
        let len = (raw.len() - offset).min(u16::MAX as usize);
        let last = offset + len >= raw.len();
        zlib.push(u8::from(last)); // BFINAL + BTYPE 00 (stored)
        zlib.extend_from_slice(&(len as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(len as u16)).to_le_bytes());
        zlib.extend_from_slice(&raw[offset..offset + len]);
        offset += len;
        if last {
            break;
        }
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());
    write_chunk(&mut out, b"IDAT", &zlib);

    write_chunk(&mut out, b"IEND", &[]);
    std::fs::write(path, out)
}

/// One PNG chunk: length, type, data, CRC-32 over type + data.
fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
