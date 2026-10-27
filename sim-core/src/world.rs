//! World state and the fixed tick pipeline.
//!
//! One tick is one sim-year in every era (D24). Systems run in a fixed order:
//! Climate, Population, Migration, Drift/Blend, Traits, Belief, Polity, Tech,
//! War (design, Architecture). Each system is a single `tick(world, rng)`
//! function so it can be tested on its own.

/// The simulation world: every mutable field the sim owns.
///
/// Rendering never writes here; the sim never reads rendering state.
pub struct World {
    /// World seed. Same seed + same actions => same history (design, Determinism).
    seed: u64,
    /// Sim-year counter. One tick is one sim-year (D24).
    tick: u64,
    /// Bumped by every tick, applied action and load (R6: repaint when dirty).
    revision: u64,
}

impl World {
    /// Creates an empty world from `seed`.
    ///
    /// Generation (terrain, climate, biomes, Eden) lands here in Milestone 1.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            tick: 0,
            revision: 1,
        }
    }

    /// The world seed this history came from.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Current sim-year.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Revision counter; the UI repaints the map only when it changes (R6).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Advances the simulation by `years` whole sim-years.
    ///
    /// Never returns mid-tick: player actions apply at boundaries only (D9), so
    /// replaying the same seed plus the same action log gives the same history.
    pub fn step(&mut self, years: u32) {
        for _ in 0..years {
            self.tick += 1;
            self.tick_systems();
            self.revision += 1;
        }
    }

    /// Runs every system once, in the fixed order.
    ///
    /// Systems arrive milestone by milestone (design, Milestones); the order is
    /// fixed from day one so replays stay comparable across milestones.
    fn tick_systems(&mut self) {
        // Climate (rare) -> Population -> Migration -> Drift/Blend -> Traits
        //   -> Belief -> Polity -> Tech -> War
        // Every 10 ticks: culture aggregates. Every 25 ticks: split + schism checks.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_advances_one_sim_year_per_tick() {
        let mut world = World::new(42);
        assert_eq!(world.tick(), 0);

        let before = world.revision();
        world.step(10);

        assert_eq!(world.tick(), 10, "one tick is one sim-year (D24)");
        assert_eq!(
            world.revision(),
            before + 10,
            "revision bumps once per tick (R6)"
        );
    }

    #[test]
    fn step_zero_years_changes_nothing() {
        let mut world = World::new(7);
        world.step(0);
        assert_eq!(world.tick(), 0);
        assert_eq!(world.revision(), 1);
    }

    #[test]
    fn same_seed_starts_same_history() {
        assert_eq!(World::new(99).seed(), World::new(99).seed());
    }
}
