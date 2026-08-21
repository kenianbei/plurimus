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
use bevy_ecs::prelude::{Commands, Component, On, Query, Res, With, Without};
use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy_input::{ButtonInput, ButtonState};
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusLost, FocusedInput, InputFocus};
use plurimus_term::PasteMessage;

use super::field::TextField;
use super::state::TextInput;
use crate::ValueChange;
use plurimus_core::UiWidget;
use plurimus_term::bevy_compat::held_modifiers;
use plurimus_ui::{Hovered, InteractionDisabled, UiTheme};
use plurimus_ui::{StateQuery, Stylable, StylistCache, hashed_bits, observed};

/// A single-line editable text field. Edits mutate [`TextInput`] directly
/// and emit [`ValueChange<String>`]: `is_final: false` per edit, `true`
/// on Enter and on focus loss.
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache, TextInput)]
pub struct EditableText;

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
    held_keys: Res<ButtonInput<KeyCode>>,
    mut fields: Query<&mut TextInput, (With<EditableText>, Without<InteractionDisabled>)>,
    mut commands: Commands,
) {
    let field = input.focused_entity;
    let Ok(mut text) = fields.get_mut(field) else {
        return;
    };
    if input.input.state != ButtonState::Pressed {
        return;
    }
    let length_before = text.value().len();
    let edited = text.handle(&input.input, held_modifiers(&held_keys));
    let submitted = input.input.logical_key == Key::Enter;
    if !edited && !submitted {
        return;
    }
    input.propagate(false);
    if submitted {
        emit(field, &text, true, &mut commands);
    } else if text.value().len() != length_before {
        emit(field, &text, false, &mut commands);
    }
}

fn emit(field: Entity, text: &TextInput, is_final: bool, commands: &mut Commands) {
    commands.trigger(ValueChange::new(field, text.value().to_owned(), is_final));
}

pub(crate) fn text_input_paste(
    mut input: On<FocusedInput<PasteMessage>>,
    mut fields: Query<&mut TextInput, (With<EditableText>, Without<InteractionDisabled>)>,
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
        (StateQuery, &TextInput, &mut StylistCache, &mut UiWidget),
        Stylable<EditableText>,
    >,
) {
    for (state, text, mut cache, mut widget) in &mut fields {
        let next = observed(state, &focus, hashed_bits(text));
        if !cache.redraws(next, theme.is_changed()) {
            continue;
        }
        *widget = UiWidget::new(TextField {
            value: text.value().to_owned(),
            cursor: text.cursor(),
            style: next.style(&theme),
        });
    }
}
