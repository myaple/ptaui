use crate::app::{AddAccountField, AddTxField, App, GitStatus, Screen, StartupGitChoice, ACCOUNT_TYPES};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent};

pub fn handle_event(app: &mut App, event: Event) -> Result<()> {
    if let Event::Key(key) = event {
        // Startup screen has its own handler — nothing else fires while it's up
        if app.screen == Screen::Startup {
            return handle_startup(app, key);
        }

        // Global keys (not available during add transaction/account)
        let in_form = matches!(app.screen, Screen::AddTransaction | Screen::AddAccount);
        match key.code {
            KeyCode::Char('q') if !in_form => {
                app.running = false;
                return Ok(());
            }
            KeyCode::Char('1') if !in_form => {
                app.navigate_to(Screen::Dashboard);
                return Ok(());
            }
            KeyCode::Char('2') if !in_form => {
                app.navigate_to(Screen::Transactions);
                return Ok(());
            }
            KeyCode::Char('3') if !in_form => {
                app.navigate_to(Screen::AddTransaction);
                return Ok(());
            }
            KeyCode::Char('4') if !in_form => {
                app.navigate_to(Screen::Reports);
                return Ok(());
            }
            // 'a' opens Add Account from dashboard or transaction list
            KeyCode::Char('a') if matches!(app.screen, Screen::Dashboard | Screen::Transactions) => {
                app.navigate_to(Screen::AddAccount);
                return Ok(());
            }
            // 'c' creates the beancount file when it doesn't exist yet
            KeyCode::Char('c') if app.screen == Screen::Dashboard && !app.file_found => {
                if let Err(e) = app.create_beancount_file() {
                    app.status_message = Some(format!("Error: {}", e));
                }
                return Ok(());
            }
            KeyCode::Char('r') if !in_form => {
                app.reload_ledger()?;
                app.status_message = Some("Ledger reloaded.".to_string());
                return Ok(());
            }
            _ => {}
        }

        match app.screen {
            Screen::Dashboard => handle_dashboard(app, key),
            Screen::Transactions => handle_transactions(app, key),
            Screen::AddTransaction => handle_add_tx(app, key)?,
            Screen::AddAccount => handle_add_account(app, key)?,
            Screen::Reports => {}
            Screen::Startup => unreachable!(),
        }
    }
    Ok(())
}

fn handle_startup(app: &mut App, key: KeyEvent) -> Result<()> {
    let is_uncontrolled =
        matches!(app.startup.git_status, GitStatus::Uncontrolled { .. });
    let already_acted = app.startup.git_init_result.is_some();

    match key.code {
        // Dismiss / continue
        KeyCode::Esc => {
            app.navigate_to(Screen::Dashboard);
        }
        // Toggle Y/N with Tab, arrow keys, or h/l
        KeyCode::Tab
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Char('h')
        | KeyCode::Char('l') => {
            if is_uncontrolled && !already_acted {
                app.startup.git_choice = match app.startup.git_choice {
                    StartupGitChoice::InitRepo => StartupGitChoice::Skip,
                    StartupGitChoice::Skip => StartupGitChoice::InitRepo,
                };
            }
        }
        // Shortcut: y / n
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if is_uncontrolled && !already_acted {
                app.startup.git_choice = StartupGitChoice::InitRepo;
                app.startup_init_git();
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            if is_uncontrolled && !already_acted {
                app.startup.git_choice = StartupGitChoice::Skip;
                app.navigate_to(Screen::Dashboard);
            }
        }
        KeyCode::Enter => {
            if is_uncontrolled && !already_acted {
                match app.startup.git_choice {
                    StartupGitChoice::InitRepo => app.startup_init_git(),
                    StartupGitChoice::Skip => app.navigate_to(Screen::Dashboard),
                }
            } else {
                // Config-only notice, or post-init — just proceed
                app.navigate_to(Screen::Dashboard);
            }
        }
        KeyCode::Char('q') => {
            app.running = false;
        }
        _ => {}
    }
    Ok(())
}

fn handle_dashboard(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            app.dashboard_scroll = app.dashboard_scroll.saturating_add(1);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.dashboard_scroll = app.dashboard_scroll.saturating_sub(1);
        }
        _ => {}
    }
}

