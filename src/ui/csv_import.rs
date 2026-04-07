use crate::app::{App, CsvImportStep, CsvMappingField};
use crate::ui::{centered_modal, render_dim};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Row, Table},
    Frame,
};

pub fn render_modal(f: &mut Frame, app: &App) {
    let state = match &app.csv_import_state {
        Some(s) => s,
        None => return,
    };

    render_dim(f);

    // Use most of the screen
    let area = f.area();
    let modal_w = (area.width * 90 / 100).max(60);
    let modal_h = (area.height * 85 / 100).max(20);
    let modal_outer = centered_modal(modal_w, modal_h, area);
    f.render_widget(Clear, modal_outer);

    let outer_block = Block::default().borders(Borders::ALL);
    let modal_area = outer_block.inner(modal_outer);
    f.render_widget(outer_block, modal_outer);

    // Step indicator at top
    let steps = ["1:File", "2:Account", "3:Columns", "4:Review"];
    let current_idx = match state.step {
        CsvImportStep::FilePath => 0,
        CsvImportStep::AccountSelect => 1,
        CsvImportStep::ColumnMapping => 2,
        CsvImportStep::Review => 3,
    };

    let step_spans: Vec<Span> = steps
        .iter()
        .enumerate()
        .flat_map(|(i, s)| {
            let style = if i == current_idx {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if i < current_idx {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let sep = if i < steps.len() - 1 { " > " } else { "" };
            vec![
                Span::styled(format!(" {} ", s), style),
                Span::styled(sep, Style::default().fg(Color::DarkGray)),
            ]
        })
        .collect();

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // step bar
            Constraint::Min(1),    // content
            Constraint::Length(3), // error/status
        ])
        .split(modal_area);

    let step_block = Block::default().borders(Borders::ALL).title(" CSV Import ");
    f.render_widget(
        Paragraph::new(Line::from(step_spans)).block(step_block),
        inner[0],
    );

    // Main content area
    match state.step {
        CsvImportStep::FilePath => render_file_path(f, app, inner[1]),
        CsvImportStep::AccountSelect => render_account_select(f, app, inner[1]),
        CsvImportStep::ColumnMapping => render_column_mapping(f, app, inner[1]),
        CsvImportStep::Review => render_review(f, app, inner[1]),
    }

    // Error bar
    if let Some(ref err) = state.error {
        let err_para = Paragraph::new(Line::from(Span::styled(
            format!(" {} {}", "\u{2717}", err),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )))
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(err_para, inner[2]);
    } else {
        let hint = match state.step {
            CsvImportStep::FilePath => "Tab: complete path  |  Enter: load file  |  Esc: cancel",
            CsvImportStep::AccountSelect => "Tab: autocomplete  |  Enter: next  |  Esc: back",
            CsvImportStep::ColumnMapping => {
                "Left/Right: change column  |  Tab/Down: next field  |  Enter: parse & review  |  Esc: back"
            }
            CsvImportStep::Review => {
                if state.editing_category {
                    "Tab: autocomplete  |  Enter/Esc: done editing"
                } else {
                    "j/k: navigate  |  Space: toggle  |  e/Tab: edit category  |  c: apply cat to same payee  |  Enter: import  |  Esc: back"
                }
            }
        };
        let hint_para = Paragraph::new(Line::from(Span::styled(
            format!(" {}", hint),
            Style::default().fg(Color::DarkGray),
        )))
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(hint_para, inner[2]);
    }
}

fn render_file_path(f: &mut Frame, app: &App, area: Rect) {
    let state = app.csv_import_state.as_ref().unwrap();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);

    let label = Paragraph::new(Line::from(vec![Span::styled(
        "  Enter the path to your CSV file:",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]))
    .block(Block::default().borders(Borders::NONE));
    f.render_widget(label, chunks[0]);

    let input = Paragraph::new(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{}\u{2588}", &state.file_path),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" File Path "));
    f.render_widget(input, chunks[1]);

    let help = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Supported: CSV files with headers",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "  Tip: use ~/path/to/file.csv for home directory",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "  Tab to autocomplete path",
            Style::default().fg(Color::DarkGray),
        )),
    ]);
    f.render_widget(help, chunks[2]);
}

