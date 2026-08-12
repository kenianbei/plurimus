//! Focus dispatch reads state written elsewhere in the same frame: polled
//! key state from `bevy_input`, and widget areas from `plurimus_ui`.
//! Nothing in an observer's own code says when it runs, so the ordering
//! that makes both readable is only ever true by assertion.

#![cfg(feature = "widgets")]

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, Res, ResMut, Resource};
use bevy_input::ButtonInput;
use bevy_input::keyboard::KeyCode as BevyKeyCode;
use bevy_input_focus::{FocusCause, FocusedInput, InputFocus};
use plurimus::core::ratatui_core::layout::{Constraint, Rect};
use plurimus::core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus::term::{InputCapabilities, KeyCode, KeyKind, KeyMessage, KeyModifiers, PasteMessage};
use plurimus::ui::UiArea;
use plurimus::widgets::{
    ActiveDescendant, TableSelection, WidgetsPlugin, button, list_item, listbox, table,
    table_footer, table_header, table_row,
};
use plurimus_test::press_key;

const AREA: Rect = Rect::new(0, 0, 20, 6);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 6));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn focus(app: &mut App, entity: Entity) {
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Pressed);
}

fn active(app: &App, container: Entity) -> Option<Entity> {
    app.world().get::<ActiveDescendant>(container).unwrap().0
}

/// Whether shift was held by the time the paste dispatch observer ran.
#[derive(Resource, Default)]
struct ShiftAtPaste(Option<bool>);

// An invariant rather than a regression guard: dropping the set-level edge
// does not fail this, because the executor drains `ButtonInput` first
// regardless - an accident of the schedule, not a promise.
//
// The legacy tier is what forces key and paste into one frame:
// `forward_keyboard` synthesizes the modifier press from the message's own
// bitfield, so it cannot arrive a frame early.
#[test]
fn a_dispatch_plurimus_adds_itself_reads_settled_key_state() {
    let mut app = app();
    app.insert_resource(InputCapabilities::default().with_modifier_keys(false));
    app.init_resource::<ShiftAtPaste>();

    let target = app
        .world_mut()
        .spawn((button("ok"), UiArea::Fixed(AREA)))
        .id();
    app.world_mut().entity_mut(target).observe(
        |_: On<FocusedInput<PasteMessage>>,
         keys: Res<ButtonInput<BevyKeyCode>>,
         mut seen: ResMut<ShiftAtPaste>| {
            seen.0 = Some(keys.pressed(BevyKeyCode::ShiftLeft));
        },
    );
    focus(&mut app, target);
    app.update();

    let world = app.world_mut();
    world.write_message(KeyMessage::new(
        KeyCode::Char('a'),
        KeyModifiers::default().with_shift(true),
        KeyKind::Press,
    ));
    world.write_message(PasteMessage("x".into()));
    app.update();

    assert_eq!(
        app.world().resource::<ShiftAtPaste>().0,
        Some(true),
        "the paste dispatch ran before bevy_input drained the synthesized \
         press, so an observer reading held keys sees none"
    );
}

#[test]
fn a_first_frame_page_moves_a_full_listbox_page() {
    let mut app = app();
    let world = app.world_mut();
    let container = world.spawn((listbox(), UiArea::Fixed(AREA))).id();
    let items: Vec<Entity> = (0..8)
        .map(|index| {
            world
                .spawn((list_item(format!("item {index}")), ChildOf(container)))
                .id()
        })
        .collect();
    world
        .entity_mut(container)
        .insert(ActiveDescendant(Some(items[0])));
    focus(&mut app, container);

    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(
        active(&app, container),
        Some(items[6]),
        "six rows are visible on the first frame too, so a page is six rows"
    );
}

#[test]
fn a_first_frame_page_moves_a_full_table_page() {
    let mut app = app();
    let world = app.world_mut();
    let container = world
        .spawn((
            table([Constraint::Length(6), Constraint::Length(6)]),
            TableSelection::Row,
            UiArea::Fixed(AREA),
        ))
        .id();
    world.spawn((table_header(["name", "date"]), ChildOf(container)));
    let rows: Vec<Entity> = (0..8)
        .map(|index| {
            world
                .spawn((
                    table_row([format!("row {index}"), "may".into()]),
                    ChildOf(container),
                ))
                .id()
        })
        .collect();
    world.spawn((table_footer(["total", "8"]), ChildOf(container)));
    world
        .entity_mut(container)
        .insert(ActiveDescendant(Some(rows[0])));
    focus(&mut app, container);

    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(
        active(&app, container),
        Some(rows[4]),
        "a header and a footer leave four body rows, so a page is four rows"
    );
}
