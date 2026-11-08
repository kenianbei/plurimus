//! The way a backend reports input: bevy's messages, named for the virtual
//! window, written through one sink.
//!
//! A terminal reports less than a window does - no physical key, no pixel -
//! and more - the modifiers held, as bits on every event. The sink fills the
//! first from the second and the terminal's own facts, and writes what
//! `bevy_winit` would have: the typed message and the same event in the
//! ordered [`WindowEvent`] stream.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{
    Commands, Component, DetectChangesMut, Query, Res, ResMut, Resource, With, World,
};
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::{Local, Single, SystemParam};
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input::mouse::{MouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy_input::touch::TouchPhase;
use bevy_math::Vec2;
use bevy_window::{CursorMoved, PrimaryWindow, Window, WindowEvent, WindowFocused};
use plurimus_core::CellSize;
use plurimus_core::ratatui_core::layout::Position;

use crate::InputCapabilities;
use crate::keyboard::{KeyModifiers, modifier_transitions};
use crate::keymap::physical_code;

/// A cell's centre, so a pointer never sits on an edge two nodes share.
const CELL_CENTER: f32 = 0.5;

/// Ordered phases of input handling in `First`, chained after the clock and
/// the message swap; `plurimus_ui` orders them before `bevy_picking` reads
/// the frame's pointer input.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputSystems {
    /// Backends drain their event source into messages.
    Pump,
    /// The input no terminal will send is written: releases for expired
    /// holds and everything held across a focus loss.
    Synthesize,
}

/// The modifiers the terminal reported with the frame's button press, which
/// bevy's pointer events carry no field for: a shift sent with a click is
/// learned here rather than from the keys held.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PointerModifiers(pub KeyModifiers);

/// Pointer input held back for the next frame.
///
/// A press or a release ends its frame's batch: bevy resolves hover once a
/// frame at the pointer's final position and sends a click against that
/// frame's maps, so what follows waits a frame to be hit-tested at its own
/// cell against the state the click left - the menu a release opened, the
/// widget a press captured. Keys and focus flow.
#[derive(Resource, Debug, Default)]
pub(crate) struct PendingInput {
    waiting: Vec<(WindowEvent, KeyModifiers)>,
    is_batch_ended: bool,
}

/// Where a backend writes what the terminal reported.
///
/// Every call becomes bevy's own message for the primary window -
/// [`KeyboardInput`] with the physical key inferred by [`physical_code`] and
/// `text` for a character press, [`MouseButtonInput`], [`MouseWheel`] and
/// [`WindowFocused`] - written at the end of the system, once as itself and
/// once in the ordered [`WindowEvent`] stream, a press recording its
/// modifiers as [`PointerModifiers`]. The pointer sits at the cell's centre in [`CellSize`] pixels, and
/// a [`CursorMoved`] is written only when the cell changed - before a button
/// or a notch at a cell the pointer never moved to, since a terminal reports
/// a click where it happened and no move ahead of it.
///
/// On a terminal that reports modifiers only as bits
/// ([`InputCapabilities::modifier_keys`] unset) the modifier key presses and
/// releases those bits imply are written ahead of the event that carried
/// them, each naming the left key, so a shift-click holds `ShiftLeft` as a
/// kitty terminal's own shift key would.
#[derive(SystemParam)]
pub struct TerminalInput<'w, 's> {
    window: Single<'w, 's, Entity, With<PrimaryWindow>>,
    cell: Res<'w, CellSize>,
    capabilities: ResMut<'w, InputCapabilities>,
    commands: Commands<'w, 's>,
    held: Local<'s, KeyModifiers>,
    last_cell: Local<'s, Option<Position>>,
}

impl TerminalInput<'_, '_> {
    /// Records what the terminal has proven it reports - `raise` given what
    /// is known so far - so the synthesis of this frame's events sees it.
    pub fn learn(&mut self, raise: impl FnOnce(InputCapabilities) -> InputCapabilities) {
        let known = *self.capabilities;
        self.capabilities.set_if_neq(raise(known));
    }

    /// A key event: `key` went down, repeated or came up with `modifiers`
    /// held. A modifier key is reported as its own key, `Key::Shift` and the
    /// like, and lands on the left side.
    pub fn key(&mut self, key: Key, state: ButtonState, repeat: bool, modifiers: KeyModifiers) {
        self.sync_modifiers(modifiers);
        let text = match &key {
            Key::Character(text) if state == ButtonState::Pressed => Some(text.clone()),
            _ => None,
        };
        let input = KeyboardInput {
            key_code: physical_code(&key),
            logical_key: key,
            state,
            text,
            repeat,
            window: *self.window,
        };
        self.queue(WindowEvent::KeyboardInput(input), modifiers);
    }

    /// A button went down or came up at cell `at`.
    pub fn button(
        &mut self,
        button: MouseButton,
        state: ButtonState,
        at: Position,
        modifiers: KeyModifiers,
    ) {
        self.sync_modifiers(modifiers);
        self.place(at, modifiers);
        let input = MouseButtonInput {
            button,
            state,
            window: *self.window,
        };
        self.queue(WindowEvent::MouseButtonInput(input), modifiers);
    }

