//! Scrollable widget content.
//!
//! A [`ScrollArea`] describes content larger than the rect it is drawn in,
//! [`ScrollOffset`] is where that content currently sits, and the wheel
//! router sends a tick to the topmost [`WheelReceptive`] widget under it that
//! can use that axis, so a tick a widget cannot use passes to whatever is
//! beneath. Scrolled widgets are drawn
//! through tui-scrollview rather than into the camera buffer directly, which
//! is why they carry [`RasterDeferred`] and are rasterized by `scrolled`
//! instead of core's widget pass.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::prelude::{
    Commands, Component, EntityEvent, MessageReader, Mut, On, Query, Res, With, Without,
};
use bevy_ecs::world::DeferredWorld;
use bevy_picking::pointer::PointerInput;
use plurimus_core::CellSize;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{RasterDeferred, UiOrder};
use tui_scrollview::ScrollbarVisibility;

use crate::interaction::{ComputedDisabled, ComputedWidgetArea, Hovered, ValueChange, z_order_key};
use crate::modal::ModalGuard;
use crate::pointer::{cell_of, scroll_step};

/// Declares a widget entity's content to be `content_size` cells,
/// windowed into the resolved area at render time by its
/// [`ScrollOffset`].
///
/// Removing it hands the widget back to core's widget pass, drawn
/// unscrolled with its offset at the origin. [`WheelReceptive`] stays,
/// since other components require it too.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[require(ScrollOffset, WheelReceptive, RasterDeferred, ComputedWidgetArea)]
#[component(on_remove = release_scroll_area)]
#[non_exhaustive]
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

    /// Sets which scrollbars the area draws for itself; `Never` when a
    /// bar beside it supplies the visuals instead.
    #[must_use]
    pub const fn with_scrollbars(mut self, scrollbars: ScrollbarVisibility) -> Self {
        self.scrollbars = scrollbars;
        self
    }

    /// Usable content width inside `area_width`, accounting for the
    /// one-column gutter tui-scrollview reserves for a visible bar.
    ///
    /// The width content is laid out at, reserved before anyone knows
    /// whether it overflows; [`Self::viewport`] is what it is drawn into.
    #[must_use]
    pub fn content_width(&self, area_width: u16) -> u16 {
        match self.scrollbars {
            ScrollbarVisibility::Never => area_width.max(1),
            _ => area_width.saturating_sub(1).max(1),
        }
    }

    /// The part of `area` the content is drawn into, once tui-scrollview
    /// has taken a column for a vertical bar and a row for a horizontal one.
    ///
    /// Automatic bars follow tui-scrollview's rule: nothing when the
    /// content fits, otherwise a bar on every axis the content does not
    /// fit, an exact fit included, since the other bar takes the line that
    /// made it fit.
    #[must_use]
    pub const fn viewport(&self, area: Rect) -> Rect {
        let content = self.content_size;
        let (horizontal, vertical) = match self.scrollbars {
            ScrollbarVisibility::Always => (true, true),
            ScrollbarVisibility::Never => (false, false),
            ScrollbarVisibility::Automatic => {
                let overflows = content.width > area.width || content.height > area.height;
                (
                    overflows && content.width >= area.width,
                    overflows && content.height >= area.height,
                )
            }
        };
        Rect {
            width: area.width.saturating_sub(vertical as u16),
            height: area.height.saturating_sub(horizontal as u16),
            ..area
        }
    }

    /// The farthest offset the content allows when drawn at `area`.
    #[must_use]
    pub const fn max_offset(&self, area: Rect) -> Position {
        max_offset(self.content_size, self.viewport(area))
    }

    /// Where `step` moves `offset` in this area drawn at `area`, clamped
    /// to the content's extent.
    pub(crate) fn stepped(self, area: Rect, offset: Position, step: (i32, i32)) -> Position {
        let max = self.max_offset(area);
        Position::new(
            stepped_offset(offset.x, step.0, max.x),
            stepped_offset(offset.y, step.1, max.y),
        )
    }
}

