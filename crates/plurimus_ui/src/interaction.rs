//! Pointer interaction state over widget screen rects.
//!
//! A widget's [`ComputedWidgetArea`] is the only thing hit-testing needs, so
//! hover, press, drag, release and click all resolve by z-ordered rect
//! search: the topmost area containing the cursor wins, and the components
//! that result ([`Hovered`], [`Pressed`], [`Click`]) are what widget crates
//! observe rather than raw mouse messages. Modal state can swallow a hit
//! entirely, which is why a batch of messages is routed one at a time instead
//! of resolved together.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{
    Commands, Component, EntityEvent, Has, Local, MessageReader, Query, Res, ResMut, With, Without,
};
use bevy_ecs::query::QueryFilter;
use bevy_ecs::system::SystemParam;
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::{
    CameraViewports, ComputedUiCamera, UiArea, UiHidden, UiOrder, UiWidget, resolve_area,
};
use plurimus_term::{CursorCell, MouseButton, MouseKind, MouseMessage};

use crate::modal::{ModalGuard, ModalRouting};

/// Whether the cursor is over the widget. Opt-in: spawn it on widgets that
/// should react to the pointer.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Hovered(pub bool);

/// Present while the pointer is pressed on the widget.
#[derive(Component, Debug, Clone, Copy)]
pub struct Pressed;

/// Disables all interaction with the widget.
#[derive(Component, Debug, Clone, Copy)]
pub struct InteractionDisabled;

/// Toggle state for checkboxes and radio buttons.
#[derive(Component, Debug, Clone, Copy)]
pub struct Checked;

/// Screen-space rect the widget occupies, resolved every frame.
///
/// Requires [`ComputedUiCamera`] because a [`UiArea`] is camera-local:
/// resolving one means knowing which camera it is local to, whether or not
/// the entity draws anything of its own.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[require(ComputedUiCamera)]
pub struct ComputedWidgetArea(pub Rect);

/// Pointer click completed on a widget: pressed and released on it.
///
/// The release edge, so a widget that closes or despawns something on being
/// clicked does it after the pointer router is done with the gesture. What
/// the click landed on is resolved from [`position`](Self::position), not
/// from where the press was: a drag that starts on one row and ends on
/// another names the one it ended on.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Click {
    /// The clicked widget.
    pub entity: Entity,
    /// Cursor cell at release time, inside the widget's area.
    pub position: Position,
}

impl Click {
    /// A click completed on `entity`, released at `position`.
    #[must_use]
    pub const fn new(entity: Entity, position: Position) -> Self {
        Self { entity, position }
    }
}

/// Pointer pressed on a widget. Sent to the topmost hovered widget only,
/// so a press on an overlay never also reaches widgets beneath it.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PointerPress {
    /// The pressed widget.
    pub entity: Entity,
    /// Cursor cell at press time.
    pub position: Position,
}

impl PointerPress {
    /// A press on `entity` at `position`.
    #[must_use]
    pub const fn new(entity: Entity, position: Position) -> Self {
        Self { entity, position }
    }
}

/// Pointer moved while a widget is [`Pressed`]. Captured to that widget:
/// it arrives regardless of where the cursor has since moved.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PointerDrag {
    /// The widget the gesture started on.
    pub entity: Entity,
    /// Current cursor cell.
    pub position: Position,
}

impl PointerDrag {
    /// A drag captured to `entity`, now at `position`.
    #[must_use]
    pub const fn new(entity: Entity, position: Position) -> Self {
        Self { entity, position }
    }
}

/// Pointer released, ending the gesture on a [`Pressed`] widget. Captured
/// like [`PointerDrag`]; [`Click`] is the separate activation event, sent
/// only when the release lands back on the widget.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PointerRelease {
    /// The widget the gesture started on.
    pub entity: Entity,
    /// Cursor cell at release time.
    pub position: Position,
}

impl PointerRelease {
    /// A release ending the gesture on `entity`.
    #[must_use]
    pub const fn new(entity: Entity, position: Position) -> Self {
        Self { entity, position }
    }
}

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
        Has<UiHidden>,
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

pub(crate) fn hover_widgets(
    cursor: Res<CursorCell>,
    mut widgets: Query<(&ComputedWidgetArea, &mut Hovered)>,
) {
    for (area, mut hovered) in &mut widgets {
        let over = cursor.0.is_some_and(|position| area.0.contains(position));
        hovered.set_if_neq(Hovered(over));
    }
}

/// Widgets an input router arbitrates between, `F` selecting which kind
/// of input they receive.
pub(crate) type AreaTargetQuery<'w, 's, F> = Query<
    'w,
    's,
    (
        Entity,
        &'static ComputedWidgetArea,
        Option<&'static UiOrder>,
    ),
    F,
>;

// Hovered marks participation by its presence: its value is the frame's
// last cursor cell, but arbitration needs each message's own position.
type PointerTargetQuery<'w, 's> =
    AreaTargetQuery<'w, 's, (With<Hovered>, Without<InteractionDisabled>)>;

type PressedQuery<'w, 's> = Query<'w, 's, (Entity, Has<InteractionDisabled>), With<Pressed>>;

/// Everything [`pointer_interaction`] routes against.
#[derive(SystemParam)]
pub(crate) struct PointerRouting<'w, 's> {
    targets: PointerTargetQuery<'w, 's>,
    pressed: PressedQuery<'w, 's>,
    focusable: Query<'w, 's, (), With<TabIndex>>,
    modal: ModalGuard<'w, 's>,
    focus: ResMut<'w, InputFocus>,
}

