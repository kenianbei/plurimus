//! Browser events on the canvas, pumped into `plurimus_term`'s messages.

use std::cell::RefCell;
use std::rc::Rc;

use bevy_ecs::prelude::{MessageWriter, NonSend, Res};
use bevy_ecs::system::SystemParam;
use bevy_input::ButtonState;
use bevy_input::keyboard::Key;
use plurimus_core::TerminalSize;
use plurimus_term::{KeyModifiers, MouseButton, PasteMessage, TerminalInput};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::{
    AddEventListenerOptions, ClipboardEvent, Event, HtmlCanvasElement, KeyboardEvent, MouseEvent,
    PointerEvent, WheelEvent,
};

use super::warn;
use crate::keys::key;
use crate::pointer::{WheelResidue, button, cell_at};

/// An event as a listener saw it, before the grid is consulted.
enum Pending {
    Key {
        key: Key,
        state: ButtonState,
        repeat: bool,
        modifiers: KeyModifiers,
    },
    Pointer {
        kind: PointerKind,
        offset: (f64, f64),
        modifiers: KeyModifiers,
    },
    Paste(String),
    Focus(bool),
}

type Queue = Rc<RefCell<Vec<Pending>>>;
type Listener = Closure<dyn FnMut(Event)>;

/// The canvas's listeners and what they have queued since the last pump; a
/// non-`Send` resource, since it holds closures the page calls into.
pub(crate) struct BrowserInput {
    queue: Queue,
    cell_css: (f64, f64),
    _listeners: Vec<Listener>,
}

/// Listens on `canvas` for everything the input contract carries.
///
/// `cell_css` is a cell's size in CSS pixels, fixed with the font, which is
/// how a pointer offset becomes a cell.
pub(crate) fn listen(
    canvas: &HtmlCanvasElement,
    passthrough: Vec<(Key, KeyModifiers)>,
    cell_css: (f64, f64),
) -> BrowserInput {
    let queue = Queue::default();
    let listeners = vec![
        on(canvas, "keydown", key_down(&queue, passthrough)),
        on(canvas, "keyup", key_up(&queue)),
        on(canvas, "pointerdown", pointer_down(&queue, canvas)),
        on(canvas, "pointerup", pointer_up(&queue)),
        on(canvas, "pointermove", pointer_move(&queue)),
        on(canvas, "wheel", wheel(&queue)),
        on(canvas, "paste", paste(&queue)),
        on(canvas, "focus", focus(&queue, true)),
        on(canvas, "blur", focus(&queue, false)),
        on(canvas, "contextmenu", |event: Event| {
            event.prevent_default();
        }),
    ];
    BrowserInput {
        queue,
        cell_css,
        _listeners: listeners,
    }
}

/// Registers `handler` for `kind`, not passive, since most handlers keep
/// the event from the browser.
fn on(canvas: &HtmlCanvasElement, kind: &str, handler: impl FnMut(Event) + 'static) -> Listener {
    let listener = Listener::new(handler);
    let options = AddEventListenerOptions::new();
    options.set_passive(false);
    if canvas
        .add_event_listener_with_callback_and_add_event_listener_options(
            kind,
            listener.as_ref().unchecked_ref(),
            &options,
        )
        .is_err()
    {
        warn(&format!("plurimus_web: could not listen for {kind}"));
    }
    listener
}

/// A press takes the key from the browser unless it passes through.
fn key_down(queue: &Queue, passthrough: Vec<(Key, KeyModifiers)>) -> impl FnMut(Event) + use<> {
    let queue = Rc::clone(queue);
    move |event: Event| {
        let event: KeyboardEvent = event.unchecked_into();
        let Some(key) = key(&event.key()) else {
            return;
        };
        let modifiers = key_modifiers(&event);
        if !passthrough
            .iter()
            .any(|(passed, held)| *passed == key && *held == modifiers)
        {
            event.prevent_default();
        }
        queue.borrow_mut().push(Pending::Key {
            key,
            state: ButtonState::Pressed,
            repeat: event.repeat(),
            modifiers,
        });
    }
}

fn key_up(queue: &Queue) -> impl FnMut(Event) + use<> {
    let queue = Rc::clone(queue);
    move |event: Event| {
        let event: KeyboardEvent = event.unchecked_into();
        let Some(key) = key(&event.key()) else {
            return;
        };
        queue.borrow_mut().push(Pending::Key {
            key,
            state: ButtonState::Released,
            repeat: false,
            modifiers: key_modifiers(&event),
        });
    }
}

/// A pointer report in the sink's terms.
#[derive(Debug, Clone, Copy)]
enum PointerKind {
    Button(MouseButton, ButtonState),
    Moved,
    Wheel(i32, i32),
}

