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
        ReportsView::NetWorth => render_net_worth(f, app, area),
    }
}

// ── Monthly view (existing) ───────────────────────────────────────────────────

fn render_monthly(f: &mut Frame, app: &App, area: Rect) {
    let filter = app.active_account_filter();
    let summary = app
        .ledger
        .monthly_summary(&app.config.currency, filter.as_ref());

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
            " Reports — Monthly Chart  Tab→next |{}",
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
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Monthly Income vs Expenses ({}) — green=income  red=expenses  Tab→next |{}",
            app.config.currency, filter_hint
        )))
        .bar_width(4)
        .bar_gap(1)
        .group_gap(2);

    for (month, income, expenses) in &recent {
        let income_val = income.to_f64().unwrap_or(0.0).max(0.0) as u64;
        let expense_val = expenses.to_f64().unwrap_or(0.0).max(0.0) as u64;
        let label = month.get(5..7).unwrap_or(month.as_str()).to_string();

        let group = BarGroup::default().label(Line::from(label)).bars(&[
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
        format!(
            " {:<10} {:>15} {:>15} {:>15} {:>10}",
            "Month", "Income", "Expenses", "Net", "Save %"
        ),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));

    let mut items = vec![header];
    for (month, income, expenses) in &recent {
        let net = income - expenses;
        let savings_rate = if *income > rust_decimal::Decimal::ZERO {
            (net / income * rust_decimal::Decimal::ONE_HUNDRED)
                .round_dp(1)
                .to_f64()
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let savings_str = format!("{:+.1}%", savings_rate);
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
                    .fg(if net >= rust_decimal::Decimal::ZERO {
                        Color::Green
                    } else {
                        Color::Red
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {:>10}", savings_str),
                Style::default()
                    .fg(if savings_rate >= 0.0 {
                        Color::Green
                    } else {
                        Color::Red
                    })
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
    render_period_selector(
        f,
        period,
        &filter_hint,
        mode_label,
        " Category Breakdown — Tab→next  c filter ",
        chunks[0],
    );

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
    title: &str,
    area: Rect,
) {
    let label = period.label();
    let content = Line::from(vec![
        Span::styled("  ◀ h/←  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(" {} ", label),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  l/→ ▶  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("  [{}]", mode_label),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("  m month  y year", Style::default().fg(Color::DarkGray)),
        Span::styled(filter_hint, Style::default().fg(Color::Yellow)),
    ]);

    let para = Paragraph::new(content).block(Block::default().borders(Borders::ALL).title(title));
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
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_income {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            let amt_style = if is_cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(amount_color)
            };

            let amt_str = crate::ui::dashboard::fmt_decimal(*amount);
            // Pad category to fill available space (rough calculation)
            let line = Line::from(vec![
                Span::styled(cursor_indicator, name_style),
                Span::styled(format!("{:<45}", category), name_style),
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

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        format!(" Categories{} ", scroll_hint),
        Style::default().fg(Color::White),
    ));

    f.render_widget(List::new(items).block(block), area);
}

fn render_breakdown_help(f: &mut Frame, area: Rect) {
    let spans = vec![
        Span::styled(
            " j/k ↑↓",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" navigate  "),
        Span::styled(
            "h/l ←→",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" prev/next period  "),
        Span::styled(
            "m",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" month  "),
        Span::styled(
            "y",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" year  "),
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" inspect transactions  "),
        Span::styled(
            "Tab",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" monthly chart  "),
        Span::styled(
            "c",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" filter"),
    ];
    let para = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(para, area);
}

// ── Net Worth view ───────────────────────────────────────────────────────────

const NETWORTH_PAGE: usize = 16;

fn render_net_worth(f: &mut Frame, app: &App, area: Rect) {
    let history = app.ledger.net_worth_history(&app.config.currency);
    let period = &app.breakdown_period;
    let mode_label = if period.is_month() { "month" } else { "year" };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(42), // bar chart
            Constraint::Length(3),      // period selector
            Constraint::Min(0),         // category list + trend
            Constraint::Length(3),      // help bar
        ])
        .split(area);

    render_net_worth_chart(f, app, &history, chunks[0]);
    render_period_selector(
        f,
        period,
        "",
        mode_label,
        " Net Worth — Tab→next ",
        chunks[1],
    );
    render_net_worth_bottom(f, app, &history, chunks[2]);
    render_net_worth_help(f, chunks[3]);
}

fn render_net_worth_chart(
    f: &mut Frame,
    app: &App,
    history: &[(String, rust_decimal::Decimal)],
    area: Rect,
) {
    if history.is_empty() {
        let para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No asset or liability transactions found.",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Net worth is computed from Assets:* and Liabilities:* postings.",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Net Worth Over Time ({}) — Tab→breakdown ",
            app.config.currency
        )));
        f.render_widget(para, area);
        return;
    }

    // Split: Y-axis scale labels | chart (need chart width to compute stride)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(10), Constraint::Min(0)])
        .split(area);

    // Determine how many bars fit, then thin history to match.
    let bar_width: u16 = 5;
    let bar_gap: u16 = 1;
    let chart_inner = (cols[1].width.saturating_sub(2)) as usize; // -2 for borders
    let max_bars = (chart_inner / (bar_width + bar_gap) as usize).max(1);

    let total = history.len();
    let stride = if max_bars >= total {
        1
    } else {
        total.div_ceil(max_bars)
    };

    // Sample from the end so the most recent data point is always included.
    let mut sampled: Vec<&(String, rust_decimal::Decimal)> = history
        .iter()
        .rev()
        .step_by(stride)
        .take(max_bars)
        .collect();
    sampled.reverse(); // back to chronological order

    let max_abs = sampled
        .iter()
        .map(|(_, v)| v.abs().to_f64().unwrap_or(0.0))
        .fold(0.0_f64, f64::max)
        .max(1.0);

    render_net_worth_y_axis(f, &sampled, cols[0]);

    let mut chart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Net Worth Over Time ({}) — Tab→breakdown ",
            app.config.currency
        )))
        .bar_width(bar_width)
        .bar_gap(bar_gap)
        .max(max_abs as u64);

    for (month, net) in &sampled {
        let bar_val = (net.abs().to_f64().unwrap_or(0.0) as u64).max(1);
        let label = if stride <= 1 {
            month.get(5..7).unwrap_or(month.as_str()).to_string()
        } else if stride >= 12 {
            month.get(0..4).unwrap_or(month.as_str()).to_string()
        } else {
            format!("{}-{}", &month[2..4], &month[5..7])
        };

        let color = if *net >= rust_decimal::Decimal::ZERO {
            Color::Green
        } else {
            Color::Red
        };

        let bar = Bar::default()
            .value(bar_val)
            .style(Style::default().fg(color))
            .value_style(Style::default().fg(Color::Black).bg(color))
            .text_value(String::new());

        chart = chart.data(BarGroup::default().label(Line::from(label)).bars(&[bar]));
    }

    f.render_widget(chart, cols[1]);
}

