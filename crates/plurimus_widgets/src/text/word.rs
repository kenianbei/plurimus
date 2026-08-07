//! Word boundaries over char-indexed cursors.
//!
//! Mirrors ratatui-textarea's rules rather than UAX #29 so that the same
//! keybinding stops at the same places in both text widgets. Words break
//! where the character class changes, which means an unspaced CJK run
//! counts as a single word.

use super::grapheme::char_to_byte;

#[derive(PartialEq, Eq, Clone, Copy)]
enum CharKind {
    Space,
    Punct,
    Other,
}

impl CharKind {
    const fn of(character: char) -> Self {
        if character.is_whitespace() {
            Self::Space
        } else if character != '_' && character.is_ascii_punctuation() {
            Self::Punct
        } else {
            Self::Other
        }
    }
}

/// Char index of the next word start after `position`, saturating at the
/// end of `value`.
pub(crate) fn word_start_forward(value: &str, position: usize) -> usize {
    let mut characters = value.chars().enumerate().skip(position);
    let Some((_, first)) = characters.next() else {
        return position;
    };
    let mut previous = CharKind::of(first);
    for (index, character) in characters {
        let kind = CharKind::of(character);
        if kind != CharKind::Space && previous != kind {
            return index;
        }
        previous = kind;
    }
    value.chars().count()
}

/// Char index just past the word containing `position`, saturating at the
/// end of `value`.
///
/// Forward deletion stops here rather than at [`word_start_forward`], so
/// deleting forward over `fn foo` leaves the separating space in place -
/// matching ratatui-textarea's `delete_next_word`.
pub(crate) fn word_end_forward(value: &str, position: usize) -> usize {
    let mut characters = value.chars().enumerate().skip(position);
    let Some((_, first)) = characters.next() else {
        return position;
    };
    let mut previous = CharKind::of(first);
    for (index, character) in characters {
        let kind = CharKind::of(character);
        if previous != CharKind::Space && previous != kind {
            return index;
        }
        previous = kind;
    }
    value.chars().count()
}

/// Char index of the word start preceding `position`, saturating at 0.
///
/// Diverges from textarea deliberately: there a leading run of whitespace
/// reports no boundary, letting the caller join the previous line. A
/// single-line field has nowhere to join, so it clamps to the head.
pub(crate) fn word_start_backward(value: &str, position: usize) -> usize {
    let end = char_to_byte(value, position);
    let mut characters = value[..end].chars().rev().enumerate();
    let Some((_, first)) = characters.next() else {
        return 0;
    };
    let mut current = CharKind::of(first);
    for (steps, character) in characters {
        let kind = CharKind::of(character);
        if current != CharKind::Space && kind != current {
            return position - steps;
        }
        current = kind;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: &str = "fn foo(a)";

    #[test]
    fn forward_stops_match_textarea() {
        let expected = [3, 6, 7, 8, 9];
        let mut stops = Vec::new();
        let mut position = 0;
        for _ in 0..expected.len() {
            position = word_start_forward(CODE, position);
            stops.push(position);
        }
        assert_eq!(stops, expected);
    }

    #[test]
    fn backward_stops_match_textarea() {
        let expected = [8, 7, 6, 3, 0];
        let mut stops = Vec::new();
        let mut position = CODE.chars().count();
        for _ in 0..expected.len() {
            position = word_start_backward(CODE, position);
            stops.push(position);
        }
        assert_eq!(stops, expected);
    }

    #[test]
    fn boundaries_saturate() {
        assert_eq!(word_start_backward(CODE, 0), 0);
        assert_eq!(word_start_forward(CODE, 9), 9);
        assert_eq!(word_start_forward("", 0), 0);
        assert_eq!(word_start_backward("", 0), 0);
    }

    #[test]
    fn forward_deletion_stops_before_the_separator() {
        assert_eq!(word_end_forward("fn foo bar", 0), 2);
        assert_eq!(word_end_forward("fn foo(a)", 3), 6);
        assert_eq!(word_end_forward("fn", 2), 2);
    }

    #[test]
    fn underscores_join_a_word() {
        assert_eq!(word_start_forward("a_b c", 0), 4);
    }
}
