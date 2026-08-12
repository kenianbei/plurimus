//! Keyboard scrolling for whichever scrolled widget holds focus.
//!
//! The wheel finds its target by hit-testing the cursor; a key has no
//! position, so it goes to the focused entity instead. Both end in the
//! same [`ScrollBy`], which is what lets a `bevy_ui` node or an app's own
//! scroll consumer page without knowing a key was pressed.

use bevy_ecs::prelude::{Commands, Component, On, Query, Without};
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use bevy_input_focus::tab_navigation::TabIndex;

use crate::interaction::{ComputedWidgetArea, InteractionDisabled};
use crate::scroll::ScrollBy;

/// What a bound key does to a scrolled widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollAction {
    /// Up one cell.
    LineUp,
    /// Down one cell.
    LineDown,
    /// Left one cell.
    LineLeft,
    /// Right one cell.
    LineRight,
    /// Up one viewport height.
    PageUp,
    /// Down one viewport height.
    PageDown,
    /// To the top of the content.
    Top,
    /// To the bottom of the content.
    Bottom,
}

/// Scroll bindings for a widget, scanned in order so the first match
/// wins. Adding it makes the entity a tab stop, since a widget that
/// cannot hold focus cannot be sent a key.
///
/// Deliberately not required by [`ScrollArea`](crate::ScrollArea): a
/// widget owning its own movement keys - a list box, a table, a text
/// editor - would otherwise answer one press twice. Add it to a scrolled
/// widget that has no keys of its own.
///
/// A page is measured from the widget's resolved area, so an area whose
/// content extent was never set pages by its own height against nothing;
/// see [`ScrollArea::content_size`](crate::ScrollArea::content_size).
///
/// Horizontal bindings exist but are unbound by default: an area that
/// does not overflow horizontally would otherwise swallow the left and
/// right arrows that move focus between widgets.
#[derive(Component, Debug, Clone)]
#[require(TabIndex, ComputedWidgetArea)]
pub struct ScrollKeys(pub Vec<(Key, ScrollAction)>);

impl Default for ScrollKeys {
    fn default() -> Self {
        Self(vec![
            (Key::PageUp, ScrollAction::PageUp),
            (Key::PageDown, ScrollAction::PageDown),
            (Key::Home, ScrollAction::Top),
            (Key::End, ScrollAction::Bottom),
            (Key::ArrowUp, ScrollAction::LineUp),
            (Key::ArrowDown, ScrollAction::LineDown),
        ])
    }
}

pub(crate) fn scroll_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    areas: Query<(&ScrollKeys, &ComputedWidgetArea), Without<InteractionDisabled>>,
    mut commands: Commands,
) {
    let entity = input.focused_entity;
    let Ok((keys, area)) = areas.get(entity) else {
        return;
    };
    let Some(action) = bound_action(keys, &input.input) else {
        return;
    };
    // Consumed even at an extreme, so a bound key never reaches
    // directional navigation and moves focus out of the pane instead.
    input.propagate(false);
    commands.trigger(ScrollBy {
        entity,
        step: step(action, area.0.height),
    });
}

fn bound_action(keys: &ScrollKeys, input: &KeyboardInput) -> Option<ScrollAction> {
    if input.state != ButtonState::Pressed {
        return None;
    }
    keys.0
        .iter()
        .find(|(key, _)| *key == input.logical_key)
        .map(|(_, action)| *action)
}

fn step(action: ScrollAction, height: u16) -> (i32, i32) {
    // A widget kept off screen holds focus with no area; a page there is
    // still movement rather than a dead key.
    let page = i32::from(height.max(1));
    match action {
        ScrollAction::LineUp => (0, -1),
        ScrollAction::LineDown => (0, 1),
        ScrollAction::LineLeft => (-1, 0),
        ScrollAction::LineRight => (1, 0),
        ScrollAction::PageUp => (0, -page),
        ScrollAction::PageDown => (0, page),
        ScrollAction::Top => (0, i32::MIN),
        ScrollAction::Bottom => (0, i32::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::{ScrollAction, step};

    #[test]
    fn a_page_is_the_viewport_height_in_either_direction() {
        assert_eq!(step(ScrollAction::PageDown, 12), (0, 12));
        assert_eq!(step(ScrollAction::PageUp, 12), (0, -12));
    }

    #[test]
    fn an_arealess_widget_still_pages_by_a_row() {
        assert_eq!(step(ScrollAction::PageDown, 0), (0, 1));
    }

    #[test]
    fn a_jump_leaves_the_horizontal_offset_alone() {
        assert_eq!(step(ScrollAction::Top, 12), (0, i32::MIN));
        assert_eq!(step(ScrollAction::Bottom, 12), (0, i32::MAX));
    }
}
