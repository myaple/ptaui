pub mod add_account;
pub mod add_tx;
pub mod dashboard;
pub mod reports;
pub mod startup;
pub mod transactions;

use crate::app::{App, Modal, Screen};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    // Startup is full-screen — nothing else renders
    if app.screen == Screen::Startup {
        startup::render(f, app);
        return;
    }

    let size = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // main content
            Constraint::Length(3), // status bar
        ])
        .split(size);

    render_tabs(f, app, chunks[0]);

    match app.screen {
        Screen::Dashboard => dashboard::render(f, app, chunks[1]),
        Screen::Transactions => transactions::render(f, app, chunks[1]),
        Screen::Reports => reports::render(f, app, chunks[1]),
        Screen::Startup => unreachable!(),
    }

    render_status(f, app, chunks[2]);

    // Modal overlays — rendered after the background so they appear on top
    match &app.modal {
        Some(Modal::AddTransaction) => add_tx::render_modal(f, app),
        Some(Modal::AddAccount) => add_account::render_modal(f, app),
        None => {}
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["[1] Accounts", "[2] Transactions", "[3] Reports"];
    let selected = match app.screen {
        Screen::Dashboard => 0,
        Screen::Transactions => 1,
        Screen::Reports => 2,
        Screen::Startup => 0,
    };
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ptaui — Plain Text Accounting "),
        )
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let msg = if !app.check_errors.is_empty() {
        let first = app.check_errors.first().map(|s| s.as_str()).unwrap_or("");
        format!("bean-check error: {}", first)
    } else if let Some(ref s) = app.status_message {
        s.clone()
    } else {
        let file = app.config.resolved_beancount_file();
        format!(
            " {} | q quit  1-3 screens  a add  r reload",
            file.display()
        )
    };

    let style = if !app.check_errors.is_empty() {
        Style::default().fg(Color::Red)
    } else if app.status_message.is_some() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let para = Paragraph::new(msg)
        .block(Block::default().borders(Borders::ALL))
        .style(style);
    f.render_widget(para, area);
}

/// Return a centered Rect, `width` × `height`, within `area`.
pub fn centered_modal(width: u16, height: u16, area: Rect) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

/// Render a semi-transparent dimming overlay behind a modal.
pub fn render_dim(f: &mut Frame) {
    let area = f.area();
    // Ratatui doesn't have true transparency, but rendering a dark block
    // over the whole screen gives a clear modal separation.
    let dim = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(dim, area);
    // Then clear just the modal region so content shows through — callers
    // do this themselves after calling centered_modal.
}
