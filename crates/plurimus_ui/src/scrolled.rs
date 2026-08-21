//! Scrolled widget content: cached rasterization and windowed extraction.
//!
//! Scrolled widgets carry [`RasterDeferred`](plurimus_core::RasterDeferred)
//! (required by [`ScrollArea`]), so core's widget extraction skips them;
//! extraction here renders the scroll window into an [`ExtractedWidget`]
//! that rasterizes in z-order with everything else.
//!
//! Rendering the whole scrollable content is too costly to repeat every
//! frame, so it is cached and only the visible window is blitted once it is
//! warm. Cache identity is the widget allocation itself, which works because
//! stylists hand out a fresh [`UiWidget`] whenever content changes - a widget
//! that mutates behind a lock breaks that signal and must say so with
//! [`LiveWidget`].

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use bevy_ecs::prelude::{Commands, Component, Entity, Has, ResMut, Resource, Without};
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::ratatui_core::widgets::{StatefulWidget, Widget};
use plurimus_core::{
    ComputedUiCamera, ExtractedWidget, MainWorld, TerminalWidget, UiArea, UiHidden, UiOrder,
    UiWidget,
};
use tui_scrollview::{ScrollView, ScrollViewState};

use crate::scroll::{ScrollArea, ScrollOffset};

/// Marks a widget whose rendered output can change while its [`UiWidget`]
/// stays the same component - content reached through interior mutability.
/// Required on any widget breaking the immutable-content contract, so the
/// render world re-rasterizes it instead of reusing earlier output.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LiveWidget;

/// Scroll-area content rasterized in an earlier frame, so steady-state
/// frames only blit the visible window.
///
/// Stylists allocate a fresh `UiWidget` whenever content changes, so a
/// widget keeping its allocation is the signal that its content still
/// holds. Widgets that mutate behind a lock break that signal and opt out
/// via [`LiveWidget`].
#[derive(Resource, Default)]
pub(crate) struct ScrolledContentCache {
    entries: HashMap<WidgetKey, CacheEntry>,
}

struct CacheEntry {
    scroll: ScrollArea,
    view: Arc<ScrollView>,
    touched: bool,
}

// Identity is the widget's allocation address. The map holding a strong
// reference is what licenses that: no later widget can reach an address
// while an entry for it is alive.
struct WidgetKey(Arc<dyn TerminalWidget>);

impl PartialEq for WidgetKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for WidgetKey {}

impl Hash for WidgetKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).cast::<()>().addr().hash(state);
    }
}

impl ScrolledContentCache {
    fn view_for(
        &mut self,
        content: &Arc<dyn TerminalWidget>,
        scroll: ScrollArea,
        cacheable: bool,
    ) -> Arc<ScrollView> {
        if !cacheable {
            return Arc::new(rasterize_content(content, scroll));
        }
        let entry = self
            .entries
            .entry(WidgetKey(Arc::clone(content)))
            .or_insert_with(|| CacheEntry {
                scroll,
                view: Arc::new(rasterize_content(content, scroll)),
                touched: true,
            });
        if entry.scroll != scroll {
            entry.scroll = scroll;
            entry.view = Arc::new(rasterize_content(content, scroll));
        }
        entry.touched = true;
        Arc::clone(&entry.view)
    }

    fn sweep(&mut self) {
        self.entries
            .retain(|_, entry| std::mem::take(&mut entry.touched));
    }
}

fn rasterize_content(content: &Arc<dyn TerminalWidget>, scroll: ScrollArea) -> ScrollView {
    let mut view = ScrollView::new(scroll.content_size).scrollbars_visibility(scroll.scrollbars);
    content.render(view.area(), view.buf_mut());
    view
}

/// The visible window of a scrolled widget, sharing the prepared view.
struct ScrolledWindow {
    view: Arc<ScrollView>,
    offset: Position,
}

impl Widget for &ScrolledWindow {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = ScrollViewState::with_offset(self.offset);
        StatefulWidget::render(&*self.view, area, buf, &mut state);
    }
}

pub(crate) fn extract_scrolled_widgets(
    mut main_world: ResMut<MainWorld>,
    mut cache: ResMut<ScrolledContentCache>,
    mut commands: Commands,
) {
    let mut widgets = main_world.query_filtered::<(
        Entity,
        &UiWidget,
        &UiArea,
        Option<&UiOrder>,
        &ComputedUiCamera,
        &ScrollArea,
        &ScrollOffset,
        Has<LiveWidget>,
    ), Without<UiHidden>>();
    for (source, widget, area, order, camera, scroll, offset, live) in widgets.iter(&main_world) {
        let view = cache.view_for(&widget.content(), *scroll, !live);
        let window = Arc::new(ScrolledWindow {
            view,
            offset: offset.0,
        });
        commands.spawn(
            ExtractedWidget::new(window, *area, source)
                .with_order(order.map_or(0, |order| order.0))
                .with_target(camera.0),
        );
    }
    cache.sweep();
}

#[cfg(test)]
mod tests {
    use plurimus_core::ratatui_core::layout::Size;
    use ratatui_widgets::paragraph::Paragraph;

    use super::{CacheEntry, ScrollArea, ScrolledContentCache, WidgetKey, rasterize_content};
    use plurimus_core::TerminalWidget;
    use std::sync::Arc;

    #[test]
    fn sweep_drops_entries_a_frame_did_not_touch() {
        let mut cache = ScrolledContentCache::default();
        let widget: Arc<dyn TerminalWidget> = Arc::new(Paragraph::new("x"));
        let scroll = ScrollArea::new(Size::new(2, 4));
        cache.entries.insert(
            WidgetKey(Arc::clone(&widget)),
            CacheEntry {
                scroll,
                view: Arc::new(rasterize_content(&widget, scroll)),
                touched: true,
            },
        );

        cache.sweep();
        assert_eq!(cache.entries.len(), 1, "a touched entry survives");

        cache.sweep();
        assert!(cache.entries.is_empty(), "an untouched entry is dropped");
    }
}
