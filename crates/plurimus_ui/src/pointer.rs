//! The pointer router: mouse messages to press, drag, release, click and
//! cancel events on the widgets they reach.
//!
//! Modal state can swallow a hit entirely, which is why a batch of messages
//! is routed one at a time instead of resolved together.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Has, Local, MessageReader, Query, Res, ResMut, With, Without};
use bevy_ecs::system::SystemParam;
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusCause, InputFocus};
use bevy_time::{Real, Time};
use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::{MouseButton, MouseKind, MouseMessage, MultiClickWindow};

use crate::click::ClickRun;
use crate::interaction::{
    AreaTargetQuery, Click, Hovered, InteractionDisabled, PointerCancel, PointerDrag, PointerPress,
    PointerRelease, PressFocusDisabled, PressPassThrough, Pressed, topmost_at,
};
use crate::modal::ModalGuard;
use crate::scroll::{WheelRouting, route_tick};

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
    wheel: WheelRouting<'w, 's>,
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

// Returns true when the message flipped modality; the rest of the
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
        MouseKind::Cancel => {
            cancel_all(routing, run_pressed, commands);
            false
        }
        _ => route_tick(message, &routing.wheel, &routing.modal, commands),
    }
}

// A toggle outside the overlays dismisses only what it does not own, so
// pressing an opener closes what it opened - by the press that toggles it,
// which a dismissal would have swallowed - and pressing another switches.
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
    let opener = target.filter(|&entity| !inert && routing.modal.affects_modality(entity));
    if routing.modal.dismisses(position) {
        let Some(opener) = opener else {
            routing.modal.dismiss_all(commands);
            routing.run.reset();
            return true;
        };
        routing.modal.dismiss_unowned(opener, commands);
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

/// Every gesture in flight, as `(entity, count, disabled)`: the settled
/// [`Pressed`] widgets, then this batch's presses whose `Pressed` has not
/// landed yet.
fn gestures<'a>(
    pressed: &'a PressedQuery,
    run_pressed: &'a mut Vec<(Entity, u8)>,
) -> impl Iterator<Item = (Entity, u8, bool)> + 'a {
    let held = pressed
        .iter()
        .map(|(entity, pressed, disabled)| (entity, pressed.0, disabled));
    // Run-pressed entities cannot be disabled: an absorbed press never
    // reaches `run_pressed`, and nothing disables them mid-run.
    let fresh = run_pressed
        .drain(..)
        .filter(|&(entity, _)| !pressed.contains(entity))
        .map(|(entity, count)| (entity, count, false));
    held.chain(fresh)
}

fn release_all(
    routing: &PointerRouting,
    run_pressed: &mut Vec<(Entity, u8)>,
    position: Position,
    commands: &mut Commands,
) -> bool {
    let mut menu_clicked = false;
    for (entity, count, disabled) in gestures(&routing.pressed, run_pressed) {
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

// The run ends too: a press after focus returns is not the second of one
// made before it left.
fn cancel_all(
    routing: &mut PointerRouting,
    run_pressed: &mut Vec<(Entity, u8)>,
    commands: &mut Commands,
) {
    for (entity, _, disabled) in gestures(&routing.pressed, run_pressed) {
        if !disabled {
            commands.trigger(PointerCancel { entity });
        }
        commands.entity(entity).remove::<Pressed>();
    }
    routing.run.reset();
}
