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