    /// The pointer moved to cell `at`, a button held or not; nothing is
    /// written while it stays in its cell.
    pub fn moved(&mut self, at: Position, modifiers: KeyModifiers) {
        self.sync_modifiers(modifiers);
        self.place(at, modifiers);
    }

    /// The wheel turned at cell `at`: `y` is `1` for a notch up and `-1`
    /// for one down, `x` is `1` for a notch to the left and `-1` for one to
    /// the right, as a wheel reports them. A notch is one cell, written in
    /// [`CellSize`] pixels, so a node scrolls a row per notch as a widget
    /// does.
    pub fn wheel(&mut self, at: Position, x: i32, y: i32, modifiers: KeyModifiers) {
        self.sync_modifiers(modifiers);
        self.place(at, modifiers);
        let wheel = MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: x as f32 * self.cell.x,
            y: y as f32 * self.cell.y,
            window: *self.window,
            phase: TouchPhase::Moved,
        };
        self.queue(WindowEvent::MouseWheel(wheel), modifiers);
    }

    /// The terminal gained or lost focus. A loss first releases the modifier
    /// keys the sink wrote, being their one writer.
    pub fn focus(&mut self, focused: bool) {
        if !focused {
            self.sync_modifiers(KeyModifiers::none());
        }
        let focused = WindowFocused {
            window: *self.window,
            focused,
        };
        self.queue(WindowEvent::WindowFocused(focused), KeyModifiers::none());
    }

    fn place(&mut self, at: Position, modifiers: KeyModifiers) {
        if *self.last_cell == Some(at) {
            return;
        }
        *self.last_cell = Some(at);
        let moved = CursorMoved {
            window: *self.window,
            position: pointer_position(at, *self.cell),
            delta: None,
        };
        self.queue(WindowEvent::CursorMoved(moved), modifiers);
    }

    fn sync_modifiers(&mut self, modifiers: KeyModifiers) {
        if self.capabilities.modifier_keys {
            return;
        }
        let was = std::mem::replace(&mut *self.held, modifiers);
        for input in modifier_transitions(was, modifiers, *self.window) {
            self.queue(WindowEvent::KeyboardInput(input), modifiers);
        }
    }

    fn queue(&mut self, event: WindowEvent, modifiers: KeyModifiers) {
        self.commands
            .queue(move |world: &mut World| write_input(world, event, modifiers));
    }
}

/// Where a pointer at cell `at` sits, in pixels: the cell's centre.
fn pointer_position(at: Position, cell: CellSize) -> Vec2 {
    Vec2::new(f32::from(at.x) + CELL_CENTER, f32::from(at.y) + CELL_CENTER) * cell.vec()
}

/// Writes `event` as a backend would: once as itself in the ordered
/// [`WindowEvent`] stream, and once as its own message type, so a reader
/// of one kind and a reader needing the order across kinds both see it. A
/// button press records `modifiers` as [`PointerModifiers`]; a pointer
/// event after the frame's first button waits for the next frame, a press or
/// a release ending its frame's batch.
pub(crate) fn write_input(world: &mut World, event: WindowEvent, modifiers: KeyModifiers) {
    let is_pointer = matches!(
        event,
        WindowEvent::CursorMoved(_) | WindowEvent::MouseButtonInput(_) | WindowEvent::MouseWheel(_)
    );
    let mut pending = world.resource_mut::<PendingInput>();
    if is_pointer && pending.is_batch_ended {
        pending.waiting.push((event, modifiers));
        return;
    }
    match &event {
        WindowEvent::KeyboardInput(input) => {
            world.write_message(input.clone());
        }
        WindowEvent::WindowFocused(focused) => {
            world.write_message(focused.clone());
        }
        WindowEvent::CursorMoved(moved) => {
            world.write_message(moved.clone());
        }
        WindowEvent::MouseButtonInput(input) => {
            pending.is_batch_ended = true;
            if input.state == ButtonState::Pressed {
                world.resource_mut::<PointerModifiers>().0 = modifiers;
            }
            world.write_message(*input);
        }
        WindowEvent::MouseWheel(wheel) => {
            world.write_message(*wheel);
        }
        _ => {}
    }
    world.write_message(event);
}

/// Writes what the last frame held back, ahead of this frame's pump and
/// under the same rule.
pub(crate) fn flush_pending(world: &mut World) {
    if !world.resource::<PendingInput>().is_batch_ended {
        return;
    }
    let mut pending = world.resource_mut::<PendingInput>();
    pending.is_batch_ended = false;
    let waiting = std::mem::take(&mut pending.waiting);
    for (event, modifiers) in waiting {
        write_input(world, event, modifiers);
    }
}

/// The primary window lent to an app that opened none, kept at the
/// terminal's size in [`CellSize`] pixels so whatever reads a window's size
/// reads the terminal's. A window an app opens itself carries none and is
/// never resized.
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(Window, PrimaryWindow)]
pub struct VirtualWindow;

// Every message names a window and the focus dispatch queries one, so an
// app opening no window of its own is lent one; an app's own exists by
// now, WindowPlugin spawning it on build.
pub(crate) fn spawn_virtual_window(
    primary: Query<(), With<PrimaryWindow>>,
    mut commands: Commands,
) {
    if primary.is_empty() {
        commands.spawn(VirtualWindow);
    }
}

#[cfg(test)]
mod tests;
