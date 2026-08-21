//! Cluster-correct rendering and cursor highlighting for `EditableText`.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::{Color, Modifier, Style};
use plurimus_core::{CorePlugin, FrameBuffer, TerminalCamera, TerminalRenderApp, TerminalSize};
use plurimus_term::KeyCode;
use plurimus_test::press_key;
use plurimus_ui::{UiArea, UiTheme};
use plurimus_widgets::{WidgetsPlugin, editable_text};

const ACCENT: &str = "e\u{301}";
const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
const WIDTH: u16 = 10;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(WIDTH, 1));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_field(app: &mut App, value: &str) -> Entity {
    let world = app.world_mut();
    let field = world
        .spawn((
            editable_text(value),
            UiArea::Fixed(Rect::new(0, 0, WIDTH, 1)),
        ))
        .id();
    world
        .resource_mut::<InputFocus>()
        .set(field, FocusCause::Pressed);
    field
}

fn frame(app: &mut App) -> Buffer {
    app.update();
    app.sub_app(TerminalRenderApp)
        .world()
        .resource::<FrameBuffer>()
        .0
        .clone()
}

fn symbols(buffer: &Buffer) -> Vec<String> {
    (0..WIDTH)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol().to_owned())
        .collect()
}

fn reversed_columns(buffer: &Buffer) -> Vec<u16> {
    (0..WIDTH)
        .filter(|x| {
            buffer
                .cell((*x, 0))
                .unwrap()
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        })
        .collect()
}

#[test]
fn a_combining_cluster_renders_into_one_cell() {
    let mut app = app();
    spawn_field(&mut app, ACCENT);
    press_key(&mut app, KeyCode::Home);
    let buffer = frame(&mut app);
    assert_eq!(symbols(&buffer)[0], ACCENT, "the accent must survive");
}

#[test]
fn a_zwj_sequence_renders_as_one_symbol() {
    let mut app = app();
    spawn_field(&mut app, FAMILY);
    press_key(&mut app, KeyCode::Home);
    let buffer = frame(&mut app);
    let cells = symbols(&buffer);
    assert_eq!(cells[0], FAMILY, "the whole sequence is one symbol");
    assert_eq!(cells[1], "", "its second column is a continuation");
    assert_eq!(cells[2], " ", "and it occupies only two columns");
}

#[test]
fn the_cursor_spans_a_wide_cluster() {
    let mut app = app();
    spawn_field(&mut app, FAMILY);
    press_key(&mut app, KeyCode::Home);
    let buffer = frame(&mut app);
    assert_eq!(reversed_columns(&buffer), vec![0, 1]);
}

#[test]
fn the_cursor_is_one_cell_on_a_narrow_cluster() {
    let mut app = app();
    spawn_field(&mut app, "ab");
    press_key(&mut app, KeyCode::Home);
    let buffer = frame(&mut app);
    assert_eq!(reversed_columns(&buffer), vec![0]);
}

#[test]
fn a_cursor_past_the_end_highlights_one_blank() {
    let mut app = app();
    spawn_field(&mut app, FAMILY);
    let buffer = frame(&mut app);
    assert_eq!(reversed_columns(&buffer), vec![2]);
}

// A screenful of fields each drawing a block would claim each of them has
// the keys, when only one of them does.
#[test]
fn only_a_focused_field_draws_its_caret() {
    let mut app = app();
    spawn_field(&mut app, "ab");
    press_key(&mut app, KeyCode::Home);
    assert_eq!(reversed_columns(&frame(&mut app)), vec![0]);

    app.world_mut().resource_mut::<InputFocus>().clear();
    let buffer = frame(&mut app);

    assert!(reversed_columns(&buffer).is_empty());
    assert_eq!(symbols(&buffer)[0], "a", "though the value stays drawn");
}

#[test]
fn the_theme_styles_the_caret() {
    let mut app = app();
    spawn_field(&mut app, "ab");
    press_key(&mut app, KeyCode::Home);

    app.insert_resource(UiTheme::new().with_caret(Style::new().fg(Color::Red)));
    let buffer = frame(&mut app);

    assert_eq!(buffer.cell((0, 0)).unwrap().style().fg, Some(Color::Red));
    assert!(
        reversed_columns(&buffer).is_empty(),
        "the theme replaces the default rather than adding to it"
    );
}
