//! The list box stylist: rows into a ratatui `List`.
//!
//! A list draws two things its rows do not own - the marker column beside
//! every row and the symbol beside the cursor row - and it draws them in two
//! styles rather than one. Keeping the cursor's style apart from every other
//! row's is what stops a focused list from repainting all of its rows.

use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Has, Query, Res, With};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;
use ratatui_widgets::list::{List, ListItem as ListRow, ListState};

use crate::UiLabel;
use crate::listbox::{ActiveDescendant, ListBox, ListBoxCursor, ListBoxSelectionMarker, ListItem};
use crate::stylist::{
    StateQuery, Stylable, StylistCache, UiStyle, decorate, hashed_bits, observed,
};
use crate::theme::UiTheme;
use plurimus_core::UiWidget;
use plurimus_ui::Checked;

// Drawn beside the cursor row unless a `ListBoxCursor` replaces it.
const CURSOR_SYMBOL: &str = "> ";

// Drawn beside every row when the marker column is asked for.
const CHECKED_MARKER: &str = "▪ ";
const UNCHECKED_MARKER: &str = "  ";

// A row's entity, label, checked state, and per-row style override.
type Row<'a> = (Entity, &'a Line<'static>, bool, Option<Style>);

type RowItems<'w, 's> =
    Query<'w, 's, (&'static UiLabel, Has<Checked>, Option<&'static UiStyle>), With<ListItem>>;

// What every row is drawn in, and what the cursor row adds on top.
struct RowStyles {
    every: Style,
    cursor: Style,
}

// The columns a list draws left of its rows: a marker beside every row,
// and a symbol beside the cursor row.
struct Gutters {
    marker: bool,
    cursor: Line<'static>,
}

impl Gutters {
    fn new(marker: bool, cursor: Option<&Line<'static>>) -> Self {
        Self {
            marker,
            cursor: cursor.cloned().unwrap_or_else(|| Line::from(CURSOR_SYMBOL)),
        }
    }
}

pub(crate) fn style_listboxes(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut boxes: Query<
        (
            StateQuery,
            &ActiveDescendant,
            &Children,
            Has<ListBoxSelectionMarker>,
            Option<&ListBoxCursor>,
            &mut StylistCache,
            &mut UiWidget,
        ),
        Stylable<ListBox>,
    >,
    items: RowItems,
) {
    for (state, active, children, marker, cursor, mut cache, mut widget) in &mut boxes {
        let rows = styled_rows(children, &items);
        let selected = active
            .0
            .and_then(|item| rows.iter().position(|(row, ..)| *row == item));
        let symbol = cursor.map(|cursor| &cursor.0);

        let next = observed(
            state,
            &focus,
            hashed_bits((&rows, selected, marker, symbol)),
        );
        if !theme.is_changed() && next == *cache {
            continue;
        }
        *cache = next;
        let styles = RowStyles {
            every: next.resting_style(&theme),
            cursor: next.style(&theme),
        };
        *widget = list_widget(&rows, selected, Gutters::new(marker, symbol), styles);
    }
}

fn styled_rows<'a>(children: &Children, items: &'a RowItems) -> Vec<Row<'a>> {
    children
        .iter()
        .filter_map(|&child| {
            let (label, checked, over) = items.get(child).ok()?;
            Some((child, &label.0, checked, over.map(|style| style.0)))
        })
        .collect()
}

fn list_widget(
    rows: &[Row],
    selected: Option<usize>,
    gutters: Gutters,
    styles: RowStyles,
) -> UiWidget {
    let items: Vec<ListRow> = rows
        .iter()
        .map(|row| list_row(row, gutters.marker))
        .collect();
    let mut highlight = ListState::default();
    highlight.select(selected);
    UiWidget::stateful(
        List::new(items)
            .style(styles.every)
            .highlight_style(styles.cursor)
            .highlight_symbol(gutters.cursor),
        highlight,
    )
}

fn list_row((_, label, checked, over): &Row, marker: bool) -> ListRow<'static> {
    let line = if marker {
        let mark = if *checked {
            CHECKED_MARKER
        } else {
            UNCHECKED_MARKER
        };
        decorate(mark, label, "")
    } else {
        (*label).clone()
    };
    let row = ListRow::new(line);
    // Applied over the whole row rather than the label's own cells, which
    // is what reaches the cursor gutter.
    match over {
        Some(over) => row.style(*over),
        None => row,
    }
}
