//! The terminal's pointer on `bevy_picking`: the backend that reports the
//! widgets under the pointer, and the observers that turn bevy's pointer
//! events into a widget gesture. The pointer itself is bevy's, fed by
//! `PointerInputPlugin` from the window events the terminal backend writes.
//!
//! Widgets are hit by screen rect, one `PointerHits` per terminal camera at
//! an order just above `bevy_ui`'s nodes on that camera, so a widget stands
//! over a node and `Pickable::should_block_lower` hides what is beneath.
//! Outside every open overlay the one hit is the [`ModalBarrier`], above
//! everything, so a press or a tick there dismisses and reaches nothing.

use bevy_camera::Camera;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageCursor;
use bevy_ecs::prelude::{
    ChildOf, Commands, Component, Has, Local, MessageReader, MessageWriter, Messages, On, Query,
    Res, ResMut, Resource, With, Without,
};
use bevy_ecs::system::SystemParam;
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusCause, InputFocus, InputFocusVisible};
use bevy_math::{Vec2, Vec3};
use bevy_picking::backend::{HitData, PointerHits};
use bevy_picking::events::{Pointer, PointerCancel, PointerClick, PointerPress, PointerRelease};
use bevy_picking::hover::PointerCaptureMap;
use bevy_picking::pointer::{Location, PointerAction, PointerId, PointerInput, PointerLocation};
use bevy_ui_widgets::{Button, Checkbox, RadioButton};
use bevy_window::{Window, WindowFocused};
use plurimus_core::ratatui_core::layout::Position;
use plurimus_core::{
    CellSize, ComputedUiCamera, DefaultCamera, ResolvedViewport, TerminalCamera, UiOrder,
};

use crate::interaction::{
    ComputedDisabled, ComputedWidgetArea, Hovered, PressFocusDisabled, Pressed, z_order_key,
};
use crate::modal::{ModalGuard, ModalOpen, ModalityToggle};

/// Widgets stand this far above `bevy_ui`'s nodes on the same camera, which
/// sit at `camera.order + 0.5`.
const WIDGET_LAYER: f32 = 0.75;

/// The one entity picked outside every open overlay: a press on it
/// dismisses them, and nothing beneath it is reached. It refuses focus as
/// any `PressFocusDisabled` entity does.
#[derive(Component, Debug, Clone, Copy)]
#[require(PressFocusDisabled)]
pub(crate) struct ModalBarrier;

/// The cell a pointer is on, from its position in [`CellSize`] pixels.
#[must_use]
pub fn pointer_cell(pointer: &Pointer, cell: CellSize) -> Position {
    cell_of(pointer.position, cell)
}

/// The cell a click was released on, when that is inside `area`: the rule a
/// widget applies to decide whether a captured gesture completed on it.
#[must_use]
pub fn click_cell(
    click: &PointerClick,
    area: &ComputedWidgetArea,
    cell: CellSize,
) -> Option<Position> {
    let position = pointer_cell(&click.pointer, cell);
    area.0.contains(position).then_some(position)
}

pub(crate) fn cell_of(position: Vec2, cell: CellSize) -> Position {
    let scaled = position / cell.vec();
    Position::new(scaled.x as u16, scaled.y as u16)
}

pub(crate) fn spawn_barrier(mut commands: Commands) {
    commands
        .spawn(ModalBarrier)
        .observe(dismiss_on_barrier_press);
}

/// A cancel held back to the frame after a press or a release it followed:
/// the hover map is emptied for a cancelled pointer before its events run,
/// so a cancel landing with a press would swallow the press it should end.
#[derive(Resource, Debug, Default)]
pub(crate) struct DeferredCancel(Option<Location>);

