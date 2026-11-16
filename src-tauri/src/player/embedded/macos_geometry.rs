//! Page CSS pixels are AppKit points (Vela does not zoom its page). Only
//! backing storage scales; multiplying the view frame by Retina scale would
//! move/size it twice. AppKit converts this local frame to the content view.

use super::session::Rect;

#[derive(Debug, PartialEq)]
pub(super) struct Placement {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) backing_width: i32,
    pub(super) backing_height: i32,
}

pub(super) fn placement(page: (f64, f64), rect: Rect, flipped: bool, scale: f64) -> Placement {
    let left = f64::from(rect.x).clamp(0.0, page.0);
    let top = f64::from(rect.y).clamp(0.0, page.1);
    let right = f64::from(rect.x.saturating_add(rect.width)).clamp(left, page.0);
    let bottom = f64::from(rect.y.saturating_add(rect.height)).clamp(top, page.1);
    let width = right - left;
    let height = bottom - top;
    Placement {
        x: left,
        y: if flipped { top } else { page.1 - bottom },
        width,
        height,
        backing_width: (width * scale).round() as i32,
        backing_height: (height * scale).round() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_placement_keeps_points_and_scales_only_the_backing_pixels() {
        for scale in [1.0, 2.0] {
            for flipped in [false, true] {
                let frame = placement(
                    (800.0, 600.0),
                    Rect {
                        x: 20,
                        y: 30,
                        width: 400,
                        height: 200,
                    },
                    flipped,
                    scale,
                );
                assert_eq!(
                    frame,
                    Placement {
                        x: 20.0,
                        y: if flipped { 30.0 } else { 370.0 },
                        width: 400.0,
                        height: 200.0,
                        backing_width: (400.0 * scale) as i32,
                        backing_height: (200.0 * scale) as i32,
                    }
                );
            }
        }
    }

    #[test]
    fn macos_placement_clips_to_the_page_including_empty_rectangles() {
        let frame = placement(
            (800.0, 600.0),
            Rect {
                x: -20,
                y: 500,
                width: 900,
                height: 300,
            },
            false,
            2.0,
        );
        assert_eq!(
            frame,
            Placement {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 100.0,
                backing_width: 1600,
                backing_height: 200
            }
        );
        let frame = placement(
            (800.0, 600.0),
            Rect {
                x: i32::MAX,
                y: i32::MAX,
                width: i32::MAX,
                height: i32::MAX,
            },
            true,
            1.0,
        );
        assert_eq!((frame.width, frame.height), (0.0, 0.0));
    }
}
