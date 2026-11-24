//! Where the lively mascot looks (#145): the cursor's direction from the
//! island, in a few steps, so the island hears only when the direction
//! changes and nothing at all while the mouse rests.

use serde::Serialize;

/// Steps on each side of straight ahead: four across, three up and down.
pub const STEPS_X: i8 = 4;
pub const STEPS_Y: i8 = 3;
/// Logical points at which the eyes are about three quarters of the way
/// across (`tanh`): far enough that a cursor anywhere on the screen still
/// reads as a direction, near enough that a move beside the island shows.
const SPREAD_X: f64 = 260.0;
const SPREAD_Y: f64 = 200.0;
/// Closer than this, in logical points from the island's center, the mascot
/// is curious.
const NEAR: f64 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Gaze {
    /// From `-STEPS_X` (left) to `STEPS_X` (right).
    pub x: i8,
    /// From `-STEPS_Y` (up) to `STEPS_Y` (down).
    pub y: i8,
    pub near: bool,
}

impl Gaze {
    pub const AHEAD: Gaze = Gaze {
        x: 0,
        y: 0,
        near: false,
    };
}

/// The direction of `cursor` seen from `center`, both in physical pixels;
/// `scale` turns pixels into logical points. Anything not finite looks ahead.
pub fn gaze(cursor: (f64, f64), center: (f64, f64), scale: f64) -> Gaze {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let dx = (cursor.0 - center.0) / scale;
    let dy = (cursor.1 - center.1) / scale;
    if !dx.is_finite() || !dy.is_finite() {
        return Gaze::AHEAD;
    }
    // tanh keeps the result in -1..=1, so the cast cannot overflow.
    let step =
        |d: f64, spread: f64, steps: i8| ((d / spread).tanh() * f64::from(steps)).round() as i8;
    Gaze {
        x: step(dx, SPREAD_X, STEPS_X),
        y: step(dy, SPREAD_Y, STEPS_Y),
        near: dx.hypot(dy) < NEAR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CENTER: (f64, f64) = (1000.0, 20.0);

    #[test]
    fn a_cursor_on_the_island_looks_ahead_and_is_near() {
        assert_eq!(
            gaze(CENTER, CENTER, 1.0),
            Gaze {
                x: 0,
                y: 0,
                near: true
            }
        );
    }

    #[test]
    fn far_away_reaches_the_last_step_and_never_beyond() {
        let right = gaze((1_000_000.0, 20.0), CENTER, 1.0);
        assert_eq!((right.x, right.y, right.near), (STEPS_X, 0, false));
        let left_down = gaze((-1_000_000.0, 1_000_000.0), CENTER, 1.0);
        assert_eq!((left_down.x, left_down.y), (-STEPS_X, STEPS_Y));
        let up = gaze((1000.0, -1_000_000.0), CENTER, 1.0);
        assert_eq!(up.y, -STEPS_Y);
    }

    #[test]
    fn a_small_move_beside_the_island_changes_the_step() {
        let a = gaze((1060.0, 20.0), CENTER, 1.0);
        let b = gaze((1160.0, 20.0), CENTER, 1.0);
        assert!(a.x >= 1 && b.x > a.x, "{a:?} {b:?}");
        assert!(a.near && !b.near);
    }

    #[test]
    fn the_scale_turns_pixels_into_points() {
        // 200 px at 2x are 100 points: near; at 1x they are 200 points: not.
        assert!(gaze((1200.0, 20.0), CENTER, 2.0).near);
        assert!(!gaze((1200.0, 20.0), CENTER, 1.0).near);
        assert_eq!(
            gaze((1200.0, 20.0), CENTER, 0.0),
            gaze((1200.0, 20.0), CENTER, 1.0)
        );
    }

    #[test]
    fn anything_not_finite_looks_ahead() {
        assert_eq!(gaze((f64::NAN, 0.0), CENTER, 1.0), Gaze::AHEAD);
        assert_eq!(gaze((f64::INFINITY, 0.0), CENTER, 1.0), Gaze::AHEAD);
        assert_eq!(gaze((0.0, 0.0), (f64::NAN, 0.0), f64::NAN), Gaze::AHEAD);
    }
}
