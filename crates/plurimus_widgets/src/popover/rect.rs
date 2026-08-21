//! Where an anchored rect lands: the side, the mirror, the alignment, the
//! clamp.
//!
//! Pure geometry against a screen-space anchor and a viewport, so the
//! placement rules are testable without an app and the systems beside it
//! decide only what the anchor is.

use plurimus_core::ratatui_core::layout::{Rect, Size};

use super::{Popover, PopoverAlign, PopoverSide};

pub(super) fn popover_rect(anchor: Rect, popover: &Popover, viewport: Rect) -> Rect {
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
            cell: None,
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
