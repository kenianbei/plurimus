//! Menus on `bevy_ui_widgets`' own [`MenuButton`], [`MenuPopup`] and
//! [`MenuItem`], over the popover placement machinery.
//!
//! The model is upstream's: the button asks for its menu through a
//! bubbling [`MenuEvent`], the open popup takes focus onto its first or
//! last item, its keys step and activate, and it closes when focus leaves
//! it - so a press outside closes the menu and still acts on what it hit.
//! Upstream leaves answering the event to the app, the way a window app's
//! theme does; [`drive_menu_popups`] is that answer here, showing and
//! hiding the popup through `Propagate<Hidden>`. What else is the crate's is
//! drawing and placement: the popup is a `Popover` child of its button,
//! sized around its items, and each item is given its row.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::Res;
use bevy_ecs::prelude::{Commands, Has, On, Query, ResMut, With};
use bevy_input_focus::tab_navigation::{NavAction, TabIndex};
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::{Rect, Size};
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;
use plurimus_core::ratatui_core::widgets::Widget;
use ratatui_widgets::block::Block;
use ratatui_widgets::clear::Clear;
use ratatui_widgets::paragraph::Paragraph;

use crate::popover::Popover;
use plurimus_core::{Hidden, Propagate, UiOrder, UiWidget};
use plurimus_ui::bevy_ui_widgets::{
    MenuAction, MenuButton, MenuEvent, MenuFocusState, MenuItem, MenuPopup,
};
use plurimus_ui::{Hovered, PressFocusDisabled, UiLabel, UiStyle, UiTheme, WheelReceptive};
use plurimus_ui::{InteractionState, LabeledQuery, Stylable, StylistCache, decorate, restyle};

/// Items render above the popup frame.
const ITEM_ORDER: UiOrder = UiOrder(UiOrder::OVERLAY.0 + 1);

/// Spawn bundle for a menu button; parent its [`menu_popup`] to it.
pub fn menu_button(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        MenuButton,
        Hovered::default(),
        StylistCache::default(),
        UiLabel(label.into()),
        TabIndex(0),
        UiWidget::default(),
    )
}

/// Spawn bundle for a menu popup; parent it to its `anchor` button.
///
/// Hidden until its button opens it. A press on its frame keeps focus on
/// the item that holds it rather than walking up to the button, whose focus
/// would close the menu, and a wheel tick over it scrolls nothing beneath.
#[must_use]
pub fn menu_popup(anchor: Entity) -> impl Bundle {
    (
        MenuPopup::default(),
        Popover::new(anchor, Size::ZERO),
        Propagate(Hidden),
        PressFocusDisabled,
        WheelReceptive,
        StylistCache::default(),
        UiWidget::default(),
    )
}

/// Spawn bundle for a menu item; parent it to a [`MenuPopup`].
///
/// Focusable, since an open popup takes focus onto its items and stays
/// open while one holds it.
pub fn menu_item(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        MenuItem,
        UiLabel(label.into()),
        TabIndex(0),
        Hovered::default(),
        StylistCache::default(),
        ITEM_ORDER,
        UiWidget::default(),
    )
}

/// Answers a [`MenuEvent`] reaching a [`MenuButton`] whose popup is this
/// crate's: `Open` and `Toggle` show it and hand upstream the item to focus,
/// `CloseAll` hides it, `FocusRoot` focuses the button.
///
/// Keyed on the popup's [`Popover`], so a menu built of nodes, whose app
/// answers its own events, is left alone.
pub(crate) fn drive_menu_popups(
    mut event: On<MenuEvent>,
    buttons: Query<&Children, With<MenuButton>>,
    popups: Query<Has<Propagate<Hidden>>, (With<MenuPopup>, With<Popover>)>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let button = event.source;
    let Some((popup, is_hidden)) = buttons.get(button).ok().and_then(|children| {
        children
            .iter()
            .find_map(|&child| Some((child, popups.get(child).ok()?)))
    }) else {
        return;
    };
    event.propagate(false);
    match event.action {
        MenuAction::Open(nav) => show(popup, nav, &mut commands),
        MenuAction::Toggle if is_hidden => show(popup, NavAction::First, &mut commands),
        MenuAction::Toggle | MenuAction::CloseAll => {
            commands
                .entity(popup)
                .insert((Propagate(Hidden), MenuFocusState::Closed));
        }
        MenuAction::FocusRoot => focus.set(button, FocusCause::Navigated),
    }
}

fn show(popup: Entity, nav: NavAction, commands: &mut Commands) {
    commands
        .entity(popup)
        .remove::<Propagate<Hidden>>()
        .insert(MenuFocusState::Opening(nav));
}

pub(crate) fn style_menu_items(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut items: LabeledQuery<MenuItem>,
) {
    restyle(
        &theme,
        theme.is_changed(),
        &focus,
        &mut items,
        |_, label, style| UiWidget::new(Paragraph::new(decorate(" ", label, " ")).style(style)),
    );
}

pub(crate) fn style_menu_popups(
    theme: Res<UiTheme>,
    mut popups: Query<(Option<&UiStyle>, &mut StylistCache, &mut UiWidget), Stylable<MenuPopup>>,
) {
    for (over, mut cache, mut widget) in &mut popups {
        // A popup frame has no hover or press to resolve, so its state is
        // the override alone.
        let next = StylistCache::new(InteractionState::default(), over);
        if !cache.redraws(next, theme.is_changed()) {
            continue;
        }
        *widget = UiWidget::new(PopupFrame {
            style: next.style(&theme),
        });
    }
}

/// Opaque popup body with a line border, so lower widgets never show
/// through the menu.
struct PopupFrame {
    style: Style,
}

impl Widget for &PopupFrame {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(Clear, area, buffer);
        Widget::render(Block::bordered().style(self.style), area, buffer);
    }
}
