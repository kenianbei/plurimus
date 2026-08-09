//! The list box stylist: rows into a ratatui `List`.
//!
//! A list draws two things its rows do not own - the marker column beside
//! every row and the symbol beside the cursor row - and it draws them in two
//! styles rather than one. Keeping the cursor's style apart from every other
//! row's is what stops a focused list from repainting all of its rows.
//!
//! What makes it redraw is [`StylistCache`] for its own state and
//! [`ContentDirty`](crate::rows::ContentDirty) for its rows'.

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
use crate::rows::ContentDirty;
use crate::stylist::{
    StateQuery, Stylable, StylistCache, UiStyle, cursor_symbol, decorate, hashed_bits, observed,
};
use crate::theme::UiTheme;
use plurimus_core::UiWidget;
use plurimus_ui::Checked;

// `ListItem` catches a child that becomes a row without its label
// changing; the rest is what a row draws with.
pub(crate) type ListRowsChanged = Or<(
    Changed<ListItem>,
    Changed<UiLabel>,
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
        let styles = RowStyles {
            every: next.resting_style(&theme),
            cursor: next.style(&theme),
        };
        let gutters = Gutters {
            marker,
            cursor: cursor_symbol(cursor.map(|cursor| &cursor.0)),
        };
        *widget = list_widget((children, &items), active.0, gutters, styles);
    }
}

// One pass over the children builds the rows and notes where the cursor
// landed among them, as the table's `bands` does.
fn list_widget(
    (children, items): (&Children, &RowItems),
    active: Option<Entity>,
    gutters: Gutters,
    styles: RowStyles,
) -> UiWidget {
    let mut rows = Vec::new();
    let mut selected = None;
    for &child in children {
        let Ok((label, checked, over)) = items.get(child) else {
            continue;
        };
        if active == Some(child) {
            selected = Some(rows.len());
        }
        rows.push(list_row((&label.0, checked, over), gutters.marker));
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

fn list_row(
    (label, checked, over): (&Line<'static>, bool, Option<&UiStyle>),
    marker: bool,
) -> ListRow<'static> {
    let line = if marker {
        let mark = if checked {
            CHECKED_MARKER
        } else {
            UNCHECKED_MARKER
        };
        decorate(mark, label, "")
    } else {
        label.clone()
    };
    let row = ListRow::new(line);
    // Applied over the whole row rather than the label's own cells, which
    // is what reaches the cursor gutter.
    match over {
        Some(over) => row.style(over.0),
        None => row,
    }
}
