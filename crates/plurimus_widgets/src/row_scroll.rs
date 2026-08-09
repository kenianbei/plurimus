//! Scroll extent for a container drawn from row children.
//!
//! A scroll area windows a widget whole, so the content it windows is as
//! tall as the rows the container draws - bands included, which is why a
//! scrolled table's header scrolls with its body.

use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Changed, Component, Or, Query, With};
use plurimus_core::ratatui_core::layout::Size;

use plurimus_ui::{ComputedWidgetArea, ScrollArea};

/// Keeps a scrollable container's content size at (content width, row
/// count), so the generic scroll machinery windows it correctly.
pub(crate) fn sync_row_scroll<Container: Component, Row: Component>(
    mut containers: Query<
        (&ComputedWidgetArea, &Children, &mut ScrollArea),
        (
            With<Container>,
            Or<(
                Changed<Children>,
                Changed<ComputedWidgetArea>,
                Changed<ScrollArea>,
            )>,
        ),
    >,
    rows: Query<(), With<Row>>,
) {
    for (area, children, mut scroll) in &mut containers {
        let lines = children.iter().filter(|&&row| rows.contains(row)).count();
        let content = Size::new(
            scroll.content_width(area.0.width),
            u16::try_from(lines).unwrap_or(u16::MAX),
        );
        if scroll.content_size != content {
            scroll.content_size = content;
        }
    }
}
