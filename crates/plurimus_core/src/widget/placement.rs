//! Where a widget draws: which camera, which rect, in what order.
//!
//! [`UiArea`] is camera-local - a [`UiArea::Fixed`] rect is offset from the
//! camera's viewport origin and clipped to it, never a screen rect - and
//! [`resolve_area`] is what performs that resolution, with [`local_area`]
//! the way back for a crate holding a screen rect. A propagated
//! [`UiCamera`] names the target camera, [`ComputedUiCamera`] is the one a
//! widget actually draws on once the default has had its say, [`UiOrder`] sorts
//! widgets within a camera, and a [`Propagate<Hidden>`](bevy_app::Propagate) takes a subtree out
//! of both drawing and interaction. Crates that place their own widgets read
//! [`ComputedUiCamera`] and go through [`resolve_area`] instead of repeating
//! the rules.

use bevy_app::{
    App, HierarchyPropagatePlugin, Last, PreUpdate, PropagateSet, propagate_inherited,
    propagate_output, update_removed_limit, update_source,
};
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{Component, Entity, IntoScheduleConfigs, Query, Res};
use ratatui_core::layout::Rect;

use crate::camera::DefaultCamera;
use crate::viewport::CameraSystems;

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

/// Present on every entity a [`Propagate<Hidden>`](bevy_app::Propagate) reaches - the entity
/// carrying it and everything beneath through `ChildOf`: none of it is
/// rendered or interactive while present.
///
/// Hide a subtree by putting `Propagate(Hidden)` on its root and show it by
/// removing that; bevy's propagation lands this marker and takes it back.
/// Resolved twice a frame - in
/// [`CameraSystems::PropagateCameras`](crate::CameraSystems::PropagateCameras),
/// so input routes against it, and in `Last`, so a subtree hidden during the
/// frame is not drawn by it. Read it rather than write it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hidden;

pub(crate) fn install(app: &mut App) {
    app.add_plugins((
        HierarchyPropagatePlugin::<Hidden>::new(PreUpdate),
        HierarchyPropagatePlugin::<UiCamera>::new(PreUpdate),
    ));
    app.configure_sets(
        PreUpdate,
        (
            PropagateSet::<Hidden>::default(),
            PropagateSet::<UiCamera>::default(),
        )
            .in_set(CameraSystems::PropagateCameras),
    );
    app.add_systems(
        PreUpdate,
        resolve_ui_cameras
            .in_set(CameraSystems::PropagateCameras)
            .after(PropagateSet::<UiCamera>::default()),
    );
    app.add_systems(
        Last,
        (
            update_source::<Hidden, (), ChildOf>,
            update_removed_limit::<Hidden, (), ChildOf>,
            propagate_inherited::<Hidden, (), ChildOf>,
            propagate_output::<Hidden, ()>,
        )
            .chain()
            .in_set(PropagateSet::<Hidden>::default()),
    );
}

/// The camera an entity draws on, named by a
/// [`Propagate<UiCamera>`](bevy_app::Propagate) on it or on an ancestor:
/// bevy's propagation lands this on the entity carrying the `Propagate` and
/// on everything beneath it through `ChildOf`, a nearer `Propagate` winning
/// over a farther one, and takes it back when the `Propagate` goes. Absent,
/// the entity draws on the active camera with the lowest order; see
/// [`ComputedUiCamera`] for the resolved answer.
///
/// A bare `UiCamera` on an entity nothing propagates to is read as its own,
/// which is all a leaf needs; one under a `Propagate<UiCamera>` ancestor is
/// overwritten by it.
///
/// Overlay widgets over world-pipeline content need a dedicated camera -
/// a [`TerminalCamera::Docked`](crate::TerminalCamera::Docked) strip or one
/// clearing to `ClearColorConfig::None`; see
/// [`UiWidget`](crate::UiWidget) for why.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiCamera(pub Entity);

/// The camera a widget targets: its [`UiCamera`], else the default camera.
#[must_use]
pub fn resolve_camera(
    explicit: Option<&UiCamera>,
    default_camera: &DefaultCamera,
) -> Option<Entity> {
    explicit.map(|camera| camera.0).or(default_camera.0)
}

/// The camera a widget actually draws on this frame: the [`UiCamera`] that
/// reached it, else the default camera. `None` names no camera at all,
/// which is what a widget has while none is active.
///
/// Resolved once a frame in
/// [`CameraSystems::PropagateCameras`](crate::CameraSystems::PropagateCameras),
/// after the propagation. Read it rather than write it: a widget that must
/// draw somewhere its hierarchy does not put
/// it - a popover following an anchor it is not parented to - says so with
/// a `Propagate<UiCamera>` of its own, which carries the answer to its
/// children too.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComputedUiCamera(pub Option<Entity>);

pub(crate) fn resolve_ui_cameras(
    default_camera: Res<DefaultCamera>,
    mut rows: Query<(Option<&UiCamera>, &mut ComputedUiCamera)>,
) {
    for (named, mut computed) in &mut rows {
        computed.set_if_neq(ComputedUiCamera(resolve_camera(named, &default_camera)));
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
