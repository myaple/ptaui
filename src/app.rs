use crate::beancount::parser::Ledger;
use crate::beancount::validator::{bean_check, CheckResult};
use crate::beancount::writer::{append_transaction, NewPosting, NewTransaction};
use crate::config::Config;
use anyhow::Result;
use chrono::Local;
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Dashboard,
    Transactions,
    AddTransaction,
    Reports,
}

/// Which field is focused in the Add Transaction form
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

    /// Complete the currently focused account field with the first matching suggestion
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

pub struct App {
    pub config: Config,
    pub ledger: Ledger,
    pub screen: Screen,
    pub tx_scroll: usize,
    pub dashboard_scroll: usize,
    pub add_tx_form: Option<AddTxForm>,
    pub status_message: Option<String>,
    pub check_errors: Vec<String>,
    pub running: bool,
    #[allow(dead_code)]
    pub help_visible: bool,
}

impl App {
    pub fn new(config: Config, ledger: Ledger) -> Self {
        Self {
            config,
            ledger,
            screen: Screen::Dashboard,
            tx_scroll: 0,
            dashboard_scroll: 0,
            add_tx_form: None,
            status_message: None,
            check_errors: Vec::new(),
            running: true,
            help_visible: false,
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
        self.ledger
            .accounts
            .iter()
            .map(|a| a.name.clone())
            .collect()
    }

    pub fn navigate_to(&mut self, screen: Screen) {
        match screen {
            Screen::AddTransaction => {
                let accounts = self.account_names();
                self.add_tx_form = Some(AddTxForm::new(&self.config.currency, &accounts));
            }
            _ => {}
        }
        self.screen = screen;
        self.status_message = None;
        self.check_errors.clear();
    }

    /// Commit the current add_tx_form to the beancount file
    pub fn commit_transaction(&mut self) -> Result<()> {
        let form = self.add_tx_form.as_mut().unwrap();
        // Validate
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

        let new_txn = NewTransaction {
            date,
            flag: '*',
            payee,
            narration,
            postings: vec![
                NewPosting {
                    account: form.to_account.trim().to_string(),
                    amount: Some(amount),
                    currency: Some(currency.clone()),
                },
                NewPosting {
                    account: form.from_account.trim().to_string(),
                    amount: Some(-amount),
                    currency: Some(currency),
                },
            ],
        };

        let path = self.config.resolved_beancount_file();
        append_transaction(&path, &new_txn)?;
        self.reload_ledger()?;

        // Run bean-check if configured
        if self.config.auto_bean_check {
            match bean_check(&path) {
                CheckResult::Ok => {
                    self.status_message = Some("Transaction saved. bean-check: OK".to_string());
                    self.check_errors.clear();
                }
                CheckResult::Errors(errs) => {
                    self.status_message = Some(format!(
                        "Transaction saved but bean-check reported {} error(s)",
                        errs.len()
                    ));
                    self.check_errors = errs;
                }
                CheckResult::NotInstalled => {
                    self.status_message =
                        Some("Transaction saved. (bean-check not installed)".to_string());
                    self.check_errors.clear();
                }
            }
        } else {
            self.status_message = Some("Transaction saved.".to_string());
        }

        // Optionally launch fava
        if self.config.launch_fava_after_entry {
            let _ = crate::beancount::validator::launch_fava(&path, self.config.fava_port);
        }

        Ok(())
    }
}
