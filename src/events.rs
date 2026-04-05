use crate::app::{AddTxField, App, Screen};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent};

pub fn handle_event(app: &mut App, event: Event) -> Result<()> {
    if let Event::Key(key) = event {
        // Global keys
        match key.code {
            KeyCode::Char('q') if app.screen != Screen::AddTransaction => {
                app.running = false;
                return Ok(());
            }
            KeyCode::Char('1') if app.screen != Screen::AddTransaction => {
                app.navigate_to(Screen::Dashboard);
                return Ok(());
            }
            KeyCode::Char('2') if app.screen != Screen::AddTransaction => {
                app.navigate_to(Screen::Transactions);
                return Ok(());
            }
            KeyCode::Char('3') => {
                app.navigate_to(Screen::AddTransaction);
                return Ok(());
            }
            KeyCode::Char('4') if app.screen != Screen::AddTransaction => {
                app.navigate_to(Screen::Reports);
                return Ok(());
            }
            KeyCode::Char('r') if app.screen != Screen::AddTransaction => {
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
            Screen::Reports => {}
        }
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
            // On account fields, try autocomplete first if not already completed
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
