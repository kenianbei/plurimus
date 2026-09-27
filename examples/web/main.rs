//! plurimus in a browser page: a list to click, an editor to type, paste,
//! copy (ctrl-c, after selecting with shift-arrows) and scroll with the
//! wheel, and buttons that set the page's title and quit. The README's
//! Examples section has the steps to build and serve it.

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
    eprintln!("the web example runs in a browser: see the README's Examples section");
}
