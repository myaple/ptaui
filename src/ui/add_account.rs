use crate::app::{AddAccountField, App, ACCOUNT_TYPES};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    if app.add_account_form.is_none() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    render_form(f, app, chunks[0]);
    render_help(f, app, chunks[1]);
}

fn render_form(f: &mut Frame, app: &App, area: Rect) {
    let form = app.add_account_form.as_ref().unwrap();

    let mut lines: Vec<Line> = vec![Line::from(""), Line::from("")];

    // ── Account Type selector ────────────────────────────────────────────────
    let type_focused = form.focused == AddAccountField::AccountType;
    lines.push(Line::from(Span::styled(
        "  Account Type",
        if type_focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    )));
    lines.push(Line::from(""));

    // Render each type as a selectable chip
    let mut chips: Vec<Span> = vec![Span::raw("  ")];
    for (i, t) in ACCOUNT_TYPES.iter().enumerate() {
        let selected = i == form.type_idx;
        let focused = type_focused;
        let style = match (selected, focused) {
            (true, true) => Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            (true, false) => Style::default()
                .fg(Color::Black)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            (false, _) => Style::default().fg(Color::DarkGray),
        };
        chips.push(Span::styled(format!(" {} ", t), style));
        chips.push(Span::raw("  "));
    }
    lines.push(Line::from(chips));
    lines.push(Line::from(""));

    // ── Sub-name ─────────────────────────────────────────────────────────────
    let sub_focused = form.focused == AddAccountField::SubName;
    let cursor = if sub_focused { "█" } else { " " };
    lines.push(Line::from(vec![
        Span::styled(
            "  Sub-name   : ",
            if sub_focused {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
        Span::styled(
            format!("{}{}", form.sub_name, cursor),
            Style::default().fg(Color::White),
        ),
    ]));
    // Live preview of full account name
    lines.push(Line::from(vec![
        Span::styled("               → ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            form.account_name(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    // ── Currencies ───────────────────────────────────────────────────────────
    let cur_focused = form.focused == AddAccountField::Currencies;
    let cursor = if cur_focused { "█" } else { " " };
    lines.push(Line::from(vec![
        Span::styled(
            "  Currencies : ",
            if cur_focused {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
        Span::styled(
            format!("{}{}", form.currencies, cursor),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        "               (space-separated, e.g.  USD  EUR)",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    // ── Date ─────────────────────────────────────────────────────────────────
    let date_focused = form.focused == AddAccountField::Date;
    let cursor = if date_focused { "█" } else { " " };
    lines.push(Line::from(vec![
        Span::styled(
            "  Open date  : ",
            if date_focused {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
        Span::styled(
            format!("{}{}", form.date, cursor),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(""));

    // ── Confirm ───────────────────────────────────────────────────────────────
    let confirm_focused = form.focused == AddAccountField::Confirm;
    lines.push(Line::from(vec![Span::styled(
        if confirm_focused {
            "  ► [ ADD ACCOUNT ] ◄"
        } else {
            "    [ ADD ACCOUNT ]  "
        },
        if confirm_focused {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    )]));

    // ── Error ─────────────────────────────────────────────────────────────────
    if let Some(ref err) = form.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  Error: {}", err),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
    }

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Add Account "));
    f.render_widget(para, area);
}

fn render_help(f: &mut Frame, _app: &App, area: Rect) {
    let help_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Navigation",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("  Tab / ↓   Next field", Style::default().fg(Color::White))),
        Line::from(Span::styled("  Shift+Tab Previous field", Style::default().fg(Color::White))),
        Line::from(Span::styled("  ← / →     Change account type", Style::default().fg(Color::White))),
        Line::from(Span::styled("  Enter     Confirm / Submit", Style::default().fg(Color::White))),
        Line::from(Span::styled("  Esc       Cancel", Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(Span::styled(
            "  Account naming",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("  Each segment: CapitalFirst", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  Use : for sub-accounts", Style::default().fg(Color::DarkGray))),
        Line::from(""),
        Line::from(Span::styled(
            "  Examples",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("  Checking", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  → Assets:Checking", Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(Span::styled("  Food:Restaurants", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  → Expenses:Food:Restaurants", Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(Span::styled("  Visa", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  → Liabilities:Visa", Style::default().fg(Color::White))),
    ];

    let para = Paragraph::new(help_lines)
        .block(Block::default().borders(Borders::ALL).title(" Help "));
    f.render_widget(para, area);
}
