mod app;
mod beancount;
mod config;
mod events;
mod git;
mod ui;

use anyhow::{Context, Result};
use app::{App, GitStatus, StartupGitChoice, StartupState};
use beancount::parser;
use config::Config;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

fn main() -> Result<()> {
    // Load config — returns (config, freshly_created)
    let (config, config_just_created) = Config::load().context("Loading config")?;
    let config_path = Config::config_path()?;

    // Load ledger (ok if file doesn't exist yet)
    let ledger = {
        let path = config.resolved_beancount_file();
        if path.exists() {
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Reading beancount file: {}", path.display()))?;
            parser::parse(&source).context("Parsing beancount file")?
        } else {
            parser::Ledger::default()
        }
    };

    // Determine git status of the beancount file's directory
    let git_status = {
        let path = config.resolved_beancount_file();
        if let Some(dir) = path.parent() {
            if !dir.exists() || !path.exists() {
                GitStatus::NoFile
            } else if git::is_git_repo(dir) {
                GitStatus::Controlled
            } else {
                GitStatus::Uncontrolled { dir: dir.to_path_buf() }
            }
        } else {
            GitStatus::NoFile
        }
    };

    let startup = StartupState {
        config_just_created,
        config_path,
        git_status,
        git_choice: StartupGitChoice::InitRepo,
        git_init_result: None,
    };

    let mut app = App::new(config, ledger, startup);

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result?;
    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            let ev = event::read()?;
            events::handle_event(app, ev)?;
        }

        if !app.running {
            break;
        }
    }
    Ok(())
}
