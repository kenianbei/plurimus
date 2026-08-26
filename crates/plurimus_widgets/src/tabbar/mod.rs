//! The tab bar widget: a focusable strip of [`TabItem`] children.
//!
//! The bar is the one tab stop and holds the keys; its items are child
//! entities labelled by [`UiLabel`], placed by the bar and drawn by
//! themselves. The active item is the child carrying
//! [`Checked`](plurimus_ui::Checked), as a [`RadioGroup`](crate::RadioGroup)'s
//! selected option is: the bar holds no cursor, so stepping activates,
//! emitting [`ValueChange<Entity>`](crate::ValueChange) on the bar, and a
//! controlled app applies it while an uncontrolled one attaches
//! [`tab_bar_self_update`](crate::tab_bar_self_update).
//!
//! What the bar draws is chrome - its fill, the dividers, and the baseline
//! a joined look opens the active box onto. What an item draws is its own
//! label in the bar's [`TabBarLook`], so hover, press, disabled and a
//! per-item [`UiStyle`](plurimus_ui::UiStyle) cost nothing on an idle frame.

mod input;
mod layout;
mod style;

pub(crate) use input::{tab_bar_key, tab_item_click};
pub(crate) use layout::place_tab_items;
pub(crate) use style::{style_tab_bars, style_tab_items};

use bevy_ecs::bundle::Bundle;
use bevy_ecs::prelude::Component;
use bevy_input::keyboard::Key;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::Edge;
use plurimus_core::ratatui_core::style::{Modifier, Style};
use plurimus_core::ratatui_core::text::Line;
use ratatui_widgets::borders::BorderType;

use plurimus_core::{UiOrder, UiWidget};
use plurimus_ui::{ComputedWidgetArea, Hovered, KeyBinding, StylistCache, UiLabel};

/// Cells a border takes on each side of a boxed item.
pub(crate) const FRAME: u16 = 1;

/// A focusable strip of [`TabItem`] children, drawn in its [`TabBarLook`].
///
/// The one tab stop of the strip: keys are observed here and the active
/// item is drawn as focused while the bar holds focus. Activation emits
/// [`ValueChange<Entity>`](crate::ValueChange) naming the item; attach
/// [`tab_bar_self_update`](crate::tab_bar_self_update) for uncontrolled
/// behavior.
#[derive(Component, Debug, Clone, Copy)]
#[require(
    StylistCache,
    TabIndex,
    TabBarKeys,
    TabBarLook,
    TabBarActiveStyle,
    ComputedWidgetArea
)]
pub struct TabBar;

/// One tab of a [`TabBar`]: a child entity labelled by [`UiLabel`], the
/// active one carrying [`Checked`](plurimus_ui::Checked). Placed by the
/// bar, so an app gives it no area of its own.
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache, ComputedWidgetArea, UiOrder)]
pub struct TabItem;

/// Which way a [`TabBar`] runs.
///
/// Closed: a strip runs one of two ways.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TabBarOrientation {
    /// Items side by side, left to right.
    #[default]
    Horizontal,
    /// Items stacked, top to bottom, each as wide as the bar.
    Vertical,
}

/// How a [`TabBar`] draws its items.
///
/// The looks are field settings rather than variants, so they compose:
/// the default is a plain strip of padded labels; a `divider` draws
/// between items; a `border` boxes each item, making the bar three cells
/// thick; `joined` opens the active box onto a baseline along one edge of
/// the bar. [`thickness`](Self::thickness) is what the cross axis needs.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TabBarLook {
    /// Which way the items run.
    pub orientation: TabBarOrientation,
    /// Boxes each item in this border; `None` draws bare labels.
    pub border: Option<BorderType>,
    /// Drawn in the one cell between items; `None` leaves them touching.
    pub divider: Option<Line<'static>>,
    /// Cells beside a label, inside any border.
    pub padding: u16,
    /// The edge of the bar the active box opens onto, drawing a baseline
    /// along it. Only an edge across the bar's axis - `Top` or `Bottom` on
    /// a horizontal bar, `Left` or `Right` on a vertical one - and only a
    /// border with junction glyphs can join; any other setting draws
    /// closed boxes.
    pub joined: Option<Edge>,
}

impl TabBarLook {
    /// Sets which way the items run.
    #[must_use]
    pub const fn with_orientation(mut self, orientation: TabBarOrientation) -> Self {
        self.orientation = orientation;
        self
    }

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
        if self.border.is_some() {
            1 + 2 * FRAME
        } else {
            1
        }
    }

    pub(crate) const fn frame(&self) -> u16 {
        if self.border.is_some() { FRAME } else { 0 }
    }
}

impl Default for TabBarLook {
    fn default() -> Self {
        Self {
            orientation: TabBarOrientation::Horizontal,
            border: None,
            divider: None,
            padding: 1,
            joined: None,
        }
    }
}

/// Styles a [`TabBar`]'s active item, patched over the theme's style and
/// beneath the item's own [`UiStyle`](plurimus_ui::UiStyle), so the active
/// tab reads while the bar does not hold focus. Defaults to reversed.
#[derive(Component, Debug, Clone, Copy)]
pub struct TabBarActiveStyle(pub Style);

impl Default for TabBarActiveStyle {
    fn default() -> Self {
        Self(Style::new().add_modifier(Modifier::REVERSED))
    }
}

/// What a key does to a [`TabBar`]. Stepping activates: there is no cursor
/// to move first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TabBarAction {
    /// Activate the item before the active one; nothing on the first.
    Previous,
    /// Activate the item after the active one; nothing on the last, and
    /// the first when none is active.
    Next,
    /// Activate the first item.
    First,
    /// Activate the last item.
    Last,
    /// Activate the item at this index among the enabled ones. An index
    /// past the last activates nothing and lets the key propagate.
    Select(usize),
}

/// A [`TabBar`]'s key bindings, scanned in order so the first match wins.
///
/// Replace it to remap: `[` and `]`, or digits through
/// [`TabBarAction::Select`], are an app's to bind. Defaults to the arrows
/// on both axes, whatever the orientation, and `Home` and `End`; no
/// printable key is bound.
#[derive(Component, Debug, Clone)]
pub struct TabBarKeys(pub Vec<(KeyBinding, TabBarAction)>);

impl Default for TabBarKeys {
    fn default() -> Self {
        Self(vec![
            (Key::ArrowLeft.into(), TabBarAction::Previous),
            (Key::ArrowUp.into(), TabBarAction::Previous),
            (Key::ArrowRight.into(), TabBarAction::Next),
            (Key::ArrowDown.into(), TabBarAction::Next),
            (Key::Home.into(), TabBarAction::First),
            (Key::End.into(), TabBarAction::Last),
        ])
    }
}

/// Spawn bundle for a tab bar; parent [`tab_item`]s to it.
#[must_use]
pub fn tab_bar() -> impl Bundle {
    (TabBar, UiWidget::default())
}

/// Spawn bundle for one tab.
pub fn tab_item(label: impl Into<Line<'static>>) -> impl Bundle {
    (TabItem, UiLabel(label.into()), UiWidget::default())
}
