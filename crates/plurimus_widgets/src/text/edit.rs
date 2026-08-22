//! Key and paste dispatch into [`TextInput`].
//!
//! Published rather than kept behind the focused observers, so a host that
//! routes its own keys drives a field it never focuses - a command palette
//! typing while a list takes the arrows - and keeps the cluster-correct
//! stepping [`TextInput`] owns instead of reimplementing it over `char`s.
//! The stock observers in `input` are callers of this like any other.

use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use plurimus_term::KeyModifiers;
use plurimus_ui::first_bound;

use super::keys::{TextInputAction, TextInputKeys};
use super::state::TextInput;
use super::word::{word_end_forward, word_start_backward, word_start_forward};

impl TextInput {
    /// Applies one key's editing action, reporting whether the field took it.
    ///
    /// Presses and repeats edit; a release never does. A key the field has no
    /// edit for is left untaken, [`Key::Enter`] among them - submitting is
    /// the dispatcher's decision rather than the field's, so a host binds it
    /// beside this call. A character chorded with anything but shift is
    /// untaken too, which is what lets ctrl+c reach whoever binds it instead
    /// of typing a `c`.
    ///
    /// `keys` is the table scanned first; an unbound unchorded character
    /// inserts itself, and [`TextInputAction::Submit`] is left untaken along
    /// with everything unbound, since committing is the dispatcher's. `held`
    /// is the modifier state a chord is matched against;
    /// [`HeldModifiers`](plurimus_term::bevy_compat::HeldModifiers) is where
    /// a bevy app gets it.
    ///
    /// Taking a key is not the same as changing the value - an arrow key
    /// takes one and edits nothing - so a host notifying on edits compares
    /// [`value`](Self::value) across the call rather than reading this.
    /// Showing the caret is the host's too: the stock stylist draws one for
    /// the focused field alone, which a field driven without focus is not.
    pub fn handle(
        &mut self,
        keys: &TextInputKeys,
        input: &KeyboardInput,
        held: KeyModifiers,
    ) -> bool {
        if input.state != ButtonState::Pressed {
            return false;
        }
        match first_bound(&keys.0, input, held) {
            Some(TextInputAction::Submit) => return false,
            Some(action) => self.apply(action),
            None => return self.insert_unbound(&input.logical_key, held),
        }
        true
    }

    /// Applies one [`TextInputAction`], for a host that resolved the key
    /// itself. [`Submit`](TextInputAction::Submit) edits nothing.
    pub fn apply(&mut self, action: TextInputAction) {
        let cursor = self.cursor();
        match action {
            // Word bindings mirror TextEditor's, whose engine binds
            // ctrl+arrows to word motion and alt+Backspace/Delete to word
            // deletion.
            TextInputAction::WordLeft => self.move_to(word_start_backward(self.value(), cursor)),
            TextInputAction::WordRight => self.move_to(word_start_forward(self.value(), cursor)),
            TextInputAction::WordBackspace => {
                self.delete_to(word_start_backward(self.value(), cursor));
            }
            TextInputAction::WordDelete => self.delete_to(word_end_forward(self.value(), cursor)),
            // A one-past-the-cursor target is a whole cluster step: the snap
            // carries it the rest of the way across the cluster.
            TextInputAction::Left => self.move_to(cursor.saturating_sub(1)),
            TextInputAction::Right => self.move_to(cursor + 1),
            TextInputAction::Backspace => self.delete_to(cursor.saturating_sub(1)),
            TextInputAction::Delete => self.delete_to(cursor + 1),
            TextInputAction::Home => self.move_start(),
            TextInputAction::End => self.move_end(),
            TextInputAction::Submit => {}
        }
    }

    /// What an unbound key types, which is nothing unless it is an unchorded
    /// character.
    ///
    /// Shift is not a chord: the kitty protocol reports a shifted letter with
    /// the bit set, so blocking it would stop capitals.
    fn insert_unbound(&mut self, key: &Key, held: KeyModifiers) -> bool {
        let chorded = held.ctrl || held.alt || held.super_key || held.hyper || held.meta;
        if chorded {
            return false;
        }
        match key {
            Key::Character(characters) => characters.chars().for_each(|c| self.insert(c)),
            Key::Space => self.insert(' '),
            _ => return false,
        }
        true
    }

    /// Inserts `text` at the cursor, dropping the control characters a
    /// bracketed paste can carry, and reports whether anything was inserted.
    pub fn paste(&mut self, text: &str) -> bool {
        let insertable: String = text.chars().filter(|c| !c.is_control()).collect();
        if insertable.is_empty() {
            return false;
        }
        self.insert_str(&insertable);
        true
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::entity::Entity;
    use bevy_input::ButtonState;
    use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
    use plurimus_term::KeyModifiers;

    use super::TextInput;
    use crate::text::keys::TextInputKeys;

    fn keys() -> TextInputKeys {
        TextInputKeys::default()
    }

    fn ctrl() -> KeyModifiers {
        KeyModifiers::default().with_ctrl(true)
    }

    // `handle` matches the logical key, so the physical code is arbitrary.
    fn pressed(logical_key: Key) -> KeyboardInput {
        KeyboardInput {
            key_code: KeyCode::Escape,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    fn character(character: &str) -> Key {
        Key::Character(character.into())
    }

    #[test]
    fn a_press_types_and_a_release_does_not() {
        let mut text = TextInput::new("");

        assert!(text.handle(&keys(), &pressed(character("a")), KeyModifiers::default()));
        let mut release = pressed(character("b"));
        release.state = ButtonState::Released;
        assert!(!text.handle(&keys(), &release, KeyModifiers::default()));

        assert_eq!(text.value(), "a");
    }

    #[test]
    fn a_repeat_edits_like_a_press() {
        let mut text = TextInput::new("ab");
        let mut repeat = pressed(Key::Backspace);
        repeat.repeat = true;

        assert!(text.handle(&keys(), &repeat, KeyModifiers::default()));

        assert_eq!(text.value(), "a", "holding backspace keeps deleting");
    }

    #[test]
    fn a_chorded_character_is_left_for_whoever_binds_it() {
        let mut text = TextInput::new("");

        assert!(!text.handle(&keys(), &pressed(character("c")), ctrl()));

        assert_eq!(text.value(), "", "ctrl+c copies somewhere, it never types");
    }

    #[test]
    fn shift_still_types() {
        let mut text = TextInput::new("");
        let shift = KeyModifiers::default().with_shift(true);

        assert!(text.handle(&keys(), &pressed(character("A")), shift));

        assert_eq!(text.value(), "A");
    }

    #[test]
    fn enter_is_left_to_the_dispatcher() {
        let mut text = TextInput::new("done");

        assert!(!text.handle(&keys(), &pressed(Key::Enter), KeyModifiers::default()));

        assert_eq!(text.value(), "done");
    }

    #[test]
    fn chords_move_by_word_and_plain_keys_by_cluster() {
        const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let mut text = TextInput::new(format!("one {FAMILY}"));

        assert!(text.handle(&keys(), &pressed(Key::Backspace), KeyModifiers::default()));
        assert_eq!(text.value(), "one ", "one press clears the whole cluster");

        assert!(text.handle(&keys(), &pressed(Key::ArrowLeft), ctrl()));
        assert_eq!(text.cursor(), 0, "ctrl+left crosses the word");
    }

    #[test]
    fn paste_drops_control_characters() {
        let mut text = TextInput::new("");

        assert!(text.paste("wo\nrld"));
        assert!(!text.paste(""));

        assert_eq!(text.value(), "world");
    }
}