/// Render Y-axis scale labels aligned with the bar chart's inner area.
///
/// Labels always show absolute values.  The bars being red/green already
/// conveys positive/negative — the axis labels stay neutral.
fn render_net_worth_y_axis(f: &mut Frame, recent: &[&(String, rust_decimal::Decimal)], area: Rect) {
    let max_val = recent
        .iter()
        .map(|(_, v)| *v)
        .max()
        .unwrap_or(rust_decimal::Decimal::ZERO);
    let min_val = recent
        .iter()
        .map(|(_, v)| *v)
        .min()
        .unwrap_or(rust_decimal::Decimal::ZERO);

    // Chart top = the actual value whose |value| is largest (tallest bar).
    let top_actual = if max_val.abs() >= min_val.abs() {
        max_val
    } else {
        min_val
    };
    let mid_actual = top_actual / rust_decimal::Decimal::TWO;

    let h = area.height as usize;
    let mut lines: Vec<Line> = vec![Line::from(""); h];

    let style = Style::default().fg(Color::DarkGray);

    // Row 1 aligns with the chart's inner top (after its top border).
    if h > 4 {
        lines[1] = Line::from(Span::styled(
            format!("{:>9}", fmt_short_amount(top_actual.abs())),
            style,
        ));
    }
    // Middle of chart.
    if h > 8 {
        lines[h / 2] = Line::from(Span::styled(
            format!("{:>9}", fmt_short_amount(mid_actual.abs())),
            style,
        ));
    }
    // Row h-2 aligns with the chart's inner bottom (before its bottom border) = $0.
    if h > 5 {
        lines[h - 2] = Line::from(Span::styled(format!("{:>9}", "0"), style));
    }

    f.render_widget(Paragraph::new(lines), area);
}

fn fmt_short_amount(d: rust_decimal::Decimal) -> String {
    let abs = d.abs();
    let rounded = abs.round_dp(0);
    let v = rounded.to_u64().unwrap_or(0);
    let suffix = if v >= 1_000_000 {
        format!("{:.1}M", v as f64 / 1_000_000.0)
    } else if v >= 1_000 {
        format!("{:.1}k", v as f64 / 1_000.0)
    } else {
        v.to_string()
    };
    if d < rust_decimal::Decimal::ZERO {
        format!("-{}", suffix)
    } else {
        suffix
    }
}

