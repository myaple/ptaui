use crate::app::{App, GitStatus, StartupGitChoice};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    // Dim background by rendering a full-area block first
    let bg = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(bg, area);

    // Center a dialog box
    let dialog = centered_rect(72, 28, area);
    f.render_widget(Clear, dialog);

    let startup = &app.startup;

    let mut lines: Vec<Line> = vec![Line::from("")];

    // ── Title ────────────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled(
        "  Welcome to ptaui",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // ── Config section ───────────────────────────────────────────────────────
    if startup.config_just_created {
        lines.push(Line::from(Span::styled(
            "  Config created",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {}", startup.config_path.display()),
            Style::default().fg(Color::White),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Edit it to set your beancount file path, then press r to reload.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
    }

    // ── Git section ──────────────────────────────────────────────────────────
    match &startup.git_status {
        GitStatus::Controlled => {
            lines.push(Line::from(Span::styled(
                "  ✓ Beancount file is version-controlled with git.",
                Style::default().fg(Color::Green),
            )));
        }
        GitStatus::NoFile => {
            lines.push(Line::from(Span::styled(
                "  Beancount file does not exist yet.",
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                "  Create it and press r to reload.",
                Style::default().fg(Color::DarkGray),
            )));
        }
        GitStatus::Uncontrolled { dir } => {
            lines.push(Line::from(Span::styled(
                "  ⚠  Beancount file is NOT version-controlled.",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                "  Without version control you cannot recover from accidental edits.",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("  Directory:  {}", dir.display()),
                Style::default().fg(Color::White),
            )));
            lines.push(Line::from(""));

            // If we already attempted an init, show the result instead
            if let Some(ref result) = startup.git_init_result {
                let color = if result.starts_with("Git repo") {
                    Color::Green
                } else {
                    Color::Red
                };
                lines.push(Line::from(Span::styled(
                    format!("  {}", result),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )));
            } else {
                // Render Y/N buttons
                lines.push(Line::from(Span::styled(
                    "  Initialise a git repo in that directory?",
                    Style::default().fg(Color::White),
                )));
                lines.push(Line::from(""));

                let (yes_style, no_style) = match startup.git_choice {
                    StartupGitChoice::InitRepo => (
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(Color::DarkGray),
                    ),
                    StartupGitChoice::Skip => (
                        Style::default().fg(Color::DarkGray),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                };

                lines.push(Line::from(vec![
                    Span::raw("     "),
                    Span::styled(" [ Y ] Init git repo ", yes_style),
                    Span::raw("   "),
                    Span::styled(" [ N ] Skip ", no_style),
                ]));
            }
        }
    }

    // ── Footer ───────────────────────────────────────────────────────────────
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Tab / ← → to switch   Enter to confirm   Esc to continue",
        Style::default().fg(Color::DarkGray),
    )));

    let dialog_block = Block::default()
        .borders(Borders::ALL)
        .title(" ptaui — Setup ")
        .title_alignment(Alignment::Center)
        .style(Style::default().fg(Color::Cyan));

    let para = Paragraph::new(lines).block(dialog_block);
    f.render_widget(para, dialog);
}

/// Return a centered Rect of the given width and height within `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
