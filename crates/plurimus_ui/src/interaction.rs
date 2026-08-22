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
use bevy_time::{Real, Time};
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::{
    CameraViewports, ComputedUiCamera, UiArea, UiHidden, UiOrder, UiWidget, resolve_area,
};
use plurimus_term::{CursorCell, MouseButton, MouseKind, MouseMessage, MultiClickWindow};

use crate::click::ClickRun;
use crate::modal::ModalGuard;

/// Whether the cursor is over the widget. Opt-in: spawn it on widgets that
/// should react to the pointer.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Hovered(pub bool);

/// Present while the pointer is pressed on the widget, carrying the click
/// count of the press that set it - see [`PointerPress::count`].
///
/// The count lives here because a gesture outlives the message that started
/// it: the [`Click`] a release completes reports the same number the press
/// did, and a [`PointerDrag`] observer reads it off the entity, a drag
/// through the second press of a run being a different gesture from a drag
/// through the first.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pressed(pub u8);

impl Default for Pressed {
    /// A lone press: what a gesture something other than this router started
    /// counts as.
    fn default() -> Self {
        Self(1)
    }
}

/// Disables all interaction with the widget.
///
/// A disabled widget is inert, not invisible: it still wins press
/// arbitration and absorbs the press - no event, no focus movement, and
/// nothing beneath it is pressed - the way a disabled control blocks a
/// click everywhere else. [`PressPassThrough`] is the opt-out. Wheel ticks
/// are the exception: a disabled widget consumes none, so they fall
/// through, the same as an axis it cannot scroll.
#[derive(Component, Debug, Clone, Copy)]
pub struct InteractionDisabled;

/// Exempts the widget from press hit-testing: a press lands on whatever
/// is beneath it. Presses only - the widget keeps its area for hover, the
/// wheel, and navigation. On a widget with [`InteractionDisabled`], this
/// restores fall-through where the default is to absorb.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PressPassThrough;

/// Keeps a press on the widget from moving focus, while the press itself
/// still lands - [`Pressed`], [`PointerDrag`], [`Click`] all arrive. For a
/// toolbar control beside an editor: tab-reachable through its `TabIndex`,
/// but a click on it leaves the keyboard - and any armed selection - where
/// they were.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PressFocusDisabled;

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
    /// Click count of the press this completes; see
    /// [`PointerPress::count`].
    pub count: u8,
}

impl Click {
    /// A click completed on `entity`, released at `position`, the first of
    /// its run.
    #[must_use]
    pub const fn new(entity: Entity, position: Position) -> Self {
        Self {
            entity,
            position,
            count: 1,
        }
    }

    /// The same click, counted as the `count`th of its run.
    #[must_use]
    pub const fn with_count(mut self, count: u8) -> Self {
        self.count = count;
        self
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
    /// How many presses have run together here: 1 for a lone press, 2 for a
    /// double click, and up.
    ///
    /// Synthesized, since no terminal reports one, from the run of presses
    /// this crate keeps against the app-wide
    /// [`MultiClickWindow`](plurimus_term::MultiClickWindow). A run is this
    /// widget's and this cell's: a press elsewhere, on another widget, or on
    /// none at all starts the next press over at 1. It saturates rather than
    /// wrapping, so what a long run means is this widget's to decide.
    pub count: u8,
}

impl PointerPress {
    /// A press on `entity` at `position`, the first of its run.
    #[must_use]
    pub const fn new(entity: Entity, position: Position) -> Self {
        Self {
            entity,
            position,
            count: 1,
        }
    }

