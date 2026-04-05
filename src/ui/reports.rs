use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use rust_decimal::prelude::ToPrimitive;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let summary = app.ledger.monthly_summary(&app.config.currency);

    if summary.is_empty() {
        let para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No income or expense transactions found.",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Transactions using Income:* or Expenses:* accounts will appear here.",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(Block::default().borders(Borders::ALL).title(" Reports "));
        f.render_widget(para, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    render_bar_chart(f, app, &summary, chunks[0]);
    render_summary_table(f, &summary, chunks[1]);
}

fn render_bar_chart(
    f: &mut Frame,
    app: &App,
    summary: &[(String, rust_decimal::Decimal, rust_decimal::Decimal)],
    area: Rect,
) {
    // Show last 12 months
    let recent: Vec<_> = summary.iter().rev().take(12).rev().collect();

    // Build one BarGroup per month, each with an Income bar and an Expenses bar
    let mut chart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " Monthly Income vs Expenses ({}) — green=income  red=expenses ",
                    app.config.currency
                )),
        )
        .bar_width(4)
        .bar_gap(1)
        .group_gap(2);

    for (month, income, expenses) in &recent {
        let income_val = income.to_f64().unwrap_or(0.0).max(0.0) as u64;
        let expense_val = expenses.to_f64().unwrap_or(0.0).max(0.0) as u64;
        // Use the two-digit month as the group label (e.g. "01")
        let label = month.get(5..7).unwrap_or(month.as_str()).to_string();

        let group = BarGroup::default()
            .label(Line::from(label))
            .bars(&[
                Bar::default()
                    .value(income_val)
                    .style(Style::default().fg(Color::Green))
                    .value_style(Style::default().fg(Color::Black).bg(Color::Green)),
                Bar::default()
                    .value(expense_val)
                    .style(Style::default().fg(Color::Red))
                    .value_style(Style::default().fg(Color::Black).bg(Color::Red)),
            ]);
        chart = chart.data(group);
    }

    f.render_widget(chart, area);
}

fn render_summary_table(
    f: &mut Frame,
    summary: &[(String, rust_decimal::Decimal, rust_decimal::Decimal)],
    area: Rect,
) {
    let recent: Vec<_> = summary.iter().rev().take(6).rev().collect();

    let header = ListItem::new(Line::from(Span::styled(
        format!(" {:<10} {:>15} {:>15} {:>15}", "Month", "Income", "Expenses", "Net"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));

    let mut items = vec![header];
    for (month, income, expenses) in &recent {
        let net = income - expenses;
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!(" {:<10}", month), Style::default().fg(Color::White)),
            Span::styled(
                format!(" {:>15}", crate::ui::dashboard::fmt_decimal(*income)),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                format!(" {:>15}", crate::ui::dashboard::fmt_decimal(*expenses)),
                Style::default().fg(Color::Red),
            ),
            Span::styled(
                format!(" {:>15}", crate::ui::dashboard::fmt_decimal(net)),
                Style::default()
                    .fg(if net >= rust_decimal::Decimal::ZERO { Color::Green } else { Color::Red })
                    .add_modifier(Modifier::BOLD),
            ),
        ])));
    }

    f.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Monthly Summary (last 6 months) "),
        ),
        area,
    );
}
