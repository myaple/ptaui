use crate::beancount::parser::Ledger;
use crate::beancount::validator::{bean_check, CheckResult};
use crate::beancount::writer::{append_account_open, append_transaction, NewPosting, NewTransaction};
use crate::config::Config;
use crate::git;
use anyhow::{Context, Result};
use chrono::Local;
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
    AddTransaction,
    AddAccount,
    Reports,
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
    FromAccount,
    ToAccount,
    Amount,
    Currency,
    Confirm,
}

impl AddTxField {
    pub fn next(&self) -> Self {
        match self {
            Self::Date => Self::Payee,
            Self::Payee => Self::Narration,
            Self::Narration => Self::FromAccount,
            Self::FromAccount => Self::ToAccount,
            Self::ToAccount => Self::Amount,
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
            Self::FromAccount => Self::Narration,
            Self::ToAccount => Self::FromAccount,
            Self::Amount => Self::ToAccount,
            Self::Currency => Self::Amount,
            Self::Confirm => Self::Currency,
        }
    }
}

pub struct AddTxForm {
    pub date: String,
    pub payee: String,
    pub narration: String,
    pub from_account: String,
    pub to_account: String,
    pub amount: String,
    pub currency: String,
    pub focused: AddTxField,
    pub account_suggestions: Vec<String>,
    pub error: Option<String>,
}

impl AddTxForm {
    pub fn new(currency: &str, accounts: &[String]) -> Self {
        Self {
            date: Local::now().date_naive().format("%Y-%m-%d").to_string(),
            payee: String::new(),
            narration: String::new(),
            from_account: String::new(),
            to_account: String::new(),
            amount: String::new(),
            currency: currency.to_string(),
            focused: AddTxField::Date,
            account_suggestions: accounts.to_vec(),
            error: None,
        }
    }

    pub fn current_field_mut(&mut self) -> &mut String {
        match self.focused {
            AddTxField::Date => &mut self.date,
            AddTxField::Payee => &mut self.payee,
            AddTxField::Narration => &mut self.narration,
            AddTxField::FromAccount => &mut self.from_account,
            AddTxField::ToAccount => &mut self.to_account,
            AddTxField::Amount => &mut self.amount,
            AddTxField::Currency => &mut self.currency,
            AddTxField::Confirm => &mut self.narration, // dummy
        }
    }

    pub fn autocomplete(&mut self) {
        let prefix = match self.focused {
            AddTxField::FromAccount => self.from_account.clone(),
            AddTxField::ToAccount => self.to_account.clone(),
            _ => return,
        };
        if let Some(suggestion) = self
            .account_suggestions
            .iter()
            .find(|a| a.to_lowercase().contains(&prefix.to_lowercase()))
        {
            match self.focused {
                AddTxField::FromAccount => self.from_account = suggestion.clone(),
                AddTxField::ToAccount => self.to_account = suggestion.clone(),
                _ => {}
            }
        }
    }

