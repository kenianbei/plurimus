//! The single-line text input widget.
//!
//! [`EditableText`] is the component an app spawns; the editing state lives
//! in [`TextInput`](super::TextInput) beside it and the row is drawn by
//! `field`. Keys are handled here rather than by an engine, which is what
//! keeps the field's bindings deliberately aligned with the multi-line
//! editor's - the same chord moves by word in both - while leaving the field
//! free of ratatui-textarea entirely.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Component, EntityEvent, On, Query, Res, With, Without};
use bevy_input::ButtonState;
use bevy_input::keyboard::KeyboardInput;
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusLost, FocusedInput, InputFocus};
use plurimus_term::PasteMessage;

use super::field::{TextField, mask_value, place_caret};
use super::keys::{TextInputAction, TextInputKeys};
use super::state::TextInput;
use crate::ValueChange;
use plurimus_core::UiWidget;
use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::bevy_compat::HeldModifiers;
use plurimus_ui::{
    ComputedDisabled, ComputedWidgetArea, Hovered, UiTheme, WidgetCursor, first_bound,
};
use plurimus_ui::{StateQuery, Stylable, StylistCache, hashed_bits, observed};

/// A single-line editable text field. Edits mutate [`TextInput`] directly
/// and emit [`ValueChange<String>`]: `is_final: false` per edit, `true`
/// on submit and on focus loss.
///
/// Which keys edit and which submits is [`TextInputKeys`], required here and
/// defaulting to what the field always bound.
///
/// The caret is drawn into the row while the field has focus, and its cell
/// is published in the required [`WidgetCursor`], so the terminal's own
/// cursor sits on it too. Only the cell is written: a shape set with
/// [`WidgetCursor::with_style`] is kept.
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache, TextInput, TextInputKeys, WidgetCursor)]
pub struct EditableText;

/// Draws an [`EditableText`] as one of this glyph per grapheme cluster, the
/// way a password field is drawn.
///
/// Only the drawn row is masked: [`TextInput`], [`ValueChange<String>`] and
/// [`Submit`] still carry the plaintext. The glyph is laid out like any other
/// cluster, so a double-width glyph takes two cells.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextMask(pub char);

/// An [`EditableText`] was submitted with Enter, carrying the value at that
/// moment.
///
/// The final [`ValueChange<String>`] fires beside it, and again whenever the
/// field loses focus, so the two are indistinguishable to a consumer that
/// reads only that. Listening here is how committing an entry stays separate
/// from abandoning one.
#[derive(EntityEvent, Debug, Clone)]
#[non_exhaustive]
pub struct Submit {
    /// The submitted field.
    pub entity: Entity,
    /// The field's value at submission.
    pub value: String,
}

impl Submit {
    /// A submission of `entity` carrying `value`.
    #[must_use]
    pub const fn new(entity: Entity, value: String) -> Self {
        Self { entity, value }
    }
}

/// Spawn bundle for a single-line text field.
pub fn editable_text(value: impl Into<String>) -> impl Bundle {
    (
        EditableText,
        TextInput::new(value),
        TabIndex(0),
        UiWidget::default(),
    )
}

pub(crate) fn text_input_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    held: HeldModifiers,
    mut fields: Query<
        (&mut TextInput, &TextInputKeys),
        (With<EditableText>, Without<ComputedDisabled>),
    >,
    mut commands: Commands,
) {
    let field = input.focused_entity;
    let Ok((mut text, keys)) = fields.get_mut(field) else {
        return;
    };
    if input.input.state != ButtonState::Pressed {
        return;
    }
    let held = held.get();
    if first_bound(&keys.0, &input.input, held) == Some(TextInputAction::Submit) {
        // One intent commits once, however long the key is held; it is
        // consumed either way, being the field's.
        if !input.input.repeat {
            emit(field, &text, true, &mut commands);
            commands.trigger(Submit::new(field, text.value().to_owned()));
        }
        input.propagate(false);
        return;
    }
    let length_before = text.value().len();
    if !text.handle(keys, &input.input, held) {
        return;
    }
    input.propagate(false);
    if text.value().len() != length_before {
        emit(field, &text, false, &mut commands);
    }
}

fn emit(field: Entity, text: &TextInput, is_final: bool, commands: &mut Commands) {
    commands.trigger(ValueChange::new(field, text.value().to_owned(), is_final));
}

pub(crate) fn text_input_paste(
    mut input: On<FocusedInput<PasteMessage>>,
    mut fields: Query<&mut TextInput, (With<EditableText>, Without<ComputedDisabled>)>,
    mut commands: Commands,
) {
    let field = input.focused_entity;
    let Ok(mut text) = fields.get_mut(field) else {
        return;
    };
    input.propagate(false);
    if text.paste(&input.input.0) {
        emit(field, &text, false, &mut commands);
    }
}

pub(crate) fn text_input_blur(
    lost: On<FocusLost>,
    fields: Query<&TextInput, With<EditableText>>,
    mut commands: Commands,
) {
    if let Ok(text) = fields.get(lost.entity) {
        emit(lost.entity, text, true, &mut commands);
    }
}

pub(crate) fn style_text_inputs(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut fields: Query<
        (
            StateQuery,
            &TextInput,
            Option<&TextMask>,
            &ComputedWidgetArea,
            &mut StylistCache,
            &mut UiWidget,
            &mut WidgetCursor,
        ),
        Stylable<EditableText>,
    >,
) {
    for (state, text, mask, area, mut cache, mut widget, mut caret) in &mut fields {
        let row = area.0;
        let next = observed(state, &focus, hashed_bits((text, mask, row.as_size())));
        if !cache.redraws(next, theme.is_changed()) {
            continue;
        }
        let (value, cursor) = match mask {
            Some(TextMask(glyph)) => mask_value(text.value(), text.cursor(), *glyph),
            None => (text.value().to_owned(), text.cursor()),
        };
        caret.cell = (!row.is_empty())
            .then(|| Position::new(place_caret(&value, cursor, row.width).column, 0));
        *widget = UiWidget::new(TextField {
            value,
            cursor,
            style: next.style(&theme),
            caret: next.state().focused.then_some(theme.caret),
        });
    }
}
