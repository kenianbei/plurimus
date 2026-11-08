//! Upstream's `TextInput` on a node, driven through the terminal's keys and
//! mouse: upstream's observers decide what each input means, and these pin
//! that the terminal applies it, draws it and reports it back through
//! `EditableText` as a window would.

use bevy_app::App;
use bevy_color::Color;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::directional_navigation::DirectionalNavigationMap;
use bevy_input_focus::tab_navigation::{TabGroup, TabIndex};
use bevy_input_focus::{FocusedInput, InputFocus};
use bevy_math::CompassOctant;
use bevy_text::{EditableText, TextCursorStyle, TextEdit, TextReadWriteMode};
use bevy_ui::{Node, PositionType, Val};
use plurimus_bui::BuiPlugin;
use plurimus_bui::bevy_ui_widgets::{TextInput, TextInputPlugin};
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::{
    CellSize, CorePlugin, TerminalCamera, TerminalCursor, TerminalSize, UiArea, UiWidget,
};
use plurimus_term::KeyModifiers;
use plurimus_test::{
    character, clipboard_writes, composed_frame, composed_styled_frame, move_to, press_at,
    press_key, press_key_with, release_at, send_paste, set_focus,
};
use plurimus_ui::UiPlugin;

const FIELD_X: u16 = 2;
const FIELD_Y: u16 = 1;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize::new(30, 4));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

/// A focused field `width` cells wide at row 1, column 2, holding `value`,
/// in whatever `CellSize` the app has.
fn field(app: &mut App, width: f32, value: &str) -> Entity {
    let cell = app.world().resource::<CellSize>().vec();
    let root = app
        .world_mut()
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        })
        .id();
    let field = app
        .world_mut()
        .spawn((
            TextInput,
            EditableText::new(value),
            TabIndex(0),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(f32::from(FIELD_X) * cell.x),
                top: Val::Px(f32::from(FIELD_Y) * cell.y),
                width: Val::Px(width * cell.x),
                ..Node::default()
            },
            ChildOf(root),
        ))
        .id();
    set_focus(app, field);
    app.update();
    app.update();
    field
}

fn value(app: &App, field: Entity) -> String {
    app.world()
        .get::<EditableText>(field)
        .unwrap()
        .value()
        .to_string()
}

fn engine(app: &App, field: Entity) -> &plurimus_ui::TextInput {
    app.world().get::<plurimus_ui::TextInput>(field).unwrap()
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press_key(app, character(c));
    }
}

fn row(app: &App) -> String {
    composed_frame(app)
        .lines()
        .nth(usize::from(FIELD_Y))
        .unwrap()
        .to_owned()
}

fn ctrl() -> KeyModifiers {
    KeyModifiers::default().with_ctrl(true)
}

#[test]
fn typing_edits_the_value_and_draws_it_in_the_node() {
    let mut app = app();
    let field = field(&mut app, 10.0, "");
    type_text(&mut app, "ab c");
    assert_eq!(value(&app, field), "ab c");
    assert_eq!(&row(&app)[usize::from(FIELD_X)..][..4], "ab c");
}

#[test]
fn upstreams_keys_move_select_and_delete() {
    let mut app = app();
    let field = field(&mut app, 10.0, "abcde");
    press_key(&mut app, Key::ArrowLeft);
    press_key(&mut app, Key::ArrowLeft);
    press_key_with(
        &mut app,
        Key::ArrowLeft,
        KeyModifiers::default().with_shift(true),
    );
    assert_eq!(engine(&app, field).selection(), Some(2..3));
    press_key(&mut app, Key::Backspace);
    assert_eq!(value(&app, field), "abde");
    press_key_with(&mut app, character('a'), ctrl());
    press_key(&mut app, Key::Delete);
    assert_eq!(value(&app, field), "");
    type_text(&mut app, "one two");
    press_key_with(&mut app, Key::Backspace, ctrl());
    assert_eq!(value(&app, field), "one ");
}

#[derive(Resource, Default)]
struct Seen(Vec<Key>);

#[test]
fn enter_and_escape_reach_the_app_and_escape_blurs() {
    let mut app = app();
    app.init_resource::<Seen>();
    let field = field(&mut app, 10.0, "draft");
    let parent = app.world().get::<ChildOf>(field).unwrap().parent();
    app.world_mut().entity_mut(parent).observe(
        |input: On<FocusedInput<KeyboardInput>>, mut seen: ResMut<Seen>| {
            if input.input.state.is_pressed() {
                seen.0.push(input.input.logical_key.clone());
            }
        },
    );
    press_key(&mut app, Key::Enter);
    press_key(&mut app, character('x'));
    assert_eq!(app.world().resource::<Seen>().0, [Key::Enter]);
    assert_eq!(value(&app, field), "draftx");
    press_key(&mut app, Key::Escape);
    assert_eq!(app.world().resource::<InputFocus>().get(), None);
}

#[test]
fn a_press_places_the_caret_and_a_drag_selects() {
    let mut app = app();
    let field = field(&mut app, 20.0, "hello world");
    press_at(&mut app, FIELD_X + 1, FIELD_Y);
    assert_eq!(engine(&app, field).cursor(), 1);
    move_to(&mut app, FIELD_X + 4, FIELD_Y);
    release_at(&mut app, FIELD_X + 4, FIELD_Y);
    assert_eq!(engine(&app, field).selection(), Some(1..4));
}

