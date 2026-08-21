//! What a press on a disabled widget does: absorbs, unless the widget
//! opts out of hit-testing entirely.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{On, ResMut, Resource};
use bevy_input_focus::tab_navigation::TabGroup;
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiOrder, UiWidget};
use plurimus_term::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{click, press_key, send_mouse};
use plurimus_ui::{
    Click, Hovered, InteractionDisabled, PointerDrag, PointerPress, PressFocusDisabled,
    PressPassThrough, Pressed, UiArea,
};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{MenuOpen, WidgetsPlugin, menu_button, menu_item, menu_popup};

const AREA: Rect = Rect::new(0, 2, 10, 3);

#[derive(Resource, Default)]
struct Presses(Vec<Entity>);

#[derive(Resource, Default)]
struct Gestures(Vec<&'static str>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Presses>();
    app.add_observer(|press: On<PointerPress>, mut log: ResMut<Presses>| log.0.push(press.entity));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_widget(app: &mut App, area: Rect) -> Entity {
    app.world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("widget")),
            UiArea::Fixed(area),
            Hovered::default(),
        ))
        .id()
}

// A band above `beneath`, so the cover wins arbitration outright rather
// than by the entity tiebreak - the tests must prove absorption and
// transparency, not tie direction.
fn spawn_cover(app: &mut App, area: Rect) -> Entity {
    let cover = spawn_widget(app, area);
    app.world_mut().entity_mut(cover).insert(UiOrder(1));
    cover
}

fn was_pressed(app: &App, entity: Entity) -> bool {
    app.world().resource::<Presses>().0.contains(&entity)
}

// Two widgets on the same rect: the covering one disabled, the covered one
// live. Before the press knew "inert", the disabled one was invisible and
// the press went straight through it.
#[test]
fn a_press_on_a_disabled_widget_reaches_nothing_beneath() {
    let mut app = app();
    let beneath = spawn_widget(&mut app, AREA);
    let cover = spawn_cover(&mut app, AREA);
    app.world_mut()
        .entity_mut(cover)
        .insert(InteractionDisabled);
    app.update();

    click(&mut app, AREA.x + 1, AREA.y + 1);

    assert!(!was_pressed(&app, beneath), "the press was absorbed");
    assert!(
        !was_pressed(&app, cover),
        "and the disabled widget is inert"
    );
}

#[test]
fn a_press_on_a_disabled_widget_moves_no_focus() {
    let mut app = app();
    let cover = spawn_widget(&mut app, AREA);
    app.world_mut()
        .entity_mut(cover)
        .insert((InteractionDisabled, TabIndex(0)));
    let elsewhere = app.world_mut().spawn(()).id();
    app.update();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(elsewhere, FocusCause::Pressed);

    click(&mut app, AREA.x + 1, AREA.y + 1);

    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(elsewhere),
        "focus stays where it was"
    );
}

#[test]
fn pass_through_restores_the_fall_through() {
    let mut app = app();
    let beneath = spawn_widget(&mut app, AREA);
    let cover = spawn_cover(&mut app, AREA);
    app.world_mut()
        .entity_mut(cover)
        .insert((InteractionDisabled, PressPassThrough));
    app.update();

    click(&mut app, AREA.x + 1, AREA.y + 1);

    assert!(was_pressed(&app, beneath), "the press fell through");
    assert!(!was_pressed(&app, cover));
}

// The marker is not gated on being disabled: it is press transparency.
#[test]
fn an_enabled_pass_through_widget_is_invisible_to_the_press() {
    let mut app = app();
    let beneath = spawn_widget(&mut app, AREA);
    let cover = spawn_cover(&mut app, AREA);
    app.world_mut().entity_mut(cover).insert(PressPassThrough);
    app.update();

    click(&mut app, AREA.x + 1, AREA.y + 1);

    assert!(was_pressed(&app, beneath));
    assert!(!was_pressed(&app, cover));
}

// The guard's outside rule runs before target semantics: greyed chrome
// does not shield an open menu from dismissal.
#[test]
fn a_press_on_disabled_chrome_still_dismisses_the_menu() {
    let mut app = app();
    let button = app
        .world_mut()
        .spawn((menu_button("File"), UiArea::Fixed(Rect::new(11, 0, 8, 1))))
        .id();
    let popup = app
        .world_mut()
        .spawn((menu_popup(button), ChildOf(button)))
        .id();
    app.world_mut().spawn((menu_item("Open"), ChildOf(popup)));
    let chrome = spawn_widget(&mut app, Rect::new(0, 6, 6, 1));
    app.world_mut()
        .entity_mut(chrome)
        .insert(InteractionDisabled);
    app.update();
    click(&mut app, 12, 0);
    assert!(app.world().entity(popup).contains::<MenuOpen>());

    click(&mut app, 1, 6);

    assert!(
        !app.world().entity(popup).contains::<MenuOpen>(),
        "the outside press dismissed"
    );
    assert!(!was_pressed(&app, chrome), "and the chrome stayed inert");
}

