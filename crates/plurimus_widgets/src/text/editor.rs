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
use bevy_ecs::prelude::{
    Added, Commands, Component, EntityEvent, MessageWriter, On, Query, Res, Without,
};
use bevy_ecs::system::SystemParam;
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::widgets::Widget;
use plurimus_term::{KeyModifiers, LastCopied, PasteMessage, TerminalRequest};
use ratatui_textarea::{CursorMove, DataCursor, TextArea};

use super::editor_keys::{TextEditorAction, TextEditorKeys};
use super::grapheme::{cluster_len_after, cluster_len_before};
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
/// text through [`TerminalRequest`], so a copy leaves the app; neither sends
/// anything when there is no selection. Whether it reaches a system
/// clipboard is the backend's business - `plurimus_crossterm` writes none
/// until asked - but it always reaches [`LastCopied`], and paste inserts
/// from there, so a copy in one editor is a paste in another.
///
/// Paste therefore means the app's clipboard, and
/// [`Yank`](TextEditorAction::Yank) the engine's own kill ring, which the
/// line and word deletions fill. They are deliberately separate: a copy
/// anywhere in the app would otherwise displace a kill the user has not yet
/// put back.
///
/// Movement and deletion step whole grapheme clusters, but the engine
/// records one history entry per scalar, so undoing a deleted multi-scalar
/// cluster restores it one scalar at a time.
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

/// The clipboard both ways: what an editor offers the terminal, and what
/// it pastes from.
#[derive(SystemParam)]
pub(crate) struct Clipboard<'w> {
    requests: MessageWriter<'w, TerminalRequest>,
    copied: Res<'w, LastCopied>,
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
    let mut area = editor.lock();
    let edited = if let Some(action) = first_bound(&keys.0, &input.input, held) {
        (0..cluster_steps(&area, action)).fold(false, |edited, _| {
            apply(&mut area, action, &mut clipboard) | edited
        })
    } else if let Some(text) = unbound_text(&input.input.logical_key, held) {
        area.insert_str(text)
    } else {
        return;
    };
    input.propagate(false);
    if edited {
        commands.trigger(TextChanged { entity });
    }
}

/// What an unbound key types, which is nothing unless it is an unchorded
/// character. Shift is not a chord, or capitals would stop.
fn unbound_text(key: &Key, held: KeyModifiers) -> Option<&str> {
    if held.ctrl || held.alt || held.super_key || held.hyper || held.meta {
        return None;
    }
    match key {
        Key::Character(characters) => Some(characters.as_str()),
        Key::Space => Some(" "),
        _ => None,
    }
}

/// Applies one step of `action` and reports whether the text changed.
fn apply(
    area: &mut TextArea<'static>,
    action: TextEditorAction,
    clipboard: &mut Clipboard,
) -> bool {
    match action {
        TextEditorAction::Move(motion) => {
            area.cancel_selection();
            area.move_cursor(motion);
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
        TextEditorAction::Backspace => area.delete_char(),
        TextEditorAction::Delete => area.delete_next_char(),
        TextEditorAction::DeleteWord => area.delete_word(),
        TextEditorAction::DeleteNextWord => area.delete_next_word(),
        TextEditorAction::DeleteToLineEnd => area.delete_line_by_end(),
        TextEditorAction::DeleteToLineHead => area.delete_line_by_head(),
        TextEditorAction::Undo => area.undo(),
        TextEditorAction::Redo => area.redo(),
        TextEditorAction::Yank => area.paste(),
        TextEditorAction::Copy | TextEditorAction::Cut | TextEditorAction::Paste => {
            apply_clipboard(area, action, clipboard)
        }
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

/// Extends the selection by `motion`, starting one if there is none - unless
/// the cursor could not move, which the engine answers by starting none.
fn select(area: &mut TextArea<'static>, motion: CursorMove) {
    if area.is_selecting() {
        area.move_cursor(motion);
        return;
    }
    let before = area.cursor();
    area.start_selection();
    area.move_cursor(motion);
    if area.cursor() == before {
        area.cancel_selection();
    }
}

/// Applies a clipboard action and reports whether the text changed.
fn apply_clipboard(
    area: &mut TextArea<'static>,
    action: TextEditorAction,
    clipboard: &mut Clipboard,
) -> bool {
    match action {
        // `copy` cancels the selection and reports nothing, so whether
        // there was one has to be asked before the call rather than after.
        TextEditorAction::Copy => {
            if area.is_selecting() {
                area.copy();
                offer_yank(area, &mut clipboard.requests);
            }
            false
        }
        TextEditorAction::Cut => {
            let cut = area.cut();
            if cut {
                offer_yank(area, &mut clipboard.requests);
            }
            cut
        }
        // `insert_str` is `paste` with the text supplied rather than taken
        // from the engine's yank, which is what keeps a kill intact.
        TextEditorAction::Paste => clipboard
            .copied
            .0
            .as_deref()
            .is_some_and(|text| area.insert_str(text)),
        _ => false,
    }
}

/// Sends what the engine just yanked to the terminal, unless it is empty -
/// an empty copy would take away whatever the user last put there.
fn offer_yank(area: &TextArea<'static>, requests: &mut MessageWriter<TerminalRequest>) {
    let yanked = area.yank_text();
    if !yanked.is_empty() {
        requests.write(TerminalRequest::copy(yanked));
    }
}

/// How many times one step of `action` should repeat to cross a whole
/// grapheme cluster.
///
/// Never zero: the engine owns line wrapping and joining at column edges.
/// Never above one for a deletion while selecting: the first step eats the
/// whole selection.
fn cluster_steps(area: &TextArea<'static>, action: TextEditorAction) -> usize {
    let DataCursor(row, column) = area.cursor();
    let Some(line) = area.lines().get(row) else {
        return 1;
    };
    let deleting_selection = area.is_selecting();
    let steps = match action {
        TextEditorAction::Move(CursorMove::Back) | TextEditorAction::Select(CursorMove::Back) => {
            cluster_len_before(line, column)
        }
        TextEditorAction::Move(CursorMove::Forward)
        | TextEditorAction::Select(CursorMove::Forward) => cluster_len_after(line, column),
        TextEditorAction::Backspace if !deleting_selection => cluster_len_before(line, column),
        TextEditorAction::Delete if !deleting_selection => cluster_len_after(line, column),
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