pub(crate) fn pointer_interaction(
    mut mouse: MessageReader<MouseMessage>,
    mut carryover: Local<Vec<MouseMessage>>,
    mut routing: PointerRouting,
    mut commands: Commands,
) {
    let mut pending = std::mem::take(&mut *carryover);
    pending.extend(mouse.read().copied());
    // Entities pressed during this run: Pressed lands at command flush,
    // so a same-batch release consults this alongside the query.
    let mut run_pressed = Vec::new();
    let mut messages = pending.into_iter();
    while let Some(message) = messages.next() {
        if route_message(message, &mut routing, &mut run_pressed, &mut commands) {
            carryover.extend(messages);
            break;
        }
    }
}

// Returns true when the message flipped menu modality; the rest of the
// batch then waits a frame, hit-testing the settled state instead of the
// one the flip is about to replace.
fn route_message(
    message: MouseMessage,
    routing: &mut PointerRouting,
    run_pressed: &mut Vec<Entity>,
    commands: &mut Commands,
) -> bool {
    match message.kind {
        MouseKind::Down(MouseButton::Left) => {
            route_press(message.position, routing, run_pressed, commands)
        }
        MouseKind::Drag(MouseButton::Left) => {
            drag_pressed(&routing.pressed, run_pressed, message.position, commands);
            false
        }
        MouseKind::Up(MouseButton::Left) => {
            release_all(routing, run_pressed, message.position, commands)
        }
        _ => false,
    }
}

// A press inside an open modal reaches that modal's subtree or nothing:
// falling through to what the overlay covers is the click-through the
// wheel path already refuses. Only a press outside them all dismisses,
// and a toggle out there routes instead - that is how an opener closes
// the menu it opened.
fn route_press(
    position: Position,
    routing: &mut PointerRouting,
    run_pressed: &mut Vec<Entity>,
    commands: &mut Commands,
) -> bool {
    let target = match routing.modal.routing(position) {
        ModalRouting::Unguarded => topmost_at(position, &routing.targets, |_| true),
        ModalRouting::Confined => topmost_at(position, &routing.targets, |entity| {
            routing.modal.admits(position, entity)
        }),
        ModalRouting::Outside => {
            let target = topmost_at(position, &routing.targets, |_| true);
            if !target.is_some_and(|entity| routing.modal.affects_modality(entity)) {
                routing.modal.dismiss_all(commands);
                return true;
            }
            target
        }
    };
    if let Some(entity) = target {
        commands.trigger(PointerPress { entity, position });
        press(entity, &routing.focusable, &mut routing.focus, commands);
        run_pressed.push(entity);
    }
    false
}

/// The topmost target containing `position` that `accepts` the input, by
/// [`z_order_key`]. Rejected targets fall through to the next beneath.
pub(crate) fn topmost_at<F: QueryFilter>(
    position: Position,
    targets: &AreaTargetQuery<'_, '_, F>,
    accepts: impl Fn(Entity) -> bool,
) -> Option<Entity> {
    targets
        .iter()
        .filter(|(entity, area, _)| area.0.contains(position) && accepts(*entity))
        .max_by_key(|(entity, area, order)| z_order_key(*entity, *order, area.0))
        .map(|(entity, ..)| entity)
}

fn drag_pressed(
    pressed: &PressedQuery,
    run_pressed: &[Entity],
    position: Position,
    commands: &mut Commands,
) {
    for (entity, disabled) in pressed {
        if !disabled {
            commands.trigger(PointerDrag { entity, position });
        }
    }
    for &entity in run_pressed {
        commands.trigger(PointerDrag { entity, position });
    }
}

fn press(
    target: Entity,
    focusable: &Query<(), With<TabIndex>>,
    focus: &mut ResMut<InputFocus>,
    commands: &mut Commands,
) {
    commands.entity(target).insert(Pressed);
    if focusable.contains(target) {
        focus.set(target, FocusCause::Pressed);
    }
}

fn release_all(
    routing: &PointerRouting,
    run_pressed: &mut Vec<Entity>,
    position: Position,
    commands: &mut Commands,
) -> bool {
    // Run-pressed entities came from a query excluding disabled widgets,
    // and nothing disables them mid-run.
    let fresh = run_pressed
        .drain(..)
        .filter(|&entity| !routing.pressed.contains(entity))
        .map(|entity| (entity, false));
    let mut menu_clicked = false;
    for (entity, disabled) in routing.pressed.iter().chain(fresh) {
        if !disabled {
            commands.trigger(PointerRelease { entity, position });
        }
        let released_on = routing
            .targets
            .get(entity)
            .is_ok_and(|(_, area, _)| area.0.contains(position));
        if released_on {
            commands.trigger(Click { entity, position });
            menu_clicked |= routing.modal.affects_modality(entity);
        }
        commands.entity(entity).remove::<Pressed>();
    }
    menu_clicked
}

/// A widget's value changed. `T` is the value type: `bool` for toggles,
/// `f32` for sliders, [`Entity`] for selections, `String` for text,
/// `Position` for scroll offsets.
#[derive(EntityEvent, Debug, Clone)]
#[non_exhaustive]
pub struct ValueChange<T: Send + Sync + 'static> {
    /// The widget whose value changed.
    #[event_target]
    pub source: Entity,
    /// The new value.
    pub value: T,
    /// Whether this ends an interaction (a released drag, Enter) rather
    /// than an intermediate step of one.
    pub is_final: bool,
}

impl<T: Send + Sync + 'static> ValueChange<T> {
    /// A change of `source` to `value`, `is_final` when it ends an
    /// interaction rather than stepping through one.
    #[must_use]
    pub const fn new(source: Entity, value: T, is_final: bool) -> Self {
        Self {
            source,
            value,
            is_final,
        }
    }
}
