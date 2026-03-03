#![cfg(feature = "tachyonfx")]

use bevy::prelude::*;
use bevy_ratatui::RatatuiPlugins;
use plurimus::*;

#[derive(Component)]
struct PreviewCanvas;

#[derive(Component)]
struct StatusBar;

const PREVIEW_ART: &str = r#"
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣠⡤⠴⡿⠓⠶⠾⠿⠶⣤⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣠⡖⠋⠉⠁⠀⠀⠀⠀⠀⠀⠀⠀⠈⠙⠷⣤⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣴⡿⠁⠀⠀⠀⠀⠀⠀⠀⠀⣤⣤⣿⠖⠻⠷⡶⣮⡙⣦⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣠⠞⠉⠀⠀⠀⠀⠀⠀⠀⢀⣚⡯⠉⠀⠀⠀⠀⠀⠀⠀⠉⠛⢷⣄⣀⣀⣀⣀⣠⣤⣄⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⠟⠀⠀⠀⠀⠀⠀⠀⢀⣰⠿⠛⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠉⠉⠀⠉⠉⠛⠿⣿⣆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣠⠏⠀⠀⠀⠀⠀⠀⠀⠀⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡴⣾⣉⠳⠄⠀⠀⠀⠀⠀⠀⠀⠉⠻⢶⣄⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣴⠏⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⣅⡀⠉⠁⠀⠀⠀⠀⢠⣴⣤⡀⠀⠀⠀⠙⢷⣄⡀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣠⡴⣿⣿⠋⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣳⣾⠿⠁⠀⠀⠀⠀⠀⠀⠻⠿⠿⠟⠀⠀⠀⠀⠀⠉⠻⣦⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣤⠖⠋⠁⢀⣼⡧⠀⠀⠀⠀⠀⠘⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠠⢶⡿⠀⠀⠀⠀⠀⠀⠀⠀⢀⣀⣀⣀⣠⣤⣴⡒⠒⠶⣤⣿⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⢀⡴⠏⠁⠀⢀⣠⣼⡟⠀⠀⠀⠀⠀⠀⠀⣿⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢠⣿⣿⣤⠀⠀⠀⠤⠖⠚⠉⠉⣀⡠⠤⠒⢲⡆⠁⢀⡴⢩⡿⢤⡀⠀⠀
⠀⠀⠀⠀⠀⠀⢀⣴⠋⠀⢀⣴⠞⠋⠉⢸⡇⠀⠀⠀⠀⠀⠀⠀⢽⣟⠂⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢠⠴⠋⠀⠀⠀⠀⠀⢀⡠⠖⠋⠁⢀⣤⣾⣥⠤⠴⠛⠋⠉⠙⣆⠉⠢⡄
⠀⠀⠀⠀⠀⣠⠟⠁⢠⡾⠋⠁⠀⠀⠀⣼⡇⠀⡀⠀⠀⠀⠀⠀⢰⣿⣗⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⡔⠋⠀⠀⠀⢠⠞⠋⠀⠀⠀⠀⠀⠀⠀⠀⠈⢆⠀⠈
⠀⠀⠀⠀⣴⠋⢀⡴⠋⠀⠀⠀⠀⠀⠀⣿⠿⢛⣣⣄⣀⡀⠀⠀⠀⢨⣿⣧⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⠀⠀⢠⣄⣴⠋⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠘⡆⠀
⠀⠀⠀⣼⠇⢀⡟⠁⠀⠀⠀⠀⠀⠀⠰⣿⠀⠈⠈⢻⣟⠉⠉⠉⠉⠉⠛⠻⢶⣤⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣼⢠⣶⠏⢸⠛⠛⠒⢲⣶⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⢠⡟⠀⣸⠁⠀⠀⠀⠀⠀⠀⠀⢰⡟⠀⠀⠀⡀⣿⣷⣄⠀⠀⠀⠀⠀⠀⠙⠿⣿⣀⢀⣀⣤⣄⠀⠀⠀⠀⣀⣀⣾⣿⣿⣄⣠⣏⠀⠀⠀⠺⣯⣿⣆⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⣸⡇⠀⣿⠀⠀⠀⠀⠀⠀⠀⠀⠸⣧⠘⣷⣤⢹⣄⢻⣿⡇⠀⠀⠀⠀⠀⠀⠀⠀⠙⠛⠻⣯⣤⣴⣦⣾⠷⣿⡋⠀⠀⠈⠉⢹⣿⣦⣿⠛⢷⣬⣿⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⣿⡇⠀⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠸⣧⡏⠙⢿⠟⠟⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⢻⣯⡀⠀⠀⠈⢿⡷⣦⡀⠀ ⠀  ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⢻⡇⢠⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀ ⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢹⡏⠀⣇⠘⡆⢳⣬⣿⡄⠀⠀⠀⠀⠀⠀⠀
⠀⠀⣸⡇⢸⠇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠘⣷⡾⢻⣶⠿⣶⡏⠉⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⣿⠃⡾⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀  ⠀ ⠀  ⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⣼⣏⡼⠃⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⣼⣿⠞⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
"#;

