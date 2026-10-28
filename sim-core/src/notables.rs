//! Notables and legends (design, Individual budgets).
//!
//! A witness is a person: generated name, a people, a tile, and a birth year.
//! Witnesses that no prophet, heretic or hero role claims within 20 ticks are
//! demoted back into the cohort, leaving a legend record.

use crate::culture::Favor;

/// Hard cap on individuals of every kind.
pub const GLOBAL_CAP: usize = 2_000;
/// Witnesses in total.
pub const WITNESS_BUDGET: usize = 300;
/// Notables of the Chosen people.
pub const CHOSEN_BUDGET: usize = 200;
/// Notables of any single `Other` people.
pub const OTHER_BUDGET: usize = 15;
/// Ticks a witness has to be elevated before it becomes an ordinary person.
pub const WITNESS_TENURE: u64 = 20;

/// What a notable is. Prophets arrive with belief (Milestone 3); heretics,
/// heroes and rulers arrive with war and kingdoms.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Role {
    Witness,
    Prophet,
}

/// A named individual who saw something happen.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Notable {
    pub id: u32,
    pub culture_id: u16,
    pub name: String,
    pub tile: u32,
    pub born_tick: u64,
    pub role: Role,
    pub salience: f32,
    /// How the person looks, sampled from their tile when promoted
    /// (design, Physical adaptation).
    pub skin: f32,
    pub stature: f32,
}

impl Notable {
    /// Scripture mentions plus rank; the eviction order reads it (design).
    pub fn salience(&self, scripture_mentions: u32) -> f32 {
        self.salience + scripture_mentions as f32
    }
}

/// A compact record of someone the scripture still cites.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Legend {
    pub id: u32,
    pub culture_id: u16,
    pub name: String,
    pub born_tick: u64,
    pub died_tick: u64,
    pub role: Role,
}

/// Which budget a person draws from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Budget {
    Witness,
    Chosen,
    Other,
}

/// The budget a person of `favor` draws from.
pub fn budget_for(favor: Favor) -> Budget {
    match favor {
        Favor::Chosen => Budget::Chosen,
        Favor::Other => Budget::Other,
    }
}

/// Cap for a budget.
pub fn cap(budget: Budget) -> usize {
    match budget {
        Budget::Witness => WITNESS_BUDGET,
        Budget::Chosen => CHOSEN_BUDGET,
        Budget::Other => OTHER_BUDGET,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_match_the_design_table() {
        assert_eq!(cap(Budget::Witness), 300);
        assert_eq!(cap(Budget::Chosen), 200);
        assert_eq!(cap(Budget::Other), 15);
        assert_eq!(GLOBAL_CAP, 2_000);
        assert_eq!(WITNESS_TENURE, 20);
    }
}
