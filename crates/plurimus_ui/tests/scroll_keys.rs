//! Keyboard scrolling of the focused scroll area, driven headlessly
//! through the real key path.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::KeyCode;
use plurimus_test::press_key;
use plurimus_ui::{
    InteractionDisabled, Key, ScrollAction, ScrollArea, ScrollKeys, ScrollOffset, UiArea, UiPlugin,
};

const AREA: Rect = Rect::new(0, 0, 10, 4);
/// Overflows the area on both axes, so a horizontal offset has somewhere
/// to be and is not clamped back to zero by the vertical assertions.
const CONTENT: Size = Size::new(30, 20);
/// `content.height - area.height`, the furthest the offset can travel.
const MAX_ROW: u16 = 16;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(10, 4));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_pane_at(app: &mut App, area: Rect) -> Entity {
    app.world_mut()
        .spawn((
            ScrollArea::new(CONTENT),
            ScrollKeys::default(),
            UiArea::Fixed(area),
        ))
        .id()
}

fn spawn_pane(app: &mut App) -> Entity {
    spawn_pane_at(app, AREA)
}

/// Focus after a frame has run: `bevy_input_focus` hands focus to the
/// virtual window in `PostStartup`, which would overwrite an earlier set.
fn focus(app: &mut App, entity: Entity) {
    app.update();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Pressed);
}

fn offset(app: &App, entity: Entity) -> Position {
    app.world().entity(entity).get::<ScrollOffset>().unwrap().0
}

fn row(app: &App, entity: Entity) -> u16 {
    offset(app, entity).y
}

#[test]
fn page_down_moves_the_offset_by_the_viewport_height() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    focus(&mut app, pane);

    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(row(&app, pane), AREA.height);
}

// A single saturated jump does not exercise this: the clamp has to hold
// on the press that crosses the bound and on every press after it.
#[test]
fn repeated_paging_settles_at_each_bound() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    focus(&mut app, pane);

    for _ in 0..10 {
        press_key(&mut app, KeyCode::PageDown);
    }
    assert_eq!(row(&app, pane), MAX_ROW);

    for _ in 0..10 {
        press_key(&mut app, KeyCode::PageUp);
    }
    assert_eq!(row(&app, pane), 0);
}

#[test]
fn end_and_home_reach_both_extremes_in_one_press() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    focus(&mut app, pane);

    press_key(&mut app, KeyCode::End);
    assert_eq!(row(&app, pane), MAX_ROW);

    press_key(&mut app, KeyCode::Home);
    assert_eq!(row(&app, pane), 0);
}

#[test]
fn a_jump_leaves_the_horizontal_offset_where_it_was() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    focus(&mut app, pane);
    app.world_mut()
        .entity_mut(pane)
        .insert(ScrollOffset(Position::new(3, 0)));

    press_key(&mut app, KeyCode::End);

    assert_eq!(offset(&app, pane), Position::new(3, MAX_ROW));
}

#[test]
fn the_arrows_move_one_row() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    focus(&mut app, pane);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Down);
    assert_eq!(row(&app, pane), 2);

    press_key(&mut app, KeyCode::Up);
    assert_eq!(row(&app, pane), 1);
}

// The neighbor sits in the direction pressed, so an unconsumed arrow has
// somewhere to go and the assertion can tell consumption from inertia.
#[test]
fn a_bound_key_at_an_extreme_is_still_consumed() {
    let mut app = app();
    app.insert_resource(TerminalSize::new(10, 8));
    let above = spawn_pane_at(&mut app, Rect::new(0, 0, 10, 4));
    let pane = spawn_pane_at(&mut app, Rect::new(0, 4, 10, 4));
    focus(&mut app, pane);

    press_key(&mut app, KeyCode::Up);

    assert_eq!(row(&app, pane), 0);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(pane));
    assert_eq!(row(&app, above), 0);
}

#[test]
fn an_unfocused_pane_ignores_the_keys() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    app.update();

    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(row(&app, pane), 0);
}

#[test]
fn a_scroll_area_without_the_component_ignores_the_keys() {
    let mut app = app();
    let pane = app
        .world_mut()
        .spawn((ScrollArea::new(CONTENT), UiArea::Fixed(AREA)))
        .id();
    focus(&mut app, pane);

    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(row(&app, pane), 0);
}

#[test]
fn a_disabled_pane_ignores_the_keys() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    app.world_mut().entity_mut(pane).insert(InteractionDisabled);
    focus(&mut app, pane);

    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(row(&app, pane), 0);
}

#[test]
fn a_remapped_binding_replaces_the_default_one() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    app.world_mut().entity_mut(pane).insert(ScrollKeys(vec![
        (Key::Character("j".into()), ScrollAction::LineDown),
        (Key::Character("G".into()), ScrollAction::Bottom),
    ]));
    focus(&mut app, pane);

    press_key(&mut app, KeyCode::Char('j'));
    assert_eq!(row(&app, pane), 1);

    press_key(&mut app, KeyCode::PageDown);
    assert_eq!(row(&app, pane), 1);
}

#[test]
fn the_horizontal_actions_move_the_column() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    app.world_mut().entity_mut(pane).insert(ScrollKeys(vec![
        (Key::ArrowRight, ScrollAction::LineRight),
        (Key::ArrowLeft, ScrollAction::LineLeft),
    ]));
    focus(&mut app, pane);

    press_key(&mut app, KeyCode::Right);
    press_key(&mut app, KeyCode::Right);
    assert_eq!(offset(&app, pane), Position::new(2, 0));

    press_key(&mut app, KeyCode::Left);
    assert_eq!(offset(&app, pane), Position::new(1, 0));
}

// Pins the require list: dropping TabIndex from it would leave a pane
// that can never be focused, and so never take a key, with nothing else
// failing.
#[test]
fn the_component_makes_the_pane_a_tab_stop() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    app.update();

    assert!(
        app.world()
            .entity(pane)
            .contains::<bevy_input_focus::tab_navigation::TabIndex>()
    );
}