/// Ends the mouse pointer's gesture when the terminal reports losing focus:
/// one [`PointerAction::Cancel`] at the cell the pointer last reached - this
/// frame's, ahead of the location picking resolves later - written now, or
/// after a press or a release in the same frame deferred to the frame's end
/// by [`DeferredCancel`], where it lands ahead of what the press held back.
/// Before any pointer has reported there is nothing to cancel. bevy's own
/// focus-loss handling reaches the keyboard alone.
///
/// Reading and writing one message type through two system params
/// conflicts, so it reads through a cursor of its own, advanced every frame.
pub(crate) fn cancel_on_focus_loss(
    mut focus: MessageReader<WindowFocused>,
    mut pumped: Local<MessageCursor<PointerInput>>,
    mut inputs: ResMut<Messages<PointerInput>>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    mut deferred: ResMut<DeferredCancel>,
) {
    if !focus.read().any(|message| !message.focused) {
        pumped.clear(&inputs);
        return;
    }
    let (mut last, mut ends_batch) = (None, false);
    for input in pumped.read(&inputs) {
        if input.pointer_id != PointerId::Mouse {
            continue;
        }
        last = Some(&input.location);
        ends_batch |= matches!(
            input.action,
            PointerAction::Press(_) | PointerAction::Release(_)
        );
    }
    let location = last.cloned().or_else(|| {
        pointers
            .iter()
            .find(|(id, _)| **id == PointerId::Mouse)
            .and_then(|(_, location)| location.location().cloned())
    });
    match location {
        Some(location) if ends_batch => deferred.0 = Some(location),
        Some(location) => {
            inputs.write(cancel_at(location));
        }
        None => {}
    }
}

pub(crate) fn flush_deferred_cancel(
    mut deferred: ResMut<DeferredCancel>,
    mut inputs: MessageWriter<PointerInput>,
) {
    if let Some(location) = deferred.0.take() {
        inputs.write(cancel_at(location));
    }
}

fn cancel_at(location: Location) -> PointerInput {
    PointerInput::new(PointerId::Mouse, location, PointerAction::Cancel)
}

/// The step a scroll input asks for, in cells: a notch down is a negative
/// `y` on a wheel and a positive row to a scroll, one per notch, since the
/// terminal backends report a wheel one notch at a time.
pub(crate) const fn scroll_step(action: PointerAction) -> Option<(i32, i32)> {
    match action {
        PointerAction::Scroll { x, y, .. } => Some((cells_of(x), cells_of(y))),
        _ => None,
    }
}

const fn cells_of(delta: f32) -> i32 {
    (delta < 0.0) as i32 - (delta > 0.0) as i32
}

type Target = (
    Entity,
    &'static ComputedWidgetArea,
    &'static ComputedUiCamera,
    Option<&'static UiOrder>,
);

type LiveToggle = (
    With<Hovered>,
    With<ModalityToggle>,
    Without<ComputedDisabled>,
);

/// Everything the backend hit-tests against.
#[derive(SystemParam)]
pub(crate) struct Picking<'w, 's> {
    pointers: Query<'w, 's, &'static PointerLocation, With<PointerId>>,
    cell: Res<'w, CellSize>,
    default_camera: Res<'w, DefaultCamera>,
    cameras:
        Query<'w, 's, (Entity, &'static Camera, &'static ResolvedViewport), With<TerminalCamera>>,
    targets: Query<'w, 's, Target, With<Hovered>>,
    toggles: Query<'w, 's, Target, LiveToggle>,
    modal: ModalGuard<'w, 's>,
    barrier: Query<'w, 's, Entity, With<ModalBarrier>>,
}

impl Picking<'_, '_> {
    // A disabled opener cannot activate, so exempting it would leave the
    // menu open with the press dead.
    fn outside(&self, at: Position) -> Option<PointerHits> {
        let toggle = self
            .toggles
            .iter()
            .filter(|(_, area, ..)| area.0.contains(at))
            .max_by_key(|(entity, area, _, order)| z_order_key(*entity, *order, area.0));
        let (entity, camera, order) = if let Some((entity, _, camera, _)) = toggle {
            let camera = camera.0?;
            let order = self
                .cameras
                .get(camera)
                .map_or(WIDGET_LAYER, |(_, settings, _)| layer(settings.order));
            (entity, camera, order)
        } else {
            (
                self.barrier.single().ok()?,
                self.default_camera.0?,
                f32::INFINITY,
            )
        };
        Some(PointerHits::new(
            PointerId::Mouse,
            vec![(entity, hit(camera, 0.0, at))],
            order,
        ))
    }

