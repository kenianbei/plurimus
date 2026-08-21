//! Anchored placement: popovers position themselves against another widget's
//! resolved area each frame.
//!
//! A popover names an anchor, a preferred side and an alignment, and is
//! placed against wherever that anchor ended up this frame - so it follows a
//! moving anchor without anything having to notify it. When the preferred
//! side has no room the popover mirrors to the opposite one and, failing
//! that, is clamped into the viewport, which is why a popover near an edge
//! stays wholly visible instead of being cut off.

mod rect;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::{Commands, Component, Entity, Has, Query, Without};
use plurimus_core::ratatui_core::layout::{Rect, Size};

use plurimus_core::{
    CameraViewports, ComputedUiCamera, UiArea, UiCamera, UiHidden, UiOrder, local_area,
};
use plurimus_ui::ComputedWidgetArea;

use rect::popover_rect;

/// Which side of the anchor the popover opens on; mirrored to the
/// opposite side when it would overflow the camera viewport.
///
/// Closed: a rect has four sides, which is what makes [`Self::mirror`]
/// total.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PopoverSide {
    /// Above the anchor.
    Top,
    /// Below the anchor.
    #[default]
    Bottom,
    /// Left of the anchor.
    Left,
    /// Right of the anchor.
    Right,
}

impl PopoverSide {
    /// The opposite side, used when the preferred side overflows.
    #[must_use]
    pub const fn mirror(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

/// Alignment along the anchor edge the popover attaches to.
///
/// Closed: start, center and end span the edge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PopoverAlign {
    /// Leading edges aligned.
    #[default]
    Start,
    /// Centered on the anchor.
    Center,
    /// Trailing edges aligned.
    End,
}

/// Places the widget against `anchor`'s resolved area every frame,
/// overwriting its [`UiArea`] and its [`UiCamera`].
///
/// The camera is the anchor's, which is what lets a popover follow
/// something it is not parented to. It is written as a real [`UiCamera`]
/// rather than a resolved value, so a popover's own children inherit it the
/// way they inherit any camera - parent them to the popover and nothing
/// else is needed.
///
/// The anchor must not itself be a popover.
#[derive(Component, Debug, Clone, Copy)]
#[require(UiOrder = UiOrder::OVERLAY)]
#[non_exhaustive]
pub struct Popover {
    /// The widget this popover attaches to.
    pub anchor: Entity,
    /// Preferred side of the anchor.
    pub side: PopoverSide,
    /// Alignment along that side.
    pub align: PopoverAlign,
    /// Popover size in cells.
    pub size: Size,
}

impl Popover {
    /// A popover of `size` cells below `anchor`, leading edges aligned -
    /// the two [`Default`] placements.
    #[must_use]
    pub const fn new(anchor: Entity, size: Size) -> Self {
        Self {
            anchor,
            side: PopoverSide::Bottom,
            align: PopoverAlign::Start,
            size,
        }
    }
}

/// Takes each popover onto its anchor's camera.
///
/// Separate from [`place_popovers`] because the rect needs the anchor's
/// resolved area and so must wait for `UiSystems::Areas`, while the camera
/// needs only the anchor's own and every reader downstream wants it settled
/// before then. Writing the resolved value serves this frame; writing a real
/// [`UiCamera`] beside it is what reaches the popover's own children.
pub(crate) fn adopt_anchor_cameras(
    anchors: Query<&ComputedUiCamera, Without<Popover>>,
    mut popovers: Query<(Entity, &Popover, &mut ComputedUiCamera, Option<&UiCamera>)>,
    mut commands: Commands,
) {
    for (entity, popover, mut target, own_camera) in &mut popovers {
        let Ok(anchor_camera) = anchors.get(popover.anchor) else {
            continue;
        };
        target.set_if_neq(*anchor_camera);
        if let Some(camera) = anchor_camera.0
            && own_camera.map(|own| own.0) != Some(camera)
        {
            commands.entity(entity).insert(UiCamera(camera));
        }
    }
}

pub(crate) fn place_popovers(
    cameras: CameraViewports,
    anchors: Query<(&ComputedWidgetArea, &ComputedUiCamera), Without<Popover>>,
    mut popovers: Query<(
        &Popover,
        &mut UiArea,
        &mut ComputedWidgetArea,
        Has<UiHidden>,
    )>,
) {
    for (popover, mut area, mut computed, hidden) in &mut popovers {
        let Ok((anchor_area, anchor_camera)) = anchors.get(popover.anchor) else {
            continue;
        };
        let viewport = cameras.of(anchor_camera.0);
        let rect = viewport
            .filter(|_| !anchor_area.0.is_empty())
            .map_or(Rect::ZERO, |viewport| {
                popover_rect(anchor_area.0, popover, viewport)
            });
        // Hidden popovers track their anchor through UiArea alone;
        // compute_widget_areas keeps input zeroed, so the unhide frame
        // extracts a placed rect, not a stale one.
        if !hidden {
            computed.set_if_neq(ComputedWidgetArea(rect));
        }
        let local = viewport.map_or(rect, |viewport| local_area(rect, viewport));
        area.set_if_neq(UiArea::Fixed(local));
    }
}
