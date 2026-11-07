//! The tab bar on `bevy_ui_widgets`' own [`TabList`] and [`Tab`].
//!
//! The model is upstream's: each tab is focusable on a roving
//! [`TabIndex`](bevy_input_focus::tab_navigation::TabIndex), the selection
//! is the list's [`SelectedTab`](crate::SelectedTab) mirrored onto its tabs
//! as [`Selected`](plurimus_ui::Selected), and a click or a key emits
//! `ValueChange<Option<Entity>>` on the list for an app to apply or for
//! [`tablist_self_update`](crate::tablist_self_update) to. What a cell bar
//! adds is placement - the list lays its tabs along
//! [`TabList::orientation`], so the keys and the layout cannot disagree -
//! and drawing: the list draws chrome, its fill, the dividers, and the
//! baseline a joined look opens the selected box onto, and each tab draws
//! its own label in the list's [`TabBarLook`].

mod boxed;
mod layout;
mod style;

pub(crate) use layout::place_tab_items;
pub(crate) use style::{TabBarSelfChanged, TabItemsChanged, style_tab_bars, style_tab_items};

use bevy_ecs::bundle::Bundle;
use bevy_ecs::prelude::Component;
use plurimus_core::Edge;
use plurimus_core::ratatui_core::style::{Modifier, Style};
use plurimus_core::ratatui_core::text::Line;
use ratatui_widgets::borders::BorderType;

use crate::rows::ContentDirty;
use plurimus_core::{UiOrder, UiWidget};
use plurimus_ui::bevy_ui_widgets::{ControlOrientation, Tab, TabList};
use plurimus_ui::{ComputedWidgetArea, Hovered, StylistCache, UiLabel};

/// Cells a border takes on each side of a boxed item.
pub(crate) const FRAME: u16 = 1;

/// How a tab bar draws its tabs.
///
/// The looks are field settings rather than variants, so they compose:
/// the default is a plain strip of padded labels; a `divider` draws
/// between tabs; a `border` boxes each tab, making the bar three cells
/// thick; `joined` opens the selected box onto a baseline along one edge of
/// the bar. [`thickness`](Self::thickness) is what the cross axis needs.
/// Which way the tabs run is the [`TabList`]'s, beside it.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TabBarLook {
    /// Boxes each item in this border; `None` draws bare labels.
    pub border: Option<BorderType>,
    /// Drawn in the one cell between items; `None` leaves them touching.
    pub divider: Option<Line<'static>>,
    /// Cells beside a label, inside any border.
    pub padding: u16,
    /// The edge of the bar the selected box opens onto, drawing a baseline
    /// along it. Only an edge across the bar's axis - `Top` or `Bottom` on
    /// a horizontal bar, `Left` or `Right` on a vertical one - and only a
    /// border with junction glyphs can join; any other setting draws
    /// closed boxes.
    pub joined: Option<Edge>,
}

impl TabBarLook {
    /// Sets the border boxing each item.
    #[must_use]
    pub const fn with_border(mut self, border: Option<BorderType>) -> Self {
        self.border = border;
        self
    }

    /// Sets what is drawn between items.
    #[must_use]
    pub fn with_divider(mut self, divider: Option<Line<'static>>) -> Self {
        self.divider = divider;
        self
    }

    /// Sets the cells beside a label.
    #[must_use]
    pub const fn with_padding(mut self, padding: u16) -> Self {
        self.padding = padding;
        self
    }

    /// Sets the edge the active box opens onto.
    #[must_use]
    pub const fn with_joined(mut self, joined: Option<Edge>) -> Self {
        self.joined = joined;
        self
    }

    /// Cells the bar needs across its axis: one for bare labels, three
    /// for boxed ones.
    #[must_use]
    pub const fn thickness(&self) -> u16 {
        1 + 2 * self.frame()
    }

    pub(crate) const fn frame(&self) -> u16 {
        if self.border.is_some() { FRAME } else { 0 }
    }

    pub(crate) fn joint(&self, orientation: ControlOrientation) -> Option<boxed::Joint> {
        let edge = self.joined?;
        let across = match orientation {
            ControlOrientation::Horizontal => matches!(edge, Edge::Top | Edge::Bottom),
            ControlOrientation::Vertical => matches!(edge, Edge::Left | Edge::Right),
        };
        if !across {
            return None;
        }
        let lines = boxed::border_lines(self.border?)?;
        Some(boxed::Joint { edge, lines })
    }
}

impl Default for TabBarLook {
    fn default() -> Self {
        Self {
            border: None,
            divider: None,
            padding: 1,
            joined: None,
        }
    }
}

/// Styles a tab bar's selected tab - its label and, when boxed, its
/// border - patched over the theme's style and beneath the tab's own
/// [`UiStyle`](plurimus_ui::UiStyle), so the selected tab reads wherever
/// focus is. Defaults to bold alone; an app naming a color here
/// should name a foreground, since a background fills the whole box.
#[derive(Component, Debug, Clone, Copy)]
pub struct TabBarActiveStyle(pub Style);

impl Default for TabBarActiveStyle {
    fn default() -> Self {
        Self(Style::new().add_modifier(Modifier::BOLD))
    }
}

/// A [`TabList`] drawn at a cell rect, in its [`TabBarLook`].
///
/// Requires the list and what a cell bar is drawn and placed with, so each
/// can be given beside it in one spawn - a look, or a `TabList` of another
/// orientation or activation. The list is upstream's default otherwise:
/// horizontal, and manual, so the arrows move focus among the tabs and
/// Enter or space selects.
#[derive(Component, Debug, Clone, Copy)]
#[require(
    TabList,
    TabBarLook,
    TabBarActiveStyle,
    StylistCache,
    ComputedWidgetArea,
    ContentDirty<Self>
)]
pub struct TabBar;

/// Spawn bundle for a tab bar; parent [`tab_item`]s to it.
#[must_use]
pub fn tab_bar() -> impl Bundle {
    (TabBar, UiWidget::default())
}

/// Spawn bundle for one tab, placed by its bar.
pub fn tab_item(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        Tab,
        UiLabel(label.into()),
        Hovered::default(),
        StylistCache::default(),
        ComputedWidgetArea::default(),
        UiOrder::default(),
        UiWidget::default(),
    )
}
