use crate::beancount::parser::Ledger;
use crate::beancount::validator::{bean_check, CheckResult};
use crate::beancount::writer::{
    append_account_open, append_transaction, replace_transaction, NewPosting, NewTransaction,
};
use crate::config::Config;
use crate::git;
use anyhow::{Context, Result};
use chrono::{Datelike, Local};
use rust_decimal::Decimal;
use std::path::PathBuf;
use std::str::FromStr;

// ── Screens ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    /// Shown on first run or when the beancount dir is unversioned.
    Startup,
    Dashboard,
    Transactions,
    Reports,
}

/// Overlay modals rendered on top of the active screen.
/// While a modal is open ALL key input is captured by the modal.
#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    AddTransaction,
    EditTransaction,
    AddAccount,
    AccountFilter,
    /// Transactions for a specific category in the current breakdown period.
    CategoryTransactions,
    /// Account filter for the transaction list view.
    TxAccountFilter,
    /// Confirmation prompt before deleting the selected transaction.
    DeleteTransaction,
    /// CSV import wizard (multi-step).
    CsvImport,
}

// ── Reports view state ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ReportsView {
    /// Monthly income vs expenses bar chart (existing).
    Monthly,
    /// Per-category breakdown for a specific period.
    Breakdown,
    /// Net worth over time with category trend drill-down.
    NetWorth,
}

/// The time period shown in the category breakdown view.
#[derive(Debug, Clone, PartialEq)]
pub enum BreakdownPeriod {
    Month { year: i32, month: u32 },
    Year { year: i32 },
}

impl BreakdownPeriod {
    pub fn current_month() -> Self {
        let now = Local::now().date_naive();
        Self::Month {
            year: now.year(),
            month: now.month(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Month { year, month } => format!("{:04}-{:02}", year, month),
            Self::Year { year } => format!("{:04}", year),
        }
    }

    pub fn start(&self) -> chrono::NaiveDate {
        match self {
            Self::Month { year, month } => {
                chrono::NaiveDate::from_ymd_opt(*year, *month, 1).unwrap()
            }
            Self::Year { year } => chrono::NaiveDate::from_ymd_opt(*year, 1, 1).unwrap(),
        }
    }

    /// Exclusive end date (first day after the period).
    pub fn end(&self) -> chrono::NaiveDate {
        match self {
            Self::Month { year, month } => {
                if *month == 12 {
                    chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
                } else {
                    chrono::NaiveDate::from_ymd_opt(*year, month + 1, 1).unwrap()
                }
            }
            Self::Year { year } => chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap(),
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Month { year, month } => {
                if *month == 1 {
                    Self::Month {
                        year: year - 1,
                        month: 12,
                    }
                } else {
                    Self::Month {
                        year: *year,
                        month: month - 1,
                    }
                }
            }
            Self::Year { year } => Self::Year { year: year - 1 },
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Month { year, month } => {
                if *month == 12 {
                    Self::Month {
                        year: year + 1,
                        month: 1,
                    }
                } else {
                    Self::Month {
                        year: *year,
                        month: month + 1,
                    }
                }
            }
            Self::Year { year } => Self::Year { year: year + 1 },
        }
    }

    /// Switch to monthly mode, keeping the same year and month (or Jan for year mode).
    pub fn as_month(&self) -> Self {
        match self {
            Self::Month { .. } => self.clone(),
            Self::Year { year } => Self::Month {
                year: *year,
                month: 1,
            },
        }
    }

    /// Switch to yearly mode, keeping the same year.
    pub fn as_year(&self) -> Self {
        match self {
            Self::Month { year, .. } => Self::Year { year: *year },
            Self::Year { .. } => self.clone(),
        }
    }

    pub fn is_month(&self) -> bool {
        matches!(self, Self::Month { .. })
    }
}

// ── Startup state ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum GitStatus {
    /// Directory is inside a git repo — all good.
    Controlled,
    /// Directory exists but is not a git repo.
    Uncontrolled { dir: PathBuf },
    /// The beancount file (or its directory) doesn't exist yet.
    NoFile,
}

/// Which button is selected on the startup screen's git prompt.
#[derive(Debug, Clone, PartialEq)]
pub enum StartupGitChoice {
    InitRepo,
    Skip,
}

pub struct StartupState {
    /// Config was freshly created on this launch.
    pub config_just_created: bool,
    /// Path where the config lives (for display).
    pub config_path: PathBuf,
    /// Whether the beancount file's directory is under git.
    pub git_status: GitStatus,
    /// Current button selection for the git init prompt.
    pub git_choice: StartupGitChoice,
    /// Feedback from trying to init a repo.
    pub git_init_result: Option<String>,
}

impl StartupState {
    /// True only when there is actually something to show to the user.
    pub fn needs_display(&self) -> bool {
        self.config_just_created || matches!(self.git_status, GitStatus::Uncontrolled { .. })
    }
}

// ── Add Transaction form ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AddTxField {
    Date,
    Payee,
    Narration,
    Category,
    Account,
    Amount,
    Currency,
    Confirm,
}

impl AddTxField {
    pub fn next(&self) -> Self {
        match self {
            Self::Date => Self::Payee,
            Self::Payee => Self::Narration,
            Self::Narration => Self::Category,
            Self::Category => Self::Account,
            Self::Account => Self::Amount,
            Self::Amount => Self::Currency,
            Self::Currency => Self::Confirm,
            Self::Confirm => Self::Date,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            Self::Date => Self::Confirm,
            Self::Payee => Self::Date,
            Self::Narration => Self::Payee,
            Self::Category => Self::Narration,
            Self::Account => Self::Category,
            Self::Amount => Self::Account,
            Self::Currency => Self::Amount,
            Self::Confirm => Self::Currency,
        }
    }
}

pub struct AddTxForm {
    pub date: String,
    pub payee: String,
    pub narration: String,
    /// The expense/income category account (e.g. Expenses:Food:Restaurant).
    pub category: String,
    /// The payment account money comes out of (e.g. Liabilities:CreditCard:CapitalOne).
    pub account: String,
    pub amount: String,
    pub currency: String,
    pub focused: AddTxField,
    pub account_suggestions: Vec<String>,
    pub payee_suggestions: Vec<String>,
    pub error: Option<String>,
}

