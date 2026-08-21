//! Anchored placement: popovers position themselves against another widget's
//! resolved area each frame.
//!
//! A popover names an anchor, a preferred side and an alignment, and is
//! placed against wherever that anchor ended up this frame - so it follows a
//! moving anchor without anything having to notify it. When the preferred
//! side has no room the popover mirrors to the opposite one and, failing
//! that, is clamped into the viewport, which is why a popover near an edge
//! stays wholly visible instead of being cut off.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::{Component, Entity, Has, Query, Without};
use plurimus_core::ResolvedViewport;
use plurimus_core::ratatui_core::layout::{Rect, Size};

use plurimus_core::{ComputedUiCamera, UiArea, UiHidden, UiOrder, local_area};
use plurimus_ui::ComputedWidgetArea;

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
/// overwriting its [`UiArea`] and the camera it draws on.
///
/// The camera is the anchor's, which is what lets a popover follow
/// something it is not parented to; the popover's own
/// [`ComputedUiCamera`] carries it, leaving [`UiCamera`] the app's to set.
/// A popover's children still inherit through the hierarchy, so parent
/// them to the popover.
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

pub(crate) fn place_popovers(
    cameras: Query<&ResolvedViewport>,
    anchors: Query<(&ComputedWidgetArea, &ComputedUiCamera), Without<Popover>>,
    mut popovers: Query<(
        &Popover,
        &mut UiArea,
        &mut ComputedWidgetArea,
        &mut ComputedUiCamera,
        Has<UiHidden>,
    )>,
) {
    for (popover, mut area, mut computed, mut target, hidden) in &mut popovers {
        let Ok((anchor_area, anchor_camera)) = anchors.get(popover.anchor) else {
            continue;
        };
        let camera = anchor_camera.0;
        let viewport = camera
            .and_then(|entity| cameras.get(entity).ok())
            .map(|resolved| resolved.0);
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
        // A popover follows what it is anchored to rather than what it is
        // parented to, overriding the hierarchy propagation for itself.
        target.set_if_neq(ComputedUiCamera(camera));
    }
}

fn popover_rect(anchor: Rect, popover: &Popover, viewport: Rect) -> Rect {
    let size = popover.size;
    let placement = mirror_on_overflow(anchor, size, popover.side, viewport);
    let (width, height) = (i32::from(size.width), i32::from(size.height));
    let (x, y) = match placement {
        PopoverSide::Top => (
            aligned_x(anchor, width, popover.align),
            i32::from(anchor.top()) - height,
        ),
        PopoverSide::Bottom => (
            aligned_x(anchor, width, popover.align),
            i32::from(anchor.bottom()),
        ),
        PopoverSide::Left => (
            i32::from(anchor.left()) - width,
            aligned_y(anchor, height, popover.align),
        ),
        PopoverSide::Right => (
            i32::from(anchor.right()),
            aligned_y(anchor, height, popover.align),
        ),
    };
    Rect::new(x.max(0) as u16, y.max(0) as u16, size.width, size.height).clamp(viewport)
}

fn mirror_on_overflow(
    anchor: Rect,
    size: Size,
    preferred: PopoverSide,
    viewport: Rect,
) -> PopoverSide {
    let (width, height) = (i32::from(size.width), i32::from(size.height));
    let overflows = |side| match side {
        PopoverSide::Top => i32::from(anchor.top()) - height < i32::from(viewport.top()),
        PopoverSide::Bottom => i32::from(anchor.bottom()) + height > i32::from(viewport.bottom()),
        PopoverSide::Left => i32::from(anchor.left()) - width < i32::from(viewport.left()),
        PopoverSide::Right => i32::from(anchor.right()) + width > i32::from(viewport.right()),
    };
    if overflows(preferred) && !overflows(preferred.mirror()) {
        preferred.mirror()
    } else {
        preferred
    }
}

fn aligned_x(anchor: Rect, width: i32, align: PopoverAlign) -> i32 {
    aligned(
        i32::from(anchor.left()),
        i32::from(anchor.width),
        width,
        align,
    )
}

fn aligned_y(anchor: Rect, height: i32, align: PopoverAlign) -> i32 {
    aligned(
        i32::from(anchor.top()),
        i32::from(anchor.height),
        height,
        align,
    )
}

const fn aligned(start: i32, anchor_span: i32, span: i32, align: PopoverAlign) -> i32 {
    match align {
        PopoverAlign::Start => start,
        PopoverAlign::Center => start + (anchor_span - span) / 2,
        PopoverAlign::End => start + anchor_span - span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;

    const VIEWPORT: Rect = Rect::new(0, 0, 20, 10);

    fn popover(side: PopoverSide, align: PopoverAlign, width: u16, height: u16) -> Popover {
        Popover {
            anchor: Entity::PLACEHOLDER,
            side,
            align,
            size: Size::new(width, height),
        }
    }

    #[test]
    fn places_on_each_side() {
        let anchor = Rect::new(8, 4, 4, 2);
        let assert_side = |side, expected| {
            let placed = popover_rect(anchor, &popover(side, PopoverAlign::Start, 3, 2), VIEWPORT);
            assert_eq!(placed, expected, "{side:?}");
        };
        assert_side(PopoverSide::Top, Rect::new(8, 2, 3, 2));
        assert_side(PopoverSide::Bottom, Rect::new(8, 6, 3, 2));
        assert_side(PopoverSide::Left, Rect::new(5, 4, 3, 2));
        assert_side(PopoverSide::Right, Rect::new(12, 4, 3, 2));
    }

    #[test]
    fn aligns_center_and_end() {
        let anchor = Rect::new(8, 4, 6, 2);
        let center = popover_rect(
            anchor,
            &popover(PopoverSide::Bottom, PopoverAlign::Center, 2, 1),
            VIEWPORT,
        );
        assert_eq!(center, Rect::new(10, 6, 2, 1));
        let end = popover_rect(
            anchor,
            &popover(PopoverSide::Bottom, PopoverAlign::End, 2, 1),
            VIEWPORT,
        );
        assert_eq!(end, Rect::new(12, 6, 2, 1));
    }

    #[test]
    fn mirrors_when_preferred_side_overflows() {
        let near_bottom = Rect::new(8, 8, 4, 2);
        let placed = popover_rect(
            near_bottom,
            &popover(PopoverSide::Bottom, PopoverAlign::Start, 3, 3),
            VIEWPORT,
        );
        assert_eq!(placed, Rect::new(8, 5, 3, 3));
    }

    #[test]
    fn keeps_preferred_side_when_both_overflow() {
        let anchor = Rect::new(8, 4, 4, 2);
        let placed = popover_rect(
            anchor,
            &popover(PopoverSide::Bottom, PopoverAlign::Start, 3, 9),
            VIEWPORT,
        );
        assert_eq!(placed, Rect::new(8, 1, 3, 9));
    }

    #[test]
    fn clamps_alignment_to_viewport() {
        let at_left_edge = Rect::new(0, 4, 3, 1);
        let placed = popover_rect(
            at_left_edge,
            &popover(PopoverSide::Bottom, PopoverAlign::End, 6, 2),
            VIEWPORT,
        );
        assert_eq!(placed, Rect::new(0, 5, 6, 2));
    }
}
