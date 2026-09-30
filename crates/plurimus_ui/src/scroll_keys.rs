//! Keyboard scrolling for whichever scrolled widget holds focus.
//!
//! The wheel finds its target by hit-testing the cursor; a key has no
//! position, so it goes to the focused entity instead. Both end in the
//! same [`ScrollBy`], which is what lets a `bevy_ui` node or an app's own
//! scroll consumer page without knowing a key was pressed.

use bevy_ecs::prelude::{Commands, Component, On, Query, Without};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_term::bevy_compat::HeldModifiers;

use crate::interaction::{ComputedDisabled, ComputedWidgetArea};
use crate::keys::{KeyBinding, first_bound};
use crate::scroll::{ScrollArea, ScrollBy, ScrollOffset, WheelAxes};

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
/// Requires [`TabIndex`], which Bevy does not remove when this component
/// goes: removing it leaves an unscrollable widget in the tab order.
/// [`InteractionDisabled`](crate::InteractionDisabled) is how one is
/// turned off.
///
/// Deliberately not required by [`ScrollArea`](crate::ScrollArea): a
/// widget owning its own movement keys - a list box, a table, a text
/// editor - would otherwise answer one press twice. Add it to a scrolled
/// widget that has no keys of its own.
///
/// A page is measured from the widget's resolved area, less any bars a
/// [`ScrollArea`] draws in it, so an area whose content extent was never
/// set pages by its own height against nothing; see
/// [`ScrollArea::content_size`](crate::ScrollArea::content_size).
///
/// A key that would not move a [`ScrollArea`]'s content - an end already
/// reached, an axis the content fits - is not consumed, so an arrow moves
/// focus on and any other key reaches the widget's ancestors. Any other
/// scroll consumer clamps out of this crate's sight, so there a key passes
/// on only for an axis its [`WheelAxes`] rules out.
#[derive(Component, Debug, Clone)]
#[require(TabIndex, ComputedWidgetArea)]
pub struct ScrollKeys(pub Vec<(KeyBinding, ScrollAction)>);

impl Default for ScrollKeys {
    fn default() -> Self {
        Self(vec![
            (Key::PageUp.into(), ScrollAction::PageUp),
            (Key::PageDown.into(), ScrollAction::PageDown),
            (Key::Home.into(), ScrollAction::Top),
            (Key::End.into(), ScrollAction::Bottom),
            (Key::ArrowUp.into(), ScrollAction::LineUp),
            (Key::ArrowDown.into(), ScrollAction::LineDown),
            (Key::ArrowLeft.into(), ScrollAction::LineLeft),
            (Key::ArrowRight.into(), ScrollAction::LineRight),
        ])
    }
}

pub(crate) fn scroll_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    held: HeldModifiers,
    areas: Query<
        (
            &ScrollKeys,
            &ComputedWidgetArea,
            Option<(&ScrollArea, &ScrollOffset)>,
            Option<&WheelAxes>,
        ),
        Without<ComputedDisabled>,
    >,
    mut commands: Commands,
) {
    let entity = input.focused_entity;
    let Ok((keys, area, scroll, axes)) = areas.get(entity) else {
        return;
    };
    let Some(action) = first_bound(&keys.0, &input.input, held.get()) else {
        return;
    };
    let viewport = scroll.map_or(area.0, |(scroll, _)| scroll.viewport(area.0));
    let step = step(action, viewport.height);
    let moves = match (scroll, axes) {
        (Some((scroll, offset)), _) => scroll.stepped(area.0, offset.0, step) != offset.0,
        (None, Some(axes)) => axes.consumes(step),
        (None, None) => true,
    };
    if !moves {
        return;
    }
    input.propagate(false);
    commands.trigger(ScrollBy { entity, step });
}

fn step(action: ScrollAction, height: u16) -> (i32, i32) {
    // A hidden widget keeps focus but resolves to no area, and a page of
    // nothing is no movement.
    let page = i32::from(height);
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
    fn an_arealess_widget_pages_nowhere() {
        assert_eq!(step(ScrollAction::PageDown, 0), (0, 0));
    }

    #[test]
    fn a_jump_leaves_the_horizontal_offset_alone() {
        assert_eq!(step(ScrollAction::Top, 12), (0, i32::MIN));
        assert_eq!(step(ScrollAction::Bottom, 12), (0, i32::MAX));
    }
}
