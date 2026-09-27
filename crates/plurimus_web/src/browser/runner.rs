//! The frame loop: `requestAnimationFrame` in place of a blocking loop.

use std::cell::RefCell;
use std::rc::Rc;

use bevy_app::{App, AppExit, PluginsState};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::{CustomEvent, CustomEventInit, HtmlCanvasElement};

use super::{document, warn, window};

/// The event dispatched on the canvas when the app exits, its `detail` the
/// exit code.
pub const EXIT_EVENT: &str = "plurimus-exit";

/// What the runner needs of the page when the app exits; a non-`Send`
/// resource, since it holds a DOM element.
pub(crate) struct ExitTarget {
    pub(crate) canvas: HtmlCanvasElement,
    pub(crate) title: String,
}

type Frame = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

/// Updates the app once per animation frame until it exits.
///
/// Waits across frames for every plugin to be ready first: the font loads
/// asynchronously, and so does a GPU device an app's render stack creates.
pub(crate) fn run_on_animation_frames(mut app: App) -> AppExit {
    let frame: Frame = Rc::default();
    let next = Rc::clone(&frame);
    *frame.borrow_mut() = Some(Closure::new(move || {
        if settle_plugins(&mut app) {
            app.update();
            if let Some(exit) = app.should_exit() {
                finish_exit(&app, &exit);
                return;
            }
        }
        request_frame(&next);
    }));
    request_frame(&frame);
    AppExit::Success
}

/// Finishes the app's plugins once they are ready; whether it may update.
fn settle_plugins(app: &mut App) -> bool {
    match app.plugins_state() {
        PluginsState::Adding => return false,
        PluginsState::Ready => {
            app.finish();
            app.cleanup();
        }
        PluginsState::Finished => app.cleanup(),
        PluginsState::Cleaned => {}
    }
    true
}

fn request_frame(frame: &Frame) {
    let frame = frame.borrow();
    let Some(callback) = frame.as_ref() else {
        return;
    };
    if window()
        .request_animation_frame(callback.as_ref().unchecked_ref())
        .is_err()
    {
        warn("plurimus_web: requestAnimationFrame was refused; the app has stopped");
    }
}

/// Hands the page back: its title as it was, and the exit to decide about.
fn finish_exit(app: &App, exit: &AppExit) {
    let Some(target) = app.world().get_non_send::<ExitTarget>() else {
        return;
    };
    document().set_title(&target.title);
    let code = match exit {
        AppExit::Success => 0,
        AppExit::Error(code) => code.get(),
    };
    let init = CustomEventInit::new();
    init.set_detail(&code.into());
    let dispatched = CustomEvent::new_with_event_init_dict(EXIT_EVENT, &init)
        .and_then(|event| target.canvas.dispatch_event(&event));
    if dispatched.is_err() {
        warn("plurimus_web: the exit event could not be dispatched");
    }
}
