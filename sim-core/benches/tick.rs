//! Milestone 1 benchmark gate (design, Frame and tick scheduling): 100 years
//! per second, so one sim-year must tick in under 10 ms on a full map.
//!
//! ```text
//! cargo bench -p sim-core
//! ```
//!
//! The bench matures a world first, because a frontier map is much cheaper
//! than a settled one and the gate is about the expensive case.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use sim_core::World;
use sim_core::world::Overlay;

/// One sim-year on a fully settled map.
fn tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick");
    group.throughput(Throughput::Elements(1));
    group.bench_function("year_full_map", |b| {
        let world = matured();
        // Clone per iteration so every sample starts at the same year: a world
        // left running drifts past the gate's state and slowly gets costlier.
        b.iter_batched(
            || world.clone(),
            |mut world| world.step(1),
            criterion::BatchSize::SmallInput,
        );
    });
    group.finish();
}

/// Repainting the map must be cheap enough for every revision (R6).
fn paint(c: &mut Criterion) {
    let mut group = c.benchmark_group("paint");
    group.bench_function("biome_overlay", |b| {
        let world = matured();
        b.iter(|| world.paint_overlay(Overlay::Biome));
    });
    group.bench_function("culture_overlay", |b| {
        let world = matured();
        b.iter(|| world.paint_overlay(Overlay::Culture));
    });
    group.finish();
}

fn matured() -> World {
    let mut world = World::new(1);
    world.step(2_000);
    world
}

criterion_group!(benches, tick, paint);
criterion_main!(benches);
