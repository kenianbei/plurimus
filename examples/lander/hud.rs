use bevy_app::{App, Startup, Update};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{Commands, Component, On, Query, Res, ResMut, With};
use bevy_transform::components::Transform;
use plurimus::core::ratatui_core::layout::Rect;
use plurimus::core::{TerminalCamera, TerminalSize, Viewport};
use plurimus::ui::{UiArea, UiCamera, UiOrder, UiWidget};
use plurimus::widgets::ratatui_widgets::block::Block;
use plurimus::widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus::widgets::{Activate, button};

use crate::game::{Fuel, GROUND_CLEARANCE, Lander, Phase, reset};

const PANEL_COLS: u16 = 22;
const PANEL_ROWS: u16 = 11;
const PANEL_BODY: Rect = Rect {
    x: 2,
    y: 1,
    width: 18,
    height: 7,
};
const RESTART_BUTTON: Rect = Rect {
    x: 5,
    y: 9,
    width: 12,
    height: 1,
};

#[derive(Component)]
struct PanelBody;

#[derive(Component)]
struct PanelCamera;

pub fn add_hud(app: &mut App) {
    app.add_systems(Startup, spawn_panel);
    app.add_systems(Update, (anchor_panel, update_panel));
}

fn spawn_panel(mut commands: Commands) {
    let panel = commands
        .spawn((
            TerminalCamera {
                order: 2,
                viewport: Viewport::Fixed(Rect::ZERO),
                ..TerminalCamera::default()
            },
            PanelCamera,
        ))
        .id();
    commands.spawn((
        UiWidget::new(Block::bordered().title("LANDER")),
        UiCamera(panel),
    ));
    commands.spawn((
        UiWidget::new(Paragraph::new("")),
        UiArea::Fixed(PANEL_BODY),
        UiOrder(1),
        UiCamera(panel),
        PanelBody,
    ));
    commands
        .spawn((
            button("restart"),
            UiArea::Fixed(RESTART_BUTTON),
            UiOrder(1),
            UiCamera(panel),
        ))
        .observe(
            |_on: On<Activate>,
             mut phase: ResMut<Phase>,
             mut fuel: ResMut<Fuel>,
             mut landers: Query<(&mut Lander, &mut Transform)>| {
                reset(&mut phase, &mut fuel, &mut landers);
            },
        );
}

fn anchor_panel(
    size: Res<TerminalSize>,
    mut cameras: Query<&mut TerminalCamera, With<PanelCamera>>,
) {
    if !size.is_changed() {
        return;
    }
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    camera.viewport = Viewport::Fixed(Rect {
        x: size.cols.saturating_sub(PANEL_COLS),
        y: 0,
        width: PANEL_COLS,
        height: PANEL_ROWS,
    });
}

fn update_panel(
    phase: Res<Phase>,
    fuel: Res<Fuel>,
    landers: Query<(&Lander, &Transform)>,
    mut body: Query<&mut UiWidget, With<PanelBody>>,
) {
    let Ok((lander, transform)) = landers.single() else {
        return;
    };
    let text = format!(
        "FUEL {:>6.1}\nVX   {:+6.1}\nVY   {:+6.1}\nALT  {:>6.1}\n\n{}",
        fuel.0,
        lander.velocity.x,
        lander.velocity.y,
        transform.translation.y - GROUND_CLEARANCE,
        phase_word(*phase),
    );
    if let Ok(mut panel) = body.single_mut() {
        *panel = UiWidget::new(Paragraph::new(text));
    }
}

fn phase_word(phase: Phase) -> &'static str {
    match phase {
        Phase::Flying => "FLYING",
        Phase::Landed => "LANDED",
        Phase::Crashed => "CRASHED",
    }
}