impl AddTxForm {
    pub fn new(currency: &str, accounts: &[String], payees: &[String]) -> Self {
        Self {
            date: Local::now().date_naive().format("%Y-%m-%d").to_string(),
            payee: String::new(),
            narration: String::new(),
            category: String::new(),
            account: String::new(),
            amount: String::new(),
            currency: currency.to_string(),
            focused: AddTxField::Date,
            account_suggestions: accounts.to_vec(),
            payee_suggestions: payees.to_vec(),
            error: None,
        }
    }

    pub fn current_field_mut(&mut self) -> &mut String {
        match self.focused {
            AddTxField::Date => &mut self.date,
            AddTxField::Payee => &mut self.payee,
            AddTxField::Narration => &mut self.narration,
            AddTxField::Category => &mut self.category,
            AddTxField::Account => &mut self.account,
            AddTxField::Amount => &mut self.amount,
            AddTxField::Currency => &mut self.currency,
            AddTxField::Confirm => &mut self.narration, // dummy
        }
    }

    pub fn autocomplete(&mut self) {
        let suggestions = self.suggestions_for_current();
        if suggestions.is_empty() {
            return;
        }
        let prefix = match self.focused {
            AddTxField::Payee => &self.payee,
            AddTxField::Category => &self.category,
            AddTxField::Account => &self.account,
            _ => return,
        };
        if let Some(suggestion) = suggestions
            .iter()
            .find(|s| !s.eq_ignore_ascii_case(prefix))
            .or_else(|| suggestions.first())
        {
            let suggestion = suggestion.clone();
            match self.focused {
                AddTxField::Payee => self.payee = suggestion,
                AddTxField::Category => self.category = suggestion,
                AddTxField::Account => self.account = suggestion,
                _ => {}
            }
        }
    }

    pub fn suggestions_for_current(&self) -> Vec<String> {
        match self.focused {
            AddTxField::Payee => {
                let prefix = &self.payee;
                if prefix.is_empty() {
                    return vec![];
                }
                self.payee_suggestions
                    .iter()
                    .filter(|p| p.to_lowercase().contains(&prefix.to_lowercase()))
                    .take(8)
                    .cloned()
                    .collect()
            }
            // Category: show Expenses:* and Income:* accounts first, then others.
            AddTxField::Category => {
                let prefix = &self.category;
                if prefix.is_empty() {
                    return vec![];
                }
                let lower = prefix.to_lowercase();
                let mut primary: Vec<String> = self
                    .account_suggestions
                    .iter()
                    .filter(|a| {
                        let al = a.to_lowercase();
                        (al.starts_with("expenses:") || al.starts_with("income:"))
                            && al.contains(&lower)
                    })
                    .cloned()
                    .collect();
                let secondary: Vec<String> = self
                    .account_suggestions
                    .iter()
                    .filter(|a| {
                        let al = a.to_lowercase();
                        !(al.starts_with("expenses:") || al.starts_with("income:"))
                            && al.contains(&lower)
                    })
                    .cloned()
                    .collect();
                primary.extend(secondary);
                primary.truncate(8);
                primary
            }
            // Account: show Assets:* and Liabilities:* first, then others.
            AddTxField::Account => {
                let prefix = &self.account;
                if prefix.is_empty() {
                    return vec![];
                }
                let lower = prefix.to_lowercase();
                let mut primary: Vec<String> = self
                    .account_suggestions
                    .iter()
                    .filter(|a| {
                        let al = a.to_lowercase();
                        (al.starts_with("assets:") || al.starts_with("liabilities:"))
                            && al.contains(&lower)
                    })
                    .cloned()
                    .collect();
                let secondary: Vec<String> = self
                    .account_suggestions
                    .iter()
                    .filter(|a| {
                        let al = a.to_lowercase();
                        !(al.starts_with("assets:") || al.starts_with("liabilities:"))
                            && al.contains(&lower)
                    })
                    .cloned()
                    .collect();
                primary.extend(secondary);
                primary.truncate(8);
                primary
            }
            _ => vec![],
        }
    }
}

// ── Add Account form ──────────────────────────────────────────────────────────

pub const ACCOUNT_TYPES: &[&str] = &["Assets", "Liabilities", "Income", "Expenses", "Equity"];

#[derive(Debug, Clone, PartialEq)]
pub enum AddAccountField {
    AccountType,
    SubName,
    Currencies,
    Date,
    InitialBalance,
    Confirm,
}

impl AddAccountField {
    pub fn next(&self) -> Self {
        match self {
            Self::AccountType => Self::SubName,
            Self::SubName => Self::Currencies,
            Self::Currencies => Self::Date,
            Self::Date => Self::InitialBalance,
            Self::InitialBalance => Self::Confirm,
            Self::Confirm => Self::AccountType,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            Self::AccountType => Self::Confirm,
            Self::SubName => Self::AccountType,
            Self::Currencies => Self::SubName,
            Self::Date => Self::Currencies,
            Self::InitialBalance => Self::Date,
            Self::Confirm => Self::InitialBalance,
        }
    }
}

pub struct AddAccountForm {
    /// Index into ACCOUNT_TYPES
    pub type_idx: usize,
    /// Sub-path after the type, e.g. "Checking" or "Food:Restaurants"
    pub sub_name: String,
    /// Space-separated currencies, e.g. "USD"
    pub currencies: String,
    pub date: String,
    /// Optional opening balance amount (blank = no opening entry)
    pub initial_balance: String,
    pub focused: AddAccountField,
    pub error: Option<String>,
}

impl AddAccountForm {
    pub fn new(default_currency: &str) -> Self {
        Self {
            type_idx: 0,
            sub_name: String::new(),
            initial_balance: String::new(),
            currencies: default_currency.to_string(),
            date: Local::now().date_naive().format("%Y-%m-%d").to_string(),
            focused: AddAccountField::AccountType,
            error: None,
        }
    }

    pub fn account_name(&self) -> String {
        let t = ACCOUNT_TYPES[self.type_idx];
        if self.sub_name.trim().is_empty() {
            t.to_string()
        } else {
            format!("{}:{}", t, self.sub_name.trim())
        }
    }
}

