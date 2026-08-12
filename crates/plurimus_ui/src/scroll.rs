//! Scrollable widget content.
//!
//! A [`ScrollArea`] describes content larger than the rect it is drawn in;
//! [`ScrollOffset`] is where that content currently sits, and [`WheelAxes`]
//! tells the router which directions a widget can actually consume so a tick
//! it cannot use passes to whatever is beneath. Scrolled widgets are drawn
//! through tui-scrollview rather than into the camera buffer directly, which
//! is why they carry [`RasterDeferred`] and are rasterized by `scrolled`
//! instead of core's widget pass.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{
    Commands, Component, EntityEvent, MessageReader, Mut, On, Query, With, Without,
};
use plurimus_core::RasterDeferred;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_term::{MouseKind, MouseMessage};
use tui_scrollview::ScrollbarVisibility;

use crate::interaction::{
    AreaTargetQuery, ComputedWidgetArea, InteractionDisabled, ValueChange, topmost_at,
};
use crate::modal::ModalGuard;

/// Declares a widget entity's content to be `content_size` cells,
/// windowed into the resolved area at render time by its
/// [`ScrollOffset`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[require(ScrollOffset, WheelReceptive, RasterDeferred)]
pub struct ScrollArea {
    /// Content extent in cells.
    pub content_size: Size,
    /// Scrollbars drawn inside the area when content overflows; set
    /// `Never` when scrollbar visuals are provided elsewhere.
    pub scrollbars: ScrollbarVisibility,
}

impl ScrollArea {
    /// A scroll area with automatic scrollbars.
    #[must_use]
    pub const fn new(content_size: Size) -> Self {
        Self {
            content_size,
            scrollbars: ScrollbarVisibility::Automatic,
        }
    }

    /// Usable content width inside `area_width`, accounting for the
    /// one-column gutter tui-scrollview reserves for a visible bar.
    #[must_use]
    pub fn content_width(&self, area_width: u16) -> u16 {
        match self.scrollbars {
            ScrollbarVisibility::Never => area_width.max(1),
            _ => area_width.saturating_sub(1).max(1),
        }
    }
}

/// Scroll offset in cells from the content's top-left.
///
/// Systems mutating it clamp to `content_size - area`; the render side
/// only reads it (extraction is one-directional).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollOffset(pub Position);

/// Reveals a content-space rect by minimally adjusting the entity's
/// [`ScrollOffset`]. Emits [`ValueChange<Position>`] when it moves.
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct ScrollIntoView {
    /// The scroll-area entity.
    pub entity: Entity,
    /// Content-space rect to make visible.
    pub target: Rect,
}

/// Marks a widget as a wheel target. Every receptive widget under the
/// cursor is arbitrated by z-order and only the topmost is sent a
/// [`ScrollBy`], so stacked widgets never both scroll. Widgets with
/// [`InteractionDisabled`] are skipped, and the tick falls through to the
/// next receptive widget beneath.
///
/// Wheel arbitration only: a scroll addressed to a widget by any other
/// route reaches it without consulting this.
#[derive(Component, Debug, Clone, Copy, Default)]
#[require(WheelAxes)]
pub struct WheelReceptive;

/// Wheel axes a [`WheelReceptive`] widget can currently consume. The
/// router skips a widget that cannot move on the ticked axis, so the tick
/// falls through to the next candidate beneath it.
///
/// Axis-granular, not direction-granular: a widget scrolled to its bottom
/// still claims the vertical axis, so the target does not flicker as the
/// offset moves. Defaults to both, which is what a widget the router
/// cannot measure - a `TextEditor` scrolling its own
/// viewport - should keep.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelAxes {
    /// Consumes horizontal ticks.
    pub horizontal: bool,
    /// Consumes vertical ticks.
    pub vertical: bool,
}

impl WheelAxes {
    const fn consumes(self, (columns, rows): (i32, i32)) -> bool {
        (columns != 0 && self.horizontal) || (rows != 0 && self.vertical)
    }
}

impl Default for WheelAxes {
    fn default() -> Self {
        Self {
            horizontal: true,
            vertical: true,
        }
    }
}

/// A scroll of `step` cells, delivered to one widget.
///
/// Produced by a wheel tick over the widget and by a key bound through
/// [`ScrollKeys`](crate::ScrollKeys); an app triggers it directly to
/// scroll a widget from anywhere else.
///
/// The step is what the producer asked for, not what the content allows:
/// a consumer clamps against its own extent, so a step past the end is a
/// jump to it.
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct ScrollBy {
    /// The widget receiving the scroll.
    pub entity: Entity,
    /// Step in cells, as (columns, rows).
    pub step: (i32, i32),
}

type WheelTargetQuery<'w, 's> =
    AreaTargetQuery<'w, 's, (With<WheelReceptive>, Without<InteractionDisabled>)>;