fn render_net_worth_bottom(
    f: &mut Frame,
    app: &App,
    _history: &[(String, rust_decimal::Decimal)],
    area: Rect,
) {
    let filter = app.active_account_filter();
    let period = &app.breakdown_period;
    let breakdown = app.ledger.category_breakdown(
        &app.config.currency,
        period.start(),
        period.end(),
        filter.as_ref(),
    );

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_net_worth_category_list(f, app, &breakdown, chunks[0]);
    render_net_worth_trend(f, app, &breakdown, chunks[1]);
}

fn render_net_worth_category_list(
    f: &mut Frame,
    app: &App,
    breakdown: &[(String, rust_decimal::Decimal)],
    area: Rect,
) {
    let period = &app.breakdown_period;
    let period_label = period.label();
    let cursor = app.networth_cursor;
    let scroll = app.networth_scroll;

    if breakdown.is_empty() {
        let para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No transactions in this period.",
                Style::default().fg(Color::Yellow),
            )),
        ])
        .block(Block::default().borders(Borders::ALL).title(" Categories "));
        f.render_widget(para, area);
        return;
    }

    let total = breakdown.len();
    let visible_end = (scroll + NETWORTH_PAGE).min(total);
    let visible = &breakdown[scroll..visible_end];

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
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_income {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            let amt_style = if is_cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(amount_color)
            };

            let amt_str = crate::ui::dashboard::fmt_decimal(*amount);
            let line = Line::from(vec![
                Span::styled(cursor_indicator, name_style),
                Span::styled(format!("{:<30}", category), name_style),
                Span::styled(
                    format!(" {:>width$}", amt_str, width = max_amount_width),
                    amt_style,
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let scroll_hint = if total > NETWORTH_PAGE {
        format!("  ↑↓ {}-{}/{}", scroll + 1, visible_end, total)
    } else {
        String::new()
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        format!(" {} Categories{} ", period_label, scroll_hint),
        Style::default().fg(Color::White),
    ));

    f.render_widget(List::new(items).block(block), area);
}

fn render_net_worth_trend(
    f: &mut Frame,
    app: &App,
    breakdown: &[(String, rust_decimal::Decimal)],
    area: Rect,
) {
    // Get the category at the cursor position
    let category = breakdown
        .get(app.networth_cursor)
        .map(|(c, _)| c.clone())
        .unwrap_or_default();

    if category.is_empty() {
        let para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Select a category to see its trend.",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Category Trend "),
        );
        f.render_widget(para, area);
        return;
    }

    let trend = app.ledger.category_trend(&app.config.currency, &category);
    let is_income = category.starts_with("Income");

    if trend.is_empty() {
        let para = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  No data for {}", category),
                Style::default().fg(Color::Yellow),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} Trend ", category)),
        );
        f.render_widget(para, area);
        return;
    }

    let recent: Vec<_> = trend.iter().rev().take(12).rev().collect();
    let max_val = recent
        .iter()
        .map(|(_, v)| v.to_f64().unwrap_or(0.0))
        .fold(0.0_f64, |a, b| a.max(b));

    let bar_color = if is_income { Color::Green } else { Color::Red };

    let mut chart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} Trend ", category)),
        )
        .bar_width(4)
        .bar_gap(1)
        .max((max_val as u64).max(1));

    for (month, amount) in &recent {
        let val = amount.to_f64().unwrap_or(0.0) as u64;
        let label = month.get(5..7).unwrap_or(month.as_str()).to_string();

        let bar = Bar::default()
            .value(val.max(1))
            .style(Style::default().fg(bar_color))
            .value_style(Style::default().fg(Color::Black).bg(bar_color))
            .text_value(fmt_short_amount(*amount));

        chart = chart.data(BarGroup::default().label(Line::from(label)).bars(&[bar]));
    }

    f.render_widget(chart, area);
}

fn render_net_worth_help(f: &mut Frame, area: Rect) {
    let spans = vec![
        Span::styled(
            " j/k ↑↓",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" category  "),
        Span::styled(
            "h/l ←→",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" period  "),
        Span::styled(
            "m",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" month  "),
        Span::styled(
            "y",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" year  "),
        Span::styled(
            "Tab",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" monthly chart  "),
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" inspect"),
    ];
    let para = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(para, area);
}