/// A press focuses the canvas and captures the pointer, so a drag keeps
/// reporting after it leaves the canvas.
fn pointer_down(queue: &Queue, canvas: &HtmlCanvasElement) -> impl FnMut(Event) + use<> {
    let queue = Rc::clone(queue);
    let canvas = canvas.clone();
    move |event: Event| {
        let event: PointerEvent = event.unchecked_into();
        let _ = canvas.focus();
        let _ = canvas.set_pointer_capture(event.pointer_id());
        if let Some(pressed) = button(event.button()) {
            push_pointer(
                &queue,
                &event,
                PointerKind::Button(pressed, ButtonState::Pressed),
            );
        }
    }
}

fn pointer_up(queue: &Queue) -> impl FnMut(Event) + use<> {
    let queue = Rc::clone(queue);
    move |event: Event| {
        let event: PointerEvent = event.unchecked_into();
        if let Some(released) = button(event.button()) {
            push_pointer(
                &queue,
                &event,
                PointerKind::Button(released, ButtonState::Released),
            );
        }
    }
}

fn pointer_move(queue: &Queue) -> impl FnMut(Event) + use<> {
    let queue = Rc::clone(queue);
    move |event: Event| {
        let event: PointerEvent = event.unchecked_into();
        push_pointer(&queue, &event, PointerKind::Moved);
    }
}

fn wheel(queue: &Queue) -> impl FnMut(Event) + use<> {
    let queue = Rc::clone(queue);
    let mut residue = WheelResidue::default();
    move |event: Event| {
        event.prevent_default();
        let event: WheelEvent = event.unchecked_into();
        let (x, y) = residue.notches((event.delta_x(), event.delta_y()), event.delta_mode());
        for _ in 0..y.unsigned_abs() {
            push_pointer(&queue, &event, PointerKind::Wheel(0, y.signum()));
        }
        for _ in 0..x.unsigned_abs() {
            push_pointer(&queue, &event, PointerKind::Wheel(x.signum(), 0));
        }
    }
}

fn paste(queue: &Queue) -> impl FnMut(Event) + use<> {
    let queue = Rc::clone(queue);
    move |event: Event| {
        event.prevent_default();
        let event: ClipboardEvent = event.unchecked_into();
        let text = event
            .clipboard_data()
            .and_then(|transfer| transfer.get_data("text").ok());
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            queue.borrow_mut().push(Pending::Paste(text));
        }
    }
}

fn focus(queue: &Queue, gained: bool) -> impl FnMut(Event) + use<> {
    let queue = Rc::clone(queue);
    move |_| queue.borrow_mut().push(Pending::Focus(gained))
}

fn push_pointer(queue: &Queue, event: &MouseEvent, kind: PointerKind) {
    let modifiers = pointer_modifiers(event);
    queue.borrow_mut().push(Pending::Pointer {
        kind,
        offset: (f64::from(event.offset_x()), f64::from(event.offset_y())),
        modifiers,
    });
}

/// The modifiers a key event says are held. The browser's meta key is the
/// OS key - command on a Mac, the Windows key elsewhere - which is
/// `super_key`.
fn key_modifiers(event: &KeyboardEvent) -> KeyModifiers {
    KeyModifiers::none()
        .with_ctrl(event.ctrl_key())
        .with_alt(event.alt_key())
        .with_shift(event.shift_key())
        .with_super_key(event.meta_key())
}

/// As [`key_modifiers`], for a pointer event.
fn pointer_modifiers(event: &MouseEvent) -> KeyModifiers {
    KeyModifiers::none()
        .with_ctrl(event.ctrl_key())
        .with_alt(event.alt_key())
        .with_shift(event.shift_key())
        .with_super_key(event.meta_key())
}

#[derive(SystemParam)]
pub(crate) struct InputSinks<'w, 's> {
    input: TerminalInput<'w, 's>,
    paste: MessageWriter<'w, PasteMessage>,
}

/// Writes what the listeners queued since the last frame, in the order it
/// happened, placing pointers against this frame's grid.
pub(crate) fn pump_browser_events(
    input: NonSend<BrowserInput>,
    size: Res<TerminalSize>,
    mut sinks: InputSinks,
) {
    for pending in input.queue.borrow_mut().drain(..) {
        match pending {
            Pending::Key {
                key,
                state,
                repeat,
                modifiers,
            } => sinks.input.key(key, state, repeat, modifiers),
            Pending::Pointer {
                kind,
                offset,
                modifiers,
            } => {
                let Some(at) = cell_at(offset, input.cell_css, (size.cols, size.rows)) else {
                    continue;
                };
                match kind {
                    PointerKind::Button(button, state) => {
                        sinks.input.button(button, state, at, modifiers);
                    }
                    PointerKind::Moved => sinks.input.moved(at, modifiers),
                    PointerKind::Wheel(x, y) => sinks.input.wheel(at, x, y, modifiers),
                }
            }
            Pending::Paste(text) => {
                sinks.paste.write(PasteMessage(text));
            }
            Pending::Focus(focused) => sinks.input.focus(focused),
        }
    }
}
