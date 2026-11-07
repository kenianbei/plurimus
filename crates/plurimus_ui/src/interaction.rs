//! Interaction state over widget screen rects.
//!
//! A widget's [`ComputedWidgetArea`] is the only thing hit-testing needs:
//! the picking backend reports the areas under the pointer by z-order, and
//! `bevy_picking` resolves [`Hovered`], [`Pressed`] and the pointer events
//! widget crates observe from there.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::{Entity, EntityHashSet};
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Commands, Component, Has, Local, Query, With, Without};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{
    CameraViewports, ComputedUiCamera, Hidden, UiArea, UiOrder, UiWidget, resolve_area,
};

pub use bevy_picking::hover::Hovered;
pub use bevy_ui::{Checked, InteractionDisabled, Pressed};
pub use bevy_ui_widgets::{Activate, ValueChange};

/// Present on every entity disabled by its own [`InteractionDisabled`] or
/// an ancestor's, through `ChildOf`.
///
/// A disabled widget is inert, not invisible: it is still picked and so
/// blocks what is beneath it, but its press moves no focus, captures
/// nothing and goes no higher - the way a disabled control blocks a click
/// everywhere else. A widget that should be transparent instead carries
/// bevy's `Pickable`. Wheel ticks are the exception: a disabled widget
/// consumes none, so they fall through, the same as an axis it cannot
/// scroll.
///
/// Resolved in [`UiSystems::Areas`](crate::UiSystems::Areas), before focus
/// dispatch and the pointer events read it, so what is disabled later in a
/// frame takes effect from the next one. Read it rather than write it: the
/// resolver removes it from whatever no [`InteractionDisabled`] reaches.
#[derive(Component, Debug, Clone, Copy)]
pub struct ComputedDisabled;

pub(crate) fn propagate_disabled(
    roots: Query<Entity, With<InteractionDisabled>>,
    children: Query<&Children>,
    marked: Query<Entity, With<ComputedDisabled>>,
    mut reached: Local<EntityHashSet>,
    mut commands: Commands,
) {
    reached.clear();
    for root in &roots {
        reached.insert(root);
        reached.extend(children.iter_descendants(root));
    }
    for entity in &marked {
        if !reached.remove(&entity) {
            commands.entity(entity).try_remove::<ComputedDisabled>();
        }
    }
    for entity in reached.drain() {
        commands.entity(entity).try_insert(ComputedDisabled);
    }
}

/// Keeps a press on the widget, or on anything inside it, from moving
/// focus, while the press itself still lands - [`Pressed`], the drag and
/// the click all arrive. For a toolbar control beside an editor:
/// tab-reachable through its `TabIndex`, but a click on it leaves the
/// keyboard - and any armed selection - where they were.
///
/// A press otherwise focuses the first `TabIndex` carrier at or above what
/// it lands on through `ChildOf`, so a part of a widget - a tab of a tab
/// bar - focuses the widget. The search stops here, and at an open overlay's
/// [`ModalOpen`](crate::ModalOpen) root, whose opener lies outside it.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PressFocusDisabled;

/// Screen-space rect the widget occupies, resolved every frame.
///
/// Requires [`ComputedUiCamera`] because a [`UiArea`] is camera-local:
/// resolving one means knowing which camera it is local to, whether or not
/// the entity draws anything of its own.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[require(ComputedUiCamera)]
pub struct ComputedWidgetArea(pub Rect);

/// Arbitration key: highest band first, then the innermost (smallest)
/// rect, then a stable tiebreak. Innermost-wins is what routes a tick to
/// a nested scroller rather than its scrolling ancestor.
pub(crate) fn z_order_key(
    entity: Entity,
    order: Option<&UiOrder>,
    area: Rect,
) -> (i32, std::cmp::Reverse<u32>, Entity) {
    let cells = u32::from(area.width) * u32::from(area.height);
    (
        order.map_or(0, |order| order.0),
        std::cmp::Reverse(cells),
        entity,
    )
}

pub(crate) fn attach_widget_areas(
    widgets: Query<Entity, (With<UiWidget>, Without<ComputedWidgetArea>)>,
    mut commands: Commands,
) {
    for entity in &widgets {
        commands
            .entity(entity)
            .insert(ComputedWidgetArea::default());
    }
}

pub(crate) fn compute_widget_areas(
    cameras: CameraViewports,
    mut widgets: Query<(
        &UiArea,
        &ComputedUiCamera,
        &mut ComputedWidgetArea,
        Has<Hidden>,
    )>,
) {
    for (area, target, mut computed, hidden) in &mut widgets {
        let resolved = cameras
            .of(target.0)
            .filter(|_| !hidden)
            .map_or(Rect::ZERO, |viewport| resolve_area(*area, viewport));
        computed.set_if_neq(ComputedWidgetArea(resolved));
    }
}
