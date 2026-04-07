use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use rust_decimal::Decimal;
use std::collections::BTreeMap;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let balances = app.ledger.balances();

    // Group accounts by type — only show Assets and Liabilities
    let mut groups: BTreeMap<&str, Vec<(&str, String)>> = BTreeMap::new();
    for account in &app.ledger.accounts {
        let acct_type = account.name.split(':').next().unwrap_or("Other");
        if acct_type != "Assets" && acct_type != "Liabilities" {
            continue;
        }
        let bal_str = if let Some(curs) = balances.get(&account.name) {
            curs.iter()
                .map(|(cur, amt)| format!("{} {}", fmt_decimal(*amt), cur))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            "0".to_string()
        };
        groups
            .entry(acct_type)
            .or_default()
            .push((&account.name, bal_str));
    }

    if app.ledger.accounts.is_empty() {
        let lines = if !app.file_found {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Beancount file not found.",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    format!(
                        "  Expected: {}",
                        app.config.resolved_beancount_file().display()
                    ),
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press c to create this file with git version control.",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Or edit ~/.config/ptaui/config.json to point to an existing file,",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(Span::styled(
                    "  then press r to reload.",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        } else {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  No accounts declared in this beancount file.",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press a to add your first account.",
                    Style::default().fg(Color::Cyan),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Accounts are declared with 'open' directives, e.g.:",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(Span::styled(
                    "  2024-01-01 open Assets:Checking USD",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        };
        let para =
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Accounts "));
        f.render_widget(para, area);
        return;
    }

    // Build list items
    let mut items: Vec<ListItem> = Vec::new();
    for (group, accounts) in &groups {
        // Group header
        let color = group_color(group);
        items.push(ListItem::new(Line::from(vec![Span::styled(
            format!(" {} ", group),
            Style::default()
                .fg(color)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )])));

        for (name, bal) in accounts {
            let short = name.trim_start_matches(&format!("{}:", group));
            let indent = "  ";
            let line = Line::from(vec![
                Span::styled(
                    format!("{}  {:<45}", indent, short),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("{:>20}", bal),
                    Style::default()
                        .fg(balance_color(group, bal))
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            items.push(ListItem::new(line));
        }
        items.push(ListItem::new(Line::from("")));
    }

    // Net worth summary
    let net = compute_net(&balances, &app.config.currency);
    items.push(ListItem::new(Line::from(vec![
        Span::styled(
            "  Net Worth",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>53} {}", fmt_decimal(net), app.config.currency),
            Style::default()
                .fg(if net >= Decimal::ZERO {
                    Color::Green
                } else {
                    Color::Red
                })
                .add_modifier(Modifier::BOLD),
        ),
    ])));

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Accounts & Balances "),
    );
    f.render_widget(list, area);
}

fn group_color(group: &str) -> Color {
    match group {
        "Assets" => Color::Green,
        "Liabilities" => Color::Red,
        "Income" => Color::Cyan,
        "Expenses" => Color::Yellow,
        "Equity" => Color::Magenta,
        _ => Color::White,
    }
}

fn balance_color(group: &str, bal: &str) -> Color {
    // Assets positive = green, Liabilities positive = red
    let is_negative = bal.contains('-');
    match group {
        "Assets" => {
            if is_negative {
                Color::Red
            } else {
                Color::Green
            }
        }
        "Liabilities" => {
            if is_negative {
                Color::Green
            } else {
                Color::Red
            }
        }
        _ => Color::White,
    }
}

fn compute_net(
    balances: &std::collections::HashMap<String, std::collections::HashMap<String, Decimal>>,
    currency: &str,
) -> Decimal {
    let mut net = Decimal::ZERO;
    for (account, curs) in balances {
        let acct_type = account.split(':').next().unwrap_or("");
        if let Some(amt) = curs.get(currency) {
            match acct_type {
                "Assets" => net += amt,
                "Liabilities" => net += amt, // liabilities are typically negative
                _ => {}
            }
        }
    }
    net
}

pub fn fmt_decimal(d: Decimal) -> String {
    // Format with 2 decimal places and thousands separators
    let rounded = d.round_dp(2);
    let s = format!("{:.2}", rounded);
    // Insert commas
    let (int_part, dec_part) = s.split_once('.').unwrap_or((&s, "00"));
    let negative = int_part.starts_with('-');
    let digits = if negative { &int_part[1..] } else { int_part };
    let mut with_commas = String::new();
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            with_commas.push(',');
        }
        with_commas.push(ch);
    }
    let int_formatted: String = with_commas.chars().rev().collect();
    if negative {
        format!("-{}.{}", int_formatted, dec_part)
    } else {
        format!("{}.{}", int_formatted, dec_part)
    }
}
