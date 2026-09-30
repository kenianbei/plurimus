//! Keyboard scrolling of the focused scroll area, driven headlessly
//! through the real key path.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{On, ResMut, Resource};
use bevy_input::ButtonState;
use bevy_input::keyboard::KeyboardInput;
use bevy_input_focus::{FocusedInput, InputFocus};
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{KeyCode, ModifierKey};
use plurimus_test::{press_chord, press_key, set_focus};
use plurimus_ui::{
    InteractionDisabled, Key, KeyBinding, ScrollAction, ScrollArea, ScrollKeys, ScrollOffset,
    UiArea, UiPlugin,
};

const AREA: Rect = Rect::new(0, 0, 10, 4);
/// Overflows the area on both axes, so a horizontal offset has somewhere
/// to be and is not clamped back to zero by the vertical assertions.
const CONTENT: Size = Size::new(30, 20);
/// Rows left once the horizontal bar takes one, which is a page.
const PAGE: u16 = 3;
/// `content.height - PAGE`, the furthest the offset can travel.
const MAX_ROW: u16 = 17;

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
    set_focus(&mut app, pane);

    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(row(&app, pane), PAGE);
}

// A single saturated jump does not exercise this: the clamp has to hold
// on the press that crosses the bound and on every press after it.
#[test]
fn repeated_paging_settles_at_each_bound() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    set_focus(&mut app, pane);

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
    set_focus(&mut app, pane);

    press_key(&mut app, KeyCode::End);
    assert_eq!(row(&app, pane), MAX_ROW);

    press_key(&mut app, KeyCode::Home);
    assert_eq!(row(&app, pane), 0);
}

#[test]
fn a_jump_leaves_the_horizontal_offset_where_it_was() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    set_focus(&mut app, pane);
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
    set_focus(&mut app, pane);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Down);
    assert_eq!(row(&app, pane), 2);

    press_key(&mut app, KeyCode::Up);
    assert_eq!(row(&app, pane), 1);
}

#[test]
fn an_arrow_at_an_extreme_moves_focus_to_the_neighbor() {
    let mut app = app();
    app.insert_resource(TerminalSize::new(10, 8));
    let above = spawn_pane_at(&mut app, Rect::new(0, 0, 10, 4));
    let pane = spawn_pane_at(&mut app, Rect::new(0, 4, 10, 4));
    app.update();
    set_focus(&mut app, pane);

    press_key(&mut app, KeyCode::Up);

    assert_eq!(row(&app, pane), 0);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(above));
    assert_eq!(row(&app, above), 0);
}

#[test]
fn a_pane_its_content_fits_across_passes_the_side_arrows_on() {
    let mut app = app();
    app.insert_resource(TerminalSize::new(20, 4));
    let pane = app
        .world_mut()
        .spawn((
            ScrollArea::new(Size::new(5, 20)),
            ScrollKeys::default(),
            UiArea::Fixed(AREA),
        ))
        .id();
    let beside = spawn_pane_at(&mut app, Rect::new(10, 0, 10, 4));
    app.update();
    set_focus(&mut app, pane);

    press_key(&mut app, KeyCode::Right);

    assert_eq!(offset(&app, pane), Position::ORIGIN);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(beside));
}

#[derive(Resource, Default)]
struct Propagated(Vec<Key>);

#[test]
fn only_a_key_that_moves_nothing_reaches_the_ancestors() {
    let mut app = app();
    app.init_resource::<Propagated>();
    let pane = spawn_pane(&mut app);
    let parent = app.world_mut().spawn_empty().add_child(pane).id();
    app.world_mut().entity_mut(parent).observe(
        |input: On<FocusedInput<KeyboardInput>>, mut seen: ResMut<Propagated>| {
            if input.input.state == ButtonState::Pressed {
                seen.0.push(input.input.logical_key.clone());
            }
        },
    );
    set_focus(&mut app, pane);

    press_key(&mut app, KeyCode::Home);
    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(app.world().resource::<Propagated>().0, [Key::Home]);
    assert_eq!(row(&app, pane), PAGE);
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
    set_focus(&mut app, pane);

    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(row(&app, pane), 0);
}

#[test]
fn a_disabled_pane_ignores_the_keys() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    app.world_mut().entity_mut(pane).insert(InteractionDisabled);
    set_focus(&mut app, pane);

    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(row(&app, pane), 0);
}

#[test]
fn a_remapped_binding_replaces_the_default_one() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    app.world_mut().entity_mut(pane).insert(ScrollKeys(vec![
        (Key::Character("j".into()).into(), ScrollAction::LineDown),
        (Key::Character("G".into()).into(), ScrollAction::Bottom),
    ]));
    set_focus(&mut app, pane);

    press_key(&mut app, KeyCode::Char('j'));
    assert_eq!(row(&app, pane), 1);

    press_key(&mut app, KeyCode::PageDown);
    assert_eq!(row(&app, pane), 1);
}

// Through the real path: the modifier is a key of its own, polled as held
// when the chord's character arrives.
#[test]
fn a_chord_binds_apart_from_its_bare_key() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    app.world_mut().entity_mut(pane).insert(ScrollKeys(vec![
        (
            KeyBinding::new(Key::Character("d".into())).with_ctrl(),
            ScrollAction::PageDown,
        ),
        (Key::Character("d".into()).into(), ScrollAction::LineDown),
    ]));
    set_focus(&mut app, pane);

    press_chord(&mut app, ModifierKey::ControlLeft, KeyCode::Char('d'));
    assert_eq!(row(&app, pane), PAGE, "ctrl-d pages");

    press_key(&mut app, KeyCode::Char('d'));
    assert_eq!(row(&app, pane), PAGE + 1, "a bare d steps one line");
}

#[test]
fn the_side_arrows_move_the_column() {
    let mut app = app();
    let pane = spawn_pane(&mut app);
    set_focus(&mut app, pane);

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
