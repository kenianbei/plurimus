//! The widget primitive: any ratatui widget carried by an entity.
//!
//! [`TerminalWidget`] is blanket-implemented for everything whose reference
//! implements ratatui's [`Widget`], so the ecosystem drops in without glue.
//! A [`UiWidget`] holds that content behind an [`Arc`] and is immutable - an
//! app replaces the component to change what is drawn, which is what lets the
//! render sub-app share the content out to a pipeline rather than copy it.
//! [`placement`] supplies the vocabulary for where a widget lands; the
//! `raster` submodule extracts and draws it.

use std::sync::Arc;

use bevy_ecs::prelude::Component;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::{StatefulWidget, Widget};

use placement::{ComputedUiCamera, UiArea};

pub mod placement;
pub(crate) mod raster;

/// A widget renderable into a camera buffer.
///
/// Blanket-implemented for every type whose reference implements
/// [`Widget`] - all ratatui built-ins and idiomatic third-party widgets.
/// A widget following that convention needs nothing else. One that does
/// not - drawing through the subcell grids rather than into cells - can
/// implement this trait directly instead, which the `headless` example
/// does: the blanket impl is conditional, so it does not collide.
pub trait TerminalWidget: Send + Sync + 'static {
    /// Renders the widget into `area` of `buffer`.
    fn render(&self, area: Rect, buffer: &mut Buffer);
}

impl<W> TerminalWidget for W
where
    W: Send + Sync + 'static,
    for<'a> &'a W: Widget,
{
    fn render(&self, area: Rect, buffer: &mut Buffer) {
        Widget::render(self, area, buffer);
    }
}

/// An entity's widget content.
///
/// Content is immutable; replace the component to change what is drawn.
///
/// Widgets render with ratatui semantics directly into their target
/// camera's buffer: default-styled cells inherit whatever earlier passes
/// drew there, so a widget sharing a camera with a world pipeline (2d,
/// 3d) takes on that content's colors. Overlays belong on their own
/// camera - see [`UiCamera`](crate::UiCamera).
#[derive(Component, Clone)]
#[require(UiArea, ComputedUiCamera)]
pub struct UiWidget(pub(crate) Arc<dyn TerminalWidget>);

impl UiWidget {
    /// Wraps any [`TerminalWidget`] for spawning.
    #[must_use]
    pub fn new(widget: impl TerminalWidget) -> Self {
        Self(Arc::new(widget))
    }

    /// Wraps a ratatui [`StatefulWidget`] with a snapshot of its state.
    ///
    /// State mutations made by the widget during render are discarded:
    /// the app owns state in components and replaces the `UiWidget` when
    /// it changes. The by-value + [`Clone`] bound admits widgets like
    /// `Scrollbar` that have no by-reference `StatefulWidget` impl.
    #[must_use]
    pub fn stateful<W, S>(widget: W, state: S) -> Self
    where
        W: StatefulWidget<State = S> + Clone + Send + Sync + 'static,
        S: Clone + Send + Sync + 'static,
    {
        Self::new(StatefulPair { widget, state })
    }

    /// The shared widget content, for pipelines that rasterize the widget
    /// themselves (see [`RasterDeferred`](crate::RasterDeferred)).
    #[must_use]
    pub fn content(&self) -> Arc<dyn TerminalWidget> {
        Arc::clone(&self.0)
    }
}

/// A widget drawing nothing, so an entity has content to hold before its
/// first restyle replaces it - what a widget library's spawn bundles give a
/// widget that has not been styled yet.
///
/// Reach for it in a spawn bundle rather than `#[require(UiWidget)]` on a
/// widget marker: this type requires [`UiArea`], so requiring it from a
/// marker gives every entity carrying that marker an area of its own, which
/// is wrong wherever a layout engine already owns the rect - a `plurimus_bui`
/// node with a widget marker on it for behavior alone.
impl Default for UiWidget {
    fn default() -> Self {
        Self::new(Blank)
    }
}

struct Blank;

impl TerminalWidget for Blank {
    fn render(&self, _area: Rect, _buffer: &mut Buffer) {}
}

struct StatefulPair<W, S> {
    widget: W,
    state: S,
}

impl<W, S> Widget for &StatefulPair<W, S>
where
    W: StatefulWidget<State = S> + Clone,
    S: Clone,
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.widget
            .clone()
            .render(area, buf, &mut self.state.clone());
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;

    use super::UiWidget;

    #[test]
    fn a_default_widget_draws_nothing() {
        let area = Rect::new(0, 0, 4, 2);
        let mut buffer = Buffer::empty(area);
        buffer.set_string(0, 0, "keep", ratatui_core::style::Style::default());
        let untouched = buffer.clone();

        UiWidget::default().0.render(area, &mut buffer);

        assert_eq!(buffer, untouched);
    }
}