    /// The same press, counted as the `count`th of its run.
    #[must_use]
    pub const fn with_count(mut self, count: u8) -> Self {
        self.count = count;
        self
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
    AreaTargetQuery<'w, 's, (With<Hovered>, Without<PressPassThrough>)>;

type PressedQuery<'w, 's> = Query<'w, 's, (Entity, &'static Pressed, Has<InteractionDisabled>)>;

type FocusableQuery<'w, 's> = Query<'w, 's, (), (With<TabIndex>, Without<PressFocusDisabled>)>;

/// Everything [`pointer_interaction`] routes against.
#[derive(SystemParam)]
pub(crate) struct PointerRouting<'w, 's> {
    targets: PointerTargetQuery<'w, 's>,
    pressed: PressedQuery<'w, 's>,
    focusable: FocusableQuery<'w, 's>,
    disabled: Query<'w, 's, (), With<InteractionDisabled>>,
    modal: ModalGuard<'w, 's>,
    focus: ResMut<'w, InputFocus>,
    run: ResMut<'w, ClickRun>,
    window: Res<'w, MultiClickWindow>,
    time: Res<'w, Time<Real>>,
}

pub(crate) fn pointer_interaction(
    mut mouse: MessageReader<MouseMessage>,
    mut carryover: Local<Vec<MouseMessage>>,
    mut routing: PointerRouting,
    mut commands: Commands,
) {
    let mut pending = std::mem::take(&mut *carryover);
    pending.extend(mouse.read().copied());
    // Entities pressed during this run, with their counts: Pressed lands at
    // command flush, so a same-batch release consults this alongside the
    // query.
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
    run_pressed: &mut Vec<(Entity, u8)>,
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

// A toggle outside the overlays is exempt from dismissal so that pressing
// an opener closes what it opened, rather than the dismissal swallowing
// the press that would have toggled it.
fn route_press(
    position: Position,
    routing: &mut PointerRouting,
    run_pressed: &mut Vec<(Entity, u8)>,
    commands: &mut Commands,
) -> bool {
    let target = topmost_at(position, &routing.targets, |entity| {
        routing.modal.admits(position, entity)
    });
    let inert = target.is_some_and(|entity| routing.disabled.contains(entity));
    // A disabled opener cannot activate, so exempting it would leave the
    // menu open with the press dead.
    let opener = !inert && target.is_some_and(|entity| routing.modal.affects_modality(entity));
    if routing.modal.dismisses(position) && !opener {
        routing.modal.dismiss_all(commands);
        routing.run.reset();
        return true;
    }
    // A press that reaches no widget ends the run: what it dismissed or
    // what absorbed it is not what the next press will land on.
    let Some(entity) = target.filter(|_| !inert) else {
        routing.run.reset();
        return false;
    };
    let count = routing
        .run
        .step(entity, position, routing.time.elapsed(), routing.window.0);
    commands.trigger(PointerPress {
        entity,
        position,
        count,
    });
    press(entity, count, routing, commands);
    run_pressed.push((entity, count));
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
    run_pressed: &[(Entity, u8)],
    position: Position,
    commands: &mut Commands,
) {
    for (entity, _, disabled) in pressed {
        if !disabled {
            commands.trigger(PointerDrag { entity, position });
        }
    }
    for &(entity, _) in run_pressed {
        commands.trigger(PointerDrag { entity, position });
    }
}

fn press(target: Entity, count: u8, routing: &mut PointerRouting, commands: &mut Commands) {
    commands.entity(target).insert(Pressed(count));
    if routing.focusable.contains(target) {
        routing.focus.set(target, FocusCause::Pressed);
    }
}

fn release_all(
    routing: &PointerRouting,
    run_pressed: &mut Vec<(Entity, u8)>,
    position: Position,
    commands: &mut Commands,
) -> bool {
    let held = routing
        .pressed
        .iter()
        .map(|(entity, pressed, disabled)| (entity, pressed.0, disabled));
    // Run-pressed entities cannot be disabled: an absorbed press never
    // reaches `run_pressed`, and nothing disables them mid-run.
    let fresh = run_pressed
        .drain(..)
        .filter(|&(entity, _)| !routing.pressed.contains(entity))
        .map(|(entity, count)| (entity, count, false));
    let mut menu_clicked = false;
    for (entity, count, disabled) in held.chain(fresh) {
        if !disabled {
            commands.trigger(PointerRelease { entity, position });
        }
        // The disabled term keeps a widget disabled mid-gesture from
        // activating: the target query no longer excludes it.
        let released_on = !disabled
            && routing
                .targets
                .get(entity)
                .is_ok_and(|(_, area, _)| area.0.contains(position));
        if released_on {
            commands.trigger(Click {
                entity,
                position,
                count,
            });
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
