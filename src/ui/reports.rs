use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BarChart, Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use rust_decimal::prelude::ToPrimitive;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let summary = app.ledger.monthly_summary(&app.config.currency);

    if summary.is_empty() {
        let para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No transaction data available for reports.",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(Span::styled(
                "  Add transactions to see spending and income trends.",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(Block::default().borders(Borders::ALL).title(" Reports "));
        f.render_widget(para, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    render_bar_chart(f, app, &summary, chunks[0]);
    render_summary_table(f, app, &summary, chunks[1]);
}

fn render_bar_chart(
    f: &mut Frame,
    app: &App,
    summary: &[(String, rust_decimal::Decimal, rust_decimal::Decimal)],
    area: Rect,
) {
    // Show last 12 months
    let recent: Vec<_> = summary.iter().rev().take(12).rev().collect();

    // Build bar data: income and expenses interleaved
    let bar_data: Vec<(&str, u64)> = recent
        .iter()
        .flat_map(|(month, income, expenses)| {
            let income_cents = income.to_f64().unwrap_or(0.0).max(0.0) as u64;
            let expense_cents = expenses.to_f64().unwrap_or(0.0).max(0.0) as u64;
            let month_short = month.get(5..7).unwrap_or(month.as_str());
            // We'll encode month as a static str via leak — acceptable for TUI
            vec![
                (Box::leak(format!("{}I", month_short).into_boxed_str()) as &str, income_cents),
                (Box::leak(format!("{}E", month_short).into_boxed_str()) as &str, expense_cents),
            ]
        })
        .collect();

    let bar_chart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Monthly Income vs Expenses ({}) ", app.config.currency)),
        )
        .data(&bar_data)
        .bar_width(3)
        .bar_gap(1)
        .bar_style(Style::default().fg(Color::Cyan))
        .value_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .label_style(Style::default().fg(Color::DarkGray));

    f.render_widget(bar_chart, area);
}

fn render_summary_table(
    f: &mut Frame,
    _app: &App,
    summary: &[(String, rust_decimal::Decimal, rust_decimal::Decimal)],
    area: Rect,
) {
    let recent: Vec<_> = summary.iter().rev().take(6).rev().collect();

    let mut items: Vec<ListItem> = vec![ListItem::new(Line::from(vec![
        Span::styled(
            format!(" {:<10} {:>15} {:>15} {:>15}", "Month", "Income", "Expenses", "Net"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
    ]))];

    for (month, income, expenses) in &recent {
        let net = income - expenses;
        let net_color = if net >= rust_decimal::Decimal::ZERO {
            Color::Green
        } else {
            Color::Red
        };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                format!(" {:<10}", month),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!(" {:>15}", crate::ui::dashboard::fmt_decimal(*income)),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                format!(" {:>15}", crate::ui::dashboard::fmt_decimal(*expenses)),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!(" {:>15}", crate::ui::dashboard::fmt_decimal(net)),
                Style::default().fg(net_color).add_modifier(Modifier::BOLD),
            ),
        ])));
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Monthly Summary (last 6 months) "),
    );
    f.render_widget(list, area);
}
