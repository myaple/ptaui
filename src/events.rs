use crate::app::{
    AddAccountField, AddTxField, App, CsvImportStep, CsvMappingField, GitStatus, Modal,
    ReportsView, Screen, StartupGitChoice, ACCOUNT_TYPES,
};
use anyhow::Result;
use crossterm::event::KeyModifiers;
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

        // Reconcile mode absorbs all input on the Transactions screen
        if app.reconcile_mode && app.screen == Screen::Transactions {
            return handle_reconcile_mode(app, key);
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
            KeyCode::Char('i') if app.screen == Screen::Transactions => {
                app.open_modal(Modal::CsvImport);
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
        Some(Modal::DeleteTransaction) => handle_delete_tx(app, key),
        Some(Modal::CsvImport) => handle_csv_import(app, key),
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
    let filter = app.active_tx_account_filter();
    let max = app
        .ledger
        .transactions
        .iter()
        .filter(|txn| match &filter {
            None => true,
            Some(set) => txn.postings.iter().any(|p| set.contains(&p.account)),
        })
        .count()
        .saturating_sub(1);
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
        KeyCode::Char('d') => {
            if !app.ledger.transactions.is_empty() {
                app.open_modal(Modal::DeleteTransaction);
            }
        }
        // Enter reconcile mode (uppercase R = Shift+R)
        KeyCode::Char('R') => {
            app.reconcile_mode = true;
            app.reconcile_selected.clear();
        }
        _ => {}
    }
}

// ── Reconcile mode ────────────────────────────────────────────────────────────

