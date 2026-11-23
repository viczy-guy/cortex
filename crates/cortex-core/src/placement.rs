//! Island placement in logical screen coordinates.

pub const POSITIONS: [&str; 5] = ["top", "left", "right", "bottom-left", "bottom-right"];

pub fn is_position(position: &str) -> bool {
    POSITIONS.contains(&position)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn island_origin(
    position: &str,
    screen: Rect,
    work: Rect,
    width: f64,
    height: f64,
) -> (f64, f64) {
    let right = work.x + work.width - width;
    let middle = work.y + (work.height - height) / 2.0;
    let bottom = work.y + work.height - height;
    let (x, y) = match position {
        "left" => (work.x, middle),
        "right" => (right, middle),
        "bottom-left" => (work.x, bottom),
        "bottom-right" => (right, bottom),
        _ => (screen.x + (screen.width - width) / 2.0, screen.y),
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
    fn top_centers_on_the_full_screen() {
        assert_eq!(
            island_origin("top", SCREEN, WORK, 200.0, 100.0),
            (-700.0, 50.0)
        );
    }

    #[test]
    fn left_centers_on_the_left_edge_of_the_work_area() {
        assert_eq!(
            island_origin("left", SCREEN, WORK, 200.0, 100.0),
            (-1180.0, 395.0)
        );
    }

    #[test]
    fn right_centers_on_the_right_edge_of_the_work_area() {
        assert_eq!(
            island_origin("right", SCREEN, WORK, 200.0, 100.0),
            (-230.0, 395.0)
        );
    }

    #[test]
    fn bottom_left_uses_the_lower_left_corner_of_the_work_area() {
        assert_eq!(
            island_origin("bottom-left", SCREEN, WORK, 200.0, 100.0),
            (-1180.0, 710.0)
        );
    }

    #[test]
    fn bottom_right_uses_the_lower_right_corner_of_the_work_area() {
        assert_eq!(
            island_origin("bottom-right", SCREEN, WORK, 200.0, 100.0),
            (-230.0, 710.0)
        );
    }

    #[test]
    fn an_unknown_position_uses_the_top() {
        assert_eq!(
            island_origin("middle", SCREEN, WORK, 200.0, 100.0),
            (-700.0, 50.0)
        );
    }

    #[test]
    fn a_window_taller_than_the_work_area_stays_inside_the_screen() {
        assert_eq!(
            island_origin("bottom-right", SCREEN, WORK, 200.0, 780.0),
            (-230.0, 50.0)
        );
    }
}
