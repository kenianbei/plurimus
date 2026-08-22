//! Menu button, popup, and items: stateless controllers over the popover
//! placement machinery.
//!
//! A menu is assembled rather than built in: the button opens a popup, the
//! popup is placed by `popover`, and the rows are ordinary entities. Menus
//! own no dismissal logic of their own - they mark the popup modal and let
//! `plurimus_ui`'s routers confine the pointer to it and swallow what lands
//! outside - so what remains here is opening, closing, and moving the
//! highlight.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::Res;
use bevy_ecs::prelude::{Commands, Component, On, Query, ResMut, With, Without};
use bevy_ecs::system::SystemParam;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusCause, FocusedInput, InputFocus};
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::{Rect, Size};
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;
use plurimus_core::ratatui_core::widgets::Widget;
use ratatui_widgets::block::Block;
use ratatui_widgets::clear::Clear;
use ratatui_widgets::paragraph::Paragraph;

use crate::popover::Popover;
use crate::{Activate, Button};
use plurimus_core::{UiHidden, UiOrder, UiWidget};
use plurimus_term::bevy_compat::HeldModifiers;
use plurimus_ui::UiLabel;
use plurimus_ui::{
    Click, Hovered, InteractionDisabled, ModalDismiss, ModalOpen, ModalityToggle, UiStyle, UiTheme,
};
use plurimus_ui::{InteractionState, LabeledQuery, Stylable, StylistCache, decorate, restyle};
use plurimus_ui::{KeyBinding, first_bound};

/// Items render above the popup frame.
const ITEM_ORDER: UiOrder = UiOrder(UiOrder::OVERLAY.0 + 1);

/// A button that toggles its child [`MenuPopup`] on [`Activate`].
#[derive(Component, Debug, Clone, Copy)]
pub struct MenuButton;

/// The popup container; a child of its [`MenuButton`], auto-sized around
/// its [`MenuItem`] children.
#[derive(Component, Debug, Clone, Copy)]
#[require(StylistCache, MenuKeys)]
pub struct MenuPopup;

/// What a [`MenuKeys`] binding does to the open menu.
///
/// Closed: an open menu moves its highlight, commits, or gives up, and a
/// fourth thing is not something a menu does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Move focus to the item above, wrapping.
    Previous,
    /// Move focus to the item below, wrapping.
    Next,
    /// Activate the focused item and close.
    Activate,
    /// Close without activating.
    Close,
}

/// An open [`MenuPopup`]'s key bindings, scanned in order so the first
/// match wins.
///
/// One table per menu rather than one per row, and read from the popup the
/// focused item sits in. Defaults to the up and down arrows, `Escape` to
/// close, and `Enter` and space to activate - the same pair
/// [`ActivateKeys`](crate::ActivateKeys) defaults to, and now for the same
/// reason rather than as a second literal beside it.
#[derive(Component, Debug, Clone)]
pub struct MenuKeys(pub Vec<(KeyBinding, MenuAction)>);

impl Default for MenuKeys {
    fn default() -> Self {
        Self(vec![
            (Key::ArrowUp.into(), MenuAction::Previous),
            (Key::ArrowDown.into(), MenuAction::Next),
            (Key::Escape.into(), MenuAction::Close),
            (Key::Enter.into(), MenuAction::Activate),
            (Key::Character(" ".into()).into(), MenuAction::Activate),
        ])
    }
}

/// Present on a [`MenuPopup`] while it is open.
#[derive(Component, Debug, Clone, Copy)]
pub struct MenuOpen;

/// One row inside a [`MenuPopup`]. Emits [`Activate`] and closes the
/// menu when activated.
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache, ModalityToggle)]
pub struct MenuItem;

/// Spawn bundle for a menu button.
pub fn menu_button(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        MenuButton,
        Button,
        ModalityToggle,
        UiLabel(label.into()),
        TabIndex(0),
        UiWidget::default(),
    )
}

/// Spawn bundle for a menu popup; parent it to its `anchor` button.
#[must_use]
pub fn menu_popup(anchor: Entity) -> impl Bundle {
    (
        MenuPopup,
        Popover::new(anchor, Size::ZERO),
        UiHidden,
        UiWidget::default(),
    )
}

/// Spawn bundle for a menu item; parent it to a [`MenuPopup`].
pub fn menu_item(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        MenuItem,
        UiLabel(label.into()),
        ITEM_ORDER,
        UiHidden,
        UiWidget::default(),
    )
}

/// Queries every menu open/close transition needs.
#[derive(SystemParam)]
pub(crate) struct MenuAccess<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    parents: Query<'w, 's, &'static ChildOf>,
    popups: Query<'w, 's, Entity, With<MenuPopup>>,
    open: Query<'w, 's, Entity, With<MenuOpen>>,
    items: Query<'w, 's, Entity, With<MenuItem>>,
    keys: Query<'w, 's, &'static MenuKeys>,
    live_items: Query<'w, 's, (), (With<MenuItem>, Without<InteractionDisabled>)>,
}

