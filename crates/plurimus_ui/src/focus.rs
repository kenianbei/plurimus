//! Adoption of the bevy input-focus stack for terminal apps.
//!
//! `bevy_input_focus` assumes windows and keyboard events, neither of which a
//! terminal has, so this file supplies both: terminal key messages forward
//! into `bevy_input`, and a virtual [`PrimaryWindow`] is spawned for focus
//! dispatch to query where the app opens no window of its own.

use bevy_app::{App, PreStartup, PreUpdate};
use bevy_ecs::prelude::{Commands, IntoScheduleConfigs, Query, With};
use bevy_input::InputPlugin;
use bevy_input_focus::tab_navigation::TabNavigationPlugin;
use bevy_input_focus::{
    InputDispatchPlugin, InputFocusPlugin, InputFocusSystems, dispatch_focused_input,
};
use bevy_picking::PickingSystems;
use bevy_window::{PrimaryWindow, Window};
use plurimus_term::{InputSystems, PasteMessage, bevy_compat};

use crate::UiSystems;

pub(crate) fn install(app: &mut App) {
    if !app.is_plugin_added::<InputPlugin>() {
        app.add_plugins(InputPlugin);
    }
    if !app.is_plugin_added::<InputFocusPlugin>() {
        app.add_plugins((InputFocusPlugin, InputDispatchPlugin, TabNavigationPlugin));
    }
    crate::nav::install(app);
    // On the set rather than the systems, so a dispatch an app adds
    // inherits the edges; upstream orders only the three it registers.
    app.configure_sets(
        PreUpdate,
        (
            InputSystems::Update.before(bevy_input::InputSystems),
            InputFocusSystems::Dispatch
                .after(bevy_input::InputSystems)
                .after(UiSystems::Areas)
                .before(PickingSystems::Hover),
        ),
    );
    app.add_systems(
        PreUpdate,
        bevy_compat::forward_keyboard.in_set(InputSystems::Update),
    );
    app.add_systems(
        PreUpdate,
        dispatch_focused_input::<PasteMessage>.in_set(InputFocusSystems::Dispatch),
    );
    app.add_systems(PreStartup, spawn_virtual_window);
}

// Dispatch queries one PrimaryWindow and Window's presence ends the bubble;
// an app's own window exists by now, WindowPlugin spawning it on build.
fn spawn_virtual_window(primary: Query<(), With<PrimaryWindow>>, mut commands: Commands) {
    if primary.is_empty() {
        commands.spawn((Window::default(), PrimaryWindow));
    }
}
