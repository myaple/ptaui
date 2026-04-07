use crate::app::App;
use crate::ui::{centered_modal, render_dim};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

const LIST_HEIGHT: usize = 18;

pub fn render_modal(f: &mut Frame, app: &App) {
    render_dim(f);

    let modal_height = (LIST_HEIGHT + 8) as u16;
    let area = centered_modal(92, modal_height, f.area());
    f.render_widget(Clear, area);

    let outer_block = Block::default().borders(Borders::ALL);
    let inner = outer_block.inner(area);
    f.render_widget(outer_block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(inner);

    render_tx_list(f, app, chunks[0]);
    render_help(f, chunks[1]);
}

fn render_tx_list(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let category = &app.category_tx_category;
    let period = &app.breakdown_period;
    let cursor = app.category_tx_cursor;
    let scroll = app.category_tx_scroll;

    let transactions = app.ledger.transactions_for_category(
        &app.config.currency,
        period.start(),
        period.end(),
        category,
    );

    let total = transactions.len();
    let visible_end = (scroll + LIST_HEIGHT).min(total);

    let title = format!(
        " {} — {} ({}) ",
        category,
        period.label(),
        app.config.currency,
    );

    let header = ListItem::new(Line::from(vec![Span::styled(
        format!(
            "   {:<12}  {:<35}  {:>12}",
            "Date", "Payee / Narration", "Amount"
        ),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )]));

    let mut items = vec![header];

    if transactions.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "   No transactions found.",
            Style::default().fg(Color::Yellow),
        ))));
    } else {
        let visible = &transactions[scroll..visible_end];
        for (rel_idx, txn) in visible.iter().enumerate() {
            let abs_idx = scroll + rel_idx;
            let is_cursor = abs_idx == cursor;

            // Find the amount from the category posting
            let amount = txn.postings.iter().find_map(|p| {
                if p.account == *category
                    && p.currency.as_deref().unwrap_or("") == app.config.currency
                {
                    p.amount
                } else {
                    None
                }
            });

            let display_amount = if let Some(amt) = amount {
                // Expenses: positive, Income: negate beancount's negative
                if category.starts_with("Income") {
                    crate::ui::dashboard::fmt_decimal(-amt)
                } else {
                    crate::ui::dashboard::fmt_decimal(amt)
                }
            } else {
                "-".to_string()
            };

            let is_income = category.starts_with("Income");
            let amount_color = if is_income { Color::Green } else { Color::Red };

            let payee_narration = match (&txn.payee, txn.narration.as_str()) {
                (Some(p), n) if !n.is_empty() => format!("{} / {}", p, n),
                (Some(p), _) => p.clone(),
                (None, n) => n.to_string(),
            };
            // Truncate to fit column
            let payee_display = if payee_narration.len() > 35 {
                format!("{}…", &payee_narration[..34])
            } else {
                payee_narration
            };

            let row_style = if is_cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let amt_style = if is_cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(amount_color)
            };

            let cursor_indicator = if is_cursor { " ▶ " } else { "   " };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(cursor_indicator, row_style),
                Span::styled(
                    format!("{:<12}  {:<35}  ", txn.date, payee_display),
                    row_style,
                ),
                Span::styled(format!("{:>12}", display_amount), amt_style),
            ])));
        }
    }

    let scroll_hint = if total > LIST_HEIGHT {
        format!("  ↑↓ {}-{}/{}", scroll + 1, visible_end, total)
    } else {
        String::new()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            format!("{}{}", title, scroll_hint),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    f.render_widget(List::new(items).block(block), area);
}

fn render_help(f: &mut Frame, area: ratatui::layout::Rect) {
    let spans = vec![
        Span::styled(
            " j/k ↑↓",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" navigate  "),
        Span::styled(
            "Esc/Enter",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" close"),
    ];
    let para = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(para, area);
}
