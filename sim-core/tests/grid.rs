//! Hex grid contracts (design, Grid; user direction 2026-10-27).
//!
//! The world is odd-r flat-top hexes stored in the same flat Vec layout
//! (`y * GRID_WIDTH + x`): only the neighbor math and the distance metric
//! change from the square grid. East-west still wraps (design); north-south
//! does not.

use sim_core::world::{GRID_HEIGHT, GRID_WIDTH, hex};

#[test]
fn every_interior_tile_has_six_neighbors() {
    for y in 1..GRID_HEIGHT - 1 {
        for x in 1..GRID_WIDTH - 1 {
            assert_eq!(
                hex::neighbors(x, y).count(),
                6,
                "tile ({x},{y}) should touch six others"
            );
        }
    }
}

#[test]
fn the_world_wraps_east_to_west_and_not_north_to_south() {
    // Column 0 and the last column are neighbors: the design's wrap.
    let east_edge: Vec<(usize, usize)> = hex::neighbors(GRID_WIDTH - 1, 40).collect();
    assert!(
        east_edge.contains(&(0, 40)),
        "the east edge touches the west edge: {east_edge:?}"
    );
    // The poles do not wrap.
    let north_edge: Vec<(usize, usize)> = hex::neighbors(40, 0).collect();
    assert!(
        !north_edge.iter().any(|&(_, y)| y >= GRID_HEIGHT),
        "the north edge stops: {north_edge:?}"
    );
    let south_edge: Vec<(usize, usize)> = hex::neighbors(40, GRID_HEIGHT - 1).collect();
    assert!(
        !south_edge.iter().any(|&(_, y)| y >= GRID_HEIGHT),
        "the south edge stops: {south_edge:?}"
    );
}

#[test]
fn neighbors_are_symmetric() {
    // If A touches B, B touches A, on every row parity.
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            for (nx, ny) in hex::neighbors(x, y) {
                assert!(
                    hex::neighbors(nx, ny).any(|(bx, by)| bx == x && by == y),
                    "({nx},{ny}) should touch ({x},{y}) back"
                );
            }
        }
    }
}

#[test]
fn hex_distance_counts_steps_not_lines() {
    assert_eq!(hex::distance(0, 0, 0, 0), 0, "a tile is zero from itself");
    assert_eq!(hex::distance(0, 0, 1, 0), 1, "one step east");
    assert_eq!(hex::distance(0, 0, 0, 1), 1, "one step south");
    assert_eq!(
        hex::distance(0, 0, 1, 1),
        2,
        "odd-r: the far diagonal is two steps"
    );
    assert_eq!(hex::distance(0, 0, 2, 0), 2, "two steps east");
    assert_eq!(hex::distance(0, 0, 0, 2), 2, "two rows south");
    assert_eq!(hex::distance(3, 7, 9, 11), 8, "a longer walk");
}

#[test]
fn a_disc_of_radius_one_is_seven_tiles() {
    let disc = hex::disc(40, 40, 1);
    assert_eq!(disc.len(), 7, "a hex and her six neighbors");
    assert!(disc.contains(&(40, 40)), "the center is in her own disc");
}

#[test]
fn a_disc_of_radius_two_is_nineteen_tiles() {
    assert_eq!(hex::disc(40, 40, 2).len(), 19, "3r^2 + 3r + 1");
}

#[test]
fn a_disc_wraps_without_losing_tiles() {
    // A disc centered on the east edge still holds every tile: the wrap is
    // part of the world, not a hole in it.
    let disc = hex::disc(GRID_WIDTH - 1, 40, 2);
    assert_eq!(disc.len(), 19, "wrapping keeps the disc whole");
    assert!(disc.contains(&(1, 40)), "the far side of the wrap is near");
}

#[test]
fn a_disc_clips_at_the_poles() {
    // Radius 3 holds 37 tiles; rows -3, -2 and -1 (15 tiles) do not exist.
    let disc = hex::disc(40, 0, 3);
    assert_eq!(disc.len(), 22, "the pole clips the three northern rows");
}

#[test]
fn the_same_seed_of_calls_returns_the_same_geometry() {
    let a = hex::disc(10, 10, 3);
    let b = hex::disc(10, 10, 3);
    assert_eq!(a, b, "geometry is pure math, never drawn");
}
