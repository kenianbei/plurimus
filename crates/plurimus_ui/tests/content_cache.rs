//! Render-world content cache for scrolled widgets, fully headless.
//!
//! The widgets here count their own rasterizations, so the tests assert on
//! cache behavior directly rather than inferring it from timings.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bevy_app::App;
use bevy_ecs::entity::Entity;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::widgets::Widget;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::composed_frame;
use plurimus_ui::tui_scrollview::ScrollbarVisibility;
use plurimus_ui::{LiveWidget, ScrollArea, ScrollOffset, UiArea, UiPlugin, UiWidget};

/// Writes `r0`, `r1`, ... one per content row, counting each rasterization.
#[derive(Clone)]
struct Rows(Arc<AtomicUsize>);

impl Widget for &Rows {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.0.fetch_add(1, Ordering::Relaxed);
        for row in 0..area.height {
            buffer.set_string(area.x, area.y + row, format!("r{row}"), Style::new());
        }
    }
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize { cols: 4, rows: 2 });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn scroll_area(height: u16) -> ScrollArea {
    ScrollArea {
        content_size: Size::new(4, height),
        scrollbars: ScrollbarVisibility::Never,
    }
}

fn spawn_scrolled(app: &mut App, renders: &Arc<AtomicUsize>) -> Entity {
    app.world_mut()
        .spawn((
            UiWidget::new(Rows(Arc::clone(renders))),
            UiArea::Fixed(Rect::new(0, 0, 4, 2)),
            scroll_area(10),
        ))
        .id()
}

fn rasterizations(renders: &Arc<AtomicUsize>) -> usize {
    renders.load(Ordering::Relaxed)
}

#[test]
fn scrolled_content_rasterizes_once_across_frames() {
    let renders = Arc::new(AtomicUsize::new(0));
    let mut app = app();
    spawn_scrolled(&mut app, &renders);

    app.update();
    assert_eq!(rasterizations(&renders), 1);

    app.update();
    app.update();
    assert_eq!(rasterizations(&renders), 1, "steady frames reuse the cache");
}

#[test]
fn live_widgets_rasterize_every_frame() {
    let renders = Arc::new(AtomicUsize::new(0));
    let mut app = app();
    let widget = spawn_scrolled(&mut app, &renders);
    app.world_mut().entity_mut(widget).insert(LiveWidget);

    app.update();
    app.update();
    app.update();

    assert_eq!(
        rasterizations(&renders),
        3,
        "LiveWidget opts out of caching"
    );
}

#[test]
fn scrolling_a_cached_widget_still_moves_the_window() {
    let renders = Arc::new(AtomicUsize::new(0));
    let mut app = app();
    let widget = spawn_scrolled(&mut app, &renders);

    app.update();
    assert_eq!(composed_frame(&app), "r0  \nr1  ");

    app.world_mut()
        .entity_mut(widget)
        .insert(ScrollOffset(Position::new(0, 3)));
    app.update();

    assert_eq!(composed_frame(&app), "r3  \nr4  ");
    assert_eq!(rasterizations(&renders), 1, "offset alone is a window blit");
}

#[test]
fn replacing_the_widget_rerasterizes() {
    let renders = Arc::new(AtomicUsize::new(0));
    let mut app = app();
    let widget = spawn_scrolled(&mut app, &renders);

    app.update();
    assert_eq!(rasterizations(&renders), 1);

    app.world_mut()
        .entity_mut(widget)
        .insert(UiWidget::new(Rows(Arc::clone(&renders))));
    app.update();

    assert_eq!(rasterizations(&renders), 2, "a new widget invalidates");
}

#[test]
fn resizing_the_content_rerasterizes() {
    let renders = Arc::new(AtomicUsize::new(0));
    let mut app = app();
    let widget = spawn_scrolled(&mut app, &renders);

    app.update();
    assert_eq!(rasterizations(&renders), 1);

    app.world_mut().entity_mut(widget).insert(scroll_area(20));
    app.update();

    assert_eq!(rasterizations(&renders), 2, "a resized entry is rebuilt");
}

#[test]
fn a_respawned_widget_rasterizes_fresh() {
    let mut app = app();
    let first = Arc::new(AtomicUsize::new(0));
    let widget = spawn_scrolled(&mut app, &first);
    app.update();
    app.world_mut().entity_mut(widget).despawn();
    app.update();

    let second = Arc::new(AtomicUsize::new(0));
    spawn_scrolled(&mut app, &second);
    app.update();

    assert_eq!(rasterizations(&second), 1, "no stale entry is inherited");
}
