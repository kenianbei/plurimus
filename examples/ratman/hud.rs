//! The status strip along the top and the centered notices.

use bevy_ecs::prelude::{Commands, Component, Entity, Query, Res, With};
use plurimus::core::ratatui_core::layout::Constraint;
use plurimus::core::ratatui_core::style::{Color, Style};
use plurimus::core::{TerminalSize, UiArea, UiCamera, UiHidden, UiWidget};
use plurimus::widgets::ratatui_widgets::block::{Block, Padding};
use plurimus::widgets::ratatui_widgets::paragraph::Paragraph;

use crate::game::{Lives, Phase, Score, terminal_fits};
use crate::maze::{Cheese, REQUIRED_COLS, REQUIRED_ROWS};

const NOTICE_WIDTH: u16 = 34;
const NOTICE_HEIGHT: u16 = 5;

#[derive(Component)]
pub struct HudCamera;

#[derive(Component)]
pub struct HudLine;

#[derive(Component)]
pub struct Notice;

pub fn spawn_hud(commands: &mut Commands, hud_camera: Entity, overlay: Entity) {
    commands.spawn((
        HudLine,
        UiWidget::new(Paragraph::new("")),
        UiCamera(hud_camera),
    ));
    commands.spawn((
        Notice,
        UiWidget::new(Paragraph::new("")),
        UiCamera(overlay),
        UiArea::Fill,
        UiHidden,
    ));
}

pub fn update_hud(
    score: Res<Score>,
    lives: Res<Lives>,
    cheese: Query<&Cheese>,
    mut lines: Query<&mut UiWidget, With<HudLine>>,
) {
    let Ok(mut line) = lines.single_mut() else {
        return;
    };
    let text = format!(
        "  score {}     cheese {}     lives {}     arrows or wasd to move, q to quit",
        score.0,
        cheese.iter().count(),
        lives.0
    );
    *line = UiWidget::new(Paragraph::new(text).style(Style::new().fg(Color::Yellow)));
}

pub fn update_notice(
    mut commands: Commands,
    phase: Res<Phase>,
    size: Res<TerminalSize>,
    mut notices: Query<(Entity, &mut UiWidget, &mut UiArea), With<Notice>>,
) {
    let Ok((entity, mut widget, mut area)) = notices.single_mut() else {
        return;
    };
    let Some(text) = notice_text(&phase, &size) else {
        commands.entity(entity).insert(UiHidden);
        return;
    };
    commands.entity(entity).remove::<UiHidden>();
    *area = UiArea::Fixed(size.rect().centered(
        Constraint::Length(NOTICE_WIDTH),
        Constraint::Length(NOTICE_HEIGHT),
    ));
    *widget = UiWidget::new(
        Paragraph::new(text)
            .centered()
            .style(Style::new().fg(Color::White).bg(Color::Rgb(20, 20, 40)))
            .block(Block::bordered().padding(Padding::vertical(1))),
    );
}

fn notice_text(phase: &Phase, size: &TerminalSize) -> Option<String> {
    if !terminal_fits(size) {
        return Some(format!(
            "the terminal is too small\nthe maze needs {REQUIRED_COLS} x {REQUIRED_ROWS} cells"
        ));
    }
    match phase {
        Phase::Playing => None,
        Phase::Won => Some("the maze is clear\npress r to play again".into()),
        Phase::Lost => Some("the birds got you\npress r to play again".into()),
    }
}
