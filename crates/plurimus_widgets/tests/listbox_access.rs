//! A list box's cursor moved by whoever holds focus elsewhere.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::system::RunSystemOnce;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::set_focus;
use plurimus_ui::{InteractionDisabled, UiArea};
use plurimus_widgets::{
    ActiveDescendant, ListBoxAccess, ListBoxAction, WidgetsPlugin, list_item, listbox,
};

const AREA: Rect = Rect::new(0, 0, 12, 3);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(AREA.width, AREA.height));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

/// Settled by one update, so the list has an area before any motion.
fn spawn_driven_list(app: &mut App, rows: usize) -> (Entity, Vec<Entity>) {
    let list = app.world_mut().spawn((listbox(), UiArea::Fixed(AREA))).id();
    let items = (0..rows)
        .map(|index| {
            app.world_mut()
                .spawn((list_item(format!("item {index}")), ChildOf(list)))
                .id()
        })
        .collect();
    let elsewhere = app.world_mut().spawn(()).id();
    app.update();
    set_focus(app, elsewhere);
    (list, items)
}

fn apply(app: &mut App, list: Entity, action: ListBoxAction) -> bool {
    app.world_mut()
        .run_system_once(move |mut access: ListBoxAccess| access.apply(list, action))
        .expect("the applier runs as a system")
}

fn cursor(app: &App, list: Entity) -> Option<Entity> {
    app.world().get::<ActiveDescendant>(list).unwrap().0
}

#[test]
fn a_motion_moves_an_unfocused_list_and_says_whether_it_did() {
    let mut app = app();
    let (list, items) = spawn_driven_list(&mut app, 3);

    assert!(
        apply(&mut app, list, ListBoxAction::Down),
        "no cursor gains one"
    );
    assert_eq!(cursor(&app, list), Some(items[0]));
    assert!(
        !apply(&mut app, list, ListBoxAction::Up),
        "the first row stays"
    );
    assert!(apply(&mut app, list, ListBoxAction::Last));
    assert!(
        !apply(&mut app, list, ListBoxAction::Down),
        "the last row stays"
    );
    assert_eq!(cursor(&app, list), Some(items[2]));
}

#[test]
fn select_and_copy_are_not_motions() {
    let mut app = app();
    let (list, items) = spawn_driven_list(&mut app, 3);
    assert!(apply(&mut app, list, ListBoxAction::Down));

    assert!(!apply(&mut app, list, ListBoxAction::Select));
    assert!(!apply(&mut app, list, ListBoxAction::Copy));
    assert_eq!(cursor(&app, list), Some(items[0]));
}

#[test]
fn nothing_to_move_answers_false() {
    let mut app = app();
    let (empty, _) = spawn_driven_list(&mut app, 0);
    let (disabled, _) = spawn_driven_list(&mut app, 3);
    app.world_mut()
        .entity_mut(disabled)
        .insert(InteractionDisabled);
    let bystander = app.world_mut().spawn(()).id();
    app.update();

    assert!(!apply(&mut app, empty, ListBoxAction::Down));
    assert!(!apply(&mut app, disabled, ListBoxAction::Down));
    assert!(!apply(&mut app, bystander, ListBoxAction::Down));
    assert_eq!(cursor(&app, disabled), None);
}