// ── CSV Import ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CsvImportStep {
    FilePath,
    AccountSelect,
    ColumnMapping,
    Review,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CsvMappingField {
    Date,
    Payee,
    AmountMode,
    Amount,
    Debit,
    Credit,
    DateFormat,
    Negate,
}

#[derive(Debug, Clone)]
pub struct CsvImportState {
    pub step: CsvImportStep,
    pub file_path: String,
    pub account: String,
    pub account_suggestions: Vec<String>,
    pub headers: Vec<String>,
    pub raw_rows: Vec<Vec<String>>,
    pub date_col: usize,
    pub payee_col: usize,
    pub amount_col: usize,
    pub use_debit_credit: bool,
    pub debit_col: Option<usize>,
    pub credit_col: Option<usize>,
    pub mapping_focused: CsvMappingField,
    pub date_format: String,
    pub negate_amounts: bool,
    pub rows: Vec<crate::beancount::csv::CsvRow>,
    pub cursor: usize,
    pub scroll: usize,
    pub category_suggestions: Vec<String>,
    pub error: Option<String>,
    pub editing_category: bool,
    pub filter_no_category: bool,
    pub sort_by_category: bool,
}

impl CsvImportState {
    pub fn new(account_suggestions: Vec<String>, category_suggestions: Vec<String>) -> Self {
        Self {
            step: CsvImportStep::FilePath,
            file_path: String::new(),
            account: String::new(),
            account_suggestions,
            headers: Vec::new(),
            raw_rows: Vec::new(),
            date_col: 0,
            payee_col: 1,
            amount_col: 2,
            use_debit_credit: false,
            debit_col: None,
            credit_col: None,
            mapping_focused: CsvMappingField::Date,
            date_format: "%m/%d/%Y".to_string(),
            negate_amounts: false,
            rows: Vec::new(),
            cursor: 0,
            scroll: 0,
            category_suggestions,
            error: None,
            editing_category: false,
            filter_no_category: false,
            sort_by_category: false,
        }
    }

    /// Filter account suggestions by prefix (case-insensitive).
    pub fn filtered_account_suggestions(&self) -> Vec<&str> {
        if self.account.is_empty() {
            return self
                .account_suggestions
                .iter()
                .map(|s| s.as_str())
                .take(10)
                .collect();
        }
        let lower = self.account.to_lowercase();
        self.account_suggestions
            .iter()
            .filter(|s| s.to_lowercase().contains(&lower))
            .map(|s| s.as_str())
            .take(10)
            .collect()
    }

    /// Returns row indices that pass the current filter, in display order.
    pub fn visible_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.rows.len())
            .filter(|&i| {
                if self.filter_no_category {
                    self.rows[i].category.is_empty()
                } else {
                    true
                }
            })
            .collect();

        if self.sort_by_category {
            indices.sort_by(|&a, &b| {
                self.rows[a]
                    .category
                    .cmp(&self.rows[b].category)
                    .then_with(|| self.rows[a].date.cmp(&self.rows[b].date))
            });
        }

        indices
    }

    /// Filter category suggestions for the current row.
    pub fn filtered_category_suggestions(&self) -> Vec<&str> {
        let actual_idx = self.visible_indices().get(self.cursor).copied();
        if let Some(row_idx) = actual_idx {
            if let Some(row) = self.rows.get(row_idx) {
                if row.category.is_empty() {
                    return self
                        .category_suggestions
                        .iter()
                        .map(|s| s.as_str())
                        .take(10)
                        .collect();
                }
                let lower = row.category.to_lowercase();
                return self
                    .category_suggestions
                    .iter()
                    .filter(|s| s.to_lowercase().contains(&lower))
                    .map(|s| s.as_str())
                    .take(10)
                    .collect();
            }
        }
        Vec::new()
    }

    /// Number of columns in the CSV.
    pub fn num_cols(&self) -> usize {
        self.headers.len()
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub config: Config,
    pub ledger: Ledger,
    /// True when the beancount file was found and successfully read.
    pub file_found: bool,
    pub screen: Screen,
    /// Currently open modal overlay (if any). Captures all key input.
    pub modal: Option<Modal>,
    pub startup: StartupState,
    /// Scroll offset for the transaction list (index of first visible item in sorted order).
    pub tx_scroll: usize,
    /// Index of the currently selected transaction in the sorted (reverse-chrono) list.
    pub tx_selected: usize,
    /// When editing a transaction, the 0-based start line of that transaction in the file.
    pub edit_tx_orig_line: Option<usize>,
    pub dashboard_scroll: usize,
    pub add_tx_form: Option<AddTxForm>,
    pub add_account_form: Option<AddAccountForm>,
    pub status_message: Option<String>,
    pub check_errors: Vec<String>,
    pub running: bool,
    /// (account_name, is_checked) — persists across modal open/close.
    pub account_filter: Vec<(String, bool)>,
    /// Cursor position inside the account filter modal.
    pub account_filter_cursor: usize,
    /// Scroll offset inside the account filter modal list.
    pub account_filter_scroll: usize,
    // ── Reports state ──────────────────────────────────────────────────────
    /// Whether to show the monthly bar chart or the per-category breakdown.
    pub reports_view: ReportsView,
    /// The period displayed in the category breakdown view.
    pub breakdown_period: BreakdownPeriod,
    /// Selected category row index in the breakdown list.
    pub breakdown_cursor: usize,
    /// Scroll offset for the breakdown list.
    pub breakdown_scroll: usize,
    /// Cursor for the category list in the NetWorth view.
    pub networth_cursor: usize,
    /// Scroll offset for the category list in the NetWorth view.
    pub networth_scroll: usize,
    /// The category name currently open in the CategoryTransactions modal.
    pub category_tx_category: String,
    /// Scroll offset inside the category transactions modal.
    pub category_tx_scroll: usize,
    /// Selected transaction index inside the category transactions modal.
    pub category_tx_cursor: usize,
    /// (account_name, is_checked) — filter for the transaction list view.
    pub tx_account_filter: Vec<(String, bool)>,
    /// Cursor position inside the tx account filter modal.
    pub tx_account_filter_cursor: usize,
    /// Scroll offset inside the tx account filter modal list.
    pub tx_account_filter_scroll: usize,
    /// True = "Yes" selected in the delete-transaction confirmation modal.
    pub delete_tx_confirm: bool,
    /// Whether the transaction list is in reconcile mode.
    pub reconcile_mode: bool,
    /// Set of transaction file-line numbers (txn.line) currently multi-selected in reconcile mode.
    pub reconcile_selected: std::collections::HashSet<usize>,
    /// State for the CSV import wizard.
    pub csv_import_state: Option<CsvImportState>,
}

