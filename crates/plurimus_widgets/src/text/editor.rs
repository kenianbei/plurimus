//! The multi-line text editor widget, backed by ratatui-textarea.
//!
//! Unlike the single-line field, the editor does not own its text: a
//! [`TextArea`] behind an [`Arc`]/[`Mutex`] does, and this file translates key
//! and wheel input into that engine's own [`Input`] vocabulary. That is why
//! the widget is the crate's one [`LiveWidget`](plurimus_ui::LiveWidget) - its
//! rendered output can change without the [`UiWidget`] component being
//! replaced - and why edits are applied first and announced afterwards with
//! [`TextChanged`].

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Added, Commands, Component, EntityEvent, On, Query, Without};
use bevy_input::ButtonState;
use bevy_input::keyboard::KeyboardInput;
use bevy_input_focus::FocusedInput;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::widgets::Widget;
use plurimus_term::PasteMessage;
use ratatui_textarea::{CursorMove, DataCursor, TextArea};

use super::editor_keys::{TextEditorAction, TextEditorKeys};
use super::grapheme::{cluster_len_after, cluster_len_before};
use super::keys::unbound_text;
use crate::clipboard::Clipboard;
use plurimus_core::UiWidget;
use plurimus_term::bevy_compat::HeldModifiers;
use plurimus_ui::{ComputedDisabled, Hovered, first_bound};
use plurimus_ui::{ScrollBy, WheelReceptive};

use plurimus_ui::LiveWidget;

/// A multi-line text editor. State lives behind a lock (`TextArea` is
/// `!Sync`); the widget is a live view onto it, so edits render without a
/// widget rebuild. Edits emit [`TextChanged`]; read content via
/// [`TextEditor::lock`].
///
/// Keys come from [`TextEditorKeys`], not the engine's own emacs keymap:
/// a bound key applies its [`TextEditorAction`] and is consumed whether or
/// not it changed anything, an unbound unchorded character types itself,
/// and an unbound chord propagates. Tab is unbound by default, left to
/// focus navigation.
///
/// Copy and cut act as the engine would and also ask the terminal for the
/// text through [`TerminalRequest`](plurimus_term::TerminalRequest), so a
/// copy leaves the app; neither sends anything when there is no selection.
/// Whether it reaches a system clipboard is the backend's business -
/// `plurimus_crossterm` writes none until asked - but it always reaches
/// [`LastCopied`](plurimus_term::LastCopied), and paste inserts from there,
/// so a copy in one editor is a paste in another.
///
/// Paste therefore means the app's clipboard, and
/// [`Yank`](TextEditorAction::Yank) the engine's own kill ring, which the
/// line and word deletions fill. They are deliberately separate: a copy
/// anywhere in the app would otherwise displace a kill the user has not yet
/// put back.
///
/// Movement and deletion step whole grapheme clusters, and a deleted cluster
/// is one edit to undo.
#[derive(Component, Clone)]
#[require(Hovered, WheelReceptive, LiveWidget, TextEditorKeys)]
pub struct TextEditor(Arc<Mutex<TextArea<'static>>>);

impl TextEditor {
    /// An editor pre-filled with `text`.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let lines: Vec<String> = text.into().lines().map(str::to_owned).collect();
        Self(Arc::new(Mutex::new(TextArea::new(lines))))
    }

    /// Locks the editor state for reading or programmatic editing.
    pub fn lock(&self) -> MutexGuard<'_, TextArea<'static>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The editor's content changed; read it via [`TextEditor::lock`].
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct TextChanged {
    /// The editor entity.
    pub entity: Entity,
}

/// Spawn bundle for a multi-line text editor.
pub fn text_editor(text: impl Into<String>) -> impl Bundle {
    (TextEditor::new(text), TabIndex(0))
}

pub(crate) fn install_editor_views(
    editors: Query<(Entity, &TextEditor), Added<TextEditor>>,
    mut commands: Commands,
) {
    for (entity, editor) in &editors {
        commands
            .entity(entity)
            .insert(UiWidget::new(SharedTextArea(Arc::clone(&editor.0))));
    }
}

pub(crate) fn text_editor_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    held: HeldModifiers,
    editors: Query<(&TextEditor, &TextEditorKeys), Without<ComputedDisabled>>,
    mut clipboard: Clipboard,
    mut commands: Commands,
) {
    let entity = input.focused_entity;
    let Ok((editor, keys)) = editors.get(entity) else {
        return;
    };
    if input.input.state != ButtonState::Pressed {
        return;
    }
    let held = held.get();
    let bound = first_bound(&keys.0, &input.input, held);
    let typed = unbound_text(&input.input.logical_key, held);
    if bound.is_none() && typed.is_none() {
        return;
    }
    let mut area = editor.lock();
    let edited = match bound {
        Some(action) => apply(&mut area, action, &mut clipboard),
        None => typed.is_some_and(|text| area.insert_str(text)),
    };
    input.propagate(false);
    if edited {
        commands.trigger(TextChanged { entity });
    }
}

