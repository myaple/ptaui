use crate::app::App;
use crate::ui::{centered_modal, render_dim};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_modal(f: &mut Frame, app: &App) {
    render_dim(f);

    let area = centered_modal(46, 9, f.area());
    f.render_widget(Clear, area);

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(Span::styled(
            " Delete Transaction ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    let inner = outer_block.inner(area);
    f.render_widget(outer_block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // message
            Constraint::Length(1), // buttons
        ])
        .split(inner);

    // Message
    let msg = Paragraph::new(Line::from(vec![Span::styled(
        "  Delete this transaction?",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )]));
    f.render_widget(msg, chunks[0]);

    // Yes / No buttons
    let yes_style = if app.delete_tx_confirm {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let no_style = if !app.delete_tx_confirm {
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let buttons = Paragraph::new(Line::from(vec![
        Span::raw("  "),
        Span::styled("  Yes  ", yes_style),
        Span::raw("   "),
        Span::styled("  No  ", no_style),
    ]));
    f.render_widget(buttons, chunks[1]);
}
