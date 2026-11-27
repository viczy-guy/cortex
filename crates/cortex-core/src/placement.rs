//! Island placement in logical screen coordinates.

pub const POSITIONS: [&str; 5] = ["top", "left", "right", "bottom-left", "bottom-right"];

pub fn is_position(position: &str) -> bool {
    POSITIONS.contains(&position)
}

/// Where along its edge the island sits (#184), from 0 (the left, or the
/// top) to 1; in the middle by default.
pub const CENTER: f64 = 0.5;
/// Within this many points of the middle, a dragged island snaps to it.
pub const SNAP_POINTS: f64 = 24.0;

pub fn is_offset(offset: f64) -> bool {
    (0.0..=1.0).contains(&offset)
}

/// The top slides sideways and the side edges up and down; the bottom
/// corners stay where they are.
pub fn drags(position: &str) -> bool {
    matches!(position, "top" | "left" | "right")
}

/// Where a drag of `delta` points takes an island `size` points long from
/// `from`, along an edge `length` points long: never past the edge, and
/// snapped to the middle when near it.
pub fn dragged(from: f64, delta: f64, length: f64, size: f64) -> f64 {
    if !(length > 0.0 && from.is_finite() && delta.is_finite() && size.is_finite()) {
        return CENTER;
    }
    let half = (size.max(0.0) / 2.0 / length).min(CENTER);
    let to = (from + delta / length).clamp(half, 1.0 - half);
    // A hair over, for the rounding of `delta / length`.
    if ((to - CENTER) * length).abs() <= SNAP_POINTS + 1e-6 {
        CENTER
    } else {
        to
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn island_width(width: f64, cutout_width: Option<f64>, max_width: f64) -> f64 {
    const CUTOUT_SIDE_WIDTH: f64 = 64.0;
    cutout_width.map_or(width, |cutout| {
        width.max(cutout + 2.0 * CUTOUT_SIDE_WIDTH).min(max_width)
    })
}

/// The island's top-left corner: at its edge, centered on `offset` along it
/// (#184), inside the screen; at the sides, inside the work area too.
pub fn island_origin(
    position: &str,
    screen: Rect,
    work: Rect,
    width: f64,
    height: f64,
    offset: f64,
) -> (f64, f64) {
    let offset = if is_offset(offset) { offset } else { CENTER };
    let right = work.x + work.width - width;
    let along = (work.y + offset * work.height - height / 2.0)
        .min(work.y + work.height - height)
        .max(work.y);
    let bottom = work.y + work.height - height;
    let (x, y) = match position {
        "left" => (work.x, along),
        "right" => (right, along),
        "bottom-left" => (work.x, bottom),
        "bottom-right" => (right, bottom),
        _ => (screen.x + offset * screen.width - width / 2.0, screen.y),
    };
    (
        x.clamp(screen.x, screen.x + (screen.width - width).max(0.0)),
        y.clamp(screen.y, screen.y + (screen.height - height).max(0.0)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: -1200.0,
        y: 50.0,
        width: 1200.0,
        height: 800.0,
    };
    const WORK: Rect = Rect {
        x: -1180.0,
        y: 80.0,
        width: 1150.0,
        height: 730.0,
    };

    #[test]
    fn a_cutout_leaves_room_on_both_sides_within_the_maximum_width() {
        assert_eq!(island_width(90.0, Some(180.0), 460.0), 308.0);
        assert_eq!(island_width(400.0, Some(180.0), 460.0), 400.0);
        assert_eq!(island_width(90.0, Some(350.0), 460.0), 460.0);
        assert_eq!(island_width(90.0, None, 460.0), 90.0);
    }

    #[test]
    fn top_centers_on_the_full_screen() {
        assert_eq!(
            island_origin("top", SCREEN, WORK, 200.0, 100.0, CENTER),
            (-700.0, 50.0)
        );
    }

    #[test]
    fn left_centers_on_the_left_edge_of_the_work_area() {
        assert_eq!(
            island_origin("left", SCREEN, WORK, 200.0, 100.0, CENTER),
            (-1180.0, 395.0)
        );
    }

    #[test]
    fn right_centers_on_the_right_edge_of_the_work_area() {
        assert_eq!(
            island_origin("right", SCREEN, WORK, 200.0, 100.0, CENTER),
            (-230.0, 395.0)
        );
    }

    #[test]
    fn bottom_left_uses_the_lower_left_corner_of_the_work_area() {
        assert_eq!(
            island_origin("bottom-left", SCREEN, WORK, 200.0, 100.0, CENTER),
            (-1180.0, 710.0)
        );
    }

    #[test]
    fn bottom_right_uses_the_lower_right_corner_of_the_work_area() {
        assert_eq!(
            island_origin("bottom-right", SCREEN, WORK, 200.0, 100.0, CENTER),
            (-230.0, 710.0)
        );
    }

    #[test]
    fn an_unknown_position_uses_the_top() {
        assert_eq!(
            island_origin("middle", SCREEN, WORK, 200.0, 100.0, CENTER),
            (-700.0, 50.0)
        );
    }

    #[test]
    fn a_dragged_island_sits_along_its_edge_and_stays_inside() {
        // Top: centered on the offset, along the whole screen.
        assert_eq!(
            island_origin("top", SCREEN, WORK, 200.0, 100.0, 0.25),
            (-1000.0, 50.0)
        );
        assert_eq!(
            island_origin("top", SCREEN, WORK, 200.0, 100.0, 0.0),
            (-1200.0, 50.0)
        );
        // Sides: along the work area, never under the menu bar or the Dock.
        assert_eq!(
            island_origin("left", SCREEN, WORK, 200.0, 100.0, 0.25),
            (-1180.0, 212.5)
        );
        assert_eq!(
            island_origin("right", SCREEN, WORK, 200.0, 100.0, 1.0),
            (-230.0, 710.0)
        );
        // The bottom corners ignore it; a broken one is the middle.
        assert_eq!(
            island_origin("bottom-left", SCREEN, WORK, 200.0, 100.0, 0.1),
            (-1180.0, 710.0)
        );
        assert_eq!(
            island_origin("top", SCREEN, WORK, 200.0, 100.0, f64::NAN),
            (-700.0, 50.0)
        );
    }

    #[test]
    fn a_drag_follows_the_pointer_snaps_to_the_middle_and_keeps_the_island_in() {
        // 1200 points of edge: 120 points is a tenth.
        assert_eq!(dragged(CENTER, -360.0, 1200.0, 200.0), 0.2);
        // Within 24 points of the middle, it snaps.
        assert_eq!(dragged(0.2, 340.0, 1200.0, 200.0), CENTER);
        assert_eq!(dragged(CENTER, 24.0, 1200.0, 200.0), CENTER);
        assert!(dragged(CENTER, 30.0, 1200.0, 200.0) > CENTER);
        // Never past the edge: half the island stays in.
        assert_eq!(dragged(CENTER, -5000.0, 1200.0, 200.0), 100.0 / 1200.0);
        assert_eq!(dragged(CENTER, 5000.0, 1200.0, 200.0), 1.0 - 100.0 / 1200.0);
        // Nothing to measure: the middle.
        assert_eq!(dragged(0.2, f64::NAN, 1200.0, 200.0), CENTER);
        assert_eq!(dragged(0.2, 10.0, 0.0, 200.0), CENTER);
        assert!(drags("top") && drags("left") && drags("right"));
        assert!(!drags("bottom-left") && !drags("bottom-right"));
    }

    #[test]
    fn a_window_taller_than_the_work_area_stays_inside_the_screen() {
        assert_eq!(
            island_origin("bottom-right", SCREEN, WORK, 200.0, 780.0, CENTER),
            (-230.0, 50.0)
        );
    }
}