// Bevy leaves required components behind. `try_`, since a despawn runs
// this too and a plain remove would warn about the vanished entity.
fn release_scroll_area(mut world: DeferredWorld, context: HookContext) {
    if let Some(mut offset) = world.get_mut::<ScrollOffset>(context.entity) {
        *offset = ScrollOffset::default();
    }
    world
        .commands()
        .entity(context.entity)
        .try_remove::<RasterDeferred>();
}

/// Scroll offset in cells from the content's top-left.
///
/// Systems mutating it clamp to the [`max_offset`] of the content in the
/// area's [`ScrollArea::viewport`]; the render side only reads it
/// (extraction is one-directional).
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
/// [`ScrollBy`], so stacked widgets never both scroll. Widgets disabled
/// by [`InteractionDisabled`](crate::InteractionDisabled), their own or an
/// ancestor's, are skipped, and the tick falls through to the next
/// receptive widget beneath.
///
/// Wheel arbitration only: a scroll addressed to a widget by any other
/// route reaches it without consulting this. Requires [`Hovered`], which
/// is what makes the widget a pick, so a tick over it never also reaches
/// a node it is drawn over.
#[derive(Component, Debug, Clone, Copy, Default)]
#[require(Hovered)]
pub struct WheelReceptive;

/// A scroll of `step` cells, delivered to one widget.
///
/// Produced by a wheel tick over the widget; an app triggers it directly to
/// scroll a widget from anywhere else, a key handler included.
///
/// The step is what the producer asked for, not what the content allows:
/// a consumer clamps against its own extent, so a step past the end is a
/// jump to it.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct ScrollBy {
    /// The widget receiving the scroll.
    pub entity: Entity,
    /// Step in cells, as (columns, rows).
    pub step: (i32, i32),
}

impl ScrollBy {
    /// A scroll of `step` cells addressed to `entity`.
    #[must_use]
    pub const fn new(entity: Entity, step: (i32, i32)) -> Self {
        Self { entity, step }
    }
}

// Disabled still means fall-through here, unlike the press: consuming a
// tick is this router's own opt-in, and a disabled widget consuming
// nothing is the same as an axis it cannot scroll.
type WheelTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ComputedWidgetArea,
        Option<&'static UiOrder>,
        Option<&'static ScrollArea>,
    ),
    (With<WheelReceptive>, Without<ComputedDisabled>),
>;

/// The topmost receptive widget at `position` that can move by `step` and
/// the open modals admit.
fn wheel_target(
    targets: &WheelTargetQuery,
    position: Position,
    step: (i32, i32),
    modal: &ModalGuard,
) -> Option<Entity> {
    targets
        .iter()
        .filter(|(entity, area, _, scroll)| {
            area.0.contains(position)
                && consumes(*scroll, area.0, step)
                && modal.admits(position, *entity)
        })
        .max_by_key(|(entity, area, order, _)| z_order_key(*entity, *order, area.0))
        .map(|(entity, ..)| entity)
}

// Axis-granular, not direction-granular: a widget scrolled to its bottom
// still claims the vertical axis, so the target does not flicker as the
// offset moves. A receptive widget the router cannot measure - a text editor
// scrolling its own viewport - claims both.
const fn consumes(scroll: Option<&ScrollArea>, area: Rect, (columns, rows): (i32, i32)) -> bool {
    let Some(scroll) = scroll else {
        return true;
    };
    let max = scroll.max_offset(area);
    (columns != 0 && max.x > 0) || (rows != 0 && max.y > 0)
}

