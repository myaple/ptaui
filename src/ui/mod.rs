pub mod add_account;
pub mod add_tx;
pub mod dashboard;
pub mod reports;
pub mod startup;
pub mod transactions;

use crate::app::{App, Screen};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    // Startup screen takes the whole frame — no tab bar
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
        Screen::AddTransaction => add_tx::render(f, app, chunks[1]),
        Screen::AddAccount => add_account::render(f, app, chunks[1]),
        Screen::Reports => reports::render(f, app, chunks[1]),
        Screen::Startup => unreachable!(),
    }

    render_status(f, app, chunks[2]);
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["[1] Accounts", "[2] Transactions", "[3] Add", "[4] Reports"];
    let selected = match app.screen {
        Screen::Dashboard => 0,
        Screen::Transactions => 1,
        Screen::AddTransaction | Screen::AddAccount => 2,
        Screen::Reports => 3,
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
        format!(" {} | q: quit  1-4: screens  r: reload", file.display())
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