impl App {
    pub fn new(config: Config, ledger: Ledger, file_found: bool, startup: StartupState) -> Self {
        let initial_screen = if startup.needs_display() {
            Screen::Startup
        } else {
            Screen::Dashboard
        };
        let mut app = Self {
            config,
            ledger,
            file_found,
            screen: initial_screen,
            modal: None,
            startup,
            tx_scroll: 0,
            tx_selected: 0,
            edit_tx_orig_line: None,
            dashboard_scroll: 0,
            add_tx_form: None,
            add_account_form: None,
            status_message: None,
            check_errors: Vec::new(),
            running: true,
            account_filter: Vec::new(),
            account_filter_cursor: 0,
            account_filter_scroll: 0,
            reports_view: ReportsView::Monthly,
            breakdown_period: BreakdownPeriod::current_month(),
            breakdown_cursor: 0,
            breakdown_scroll: 0,
            networth_cursor: 0,
            networth_scroll: 0,
            category_tx_category: String::new(),
            category_tx_scroll: 0,
            category_tx_cursor: 0,
            tx_account_filter: Vec::new(),
            tx_account_filter_cursor: 0,
            tx_account_filter_scroll: 0,
            delete_tx_confirm: true,
            reconcile_mode: false,
            reconcile_selected: std::collections::HashSet::new(),
            csv_import_state: None,
        };
        app.rebuild_account_filter();
        app.rebuild_tx_account_filter();
        app
    }

    /// Rebuild the account filter list from current ledger Income/Expenses accounts.
    /// Existing checked state is preserved for accounts that still exist.
    pub fn rebuild_account_filter(&mut self) {
        use std::collections::{HashMap, HashSet};
        let existing: HashMap<String, bool> = self.account_filter.iter().cloned().collect();
        let mut seen: HashSet<String> = HashSet::new();
        for txn in &self.ledger.transactions {
            for posting in &txn.postings {
                let acct = &posting.account;
                if acct.starts_with("Income") || acct.starts_with("Expenses") {
                    seen.insert(acct.clone());
                }
            }
        }
        let mut sorted: Vec<String> = seen.into_iter().collect();
        sorted.sort();
        self.account_filter = sorted
            .into_iter()
            .map(|a| {
                let checked = existing.get(&a).copied().unwrap_or(true);
                (a, checked)
            })
            .collect();
        if self.account_filter_cursor >= self.account_filter.len() {
            self.account_filter_cursor = 0;
            self.account_filter_scroll = 0;
        }
    }

    /// Returns `None` when all accounts are enabled (no filtering needed),
    /// or `Some(set)` containing only the checked account names.
    pub fn active_account_filter(&self) -> Option<std::collections::HashSet<String>> {
        if self.account_filter.iter().all(|(_, c)| *c) {
            None
        } else {
            Some(
                self.account_filter
                    .iter()
                    .filter(|(_, c)| *c)
                    .map(|(a, _)| a.clone())
                    .collect(),
            )
        }
    }

    /// Rebuild the tx account filter list from all accounts appearing in transactions.
    /// Existing checked state is preserved for accounts that still exist.
    pub fn rebuild_tx_account_filter(&mut self) {
        use std::collections::{HashMap, HashSet};
        let existing: HashMap<String, bool> = self.tx_account_filter.iter().cloned().collect();
        let mut seen: HashSet<String> = HashSet::new();
        for txn in &self.ledger.transactions {
            for posting in &txn.postings {
                seen.insert(posting.account.clone());
            }
        }
        let mut sorted: Vec<String> = seen.into_iter().collect();
        sorted.sort();
        self.tx_account_filter = sorted
            .into_iter()
            .map(|a| {
                let checked = existing.get(&a).copied().unwrap_or(true);
                (a, checked)
            })
            .collect();
        if self.tx_account_filter_cursor >= self.tx_account_filter.len() {
            self.tx_account_filter_cursor = 0;
            self.tx_account_filter_scroll = 0;
        }
    }

    /// Returns `None` when all tx accounts are enabled (no filtering needed),
    /// or `Some(set)` containing only the checked account names.
    pub fn active_tx_account_filter(&self) -> Option<std::collections::HashSet<String>> {
        if self.tx_account_filter.iter().all(|(_, c)| *c) {
            None
        } else {
            Some(
                self.tx_account_filter
                    .iter()
                    .filter(|(_, c)| *c)
                    .map(|(a, _)| a.clone())
                    .collect(),
            )
        }
    }

    pub fn reload_ledger(&mut self) -> Result<()> {
        let path = self.config.resolved_beancount_file();
        if path.exists() {
            let source = std::fs::read_to_string(&path)?;
            self.ledger = crate::beancount::parser::parse(&source)?;
            self.file_found = true;
        } else {
            self.file_found = false;
        }
        self.rebuild_account_filter();
        self.rebuild_tx_account_filter();
        Ok(())
    }