/// Every tick of the frame, at its own cell: the topmost receptive widget
/// that can move by it and the open overlays admit is sent a [`ScrollBy`],
/// and a tick outside every open overlay dismisses them.
pub(crate) fn route_wheel(
    mut inputs: MessageReader<PointerInput>,
    cell: Res<CellSize>,
    targets: WheelTargetQuery,
    modal: ModalGuard,
    mut commands: Commands,
) {
    for input in inputs.read() {
        let Some(step) = scroll_step(input.action) else {
            continue;
        };
        let position = cell_of(input.location.position, *cell);
        if modal.dismisses(position) {
            modal.dismiss_all(&mut commands);
            continue;
        }
        if let Some(entity) = wheel_target(&targets, position, step, &modal) {
            commands.trigger(ScrollBy { entity, step });
        }
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
    let stepped = scroll.stepped(computed.0, offset.0, event.step);
    apply_offset(event.entity, stepped, &mut offset, &mut commands);
}

// Saturating rather than wrapping, because a step asking for an extreme
// is i32 at full range and would overflow the sum on the way there.
fn stepped_offset(offset: u16, step: i32, max: u16) -> u16 {
    i32::from(offset)
        .saturating_add(step)
        .clamp(0, i32::from(max)) as u16
}

pub(crate) fn scroll_into_view(
    event: On<ScrollIntoView>,
    mut areas: OffsetQuery,
    mut commands: Commands,
) {
    let Ok((computed, scroll, mut offset)) = areas.get_mut(event.entity) else {
        return;
    };
    let viewport = scroll.viewport(computed.0);
    let max = scroll.max_offset(computed.0);
    let revealed = Position::new(
        reveal_axis(
            offset.0.x,
            event.target.x,
            event.target.width,
            viewport.width,
        )
        .min(max.x),
        reveal_axis(
            offset.0.y,
            event.target.y,
            event.target.height,
            viewport.height,
        )
        .min(max.y),
    );
    apply_offset(event.entity, revealed, &mut offset, &mut commands);
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
/// and scrolled by `offset`; a widget carrying no [`ScrollOffset`] is not
/// scrolled.
///
/// Clamps into the area rather than refusing, because a drag is captured
/// to the widget it began on: the cursor may be well outside by now, and
/// the nearest cell is what keeps it selecting. A press is hit-tested
/// inside the area before it is routed, so for one the clamp never binds.
/// `None` only for an empty area, which addresses no cell at all.
#[must_use]
pub fn content_cell(
    screen: Position,
    area: Rect,
    offset: Option<&ScrollOffset>,
) -> Option<Position> {
    if area.is_empty() {
        return None;
    }
    let offset = offset.map_or(Position::ORIGIN, |offset| offset.0);
    let x = screen.x.clamp(area.x, area.right() - 1) - area.x;
    let y = screen.y.clamp(area.y, area.bottom() - 1) - area.y;
    Some(Position::new(
        x.saturating_add(offset.x),
        y.saturating_add(offset.y),
    ))
}

/// Where a content cell lands on screen, for a widget drawn at `area` and
/// scrolled by `offset`; a widget carrying no [`ScrollOffset`] is not
/// scrolled.
///
/// The inverse of [`content_cell`], and unlike it this refuses rather than
/// clamps: a content cell scrolled out of the area is not on screen at all,
/// and answering with the nearest edge would put a caret where its
/// character is not. `None` for an empty area too.
#[must_use]
pub fn screen_cell(
    content: Position,
    area: Rect,
    offset: Option<&ScrollOffset>,
) -> Option<Position> {
    if area.is_empty() {
        return None;
    }
    let offset = offset.map_or(Position::ORIGIN, |offset| offset.0);
    let x = content.x.checked_sub(offset.x)?;
    let y = content.y.checked_sub(offset.y)?;
    (x < area.width && y < area.height).then(|| Position::new(area.x + x, area.y + y))
}

/// The largest valid [`ScrollOffset`] for `content` windowed by `area`;
/// for a [`ScrollArea`] that is its [`ScrollArea::viewport`], not the whole
/// area it is drawn in.
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
mod tests;
