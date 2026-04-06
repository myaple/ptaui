use crate::app::App;
use crate::ui::{centered_modal, render_dim};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

/// Height of the scrollable account list inside the modal.
const LIST_HEIGHT: usize = 18;

pub fn render_modal(f: &mut Frame, app: &App) {
    render_dim(f);

    let modal_height = (LIST_HEIGHT + 6) as u16;
    let area = centered_modal(64, modal_height, f.area());
    f.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    render_list(f, app, chunks[0]);
    render_help(f, chunks[1]);
}

fn render_list(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let filter = &app.tx_account_filter;
    let cursor = app.tx_account_filter_cursor;
    let scroll = app.tx_account_filter_scroll;

    let checked_count = filter.iter().filter(|(_, c)| *c).count();
    let total = filter.len();

    let title = format!(" Account Filter  [{}/{}] ", checked_count, total);

    let visible_end = (scroll + LIST_HEIGHT).min(total);
    let visible = &filter[scroll..visible_end];

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(rel_idx, (name, checked))| {
            let abs_idx = scroll + rel_idx;
            let is_cursor = abs_idx == cursor;
            let checkbox = if *checked { "[x]" } else { "[ ]" };
            let checkbox_style = if *checked {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let name_style = if is_cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if *checked {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let prefix = if is_cursor { " ▶ " } else { "   " };

            ListItem::new(Line::from(vec![
                Span::raw(prefix),
                Span::styled(
                    checkbox,
                    if is_cursor {
                        Style::default().fg(Color::Black).bg(Color::Cyan)
                    } else {
                        checkbox_style
                    },
                ),
                Span::styled(format!(" {}", name), name_style),
            ]))
        })
        .collect();

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

    if filter.is_empty() {
        let para = Paragraph::new(Line::from(Span::styled(
            "  No accounts found.",
            Style::default().fg(Color::Yellow),
        )))
        .block(block);
        f.render_widget(para, area);
    } else {
        f.render_widget(List::new(items).block(block), area);
    }
}

fn render_help(f: &mut Frame, area: ratatui::layout::Rect) {
    let spans = vec![
        Span::styled(
            " Space",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" toggle  "),
        Span::styled(
            "a",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" check all  "),
        Span::styled(
            "u",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" uncheck all  "),
        Span::styled(
            "j/k ↑↓",
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