fn handle_reconcile_mode(app: &mut App, key: KeyEvent) -> Result<()> {
    let filter = app.active_tx_account_filter();
    let mut sorted: Vec<&crate::beancount::parser::Transaction> = app
        .ledger
        .transactions
        .iter()
        .filter(|txn| match &filter {
            None => true,
            Some(set) => txn.postings.iter().any(|p| set.contains(&p.account)),
        })
        .collect();
    sorted.sort_by(|a, b| b.date.cmp(&a.date));
    let max = sorted.len().saturating_sub(1);

    match key.code {
        KeyCode::Esc => {
            app.reconcile_mode = false;
            app.reconcile_selected.clear();
        }
        // Navigation — same as normal
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
        // Space: toggle current transaction in multi-select
        KeyCode::Char(' ') => {
            if !sorted.is_empty() {
                let selected = app.tx_selected.min(max);
                let line = sorted[selected].line;
                if app.reconcile_selected.contains(&line) {
                    app.reconcile_selected.remove(&line);
                } else {
                    app.reconcile_selected.insert(line);
                }
            }
        }
        // r: reconcile selected (or current) transactions
        KeyCode::Char('r') => match app.commit_reconcile_transactions(true) {
            Ok(()) => {}
            Err(e) => app.status_message = Some(format!("Error: {}", e)),
        },
        // u: unreconcile selected (or current) transactions
        KeyCode::Char('u') => match app.commit_reconcile_transactions(false) {
            Ok(()) => {}
            Err(e) => app.status_message = Some(format!("Error: {}", e)),
        },
        _ => {}
    }
    Ok(())
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
                        AddTxField::Payee => &form.payee,
                        AddTxField::Category => &form.category,
                        AddTxField::Account => &form.account,
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
                        AddTxField::Payee => &form.payee,
                        AddTxField::Category => &form.category,
                        AddTxField::Account => &form.account,
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
        KeyCode::Left | KeyCode::Char('h') if form.focused == AddAccountField::AccountType => {
            form.type_idx = if form.type_idx == 0 {
                ACCOUNT_TYPES.len() - 1
            } else {
                form.type_idx - 1
            };
        }
        KeyCode::Right | KeyCode::Char('l') if form.focused == AddAccountField::AccountType => {
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
            AddAccountField::SubName => {
                form.sub_name.pop();
            }
            AddAccountField::Currencies => {
                form.currencies.pop();
            }
            AddAccountField::Date => {
                form.date.pop();
            }
            AddAccountField::InitialBalance => {
                form.initial_balance.pop();
            }
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

// ── Modal: Delete Transaction ─────────────────────────────────────────────────

fn handle_delete_tx(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            app.close_modal();
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::Char('h') | KeyCode::Char('l') => {
            app.delete_tx_confirm = !app.delete_tx_confirm;
        }
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
            if app.delete_tx_confirm {
                app.close_modal();
                match app.commit_delete_transaction() {
                    Ok(()) => app.status_message = Some("Transaction deleted.".to_string()),
                    Err(e) => app.status_message = Some(format!("Error: {}", e)),
                }
            } else {
                app.close_modal();
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
        .category_breakdown(
            &app.config.currency,
            period.start(),
            period.end(),
            filter.as_ref(),
        )
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

// ── Modal: CSV Import ────────────────────────────────────────────────────────

/// Approximate visible rows in the CSV review table.
const CSV_REVIEW_PAGE: usize = 15;

fn handle_csv_import(app: &mut App, key: KeyEvent) -> Result<()> {
    let state = match app.csv_import_state.as_mut() {
        Some(s) => s,
        None => return Ok(()),
    };

    // Clear error on any key press
    state.error = None;

    match state.step {
        CsvImportStep::FilePath => handle_csv_file_path(app, key),
        CsvImportStep::AccountSelect => handle_csv_account_select(app, key),
        CsvImportStep::ColumnMapping => handle_csv_column_mapping(app, key),
        CsvImportStep::Review => handle_csv_review(app, key),
    }
}

fn handle_csv_file_path(app: &mut App, key: KeyEvent) -> Result<()> {
    let state = app.csv_import_state.as_mut().unwrap();
    match key.code {
        KeyCode::Esc => {
            app.close_modal();
        }
        KeyCode::Enter => {
            // Expand ~ and try to read the CSV
            let path_str = state.file_path.trim().to_string();
            let expanded = if let Some(stripped) = path_str.strip_prefix("~/") {
                if let Some(home) = dirs::home_dir() {
                    home.join(stripped)
                } else {
                    std::path::PathBuf::from(&path_str)
                }
            } else {
                std::path::PathBuf::from(&path_str)
            };

            match crate::beancount::csv::read_csv(&expanded) {
                Ok((headers, rows)) => {
                    let state = app.csv_import_state.as_mut().unwrap();
                    let num_cols = headers.len();
                    state.headers = headers;
                    state.raw_rows = rows;
                    state.date_col = 0;
                    state.payee_col = if num_cols > 1 { 1 } else { 0 };
                    state.amount_col = if num_cols > 2 { 2 } else { 0 };
                    state.step = CsvImportStep::AccountSelect;
                }
                Err(e) => {
                    let state = app.csv_import_state.as_mut().unwrap();
                    state.error = Some(format!("Failed to read CSV: {}", e));
                }
            }
        }
        KeyCode::Tab => {
            // Filesystem path tab-completion
            let completed = tab_complete_path(&state.file_path);
            state.file_path = completed;
        }
        KeyCode::Backspace => {
            state.file_path.pop();
        }
        KeyCode::Char(c) => {
            state.file_path.push(c);
        }
        _ => {}
    }
    Ok(())
}

/// Tab-complete a filesystem path. Expands `~`, finds matching entries in the
/// parent directory, and completes to the longest common prefix. If there's a
/// single match and it's a directory, appends `/`.
fn tab_complete_path(input: &str) -> String {
    let expanded = if let Some(stripped) = input.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(stripped).to_string_lossy().to_string()
        } else {
            input.to_string()
        }
    } else if input == "~" {
        if let Some(home) = dirs::home_dir() {
            home.to_string_lossy().to_string()
        } else {
            input.to_string()
        }
    } else {
        input.to_string()
    };

    let path = std::path::Path::new(&expanded);

    // Determine the directory to list and the prefix to match
    let (dir, prefix) = if path.is_dir() && expanded.ends_with('/') {
        (path.to_path_buf(), String::new())
    } else {
        let dir = path.parent().unwrap_or(std::path::Path::new("."));
        let prefix = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        (dir.to_path_buf(), prefix)
    };

    // Read directory entries matching the prefix
    let entries = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => return input.to_string(),
    };

    let mut matches: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) {
            matches.push(name);
        }
    }

    if matches.is_empty() {
        return input.to_string();
    }

    matches.sort();

    // Find longest common prefix among matches
    let lcp = longest_common_prefix(&matches);

    // Build the completed path
    let completed_path = dir.join(&lcp);
    let mut result = completed_path.to_string_lossy().to_string();

    // If single match and it's a directory, append /
    if matches.len() == 1 && completed_path.is_dir() && !result.ends_with('/') {
        result.push('/');
    }

    // Restore ~/  prefix if original input used it
    if input.starts_with("~/") || input == "~" {
        if let Some(home) = dirs::home_dir() {
            let home_str = home.to_string_lossy().to_string();
            if let Some(rest) = result.strip_prefix(&home_str) {
                return format!("~{}", rest);
            }
        }
    }

    result
}

fn longest_common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    let first = &strings[0];
    let mut len = first.len();
    for s in &strings[1..] {
        len = len.min(s.len());
        for (i, (a, b)) in first.chars().zip(s.chars()).enumerate() {
            if a != b {
                len = len.min(i);
                break;
            }
        }
    }
    first[..len].to_string()
}

