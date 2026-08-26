//! The two tab bar stylists: the bar's chrome, and each item's label.
//!
//! An item never holds focus and never learns the bar's look on its own,
//! so its stylist reads both off the parent and hashes the bar's focus into
//! its cache - the one term `observed` cannot see. The bar redraws on
//! `ContentDirty`, which a moved item or a changed look marks, the way a
//! list or a table hears about its rows.

use bevy_ecs::change_detection::{DetectChanges, Ref};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::{Changed, Has, Or, Query, Res, With};
use bevy_input_focus::InputFocus;
use plurimus_core::Edge;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;
use plurimus_core::ratatui_core::widgets::Widget;
use ratatui_widgets::block::{Block, Padding};
use ratatui_widgets::paragraph::Paragraph;

use super::boxed::{Boxed, Joint};
use super::{TabBar, TabBarActiveStyle, TabBarLook, TabBarOrientation, TabItem};
use crate::rows::ContentDirty;
use plurimus_core::UiWidget;
use plurimus_ui::{ComputedWidgetArea, InteractionDisabled, UiLabel, UiStyle, UiTheme};
use plurimus_ui::{InteractionState, StateQuery, Stylable, StylistCache, hashed_bits, observed};

pub(crate) type TabItemsChanged = Changed<ComputedWidgetArea>;

pub(crate) type TabBarSelfChanged = Or<(Changed<Children>, Changed<TabBarLook>)>;

type Bars<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TabBarLook,
        Has<InteractionDisabled>,
        Option<&'static UiStyle>,
        &'static ComputedWidgetArea,
        &'static Children,
        Ref<'static, ContentDirty<TabBar>>,
        &'static mut StylistCache,
        &'static mut UiWidget,
    ),
    Stylable<TabBar>,
>;

type ItemRects<'w, 's> = Query<'w, 's, &'static ComputedWidgetArea, With<TabItem>>;

pub(crate) fn style_tab_bars(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut bars: Bars,
    items: ItemRects,
) {
    for (bar, look, disabled, over, area, children, content, mut cache, mut widget) in &mut bars {
        let state = InteractionState::default()
            .with_disabled(disabled)
            .with_focused(focus.get() == Some(bar));
        let next = StylistCache::new(state, over);
        if !cache.redraws(next, theme.is_changed() || content.is_changed()) {
            continue;
        }
        let gaps = look.divider.as_ref().map_or_else(Vec::new, |_| {
            let placed: Vec<Rect> = children
                .iter()
                .filter_map(|&child| items.get(child).ok())
                .map(|rect| rect.0)
                .collect();
            gaps(look, &placed)
        });
        *widget = UiWidget::new(Chrome {
            style: next.resting_style(&theme),
            divider: look.divider.clone(),
            gaps,
            baseline: look.joint().map(|joint| baseline(look, area.0, joint)),
        });
    }
}

fn gaps(look: &TabBarLook, placed: &[Rect]) -> Vec<Rect> {
    placed
        .windows(2)
        .filter(|pair| !pair[0].is_empty() && !pair[1].is_empty())
        .map(|pair| {
            let (from, to) = (pair[0], pair[1]);
            match look.orientation {
                TabBarOrientation::Horizontal => Rect::new(
                    from.right(),
                    from.y.saturating_add(look.frame()),
                    to.x.saturating_sub(from.right()),
                    1,
                ),
                TabBarOrientation::Vertical => Rect::new(
                    from.x,
                    from.bottom(),
                    from.width,
                    to.y.saturating_sub(from.bottom()),
                ),
            }
        })
        .collect()
}

// The baseline runs the bar's whole length along the items' joined edge.
const fn baseline(look: &TabBarLook, bar: Rect, joint: Joint) -> (Rect, &'static str) {
    let last = look.thickness().saturating_sub(1);
    match joint.edge {
        Edge::Top => (
            Rect::new(bar.x, bar.y, bar.width, 1),
            joint.lines.horizontal,
        ),
        Edge::Bottom => (
            Rect::new(bar.x, bar.y.saturating_add(last), bar.width, 1),
            joint.lines.horizontal,
        ),
        Edge::Left => (Rect::new(bar.x, bar.y, 1, bar.height), joint.lines.vertical),
        Edge::Right => (
            Rect::new(bar.right().saturating_sub(1), bar.y, 1, bar.height),
            joint.lines.vertical,
        ),
    }
}

struct Chrome {
    style: Style,
    divider: Option<Line<'static>>,
    gaps: Vec<Rect>,
    baseline: Option<(Rect, &'static str)>,
}

impl Widget for &Chrome {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(Block::new().style(self.style), area, buffer);
        if let Some((strip, glyph)) = self.baseline {
            for position in strip.intersection(area).positions() {
                if let Some(cell) = buffer.cell_mut(position) {
                    cell.set_symbol(glyph);
                }
            }
        }
        let Some(divider) = &self.divider else {
            return;
        };
        for &gap in &self.gaps {
            Widget::render(divider, gap.intersection(area), buffer);
        }
    }
}

type Items<'w, 's> = Query<
    'w,
    's,
    (
        StateQuery<'static>,
        Ref<'static, UiLabel>,
        &'static ChildOf,
        &'static mut StylistCache,
        &'static mut UiWidget,
    ),
    Stylable<TabItem>,
>;

type ParentBars<'w, 's> = Query<
    'w,
    's,
    (
        Ref<'static, TabBarLook>,
        Ref<'static, TabBarActiveStyle>,
        Has<InteractionDisabled>,
    ),
    With<TabBar>,
>;

pub(crate) fn style_tab_items(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut items: Items,
    bars: ParentBars,
) {
    for (
        (entity, hovered, pressed, disabled, checked, over),
        label,
        parent,
        mut cache,
        mut widget,
    ) in &mut items
    {
        let Ok((look, active, bar_disabled)) = bars.get(parent.parent()) else {
            continue;
        };
        let bar_focused = focus.get() == Some(parent.parent());
        let state = (
            entity,
            hovered,
            pressed,
            disabled || bar_disabled,
            checked,
            over,
        );
        let next = observed(state, &focus, hashed_bits(bar_focused));
        let dirty =
            theme.is_changed() || label.is_changed() || look.is_changed() || active.is_changed();
        if !cache.redraws(next, dirty) {
            continue;
        }
        let style = item_style(next, bar_focused, active.0, over, &theme);
        *widget = item_widget(&look, &label.0, style, next.checked());
    }
}

// The active item is focused while its bar is, and carries the bar's
// active style beneath its own override; every other item is what the
// theme says it is.
fn item_style(
    next: StylistCache,
    bar_focused: bool,
    active: Style,
    over: Option<&UiStyle>,
    theme: &UiTheme,
) -> Style {
    if !next.checked() {
        return next.style(theme);
    }
    let driven = next.with_focused(next.state().focused || bar_focused);
    theme
        .resolve(driven.state())
        .patch(active)
        .patch(over.map_or_else(Style::default, |over| over.0))
}

fn item_widget(look: &TabBarLook, label: &Line<'static>, style: Style, active: bool) -> UiWidget {
    match look.border {
        Some(border) => UiWidget::new(Boxed {
            label: label.clone().style(style),
            border,
            padding: look.padding,
            style,
            joint: look.joint(),
            active,
        }),
        None => UiWidget::new(
            Paragraph::new(label.clone())
                .style(style)
                .block(Block::new().padding(Padding::horizontal(look.padding))),
        ),
    }
}
