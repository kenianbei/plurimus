//! Snapshot tests over composed frames.

use bevy_app::App;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, Viewport};
use plurimus_test::composed_frame;
use plurimus_ui::tui_scrollview::ScrollbarVisibility;
use plurimus_ui::{ScrollArea, ScrollOffset, UiArea, UiCamera, UiHidden, UiOrder, UiWidget};
use plurimus_widgets::{Popover, PopoverAlign, PopoverSide, WidgetsPlugin, scrollbar};
use ratatui_widgets::list::{List, ListState};
use ratatui_widgets::paragraph::Paragraph;
use ratatui_widgets::scrollbar::{Scrollbar, ScrollbarOrientation, ScrollbarState};

fn app(cols: u16, rows: u16) -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols, rows });
    app
}

#[test]
fn paragraph_in_fixed_rect() {
    let mut app = app(8, 3);
    app.world_mut().spawn(TerminalCamera::default());
    app.world_mut().spawn((
        UiWidget::new(Paragraph::new("hi")),
        UiArea::Fixed(Rect::new(2, 1, 4, 1)),
    ));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn z_order_beats_spawn_order() {
    let mut app = app(6, 1);
    app.world_mut().spawn(TerminalCamera::default());
    app.world_mut()
        .spawn((UiWidget::new(Paragraph::new("TOPTOP")), UiOrder(1)));
    app.world_mut()
        .spawn((UiWidget::new(Paragraph::new("bottom")), UiOrder(0)));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn widgets_target_cameras_explicitly_or_by_default() {
    let mut app = app(8, 1);
    app.world_mut().spawn(TerminalCamera {
        viewport: Viewport::Fixed(Rect::new(0, 0, 4, 1)),
        ..TerminalCamera::default()
    });
    let right = app
        .world_mut()
        .spawn(TerminalCamera {
            order: 1,
            viewport: Viewport::Fixed(Rect::new(4, 0, 4, 1)),
            ..TerminalCamera::default()
        })
        .id();
    app.world_mut().spawn(UiWidget::new(Paragraph::new("main")));
    app.world_mut()
        .spawn((UiWidget::new(Paragraph::new("side")), UiCamera(right)));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn hidden_widget_skips_render_until_unhidden() {
    let mut app = app(6, 1);
    app.world_mut().spawn(TerminalCamera::default());
    let widget = app
        .world_mut()
        .spawn((UiWidget::new(Paragraph::new("shown!")), UiHidden))
        .id();

    app.update();
    insta::assert_snapshot!("hidden_widget", composed_frame(&app));

    app.world_mut().entity_mut(widget).remove::<UiHidden>();
    app.update();
    insta::assert_snapshot!("unhidden_widget", composed_frame(&app));
}

#[test]
fn popover_renders_anchored_with_overlay_order() {
    let mut app = app(10, 4);
    app.world_mut().spawn(TerminalCamera::default());
    let anchor = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("[menu]")),
            UiArea::Fixed(Rect::new(1, 0, 6, 1)),
        ))
        .id();
    let popover = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("open\nsave")),
            Popover {
                anchor,
                side: PopoverSide::Bottom,
                align: PopoverAlign::Start,
                size: Size::new(4, 2),
            },
        ))
        .id();

    app.update();

    assert_eq!(app.world().get::<UiOrder>(popover), Some(&UiOrder::OVERLAY));
    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn stateful_list_renders_selection() {
    let mut app = app(8, 3);
    app.world_mut().spawn(TerminalCamera::default());
    let mut selection = ListState::default();
    selection.select(Some(1));
    let list = List::new(["one", "two", "three"]).highlight_symbol("> ");
    app.world_mut().spawn(UiWidget::stateful(list, selection));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn stateful_scrollbar_renders_thumb() {
    let mut app = app(1, 4);
    app.world_mut().spawn(TerminalCamera::default());
    let track = ScrollbarState::new(8).position(6);
    app.world_mut().spawn(UiWidget::stateful(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        track,
    ));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn scroll_area_windows_content() {
    let mut app = app(6, 3);
    app.world_mut().spawn(TerminalCamera::default());
    let scrolled = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("l1\nl2\nl3\nl4\nl5\nl6")),
            ScrollArea::new(Size::new(5, 6)),
        ))
        .id();

    app.update();
    insta::assert_snapshot!("scroll_top", composed_frame(&app));

    app.world_mut()
        .entity_mut(scrolled)
        .insert(ScrollOffset(Position::new(0, 3)));
    app.update();
    insta::assert_snapshot!("scroll_bottom", composed_frame(&app));
}

#[test]
fn scroll_area_fitting_content_shows_no_bars() {
    let mut app = app(6, 3);
    app.world_mut().spawn(TerminalCamera::default());
    app.world_mut().spawn((
        UiWidget::new(Paragraph::new("ab\ncd")),
        ScrollArea::new(Size::new(2, 2)),
    ));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn scrollbar_entity_tracks_target() {
    let mut app = app(7, 4);
    app.world_mut().spawn(TerminalCamera::default());
    let target = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8")),
            UiArea::Fixed(Rect::new(0, 0, 6, 4)),
            ScrollArea {
                content_size: Size::new(6, 8),
                scrollbars: ScrollbarVisibility::Never,
            },
            ScrollOffset(Position::new(0, 4)),
        ))
        .id();
    app.world_mut().spawn((
        scrollbar(target, ScrollbarOrientation::VerticalRight),
        UiArea::Fixed(Rect::new(6, 0, 1, 4)),
    ));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn scroll_area_never_hides_scrollbars() {
    let mut app = app(5, 2);
    app.world_mut().spawn(TerminalCamera::default());
    app.world_mut().spawn((
        UiWidget::new(Paragraph::new("aa\nbb\ncc")),
        ScrollArea {
            content_size: Size::new(5, 3),
            scrollbars: ScrollbarVisibility::Never,
        },
    ));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn widget_on_inactive_camera_is_skipped() {
    let mut app = app(4, 1);
    app.world_mut().spawn(TerminalCamera::default());
    let inactive = app
        .world_mut()
        .spawn(TerminalCamera {
            active: false,
            ..TerminalCamera::default()
        })
        .id();
    app.world_mut()
        .spawn((UiWidget::new(Paragraph::new("gone")), UiCamera(inactive)));

    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}