pub(crate) fn route_wheel(
    mut mouse: MessageReader<MouseMessage>,
    targets: WheelTargetQuery,
    axes: Query<&WheelAxes>,
    modal: ModalGuard,
    mut commands: Commands,
) {
    for message in mouse.read() {
        let Some(step) = wheel_step(message.kind) else {
            continue;
        };
        if modal.intercept_wheel(message.position, &mut commands) {
            continue;
        }
        let consumes = |entity| axes.get(entity).is_ok_and(|axes| axes.consumes(step));
        let Some(entity) = topmost_at(message.position, &targets, consumes) else {
            continue;
        };
        commands.trigger(ScrollBy { entity, step });
    }
}

// Only a widget's own extents tell the router which ticks it can use;
// TextEditor keeps the default, scrolling its viewport itself.
pub(crate) fn sync_scroll_area_axes(
    mut areas: Query<(&ScrollArea, &ComputedWidgetArea, &mut WheelAxes)>,
) {
    for (scroll, computed, mut axes) in &mut areas {
        axes.set_if_neq(WheelAxes {
            horizontal: scroll.content_size.width > computed.0.width,
            vertical: scroll.content_size.height > computed.0.height,
        });
    }
}

type OffsetQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedWidgetArea,
        &'static ScrollArea,
        &'static mut ScrollOffset,
    ),
>;

pub(crate) fn scroll_area_scrolled(
    event: On<ScrollBy>,
    mut areas: OffsetQuery,
    mut commands: Commands,
) {
    let Ok((computed, scroll, mut offset)) = areas.get_mut(event.entity) else {
        return;
    };
    let max = max_offset(scroll.content_size, computed.0);
    let stepped = Position::new(
        stepped_offset(offset.0.x, event.step.0, max.x),
        stepped_offset(offset.0.y, event.step.1, max.y),
    );
    apply_offset(event.entity, stepped, &mut offset, &mut commands);
}

// Widened past both operands before the add: a step asking for an
// extreme is i32 at full range, which an i32 sum would overflow.
fn stepped_offset(offset: u16, step: i32, max: u16) -> u16 {
    (i64::from(offset) + i64::from(step)).clamp(0, i64::from(max)) as u16
}

pub(crate) fn scroll_into_view(
    event: On<ScrollIntoView>,
    mut areas: OffsetQuery,
    mut commands: Commands,
) {
    let Ok((computed, scroll, mut offset)) = areas.get_mut(event.entity) else {
        return;
    };
    let max = max_offset(scroll.content_size, computed.0);
    let revealed = Position::new(
        reveal_axis(
            offset.0.x,
            event.target.x,
            event.target.width,
            computed.0.width,
        )
        .min(max.x),
        reveal_axis(
            offset.0.y,
            event.target.y,
            event.target.height,
            computed.0.height,
        )
        .min(max.y),
    );
    apply_offset(event.entity, revealed, &mut offset, &mut commands);
}

const fn wheel_step(kind: MouseKind) -> Option<(i32, i32)> {
    match kind {
        MouseKind::ScrollUp => Some((0, -1)),
        MouseKind::ScrollDown => Some((0, 1)),
        MouseKind::ScrollLeft => Some((-1, 0)),
        MouseKind::ScrollRight => Some((1, 0)),
        _ => None,
    }
}

/// Writes `position` into the entity's [`ScrollOffset`], emitting
/// [`ValueChange<Position>`] when it moved.
pub fn apply_offset(
    entity: Entity,
    position: Position,
    offset: &mut Mut<ScrollOffset>,
    commands: &mut Commands,
) {
    if offset.set_if_neq(ScrollOffset(position)) {
        commands.trigger(ValueChange {
            source: entity,
            value: position,
            is_final: true,
        });
    }
}

/// Where a screen cell lands in the content of a widget drawn at `area`
/// and scrolled by `offset`.
///
/// Clamps into the area rather than refusing, because a drag is captured
/// to the widget it began on: the cursor may be well outside by now, and
/// the nearest cell is what keeps it selecting. A press is hit-tested
/// inside the area before it is routed, so for one the clamp never binds.
/// `None` only for an empty area, which addresses no cell at all.
#[must_use]
pub fn content_cell(screen: Position, area: Rect, offset: Position) -> Option<Position> {
    if area.is_empty() {
        return None;
    }
    let x = screen.x.clamp(area.x, area.right() - 1) - area.x;
    let y = screen.y.clamp(area.y, area.bottom() - 1) - area.y;
    Some(Position::new(
        x.saturating_add(offset.x),
        y.saturating_add(offset.y),
    ))
}