fn render_account_select(f: &mut Frame, app: &App, area: Rect) {
    let state = app.csv_import_state.as_ref().unwrap();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    // Left: input
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(chunks[0]);

    let label = Paragraph::new(Line::from(Span::styled(
        "  Which account does this CSV belong to?",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    f.render_widget(label, left_chunks[0]);

    let input = Paragraph::new(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{}\u{2588}", &state.account),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" Account "));
    f.render_widget(input, left_chunks[1]);

    let info_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  CSV: {}", state.file_path),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            format!("  Rows: {}", state.raw_rows.len()),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(Paragraph::new(info_lines), left_chunks[2]);

    // Right: suggestions
    let suggestions = state.filtered_account_suggestions();
    let items: Vec<ListItem> = if suggestions.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            " Start typing to see accounts...",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        suggestions
            .iter()
            .map(|s| {
                ListItem::new(Line::from(Span::styled(
                    *s,
                    Style::default().fg(Color::Cyan),
                )))
            })
            .collect()
    };
    f.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Suggestions "),
        ),
        chunks[1],
    );
}

fn render_column_mapping(f: &mut Frame, app: &App, area: Rect) {
    let state = app.csv_import_state.as_ref().unwrap();

    // Height: 1 blank + 2 per field + 2 borders. Single-amount = 6 fields = 15, debit/credit = 7 = 17.
    let mapping_height = if state.use_debit_credit { 17u16 } else { 15u16 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(mapping_height), // mapping fields
            Constraint::Min(1),                 // preview table
        ])
        .split(area);

    // Mapping fields
    let col_name = |idx: usize| -> String {
        state
            .headers
            .get(idx)
            .map(|h| format!("{}: {}", idx, h))
            .unwrap_or_else(|| format!("{}: ???", idx))
    };

    let sample = |idx: usize| -> String {
        state
            .raw_rows
            .first()
            .and_then(|r| r.get(idx))
            .cloned()
            .unwrap_or_default()
    };

    let focused_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let normal_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let col_name_opt = |opt: Option<usize>| -> String {
        match opt {
            None => "[--]".to_string(),
            Some(idx) => state
                .headers
                .get(idx)
                .map(|h| format!("{}: {}", idx, h))
                .unwrap_or_else(|| format!("{}: ???", idx)),
        }
    };

    let mut mappings: Vec<(CsvMappingField, &str, String, String)> = vec![
        (
            CsvMappingField::Date,
            "Date Column   ",
            col_name(state.date_col),
            format!("e.g. {}", sample(state.date_col)),
        ),
        (
            CsvMappingField::Payee,
            "Payee Column  ",
            col_name(state.payee_col),
            format!("e.g. {}", sample(state.payee_col)),
        ),
        (
            CsvMappingField::AmountMode,
            "Amount Mode   ",
            if state.use_debit_credit {
                "[Debit + Credit]".to_string()
            } else {
                "[Single Amount] ".to_string()
            },
            "Space to toggle".to_string(),
        ),
    ];

    if state.use_debit_credit {
        mappings.push((
            CsvMappingField::Debit,
            "Debit Column  ",
            col_name_opt(state.debit_col),
            state
                .debit_col
                .map(|c| format!("e.g. {}", sample(c)))
                .unwrap_or_default(),
        ));
        mappings.push((
            CsvMappingField::Credit,
            "Credit Column ",
            col_name_opt(state.credit_col),
            if state.credit_col.is_none() {
                "Space to enable (optional)".to_string()
            } else {
                state
                    .credit_col
                    .map(|c| format!("e.g. {}", sample(c)))
                    .unwrap_or_default()
            },
        ));
    } else {
        mappings.push((
            CsvMappingField::Amount,
            "Amount Column ",
            col_name(state.amount_col),
            format!("e.g. {}", sample(state.amount_col)),
        ));
    }

    mappings.push((
        CsvMappingField::DateFormat,
        "Date Format   ",
        format!("{}\u{2588}", &state.date_format),
        "e.g. %m/%d/%Y or %Y-%m-%d".to_string(),
    ));
    mappings.push((
        CsvMappingField::Negate,
        "Negate Amounts",
        if state.negate_amounts {
            "[x]".to_string()
        } else {
            "[ ]".to_string()
        },
        "Space to toggle, flip +/- signs".to_string(),
    ));

    let mut lines: Vec<Line> = vec![Line::from("")];
    for (field, label, value, hint) in &mappings {
        let is_focused = state.mapping_focused == *field;
        let arrow = if is_focused
            && matches!(
                field,
                CsvMappingField::Date
                    | CsvMappingField::Payee
                    | CsvMappingField::Amount
                    | CsvMappingField::Debit
            ) {
            "\u{25C0} \u{25B6} "
        } else if is_focused && *field == CsvMappingField::Credit && state.credit_col.is_some() {
            "\u{25C0} \u{25B6} "
        } else {
            "  "
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}: ", label),
                if is_focused {
                    focused_style
                } else {
                    normal_style
                },
            ),
            Span::styled(format!("{:<30}", value), value_style),
            Span::styled(
                format!("{}  {}", arrow, hint),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(""));
    }

    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Column Mapping "),
        ),
        chunks[0],
    );

    // Preview table
    render_csv_preview(f, state, chunks[1]);
}

