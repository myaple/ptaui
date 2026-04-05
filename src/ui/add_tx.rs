use crate::app::{AddTxField, App};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    if app.add_tx_form.is_none() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_form(f, app, chunks[0]);
    render_help_and_suggestions(f, app, chunks[1]);
}

fn field_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    }
}

fn label_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn render_form(f: &mut Frame, app: &App, area: Rect) {
    let form = app.add_tx_form.as_ref().unwrap();

    let fields = [
        (AddTxField::Date, "Date       ", &form.date),
        (AddTxField::Payee, "Payee      ", &form.payee),
        (AddTxField::Narration, "Narration  ", &form.narration),
        (AddTxField::FromAccount, "From Acct  ", &form.from_account),
        (AddTxField::ToAccount, "To Acct    ", &form.to_account),
        (AddTxField::Amount, "Amount     ", &form.amount),
        (AddTxField::Currency, "Currency   ", &form.currency),
    ];

    let mut lines: Vec<Line> = vec![Line::from("")];

    for (field, label, value) in &fields {
        let focused = form.focused == *field;
        let cursor = if focused { "█" } else { " " };
        let value_display = format!("{}{}", value, cursor);
        lines.push(Line::from(vec![
            Span::styled(format!("  {}: ", label), label_style(focused)),
            Span::styled(
                format!("{:<35}", value_display),
                field_style(focused),
            ),
        ]));
        lines.push(Line::from(""));
    }

    // Confirm button
    let confirm_focused = form.focused == AddTxField::Confirm;
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        if confirm_focused {
            "  ► [ SAVE TRANSACTION ] ◄"
        } else {
            "    [ SAVE TRANSACTION ]  "
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

    // Error message
    if let Some(ref err) = form.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  Error: {}", err),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
    }

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Add Transaction "));
    f.render_widget(para, area);
}

fn render_help_and_suggestions(f: &mut Frame, app: &App, area: Rect) {
    let form = app.add_tx_form.as_ref().unwrap();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Suggestions
    let suggestions = form.suggestions_for_current();
    let suggestion_items: Vec<ListItem> = suggestions
        .iter()
        .map(|s| ListItem::new(Line::from(Span::styled(s.as_str(), Style::default().fg(Color::Cyan)))))
        .collect();

    let suggestions_widget = if suggestion_items.is_empty() {
        List::new(vec![ListItem::new(Line::from(Span::styled(
            " Start typing to see accounts...",
            Style::default().fg(Color::DarkGray),
        )))])
    } else {
        List::new(suggestion_items)
    };

    f.render_widget(
        suggestions_widget.block(Block::default().borders(Borders::ALL).title(" Account Suggestions ")),
        chunks[0],
    );

    // Help
    let help_lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Navigation", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("  Tab / ↓   Next field", Style::default().fg(Color::White))),
        Line::from(Span::styled("  Shift+Tab Previous field", Style::default().fg(Color::White))),
        Line::from(Span::styled("  Enter     Confirm/Submit", Style::default().fg(Color::White))),
        Line::from(Span::styled("  Esc       Cancel", Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(Span::styled("  Accounts", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("  Tab       Autocomplete", Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(Span::styled("  Double-entry", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("  From → debit source", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  To   → credit target", Style::default().fg(Color::DarkGray))),
        Line::from(""),
        Line::from(Span::styled("  Example:", Style::default().fg(Color::Yellow))),
        Line::from(Span::styled("  From: Assets:Checking", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled("  To:   Expenses:Food", Style::default().fg(Color::DarkGray))),
    ];

    let help = Paragraph::new(help_lines)
        .block(Block::default().borders(Borders::ALL).title(" Help "));
    f.render_widget(help, chunks[1]);
}
