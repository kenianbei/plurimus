use bevy_ecs::prelude::Entity;
use bevy_ui::Checked;
use plurimus::bui::ComputedNodeRect;
use plurimus::core::TerminalSize;
use plurimus::widgets::TextInput;
use plurimus_test::{click, composed_frame, press_key, set_focus};

use super::*;
use model::{Part, TodoId};

const TEST_SIZE: TerminalSize = TerminalSize::new(60, 16);

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    app.insert_resource(TEST_SIZE);
    add_terminal(&mut app);
    for _ in 0..3 {
        app.update();
    }
    app
}

fn settle(app: &mut App) {
    app.update();
    app.update();
}

/// The entity of `part`, for the todo `id` when the part is a ledger cell.
fn part(app: &mut App, wanted: Part, id: Option<u32>) -> Entity {
    let world = app.world_mut();
    let mut parts = world.query::<(Entity, &Part, Option<&TodoId>)>();
    parts
        .iter(world)
        .find(|(_, part, todo)| **part == wanted && todo.map(|todo| todo.0) == id)
        .map_or_else(|| panic!("no {wanted:?} for {id:?}"), |(entity, ..)| entity)
}

fn rows(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut parts = world.query::<&Part>();
    parts
        .iter(world)
        .filter(|part| **part == Part::Check)
        .count()
}

fn click_center(app: &mut App, entity: Entity) {
    let rect = app.world().get::<ComputedNodeRect>(entity).unwrap().rect;
    click(app, rect.x + rect.width / 2, rect.y + rect.height / 2);
}

fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

#[test]
fn the_ledger_opens_with_the_seeded_todos_in_aligned_columns() {
    let app = headless_app();

    let frame = composed_frame(&app);
    let marks: Vec<&str> = frame
        .lines()
        .filter(|line| line.contains("Try the") || line.contains("Compare"))
        .collect();
    assert_eq!(marks.len(), 3, "{frame}");
    assert!(marks[0].contains("[x] Try the terminal skin"), "{frame}");
    assert!(marks[1].contains("[ ] Try the desktop skin"), "{frame}");
    let id_columns: Vec<usize> = marks.iter().map(|line| line.find('#').unwrap()).collect();
    assert!(
        id_columns.iter().all(|column| column == &id_columns[0]),
        "ids right-align in one column: {frame}"
    );
    assert!(frame.contains("2 items left"), "{frame}");
}

#[test]
fn clicking_a_checkbox_toggles_it_through_bevy_picking() {
    let mut app = headless_app();
    let check = part(&mut app, Part::Check, Some(2));
    assert!(app.world().get::<Checked>(check).is_none());

    click_center(&mut app, check);
    settle(&mut app);

    assert!(app.world().get::<Checked>(check).is_some());
    let frame = composed_frame(&app);
    assert!(frame.contains("[x] Try the desktop skin"), "{frame}");
    assert!(frame.contains("1 item left"), "{frame}");
    assert_eq!(
        focused(&app),
        Some(check),
        "a click focuses what it toggles"
    );
}

#[test]
fn typing_into_the_field_and_enter_adds_a_todo() {
    let mut app = headless_app();
    let field = part(&mut app, Part::Field, None);
    set_focus(&mut app, field);
    for letter in "milk".chars() {
        press_key(&mut app, KeyCode::Char(letter));
    }
    press_key(&mut app, KeyCode::Enter);
    settle(&mut app);

    assert_eq!(rows(&mut app), 4);
    let frame = composed_frame(&app);
    assert!(frame.contains("[ ] milk"), "{frame}");
    assert!(frame.contains("#4"), "{frame}");
    assert_eq!(app.world().get::<TextInput>(field).unwrap().value(), "");
}

#[test]
fn the_add_button_submits_the_field() {
    let mut app = headless_app();
    let field = part(&mut app, Part::Field, None);
    set_focus(&mut app, field);
    for letter in "eggs".chars() {
        press_key(&mut app, KeyCode::Char(letter));
    }
    let add = part(&mut app, Part::AddButton, None);
    click_center(&mut app, add);
    settle(&mut app);

    assert_eq!(rows(&mut app), 4);
    assert!(composed_frame(&app).contains("[ ] eggs"));
    assert_eq!(app.world().get::<TextInput>(field).unwrap().value(), "");
}

