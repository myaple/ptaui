use crate::app::{
    AddAccountField, AddTxField, App, GitStatus, Modal, ReportsView, Screen,
    StartupGitChoice, ACCOUNT_TYPES,
};
use crossterm::event::KeyModifiers;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent};

pub fn handle_event(app: &mut App, event: Event) -> Result<()> {
    if let Event::Key(key) = event {
        // Startup screen absorbs everything
        if app.screen == Screen::Startup {
            return handle_startup(app, key);
        }

        // Modal absorbs everything while open
        if app.modal.is_some() {
            return handle_modal(app, key);
        }

        // ── Global hotkeys (only when no modal is open) ───────────────────
        match key.code {
            KeyCode::Char('q') => {
                app.running = false;
                return Ok(());
            }
            KeyCode::Char('1') => {
                app.navigate_to(Screen::Dashboard);
                return Ok(());
            }
            KeyCode::Char('2') => {
                app.navigate_to(Screen::Transactions);
                return Ok(());
            }
            KeyCode::Char('3') => {
                app.navigate_to(Screen::Reports);
                return Ok(());
            }
            // 'a' is context-sensitive: add account on Dashboard, add transaction on Transactions
            KeyCode::Char('a') => {
                match app.screen {
                    Screen::Dashboard => app.open_modal(Modal::AddAccount),
                    Screen::Transactions => app.open_modal(Modal::AddTransaction),
                    _ => {}
                }
                return Ok(());
            }
            KeyCode::Char('c') if app.screen == Screen::Dashboard && !app.file_found => {
                if let Err(e) = app.create_beancount_file() {
                    app.status_message = Some(format!("Error: {}", e));
                }
                return Ok(());
            }
            KeyCode::Char('c') if app.screen == Screen::Reports => {
                app.open_modal(Modal::AccountFilter);
                return Ok(());
            }
            KeyCode::Tab if app.screen == Screen::Reports => {
                app.reports_view = match app.reports_view {
                    ReportsView::Monthly => ReportsView::Breakdown,
                    ReportsView::Breakdown => ReportsView::Monthly,
                };
                return Ok(());
            }
            KeyCode::Char('r') => {
                app.reload_ledger()?;
                app.status_message = Some("Ledger reloaded.".to_string());
                return Ok(());
            }
            _ => {}
        }

        // ── Per-screen keys ───────────────────────────────────────────────
        match app.screen {
            Screen::Dashboard => handle_dashboard(app, key),
            Screen::Transactions => handle_transactions(app, key),
            Screen::Reports => handle_reports(app, key),
            Screen::Startup => unreachable!(),
        }
    }
    Ok(())
}

// ── Startup ───────────────────────────────────────────────────────────────────

fn handle_startup(app: &mut App, key: KeyEvent) -> Result<()> {
    let is_uncontrolled = matches!(app.startup.git_status, GitStatus::Uncontrolled { .. });
    let already_acted = app.startup.git_init_result.is_some();

    match key.code {
        KeyCode::Esc => app.navigate_to(Screen::Dashboard),
        KeyCode::Tab | KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
            if is_uncontrolled && !already_acted {
                app.startup.git_choice = match app.startup.git_choice {
                    StartupGitChoice::InitRepo => StartupGitChoice::Skip,
                    StartupGitChoice::Skip => StartupGitChoice::InitRepo,
                };
            }
        }
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
                app.navigate_to(Screen::Dashboard);
            }
        }
        KeyCode::Char('q') => app.running = false,
        _ => {}
    }
    Ok(())
}

// ── Modal dispatcher ──────────────────────────────────────────────────────────

fn handle_modal(app: &mut App, key: KeyEvent) -> Result<()> {
    match app.modal.clone() {
        Some(Modal::AddTransaction) => handle_add_tx(app, key),
        Some(Modal::EditTransaction) => handle_edit_tx(app, key),
        Some(Modal::AddAccount) => handle_add_account(app, key),
        Some(Modal::AccountFilter) => handle_account_filter(app, key),
        Some(Modal::CategoryTransactions) => handle_category_transactions(app, key),
        Some(Modal::TxAccountFilter) => handle_tx_account_filter(app, key),
        None => Ok(()),
    }
}

// ── Background screens ────────────────────────────────────────────────────────

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

/// Approximate visible rows for scroll-clamping in the transaction list.
const TX_PAGE: usize = 20;

fn handle_transactions(app: &mut App, key: KeyEvent) {
    let max = app.ledger.transactions.len().saturating_sub(1);
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if app.tx_selected < max {
                app.tx_selected += 1;
                if app.tx_selected >= app.tx_scroll + TX_PAGE {
                    app.tx_scroll = app.tx_selected.saturating_sub(TX_PAGE - 1);
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.tx_selected > 0 {
                app.tx_selected -= 1;
                if app.tx_selected < app.tx_scroll {
                    app.tx_scroll = app.tx_selected;
                }
            }
        }
        KeyCode::PageDown => {
            app.tx_selected = (app.tx_selected + TX_PAGE).min(max);
            if app.tx_selected >= app.tx_scroll + TX_PAGE {
                app.tx_scroll = app.tx_selected.saturating_sub(TX_PAGE - 1);
            }
        }
        KeyCode::PageUp => {
            app.tx_selected = app.tx_selected.saturating_sub(TX_PAGE);
            if app.tx_selected < app.tx_scroll {
                app.tx_scroll = app.tx_selected;
            }
        }
        KeyCode::Char('e') => {
            if !app.ledger.transactions.is_empty() {
                app.open_edit_tx_modal();
            }
        }
        KeyCode::Char('f') => {
            app.open_modal(Modal::TxAccountFilter);
        }
        _ => {}
    }
}

