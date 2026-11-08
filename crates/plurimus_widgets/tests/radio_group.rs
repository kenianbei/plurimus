//! The radio group is the tab stop: a press on an option focuses it, its
//! arrows move the check, and the checked option shows the group's focus.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use bevy_input::keyboard::Key;
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{click, press_key, set_focus};
use plurimus_ui::{Checked, StylistCache, UiArea};
use plurimus_widgets::{WidgetsPlugin, radio, radio_group, radio_self_update};

const OPTIONS: u16 = 3;

struct Group {
    group: Entity,
    options: Vec<Entity>,
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 4));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_group(app: &mut App) -> Group {
    let world = app.world_mut();
    let group = world
        .spawn((radio_group(), UiArea::Fixed(Rect::new(0, 0, 10, OPTIONS))))
        .observe(radio_self_update)
        .id();
    let options = (0..OPTIONS)
        .map(|row| {
            world
                .spawn((
                    radio("option"),
                    UiArea::Fixed(Rect::new(0, row, 10, 1)),
                    ChildOf(group),
                ))
                .id()
        })
        .collect();
    app.update();
    Group { group, options }
}

fn checked(app: &App, group: &Group) -> Option<usize> {
    group
        .options
        .iter()
        .position(|&option| app.world().entity(option).contains::<Checked>())
}

fn drawn_focused(app: &App, entity: Entity) -> bool {
    app.world()
        .get::<StylistCache>(entity)
        .unwrap()
        .state()
        .focused
}

#[test]
fn a_press_on_an_option_checks_it_and_focuses_the_group() {
    let mut app = app();
    let group = spawn_group(&mut app);

    click(&mut app, 1, 1);

    assert_eq!(checked(&app, &group), Some(1));
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(group.group)
    );
}

#[test]
fn the_group_arrows_move_the_check_and_wrap() {
    let mut app = app();
    let group = spawn_group(&mut app);
    set_focus(&mut app, group.group);

    press_key(&mut app, Key::ArrowDown);
    assert_eq!(checked(&app, &group), Some(0), "from none, the first");
    press_key(&mut app, Key::End);
    assert_eq!(checked(&app, &group), Some(2));
    press_key(&mut app, Key::ArrowDown);
    assert_eq!(checked(&app, &group), Some(0), "past the last, the first");
    press_key(&mut app, Key::ArrowUp);
    assert_eq!(checked(&app, &group), Some(2), "and back");
}

#[test]
fn the_checked_option_shows_the_groups_focus() {
    let mut app = app();
    let group = spawn_group(&mut app);
    click(&mut app, 1, 1);
    app.update();

    assert!(drawn_focused(&app, group.options[1]));
    assert!(
        !drawn_focused(&app, group.options[0]),
        "only the checked one"
    );

    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    assert!(!drawn_focused(&app, group.options[1]));
}
