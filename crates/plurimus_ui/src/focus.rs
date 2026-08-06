//! Adoption of the bevy input-focus stack for windowless terminal apps.
//!
//! bevy_input_focus assumes windows and keyboard events, neither of which a
//! terminal has, so this file supplies both: terminal key messages forward
//! into bevy_input, and a virtual [`PrimaryWindow`] is spawned for focus
//! dispatch to query. [`FocusWithin`] is then maintained on every ancestor of
//! the focused entity, which is what lets a pane highlight its border while
//! something inside it holds focus.

use bevy_app::{App, PreStartup, PreUpdate};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::{Entity, EntityHashSet};
use bevy_ecs::prelude::{ChildOf, Commands, Component, IntoScheduleConfigs, Query, Res, With};
use bevy_input::InputPlugin;
use bevy_input_focus::tab_navigation::TabNavigationPlugin;
use bevy_input_focus::{
    InputDispatchPlugin, InputFocus, InputFocusPlugin, InputFocusSystems, dispatch_focused_input,
};
use bevy_window::{PrimaryWindow, Window};
use plurimus_input::{InputSystems, PasteMessage, bevy_compat};

/// Present on every ancestor of the focused entity, for focus-within
/// styling such as pane border highlights.
#[derive(Component, Debug, Clone, Copy)]
pub struct FocusWithin;

pub(crate) fn install(app: &mut App) {
    if !app.is_plugin_added::<InputPlugin>() {
        app.add_plugins(InputPlugin);
    }
    if !app.is_plugin_added::<InputFocusPlugin>() {
        app.add_plugins((InputFocusPlugin, InputDispatchPlugin, TabNavigationPlugin));
    }
    crate::nav::install(app);
    app.configure_sets(
        PreUpdate,
        InputSystems::Update.before(bevy_input::InputSystems),
    );
    app.add_systems(
        PreUpdate,
        bevy_compat::forward_keyboard.in_set(InputSystems::Update),
    );
    app.add_systems(
        PreUpdate,
        dispatch_focused_input::<PasteMessage>
            .in_set(InputFocusSystems::Dispatch)
            .after(bevy_input::InputSystems),
    );
    app.add_systems(
        PreUpdate,
        sync_focus_within
            .after(InputFocusSystems::Dispatch)
            .after(crate::interaction::pointer_interaction),
    );
    app.add_systems(PreStartup, spawn_virtual_window);
}

pub(crate) fn sync_focus_within(
    focus: Res<InputFocus>,
    parents: Query<&ChildOf>,
    marked: Query<Entity, With<FocusWithin>>,
    mut commands: Commands,
) {
    if !focus.is_changed() {
        return;
    }
    let ancestors: EntityHashSet = focus
        .get()
        .map(|focused| parents.iter_ancestors(focused).collect())
        .unwrap_or_default();
    for entity in &marked {
        if !ancestors.contains(&entity) {
            commands.entity(entity).remove::<FocusWithin>();
        }
    }
    for &entity in &ancestors {
        if !marked.contains(entity) {
            commands.entity(entity).insert(FocusWithin);
        }
    }
}

// The focus stack is inert without a PrimaryWindow entity: dispatch queries
// it and Window's presence terminates event bubbling.
fn spawn_virtual_window(mut commands: Commands) {
    commands
        .spawn((Window::default(), PrimaryWindow))
        .observe(crate::nav::navigate_on_arrows);
}
