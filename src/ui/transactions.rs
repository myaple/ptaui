use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let txns = &app.ledger.transactions;

    if txns.is_empty() {
        let para = Paragraph::new("  No transactions found.")
            .block(Block::default().borders(Borders::ALL).title(" Transactions "));
        f.render_widget(para, area);
        return;
    }

    // Show in reverse chronological order
    let mut sorted: Vec<_> = txns.iter().collect();
    sorted.sort_by(|a, b| b.date.cmp(&a.date));

    let visible_height = area.height.saturating_sub(2) as usize;
    let scroll = app.tx_scroll.min(sorted.len().saturating_sub(1));
    let selected = app.tx_selected.min(sorted.len().saturating_sub(1));

    let items: Vec<ListItem> = sorted
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible_height)
        .map(|(abs_idx, txn)| {
            let is_selected = abs_idx == selected;

            let payee_narration = match &txn.payee {
                Some(p) => format!("{} — {}", p, txn.narration),
                None => txn.narration.clone(),
            };

            // Find the main debit posting (first posting with positive amount)
            let amount_str = txn
                .postings
                .iter()
                .filter_map(|p| {
                    if let (Some(amt), Some(cur)) = (&p.amount, &p.currency) {
                        if *amt > rust_decimal::Decimal::ZERO {
                            return Some(format!(
                                "{} {}",
                                crate::ui::dashboard::fmt_decimal(*amt),
                                cur
                            ));
                        }
                    }
                    None
                })
                .next()
                .unwrap_or_else(|| "—".to_string());

            let accounts: Vec<String> = txn
                .postings
                .iter()
                .map(|p| p.account.split(':').next_back().unwrap_or(&p.account).to_string())
                .collect();
            let accounts_str = accounts.join(" / ");

            let flag_style = if is_selected {
                Style::default().fg(Color::Black)
            } else if txn.flag == '!' {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };

            let row = Line::from(vec![
                Span::styled(
                    format!(" {} ", txn.date.format("%Y-%m-%d")),
                    if is_selected {
                        Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Cyan)
                    },
                ),
                Span::raw(" "),
                Span::styled(txn.flag.to_string(), flag_style),
                Span::raw(" "),
                Span::styled(
                    format!("{:<40}", &payee_narration.chars().take(40).collect::<String>()),
                    if is_selected {
                        Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
                Span::styled(
                    format!("{:>14}", amount_str),
                    if is_selected {
                        Style::default().fg(Color::Black).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    },
                ),
                Span::raw("  "),
                Span::styled(
                    accounts_str.chars().take(30).collect::<String>(),
                    if is_selected {
                        Style::default().fg(Color::Black)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
            ]);

            if is_selected {
                ListItem::new(row).style(Style::default().bg(Color::Cyan))
            } else {
                ListItem::new(row)
            }
        })
        .collect();

    let title = format!(
        " Transactions ({}) — ↑↓ navigate  e edit ",
        txns.len()
    );
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(list, area);
}