// ── Modal: Add Transaction ────────────────────────────────────────────────────

fn handle_add_tx(app: &mut App, key: KeyEvent) -> Result<()> {
    let form = match app.add_tx_form.as_mut() {
        Some(f) => f,
        None => return Ok(()),
    };

    let was_on_payee = form.focused == AddTxField::Payee;

    match key.code {
        KeyCode::Esc => {
            app.close_modal();
            return Ok(());
        }
        KeyCode::Tab => {
            if matches!(
                form.focused,
                AddTxField::Payee | AddTxField::Category | AddTxField::Account
            ) {
                let suggestions = form.suggestions_for_current();
                if !suggestions.is_empty() {
                    let current = match form.focused {
                        AddTxField::Payee    => &form.payee,
                        AddTxField::Category => &form.category,
                        AddTxField::Account  => &form.account,
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
                    Ok(()) => app.close_modal(),
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

    // When leaving the Payee field, auto-fill category/account from the most
    // recent transaction with that payee (only if those fields are empty).
    let now_on_payee = app
        .add_tx_form
        .as_ref()
        .map(|f| f.focused == AddTxField::Payee)
        .unwrap_or(true);
    if was_on_payee && !now_on_payee {
        app.apply_payee_defaults();
    }

    Ok(())
}

// ── Modal: Edit Transaction ───────────────────────────────────────────────────

fn handle_edit_tx(app: &mut App, key: KeyEvent) -> Result<()> {
    let form = match app.add_tx_form.as_mut() {
        Some(f) => f,
        None => return Ok(()),
    };

    match key.code {
        KeyCode::Esc => {
            app.close_modal();
            return Ok(());
        }
        KeyCode::Tab => {
            if matches!(
                form.focused,
                AddTxField::Payee | AddTxField::Category | AddTxField::Account
            ) {
                let suggestions = form.suggestions_for_current();
                if !suggestions.is_empty() {
                    let current = match form.focused {
                        AddTxField::Payee    => &form.payee,
                        AddTxField::Category => &form.category,
                        AddTxField::Account  => &form.account,
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
                match app.commit_edit_transaction() {
                    Ok(()) => app.close_modal(),
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

// ── Modal: Add Account ────────────────────────────────────────────────────────

fn handle_add_account(app: &mut App, key: KeyEvent) -> Result<()> {
    let form = match app.add_account_form.as_mut() {
        Some(f) => f,
        None => return Ok(()),
    };

    match key.code {
        KeyCode::Esc => {
            app.close_modal();
            return Ok(());
        }
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
                    Ok(()) => app.close_modal(),
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
        KeyCode::Backspace => match form.focused {
            AddAccountField::SubName => { form.sub_name.pop(); }
            AddAccountField::Currencies => { form.currencies.pop(); }
            AddAccountField::Date => { form.date.pop(); }
            AddAccountField::InitialBalance => { form.initial_balance.pop(); }
            _ => {}
        },
        KeyCode::Char(c) => match form.focused {
            AddAccountField::SubName => form.sub_name.push(c),
            AddAccountField::Currencies => form.currencies.push(c),
            AddAccountField::Date => form.date.push(c),
            AddAccountField::InitialBalance => form.initial_balance.push(c),
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

// ── Modal: Account Filter ─────────────────────────────────────────────────────

fn handle_account_filter(app: &mut App, key: KeyEvent) -> Result<()> {
    let len = app.account_filter.len();
    if len == 0 {
        if key.code == KeyCode::Esc || key.code == KeyCode::Enter {
            app.close_modal();
        }
        return Ok(());
    }

    // Visible rows available (used for scroll clamping); approximate — renderer
    // will enforce the exact scroll, but we keep state consistent here.
    const VISIBLE: usize = 18;

    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.close_modal();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.account_filter_cursor + 1 < len {
                app.account_filter_cursor += 1;
                // Scroll down if cursor went below visible window
                if app.account_filter_cursor >= app.account_filter_scroll + VISIBLE {
                    app.account_filter_scroll = app.account_filter_cursor - VISIBLE + 1;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.account_filter_cursor > 0 {
                app.account_filter_cursor -= 1;
                if app.account_filter_cursor < app.account_filter_scroll {
                    app.account_filter_scroll = app.account_filter_cursor;
                }
            }
        }
        KeyCode::Char(' ') => {
            // Toggle the currently highlighted account
            if let Some(entry) = app.account_filter.get_mut(app.account_filter_cursor) {
                entry.1 = !entry.1;
            }
        }
        // 'a' = check all
        KeyCode::Char('a') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            for entry in app.account_filter.iter_mut() {
                entry.1 = true;
            }
        }
        // 'u' = uncheck all (u for "uncheck all" / "none")
        KeyCode::Char('u') => {
            for entry in app.account_filter.iter_mut() {
                entry.1 = false;
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Modal: Tx Account Filter ──────────────────────────────────────────────────

fn handle_tx_account_filter(app: &mut App, key: KeyEvent) -> Result<()> {
    let len = app.tx_account_filter.len();
    if len == 0 {
        if key.code == KeyCode::Esc || key.code == KeyCode::Enter {
            app.close_modal();
        }
        return Ok(());
    }

    const VISIBLE: usize = 18;

    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.close_modal();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.tx_account_filter_cursor + 1 < len {
                app.tx_account_filter_cursor += 1;
                if app.tx_account_filter_cursor >= app.tx_account_filter_scroll + VISIBLE {
                    app.tx_account_filter_scroll = app.tx_account_filter_cursor - VISIBLE + 1;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.tx_account_filter_cursor > 0 {
                app.tx_account_filter_cursor -= 1;
                if app.tx_account_filter_cursor < app.tx_account_filter_scroll {
                    app.tx_account_filter_scroll = app.tx_account_filter_cursor;
                }
            }
        }
        KeyCode::Char(' ') => {
            if let Some(entry) = app.tx_account_filter.get_mut(app.tx_account_filter_cursor) {
                entry.1 = !entry.1;
            }
        }
        KeyCode::Char('a') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            for entry in app.tx_account_filter.iter_mut() {
                entry.1 = true;
            }
        }
        KeyCode::Char('u') => {
            for entry in app.tx_account_filter.iter_mut() {
                entry.1 = false;
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Screen: Reports ───────────────────────────────────────────────────────────

fn handle_reports(app: &mut App, key: KeyEvent) {
    if app.reports_view != ReportsView::Breakdown {
        return;
    }

    const VISIBLE: usize = 20;

    let filter = app.active_account_filter();
    let period = app.breakdown_period.clone();
    let breakdown_len = app
        .ledger
        .category_breakdown(&app.config.currency, period.start(), period.end(), filter.as_ref())
        .len();

    match key.code {
        // Period navigation
        KeyCode::Left | KeyCode::Char('h') => {
            app.breakdown_period = app.breakdown_period.prev();
            app.breakdown_cursor = 0;
            app.breakdown_scroll = 0;
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.breakdown_period = app.breakdown_period.next();
            app.breakdown_cursor = 0;
            app.breakdown_scroll = 0;
        }
        // Switch period mode
        KeyCode::Char('m') => {
            app.breakdown_period = app.breakdown_period.as_month();
            app.breakdown_cursor = 0;
            app.breakdown_scroll = 0;
        }
        KeyCode::Char('y') => {
            app.breakdown_period = app.breakdown_period.as_year();
            app.breakdown_cursor = 0;
            app.breakdown_scroll = 0;
        }
        // Category list navigation
        KeyCode::Down | KeyCode::Char('j') => {
            if breakdown_len > 0 && app.breakdown_cursor + 1 < breakdown_len {
                app.breakdown_cursor += 1;
                if app.breakdown_cursor >= app.breakdown_scroll + VISIBLE {
                    app.breakdown_scroll = app.breakdown_cursor - VISIBLE + 1;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.breakdown_cursor > 0 {
                app.breakdown_cursor -= 1;
                if app.breakdown_cursor < app.breakdown_scroll {
                    app.breakdown_scroll = app.breakdown_cursor;
                }
            }
        }
        // Open category transactions modal
        KeyCode::Enter => {
            if breakdown_len > 0 {
                let filter = app.active_account_filter();
                let period = app.breakdown_period.clone();
                let bd = app.ledger.category_breakdown(
                    &app.config.currency,
                    period.start(),
                    period.end(),
                    filter.as_ref(),
                );
                if let Some((category, _)) = bd.get(app.breakdown_cursor) {
                    app.category_tx_category = category.clone();
                    app.category_tx_cursor = 0;
                    app.category_tx_scroll = 0;
                    app.open_modal(Modal::CategoryTransactions);
                }
            }
        }
        _ => {}
    }
}

// ── Modal: Category Transactions ─────────────────────────────────────────────

fn handle_category_transactions(app: &mut App, key: KeyEvent) -> Result<()> {
    const VISIBLE: usize = 18;

    let period = app.breakdown_period.clone();
    let total = app
        .ledger
        .transactions_for_category(
            &app.config.currency,
            period.start(),
            period.end(),
            &app.category_tx_category.clone(),
        )
        .len();

    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.close_modal();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if total > 0 && app.category_tx_cursor + 1 < total {
                app.category_tx_cursor += 1;
                if app.category_tx_cursor >= app.category_tx_scroll + VISIBLE {
                    app.category_tx_scroll = app.category_tx_cursor - VISIBLE + 1;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.category_tx_cursor > 0 {
                app.category_tx_cursor -= 1;
                if app.category_tx_cursor < app.category_tx_scroll {
                    app.category_tx_scroll = app.category_tx_cursor;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
