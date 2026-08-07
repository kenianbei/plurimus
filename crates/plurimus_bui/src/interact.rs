//! Feeds laid-out node rects into the widget interaction layer, so hover,
//! press, focus and widget logic run unchanged on `bevy_ui` nodes.
//!
//! A node's `visible` rect is published as a
//! [`ComputedWidgetArea`](plurimus_ui::ComputedWidgetArea), which is all
//! `plurimus_ui`'s routers need - they never learn a `bevy_ui` tree is involved.
//! Only nodes that can receive something are published: the ones already
//! hovered, and the ones carrying a `TabIndex`.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Or, Query, With};
use bevy_input_focus::tab_navigation::TabIndex;

use super::publish_area;
use super::rect::ComputedNodeRect;
use plurimus_ui::{ComputedWidgetArea, Hovered};

// Uses the previous frame's layout (same one-frame lag as bevy_picking).
// TabIndex nodes are covered so keyboard-only focusables join the
// directional-navigation map.
pub(crate) fn sync_node_widget_areas(
    mut nodes: Query<
        (Entity, &ComputedNodeRect, Option<&mut ComputedWidgetArea>),
        Or<(With<Hovered>, With<TabIndex>)>,
    >,
    mut commands: Commands,
) {
    for (entity, rect, area) in &mut nodes {
        publish_area(&mut commands, entity, area, rect.visible);
    }
}
