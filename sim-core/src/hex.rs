//! The hex grid (design, Grid; user direction 2026-10-27).
//!
//! The world is odd-r flat-top hexes: odd rows sit half a tile right of even
//! rows. Storage is unchanged — one flat `Vec` per field, indexed
//! `y * GRID_WIDTH + x` — so every per-tile system keeps its layout and only
//! the neighbor math and the distance metric live here.
//!
//! East-west wraps (design, Grid); north-south stops at the poles.

use crate::world::{GRID_HEIGHT, GRID_WIDTH};

/// The six neighbors of a tile, east-west wrapping (design, Grid).
///
/// Odd-r flat-top: on even rows the northern and southern neighbors share
/// this tile's column and the one west; on odd rows they sit one column east.
pub fn neighbors(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
    let east = i32::from(y % 2 == 1);
    let candidates: [(i32, i32); 6] = [
        (1, 0),
        (-1, 0),
        (east, -1),
        (east - 1, -1),
        (east, 1),
        (east - 1, 1),
    ];
    let (x, y) = (x as i32, y as i32);
    candidates.into_iter().filter_map(move |(dx, dy)| {
        let ny = y + dy;
        if ny < 0 || ny >= GRID_HEIGHT as i32 {
            return None;
        }
        let nx = (x + dx).rem_euclid(GRID_WIDTH as i32);
        Some((nx as usize, ny as usize))
    })
}

/// Offset (odd-r) to cube coordinates, all integers: the odd-row shift is a
/// half step, so the subtraction keeps whole numbers.
fn to_cube(x: i32, y: i32) -> (i32, i32, i32) {
    let cx = x - (y - (y & 1)) / 2;
    (cx, -cx - y, y)
}

/// Steps between two tiles, the way a person walks (design, Grid).
///
/// Cube coordinates make every step cost one, so the walk is the Manhattan
/// distance in cube space: `(|dx| + |dy| + |dz|) / 2`.
pub fn distance(x0: usize, y0: usize, x1: usize, y1: usize) -> u32 {
    // Walk the shorter way around the wrap: the world is a cylinder.
    let dx = signed_delta(x0, x1);
    let (ax, ay, az) = to_cube(x0 as i32, y0 as i32);
    let (bx, by, bz) = to_cube(x0 as i32 + dx, y1 as i32);
    let steps = (bx - ax).abs() + (by - ay).abs() + (bz - az).abs();
    (steps / 2) as u32
}

/// The east-west difference between two columns, taking the short way around.
fn signed_delta(x0: usize, x1: usize) -> i32 {
    let straight = x1 as i32 - x0 as i32;
    let wrapped = straight - GRID_WIDTH as i32 * straight.signum();
    if straight.abs() <= wrapped.abs() {
        straight
    } else {
        wrapped
    }
}

/// Every tile within `radius` steps of `(x, y)`: a true hex disc, the shape
/// blessing brushes, catchments and the beacon's light want (user direction
/// 2026-10-27: hexes make circles easy).
pub fn disc(x: usize, y: usize, radius: u32) -> Vec<(usize, usize)> {
    let r = radius as i32;
    let mut out = Vec::with_capacity((3 * r * r + 3 * r + 1) as usize);
    for dy in -r..=r {
        for dx in -r..=r {
            let ny = y as i32 + dy;
            if ny < 0 || ny >= GRID_HEIGHT as i32 {
                continue;
            }
            let nx = (x as i32 + dx).rem_euclid(GRID_WIDTH as i32) as usize;
            if distance(x, y, nx, ny as usize) <= radius {
                out.push((nx, ny as usize));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_interior_tile_touches_six() {
        assert_eq!(neighbors(40, 40).count(), 6);
    }

    #[test]
    fn the_east_edge_wraps() {
        let near: Vec<(usize, usize)> = neighbors(GRID_WIDTH - 1, 40).collect();
        assert!(near.contains(&(0, 40)));
    }

    #[test]
    fn a_disc_of_one_is_seven() {
        assert_eq!(disc(40, 40, 1).len(), 7);
    }

    #[test]
    fn a_disc_of_two_is_nineteen() {
        assert_eq!(disc(40, 40, 2).len(), 19);
    }

    #[test]
    fn distance_walks_the_short_way_around() {
        assert_eq!(distance(GRID_WIDTH - 1, 40, 0, 40), 1);
    }
}
