//! A todo list on one `bevy_ui` tree of upstream `bevy_ui_widgets`, run in
//! the terminal or, with `--desktop`, in a window.
//!
//! The tree states structure only - which nodes exist, how they nest, which
//! way they flow, what text they carry - and tags each node with a `Part`.
//! Each skin answers `Part` with its own sizes, borders and colours: cells
//! and box-drawing in the terminal, pixels, a card and pills in the window.
//! Buttons and checkboxes are bevy's own, picked by `bevy_picking` through
//! the terminal's mouse or the window's; the ledger is a `Display::Grid` of
//! unwrapped cells with right-justified ids; and the field is the one control
//! each target supplies for itself - a plurimus `EditableText` placed over
//! its node in the terminal, where upstream's `TextInput` has no font engine
//! to edit through, and that `TextInput` itself in the window. Tab moves
//! focus, Enter or Space activates, the mouse hovers and clicks. In
//! the terminal Esc unfocuses and `q` with nothing focused or ctrl-c quits;
//! in the window Esc quits.
//!
//! Laid out for a terminal of roughly 60x16 or larger, or a 720x640 window.

mod desktop;
mod model;
mod skin;
mod tree;

#[cfg(test)]
mod tests;

use std::time::Duration;

use bevy_app::{App, AppExit, PostUpdate, ScheduleRunnerPlugin, Startup, Update};
use bevy_ecs::component::Mutable;
use bevy_ecs::prelude::{
    Commands, Component, DetectChangesMut, Entity, IntoScheduleConfigs, MessageReader,
    MessageWriter, Mut, ResMut,
};
use bevy_ui::UiSystems;
use plurimus::bui::BuiPlugin;
use plurimus::core::{CorePlugin, TerminalCamera};
use plurimus::crossterm::CrosstermPlugin;
use plurimus::term::{KeyCode, KeyKind, KeyMessage};
use plurimus::ui::bevy_input_focus::InputFocus;
use plurimus::widgets::WidgetsPlugin;

use model::{Filter, Todos};

const FRAME: Duration = Duration::from_millis(16);

fn main() -> AppExit {
    if std::env::args().any(|argument| argument == "--desktop") {
        return desktop::run();
    }
    let mut app = App::new();
    app.add_plugins((
        ScheduleRunnerPlugin::run_loop(FRAME),
        CorePlugin,
        CrosstermPlugin::default(),
    ));
    add_terminal(&mut app);
    app.run()
}

/// The model and the tree, which both targets run unchanged.
fn add_shared(app: &mut App) {
    app.insert_resource(Todos::seeded())
        .init_resource::<Filter>()
        .add_systems(Startup, tree::spawn_tree)
        .add_systems(
            Update,
            (
                model::sync_rows,
                model::sync_marks,
                model::sync_count,
                model::sync_filters,
            )
                .chain(),
        );
}

fn add_terminal(app: &mut App) {
    add_shared(app);
    app.add_plugins((BuiPlugin, WidgetsPlugin))
        .add_observer(skin::shape)
        .add_systems(Startup, spawn_camera)
        .add_systems(Update, quit_on_key)
        .add_systems(
            PostUpdate,
            (
                skin::paint.before(UiSystems::Content),
                skin::place_field.after(UiSystems::PostLayout),
            ),
        );
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(TerminalCamera::default());
}

/// The facts a skin draws a control from.
#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the five facts a control is drawn from, read together"
)]
struct State {
    is_hovered: bool,
    is_pressed: bool,
    is_focused: bool,
    is_checked: bool,
    is_done: bool,
}

/// Writes a look's component where it differs, inserting it the first time;
/// an insert of an equal value would still mark it changed and redraw the node.
fn write<T: Component<Mutability = Mutable> + PartialEq>(
    entity: Entity,
    current: Option<Mut<T>>,
    value: T,
    commands: &mut Commands,
) {
    match current {
        Some(mut current) => {
            current.set_if_neq(value);
        }
        None => {
            commands.entity(entity).insert(value);
        }
    }
}

fn toggle<T: Component>(entity: Entity, wanted: Option<T>, has: bool, commands: &mut Commands) {
    match wanted {
        Some(value) if !has => {
            commands.entity(entity).insert(value);
        }
        None if has => {
            commands.entity(entity).remove::<T>();
        }
        _ => {}
    }
}

fn quit_on_key(
    mut keys: MessageReader<KeyMessage>,
    mut focus: ResMut<InputFocus>,
    mut exit: MessageWriter<AppExit>,
) {
    for key in keys.read() {
        if key.kind != KeyKind::Press {
            continue;
        }
        if key.modifiers.ctrl && key.code == KeyCode::Char('c') {
            exit.write(AppExit::Success);
            continue;
        }
        match key.code {
            KeyCode::Esc => focus.clear(),
            KeyCode::Char('q') if focus.get().is_none() => {
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}