fn handle_transactions(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            let max = app.ledger.transactions.len().saturating_sub(1);
            app.tx_scroll = (app.tx_scroll + 1).min(max);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.tx_scroll = app.tx_scroll.saturating_sub(1);
        }
        KeyCode::PageDown => {
            let max = app.ledger.transactions.len().saturating_sub(1);
            app.tx_scroll = (app.tx_scroll + 20).min(max);
        }
        KeyCode::PageUp => {
            app.tx_scroll = app.tx_scroll.saturating_sub(20);
        }
        _ => {}
    }
}

fn handle_add_tx(app: &mut App, key: KeyEvent) -> Result<()> {
    let form = match app.add_tx_form.as_mut() {
        Some(f) => f,
        None => return Ok(()),
    };

    match key.code {
        KeyCode::Esc => {
            app.navigate_to(Screen::Dashboard);
            return Ok(());
        }
        KeyCode::Tab => {
            if matches!(form.focused, AddTxField::FromAccount | AddTxField::ToAccount) {
                let suggestions = form.suggestions_for_current();
                if !suggestions.is_empty() {
                    let current = match form.focused {
                        AddTxField::FromAccount => &form.from_account,
                        AddTxField::ToAccount => &form.to_account,
                        _ => unreachable!(),
                    };
                    if !suggestions.contains(current) {
                        form.autocomplete();
                        return Ok(());
                    }
                }
            }
            let next = form.focused.next();
            form.focused = next;
        }
        KeyCode::BackTab => {
            let prev = form.focused.prev();
            form.focused = prev;
        }
        KeyCode::Down => {
            let next = form.focused.next();
            form.focused = next;
        }
        KeyCode::Up => {
            let prev = form.focused.prev();
            form.focused = prev;
        }
        KeyCode::Enter => {
            if form.focused == AddTxField::Confirm {
                match app.commit_transaction() {
                    Ok(()) => {
                        app.screen = Screen::Dashboard;
                        app.add_tx_form = None;
                    }
                    Err(e) => {
                        if let Some(form) = app.add_tx_form.as_mut() {
                            form.error = Some(e.to_string());
                        }
                    }
                }
            } else {
                let next = form.focused.next();
                form.focused = next;
            }
        }
        KeyCode::Backspace => {
            if form.focused != AddTxField::Confirm {
                let field = form.current_field_mut();
                field.pop();
            }
        }
        KeyCode::Char(c) => {
            if form.focused != AddTxField::Confirm {
                let field = form.current_field_mut();
                field.push(c);
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_add_account(app: &mut App, key: KeyEvent) -> Result<()> {
    let form = match app.add_account_form.as_mut() {
        Some(f) => f,
        None => return Ok(()),
    };

    match key.code {
        KeyCode::Esc => {
            app.navigate_to(Screen::Dashboard);
            return Ok(());
        }
        // Cycle account type with left/right arrows
        KeyCode::Left | KeyCode::Char('h')
            if form.focused == AddAccountField::AccountType =>
        {
            form.type_idx = if form.type_idx == 0 {
                ACCOUNT_TYPES.len() - 1
            } else {
                form.type_idx - 1
            };
        }
        KeyCode::Right | KeyCode::Char('l')
            if form.focused == AddAccountField::AccountType =>
        {
            form.type_idx = (form.type_idx + 1) % ACCOUNT_TYPES.len();
        }
        KeyCode::Tab | KeyCode::Down => {
            let next = form.focused.next();
            form.focused = next;
        }
        KeyCode::BackTab | KeyCode::Up => {
            let prev = form.focused.prev();
            form.focused = prev;
        }
        KeyCode::Enter => {
            if form.focused == AddAccountField::Confirm {
                match app.commit_account() {
                    Ok(()) => {
                        app.screen = Screen::Dashboard;
                        app.add_account_form = None;
                    }
                    Err(e) => {
                        if let Some(form) = app.add_account_form.as_mut() {
                            form.error = Some(e.to_string());
                        }
                    }
                }
            } else {
                let next = form.focused.next();
                form.focused = next;
            }
        }
        KeyCode::Backspace => {
            match form.focused {
                AddAccountField::SubName => { form.sub_name.pop(); }
                AddAccountField::Currencies => { form.currencies.pop(); }
                AddAccountField::Date => { form.date.pop(); }
                _ => {}
            }
        }
        KeyCode::Char(c) => {
            match form.focused {
                AddAccountField::SubName => form.sub_name.push(c),
                AddAccountField::Currencies => form.currencies.push(c),
                AddAccountField::Date => form.date.push(c),
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}
