//! The two tab bar stylists: the bar's chrome, and each item's label.
//!
//! An item never holds focus and never learns the bar's look on its own,
//! so its stylist reads both off the parent and hashes the bar's focus into
//! its cache - the one term `observed` cannot see. The bar's cache hashes
//! where its items landed and which is active, which is what the dividers
//! and the baseline are drawn between.

use std::hash::{DefaultHasher, Hash, Hasher};

use bevy_ecs::change_detection::{DetectChanges, Ref};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::{Has, Query, Res, With};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::{Line, Span};
use plurimus_core::ratatui_core::widgets::Widget;
use ratatui_widgets::block::Block;
use ratatui_widgets::paragraph::Paragraph;

use super::{TabBar, TabBarActiveStyle, TabBarLook, TabBarOrientation, TabItem};
use plurimus_core::UiWidget;
use plurimus_ui::{Checked, ComputedWidgetArea, InteractionDisabled, UiLabel, UiStyle, UiTheme};
use plurimus_ui::{InteractionState, StateQuery, Stylable, StylistCache, hashed_bits, observed};

type Bars<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Ref<'static, TabBarLook>,
        Ref<'static, TabBarActiveStyle>,
        Has<InteractionDisabled>,
        Option<&'static UiStyle>,
        &'static ComputedWidgetArea,
        &'static Children,
        &'static mut StylistCache,
        &'static mut UiWidget,
    ),
    Stylable<TabBar>,
>;

type ItemRects<'w, 's> = Query<'w, 's, (&'static ComputedWidgetArea, Has<Checked>), With<TabItem>>;

pub(crate) fn style_tab_bars(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut bars: Bars,
    items: ItemRects,
) {
    for (bar, look, active, disabled, over, area, children, mut cache, mut widget) in &mut bars {
        let state = InteractionState::default()
            .with_disabled(disabled)
            .with_focused(focus.get() == Some(bar));
        let next = StylistCache::new(state, over).with_value(placed_bits(children, &items));
        let dirty = theme.is_changed() || look.is_changed() || active.is_changed();
        if !cache.redraws(next, dirty) {
            continue;
        }
        let placed: Vec<Rect> = children
            .iter()
            .filter_map(|&child| items.get(child).ok())
            .map(|(rect, _)| rect.0)
            .collect();
        *widget = UiWidget::new(Chrome {
            style: next.resting_style(&theme),
            divider: look.divider.clone(),
            gaps: gaps(&look, area.0, &placed),
        });
    }
}

// Hashed rather than collected, so an idle frame allocates nothing.
fn placed_bits(children: &Children, items: &ItemRects) -> u64 {
    let mut hasher = DefaultHasher::new();
    for (rect, checked) in children.iter().filter_map(|&child| items.get(child).ok()) {
        (rect.0, checked).hash(&mut hasher);
    }
    hasher.finish()
}

// The cell between each placed item and the next, in the bar's own
// coordinates, on the label line of a boxed item.
fn gaps(look: &TabBarLook, bar: Rect, placed: &[Rect]) -> Vec<Rect> {
    placed
        .windows(2)
        .filter(|pair| !pair[0].is_empty() && !pair[1].is_empty())
        .map(|pair| {
            let (from, to) = (pair[0], pair[1]);
            match look.orientation {
                TabBarOrientation::Horizontal => Rect::new(
                    from.right().saturating_sub(bar.x),
                    from.y.saturating_sub(bar.y).saturating_add(look.frame()),
                    to.x.saturating_sub(from.right()),
                    1,
                ),
                TabBarOrientation::Vertical => Rect::new(
                    0,
                    from.bottom().saturating_sub(bar.y),
                    bar.width,
                    to.y.saturating_sub(from.bottom()),
                ),
            }
        })
        .collect()
}

struct Chrome {
    style: Style,
    divider: Option<Line<'static>>,
    gaps: Vec<Rect>,
}

impl Widget for &Chrome {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(Block::new().style(self.style), area, buffer);
        let Some(divider) = &self.divider else {
            return;
        };
        for gap in &self.gaps {
            let cell = Rect::new(
                area.x.saturating_add(gap.x),
                area.y.saturating_add(gap.y),
                gap.width,
                gap.height,
            )
            .intersection(area);
            Widget::render(divider, cell, buffer);
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
        *widget = item_widget(&look, &label.0, style);
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
    let state = next
        .state()
        .with_focused(next.state().focused || bar_focused);
    theme
        .resolve(state)
        .patch(active)
        .patch(over.map_or_else(Style::default, |over| over.0))
}

fn item_widget(look: &TabBarLook, label: &Line<'static>, style: Style) -> UiWidget {
    let pad = " ".repeat(usize::from(look.padding));
    let mut line = label.clone();
    line.spans.insert(0, Span::raw(pad.clone()));
    line.spans.push(Span::raw(pad));
    let paragraph = Paragraph::new(line).style(style);
    match look.border {
        Some(border) => {
            UiWidget::new(paragraph.block(Block::bordered().border_type(border).style(style)))
        }
        None => UiWidget::new(paragraph),
    }
}