    fn on_camera(&self, camera: Entity, at: Position) -> Vec<(Entity, HitData)> {
        let mut picks: Vec<_> = self
            .targets
            .iter()
            .filter(|(entity, area, target, ..)| {
                target.0 == Some(camera) && area.0.contains(at) && self.modal.admits(at, *entity)
            })
            .map(|(entity, area, _, order)| (z_order_key(entity, order, area.0), entity))
            .collect();
        picks.sort_unstable_by_key(|(key, _)| core::cmp::Reverse(*key));
        picks
            .into_iter()
            .enumerate()
            .map(|(depth, (_, entity))| (entity, hit(camera, depth as f32, at)))
            .collect()
    }
}

/// The picking backend: the widgets under the pointer, per camera, and the
/// barrier alone outside every open overlay.
pub(crate) fn pick_widgets(picking: Picking, mut hits: MessageWriter<PointerHits>) {
    let Some(position) = picking
        .pointers
        .iter()
        .find_map(|location| location.location().map(|location| location.position))
    else {
        return;
    };
    let at = cell_of(position, *picking.cell);
    if picking.modal.dismisses(at) {
        if let Some(outside) = picking.outside(at) {
            hits.write(outside);
        }
        return;
    }
    for (camera, settings, viewport) in &picking.cameras {
        if !viewport.0.contains(at) {
            continue;
        }
        let picks = picking.on_camera(camera, at);
        if !picks.is_empty() {
            hits.write(PointerHits::new(
                PointerId::Mouse,
                picks,
                layer(settings.order),
            ));
        }
    }
}

fn hit(camera: Entity, depth: f32, at: Position) -> HitData {
    let position = Vec3::new(f32::from(at.x), f32::from(at.y), 0.0);
    HitData::new(camera, depth, Some(position), None)
}

fn layer(order: isize) -> f32 {
    order as f32 + WIDGET_LAYER
}

/// The widgets `bevy_ui_widgets` observes: they press, release and click
/// by bevy's hover map as a node does, so a release off one reaches what
/// is under the pointer, and their [`Pressed`] is upstream's to insert.
type Uncaptured = (Without<Button>, Without<Checkbox>, Without<RadioButton>);

/// A press on a widget captures the pointer to it for the gesture, so a
/// drag past its edge still ends on it, and marks it [`Pressed`]. A
/// disabled widget blocks what is beneath, since it is a hit, and emits
/// nothing further: its press goes no higher.
pub(crate) fn capture_press(
    mut press: On<PointerPress>,
    widgets: Query<Has<ComputedDisabled>, (With<ComputedWidgetArea>, Uncaptured)>,
    mut capture: ResMut<PointerCaptureMap>,
    mut commands: Commands,
) {
    if press.entity != press.original_event_target() {
        return;
    }
    let Ok(disabled) = widgets.get(press.entity) else {
        return;
    };
    if disabled {
        press.propagate(false);
        return;
    }
    capture.capture(press.pointer.id, press.entity, press.hit.clone());
    commands.entity(press.entity).insert(Pressed);
}

pub(crate) fn release_press(
    release: On<PointerRelease>,
    widgets: Query<(), With<ComputedWidgetArea>>,
    mut commands: Commands,
) {
    if release.entity == release.original_event_target() && widgets.contains(release.entity) {
        commands.entity(release.entity).try_remove::<Pressed>();
    }
}