impl MenuAccess<'_, '_> {
    fn popup_of(&self, button: Entity) -> Option<Entity> {
        let children = self.children.get(button).ok()?;
        children.iter().copied().find(|&c| self.popups.contains(c))
    }

    /// The bindings an item takes its keys from: the menu's rather than its
    /// own, since they live on the popup rather than on every row of it.
    ///
    /// `None` for anything that is not a live menu item, which is what keeps
    /// a disabled row from acting on a key.
    fn item_keys(&self, item: Entity) -> Option<&MenuKeys> {
        self.live_items.get(item).ok()?;
        let popup = self.parents.get(item).ok()?.parent();
        self.keys.get(popup).ok()
    }

    pub(crate) fn item_rows(&self, popup: Entity) -> Vec<Entity> {
        let Ok(children) = self.children.get(popup) else {
            return Vec::new();
        };
        children
            .iter()
            .copied()
            .filter(|&child| self.items.contains(child))
            .collect()
    }

    fn open_menu(&self, popup: Entity, focus: &mut InputFocus, commands: &mut Commands) {
        commands
            .entity(popup)
            .insert((MenuOpen, ModalOpen))
            .remove::<UiHidden>();
        let rows = self.item_rows(popup);
        for &item in &rows {
            commands.entity(item).remove::<UiHidden>();
        }
        if let Some(&first) = rows.first() {
            focus.set(first, FocusCause::Navigated);
        }
    }

    fn close_menu(&self, popup: Entity, focus: &mut InputFocus, commands: &mut Commands) {
        commands
            .entity(popup)
            .remove::<(MenuOpen, ModalOpen)>()
            .insert(UiHidden);
        for item in self.item_rows(popup) {
            commands.entity(item).insert(UiHidden);
        }
        match self.parents.get(popup) {
            Ok(parent) => focus.set(parent.parent(), FocusCause::Navigated),
            Err(_) => focus.clear(),
        }
    }

    fn close_item_menu(&self, item: Entity, focus: &mut InputFocus, commands: &mut Commands) {
        if let Ok(popup) = self.parents.get(item).map(ChildOf::parent) {
            self.close_menu(popup, focus, commands);
        }
    }
}

pub(crate) fn menu_dismiss(
    dismiss: On<ModalDismiss>,
    popups: Query<(), With<MenuPopup>>,
    menus: MenuAccess,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    if popups.contains(dismiss.entity) {
        menus.close_menu(dismiss.entity, &mut focus, &mut commands);
    }
}

pub(crate) fn menu_button_activate(
    activate: On<Activate>,
    buttons: Query<(), (With<MenuButton>, Without<InteractionDisabled>)>,
    menus: MenuAccess,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    let Some(popup) = menus.popup_of(activate.entity) else {
        return;
    };
    if menus.open.contains(popup) {
        menus.close_menu(popup, &mut focus, &mut commands);
    } else {
        menus.open_menu(popup, &mut focus, &mut commands);
    }
}

pub(crate) fn menu_item_click(
    click: On<Click>,
    items: Query<(), (With<MenuItem>, Without<InteractionDisabled>)>,
    menus: MenuAccess,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    if !items.contains(click.entity) {
        return;
    }
    commands.trigger(Activate {
        entity: click.entity,
    });
    menus.close_item_menu(click.entity, &mut focus, &mut commands);
}

pub(crate) fn menu_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    held: HeldModifiers,
    menus: MenuAccess,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let item = input.focused_entity;
    let Some(bindings) = menus.item_keys(item) else {
        return;
    };
    let Some(action) = first_bound(&bindings.0, &input.input, held.get()) else {
        return;
    };
    match action {
        MenuAction::Previous => step_focus(item, -1, &menus, &mut focus),
        MenuAction::Next => step_focus(item, 1, &menus, &mut focus),
        MenuAction::Close => menus.close_item_menu(item, &mut focus, &mut commands),
        // A repeat commits once, the way every other activation does.
        MenuAction::Activate if !input.input.repeat => {
            commands.trigger(Activate { entity: item });
            menus.close_item_menu(item, &mut focus, &mut commands);
        }
        MenuAction::Activate => {}
    }
    input.propagate(false);
}

fn step_focus(item: Entity, step: isize, menus: &MenuAccess, focus: &mut InputFocus) {
    let Ok(popup) = menus.parents.get(item).map(ChildOf::parent) else {
        return;
    };
    let rows = menus.item_rows(popup);
    let Some(index) = rows.iter().position(|&row| row == item) else {
        return;
    };
    let next = (index as isize + step).rem_euclid(rows.len() as isize) as usize;
    focus.set(rows[next], FocusCause::Navigated);
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
