//! Where a widget draws: which camera, which rect, in what order.
//!
//! [`UiArea`] is camera-local - a [`UiArea::Fixed`] rect is offset from the
//! camera's viewport origin and clipped to it, never a screen rect - and
//! [`resolve_area`] is what performs that resolution, with [`local_area`]
//! the way back for a crate holding a screen rect. [`UiCamera`] names the
//! target camera, [`ComputedUiCamera`] is the one a widget actually draws on
//! once the hierarchy and the default have had their say, [`UiOrder`] sorts
//! widgets within a camera, and [`UiHidden`] takes a widget out of both
//! drawing and interaction. Crates that place their own widgets read
//! [`ComputedUiCamera`] and go through [`resolve_area`] instead of repeating
//! the rules.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{Component, Entity, Query, Res};
use ratatui_core::layout::Rect;

use crate::camera::DefaultCamera;

/// Where a widget renders, in camera-local cells.
///
/// Requires [`ComputedUiCamera`], because local to what is half of where:
/// an entity carrying one of these resolves against a camera whether or
/// not it draws anything itself.
///
/// Closed: a widget either takes a rect of its own or fills its camera.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[require(ComputedUiCamera)]
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
///
/// The leaf rule [`ComputedUiCamera`] applies once an ancestor search has
/// found whichever [`UiCamera`] governs; a widget reading which camera it
/// draws on wants that component rather than this.
#[must_use]
pub fn resolve_camera(
    explicit: Option<&UiCamera>,
    default_camera: &DefaultCamera,
) -> Option<Entity> {
    explicit.map(|camera| camera.0).or(default_camera.0)
}

/// The camera a widget actually draws on this frame: its own [`UiCamera`],
/// else the nearest ancestor's through `ChildOf`, else the default camera.
/// `None` names no camera at all, which is what a widget has while none is
/// active.
///
/// Resolved every frame in
/// [`CameraSystems::PropagateCameras`](crate::CameraSystems::PropagateCameras),
/// so a widget follows a parent that changes camera and a child needs no
/// [`UiCamera`] of its own to sit on its parent's - forgetting one is
/// otherwise a silent misplacement onto the default camera.
///
/// Read it rather than write it: the search reads ancestors' [`UiCamera`]
/// components, so a widget that must draw somewhere its hierarchy does not
/// put it - a popover following an anchor it is not parented to - says so
/// by holding a [`UiCamera`], which is what carries the answer to its own
/// children too.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComputedUiCamera(pub Option<Entity>);

pub(crate) fn propagate_cameras(
    default_camera: Res<DefaultCamera>,
    explicit: Query<&UiCamera>,
    parents: Query<&ChildOf>,
    mut widgets: Query<(Entity, &mut ComputedUiCamera)>,
) {
    for (entity, mut computed) in &mut widgets {
        let named = std::iter::once(entity)
            .chain(parents.iter_ancestors(entity))
            .find_map(|entity| explicit.get(entity).ok().map(|camera| camera.0));
        computed.set_if_neq(ComputedUiCamera(named.or(default_camera.0)));
    }
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
