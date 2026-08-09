//! The list box stylist: rows into a ratatui `List`.
//!
//! A list draws two things its rows do not own - the marker column beside
//! every row and the symbol beside the cursor row - and it draws them in two
//! styles rather than one. Keeping the cursor's style apart from every other
//! row's is what stops a focused list from repainting all of its rows.
//!
//! Two change signals meet here, as they do for the table.
//! [`StylistCache`] answers "does it draw differently" - hover, focus, the
//! cursor's row - and [`ContentDirty`] answers "does it draw something
//! else", which a query filter on the list cannot: rows are children, and a
//! child's change never marks its parent.

use bevy_ecs::change_detection::{DetectChanges, Ref};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Changed, Has, Or, Query, Res, With};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;
use ratatui_widgets::list::{List, ListItem as ListRow, ListState};

use crate::UiLabel;
use crate::listbox::{ActiveDescendant, ListBox, ListBoxCursor, ListBoxSelectionMarker, ListItem};
use crate::stylist::{
    CURSOR_SYMBOL, ContentDirty, StateQuery, Stylable, StylistCache, UiStyle, decorate,
    hashed_bits, observed,
};
use crate::theme::UiTheme;
use plurimus_core::UiWidget;
use plurimus_ui::Checked;

// `With<ListItem>` is load-bearing beside the `Or`, which matches an
// archetype holding any one of its terms; see `mark_dirty_content`.
pub(crate) type ListRowsChanged = (
    With<ListItem>,
    Or<(
        Changed<ListItem>,
        Changed<UiLabel>,
        Changed<UiStyle>,
        Changed<Checked>,
    )>,
);

// The cursor row is absent by design: it reaches the stylist hashed into
// `StylistCache`, which needs no ordering against whatever moved it.
pub(crate) type ListSelfChanged = (
    With<ListBox>,
    Or<(
        Changed<Children>,
        Changed<ListBoxCursor>,
        Changed<ListBoxSelectionMarker>,
    )>,
);

const CHECKED_MARKER: &str = "▪ ";
const UNCHECKED_MARKER: &str = "  ";

// A row's entity, label, checked state, and per-row style override.
type Row<'a> = (Entity, &'a Line<'static>, bool, Option<Style>);

type RowItems<'w, 's> =
    Query<'w, 's, (&'static UiLabel, Has<Checked>, Option<&'static UiStyle>), With<ListItem>>;

struct RowStyles {
    every: Style,
    cursor: Style,
}

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

type ListBoxes<'w, 's> = Query<
    'w,
    's,
    (
        StateQuery<'static>,
        &'static ActiveDescendant,
        &'static Children,
        Has<ListBoxSelectionMarker>,
        Option<&'static ListBoxCursor>,
        Ref<'static, ContentDirty<ListBox>>,
        &'static mut StylistCache,
        &'static mut UiWidget,
    ),
    Stylable<ListBox>,
>;

pub(crate) fn style_listboxes(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut boxes: ListBoxes,
    items: RowItems,
) {
    for (state, active, children, marker, cursor, content, mut cache, mut widget) in &mut boxes {
        // A `Copy` scalar, not the row content this once hashed: an idle
        // frame reaches the gate below without touching a row.
        let next = observed(state, &focus, hashed_bits(active.0));
        if !theme.is_changed() && !content.is_changed() && next == *cache {
            continue;
        }
        *cache = next;
        let rows = styled_rows(children, &items);
        let selected = active
            .0
            .and_then(|item| rows.iter().position(|(row, ..)| *row == item));
        let styles = RowStyles {
            every: next.resting_style(&theme),
            cursor: next.style(&theme),
        };
        let gutters = Gutters::new(marker, cursor.map(|cursor| &cursor.0));
        *widget = list_widget(&rows, selected, gutters, styles);
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