fn main() {
    let mut app = App::new();

    app.add_plugins((
        PlurimusPlugin,
        MinimalPlugins,
        RatatuiPlugins {
            enable_kitty_protocol: true,
            enable_input_forwarding: true,
            enable_mouse_capture: true,
        },
    ));

    app.add_systems(Startup, (startup_ui, startup_fx).chain());
    app.add_systems(Update, tick_status);

    app.run();
}

fn startup_ui(mut commands: Commands) -> Result {
    use ratatui::{
        prelude::{Color, Constraint, Direction, Layout, Rect, Style},
        widgets::Paragraph,
    };

    let layout = |area: Rect| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        (chunks[0], chunks[1])
    };

    commands.spawn((
        PreviewCanvas,
        Widget::from_render_fn(move |frame, area| {
            let p = centered_ascii_paragraph(area, PREVIEW_ART);
            frame.render_widget(p, area);
            Ok(())
        }),
        WidgetLayout::new(move |area| layout(*area).0),
        WidgetOrder(0),
        TachyonEffect,
    ));

    commands.spawn((
        StatusBar,
        Widget::from_widget(
            Paragraph::new("  ⌁  ctrl+c to quit   ·   tachyonfx per-widget effects   ·   bevy_ratatui_widgets  ")
                .style(Style::default().fg(Color::DarkGray)),
        ),
        WidgetLayout::new(move |area| layout(*area).1),
        WidgetOrder(1),
    ));

    Ok(())
}

fn startup_fx(
    mut commands: Commands,
    mut reg: NonSendMut<TachyonRegistry>,
    q_preview: Query<Entity, With<PreviewCanvas>>,
    q_status: Query<Entity, With<StatusBar>>,
) {
    use ratatui::prelude::Color;
    use tachyonfx::{
        Interpolation, Motion, fx,
        pattern::{DiagonalPattern, RadialPattern},
    };

    let Ok(preview) = q_preview.single() else {
        return;
    };
    let Ok(status) = q_status.single() else {
        return;
    };

    enable_fx(&mut commands, &mut reg, preview);
    enable_fx(&mut commands, &mut reg, status);

    add_fx(
        &mut reg,
        preview,
        fx::sequence(&[
            fx::slide_in(
                Motion::LeftToRight,
                8,
                0,
                Color::Black,
                (700, Interpolation::CubicOut),
            ),
            fx::repeating(fx::ping_pong(
                fx::dissolve(1000).with_pattern(RadialPattern::center().with_transition_width(4.0)),
            )),
        ]),
    );

    add_fx(
        &mut reg,
        status,
        fx::repeating(fx::ping_pong(
            fx::fade_to_fg(Color::Cyan, 900).with_pattern(
                DiagonalPattern::top_left_to_bottom_right().with_transition_width(2.0),
            ),
        )),
    );
}

fn centered_ascii_paragraph(
    area: ratatui::prelude::Rect,
    art: &str,
) -> ratatui::widgets::Paragraph<'static> {
    use ratatui::{
        prelude::{Alignment, Color, Line, Modifier, Style, Text},
        widgets::{Block, Borders, Paragraph, Wrap},
    };

    let mut art_lines: Vec<&str> = art.lines().collect();
    while art_lines.last().is_some_and(|l| l.trim().is_empty()) {
        art_lines.pop();
    }
    while art_lines.first().is_some_and(|l| l.trim().is_empty()) {
        art_lines.remove(0);
    }

    let art_h = art_lines.len() as i16;
    let inner_h = area.height as i16;

    let pad_top = ((inner_h - art_h).max(0) / 2) as usize;

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.extend(std::iter::repeat_n(Line::from(""), pad_top));
    lines.extend(art_lines.into_iter().map(|s| Line::from(s.to_string())));

    Paragraph::new(Text::from(lines))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::Gray).add_modifier(Modifier::DIM))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" tachyonfx ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
}

fn tick_status(mut q: Query<&mut Widget, With<StatusBar>>) {
    use ratatui::{
        prelude::{Alignment, Color, Style},
        widgets::Paragraph,
    };

    for mut w in &mut q {
        w.set_widget(
            Paragraph::new("  ⌁  ctrl+c to quit   ·   tachyonfx effects demo   ⌁  ".to_string())
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray)),
        );
    }
}
