//! Belief: what a tile's people think the god is like (design, Belief model).
//!
//! Six axes, each in [0, 1]. Tiles hold belief; cultures read the
//! population-weighted mean (Milestone 3 turns that into doctrine).
//!
//! Updates arrive at the end of a power's 5-tick outcome window, scaled by the
//! outcome for that tile (design, Witness rule).

/// The six axes, in the design's order.
pub const AXES: usize = 6;
pub const WRATHFUL: usize = 0;
pub const BENEVOLENT: usize = 1;
pub const CAPRICIOUS: usize = 2;
pub const PROTECTIVE: usize = 3;
pub const JEALOUS: usize = 4;
pub const ABSENT: usize = 5;

/// What a people believe about the god. Axis values live in [0, 1].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Belief {
    pub wrathful: f32,
    pub benevolent: f32,
    pub capricious: f32,
    pub protective: f32,
    pub jealous: f32,
    pub absent: f32,
}

impl Belief {
    /// People start unsure, slightly inclined to think the god is absent.
    pub const START: Belief = Belief {
        wrathful: 0.3,
        benevolent: 0.3,
        capricious: 0.3,
        protective: 0.3,
        jealous: 0.2,
        absent: 0.35,
    };

    /// Flattens to the tile field's six-axis layout.
    pub fn to_axes(self) -> [f32; AXES] {
        [
            self.wrathful,
            self.benevolent,
            self.capricious,
            self.protective,
            self.jealous,
            self.absent,
        ]
    }

    /// Reads six axes back out of the tile field.
    pub fn from_axes(axes: &[f32]) -> Self {
        Self {
            wrathful: axes[WRATHFUL],
            benevolent: axes[BENEVOLENT],
            capricious: axes[CAPRICIOUS],
            protective: axes[PROTECTIVE],
            jealous: axes[JEALOUS],
            absent: axes[ABSENT],
        }
    }

    /// Folds one witness's outcome into their belief and returns the change.
    ///
    /// `score` is the witness rule's outcome, already clamped to [-1, 1]. The
    /// dead zone is applied by the caller: |score| < 0.05 is "no effect".
    #[must_use]
    pub fn witness(mut self, score: f32) -> Self {
        if score < 0.0 {
            // A negative outcome raises wrathful and capricious (design).
            self.wrathful = (self.wrathful + 0.18 * score.abs()).min(1.0);
            self.capricious = (self.capricious + 0.08 * score.abs()).min(1.0);
            self.absent = (self.absent - 0.02).max(0.0);
        } else {
            // A positive outcome raises benevolent and protective (design).
            self.benevolent = (self.benevolent + 0.18 * score).min(1.0);
            self.protective = (self.protective + 0.08 * score).min(1.0);
            self.absent = (self.absent - 0.02).max(0.0);
        }
        self
    }

    /// Witnessing with no effect lowers `absent` (design, Witness rule).
    #[must_use]
    pub fn witness_silence(mut self) -> Self {
        self.absent = (self.absent - 0.01).max(0.0);
        self
    }

    /// How this belief looks on the belief overlay: the dominant axis's color,
    /// drained toward grey where the axis is weak (design, overlays).
    pub fn color(&self) -> [u8; 3] {
        let axes = self.to_axes();
        let (axis, value) = axes
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or((ABSENT, 0.5), |(axis, value)| (axis, *value));
        let hue = match axis {
            WRATHFUL => [196, 62, 48],
            BENEVOLENT => [217, 168, 69],
            CAPRICIOUS => [128, 86, 158],
            PROTECTIVE => [92, 142, 92],
            JEALOUS => [64, 130, 132],
            _ => [104, 100, 94],
        };
        let strength = ((value - 0.10) / 0.90).clamp(0.0, 1.0);
        let grey = 44.0;
        [
            (grey + (hue[0] as f32 - grey) * strength) as u8,
            (grey + (hue[1] as f32 - grey) * strength) as u8,
            (grey + (hue[2] as f32 - grey) * strength) as u8,
        ]
    }

    /// The axis a people leans on, for choosing what a verse says.
    pub fn dominant(&self) -> usize {
        self.to_axes()
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(ABSENT, |(axis, _)| axis)
    }
}

impl Default for Belief {
    fn default() -> Self {
        Self::START
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_harm_strengthens_wrath_and_a_gift_strengthens_mercy() {
        let hurt = Belief::START.witness(-0.8);
        assert!(hurt.wrathful > Belief::START.wrathful);
        assert!(hurt.capricious > Belief::START.capricious);

        let gift = Belief::START.witness(0.8);
        assert!(gift.benevolent > Belief::START.benevolent);
        assert!(gift.protective > Belief::START.protective);
    }

    #[test]
    fn belief_round_trips_through_the_tile_field() {
        let belief = Belief::START.witness(0.4);
        assert_eq!(Belief::from_axes(&belief.to_axes()), belief);
    }

    #[test]
    fn the_overlay_color_follows_the_dominant_axis() {
        let mut wrathful = Belief::START;
        wrathful.wrathful = 0.95;
        let mut benevolent = Belief::START;
        benevolent.benevolent = 0.95;
        assert_ne!(wrathful.color(), benevolent.color());
        assert!(wrathful.color()[0] > wrathful.color()[2], "wrath reads red");
    }

    #[test]
    fn axes_stay_inside_the_unit_interval() {
        let mut belief = Belief::START;
        for _ in 0..100 {
            belief = belief.witness(-1.0);
        }
        for axis in belief.to_axes() {
            assert!((0.0..=1.0).contains(&axis));
        }
    }
}
