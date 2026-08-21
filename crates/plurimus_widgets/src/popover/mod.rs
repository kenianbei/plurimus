//! Anchored placement: popovers position themselves against another widget's
//! resolved area each frame.
//!
//! A popover names an anchor, a preferred side and an alignment, and is
//! placed against wherever that anchor ended up this frame - so it follows a
//! moving anchor without anything having to notify it. When the preferred
//! side has no room the popover mirrors to the opposite one and, failing
//! that, is clamped into the viewport, which is why a popover near an edge
//! stays wholly visible instead of being cut off.
//!
//! What it attaches to is either the anchor's whole area or one cell of the
//! anchor's content, which is what puts a completion list under a caret: the
//! cell is named in content space and the anchor's own [`ScrollOffset`] maps
//! it, so an editor says where its caret is exactly once, in the component
//! it already publishes it in.
//!
//! Every side attaches to an *outer* edge, so a box drawn *inside* its
//! anchor is not a popover at all - it is a child holding a
//! [`UiArea::Fixed`], placed from the anchor's rect with
//! [`local_area`]. That case has no side to mirror and no edge to align to,
//! which is why [`PopoverSide`] has four variants and not five.

mod rect;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::{Commands, Component, Entity, Has, Query, Without};
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};

use plurimus_core::{
    CameraViewports, ComputedUiCamera, UiArea, UiCamera, UiHidden, UiOrder, local_area,
};
use plurimus_ui::{ComputedWidgetArea, ScrollOffset, screen_cell};

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
/// The camera is the anchor's unless [`camera`](Self::camera) names another,
/// which is what lets a popover follow something it is not parented to. It is
/// written as a real [`UiCamera`] rather than a resolved value, so a
/// popover's own children inherit it the way they inherit any camera - parent
/// them to the popover and nothing else is needed.
///
/// The anchor must not itself be a popover.
#[derive(Component, Debug, Clone, Copy)]
#[require(UiOrder = UiOrder::OVERLAY)]
#[non_exhaustive]
pub struct Popover {
    /// The widget this popover attaches to.
    pub anchor: Entity,
    /// Which cell of the anchor to attach to, in the anchor's content
    /// space; `None` attaches to the whole of its area.
    ///
    /// Content space rather than screen space, so a caret is named the way
    /// [`WidgetCursor`](plurimus_ui::WidgetCursor) names it and the
    /// anchor's own [`ScrollOffset`] is applied here rather than by
    /// whoever set this. A cell scrolled out of the anchor's view places
    /// the popover nowhere, since there is nothing on screen to attach to.
    pub cell: Option<Position>,
    /// Which camera to draw on; `None` takes the anchor's.
    ///
    /// The camera's viewport is also what the popover mirrors and clamps
    /// into, so naming one is how a popover escapes an anchor with no room
    /// to open against: a menu anchored to a docked one-row strip has
    /// nowhere to be within that row, and drawing on a full-terminal camera
    /// gives it the whole screen to be clamped into instead. The anchor
    /// still supplies the rect, which is in screen space and so means the
    /// same on either camera.
    pub camera: Option<Entity>,
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
            cell: None,
            camera: None,
            side: PopoverSide::Bottom,
            align: PopoverAlign::Start,
            size,
        }
    }

    /// Attaches to one cell of the anchor's content rather than to the
    /// whole of its area.
    #[must_use]
    pub const fn with_cell(mut self, cell: Position) -> Self {
        self.cell = Some(cell);
        self
    }

    /// Draws on `camera`, and is bounded by its viewport, rather than the
    /// anchor's.
    #[must_use]
    pub const fn with_camera(mut self, camera: Entity) -> Self {
        self.camera = Some(camera);
        self
    }
}

/// Takes each popover onto the camera it draws on: the one it names, else
/// its anchor's.
///
/// Separate from [`place_popovers`] because the rect needs the anchor's
/// resolved area and so must wait for `UiSystems::Areas`, while the camera
/// needs only the anchor's own and every reader downstream wants it settled
/// before then - the placement included, which resolves against the viewport
/// of whichever camera this settled on. Writing the resolved value serves
/// this frame; writing a real [`UiCamera`] beside it is what reaches the
/// popover's own children.
///
/// A popover whose anchor is gone adopts nothing, a camera it named
/// included: it is placed against that anchor or not at all.
pub(crate) fn adopt_anchor_cameras(
    anchors: Query<&ComputedUiCamera, Without<Popover>>,
    mut popovers: Query<(Entity, &Popover, &mut ComputedUiCamera, Option<&UiCamera>)>,
    mut commands: Commands,
) {
    for (entity, popover, mut target, own_camera) in &mut popovers {
        let Ok(anchor_camera) = anchors.get(popover.anchor) else {
            continue;
        };
        let drawn_on = popover.camera.or(anchor_camera.0);
        target.set_if_neq(ComputedUiCamera(drawn_on));
        if let Some(camera) = drawn_on
            && own_camera.map(|own| own.0) != Some(camera)
        {
            commands.entity(entity).insert(UiCamera(camera));
        }
    }
}

type Anchors<'w, 's> =
    Query<'w, 's, (&'static ComputedWidgetArea, Option<&'static ScrollOffset>), Without<Popover>>;

pub(crate) fn place_popovers(
    cameras: CameraViewports,
    anchors: Anchors,
    mut popovers: Query<(
        &Popover,
        &ComputedUiCamera,
        &mut UiArea,
        &mut ComputedWidgetArea,
        Has<UiHidden>,
    )>,
) {
    for (popover, target, mut area, mut computed, hidden) in &mut popovers {
        let Ok((anchor_area, offset)) = anchors.get(popover.anchor) else {
            continue;
        };
        // The popover's own camera, which adoption settled before areas
        // resolved: the anchor's rect is in screen space either way, so
        // only the bound to mirror and clamp into changes.
        let viewport = cameras.of(target.0);
        let anchored = anchor_rect(popover, anchor_area.0, offset);
        let rect = viewport
            .zip(anchored)
            .map_or(Rect::ZERO, |(viewport, anchored)| {
                popover_rect(anchored, popover, viewport)
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

/// What the popover is placed against: the anchor's whole area, or the one
/// cell of its content [`Popover::cell`] names.
///
/// `None` is "nowhere to attach to" - an anchor drawing nothing, or a cell
/// scrolled out of its window - which places the popover at [`Rect::ZERO`]
/// rather than anywhere a guess would put it.
fn anchor_rect(popover: &Popover, area: Rect, offset: Option<&ScrollOffset>) -> Option<Rect> {
    if area.is_empty() {
        return None;
    }
    let Some(cell) = popover.cell else {
        return Some(area);
    };
    let offset = offset.map_or(Position::ORIGIN, |offset| offset.0);
    let cell = screen_cell(cell, area, offset)?;
    Some(Rect::new(cell.x, cell.y, 1, 1))
}