fn render_csv_preview(f: &mut Frame, state: &crate::app::CsvImportState, area: Rect) {
    let header_cells: Vec<Span> = state
        .headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let is_debit = state.use_debit_credit && state.debit_col == Some(i);
            let is_credit = state.use_debit_credit && state.credit_col == Some(i);
            let style = if i == state.date_col || i == state.payee_col {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_debit || is_credit {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if !state.use_debit_credit && i == state.amount_col {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Span::styled(h.clone(), style)
        })
        .collect();
    let header = Row::new(header_cells)
        .style(Style::default())
        .bottom_margin(1);

    let rows: Vec<Row> = state
        .raw_rows
        .iter()
        .take(5)
        .map(|r| {
            let cells: Vec<Span> = r
                .iter()
                .map(|c| Span::styled(c.clone(), Style::default().fg(Color::White)))
                .collect();
            Row::new(cells)
        })
        .collect();

    let num_cols = state.headers.len().max(1);
    let col_width = Constraint::Percentage((100 / num_cols as u16).max(10));
    let widths: Vec<Constraint> = (0..num_cols).map(|_| col_width).collect();

    let table = Table::new(rows, &widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" CSV Preview (first 5 rows) "),
    );

    f.render_widget(table, area);
}

fn render_review(f: &mut Frame, app: &App, area: Rect) {
    let state = app.csv_import_state.as_ref().unwrap();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    // Left: transaction table
    render_review_table(f, state, chunks[0]);

    // Right: category suggestions + stats
    render_review_sidebar(f, state, chunks[1]);
}

fn render_review_table(f: &mut Frame, state: &crate::app::CsvImportState, area: Rect) {
    let widths = [
        Constraint::Length(3),  // checkbox
        Constraint::Length(12), // date
        Constraint::Min(20),    // payee
        Constraint::Length(12), // amount
        Constraint::Min(20),    // category
    ];

    let header = Row::new(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            "Date",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Payee",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Amount",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Category",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ])
    .bottom_margin(1);

    let visible_rows = area.height.saturating_sub(4) as usize; // borders + header + margin
    let rows: Vec<Row> = state
        .rows
        .iter()
        .enumerate()
        .skip(state.scroll)
        .take(visible_rows)
        .map(|(i, row)| {
            let is_selected = i == state.cursor;
            let is_dup = row.is_duplicate;

            let base_style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else if is_dup {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };

            let checkbox = if row.include { "[x]" } else { "[ ]" };
            let dup_marker = if is_dup { " DUP" } else { "" };

            let amount_style = if row.amount.is_sign_negative() {
                base_style.fg(Color::Red)
            } else {
                base_style.fg(Color::Green)
            };

            let cat_display = if is_selected && state.editing_category {
                format!("{}\u{2588}", &row.category)
            } else {
                row.category.clone()
            };

            let cat_style = if is_selected && state.editing_category {
                base_style.fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if row.category.is_empty() && !is_dup {
                base_style.fg(Color::Red)
            } else {
                base_style
            };

            Row::new(vec![
                Span::styled(checkbox.to_string(), base_style),
                Span::styled(
                    format!("{}{}", row.date.format("%Y-%m-%d"), dup_marker),
                    base_style,
                ),
                Span::styled(truncate_str(&row.payee, 30), base_style),
                Span::styled(format!("{:.2}", row.amount), amount_style),
                Span::styled(cat_display, cat_style),
            ])
        })
        .collect();

    let included = state.rows.iter().filter(|r| r.include).count();
    let dups = state.rows.iter().filter(|r| r.is_duplicate).count();
    let total = state.rows.len();
    let no_cat = state
        .rows
        .iter()
        .filter(|r| r.include && r.category.is_empty())
        .count();

    let title = format!(
        " Transactions: {} selected / {} total | {} duplicates | {} need category ",
        included, total, dups, no_cat
    );

    let table = Table::new(rows, &widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(table, area);
}

fn render_review_sidebar(f: &mut Frame, state: &crate::app::CsvImportState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Suggestions
    let suggestions = state.filtered_category_suggestions();
    let items: Vec<ListItem> = if suggestions.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            " Start typing...",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        suggestions
            .iter()
            .map(|s| {
                ListItem::new(Line::from(Span::styled(
                    *s,
                    Style::default().fg(Color::Cyan),
                )))
            })
            .collect()
    };

    let sug_title = if state.editing_category {
        " Category Suggestions "
    } else {
        " Suggestions "
    };
    f.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(sug_title)),
        chunks[0],
    );

    // Help
    let help_lines = if state.editing_category {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Editing category",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Tab    Autocomplete",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  Enter  Done",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  Esc    Cancel edit",
                Style::default().fg(Color::White),
            )),
        ]
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Navigation",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "  j/k    Up/Down",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  Space  Toggle include",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  e/Tab  Edit category",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  c      Apply cat to payee",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  C      Overwrite cat for payee",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  Enter  Import selected",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  Esc    Back to mapping",
                Style::default().fg(Color::White),
            )),
        ]
    };

    f.render_widget(
        Paragraph::new(help_lines).block(Block::default().borders(Borders::ALL).title(" Help ")),
        chunks[1],
    );
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