/// Where a content cell lands on screen, for a widget drawn at `area` and
/// scrolled by `offset`.
///
/// The inverse of [`content_cell`], and unlike it this refuses rather than
/// clamps: a content cell scrolled out of the area is not on screen at all,
/// and answering with the nearest edge would put a caret where its
/// character is not. `None` for an empty area too.
#[must_use]
pub fn screen_cell(content: Position, area: Rect, offset: Position) -> Option<Position> {
    if area.is_empty() {
        return None;
    }
    let x = content.x.checked_sub(offset.x)?;
    let y = content.y.checked_sub(offset.y)?;
    (x < area.width && y < area.height).then(|| Position::new(area.x + x, area.y + y))
}

/// The largest valid [`ScrollOffset`] for `content` windowed by `area`.
#[must_use]
pub const fn max_offset(content: Size, area: Rect) -> Position {
    Position::new(
        content.width.saturating_sub(area.width),
        content.height.saturating_sub(area.height),
    )
}

fn reveal_axis(offset: u16, start: u16, length: u16, window: u16) -> u16 {
    if start < offset {
        start
    } else {
        offset.max(start.saturating_add(length).saturating_sub(window))
    }
}

#[cfg(test)]
mod tests {
    use super::{content_cell, screen_cell, stepped_offset};
    use plurimus_core::ratatui_core::layout::{Position, Rect};

    const AREA: Rect = Rect::new(4, 2, 10, 5);
    const UNSCROLLED: Position = Position::new(0, 0);

    #[test]
    fn a_step_moves_the_offset_and_stops_at_either_bound() {
        assert_eq!(stepped_offset(10, 5, 100), 15);
        assert_eq!(stepped_offset(10, -5, 100), 5);
        assert_eq!(stepped_offset(10, -50, 100), 0);
        assert_eq!(stepped_offset(10, 500, 100), 100);
    }

    // A jump to an extreme is a step at the full range of its type, which
    // must saturate at the bound rather than overflow reaching it.
    #[test]
    fn a_step_past_the_offsets_own_range_still_lands_on_the_bound() {
        assert_eq!(stepped_offset(u16::MAX - 1, i32::MAX, u16::MAX), u16::MAX);
        assert_eq!(stepped_offset(u16::MAX - 1, i32::MIN, u16::MAX), 0);
        assert_eq!(stepped_offset(0, i32::from(i16::MAX) + 1, u16::MAX), 32768);
    }

    #[test]
    fn a_cell_inside_the_area_is_its_offset_from_the_origin() {
        assert_eq!(
            content_cell(Position::new(6, 3), AREA, UNSCROLLED),
            Some(Position::new(2, 1))
        );
    }

    #[test]
    fn the_scroll_offset_is_added_to_what_the_area_resolves() {
        assert_eq!(
            content_cell(Position::new(6, 3), AREA, Position::new(7, 20)),
            Some(Position::new(9, 21))
        );
    }

    // A captured drag reports cells outside the widget it began on; the
    // nearest one is what keeps it selecting rather than collapsing.
    #[test]
    fn a_cell_outside_the_area_clamps_to_the_nearest_edge() {
        assert_eq!(
            content_cell(Position::new(0, 0), AREA, UNSCROLLED),
            Some(Position::new(0, 0)),
            "above and left of the area"
        );
        assert_eq!(
            content_cell(Position::new(99, 99), AREA, UNSCROLLED),
            Some(Position::new(9, 4)),
            "the last cell, not one past it"
        );
    }

    #[test]
    fn a_content_cell_maps_back_to_the_screen_cell_it_came_from() {
        for screen in [
            Position::new(4, 2),
            Position::new(9, 5),
            Position::new(6, 3),
        ] {
            let offset = Position::new(7, 20);
            let content = content_cell(screen, AREA, offset).expect("inside the area");
            assert_eq!(screen_cell(content, AREA, offset), Some(screen));
        }
    }

    // A caret whose character is scrolled off has no screen cell; answering
    // with the nearest edge would draw it beside the wrong character.
    #[test]
    fn a_content_cell_outside_the_window_is_on_no_screen_cell() {
        let offset = Position::new(3, 4);
        assert_eq!(screen_cell(Position::new(2, 5), AREA, offset), None, "left");
        assert_eq!(
            screen_cell(Position::new(5, 3), AREA, offset),
            None,
            "above"
        );
        assert_eq!(
            screen_cell(Position::new(13, 5), AREA, offset),
            None,
            "past the right edge"
        );
        assert_eq!(
            screen_cell(Position::new(5, 9), AREA, offset),
            None,
            "past the bottom edge"
        );
    }

    #[test]
    fn an_empty_area_addresses_no_cell() {
        assert_eq!(
            content_cell(Position::new(4, 2), Rect::ZERO, UNSCROLLED),
            None
        );
        assert_eq!(
            content_cell(Position::new(4, 2), Rect::new(4, 2, 0, 5), UNSCROLLED),
            None,
            "zero width alone is enough"
        );
        assert_eq!(
            screen_cell(Position::new(0, 0), Rect::ZERO, UNSCROLLED),
            None
        );
    }
}
