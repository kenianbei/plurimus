//! plurimus in a browser page: a list to click, an editor to type, paste,
//! copy (ctrl-c, after selecting with shift-arrows) and scroll with the
//! wheel, and buttons that set the page's title and quit. Built for
//! `wasm32-unknown-unknown` and served as a static page:
//!
//! ```sh
//! cargo build --release --example web --target wasm32-unknown-unknown \
//!     --no-default-features --features web,widgets
//! wasm-bindgen --target web --out-dir examples/web/pkg \
//!     target/wasm32-unknown-unknown/release/examples/web.wasm
//! python -m http.server -d examples/web
//! ```
//!
//! `wasm-bindgen` is the CLI at the version `Cargo.lock` pins, which turns
//! this `main` into the page's start function.

#[cfg(any(target_arch = "wasm32", test))]
mod demo;
#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
fn main() {
    use bevy_app::App;
    use plurimus::core::CorePlugin;
    use plurimus::web::{GridFit, WebPlugin};

    let mut app = App::new();
    app.add_plugins((
        CorePlugin,
        WebPlugin::new().fit(GridFit::Cells(demo::COLUMNS, demo::ROWS)),
    ));
    demo::add_demo(&mut app);
    app.run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("the web example runs in a browser: see the build steps atop examples/web/main.rs");
}
