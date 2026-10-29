//! The focus stack's window: virtual where the app opens none, the app's own
//! where it does, arrow navigation hanging on whichever it is.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::query::QuerySingleError;
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_window::{PrimaryWindow, Window};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiArea, UiWidget};
use plurimus_term::KeyCode;
use plurimus_test::{press_key, set_focus};
use plurimus_ui::UiPlugin;
use ratatui_widgets::paragraph::Paragraph;

/// Two focusable widgets side by side, under the plugins; `with_window`
/// spawns the app's own primary window first, as `WindowPlugin` does on build.
fn app(with_window: bool) -> (App, Entity, Entity) {
    let mut app = App::new();
    if with_window {
        app.world_mut().spawn((Window::default(), PrimaryWindow));
    }
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(20, 3));
    app.world_mut().spawn(TerminalCamera::default());
    let mut focusable = |rect| {
        app.world_mut()
            .spawn((
                UiWidget::new(Paragraph::new("w")),
                UiArea::Fixed(rect),
                TabIndex(0),
            ))
            .id()
    };
    let left = focusable(Rect::new(0, 1, 6, 1));
    let right = focusable(Rect::new(10, 1, 6, 1));
    (app, left, right)
}

fn primary_window(app: &mut App) -> Result<Entity, QuerySingleError> {
    app.world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
}

fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

#[test]
fn the_apps_own_primary_window_is_adopted_and_navigates() {
    let (mut app, left, right) = app(true);
    let own = primary_window(&mut app).unwrap();
    app.update();

    assert_eq!(primary_window(&mut app).ok(), Some(own), "no second window");
    set_focus(&mut app, left);
    press_key(&mut app, KeyCode::Right);
    assert_eq!(focused(&app), Some(right));
}

#[test]
fn a_virtual_primary_window_is_spawned_where_there_is_none() {
    let (mut app, _, _) = app(false);
    assert!(matches!(
        primary_window(&mut app),
        Err(QuerySingleError::NoEntities(_))
    ));
    app.update();

    assert!(primary_window(&mut app).is_ok());
}
