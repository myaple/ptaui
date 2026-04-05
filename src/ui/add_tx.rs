use crate::app::{AddTxField, App};
use crate::ui::{centered_modal, render_dim};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

pub fn render_modal(f: &mut Frame, app: &App) {
    if app.add_tx_form.is_none() {
        return;
    }

    render_dim(f);

    let area = centered_modal(100, 30, f.area());
    f.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    render_form(f, app, chunks[0]);
    render_suggestions_and_help(f, app, chunks[1]);
}

fn label_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn field_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    }
}

fn render_form(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let form = app.add_tx_form.as_ref().unwrap();

    let fields: &[(AddTxField, &str, &str)] = &[
        (AddTxField::Date,      "Date       ", &form.date),
        (AddTxField::Payee,     "Payee      ", &form.payee),
        (AddTxField::Narration, "Narration  ", &form.narration),
        (AddTxField::Category,  "Category   ", &form.category),
        (AddTxField::Account,   "Account    ", &form.account),
        (AddTxField::Amount,    "Amount     ", &form.amount),
        (AddTxField::Currency,  "Currency   ", &form.currency),
    ];

    let mut lines: Vec<Line> = vec![Line::from("")];

    for (field, label, value) in fields {
        let focused = form.focused == *field;
        let cursor = if focused { "█" } else { " " };
        lines.push(Line::from(vec![
            Span::styled(format!("  {}: ", label), label_style(focused)),
            Span::styled(
                format!("{:<32}{}", value, cursor),
                field_style(focused),
            ),
        ]));
        lines.push(Line::from(""));
    }

    // Confirm button
    let confirm_focused = form.focused == AddTxField::Confirm;
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        if confirm_focused { "  ► [ SAVE TRANSACTION ] ◄" } else { "    [ SAVE TRANSACTION ]  " },
        if confirm_focused {
            Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    )]));

    if let Some(ref err) = form.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  ✗ {}", err),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
    }

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Add Transaction ")),
        area,
    );
}

fn render_suggestions_and_help(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let form = app.add_tx_form.as_ref().unwrap();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // ── Suggestions ──────────────────────────────────────────────────────────
    let suggestions = form.suggestions_for_current();
    let (placeholder, pane_title) = match form.focused {
        AddTxField::Payee =>
            (" Start typing to see payees…", " Suggestions — Payee "),
        AddTxField::Category =>
            (" Start typing to see categories…", " Suggestions — Category "),
        AddTxField::Account =>
            (" Start typing to see accounts…", " Suggestions — Account "),
        _ =>
            (" (no suggestions for this field)", " Suggestions "),
    };

    let items: Vec<ListItem> = if suggestions.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            placeholder,
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        suggestions
            .iter()
            .map(|s| ListItem::new(Line::from(Span::styled(
                s.as_str(),
                Style::default().fg(Color::Cyan),
            ))))
            .collect()
    };

    f.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(pane_title)),
        chunks[0],
    );

    // ── Help ─────────────────────────────────────────────────────────────────
    let help = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled("  Navigation", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("  Tab / ↓    Next field", Style::default().fg(Color::White))),
        Line::from(Span::styled("  Shift+Tab  Prev field", Style::default().fg(Color::White))),
        Line::from(Span::styled("  Enter      Confirm", Style::default().fg(Color::White))),
        Line::from(Span::styled("  Esc        Cancel", Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(Span::styled("  Autocomplete (Tab)", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("  Payee, Category, Account", Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(Span::styled("  Double-entry", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("  Category → Expenses:*", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  Account  → payment src", Style::default().fg(Color::DarkGray))),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Help "));

    f.render_widget(help, chunks[1]);
}
