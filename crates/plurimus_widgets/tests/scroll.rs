//! Wheel routing and scroll-into-view integration tests, fully headless.

use bevy_app::App;
use bevy_ecs::prelude::{On, ResMut, Resource};
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{MouseButton, MouseKind};
use plurimus_test::send_mouse;
use plurimus_ui::{
    InteractionDisabled, ScrollArea, ScrollIntoView, ScrollOffset, UiArea, UiOrder, UiWidget,
    ValueChange,
};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::ratatui_widgets::scrollbar::ScrollbarOrientation;
use plurimus_widgets::{WidgetsPlugin, scrollbar, text_editor};

#[derive(Resource, Default)]
struct OffsetChanges(Vec<Position>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 6));
    app.init_resource::<OffsetChanges>();
    app.add_observer(
        |change: On<ValueChange<Position>>, mut log: ResMut<OffsetChanges>| {
            log.0.push(change.value);
        },
    );
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn scroll_entity(app: &mut App, area: Rect, content: Size, order: i32) -> bevy_ecs::entity::Entity {
    app.world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("content")),
            UiArea::Fixed(area),
            ScrollArea::new(content),
            UiOrder(order),
        ))
        .id()
}

fn offset_of(app: &App, entity: bevy_ecs::entity::Entity) -> Position {
    app.world().get::<ScrollOffset>(entity).unwrap().0
}

#[test]
fn wheel_scrolls_topmost_area_one_cell() {
    let mut app = app();
    let bottom = scroll_entity(&mut app, Rect::new(0, 0, 10, 3), Size::new(10, 10), 0);
    let top = scroll_entity(&mut app, Rect::new(0, 0, 10, 3), Size::new(10, 10), 1);

    send_mouse(&mut app, MouseKind::ScrollDown, 5, 1);

    assert_eq!(offset_of(&app, top), Position::new(0, 1));
    assert_eq!(offset_of(&app, bottom), Position::new(0, 0));
    assert_eq!(
        app.world().resource::<OffsetChanges>().0,
        [Position::new(0, 1)]
    );
}

#[test]
fn wheel_outside_area_is_ignored() {
    let mut app = app();
    let scrolled = scroll_entity(&mut app, Rect::new(0, 0, 10, 3), Size::new(10, 10), 0);

    send_mouse(&mut app, MouseKind::ScrollDown, 15, 5);

    assert_eq!(offset_of(&app, scrolled), Position::new(0, 0));
    assert!(app.world().resource::<OffsetChanges>().0.is_empty());
}

#[test]
fn wheel_clamps_at_both_extremes() {
    let mut app = app();
    let scrolled = scroll_entity(&mut app, Rect::new(0, 0, 10, 3), Size::new(10, 4), 0);

    send_mouse(&mut app, MouseKind::ScrollUp, 5, 1);
    assert_eq!(offset_of(&app, scrolled), Position::new(0, 0));
    assert!(app.world().resource::<OffsetChanges>().0.is_empty());

    send_mouse(&mut app, MouseKind::ScrollDown, 5, 1);
    send_mouse(&mut app, MouseKind::ScrollDown, 5, 1);
    assert_eq!(offset_of(&app, scrolled), Position::new(0, 1));
    assert_eq!(
        app.world().resource::<OffsetChanges>().0,
        [Position::new(0, 1)]
    );
}

#[test]
fn wheel_scrolls_horizontally() {
    let mut app = app();
    let scrolled = scroll_entity(&mut app, Rect::new(0, 0, 5, 3), Size::new(8, 3), 0);

    send_mouse(&mut app, MouseKind::ScrollRight, 2, 1);
    assert_eq!(offset_of(&app, scrolled), Position::new(1, 0));

    send_mouse(&mut app, MouseKind::ScrollLeft, 2, 1);
    assert_eq!(offset_of(&app, scrolled), Position::new(0, 0));
}

