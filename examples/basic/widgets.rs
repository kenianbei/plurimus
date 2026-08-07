//! One tile per stock ratatui widget, each wrapped in a titled block.
//!
//! Every constructor takes its tile's title and the animation frame;
//! still tiles ignore the frame.

use plurimus::core::UiWidget;
use plurimus::core::ratatui_core::buffer::Buffer;
use plurimus::core::ratatui_core::layout::{Constraint, Rect};
use plurimus::core::ratatui_core::style::{Color, Style};
use plurimus::core::ratatui_core::symbols::Marker;
use plurimus::core::ratatui_core::text::Line;
use plurimus::core::ratatui_core::widgets::{StatefulWidget, Widget};
use ratatui_widgets::barchart::BarChart;
use ratatui_widgets::block::Block;
use ratatui_widgets::canvas::{Canvas, Circle, Rectangle};
use ratatui_widgets::chart::{Axis, Chart, Dataset, GraphType};
use ratatui_widgets::gauge::{Gauge, LineGauge};
use ratatui_widgets::list::{List, ListState};
use ratatui_widgets::paragraph::{Paragraph, Wrap};
use ratatui_widgets::scrollbar::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui_widgets::sparkline::Sparkline;
use ratatui_widgets::table::{Row, Table};
use ratatui_widgets::tabs::Tabs;

/// A gallery tile: what to draw, and whether it redraws over time.
pub struct TileSpec {
    /// Block title, also the test's proof the tile rendered.
    pub title: &'static str,
    /// Builds the tile's content for a title and animation frame.
    pub build: fn(&'static str, u32) -> UiWidget,
    /// Whether the tile rebuilds as frames advance.
    pub animated: bool,
}

const fn tile(title: &'static str, build: fn(&'static str, u32) -> UiWidget) -> TileSpec {
    TileSpec {
        title,
        build,
        animated: false,
    }
}

const fn animated(title: &'static str, build: fn(&'static str, u32) -> UiWidget) -> TileSpec {
    TileSpec {
        title,
        build,
        animated: true,
    }
}

/// Every stock ratatui widget, in grid order.
pub static TILES: [TileSpec; 11] = [
    tile("paragraph", paragraph),
    tile("list", list),
    tile("table", table),
    animated("tabs", tabs),
    animated("gauge", gauge),
    animated("line gauge", line_gauge),
    tile("bar chart", bar_chart),
    animated("sparkline", sparkline),
    tile("chart", chart),
    tile("canvas", canvas),
    tile("scrollbar", scrollbar),
];

