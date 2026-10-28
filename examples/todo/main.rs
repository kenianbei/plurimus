//! A todo list on one `bevy_ui` tree of upstream `bevy_ui_widgets`, skinned
//! for the terminal.
//!
//! The tree states structure only - which nodes exist, how they nest, which
//! way they flow, what text they carry - and tags each node with a `Part`.
//! The skin answers `Part` with sizes, borders and colours, so the same tree
//! under a desktop skin is the same app in a window. Buttons and checkboxes
//! are bevy's own, picked by `bevy_picking` through the terminal's mouse; the
//! ledger is a `Display::Grid` of unwrapped cells with right-justified ids;
//! and the field is the one control each target supplies for itself - here a
//! plurimus `EditableText` placed over its node, since upstream's `TextInput`
//! edits through a font engine. Tab moves focus, Enter or Space activates,
//! the mouse hovers and clicks, Esc unfocuses, `q` with nothing focused or
//! ctrl-c quits.
//!
//! Laid out for a terminal of roughly 60x16 or larger.

mod model;
mod skin;
mod tree;

#[cfg(test)]
mod tests;

use std::time::Duration;

use bevy_app::{App, AppExit, PostUpdate, ScheduleRunnerPlugin, Startup, Update};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, MessageReader, MessageWriter, ResMut};
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
    let mut app = App::new();
    app.add_plugins((
        ScheduleRunnerPlugin::run_loop(FRAME),
        CorePlugin,
        CrosstermPlugin::default(),
    ));
    add_demo(&mut app);
    app.run()
}

fn add_demo(app: &mut App) {
    app.add_plugins((BuiPlugin, WidgetsPlugin))
        .insert_resource(Todos::seeded())
        .init_resource::<Filter>()
        .add_observer(skin::shape)
        .add_systems(Startup, (spawn_camera, tree::spawn_tree))
        .add_systems(
            Update,
            (
                model::sync_rows,
                model::sync_marks,
                model::sync_count,
                model::sync_filters,
                quit_on_key,
            ),
        )
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