#[test]
fn wheel_arbitrates_across_widget_families() {
    let mut app = app();
    let area = Rect::new(0, 0, 10, 3);
    let scrolled = scroll_entity(&mut app, area, Size::new(10, 10), 0);
    app.world_mut().spawn((
        text_editor("l1\nl2\nl3\nl4"),
        UiArea::Fixed(area),
        UiOrder(1),
    ));
    app.update();

    send_mouse(&mut app, MouseKind::ScrollDown, 5, 1);

    assert_eq!(offset_of(&app, scrolled), Position::new(0, 0));
    assert!(app.world().resource::<OffsetChanges>().0.is_empty());
}

#[test]
fn wheel_prefers_the_innermost_of_two_same_order_areas() {
    let mut app = app();
    let outer = scroll_entity(&mut app, Rect::new(0, 0, 12, 4), Size::new(12, 40), 0);
    let inner = scroll_entity(&mut app, Rect::new(2, 1, 4, 2), Size::new(4, 40), 0);

    send_mouse(&mut app, MouseKind::ScrollDown, 3, 1);

    assert_eq!(offset_of(&app, inner), Position::new(0, 1));
    assert_eq!(offset_of(&app, outer), Position::new(0, 0));
}

#[test]
fn wheel_falls_through_an_axis_a_widget_cannot_scroll() {
    let mut app = app();
    let below = scroll_entity(&mut app, Rect::new(0, 0, 10, 3), Size::new(10, 10), 0);
    let above = scroll_entity(&mut app, Rect::new(0, 0, 10, 3), Size::new(20, 3), 1);

    send_mouse(&mut app, MouseKind::ScrollDown, 5, 1);

    assert_eq!(offset_of(&app, above), Position::new(0, 0), "no y overflow");
    assert_eq!(offset_of(&app, below), Position::new(0, 1));
}

#[test]
fn wheel_falls_through_a_disabled_area() {
    let mut app = app();
    let bottom = scroll_entity(&mut app, Rect::new(0, 0, 10, 3), Size::new(10, 10), 0);
    let top = scroll_entity(&mut app, Rect::new(0, 0, 10, 3), Size::new(10, 10), 1);
    app.world_mut().entity_mut(top).insert(InteractionDisabled);

    send_mouse(&mut app, MouseKind::ScrollDown, 5, 1);

    assert_eq!(offset_of(&app, top), Position::new(0, 0));
    assert_eq!(offset_of(&app, bottom), Position::new(0, 1));
}

#[test]
fn scrollbar_press_and_drag_seek_target() {
    let mut app = app();
    let target = scroll_entity(&mut app, Rect::new(0, 0, 10, 4), Size::new(10, 13), 0);
    app.world_mut().spawn((
        scrollbar(target, ScrollbarOrientation::VerticalRight),
        UiArea::Fixed(Rect::new(10, 0, 1, 4)),
    ));

    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 10, 3);
    assert_eq!(offset_of(&app, target), Position::new(0, 9));

    send_mouse(&mut app, MouseKind::Drag(MouseButton::Left), 10, 1);
    assert_eq!(offset_of(&app, target), Position::new(0, 3));

    send_mouse(&mut app, MouseKind::Up(MouseButton::Left), 10, 0);
    assert_eq!(offset_of(&app, target), Position::new(0, 0));
}

#[test]
fn scroll_into_view_reveals_minimally() {
    let mut app = app();
    let scrolled = scroll_entity(&mut app, Rect::new(0, 0, 5, 3), Size::new(5, 10), 0);
    app.update();

    app.world_mut().trigger(ScrollIntoView {
        entity: scrolled,
        target: Rect::new(0, 7, 1, 1),
    });
    assert_eq!(offset_of(&app, scrolled), Position::new(0, 5));

    app.world_mut().trigger(ScrollIntoView {
        entity: scrolled,
        target: Rect::new(0, 1, 1, 1),
    });
    assert_eq!(offset_of(&app, scrolled), Position::new(0, 1));

    app.world_mut().trigger(ScrollIntoView {
        entity: scrolled,
        target: Rect::new(0, 2, 1, 1),
    });
    assert_eq!(offset_of(&app, scrolled), Position::new(0, 1));
}
