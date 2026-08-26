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
use plurimus_core::ratatui_core::text::{Line, Span, Text};
use ratatui_widgets::list::{List, ListItem as ListRow, ListState};

use crate::listbox::{ListBox, ListBoxCursor, ListBoxSelectionMarker, ListBoxStripe, ListItem};
use crate::rows::ContentDirty;
use crate::rows::{
    ActiveDescendant, CURSOR_SYMBOL, ListItemText, ListItemTrailing, Marked, cursor_style,
    cursor_symbol,
};
use plurimus_core::UiWidget;
use plurimus_ui::UiLabel;
use plurimus_ui::{Checked, ComputedWidgetArea, ScrollArea, UiStyle, UiTheme};
use plurimus_ui::{StateQuery, Stylable, StylistCache, decorate, hashed_bits, observed};

// `ListItem` catches a child that becomes a row without its label
// changing; the rest is what a row draws with.
pub(crate) type ListRowsChanged = Or<(
    Changed<ListItem>,
    Changed<UiLabel>,
    Changed<ListItemText>,
    Changed<ListItemTrailing>,
    Changed<UiStyle>,
    Changed<Checked>,
    Changed<Marked>,
)>;

// The cursor row is absent by design: it reaches the stylist hashed into
// `StylistCache`, which needs no ordering against whatever moved it.
pub(crate) type ListSelfChanged = Or<(
    Changed<Children>,
    Changed<ListBoxCursor>,
    Changed<ListBoxSelectionMarker>,
    Changed<ListBoxStripe>,
)>;

const CHECKED_MARKER: &str = "▪ ";
/// Both markers are two cells wide by construction, which is what the
/// trailing slot measures against.
const MARKER_WIDTH: u16 = 2;
const UNCHECKED_MARKER: &str = "  ";

type RowItems<'w, 's> = Query<
    'w,
    's,
    (
        &'static UiLabel,
        Option<&'static ListItemText>,
        (Has<Checked>, Has<Marked>),
        Option<&'static ListItemTrailing>,
        Option<&'static UiStyle>,
    ),
    With<ListItem>,
>;

/// What one row draws with, before the list's gutters go on.
struct RowContent<'a> {
    label: &'a Line<'static>,
    text: Option<&'a ListItemText>,
    lit: bool,
    trailing: Option<&'a ListItemTrailing>,
    over: Option<&'a UiStyle>,
    banded: Option<Style>,
}

struct RowStyles {
    every: Style,
    cursor: Style,
    stripe: Option<Style>,
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
        (
            Option<&'static ListBoxCursor>,
            Option<&'static ListBoxStripe>,
        ),
        (&'static ComputedWidgetArea, Option<&'static ScrollArea>),
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
    for (state, active, children, marker, (cursor, stripe), size, content, mut cache, mut widget) in
        &mut boxes
    {
        let cursored = active.0.is_some();
        // Widths only: an idle frame reaches the gate below without building
        // a line, and a resize re-aligns rows that never changed.
        let width = row_width(size, (marker, cursor_width(cursor), cursored));
        let next = observed(state, &focus, hashed_bits((active.0, width)));
        if !cache.redraws(next, theme.is_changed() || content.is_changed()) {
            continue;
        }
        let styles = RowStyles {
            every: next.resting_style(&theme),
            cursor: cursor_style(next, cursored, &theme),
            stripe: stripe.map(|stripe| stripe.0),
        };
        let gutters = Gutters {
            marker,
            cursor: cursor_symbol(cursor.map(|cursor| &cursor.0)),
        };
        *widget = list_widget((children, &items), active.0, (gutters, width), styles);
    }
}

/// The width a row's content is drawn into: what a scrolled list windows,
/// else the area it occupies, less the gutters the list puts on first.
///
/// The cursor gutter counts only while a row is current, because that is
/// when ratatui reserves it - which is also why the cursor joins the redraw
/// comparison: gaining one shifts every row.
fn row_width(
    (area, scroll): (&ComputedWidgetArea, Option<&ScrollArea>),
    (marker, cursor, cursored): (bool, u16, bool),
) -> u16 {
    let drawn = scroll.map_or(area.0.width, |scroll| scroll.content_width(area.0.width));
    let reserved = if marker { MARKER_WIDTH } else { 0 };
    let reserved = reserved.saturating_add(if cursored { cursor } else { 0 });
    drawn.saturating_sub(reserved)
}

/// The cells the cursor gutter takes, without building the line itself -
/// this is read before the redraw gate, where an allocation would be paid
/// on every idle frame.
fn cursor_width(over: Option<&ListBoxCursor>) -> u16 {
    let width = over.map_or(CURSOR_SYMBOL.chars().count(), |cursor| cursor.0.width());
    u16::try_from(width).unwrap_or(u16::MAX)
}

fn list_widget(
    (children, items): (&Children, &RowItems),
    active: Option<Entity>,
    (gutters, width): (Gutters, u16),
    styles: RowStyles,
) -> UiWidget {
    let mut rows = Vec::new();
    let mut selected = None;
    for &child in children {
        let Ok((label, text, (checked, marked), trailing, over)) = items.get(child) else {
            continue;
        };
        if active == Some(child) {
            selected = Some(rows.len());
        }
        let banded = styles.stripe.filter(|_| rows.len() % 2 == 1);
        rows.push(list_row(
            &RowContent {
                label: &label.0,
                text,
                lit: checked || marked,
                trailing,
                over,
                banded,
            },
            (gutters.marker, width),
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

/// `label` with `trailing` pushed to `width`'s right edge, keeping at least
/// one space between them so a list too narrow for both truncates rather
/// than running the two together.
fn aligned_line(label: &Line<'static>, trailing: &Line<'static>, width: u16) -> Line<'static> {
    let used = label.width().saturating_add(trailing.width());
    let gap = usize::from(width).saturating_sub(used).max(1);
    let mut aligned = label.clone();
    aligned.push_span(Span::raw(" ".repeat(gap)));
    aligned.spans.extend(trailing.spans.iter().cloned());
    aligned
}

fn list_row(content: &RowContent, (marker, width): (bool, u16)) -> ListRow<'static> {
    let mark = if content.lit {
        CHECKED_MARKER
    } else {
        UNCHECKED_MARKER
    };
    let source = content.text.map_or(slice::from_ref(content.label), |text| {
        text.0.lines.as_slice()
    });
    // The trailing slot rides the first line only, which is the one a
    // reader's eye ends on.
    let first = content
        .trailing
        .zip(source.first())
        .map(|(trailing, line)| aligned_line(line, &trailing.0, width));
    // Ratatui blanks the cursor gutter below a row's first line; the
    // marker column is ours, so its continuation blanks are drawn here.
    let mut drawn = Text::from(
        source
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let line = first.as_ref().filter(|_| index == 0).unwrap_or(line);
                match (marker, index) {
                    (false, _) => line.clone(),
                    (true, 0) => decorate(mark, line, ""),
                    (true, _) => decorate(UNCHECKED_MARKER, line, ""),
                }
            })
            .collect::<Vec<_>>(),
    );
    if let Some(text) = content.text {
        drawn.style = text.0.style;
        drawn.alignment = text.0.alignment;
    }
    // Applied over the whole row rather than the label's own cells, which
    // is what reaches the cursor gutter. An unset `Style` patches as the
    // identity, so an absent stripe or override drops out of the chain.
    let style = content
        .banded
        .unwrap_or_default()
        .patch(content.over.map_or_else(Style::default, |over| over.0));
    ListRow::new(drawn).style(style)
}
