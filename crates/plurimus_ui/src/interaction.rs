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
pub use bevy_ui::{Checked, InteractionDisabled, Pressed, Selectable, Selected};
pub use bevy_ui_widgets::{Activate, ValueChange};

/// Present on every entity disabled by its own [`InteractionDisabled`] or
/// an ancestor's, through `ChildOf`.
///
/// A disabled widget is inert, not invisible: it is still picked and so
/// blocks what is beneath it, but its press moves no focus - an upstream
/// `Checkbox` aside, see [`PressFocusDisabled`] - captures nothing and goes
/// no higher - the way a disabled control blocks a click
/// everywhere else. A widget that should be transparent instead carries
/// bevy's `Pickable`. Wheel ticks are the exception: a disabled widget
/// consumes none, so they fall through, the same as an axis it cannot
/// scroll.
///
/// Resolved in [`UiSystems::Areas`](crate::UiSystems::Areas), before focus
/// dispatch and the pointer events read it, so what is disabled later in a
/// frame takes effect from the next one. Read it rather than write it: the
/// resolver removes it from whatever no [`InteractionDisabled`] reaches.
///
/// The resolver also writes [`InteractionDisabled`] onto every descendant
/// it reaches, since `bevy_ui_widgets`' observers check the entity alone,
/// and takes back only what it wrote: a flag an app sets on an entity
/// already flagged that way is removed with its ancestor's.
#[derive(Component, Debug, Clone, Copy)]
pub struct ComputedDisabled;

/// What [`propagate_disabled`] carries between frames: the flags it wrote,
/// and the frame's reach, kept to reuse its allocation.
#[derive(Default)]
pub(crate) struct DisabledReach {
    written: EntityHashSet,
    reached: EntityHashSet,
}

/// Resolves [`ComputedDisabled`] beneath every root, and writes
/// [`InteractionDisabled`] down onto what a root reaches, so an observer
/// checking the entity alone - `bevy_ui_widgets`' every one - sees the
/// subtree rule. What it wrote it remembers and removes once no root reaches
/// it; a written flag is no root of its own.
pub(crate) fn propagate_disabled(
    flagged: Query<Entity, With<InteractionDisabled>>,
    children: Query<&Children>,
    marked: Query<Entity, With<ComputedDisabled>>,
    mut reach: Local<DisabledReach>,
    mut commands: Commands,
) {
    let DisabledReach { written, reached } = &mut *reach;
    reached.clear();
    for root in flagged.iter().filter(|root| !written.contains(root)) {
        reached.insert(root);
        reached.extend(children.iter_descendants(root));
    }
    write_down(&flagged, reached, written, &mut commands);
    for entity in &marked {
        if !reached.remove(&entity) {
            commands.entity(entity).try_remove::<ComputedDisabled>();
        }
    }
    for entity in reached.drain() {
        commands.entity(entity).try_insert(ComputedDisabled);
    }
}

fn write_down(
    flagged: &Query<Entity, With<InteractionDisabled>>,
    reached: &EntityHashSet,
    written: &mut EntityHashSet,
    commands: &mut Commands,
) {
    written.retain(|entity| {
        let is_reached = reached.contains(entity);
        if !is_reached {
            commands.entity(*entity).try_remove::<InteractionDisabled>();
        }
        is_reached
    });
    for &entity in reached {
        if !flagged.contains(entity) {
            written.insert(entity);
            commands.entity(entity).try_insert(InteractionDisabled);
        }
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
///
/// `bevy_ui_widgets`' `Checkbox` is the exception: its own press handler
/// focuses it, disabled or not, and nothing here is ordered before it.
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