fn framed(title: &'static str) -> Block<'static> {
    Block::bordered()
        .title(title)
        .border_style(Style::new().fg(Color::DarkGray))
}

fn paragraph(title: &'static str, _frame: u32) -> UiWidget {
    UiWidget::new(
        Paragraph::new("Every widget here is an entity, placed in cells. Press q to quit.")
            .block(framed(title))
            .wrap(Wrap { trim: true }),
    )
}

fn list(title: &'static str, _frame: u32) -> UiWidget {
    // A ratatui StatefulWidget renders from a state snapshot the app
    // owns; here the snapshot selects a row.
    UiWidget::stateful(
        List::new(["alpha", "beta", "gamma", "delta"])
            .block(framed(title))
            .highlight_symbol("> ")
            .highlight_style(Style::new().fg(Color::Cyan)),
        ListState::default().with_selected(Some(1)),
    )
}

fn table(title: &'static str, _frame: u32) -> UiWidget {
    let rows = [
        Row::new(["core", "render"]),
        Row::new(["input", "keys"]),
        Row::new(["term", "present"]),
    ];
    let widths = [Constraint::Length(6), Constraint::Min(4)];
    UiWidget::new(Table::new(rows, widths).block(framed(title)))
}

const TAB_TITLES: [&str; 3] = ["one", "two", "three"];

fn tabs(title: &'static str, frame: u32) -> UiWidget {
    let selected = (frame / 8) as usize % TAB_TITLES.len();
    UiWidget::new(
        Tabs::new(TAB_TITLES.map(Line::from))
            .block(framed(title))
            .select(selected)
            .highlight_style(Style::new().fg(Color::Cyan)),
    )
}

// A triangle wave keeps the fill visibly moving in both directions.
fn wave(frame: u32, period: u32) -> f64 {
    let phase = frame % (period * 2);
    let rising = if phase < period {
        phase
    } else {
        period * 2 - phase
    };
    f64::from(rising) / f64::from(period)
}

fn gauge(title: &'static str, frame: u32) -> UiWidget {
    UiWidget::new(
        Gauge::default()
            .block(framed(title))
            .ratio(wave(frame + 16, 40))
            .gauge_style(Style::new().fg(Color::Green)),
    )
}

fn line_gauge(title: &'static str, frame: u32) -> UiWidget {
    UiWidget::new(
        LineGauge::default()
            .block(framed(title))
            .ratio(wave(frame + 20, 40))
            .filled_style(Style::new().fg(Color::Magenta)),
    )
}

fn bar_chart(title: &'static str, _frame: u32) -> UiWidget {
    let bars = [("a", 4_u64), ("b", 9), ("c", 6), ("d", 2)];
    UiWidget::new(
        BarChart::default()
            .block(framed(title))
            .data(&bars)
            .bar_width(3)
            .bar_style(Style::new().fg(Color::Yellow)),
    )
}

fn sparkline(title: &'static str, frame: u32) -> UiWidget {
    let samples: Vec<u64> = (0..24)
        .map(|step| u64::from((step + frame / 4) % 9))
        .collect();
    UiWidget::new(
        Sparkline::default()
            .block(framed(title))
            .data(samples)
            .style(Style::new().fg(Color::Cyan)),
    )
}

// Dataset borrows its points, so they outlive every frame as a static.
static CURVE: [(f64, f64); 5] = [(0.0, 0.0), (1.0, 2.0), (2.0, 1.0), (3.0, 3.0), (4.0, 2.0)];

fn chart(title: &'static str, _frame: u32) -> UiWidget {
    let datasets = vec![
        Dataset::default()
            .name("series")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .data(&CURVE)
            .style(Style::new().fg(Color::LightBlue)),
    ];
    UiWidget::new(
        Chart::new(datasets)
            .block(framed(title))
            .x_axis(Axis::default().bounds([0.0, 4.0]))
            .y_axis(Axis::default().bounds([0.0, 3.0])),
    )
}

fn canvas(title: &'static str, _frame: u32) -> UiWidget {
    UiWidget::new(
        Canvas::default()
            .block(framed(title))
            .marker(Marker::Braille)
            .x_bounds([-2.0, 2.0])
            .y_bounds([-2.0, 2.0])
            .paint(|context| {
                context.draw(&Circle {
                    x: 0.0,
                    y: 0.0,
                    radius: 1.4,
                    color: Color::Red,
                });
                context.draw(&Rectangle {
                    x: -0.7,
                    y: -0.7,
                    width: 1.4,
                    height: 1.4,
                    color: Color::White,
                });
            }),
    )
}

const SCROLLBAR_CONTENT: usize = 40;

// Scrollbar is the one stock widget with no block of its own; any type
// whose reference implements Widget is a TerminalWidget, this included.
struct ScrollbarTile {
    block: Block<'static>,
    bar: Scrollbar<'static>,
    state: ScrollbarState,
}

impl Widget for &ScrollbarTile {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self.block, area, buffer);
        let inner = self.block.inner(area);
        StatefulWidget::render(self.bar.clone(), inner, buffer, &mut self.state.clone());
    }
}

fn scrollbar(title: &'static str, _frame: u32) -> UiWidget {
    UiWidget::new(ScrollbarTile {
        block: framed(title),
        bar: Scrollbar::new(ScrollbarOrientation::VerticalRight),
        state: ScrollbarState::new(SCROLLBAR_CONTENT).position(SCROLLBAR_CONTENT / 3),
    })
}
