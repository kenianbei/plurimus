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
use plurimus_core::ratatui_core::layout::Alignment;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::{Line, Span, Text};
use ratatui_widgets::list::{List, ListItem as ListRow, ListState};

use crate::listbox::{ListBox, ListBoxCursor, ListBoxSelectionMarker, ListItem, ListItemTrailing};
use crate::rows::ContentDirty;
use crate::rows::{ActiveDescendant, ListItemText, Marked, cursor_symbol};
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
)>;

const CHECKED_MARKER: &str = "▪ ";
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
    for (state, active, children, marker, cursor, size, content, mut cache, mut widget) in
        &mut boxes
    {
        let gutters = Gutters {
            marker,
            cursor: cursor_symbol(cursor.map(|cursor| &cursor.0)),
        };
        let width = row_width(size, &gutters, active.0.is_some());
        // Copy scalars, so an idle frame reaches the gate below without
        // touching a row. The width joins them because a resize re-aligns
        // every trailing slot without any row having changed.
        let next = observed(state, &focus, hashed_bits((active.0, width)));
        if !cache.redraws(next, theme.is_changed() || content.is_changed()) {
            continue;
        }
        let styles = RowStyles {
            every: next.resting_style(&theme),
            cursor: cursor_style(next, active.0.is_some(), &theme),
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
    gutters: &Gutters,
    cursored: bool,
) -> u16 {
    let drawn = scroll.map_or(area.0.width, |scroll| scroll.content_width(area.0.width));
    let marker = if gutters.marker {
        u16::try_from(UNCHECKED_MARKER.chars().count()).unwrap_or(u16::MAX)
    } else {
        0
    };
    let cursor = if cursored {
        u16::try_from(gutters.cursor.width()).unwrap_or(u16::MAX)
    } else {
        0
    };
    drawn.saturating_sub(marker).saturating_sub(cursor)
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
        rows.push(list_row(
            &RowContent {
                label: &label.0,
                text,
                lit: checked || marked,
                trailing,
                over,
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
    let mut spans = label.spans.clone();
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(trailing.spans.iter().cloned());
    Line::from(spans)
        .style(label.style)
        .alignment(label.alignment.unwrap_or(Alignment::Left))
}

fn list_row(content: &RowContent, (marker, width): (bool, u16)) -> ListRow<'static> {
    let mark = if content.lit {
        CHECKED_MARKER
    } else {
        UNCHECKED_MARKER
    };
    let first = content.trailing.map(|trailing| {
        aligned_line(
            content.text.map_or(content.label, |text| {
                text.0.lines.first().unwrap_or(content.label)
            }),
            &trailing.0,
            width,
        )
    });
    let source = content.text.map_or(slice::from_ref(content.label), |text| {
        text.0.lines.as_slice()
    });
    let source = match (&first, source.split_first()) {
        (Some(first), Some((_, rest))) => {
            let mut lines = vec![first.clone()];
            lines.extend_from_slice(rest);
            std::borrow::Cow::Owned(lines)
        }
        _ => std::borrow::Cow::Borrowed(source),
    };
    let source = source.as_ref();
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
