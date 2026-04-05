use crate::beancount::parser::Ledger;
use crate::beancount::validator::{bean_check, CheckResult};
use crate::beancount::writer::{append_transaction, NewPosting, NewTransaction};
use crate::config::Config;
use crate::git;
use anyhow::Result;
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

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub config: Config,
    pub ledger: Ledger,
    pub screen: Screen,
    pub startup: StartupState,
    pub tx_scroll: usize,
    pub dashboard_scroll: usize,
    pub add_tx_form: Option<AddTxForm>,
    pub status_message: Option<String>,
    pub check_errors: Vec<String>,
    pub running: bool,
}

impl App {
    pub fn new(config: Config, ledger: Ledger, startup: StartupState) -> Self {
        let initial_screen = if startup.needs_display() {
            Screen::Startup
        } else {
            Screen::Dashboard
        };
        Self {
            config,
            ledger,
            screen: initial_screen,
            startup,
            tx_scroll: 0,
            dashboard_scroll: 0,
            add_tx_form: None,
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
        }
        Ok(())
    }

    pub fn account_names(&self) -> Vec<String> {
        self.ledger.accounts.iter().map(|a| a.name.clone()).collect()
    }

    pub fn navigate_to(&mut self, screen: Screen) {
        if let Screen::AddTransaction = screen {
            let accounts = self.account_names();
            self.add_tx_form = Some(AddTxForm::new(&self.config.currency, &accounts));
        }
        self.screen = screen;
        self.status_message = None;
        self.check_errors.clear();
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