#[test]
fn a_press_lands_on_its_cluster_at_any_cell_size() {
    let mut app = app();
    app.insert_resource(CellSize { x: 8.0, y: 16.0 });
    let field = field(&mut app, 20.0, "hello world");
    press_at(&mut app, FIELD_X + 7, FIELD_Y);
    release_at(&mut app, FIELD_X + 7, FIELD_Y);
    assert_eq!(engine(&app, field).cursor(), 7);
}

#[test]
fn a_double_press_selects_the_word() {
    let mut app = app();
    let field = field(&mut app, 20.0, "hello world");
    for _ in 0..2 {
        press_at(&mut app, FIELD_X + 7, FIELD_Y);
        release_at(&mut app, FIELD_X + 7, FIELD_Y);
    }
    assert_eq!(engine(&app, field).selected_text(), Some("world"));
}

#[test]
fn a_long_value_is_windowed_to_keep_the_caret_in_view() {
    let mut app = app();
    field(&mut app, 5.0, "abcdefgh");
    let drawn = &row(&app)[usize::from(FIELD_X)..][..5];
    assert_eq!(drawn, "efgh ");
}

#[test]
fn the_terminal_caret_sits_on_the_focused_field() {
    let mut app = app();
    field(&mut app, 10.0, "abc");
    assert_eq!(
        app.world().resource::<TerminalCursor>().cell,
        Some(Position::new(FIELD_X + 3, FIELD_Y))
    );
    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    assert_eq!(app.world().resource::<TerminalCursor>().cell, None);
}

#[test]
fn copy_offers_the_selection_and_paste_inserts() {
    let mut app = app();
    let field = field(&mut app, 30.0, "copy me");
    press_key_with(&mut app, character('a'), ctrl());
    press_key_with(&mut app, character('c'), ctrl());
    assert_eq!(clipboard_writes(&mut app), ["copy me"]);
    press_key(&mut app, Key::End);
    press_key_with(&mut app, character('v'), ctrl());
    assert_eq!(value(&app, field), "copy mecopy me");
    send_paste(&mut app, "!");
    app.update();
    assert_eq!(value(&app, field), "copy mecopy me!");
}

#[test]
fn the_app_clearing_the_editor_clears_the_field() {
    let mut app = app();
    let field = field(&mut app, 10.0, "draft");
    app.world_mut()
        .get_mut::<EditableText>(field)
        .unwrap()
        .clear();
    app.update();
    assert_eq!(engine(&app, field).value(), "");
    assert_eq!(row(&app).trim(), "");
    type_text(&mut app, "new");
    assert_eq!(value(&app, field), "new");
}

#[test]
fn max_characters_refuses_what_would_pass_it() {
    let mut app = app();
    let field = field(&mut app, 10.0, "");
    app.world_mut()
        .get_mut::<EditableText>(field)
        .unwrap()
        .max_characters = Some(3);
    type_text(&mut app, "abcd");
    assert_eq!(value(&app, field), "abc");
}

#[test]
fn a_read_only_field_moves_but_does_not_edit() {
    let mut app = app();
    let field = field(&mut app, 10.0, "fixed");
    app.world_mut()
        .entity_mut(field)
        .insert(TextReadWriteMode::ReadOnly);
    press_key(&mut app, Key::ArrowLeft);
    type_text(&mut app, "x");
    press_key(&mut app, Key::Backspace);
    assert_eq!(value(&app, field), "fixed");
    assert_eq!(engine(&app, field).cursor(), 4);
}

#[test]
fn a_cell_widget_app_runs_without_upstreams_field() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    assert!(!app.is_plugin_added::<TextInputPlugin>());
}

#[test]
fn the_field_takes_no_part_in_arrow_navigation() {
    let mut app = app();
    let field = field(&mut app, 10.0, "");
    let parent = app.world().get::<ChildOf>(field).unwrap().parent();
    app.world_mut().entity_mut(parent).insert(TabGroup::new(0));
    let widget = app
        .world_mut()
        .spawn((
            UiWidget::default(),
            UiArea::Fixed(Rect::new(20, FIELD_Y, 4, 1)),
            TabIndex(1),
            ChildOf(parent),
        ))
        .id();
    app.update();
    app.update();
    let map = app.world().resource::<DirectionalNavigationMap>();
    assert!(!map.neighbors.contains_key(&field));
    assert_eq!(map.get_neighbor(widget, CompassOctant::West).get(), None);
}

#[test]
fn a_text_cursor_style_colours_the_caret_and_the_selection() {
    let mut app = app();
    let field = field(&mut app, 10.0, "ab");
    app.world_mut().entity_mut(field).insert(TextCursorStyle {
        color: Color::srgb(1.0, 0.0, 0.0),
        selection_color: Color::srgb(0.0, 0.0, 1.0),
        unfocused_selection_color: Color::NONE,
        ..TextCursorStyle::default()
    });
    app.update();
    let caret = "bg:Some(Rgb(255, 0, 0))";
    let selection = "bg:Some(Rgb(0, 0, 255))";
    let frame = composed_styled_frame(&app);
    assert!(frame.contains(caret), "{frame}");
    press_key_with(&mut app, character('a'), ctrl());
    let frame = composed_styled_frame(&app);
    assert!(
        frame.contains(selection) && !frame.contains(caret),
        "{frame}"
    );
    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    app.world_mut()
        .get_mut::<EditableText>(field)
        .unwrap()
        .queue_edit(TextEdit::SelectAll);
    app.update();
    assert_eq!(engine(&app, field).selection(), Some(0..2));
    assert!(!composed_styled_frame(&app).contains(selection));
    app.world_mut()
        .entity_mut(field)
        .remove::<TextCursorStyle>();
    app.update();
    assert!(composed_styled_frame(&app).contains("mods:REVERSED"));
}
