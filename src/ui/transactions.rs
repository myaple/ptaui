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
        let para = Paragraph::new("  No transactions found.").block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Transactions "),
        );
        f.render_widget(para, area);
        return;
    }

    // Apply account filter: keep transactions that have at least one posting
    // whose account is in the active filter set.
    let filter = app.active_tx_account_filter();
    let mut sorted: Vec<_> = txns
        .iter()
        .filter(|txn| match &filter {
            None => true,
            Some(set) => txn.postings.iter().any(|p| set.contains(&p.account)),
        })
        .collect();

    // Show in reverse chronological order
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
            let is_reconcile_selected = app.reconcile_selected.contains(&txn.line);
            let is_reconciled = txn.is_reconciled();

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
                .map(|p| {
                    p.account
                        .split(':')
                        .next_back()
                        .unwrap_or(&p.account)
                        .to_string()
                })
                .collect();
            let accounts_str = accounts.join(" / ");

            // Reconcile status emoji: ✓ for reconciled, · for unreconciled
            let reconcile_icon = if is_reconciled { "✓" } else { "·" };

            // Multi-select indicator in reconcile mode: ● if selected, space otherwise
            let select_icon = if app.reconcile_mode {
                if is_reconcile_selected {
                    "●"
                } else {
                    " "
                }
            } else {
                ""
            };

            // Choose row background
            let row_bg = if is_selected && is_reconcile_selected && app.reconcile_mode {
                // Selected cursor + multi-selected in reconcile mode
                Some(Color::Magenta)
            } else if is_selected {
                Some(Color::Cyan)
            } else if is_reconcile_selected && app.reconcile_mode {
                Some(Color::Blue)
            } else {
                None
            };

            let flag_style = if row_bg.is_some() {
                Style::default().fg(Color::Black)
            } else if txn.flag == '!' {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            };

            let reconcile_style = if row_bg.is_some() {
                Style::default().fg(Color::Black)
            } else if is_reconciled {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let mut spans = vec![];

            // Multi-select indicator (only in reconcile mode)
            if app.reconcile_mode {
                spans.push(Span::styled(
                    format!("{} ", select_icon),
                    if is_reconcile_selected && row_bg.is_none() {
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Black)
                    },
                ));
            }

            spans.extend([
                Span::styled(
                    format!(" {} ", txn.date.format("%Y-%m-%d")),
                    if row_bg.is_some() {
                        Style::default()
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Cyan)
                    },
                ),
                Span::raw(" "),
                Span::styled(txn.flag.to_string(), flag_style),
                Span::raw(" "),
                // Reconcile status column
                Span::styled(format!("{} ", reconcile_icon), reconcile_style),
                Span::styled(
                    format!(
                        "{:<40}",
                        &payee_narration.chars().take(40).collect::<String>()
                    ),
                    if row_bg.is_some() {
                        Style::default()
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
                Span::styled(
                    format!("{:>14}", amount_str),
                    if row_bg.is_some() {
                        Style::default()
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    },
                ),
                Span::raw("  "),
                Span::styled(
                    accounts_str.chars().take(30).collect::<String>(),
                    if row_bg.is_some() {
                        Style::default().fg(Color::Black)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
            ]);

            let row = Line::from(spans);

            if let Some(bg) = row_bg {
                ListItem::new(row).style(Style::default().bg(bg))
            } else {
                ListItem::new(row)
            }
        })
        .collect();

    let filter_hint = if filter.is_some() {
        let checked = app.tx_account_filter.iter().filter(|(_, c)| *c).count();
        let total = app.tx_account_filter.len();
        format!("  f filter ({}/{})  ", checked, total)
    } else {
        "  f filter  ".to_string()
    };

    let title = if app.reconcile_mode {
        let sel_count = app.reconcile_selected.len();
        let sel_hint = if sel_count > 0 {
            format!("  {} selected", sel_count)
        } else {
            String::new()
        };
        format!(
            " [RECONCILE] ({}){}  space:select  r:reconcile  u:unreconcile  esc:exit",
            sorted.len(),
            sel_hint,
        )
    } else {
        format!(
            " Transactions ({}) — ↑↓ navigate  e edit  R reconcile{}",
            sorted.len(),
            filter_hint
        )
    };

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(list, area);
}