fn handle_csv_account_select(app: &mut App, key: KeyEvent) -> Result<()> {
    let state = app.csv_import_state.as_mut().unwrap();
    match key.code {
        KeyCode::Esc => {
            state.step = CsvImportStep::FilePath;
        }
        KeyCode::Tab => {
            // Autocomplete
            let suggestions = state.filtered_account_suggestions();
            if !suggestions.is_empty() && !suggestions.contains(&state.account.as_str()) {
                state.account = suggestions[0].to_string();
            }
        }
        KeyCode::Enter => {
            if state.account.trim().is_empty() {
                state.error = Some("Account cannot be empty".to_string());
            } else {
                state.step = CsvImportStep::ColumnMapping;
            }
        }
        KeyCode::Backspace => {
            state.account.pop();
        }
        KeyCode::Char(c) => {
            state.account.push(c);
        }
        _ => {}
    }
    Ok(())
}

fn handle_csv_column_mapping(app: &mut App, key: KeyEvent) -> Result<()> {
    let state = app.csv_import_state.as_mut().unwrap();
    let num_cols = state.num_cols();
    if num_cols == 0 {
        state.error = Some("CSV has no columns".to_string());
        return Ok(());
    }

    match key.code {
        KeyCode::Esc => {
            state.step = CsvImportStep::AccountSelect;
        }
        KeyCode::Tab | KeyCode::Down => {
            state.mapping_focused = state.mapping_focused.next();
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.mapping_focused = state.mapping_focused.prev();
        }
        KeyCode::Left => match state.mapping_focused {
            CsvMappingField::Date => {
                state.date_col = if state.date_col == 0 {
                    num_cols - 1
                } else {
                    state.date_col - 1
                };
            }
            CsvMappingField::Payee => {
                state.payee_col = if state.payee_col == 0 {
                    num_cols - 1
                } else {
                    state.payee_col - 1
                };
            }
            CsvMappingField::Amount => {
                state.amount_col = if state.amount_col == 0 {
                    num_cols - 1
                } else {
                    state.amount_col - 1
                };
            }
            CsvMappingField::Debit => {
                if let Some(col) = state.debit_col {
                    state.debit_col = Some(if col == 0 { num_cols - 1 } else { col - 1 });
                }
            }
            CsvMappingField::Credit => {
                if let Some(col) = state.credit_col {
                    state.credit_col = Some(if col == 0 { num_cols - 1 } else { col - 1 });
                }
            }
            CsvMappingField::DateFormat | CsvMappingField::Negate => {}
        },
        KeyCode::Right => match state.mapping_focused {
            CsvMappingField::Date => {
                state.date_col = (state.date_col + 1) % num_cols;
            }
            CsvMappingField::Payee => {
                state.payee_col = (state.payee_col + 1) % num_cols;
            }
            CsvMappingField::Amount => {
                state.amount_col = (state.amount_col + 1) % num_cols;
            }
            CsvMappingField::Debit => {
                if let Some(col) = state.debit_col {
                    state.debit_col = Some((col + 1) % num_cols);
                }
            }
            CsvMappingField::Credit => {
                if let Some(col) = state.credit_col {
                    state.credit_col = Some((col + 1) % num_cols);
                }
            }
            CsvMappingField::DateFormat | CsvMappingField::Negate => {}
        },
        KeyCode::Char(' ') if state.mapping_focused == CsvMappingField::Negate => {
            state.negate_amounts = !state.negate_amounts;
        }
        KeyCode::Char(' ') if state.mapping_focused == CsvMappingField::Debit => {
            if state.debit_col.is_some() {
                state.debit_col = None;
                state.credit_col = None; // disable credit too when debit is disabled
            } else {
                state.debit_col = Some(0);
            }
        }
        KeyCode::Char(' ') if state.mapping_focused == CsvMappingField::Credit => {
            if state.debit_col.is_some() {
                state.credit_col = if state.credit_col.is_some() { None } else { Some(0) };
            }
        }
        KeyCode::Backspace if state.mapping_focused == CsvMappingField::DateFormat => {
            state.date_format.pop();
        }
        KeyCode::Char(c) if state.mapping_focused == CsvMappingField::DateFormat => {
            state.date_format.push(c);
        }
        KeyCode::Enter => {
            // Try to parse rows with current mapping
            let mapping = crate::beancount::csv::ColumnMapping {
                date_col: state.date_col,
                payee_col: state.payee_col,
                amount_col: state.amount_col,
                debit_col: state.debit_col,
                credit_col: state.credit_col,
            };
            match crate::beancount::csv::parse_rows(
                &state.raw_rows,
                &mapping,
                &state.date_format,
                state.negate_amounts,
            ) {
                Ok(mut rows) => {
                    // Run deduplication
                    let dest_account = state.account.clone();
                    crate::beancount::csv::detect_duplicates(
                        &mut rows,
                        &app.ledger.transactions,
                        &dest_account,
                    );
                    let state = app.csv_import_state.as_mut().unwrap();
                    state.rows = rows;
                    state.cursor = 0;
                    state.scroll = 0;
                    state.editing_category = false;
                    // Apply payee defaults for categories
                    apply_csv_payee_defaults(app);
                    let state = app.csv_import_state.as_mut().unwrap();
                    state.step = CsvImportStep::Review;
                }
                Err(e) => {
                    state.error = Some(format!("Parse error: {}", e));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// For each CSV row, look up the most recent existing transaction with the same
/// payee and auto-fill the category.
fn apply_csv_payee_defaults(app: &mut App) {
    let mut payee_categories: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut sorted_txns: Vec<&crate::beancount::parser::Transaction> =
        app.ledger.transactions.iter().collect();
    sorted_txns.sort_by(|a, b| a.date.cmp(&b.date));

    for txn in sorted_txns {
        if let Some(ref payee) = txn.payee {
            let payee_lower = payee.to_lowercase();
            for posting in &txn.postings {
                if posting.account.starts_with("Expenses") || posting.account.starts_with("Income")
                {
                    payee_categories.insert(payee_lower.clone(), posting.account.clone());
                }
            }
        }
    }

    if let Some(state) = app.csv_import_state.as_mut() {
        for row in &mut state.rows {
            if row.category.is_empty() {
                let payee_lower = row.payee.to_lowercase();
                if let Some(cat) = payee_categories.get(&payee_lower) {
                    row.category = cat.clone();
                }
            }
        }
    }
}

fn handle_csv_review(app: &mut App, key: KeyEvent) -> Result<()> {
    let state = app.csv_import_state.as_mut().unwrap();

    // If editing category, handle text input
    if state.editing_category {
        match key.code {
            KeyCode::Esc => {
                state.editing_category = false;
            }
            KeyCode::Enter => {
                state.editing_category = false;
            }
            KeyCode::Tab => {
                // Autocomplete category - collect first to avoid borrow conflict
                let first_suggestion = state
                    .filtered_category_suggestions()
                    .first()
                    .map(|s| s.to_string());
                if let (Some(suggestion), Some(row)) =
                    (first_suggestion, state.rows.get_mut(state.cursor))
                {
                    if row.category != suggestion {
                        row.category = suggestion;
                    }
                }
                state.editing_category = false;
            }
            KeyCode::Backspace => {
                if let Some(row) = state.rows.get_mut(state.cursor) {
                    row.category.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(row) = state.rows.get_mut(state.cursor) {
                    row.category.push(c);
                }
            }
            _ => {}
        }
        return Ok(());
    }

    match key.code {
        KeyCode::Esc => {
            state.step = CsvImportStep::ColumnMapping;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !state.rows.is_empty() && state.cursor < state.rows.len() - 1 {
                state.cursor += 1;
                if state.cursor >= state.scroll + CSV_REVIEW_PAGE {
                    state.scroll = state.cursor - CSV_REVIEW_PAGE + 1;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.cursor > 0 {
                state.cursor -= 1;
                if state.cursor < state.scroll {
                    state.scroll = state.cursor;
                }
            }
        }
        KeyCode::Char(' ') => {
            // Toggle include/exclude
            if let Some(row) = state.rows.get_mut(state.cursor) {
                row.include = !row.include;
            }
        }
        KeyCode::Char('e') | KeyCode::Tab => {
            // Edit category for current row
            state.editing_category = true;
        }
        KeyCode::Char('c') => {
            // Apply category from current row to all rows with same payee that have no category
            if let Some(current) = state.rows.get(state.cursor) {
                let payee = current.payee.to_lowercase();
                let category = current.category.clone();
                if !category.is_empty() {
                    for row in &mut state.rows {
                        if row.payee.to_lowercase() == payee && row.category.is_empty() {
                            row.category = category.clone();
                        }
                    }
                }
            }
        }
        KeyCode::Char('C') => {
            // Apply category from current row to ALL rows with same payee
            if let Some(current) = state.rows.get(state.cursor) {
                let payee = current.payee.to_lowercase();
                let category = current.category.clone();
                if !category.is_empty() {
                    for row in &mut state.rows {
                        if row.payee.to_lowercase() == payee {
                            row.category = category.clone();
                        }
                    }
                }
            }
        }
        KeyCode::Enter => {
            // Commit import
            match app.commit_csv_import() {
                Ok(()) => {
                    app.close_modal();
                }
                Err(e) => {
                    if let Some(state) = app.csv_import_state.as_mut() {
                        state.error = Some(e.to_string());
                    }
                }
            }
            return Ok(());
        }
        _ => {}
    }
    Ok(())
}
