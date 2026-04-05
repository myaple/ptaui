use crate::app::{App, BreakdownPeriod, ReportsView};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use rust_decimal::prelude::ToPrimitive;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    match app.reports_view {
        ReportsView::Monthly => render_monthly(f, app, area),
        ReportsView::Breakdown => render_breakdown(f, app, area),
    }
}

// ── Monthly view (existing) ───────────────────────────────────────────────────

fn render_monthly(f: &mut Frame, app: &App, area: Rect) {
    let filter = app.active_account_filter();
    let summary = app.ledger.monthly_summary(&app.config.currency, filter.as_ref());

    let filter_hint = {
        let total = app.account_filter.len();
        let checked = app.account_filter.iter().filter(|(_, c)| *c).count();
        if total == 0 || checked == total {
            " c filter ".to_string()
        } else {
            format!(" c filter ({}/{}) ", checked, total)
        }
    };

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
            Line::from(""),
            Line::from(Span::styled(
                "  Press 'c' to open the account filter.  Press Tab to switch to breakdown view.",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Reports — Monthly Chart  Tab→breakdown |{}",
            filter_hint
        )));
        f.render_widget(para, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    render_bar_chart(f, app, &summary, &filter_hint, chunks[0]);
    render_summary_table(f, &summary, chunks[1]);
}

fn render_bar_chart(
    f: &mut Frame,
    app: &App,
    summary: &[(String, rust_decimal::Decimal, rust_decimal::Decimal)],
    filter_hint: &str,
    area: Rect,
) {
    let recent: Vec<_> = summary.iter().rev().take(12).rev().collect();

    let mut chart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " Monthly Income vs Expenses ({}) — green=income  red=expenses  Tab→breakdown |{}",
                    app.config.currency, filter_hint
                )),
        )
        .bar_width(4)
        .bar_gap(1)
        .group_gap(2);

    for (month, income, expenses) in &recent {
        let income_val = income.to_f64().unwrap_or(0.0).max(0.0) as u64;
        let expense_val = expenses.to_f64().unwrap_or(0.0).max(0.0) as u64;
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

// ── Breakdown view ────────────────────────────────────────────────────────────

const BREAKDOWN_PAGE: usize = 20;

pub fn render_breakdown(f: &mut Frame, app: &App, area: Rect) {
    let filter = app.active_account_filter();
    let period = &app.breakdown_period;
    let breakdown = app.ledger.category_breakdown(
        &app.config.currency,
        period.start(),
        period.end(),
        filter.as_ref(),
    );

    let filter_hint = {
        let total = app.account_filter.len();
        let checked = app.account_filter.iter().filter(|(_, c)| *c).count();
        if total == 0 || checked == total {
            String::new()
        } else {
            format!("  c filter ({}/{})", checked, total)
        }
    };

    let mode_label = if period.is_month() { "month" } else { "year" };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // period selector
            Constraint::Min(0),    // category list
            Constraint::Length(3), // help bar
        ])
        .split(area);

    // Period selector
    render_period_selector(f, period, &filter_hint, mode_label, chunks[0]);

    // Category list
    render_category_list(f, app, &breakdown, chunks[1]);

    // Help bar
    render_breakdown_help(f, chunks[2]);
}

fn render_period_selector(
    f: &mut Frame,
    period: &BreakdownPeriod,
    filter_hint: &str,
    mode_label: &str,
    area: Rect,
) {
    let label = period.label();
    let content = Line::from(vec![
        Span::styled(
            "  ◀ h/←  ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!(" {} ", label),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  l/→ ▶  ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("  [{}]", mode_label),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(
            "  m month  y year",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(filter_hint, Style::default().fg(Color::Yellow)),
    ]);

    let para = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Category Breakdown — Tab→monthly chart  c filter "),
    );
    f.render_widget(para, area);
}

fn render_category_list(
    f: &mut Frame,
    app: &App,
    breakdown: &[(String, rust_decimal::Decimal)],
    area: Rect,
) {
    let cursor = app.breakdown_cursor;
    let scroll = app.breakdown_scroll;

    if breakdown.is_empty() {
        let para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No income or expense transactions in this period.",
                Style::default().fg(Color::Yellow),
            )),
        ])
        .block(Block::default().borders(Borders::ALL).title(" Categories "));
        f.render_widget(para, area);
        return;
    }

    let total = breakdown.len();
    let visible_end = (scroll + BREAKDOWN_PAGE).min(total);
    let visible = &breakdown[scroll..visible_end];

    // Calculate column width for amounts
    let max_amount_width = visible
        .iter()
        .map(|(_, amt)| crate::ui::dashboard::fmt_decimal(*amt).len())
        .max()
        .unwrap_or(8)
        .max(8);

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(rel_idx, (category, amount))| {
            let abs_idx = scroll + rel_idx;
            let is_cursor = abs_idx == cursor;

            let is_income = category.starts_with("Income");
            let amount_color = if is_income { Color::Green } else { Color::Red };

            let cursor_indicator = if is_cursor { " ▶ " } else { "   " };

            let name_style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_income {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            let amt_style = if is_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(amount_color)
            };

            let amt_str = crate::ui::dashboard::fmt_decimal(*amount);
            // Pad category to fill available space (rough calculation)
            let line = Line::from(vec![
                Span::styled(cursor_indicator, name_style),
                Span::styled(
                    format!("{:<45}", category),
                    name_style,
                ),
                Span::styled(
                    format!(" {:>width$}", amt_str, width = max_amount_width),
                    amt_style,
                ),
                Span::styled(
                    if is_cursor { "  Enter to inspect" } else { "" },
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let scroll_hint = if total > BREAKDOWN_PAGE {
        format!("  ↑↓ {}-{}/{}", scroll + 1, visible_end, total)
    } else {
        String::new()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            format!(" Categories{} ", scroll_hint),
            Style::default().fg(Color::White),
        ));

    f.render_widget(List::new(items).block(block), area);
}

fn render_breakdown_help(f: &mut Frame, area: Rect) {
    let spans = vec![
        Span::styled(" j/k ↑↓", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" navigate  "),
        Span::styled("h/l ←→", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" prev/next period  "),
        Span::styled("m", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" month  "),
        Span::styled("y", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" year  "),
        Span::styled("Enter", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" inspect transactions  "),
        Span::styled("Tab", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" monthly chart  "),
        Span::styled("c", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" filter"),
    ];
    let para = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(para, area);
}
