//! Snapshot tests for bui decoration approximations: rounded corners,
//! gradients, and box shadows.

use bevy_app::App;
use bevy_color::Color;
use bevy_ecs::prelude::ChildOf;
use bevy_ui::{BackgroundColor, BorderColor, Node, UiRect, Val};
use plurimus_bui::BuiPlugin;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::composed_styled_frame;

fn app(cols: u16, rows: u16) -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, BuiPlugin));
    app.insert_resource(TerminalSize { cols, rows });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

#[test]
fn border_radius_rounds_corner_glyphs() {
    use bevy_ui::BorderRadius;

    let mut app = app(8, 3);
    app.world_mut().spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(1.0)),
            ..Node::default()
        },
        BorderColor::from(Color::srgb(0.0, 1.0, 0.0)),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_rounded_border", composed_styled_frame(&app));
}

#[test]
fn linear_gradient_background_samples_cells() {
    use bevy_ui::{BackgroundGradient, ColorStop, Gradient, LinearGradient};

    let mut app = app(8, 2);
    app.world_mut().spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        },
        BackgroundGradient(vec![Gradient::Linear(LinearGradient::to_right(vec![
            ColorStop::auto(Color::srgb(1.0, 0.0, 0.0)),
            ColorStop::auto(Color::srgb(0.0, 0.0, 1.0)),
        ]))]),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_linear_gradient", composed_styled_frame(&app));
}

#[test]
fn radial_gradient_background_samples_cells() {
    use bevy_ui::{BackgroundGradient, ColorStop, Gradient, RadialGradient};

    let mut app = app(9, 3);
    app.world_mut().spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        },
        BackgroundGradient(vec![Gradient::Radial(RadialGradient {
            stops: vec![
                ColorStop::auto(Color::srgb(1.0, 1.0, 1.0)),
                ColorStop::auto(Color::srgb(0.0, 0.0, 0.0)),
            ],
            ..RadialGradient::default()
        })]),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_radial_gradient", composed_styled_frame(&app));
}

#[test]
fn conic_gradient_sweeps_by_angle() {
    use bevy_ui::{AngularColorStop, BackgroundGradient, ConicGradient, Gradient, UiPosition};

    let mut app = app(9, 3);
    app.world_mut().spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        },
        BackgroundGradient(vec![Gradient::Conic(ConicGradient::new(
            UiPosition::CENTER,
            vec![
                AngularColorStop::auto(Color::srgb(1.0, 1.0, 1.0)),
                AngularColorStop::auto(Color::srgb(0.0, 0.0, 0.0)),
            ],
        ))]),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_conic_sweep", composed_styled_frame(&app));
}

#[test]
fn stop_hint_shifts_the_midpoint() {
    use bevy_ui::{BackgroundGradient, ColorStop, Gradient, LinearGradient};

    let mut app = app(8, 1);
    app.world_mut().spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Node::default()
        },
        BackgroundGradient(vec![Gradient::Linear(LinearGradient::to_right(vec![
            ColorStop::auto(Color::srgb(1.0, 0.0, 0.0)).with_hint(0.25),
            ColorStop::auto(Color::srgb(0.0, 0.0, 1.0)),
        ]))]),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_hinted_gradient", composed_styled_frame(&app));
}

#[test]
fn box_shadow_dims_cells_outside_the_node() {
    use bevy_ui::{BoxShadow, PositionType, ShadowStyle};

    let mut app = app(8, 3);
    let root = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..Node::default()
            },
            BackgroundColor(Color::srgb(0.0, 0.6, 0.0)),
        ))
        .id();
    app.world_mut().spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(1.0),
            top: Val::Px(0.0),
            width: Val::Px(4.0),
            height: Val::Px(1.0),
            ..Node::default()
        },
        BackgroundColor(Color::srgb(1.0, 1.0, 0.0)),
        BoxShadow(vec![ShadowStyle {
            color: Color::srgba(0.0, 0.0, 0.0, 1.0),
            x_offset: Val::Px(1.0),
            y_offset: Val::Px(1.0),
            spread_radius: Val::ZERO,
            blur_radius: Val::ZERO,
        }]),
        ChildOf(root),
    ));

    app.update();
    app.update();

    insta::assert_snapshot!("bui_box_shadow", composed_styled_frame(&app));
}