#[test]
fn space_toggles_and_enter_deletes_from_the_keyboard() {
    let mut app = headless_app();
    let check = part(&mut app, Part::Check, Some(3));
    set_focus(&mut app, check);
    press_key(&mut app, KeyCode::Char(' '));
    settle(&mut app);
    assert!(app.world().get::<Checked>(check).is_some());

    let delete = part(&mut app, Part::Delete, Some(1));
    set_focus(&mut app, delete);
    press_key(&mut app, KeyCode::Enter);
    settle(&mut app);
    assert_eq!(rows(&mut app), 2);
    assert!(!composed_frame(&app).contains("Try the terminal skin"));
}

#[test]
fn filters_narrow_the_ledger_and_clear_completed_empties_it() {
    let mut app = headless_app();
    let completed = part(&mut app, Part::Filter(Filter::Completed), None);
    click_center(&mut app, completed);
    settle(&mut app);
    assert_eq!(rows(&mut app), 1);
    assert!(app.world().get::<Checked>(completed).is_some());

    let clear = part(&mut app, Part::Clear, None);
    click_center(&mut app, clear);
    settle(&mut app);
    assert_eq!(rows(&mut app), 0);
    assert!(composed_frame(&app).contains("Nothing here."));

    let all = part(&mut app, Part::Filter(Filter::All), None);
    click_center(&mut app, all);
    settle(&mut app);
    assert_eq!(rows(&mut app), 2);
}

#[test]
fn tab_walks_from_the_field_into_the_ledger() {
    let mut app = headless_app();
    let field = part(&mut app, Part::Field, None);
    set_focus(&mut app, field);

    press_key(&mut app, KeyCode::Tab);
    app.update();
    assert_eq!(focused(&app), Some(part(&mut app, Part::AddButton, None)));

    press_key(&mut app, KeyCode::Tab);
    app.update();
    assert_eq!(focused(&app), Some(part(&mut app, Part::Check, Some(1))));
}

mod desktop_skin {
    use bevy_color::Color;
    use bevy_input::ButtonState;
    use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
    use bevy_text::{EditableText, Strikethrough};
    use bevy_ui::{BackgroundColor, Checked, GridTrack, Node, Outline};
    use plurimus::bui::bevy_ui_widgets::TextInput;
    use plurimus::ui::bevy_input_focus::{FocusedInput, InputFocus};

    use super::*;

    fn headless_app() -> App {
        let mut app = App::new();
        app.init_resource::<InputFocus>();
        add_shared(&mut app);
        app.add_plugins(desktop::DesktopSkin);
        settle(&mut app);
        app
    }

    fn press_enter(app: &mut App, field: Entity) {
        let window = app.world_mut().spawn_empty().id();
        let input = KeyboardInput {
            key_code: KeyCode::Enter,
            logical_key: Key::Enter,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        };
        app.world_mut()
            .trigger(FocusedInput::new(field, input, window));
    }

    #[test]
    fn the_skin_dresses_the_card_and_the_ledger() {
        let mut app = headless_app();
        let card = part(&mut app, Part::Card, None);
        let list = part(&mut app, Part::List, None);
        let done = part(&mut app, Part::ItemText, Some(1));

        let world = app.world();
        assert_ne!(world.get::<BackgroundColor>(card).unwrap().0, Color::NONE);
        let columns = &world.get::<Node>(list).unwrap().grid_template_columns;
        assert_eq!(
            columns[2],
            GridTrack::px(48.0),
            "the skin sizes the id column"
        );
        assert!(world.get::<Strikethrough>(done).is_some());
    }

    #[test]
    fn focus_draws_a_ring_on_everything_but_the_field() {
        let mut app = headless_app();
        let add = part(&mut app, Part::AddButton, None);
        let field = part(&mut app, Part::Field, None);
        assert!(
            app.world().get::<Outline>(field).is_none(),
            "the field colours its border"
        );

        app.insert_resource(InputFocus::from_entity(add));
        settle(&mut app);
        assert!(app.world().get::<Outline>(add).is_some());

        app.insert_resource(InputFocus::from_entity(field));
        settle(&mut app);
        assert!(app.world().get::<Outline>(add).is_none());
    }

    #[test]
    fn enter_in_the_upstream_field_adds_its_text() {
        let mut app = headless_app();
        let field = part(&mut app, Part::Field, None);
        assert!(app.world().get::<TextInput>(field).is_some());
        app.world_mut()
            .entity_mut(field)
            .insert(EditableText::new("milk"));

        press_enter(&mut app, field);
        settle(&mut app);
        assert_eq!(rows(&mut app), 4);
        let check = part(&mut app, Part::Check, Some(4));
        assert!(app.world().get::<Checked>(check).is_none());
        let value = app.world().get::<EditableText>(field).unwrap().value();
        assert_eq!(value.to_string(), "", "the field clears on submit");
    }
}