/// What a press's focus walk reads: whether the target is one a press
/// moves no focus for, and at each step up whether the entity can take
/// focus, refuses a press's focus, or roots an open overlay.
#[derive(SystemParam)]
pub(crate) struct FocusWalk<'w, 's> {
    skipped: Query<'w, 's, (), With<ComputedDisabled>>,
    windows: Query<'w, 's, (), With<Window>>,
    stops: Query<'w, 's, (Has<TabIndex>, Has<PressFocusDisabled>, Has<ModalOpen>)>,
    parents: Query<'w, 's, &'static ChildOf>,
}

/// Focus follows a press to the first `TabIndex` at or above what it
/// landed on, widget or node alike; `PressFocusDisabled` and an open
/// overlay's root stop the walk and keep focus, and a walk that finds none,
/// or a press on the window itself, clears it.
pub(crate) fn press_focus(
    press: On<PointerPress>,
    walk: FocusWalk,
    mut focus: ResMut<InputFocus>,
    mut visible: ResMut<InputFocusVisible>,
) {
    let target = press.entity;
    if target != press.original_event_target() || walk.skipped.contains(target) {
        return;
    }
    if visible.0 {
        visible.0 = false;
    }
    if walk.windows.contains(target) {
        focus.clear();
        return;
    }
    for entity in std::iter::once(target).chain(walk.parents.iter_ancestors(target)) {
        let Ok((focusable, refuses, overlay)) = walk.stops.get(entity) else {
            return;
        };
        if refuses || overlay {
            return;
        }
        if focusable {
            focus.set(entity, FocusCause::Pressed);
            return;
        }
    }
    focus.clear();
}

/// A press on an opener outside the overlays dismisses only what it does
/// not own, so pressing an opener closes what it opened - by the press that
/// toggles it, which a dismissal would have swallowed - and pressing another
/// switches.
pub(crate) fn toggle_dismiss(
    press: On<PointerPress>,
    toggles: Query<(), (With<ModalityToggle>, Without<ComputedDisabled>)>,
    cell: Res<CellSize>,
    modal: ModalGuard,
    mut commands: Commands,
) {
    if press.entity != press.original_event_target() || !toggles.contains(press.entity) {
        return;
    }
    if modal.dismisses(pointer_cell(&press.pointer, *cell)) {
        modal.dismiss_unowned(press.entity, &mut commands);
    }
}

fn dismiss_on_barrier_press(
    mut press: On<PointerPress>,
    modal: ModalGuard,
    mut commands: Commands,
) {
    press.propagate(false);
    modal.dismiss_all(&mut commands);
}

/// bevy's own cancel reaches nothing: the hover map is emptied for a
/// cancelled pointer before its events run. So a cancel ends the gesture
/// from here, with the event bevy would have sent: to the captured widget,
/// the capture being the gesture, or with no capture to every pressed
/// widget, which is how an uncaptured upstream widget's ends.
pub(crate) fn cancel_gestures(
    mut inputs: MessageReader<PointerInput>,
    capture: Res<PointerCaptureMap>,
    pressed: Query<(Entity, &ComputedUiCamera), (With<Pressed>, With<ComputedWidgetArea>)>,
    mut commands: Commands,
) {
    for input in inputs.read() {
        if !matches!(input.action, PointerAction::Cancel) {
            continue;
        }
        let pointer = Pointer::new(input.pointer_id, input.location.clone());
        if let Some((entity, hit)) = capture.get(&input.pointer_id) {
            cancel(entity, &pointer, hit.clone(), &mut commands);
            continue;
        }
        for (entity, camera) in &pressed {
            let Some(camera) = camera.0 else {
                continue;
            };
            let hit = HitData::new(camera, 0.0, None, None);
            cancel(entity, &pointer, hit, &mut commands);
        }
    }
}

fn cancel(entity: Entity, pointer: &Pointer, hit: HitData, commands: &mut Commands) {
    commands.trigger(PointerCancel {
        entity,
        pointer: pointer.clone(),
        hit,
    });
    commands.entity(entity).remove::<Pressed>();
}
