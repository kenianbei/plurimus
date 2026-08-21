//! The list box stylist: rows into a ratatui `List`.
//!
//! A list draws two things its rows do not own - the marker column beside
//! every row and the symbol beside the cursor row - and it draws them in two
//! styles rather than one. Keeping the cursor's style apart from every other
//! row's is what stops a focused list from repainting all of its rows.
//!
//! What makes it redraw is [`StylistCache`] for its own state and
//! [`ContentDirty`](crate::rows::ContentDirty) for its rows'.

use core::slice;

use bevy_ecs::change_detection::{DetectChanges, Ref};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Changed, Has, Or, Query, Res, With};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::{Line, Text};
use ratatui_widgets::list::{List, ListItem as ListRow, ListState};

use crate::listbox::{ListBox, ListBoxCursor, ListBoxSelectionMarker, ListItem};
use crate::rows::ContentDirty;
use crate::rows::{ActiveDescendant, ListItemText, cursor_symbol};
use plurimus_core::UiWidget;
use plurimus_ui::UiLabel;
use plurimus_ui::{Checked, UiStyle, UiTheme};
use plurimus_ui::{StateQuery, Stylable, StylistCache, decorate, hashed_bits, observed};

// `ListItem` catches a child that becomes a row without its label
// changing; the rest is what a row draws with.
pub(crate) type ListRowsChanged = Or<(
    Changed<ListItem>,
    Changed<UiLabel>,
    Changed<ListItemText>,
    Changed<UiStyle>,
    Changed<Checked>,
)>;

// The cursor row is absent by design: it reaches the stylist hashed into
// `StylistCache`, which needs no ordering against whatever moved it.
pub(crate) type ListSelfChanged = Or<(
    Changed<Children>,
    Changed<ListBoxCursor>,
    Changed<ListBoxSelectionMarker>,
)>;

const CHECKED_MARKER: &str = "▪ ";
const UNCHECKED_MARKER: &str = "  ";

type RowItems<'w, 's> = Query<
    'w,
    's,
    (
        &'static UiLabel,
        Option<&'static ListItemText>,
        Has<Checked>,
        Option<&'static UiStyle>,
    ),
    With<ListItem>,
>;

/// What one row draws with, before the list's gutters go on.
struct RowContent<'a> {
    label: &'a Line<'static>,
    text: Option<&'a ListItemText>,
    checked: bool,
    over: Option<&'a UiStyle>,
}

struct RowStyles {
    every: Style,
    cursor: Style,
}

struct Gutters {
    marker: bool,
    cursor: Line<'static>,
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
        // A `Copy` scalar, so an idle frame reaches the gate below without
        // touching a row.
        let next = observed(state, &focus, hashed_bits(active.0));
        if !cache.redraws(next, theme.is_changed() || content.is_changed()) {
            continue;
        }
        let styles = RowStyles {
            every: next.resting_style(&theme),
            cursor: cursor_style(next, active.0.is_some(), &theme),
        };
        let gutters = Gutters {
            marker,
            cursor: cursor_symbol(cursor.map(|cursor| &cursor.0)),
        };
        *widget = list_widget((children, &items), active.0, gutters, styles);
    }
}

// A list driven through `ActiveDescendant` while focus sits on whatever is
// doing the driving is still the thing being operated, so its cursor row
// takes the focused patch either way - otherwise the row a search field is
// stepping resolves to the resting style, an invisible cursor.
fn cursor_style(next: StylistCache, driven: bool, theme: &UiTheme) -> Style {
    next.with_focused(next.state().focused || driven)
        .style(theme)
}

fn list_widget(
    (children, items): (&Children, &RowItems),
    active: Option<Entity>,
    gutters: Gutters,
    styles: RowStyles,
) -> UiWidget {
    let mut rows = Vec::new();
    let mut selected = None;
    for &child in children {
        let Ok((label, text, checked, over)) = items.get(child) else {
            continue;
        };
        if active == Some(child) {
            selected = Some(rows.len());
        }
        rows.push(list_row(
            &RowContent {
                label: &label.0,
                text,
                checked,
                over,
            },
            gutters.marker,
        ));
    }
    let mut highlight = ListState::default();
    highlight.select(selected);
    UiWidget::stateful(
        List::new(rows)
            .style(styles.every)
            .highlight_style(styles.cursor)
            .highlight_symbol(gutters.cursor),
        highlight,
    )
}

fn list_row(content: &RowContent, marker: bool) -> ListRow<'static> {
    let mark = if content.checked {
        CHECKED_MARKER
    } else {
        UNCHECKED_MARKER
    };
    let source = content.text.map_or(slice::from_ref(content.label), |text| {
        text.0.lines.as_slice()
    });
    // Ratatui blanks the cursor gutter below a row's first line; the
    // marker column is ours, so its continuation blanks are drawn here.
    let mut drawn = Text::from(
        source
            .iter()
            .enumerate()
            .map(|(index, line)| match (marker, index) {
                (false, _) => line.clone(),
                (true, 0) => decorate(mark, line, ""),
                (true, _) => decorate(UNCHECKED_MARKER, line, ""),
            })
            .collect::<Vec<_>>(),
    );
    if let Some(text) = content.text {
        drawn.style = text.0.style;
        drawn.alignment = text.0.alignment;
    }
    let row = ListRow::new(drawn);
    // Applied over the whole row rather than the label's own cells, which
    // is what reaches the cursor gutter.
    match content.over {
        Some(over) => row.style(over.0),
        None => row,
    }
}