    pub fn suggestions_for_current(&self) -> Vec<String> {
        let prefix = match self.focused {
            AddTxField::FromAccount => &self.from_account,
            AddTxField::ToAccount => &self.to_account,
            _ => return vec![],
        };
        if prefix.is_empty() {
            return vec![];
        }
        self.account_suggestions
            .iter()
            .filter(|a| a.to_lowercase().contains(&prefix.to_lowercase()))
            .take(5)
            .cloned()
            .collect()
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
            Self::AccountType    => Self::SubName,
            Self::SubName        => Self::Currencies,
            Self::Currencies     => Self::Date,
            Self::Date           => Self::InitialBalance,
            Self::InitialBalance => Self::Confirm,
            Self::Confirm        => Self::AccountType,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            Self::AccountType    => Self::Confirm,
            Self::SubName        => Self::AccountType,
            Self::Currencies     => Self::SubName,
            Self::Date           => Self::Currencies,
            Self::InitialBalance => Self::Date,
            Self::Confirm        => Self::InitialBalance,
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

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub config: Config,
    pub ledger: Ledger,
    /// True when the beancount file was found and successfully read.
    pub file_found: bool,
    pub screen: Screen,
    pub startup: StartupState,
    pub tx_scroll: usize,
    pub dashboard_scroll: usize,
    pub add_tx_form: Option<AddTxForm>,
    pub add_account_form: Option<AddAccountForm>,
    pub status_message: Option<String>,
    pub check_errors: Vec<String>,
    pub running: bool,
}

impl App {
    pub fn new(config: Config, ledger: Ledger, file_found: bool, startup: StartupState) -> Self {
        let initial_screen = if startup.needs_display() {
            Screen::Startup
        } else {
            Screen::Dashboard
        };
        Self {
            config,
            ledger,
            file_found,
            screen: initial_screen,
            startup,
            tx_scroll: 0,
            dashboard_scroll: 0,
            add_tx_form: None,
            add_account_form: None,
            status_message: None,
            check_errors: Vec::new(),
            running: true,
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
        std::fs::write(&path, &starter)
            .with_context(|| format!("Creating {}", path.display()))?;

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
        self.ledger.accounts.iter().map(|a| a.name.clone()).collect()
    }

    pub fn navigate_to(&mut self, screen: Screen) {
        match screen {
            Screen::AddTransaction => {
                let accounts = self.account_names();
                self.add_tx_form = Some(AddTxForm::new(&self.config.currency, &accounts));
            }
            Screen::AddAccount => {
                self.add_account_form = Some(AddAccountForm::new(&self.config.currency));
            }
            _ => {}
        }
        self.screen = screen;
        self.status_message = None;
        self.check_errors.clear();
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
        if !sub.chars().all(|c| c.is_alphanumeric() || c == ':' || c == '-' || c == '_') {
            anyhow::bail!("Account name may only contain letters, digits, ':', '-', '_'");
        }
        // Each segment must start with uppercase
        for segment in sub.split(':') {
            if segment.is_empty() {
                anyhow::bail!("Account name segments cannot be empty");
            }
            if !segment.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                anyhow::bail!("Each account segment must start with an uppercase letter (got \"{}\")", segment);
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
                let amount = Decimal::from_str(raw)
                    .map_err(|_| anyhow::anyhow!("Invalid initial balance — enter a number, e.g. 1500.00"))?;
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
            let already_open = self
                .ledger
                .accounts
                .iter()
                .any(|a| a.name == EQUITY_ACCT);
            if !already_open {
                append_account_open(&path, date, EQUITY_ACCT, &[currency.clone()])?;
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
            Some((amt, cur)) => format!("Account '{}' added with opening balance {} {}.", account_name, amt, cur),
            None => format!("Account '{}' added.", account_name),
        }];

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
                    self.startup.git_init_result =
                        Some(format!("Git repo initialised. {}", msg));
                }
                Err(e) => {
                    self.startup.git_init_result = Some(format!("git init failed: {}", e));
                }
            }
        }
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
        if form.from_account.trim().is_empty() {
            anyhow::bail!("From account cannot be empty");
        }
        if form.to_account.trim().is_empty() {
            anyhow::bail!("To account cannot be empty");
        }
        let amount = Decimal::from_str(form.amount.trim())
            .map_err(|_| anyhow::anyhow!("Invalid amount"))?;
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
        let to_account = form.to_account.trim().to_string();
        let from_account = form.from_account.trim().to_string();

        let new_txn = NewTransaction {
            date,
            flag: '*',
            payee,
            narration,
            postings: vec![
                NewPosting {
                    account: to_account,
                    amount: Some(amount),
                    currency: Some(currency.clone()),
                },
                NewPosting {
                    account: from_account,
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
            let msg = format!(
                "txn: {} {}",
                date.format("%Y-%m-%d"),
                narration_clone
            );
            match git::commit_file(&path, &msg) {
                Ok(()) => status_parts.push("git: committed".to_string()),
                Err(e) => status_parts.push(format!("git: {}", e)),
            }
        }

        self.status_message = Some(status_parts.join("  |  "));

        if self.config.launch_fava_after_entry {
            let _ = crate::beancount::validator::launch_fava(&path, self.config.fava_port);
        }

        Ok(())
    }
}