// The browser's preventDefault-on-mousedown: the gesture is the widget's,
// the keyboard stays where it was.
#[test]
fn a_press_focus_disabled_widget_presses_without_taking_focus() {
    let mut app = app();
    let toolbar = spawn_widget(&mut app, AREA);
    app.world_mut()
        .entity_mut(toolbar)
        .insert((TabIndex(0), PressFocusDisabled));
    let editor = app.world_mut().spawn(TabIndex(1)).id();
    app.update();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(editor, FocusCause::Pressed);

    send_mouse(&mut app, MouseKind::Moved, AREA.x + 1, AREA.y + 1);
    send_mouse(
        &mut app,
        MouseKind::Down(MouseButton::Left),
        AREA.x + 1,
        AREA.y + 1,
    );

    assert!(was_pressed(&app, toolbar), "the press landed");
    assert!(app.world().entity(toolbar).contains::<Pressed>());
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(editor),
        "and the editor kept the keyboard"
    );
}

#[test]
fn a_press_focus_disabled_widget_still_drags_and_clicks() {
    let mut app = app();
    let toolbar = spawn_widget(&mut app, AREA);
    app.world_mut()
        .entity_mut(toolbar)
        .insert((TabIndex(0), PressFocusDisabled));
    app.init_resource::<Gestures>();
    app.add_observer(|_: On<PointerDrag>, mut log: ResMut<Gestures>| log.0.push("drag"));
    app.add_observer(|_: On<Click>, mut log: ResMut<Gestures>| log.0.push("click"));
    app.update();

    send_mouse(&mut app, MouseKind::Moved, AREA.x + 1, AREA.y + 1);
    send_mouse(
        &mut app,
        MouseKind::Down(MouseButton::Left),
        AREA.x + 1,
        AREA.y + 1,
    );
    send_mouse(
        &mut app,
        MouseKind::Drag(MouseButton::Left),
        AREA.x + 2,
        AREA.y + 1,
    );
    send_mouse(
        &mut app,
        MouseKind::Up(MouseButton::Left),
        AREA.x + 2,
        AREA.y + 1,
    );

    assert_eq!(
        app.world().resource::<Gestures>().0,
        vec!["drag", "click"],
        "the whole gesture belongs to the widget"
    );
}

// The marker splits the flag, it does not take the keyboard half away.
#[test]
fn tab_still_reaches_a_press_focus_disabled_widget() {
    let mut app = app();
    let root = app.world_mut().spawn(TabGroup::new(0)).id();
    let toolbar = spawn_widget(&mut app, AREA);
    app.world_mut()
        .entity_mut(toolbar)
        .insert((TabIndex(0), PressFocusDisabled, ChildOf(root)));
    app.update();

    press_key(&mut app, KeyCode::Tab);

    assert_eq!(app.world().resource::<InputFocus>().get(), Some(toolbar));
}

// Disabling a widget mid-gesture silences the whole rest of the gesture:
// the release must not activate a control that went grey under the
// pointer. The target query no longer excludes disabled widgets, so this
// is the release path's own term, not the query's.
#[test]
fn a_widget_disabled_mid_gesture_does_not_click() {
    let mut app = app();
    let widget = spawn_widget(&mut app, AREA);
    app.init_resource::<Gestures>();
    app.add_observer(|_: On<Click>, mut log: ResMut<Gestures>| log.0.push("click"));
    app.update();

    send_mouse(&mut app, MouseKind::Moved, AREA.x + 1, AREA.y + 1);
    send_mouse(
        &mut app,
        MouseKind::Down(MouseButton::Left),
        AREA.x + 1,
        AREA.y + 1,
    );
    assert!(app.world().entity(widget).contains::<Pressed>());
    app.world_mut()
        .entity_mut(widget)
        .insert(InteractionDisabled);
    send_mouse(
        &mut app,
        MouseKind::Up(MouseButton::Left),
        AREA.x + 1,
        AREA.y + 1,
    );

    assert!(
        app.world().resource::<Gestures>().0.is_empty(),
        "no click from a widget that went grey mid-gesture"
    );
    assert!(
        !app.world().entity(widget).contains::<Pressed>(),
        "but the gesture still ended"
    );
}

// Inside an open modal the guard confines rather than dismisses, and a
// disabled child then absorbs like anywhere else.
#[test]
fn a_disabled_child_inside_a_modal_absorbs_without_dismissing() {
    let mut app = app();
    let button = app
        .world_mut()
        .spawn((menu_button("File"), UiArea::Fixed(Rect::new(11, 0, 8, 1))))
        .id();
    let popup = app
        .world_mut()
        .spawn((menu_popup(button), ChildOf(button)))
        .id();
    app.world_mut().spawn((menu_item("Open"), ChildOf(popup)));
    app.update();
    click(&mut app, 12, 0);
    app.update();
    let frame = app
        .world()
        .get::<plurimus_ui::ComputedWidgetArea>(popup)
        .unwrap()
        .0;
    let footer = Rect::new(frame.x, frame.y + frame.height - 1, frame.width, 1);
    let child = spawn_widget(&mut app, footer);
    app.world_mut()
        .entity_mut(child)
        .insert((InteractionDisabled, ChildOf(popup)));
    app.update();

    click(&mut app, footer.x + 1, footer.y);

    assert!(
        app.world().entity(popup).contains::<MenuOpen>(),
        "the confined press dismissed nothing"
    );
    assert!(
        !was_pressed(&app, child),
        "and the disabled child absorbed it"
    );
}
