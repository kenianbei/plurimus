//! A standalone scrollbar controlling a target scroll area.
//!
//! The bar is a separate entity that names its target, rather than a part of
//! the scrollable widget, so it can be placed anywhere - beside a list, on a
//! pane's edge - and several bars can drive one area. Press, drag and release
//! all seek the same way: the pointer's position along the track becomes a
//! ratio, and the ratio becomes the target's offset.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Component, On, Query, Res, Without};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::layout::Position;
use ratatui_widgets::scrollbar::{
    Scrollbar as ScrollbarWidget, ScrollbarOrientation, ScrollbarState,
};

use plurimus_ui::bevy_ui_widgets::ControlOrientation;

use plurimus_core::CellSize;
use plurimus_core::UiWidget;
use plurimus_ui::bevy_picking::events::{PointerClick, PointerDrag, PointerPress};
use plurimus_ui::bevy_picking::pointer::PointerButton;
use plurimus_ui::{
    ComputedWidgetArea, Hovered, ScrollArea, ScrollOffset, StylistDisabled, UiTheme, apply_offset,
    pointer_cell,
};
use plurimus_ui::{StateQuery, StylistCache, hashed_bits, observed};

/// A scrollbar for a target [`ScrollArea`] entity. The stock stylist
/// derives track and thumb from the target; press/drag on the track
/// seeks the target's [`ScrollOffset`]. Set the target's scrollbars to
/// [`Never`](plurimus_ui::tui_scrollview::ScrollbarVisibility::Never) so bars are
/// not drawn twice.
#[derive(Component, Debug, Clone)]
#[require(Hovered, StylistCache)]
#[non_exhaustive]
pub struct Scrollbar {
    /// The scroll-area entity this bar controls.
    pub target: Entity,
    /// The bar's axis. Which edge it sits on is where the app places its
    /// strip, the bar filling the area it is given.
    pub orientation: ControlOrientation,
}

impl Scrollbar {
    /// A bar driving `target` along `orientation`'s axis.
    #[must_use]
    pub const fn new(target: Entity, orientation: ControlOrientation) -> Self {
        Self {
            target,
            orientation,
        }
    }
}

/// Spawn bundle for a scrollbar driving `target`.
#[must_use]
pub fn scrollbar(target: Entity, orientation: ControlOrientation) -> impl Bundle {
    (Scrollbar::new(target, orientation), UiWidget::default())
}

type Bar<'a> = (&'a ComputedWidgetArea, &'a Scrollbar);

type TargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ScrollArea,
        &'static ComputedWidgetArea,
        &'static mut ScrollOffset,
    ),
>;

pub(crate) fn scrollbar_press(
    mut event: On<PointerPress>,
    cell: Res<CellSize>,
    bars: Query<Bar>,
    mut targets: TargetQuery,
    mut commands: Commands,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok(bar) = bars.get(event.entity) else {
        return;
    };
    event.propagate(false);
    seek(
        bar,
        pointer_cell(&event.pointer, *cell),
        &mut targets,
        &mut commands,
    );
}

pub(crate) fn scrollbar_drag(
    mut event: On<PointerDrag>,
    cell: Res<CellSize>,
    bars: Query<Bar>,
    mut targets: TargetQuery,
    mut commands: Commands,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok(bar) = bars.get(event.entity) else {
        return;
    };
    event.propagate(false);
    seek(
        bar,
        pointer_cell(&event.pointer, *cell),
        &mut targets,
        &mut commands,
    );
}

// The click rather than the release: bevy sends a release to whatever the
// pointer is over, and a click only to what this pointer pressed.
pub(crate) fn scrollbar_release(
    mut event: On<PointerClick>,
    cell: Res<CellSize>,
    bars: Query<Bar>,
    mut targets: TargetQuery,
    mut commands: Commands,
) {
    if event.button != PointerButton::Primary {
        return;
    }
    let Ok(bar) = bars.get(event.entity) else {
        return;
    };
    event.propagate(false);
    seek(
        bar,
        pointer_cell(&event.pointer, *cell),
        &mut targets,
        &mut commands,
    );
}

fn seek(
    (bar_area, bar): Bar,
    pointer: Position,
    targets: &mut TargetQuery,
    commands: &mut Commands,
) {
    let track = bar_area.0;
    let Ok((scroll, target_area, mut offset)) = targets.get_mut(bar.target) else {
        return;
    };
    let max = scroll.max_offset(target_area.0);
    let sought = if bar.orientation == ControlOrientation::Vertical {
        Position::new(
            offset.0.x,
            track_value(track.y, track.height, pointer.y, max.y),
        )
    } else {
        Position::new(
            track_value(track.x, track.width, pointer.x, max.x),
            offset.0.y,
        )
    };
    apply_offset(bar.target, sought, &mut offset, commands);
}

fn track_value(start: u16, length: u16, pointer: u16, max: u16) -> u16 {
    (super::track_ratio(start, length, pointer) * f32::from(max)).round() as u16
}

pub(crate) fn style_scrollbars(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut bars: Query<
        (StateQuery, &Scrollbar, &mut StylistCache, &mut UiWidget),
        Without<StylistDisabled>,
    >,
    targets: Query<(&ScrollArea, &ScrollOffset, &ComputedWidgetArea)>,
) {
    for (state, bar, mut cache, mut widget) in &mut bars {
        let Ok((scroll, offset, area)) = targets.get(bar.target) else {
            continue;
        };
        let shown = scroll.viewport(area.0);
        let (position, content, viewport) = if bar.orientation == ControlOrientation::Vertical {
            (offset.0.y, scroll.content_size.height, shown.height)
        } else {
            (offset.0.x, scroll.content_size.width, shown.width)
        };
        let next = observed(state, &focus, hashed_bits((position, content, viewport)));
        if !cache.redraws(next, theme.is_changed()) {
            continue;
        }
        *widget = UiWidget::stateful(
            ScrollbarWidget::new(drawn_orientation(bar.orientation)).style(next.style(&theme)),
            ScrollbarState::new(usize::from(content))
                .position(usize::from(position))
                .viewport_content_length(usize::from(viewport)),
        );
    }
}

// A bar's area is its own one-cell strip, so either edge of it is the bar.
const fn drawn_orientation(orientation: ControlOrientation) -> ScrollbarOrientation {
    match orientation {
        ControlOrientation::Vertical => ScrollbarOrientation::VerticalRight,
        ControlOrientation::Horizontal => ScrollbarOrientation::HorizontalBottom,
    }
}

#[cfg(test)]
mod tests {
    use super::track_value;

    #[test]
    fn track_value_maps_cells_to_offsets() {
        assert_eq!(track_value(1, 4, 1, 9), 0);
        assert_eq!(track_value(1, 4, 2, 9), 3);
        assert_eq!(track_value(1, 4, 4, 9), 9);
        assert_eq!(track_value(1, 4, 40, 9), 9);
    }
}
