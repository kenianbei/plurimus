//! `bevy_picking` over a terminal: the two halves a picking stack expects a
//! platform to supply, so `bevy_ui`'s own backend and the upstream widgets
//! run unchanged.
//!
//! The pointer feed turns mouse messages into `PointerInput` for the one
//! mouse pointer, placed on the primary window `plurimus_ui` keeps for
//! focus, which is the target every terminal camera renders to. The hit
//! gate then drops every node pick at a cell a `plurimus_ui` widget would
//! take the press or the tick at, and all of them while a `plurimus_ui`
//! overlay is open - widgets draw over nodes, so a press reaches one or
//! the other.

use bevy_camera::NormalizedRenderTarget;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{Commands, Local, MessageReader, MessageWriter, Query, Res, ResMut, With};
use bevy_input::mouse::MouseScrollUnit;
use bevy_input::touch::TouchPhase;
use bevy_math::Vec2;
use bevy_picking::backend::PointerHits;
use bevy_picking::pointer::{
    Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
};
use bevy_window::{PrimaryWindow, WindowRef};
use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::{MouseButton, MouseKind, MouseMessage};
use plurimus_ui::PointerTargets;

use super::rect::CellSize;

// A cell's center, so a pointer never sits on an edge two nodes share.
const CELL_CENTER: f32 = 0.5;
// One tick scrolls one row or column.
const TICK: f32 = 1.0;

pub(crate) fn spawn_pointer(mut commands: Commands) {
    commands.spawn(PointerId::Mouse);
}

pub(crate) fn feed_pointer(
    mut messages: MessageReader<MouseMessage>,
    windows: Query<Entity, With<PrimaryWindow>>,
    cell: Res<CellSize>,
    mut last: Local<Vec2>,
    mut inputs: MessageWriter<PointerInput>,
) {
    let Some(target) = windows
        .single()
        .ok()
        .and_then(|window| WindowRef::Entity(window).normalize(None))
        .map(NormalizedRenderTarget::Window)
    else {
        return;
    };
    for message in messages.read() {
        let position = pointer_position(message.position, cell.vec());
        let Some(action) = pointer_action(message.kind, position - *last) else {
            continue;
        };
        let location = Location {
            target: target.clone(),
            position,
        };
        // Only a move places the pointer, and a terminal reports a click
        // or a tick at a cell it never moved to, so one is moved to first.
        if !matches!(action, PointerAction::Move { .. }) && *last != position {
            inputs.write(PointerInput::new(
                PointerId::Mouse,
                location.clone(),
                PointerAction::Move {
                    delta: position - *last,
                },
            ));
        }
        *last = position;
        inputs.write(PointerInput::new(PointerId::Mouse, location, action));
    }
}

fn pointer_position(at: Position, cell: Vec2) -> Vec2 {
    Vec2::new(f32::from(at.x) + CELL_CENTER, f32::from(at.y) + CELL_CENTER) * cell
}

fn pointer_cell(position: Vec2, cell: Vec2) -> Position {
    let at = position / cell;
    Position::new(at.x as u16, at.y as u16)
}

fn pointer_action(kind: MouseKind, delta: Vec2) -> Option<PointerAction> {
    let scroll = |x: f32, y: f32| PointerAction::Scroll {
        unit: MouseScrollUnit::Pixel,
        x,
        y,
        phase: TouchPhase::Moved,
    };
    Some(match kind {
        MouseKind::Down(button) => PointerAction::Press(pointer_button(button)?),
        MouseKind::Up(button) => PointerAction::Release(pointer_button(button)?),
        MouseKind::Moved | MouseKind::Drag(_) => PointerAction::Move { delta },
        MouseKind::ScrollUp => scroll(0.0, TICK),
        MouseKind::ScrollDown => scroll(0.0, -TICK),
        MouseKind::ScrollLeft => scroll(TICK, 0.0),
        MouseKind::ScrollRight => scroll(-TICK, 0.0),
        MouseKind::Cancel => PointerAction::Cancel,
        _ => return None,
    })
}

const fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        _ => None,
    }
}

pub(crate) fn gate_hits(
    mut hits: ResMut<Messages<PointerHits>>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    cell: Res<CellSize>,
    widgets: PointerTargets,
) {
    if hits.is_empty() {
        return;
    }
    let covered: Vec<PointerId> = pointers
        .iter()
        .filter(|(_, location)| {
            location
                .location()
                .is_some_and(|location| widgets.covers(pointer_cell(location.position, cell.vec())))
        })
        .map(|(pointer, _)| *pointer)
        .collect();
    if covered.is_empty() {
        return;
    }
    let kept: Vec<PointerHits> = hits
        .drain()
        .filter(|hit| !covered.contains(&hit.pointer))
        .collect();
    hits.write_batch(kept);
}