/// Applies `action` and reports whether the text changed.
fn apply(
    area: &mut TextArea<'static>,
    action: TextEditorAction,
    clipboard: &mut Clipboard,
) -> bool {
    match action {
        TextEditorAction::Move(motion) => {
            area.cancel_selection();
            step(area, motion);
            false
        }
        TextEditorAction::Select(motion) => {
            select(area, motion);
            false
        }
        TextEditorAction::Scroll(scrolling) => {
            area.cancel_selection();
            area.scroll(scrolling);
            false
        }
        TextEditorAction::Newline => {
            area.insert_newline();
            true
        }
        TextEditorAction::InsertTab => area.insert_tab(),
        TextEditorAction::Backspace => {
            delete_cluster(area, CursorMove::Back, TextArea::delete_char)
        }
        TextEditorAction::Delete => {
            delete_cluster(area, CursorMove::Forward, TextArea::delete_next_char)
        }
        TextEditorAction::DeleteWord => area.delete_word(),
        TextEditorAction::DeleteNextWord => area.delete_next_word(),
        TextEditorAction::DeleteToLineEnd => area.delete_line_by_end(),
        TextEditorAction::DeleteToLineHead => area.delete_line_by_head(),
        TextEditorAction::Undo => area.undo(),
        TextEditorAction::Redo => area.redo(),
        TextEditorAction::Yank => area.paste(),
        TextEditorAction::Copy => copy(area, clipboard),
        TextEditorAction::Cut => cut(area, clipboard),
        TextEditorAction::Paste => paste(area, clipboard),
        TextEditorAction::SelectAll => {
            area.select_all();
            false
        }
        TextEditorAction::CancelSelection => {
            area.cancel_selection();
            false
        }
    }
}

/// Moves by `motion`, across a whole grapheme cluster where the engine
/// steps one scalar.
fn step(area: &mut TextArea<'static>, motion: CursorMove) {
    for _ in 0..cluster_steps(area, motion) {
        area.move_cursor(motion);
    }
}

/// Extends the selection by `motion`, starting one if there is none -
/// unless the cursor could not move, so no empty selection is left behind.
fn select(area: &mut TextArea<'static>, motion: CursorMove) {
    if area.is_selecting() {
        step(area, motion);
        return;
    }
    let before = area.cursor();
    area.start_selection();
    step(area, motion);
    if area.cursor() == before {
        area.cancel_selection();
    }
}

/// Deletes the selection, or selects the cluster `motion` crosses and
/// deletes that, so the engine records the cluster as one edit.
///
/// `delete` has to be the engine's deletion in the same direction: an empty
/// selection, left where `motion` could not move, falls through to it.
fn delete_cluster(
    area: &mut TextArea<'static>,
    motion: CursorMove,
    delete: fn(&mut TextArea<'static>) -> bool,
) -> bool {
    if !area.is_selecting() {
        area.start_selection();
        step(area, motion);
    }
    delete(area)
}

// `copy` cancels the selection and reports nothing, so whether there was
// one has to be asked before the call rather than after.
fn copy(area: &mut TextArea<'static>, clipboard: &mut Clipboard) -> bool {
    if area.is_selecting() {
        area.copy();
        clipboard.offer(&area.yank_text());
    }
    false
}

fn cut(area: &mut TextArea<'static>, clipboard: &mut Clipboard) -> bool {
    let cut = area.cut();
    if cut {
        clipboard.offer(&area.yank_text());
    }
    cut
}

// `insert_str` is `paste` with the text supplied rather than taken from the
// engine's yank, which is what keeps a kill intact.
fn paste(area: &mut TextArea<'static>, clipboard: &Clipboard) -> bool {
    clipboard
        .last_copied()
        .is_some_and(|text| area.insert_str(text))
}

/// How many engine steps of `motion` cross one grapheme cluster.
///
/// Never zero: the engine owns line wrapping and joining at column edges.
fn cluster_steps(area: &TextArea<'static>, motion: CursorMove) -> usize {
    let DataCursor(row, column) = area.cursor();
    let Some(line) = area.lines().get(row) else {
        return 1;
    };
    let steps = match motion {
        CursorMove::Back => cluster_len_before(line, column),
        CursorMove::Forward => cluster_len_after(line, column),
        _ => return 1,
    };
    steps.max(1)
}

pub(crate) fn text_editor_paste(
    mut input: On<FocusedInput<PasteMessage>>,
    editors: Query<&TextEditor, Without<ComputedDisabled>>,
    mut commands: Commands,
) {
    let entity = input.focused_entity;
    let Ok(editor) = editors.get(entity) else {
        return;
    };
    input.propagate(false);
    if input.input.0.is_empty() {
        return;
    }
    editor.lock().insert_str(&input.input.0);
    commands.trigger(TextChanged { entity });
}

pub(crate) fn text_editor_scrolled(event: On<ScrollBy>, editors: Query<&TextEditor>) {
    let Ok(editor) = editors.get(event.entity) else {
        return;
    };
    let (step_x, step_y) = event.step;
    // The engine's own delta is i16; a larger step is a jump it clamps
    // to its extent anyway.
    editor.lock().scroll((narrowed(step_y), narrowed(step_x)));
}

fn narrowed(step: i32) -> i16 {
    step.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

/// Deliberate exception to the immutable-`UiWidget` contract: reads live
/// state through the lock, which is why `TextEditor` requires `LiveWidget`.
/// Race-free - the render sub-app runs serially.
struct SharedTextArea(Arc<Mutex<TextArea<'static>>>);

impl Widget for &SharedTextArea {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let editor = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        Widget::render(&*editor, area, buffer);
    }
}
