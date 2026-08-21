//! Where a widget draws: which camera, which rect, in what order.
//!
//! [`UiArea`] is camera-local - a [`UiArea::Fixed`] rect is offset from the
//! camera's viewport origin and clipped to it, never a screen rect - and
//! [`resolve_area`] is what performs that resolution. [`UiCamera`] names the
//! target camera and falls back to the default one when absent, [`UiOrder`]
//! sorts widgets within a camera, and [`UiHidden`] takes a widget out of both
//! drawing and interaction. Crates that place their own widgets go through
//! [`resolve_camera`] and [`resolve_area`] instead of repeating the rules.

use bevy_ecs::prelude::{Component, Entity};
use ratatui_core::layout::Rect;

use crate::camera::DefaultCamera;

/// Where a widget renders, in camera-local cells.
///
/// Closed: a widget either takes a rect of its own or fills its camera.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiArea {
    /// The camera's whole viewport.
    #[default]
    Fill,
    /// A fixed rectangle, clipped to the viewport.
    Fixed(Rect),
}

/// Z-order within a camera; higher renders later, on top.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiOrder(pub i32);

impl UiOrder {
    /// Reserved band for popups and other overlays, above app content.
    pub const OVERLAY: Self = Self(i32::MAX / 2);
}

/// Hides the widget: it is neither rendered nor interactive while present.
#[derive(Component, Debug, Clone, Copy)]
pub struct UiHidden;

/// Explicit target camera (main-world entity). Absent: the active camera
/// with the lowest order.
///
/// Overlay widgets over world-pipeline content need a dedicated camera -
/// a [`Viewport::Docked`](crate::Viewport::Docked) strip or one with
/// [`Background::Transparent`](crate::Background::Transparent); see
/// [`UiWidget`](crate::UiWidget) for why.
#[derive(Component, Debug, Clone, Copy)]
pub struct UiCamera(pub Entity);

/// The camera a widget targets: its explicit [`UiCamera`], else the
/// default camera.
#[must_use]
pub fn resolve_camera(
    explicit: Option<&UiCamera>,
    default_camera: &DefaultCamera,
) -> Option<Entity> {
    explicit.map(|camera| camera.0).or(default_camera.0)
}

/// A screen rect expressed camera-locally, for storing in a
/// [`UiArea::Fixed`]: `rect` offset back by `viewport`'s origin.
///
/// The inverse of the offset [`resolve_area`] applies, and only of that -
/// the clip it also applies discards what fell outside the viewport, which
/// no inverse restores. A rect originating left of or above the viewport
/// saturates to its edge rather than wrapping.
#[must_use]
pub const fn local_area(rect: Rect, viewport: Rect) -> Rect {
    Rect::new(
        rect.x.saturating_sub(viewport.x),
        rect.y.saturating_sub(viewport.y),
        rect.width,
        rect.height,
    )
}

/// A widget's screen rect: its [`UiArea`] resolved against `viewport`.
#[must_use]
pub fn resolve_area(area: UiArea, viewport: Rect) -> Rect {
    match area {
        UiArea::Fill => viewport,
        UiArea::Fixed(rect) => Rect::new(
            viewport.x.saturating_add(rect.x),
            viewport.y.saturating_add(rect.y),
            rect.width,
            rect.height,
        )
        .intersection(viewport),
    }
}

#[cfg(test)]
mod tests {
    use super::{UiArea, local_area, resolve_area};
    use ratatui_core::layout::Rect;

    const VIEWPORT: Rect = Rect::new(4, 2, 10, 6);

    #[test]
    fn a_local_rect_survives_the_round_trip() {
        let local = Rect::new(2, 1, 3, 2);

        let screen = resolve_area(UiArea::Fixed(local), VIEWPORT);

        assert_eq!(screen, Rect::new(6, 3, 3, 2));
        assert_eq!(local_area(screen, VIEWPORT), local);
    }

    // The clip has no inverse, so only what the viewport kept comes back -
    // which is the rect a caller has in hand to store.
    #[test]
    fn a_rect_the_viewport_clipped_localizes_to_what_survived() {
        let screen = resolve_area(UiArea::Fixed(Rect::new(8, 5, 6, 4)), VIEWPORT);

        assert_eq!(screen, Rect::new(12, 7, 2, 1));
        assert_eq!(local_area(screen, VIEWPORT), Rect::new(8, 5, 2, 1));
    }

    #[test]
    fn a_rect_outside_the_origin_saturates() {
        assert_eq!(
            local_area(Rect::new(1, 0, 2, 2), VIEWPORT),
            Rect::new(0, 0, 2, 2)
        );
    }
}
