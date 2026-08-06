//! Keyboard half of the input contract: key messages and modifier state.

use bevy_ecs::message::Message;

/// A key event forwarded from the terminal backend.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyMessage {
    /// The key.
    pub code: KeyCode,
    /// Modifier state at event time.
    pub modifiers: KeyModifiers,
    /// Press, repeat, or release.
    pub kind: KeyKind,
}

/// The kind of a key event.
///
/// Legacy terminals only report presses; releases are synthesized after
/// [`crate::ReleaseTimeout`] unless [`crate::InputCapabilities::key_release`]
/// is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyKind {
    /// Key went down.
    Press,
    /// Key is held (terminal autorepeat or kitty repeat).
    Repeat,
    /// Key went up.
    Release,
}

/// Keys reported by terminal backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyCode {
    /// A printable character.
    Char(char),
    /// The enter/return key.
    Enter,
    /// The escape key.
    Esc,
    /// The tab key.
    Tab,
    /// The backspace key.
    Backspace,
    /// The delete key.
    Delete,
    /// The insert key.
    Insert,
    /// The up arrow.
    Up,
    /// The down arrow.
    Down,
    /// The left arrow.
    Left,
    /// The right arrow.
    Right,
    /// The home key.
    Home,
    /// The end key.
    End,
    /// The page-up key.
    PageUp,
    /// The page-down key.
    PageDown,
    /// A function key.
    F(u8),
    /// The caps lock key (kitty tier only).
    CapsLock,
    /// The num lock key (kitty tier only).
    NumLock,
    /// The scroll lock key (kitty tier only).
    ScrollLock,
    /// A modifier key pressed as a key in its own right (kitty tier only;
    /// legacy terminals never emit these).
    Modifier(ModifierKey),
}

/// Modifier keys reported as keys, with left/right distinguished.
///
/// [`KeyModifiers`] does not keep that distinction: both sides drive one
/// flag there. Iso-level shift keys are not represented and stay dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ModifierKey {
    /// Left shift.
    ShiftLeft,
    /// Right shift.
    ShiftRight,
    /// Left control.
    ControlLeft,
    /// Right control.
    ControlRight,
    /// Left alt.
    AltLeft,
    /// Right alt.
    AltRight,
    /// Left super.
    SuperLeft,
    /// Right super.
    SuperRight,
    /// Left hyper.
    HyperLeft,
    /// Right hyper.
    HyperRight,
    /// Left meta.
    MetaLeft,
    /// Right meta.
    MetaRight,
}

/// Modifier keys held during an input event.
///
/// One flag per modifier, with left and right collapsed: a `ShiftRight`
/// press sets `shift`. Super/hyper/meta are only reported on the kitty
/// tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct KeyModifiers {
    /// Control held.
    pub ctrl: bool,
    /// Alt held.
    pub alt: bool,
    /// Shift held.
    pub shift: bool,
    /// Super/command held.
    pub super_key: bool,
    /// Hyper held.
    pub hyper: bool,
    /// Meta held.
    pub meta: bool,
}

impl KeyModifiers {
    /// Whether the flag `key` drives is held.
    ///
    /// Sides collapse: `ShiftLeft` and `ShiftRight` both read `shift`.
    #[must_use]
    pub const fn holds(mut self, key: ModifierKey) -> bool {
        *self.slot(key)
    }

    /// Sets whether control is held.
    #[must_use]
    pub const fn with_ctrl(mut self, held: bool) -> Self {
        self.ctrl = held;
        self
    }

    /// Sets whether alt is held.
    #[must_use]
    pub const fn with_alt(mut self, held: bool) -> Self {
        self.alt = held;
        self
    }

    /// Sets whether shift is held.
    #[must_use]
    pub const fn with_shift(mut self, held: bool) -> Self {
        self.shift = held;
        self
    }

    /// Sets whether super/command is held.
    #[must_use]
    pub const fn with_super_key(mut self, held: bool) -> Self {
        self.super_key = held;
        self
    }

    /// Sets whether hyper is held.
    #[must_use]
    pub const fn with_hyper(mut self, held: bool) -> Self {
        self.hyper = held;
        self
    }

    /// Sets whether meta is held.
    #[must_use]
    pub const fn with_meta(mut self, held: bool) -> Self {
        self.meta = held;
        self
    }

    pub(crate) const fn slot(&mut self, key: ModifierKey) -> &mut bool {
        use ModifierKey as M;
        match key {
            M::ShiftLeft | M::ShiftRight => &mut self.shift,
            M::ControlLeft | M::ControlRight => &mut self.ctrl,
            M::AltLeft | M::AltRight => &mut self.alt,
            M::SuperLeft | M::SuperRight => &mut self.super_key,
            M::HyperLeft | M::HyperRight => &mut self.hyper,
            M::MetaLeft | M::MetaRight => &mut self.meta,
        }
    }
}

impl From<ModifierKey> for KeyModifiers {
    /// The modifier state implied by holding `key`, with no other
    /// modifiers. Sides collapse, so both shift keys yield `shift`.
    fn from(key: ModifierKey) -> Self {
        let mut modifiers = Self::default();
        *modifiers.slot(key) = true;
        modifiers
    }
}