    /// Create the beancount file (and its parent directory) from scratch,
    /// then init git in that directory if it isn't already a repo,
    /// and make an initial commit.
    pub fn create_beancount_file(&mut self) -> Result<()> {
        let path = self.config.resolved_beancount_file();

        // Create parent directory if needed
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("Creating directory {}", dir.display()))?;
        }

        // Write a minimal starter file
        let starter = format!(
            "; Beancount file managed by ptaui\n\
             option \"title\" \"My Finances\"\n\
             option \"operating_currency\" \"{}\"\n",
            self.config.currency
        );
        std::fs::write(&path, &starter).with_context(|| format!("Creating {}", path.display()))?;

        self.file_found = true;
        self.reload_ledger()?;

        let mut status_parts = vec![format!("Created {}", path.display())];

        // Init git if not already a repo
        let dir = path.parent().unwrap_or(&path);
        if !git::is_git_repo(dir) {
            match git::init_repo(dir) {
                Ok(_) => {
                    self.startup.git_status = GitStatus::Controlled;
                    status_parts.push("git init: done".to_string());
                }
                Err(e) => {
                    status_parts.push(format!("git init failed: {}", e));
                }
            }
        }

        // Initial commit
        if git::is_git_repo(dir) {
            self.startup.git_status = GitStatus::Controlled;
            match git::commit_file(&path, "chore: initial beancount file") {
                Ok(()) => status_parts.push("git: initial commit".to_string()),
                Err(e) => status_parts.push(format!("git: {}", e)),
            }
        }

        self.status_message = Some(status_parts.join("  |  "));
        Ok(())
    }

    pub fn account_names(&self) -> Vec<String> {
        self.ledger
            .accounts
            .iter()
            .map(|a| a.name.clone())
            .collect()
    }

    /// If the add_tx_form payee matches a known payee, auto-fill the category
    /// and account fields from that payee's most recent transaction, but only
    /// when those fields are currently empty.
    pub fn apply_payee_defaults(&mut self) {
        let payee = match self.add_tx_form.as_ref() {
            Some(f) if !f.payee.trim().is_empty() => f.payee.trim().to_string(),
            _ => return,
        };

        // Skip if all three fields are already filled in.
        let (cat_empty, acct_empty, narr_empty) = match self.add_tx_form.as_ref() {
            Some(f) => (
                f.category.is_empty(),
                f.account.is_empty(),
                f.narration.is_empty(),
            ),
            None => return,
        };
        if !cat_empty && !acct_empty && !narr_empty {
            return;
        }

        // Find the most recent transaction with this payee (case-insensitive).
        let maybe_defaults = self
            .ledger
            .transactions
            .iter()
            .filter(|t| {
                t.payee
                    .as_deref()
                    .map(|p| p.eq_ignore_ascii_case(&payee))
                    .unwrap_or(false)
            })
            .max_by_key(|t| t.date)
            .map(|txn| {
                let category = txn
                    .postings
                    .iter()
                    .find(|p| {
                        p.account.starts_with("Expenses:") || p.account.starts_with("Income:")
                    })
                    .map(|p| p.account.clone());
                let account = txn
                    .postings
                    .iter()
                    .find(|p| {
                        p.account.starts_with("Assets:") || p.account.starts_with("Liabilities:")
                    })
                    .map(|p| p.account.clone());
                (txn.narration.clone(), category, account)
            });

        if let Some((narration, category, account)) = maybe_defaults {
            if let Some(form) = self.add_tx_form.as_mut() {
                if form.narration.is_empty() {
                    form.narration = narration;
                }
                if form.category.is_empty() {
                    if let Some(c) = category {
                        form.category = c;
                    }
                }
                if form.account.is_empty() {
                    if let Some(a) = account {
                        form.account = a;
                    }
                }
            }
        }
    }

    /// Unique, sorted payees extracted from all transactions in the ledger.
    pub fn known_payees(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut payees: Vec<String> = self
            .ledger
            .transactions
            .iter()
            .filter_map(|t| t.payee.clone())
            .filter(|p| seen.insert(p.clone()))
            .collect();
        payees.sort_by_key(|a| a.to_lowercase());
        payees
    }

    pub fn navigate_to(&mut self, screen: Screen) {
        self.screen = screen;
        self.status_message = None;
        self.check_errors.clear();
    }

    /// Open a modal overlay. Captures all key input until closed.
    pub fn open_modal(&mut self, modal: Modal) {
        match modal {
            Modal::AddTransaction => {
                let accounts = self.account_names();
                let payees = self.known_payees();
                self.add_tx_form = Some(AddTxForm::new(&self.config.currency, &accounts, &payees));
            }
            Modal::AddAccount => {
                self.add_account_form = Some(AddAccountForm::new(&self.config.currency));
            }
            Modal::AccountFilter => {
                // Ensure the list is up-to-date; preserve existing selections.
                self.rebuild_account_filter();
            }
            Modal::TxAccountFilter => {
                self.rebuild_tx_account_filter();
            }
            Modal::DeleteTransaction => {
                self.delete_tx_confirm = true; // default to Yes
            }
            Modal::EditTransaction => {
                // Opened via open_edit_tx_modal() which sets up form state directly.
            }
            Modal::CategoryTransactions => {
                // State set by the caller before calling open_modal.
            }
            Modal::CsvImport => {
                let accounts = self.account_names();
                // Category suggestions: Expenses and Income accounts
                let categories: Vec<String> = accounts
                    .iter()
                    .filter(|a| a.starts_with("Expenses") || a.starts_with("Income"))
                    .cloned()
                    .collect();
                self.csv_import_state = Some(CsvImportState::new(accounts, categories));
            }
        }
        self.modal = Some(modal);
        // Don't clear status_message — it stays visible in the background
    }

    /// Close the active modal and discard its form state.
    pub fn close_modal(&mut self) {
        self.modal = None;
        self.add_tx_form = None;
        self.add_account_form = None;
        self.edit_tx_orig_line = None;
        self.csv_import_state = None;
        // account_filter state is intentionally kept across close so selections persist.
    }

    /// Open the edit-transaction modal pre-populated with the currently selected transaction.
    pub fn open_edit_tx_modal(&mut self) {
        if self.ledger.transactions.is_empty() {
            return;
        }

        // Build the same sorted+filtered order used by the transactions UI (reverse chrono).
        let filter = self.active_tx_account_filter();
        let mut sorted: Vec<&crate::beancount::parser::Transaction> = self
            .ledger
            .transactions
            .iter()
            .filter(|txn| match &filter {
                None => true,
                Some(set) => txn.postings.iter().any(|p| set.contains(&p.account)),
            })
            .collect();
        sorted.sort_by(|a, b| b.date.cmp(&a.date));

        let selected = self.tx_selected.min(sorted.len().saturating_sub(1));
        let txn = sorted[selected];

        // Map postings to the form's category / account fields.
        // Category = first Expenses:* or Income:* posting.
        // Account  = first Assets:* or Liabilities:* posting.
        let category_posting = txn
            .postings
            .iter()
            .find(|p| p.account.starts_with("Expenses:") || p.account.starts_with("Income:"))
            .or_else(|| txn.postings.first());

        let account_posting = txn
            .postings
            .iter()
            .find(|p| {
                p.account.starts_with("Assets:")
                    || p.account.starts_with("Liabilities:")
                    || p.account.starts_with("Equity:")
            })
            .or_else(|| txn.postings.get(1));

        let (category, amount_str, currency) = if let Some(p) = category_posting {
            let amt = p.amount.map(|a| a.abs().to_string()).unwrap_or_default();
            let cur = p
                .currency
                .clone()
                .unwrap_or_else(|| self.config.currency.clone());
            (p.account.clone(), amt, cur)
        } else {
            (String::new(), String::new(), self.config.currency.clone())
        };

        let account = account_posting
            .map(|p| p.account.clone())
            .unwrap_or_default();

        let accounts = self.account_names();
        let payees = self.known_payees();
        let mut form = AddTxForm::new(&currency, &accounts, &payees);
        form.date = txn.date.format("%Y-%m-%d").to_string();
        form.payee = txn.payee.clone().unwrap_or_default();
        form.narration = txn.narration.clone();
        form.category = category;
        form.account = account;
        form.amount = amount_str;
        form.currency = currency;

        self.edit_tx_orig_line = Some(txn.line);
        self.add_tx_form = Some(form);
        self.modal = Some(Modal::EditTransaction);
    }

    /// Delete the currently selected transaction from the beancount file.
    pub fn commit_delete_transaction(&mut self) -> Result<()> {
        if self.ledger.transactions.is_empty() {
            return Ok(());
        }

        let filter = self.active_tx_account_filter();
        let mut sorted: Vec<&crate::beancount::parser::Transaction> = self
            .ledger
            .transactions
            .iter()
            .filter(|txn| match &filter {
                None => true,
                Some(set) => txn.postings.iter().any(|p| set.contains(&p.account)),
            })
            .collect();
        sorted.sort_by(|a, b| b.date.cmp(&a.date));

        if sorted.is_empty() {
            return Ok(());
        }

        let selected = self.tx_selected.min(sorted.len().saturating_sub(1));
        let txn = sorted[selected];
        let line = txn.line;
        // Capture description before the ledger is reloaded.
        let date = txn.date;
        let narration = txn.narration.clone();

        let path = self.config.resolved_beancount_file();
        crate::beancount::writer::delete_transaction(&path, line)?;
        self.reload_ledger()?;

        // Clamp cursor after deletion
        let new_len = self.ledger.transactions.len();
        if new_len == 0 {
            self.tx_selected = 0;
            self.tx_scroll = 0;
        } else if self.tx_selected >= new_len {
            self.tx_selected = new_len - 1;
        }

        if git::is_git_repo(path.parent().unwrap_or(&path)) {
            let msg = format!("txn: delete {} {}", date.format("%Y-%m-%d"), narration);
            match git::commit_file(&path, &msg) {
                Ok(()) => {
                    self.status_message = Some("Transaction deleted  |  git: committed".to_string())
                }
                Err(e) => self.status_message = Some(format!("Transaction deleted  |  git: {}", e)),
            }
        }

        Ok(())
    }

    /// Commit the current add_account_form to the beancount file.
    pub fn commit_account(&mut self) -> Result<()> {
        let form = self.add_account_form.as_ref().unwrap();

        let date = chrono::NaiveDate::parse_from_str(&form.date, "%Y-%m-%d")
            .map_err(|_| anyhow::anyhow!("Invalid date format. Use YYYY-MM-DD"))?;

        if form.sub_name.trim().is_empty() {
            anyhow::bail!("Account sub-name cannot be empty");
        }
        // Validate: only letters, digits, colons, hyphens
        let sub = form.sub_name.trim();
        if !sub
            .chars()
            .all(|c| c.is_alphanumeric() || c == ':' || c == '-' || c == '_')
        {
            anyhow::bail!("Account name may only contain letters, digits, ':', '-', '_'");
        }
        // Each segment must start with uppercase
        for segment in sub.split(':') {
            if segment.is_empty() {
                anyhow::bail!("Account name segments cannot be empty");
            }
            if !segment
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                anyhow::bail!(
                    "Each account segment must start with an uppercase letter (got \"{}\")",
                    segment
                );
            }
        }

        let account_name = form.account_name();
        let currencies: Vec<String> = form
            .currencies
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_matches(',').to_uppercase())
            .collect();

        // Parse optional initial balance before any writes
        let initial_balance: Option<(Decimal, String)> = {
            let raw = form.initial_balance.trim();
            if raw.is_empty() {
                None
            } else {
                let amount = Decimal::from_str(raw).map_err(|_| {
                    anyhow::anyhow!("Invalid initial balance — enter a number, e.g. 1500.00")
                })?;
                let currency = currencies
                    .first()
                    .cloned()
                    .unwrap_or_else(|| self.config.currency.clone());
                Some((amount, currency))
            }
        };

        let path = self.config.resolved_beancount_file();

        // Write the open directive for the new account
        append_account_open(&path, date, &account_name, &currencies)?;

        // If an initial balance was given, also write an opening transaction
        if let Some((amount, currency)) = &initial_balance {
            const EQUITY_ACCT: &str = "Equity:OpeningBalances";

            // Ensure Equity:OpeningBalances is open — add its open directive if missing
            let already_open = self.ledger.accounts.iter().any(|a| a.name == EQUITY_ACCT);
            if !already_open {
                append_account_open(&path, date, EQUITY_ACCT, std::slice::from_ref(currency))?;
            }

            let opening_txn = NewTransaction {
                date,
                flag: '*',
                payee: None,
                narration: format!("Opening balance for {}", account_name),
                postings: vec![
                    NewPosting {
                        account: account_name.clone(),
                        amount: Some(*amount),
                        currency: Some(currency.clone()),
                    },
                    NewPosting {
                        account: EQUITY_ACCT.to_string(),
                        amount: None,
                        currency: None,
                    },
                ],
            };
            append_transaction(&path, &opening_txn)?;
        }

        self.reload_ledger()?;

        let mut status_parts = vec![match &initial_balance {
            Some((amt, cur)) => format!(
                "Account '{}' added with opening balance {} {}.",
                account_name, amt, cur
            ),
            None => format!("Account '{}' added.", account_name),
        }];

        if self.config.auto_bean_check {
            match bean_check(&path) {
                CheckResult::Ok => {
                    status_parts.push("bean-check: OK".to_string());
                    self.check_errors.clear();
                }
                CheckResult::Errors(errs) => {
                    status_parts.push(format!("bean-check: {} error(s)", errs.len()));
                    self.check_errors = errs;
                }
                CheckResult::NotInstalled => {
                    status_parts.push("(bean-check not installed)".to_string());
                    self.check_errors.clear();
                }
            }
        }

        if git::is_git_repo(path.parent().unwrap_or(&path)) {
            let msg = format!("account: open {}", account_name);
            match git::commit_file(&path, &msg) {
                Ok(()) => status_parts.push("git: committed".to_string()),
                Err(e) => status_parts.push(format!("git: {}", e)),
            }
        }

        self.status_message = Some(status_parts.join("  |  "));
        Ok(())
    }

    /// Handle the user pressing Enter/Y on the startup screen's git init prompt.
    pub fn startup_init_git(&mut self) {
        if let GitStatus::Uncontrolled { ref dir } = self.startup.git_status.clone() {
            match git::init_repo(dir) {
                Ok(msg) => {
                    self.startup.git_status = GitStatus::Controlled;
                    self.startup.git_init_result = Some(format!("Git repo initialised. {}", msg));
                }
                Err(e) => {
                    self.startup.git_init_result = Some(format!("git init failed: {}", e));
                }
            }
        }
    }

    /// Replace the transaction at `edit_tx_orig_line` with the edited form data.
    pub fn commit_edit_transaction(&mut self) -> Result<()> {
        let orig_line = self
            .edit_tx_orig_line
            .ok_or_else(|| anyhow::anyhow!("No transaction being edited"))?;

        let form = self.add_tx_form.as_ref().unwrap();

        let date = chrono::NaiveDate::parse_from_str(&form.date, "%Y-%m-%d")
            .map_err(|_| anyhow::anyhow!("Invalid date format. Use YYYY-MM-DD"))?;
        let narration = form.narration.trim().to_string();
        if narration.is_empty() {
            anyhow::bail!("Narration cannot be empty");
        }
        if form.category.trim().is_empty() {
            anyhow::bail!("Category cannot be empty");
        }
        if form.account.trim().is_empty() {
            anyhow::bail!("Account cannot be empty");
        }
        let amount =
            Decimal::from_str(form.amount.trim()).map_err(|_| anyhow::anyhow!("Invalid amount"))?;
        let currency = form.currency.trim().to_string();
        if currency.is_empty() {
            anyhow::bail!("Currency cannot be empty");
        }
        let payee = if form.payee.trim().is_empty() {
            None
        } else {
            Some(form.payee.trim().to_string())
        };

        let narration_clone = narration.clone();
        let category = form.category.trim().to_string();
        let account = form.account.trim().to_string();

        let updated_txn = NewTransaction {
            date,
            flag: '*',
            payee,
            narration,
            postings: vec![
                NewPosting {
                    account: category,
                    amount: Some(amount),
                    currency: Some(currency.clone()),
                },
                NewPosting {
                    account,
                    amount: Some(-amount),
                    currency: Some(currency),
                },
            ],
        };

        let path = self.config.resolved_beancount_file();
        replace_transaction(&path, orig_line, &updated_txn)?;
        self.edit_tx_orig_line = None;
        self.reload_ledger()?;

        let mut status_parts: Vec<String> = vec!["Transaction updated.".to_string()];

        if self.config.auto_bean_check {
            match bean_check(&path) {
                CheckResult::Ok => {
                    status_parts.push("bean-check: OK".to_string());
                    self.check_errors.clear();
                }
                CheckResult::Errors(errs) => {
                    status_parts.push(format!("bean-check: {} error(s)", errs.len()));
                    self.check_errors = errs;
                }
                CheckResult::NotInstalled => {
                    status_parts.push("(bean-check not installed)".to_string());
                    self.check_errors.clear();
                }
            }
        }

        if git::is_git_repo(path.parent().unwrap_or(&path)) {
            let msg = format!("txn: edit {} {}", date.format("%Y-%m-%d"), narration_clone);
            match git::commit_file(&path, &msg) {
                Ok(()) => status_parts.push("git: committed".to_string()),
                Err(e) => status_parts.push(format!("git: {}", e)),
            }
        }

        self.status_message = Some(status_parts.join("  |  "));
        Ok(())
    }

    /// Commit the current add_tx_form to the beancount file.
    pub fn commit_transaction(&mut self) -> Result<()> {
        let form = self.add_tx_form.as_mut().unwrap();

        let date = chrono::NaiveDate::parse_from_str(&form.date, "%Y-%m-%d")
            .map_err(|_| anyhow::anyhow!("Invalid date format. Use YYYY-MM-DD"))?;
        let narration = form.narration.trim().to_string();
        if narration.is_empty() {
            anyhow::bail!("Narration cannot be empty");
        }
        if form.category.trim().is_empty() {
            anyhow::bail!("Category cannot be empty");
        }
        if form.account.trim().is_empty() {
            anyhow::bail!("Account cannot be empty");
        }
        let amount =
            Decimal::from_str(form.amount.trim()).map_err(|_| anyhow::anyhow!("Invalid amount"))?;
        let currency = form.currency.trim().to_string();
        if currency.is_empty() {
            anyhow::bail!("Currency cannot be empty");
        }
        let payee = if form.payee.trim().is_empty() {
            None
        } else {
            Some(form.payee.trim().to_string())
        };

        // Clone values we need after the mutable borrow ends
        let narration_clone = narration.clone();
        // category (Expenses:*/Income:*) is debited (positive amount)
        // account (Assets:*/Liabilities:*) is credited (negative amount)
        let category = form.category.trim().to_string();
        let account = form.account.trim().to_string();

        let new_txn = NewTransaction {
            date,
            flag: '*',
            payee,
            narration,
            postings: vec![
                NewPosting {
                    account: category,
                    amount: Some(amount),
                    currency: Some(currency.clone()),
                },
                NewPosting {
                    account,
                    amount: Some(-amount),
                    currency: Some(currency),
                },
            ],
        };

        let path = self.config.resolved_beancount_file();
        append_transaction(&path, &new_txn)?;
        self.reload_ledger()?;

        // bean-check
        let mut status_parts: Vec<String> = vec!["Transaction saved.".to_string()];

        if self.config.auto_bean_check {
            match bean_check(&path) {
                CheckResult::Ok => {
                    status_parts.push("bean-check: OK".to_string());
                    self.check_errors.clear();
                }
                CheckResult::Errors(errs) => {
                    status_parts.push(format!("bean-check: {} error(s)", errs.len()));
                    self.check_errors = errs;
                }
                CheckResult::NotInstalled => {
                    status_parts.push("(bean-check not installed)".to_string());
                    self.check_errors.clear();
                }
            }
        }

        // git commit
        if git::is_git_repo(path.parent().unwrap_or(&path)) {
            let msg = format!("txn: {} {}", date.format("%Y-%m-%d"), narration_clone);
            match git::commit_file(&path, &msg) {
                Ok(()) => status_parts.push("git: committed".to_string()),
                Err(e) => status_parts.push(format!("git: {}", e)),
            }
        }

        self.status_message = Some(status_parts.join("  |  "));

        Ok(())
    }

    /// Write all included CSV import rows to the beancount file.
    pub fn commit_csv_import(&mut self) -> Result<()> {
        let state = self
            .csv_import_state
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No CSV import state"))?;

        let included: Vec<_> = state.rows.iter().filter(|r| r.include).collect();
        if included.is_empty() {
            anyhow::bail!("No transactions selected for import");
        }

        // Validate all included rows have categories
        for (i, row) in included.iter().enumerate() {
            if row.category.trim().is_empty() {
                anyhow::bail!(
                    "Transaction {} ({} {}) has no category assigned",
                    i + 1,
                    row.date,
                    row.payee
                );
            }
        }

        let path = self.config.resolved_beancount_file();
        let currency = self.config.currency.clone();
        let account = state.account.clone();
        let count = included.len();

        for row in &included {
            let new_txn = NewTransaction {
                date: row.date,
                flag: '*',
                payee: if row.payee.is_empty() {
                    None
                } else {
                    Some(row.payee.clone())
                },
                narration: row.payee.clone(),
                postings: vec![
                    NewPosting {
                        account: row.category.clone(),
                        amount: Some(row.amount.abs()),
                        currency: Some(currency.clone()),
                    },
                    NewPosting {
                        account: account.clone(),
                        amount: Some(if row.amount.is_sign_negative() {
                            row.amount
                        } else {
                            -row.amount
                        }),
                        currency: Some(currency.clone()),
                    },
                ],
            };
            append_transaction(&path, &new_txn)?;
        }

        self.reload_ledger()?;

        let mut status_parts: Vec<String> = vec![format!("Imported {} transaction(s).", count)];

        if self.config.auto_bean_check {
            match bean_check(&path) {
                CheckResult::Ok => {
                    status_parts.push("bean-check: OK".to_string());
                    self.check_errors.clear();
                }
                CheckResult::Errors(errs) => {
                    status_parts.push(format!("bean-check: {} error(s)", errs.len()));
                    self.check_errors = errs;
                }
                CheckResult::NotInstalled => {
                    status_parts.push("(bean-check not installed)".to_string());
                    self.check_errors.clear();
                }
            }
        }

        if git::is_git_repo(path.parent().unwrap_or(&path)) {
            let msg = format!("csv import: {} transactions", count);
            match git::commit_file(&path, &msg) {
                Ok(()) => status_parts.push("git: committed".to_string()),
                Err(e) => status_parts.push(format!("git: {}", e)),
            }
        }

        self.status_message = Some(status_parts.join("  |  "));
        Ok(())
    }

    /// Mark or unmark transactions as reconciled.
    ///
    /// `force`: `true` = reconcile, `false` = unreconcile.
    ///
    /// Operates on `reconcile_selected` if non-empty, otherwise on the
    /// currently highlighted transaction. After writing, reloads the ledger
    /// and commits to git.
    pub fn commit_reconcile_transactions(&mut self, force: bool) -> Result<()> {
        if self.ledger.transactions.is_empty() {
            return Ok(());
        }

        // Build sorted+filtered list (same order as the UI).
        let filter = self.active_tx_account_filter();
        let mut sorted: Vec<&crate::beancount::parser::Transaction> = self
            .ledger
            .transactions
            .iter()
            .filter(|txn| match &filter {
                None => true,
                Some(set) => txn.postings.iter().any(|p| set.contains(&p.account)),
            })
            .collect();
        sorted.sort_by(|a, b| b.date.cmp(&a.date));

        if sorted.is_empty() {
            return Ok(());
        }

        // Collect target line numbers (stable file positions).
        let target_lines: Vec<usize> = if !self.reconcile_selected.is_empty() {
            // Use the multi-select set
            self.reconcile_selected.iter().copied().collect()
        } else {
            // Fall back to the current cursor
            let selected = self.tx_selected.min(sorted.len().saturating_sub(1));
            vec![sorted[selected].line]
        };

        let path = self.config.resolved_beancount_file();
        let mut changed = 0usize;
        for line in &target_lines {
            crate::beancount::writer::set_reconcile_tag(&path, *line, force)?;
            changed += 1;
        }

        self.reload_ledger()?;
        self.reconcile_selected.clear();

        let action = if force { "reconciled" } else { "unreconciled" };
        let mut status_parts = vec![format!("{} transaction(s) {}.", changed, action)];

        if git::is_git_repo(path.parent().unwrap_or(&path)) {
            let msg = format!("reconcile: {} {} transaction(s)", action, changed);
            match git::commit_file(&path, &msg) {
                Ok(()) => status_parts.push("git: committed".to_string()),
                Err(e) => status_parts.push(format!("git: {}", e)),
            }
        }

        self.status_message = Some(status_parts.join("  |  "));
        Ok(())
    }
}
