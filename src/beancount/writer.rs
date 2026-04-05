use anyhow::{Context, Result};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::fmt::Write as FmtWrite;
use std::path::Path;

pub struct NewTransaction {
    pub date: NaiveDate,
    pub flag: char,
    pub payee: Option<String>,
    pub narration: String,
    pub postings: Vec<NewPosting>,
}

pub struct NewPosting {
    pub account: String,
    pub amount: Option<Decimal>,
    pub currency: Option<String>,
}

pub fn format_transaction(txn: &NewTransaction) -> String {
    let mut out = String::new();
    // Header line
    let header = match &txn.payee {
        Some(p) => format!(
            "{} {} \"{}\" \"{}\"",
            txn.date.format("%Y-%m-%d"),
            txn.flag,
            p,
            txn.narration
        ),
        None => format!(
            "{} {} \"{}\"",
            txn.date.format("%Y-%m-%d"),
            txn.flag,
            txn.narration
        ),
    };
    out.push_str(&header);
    out.push('\n');

    for posting in &txn.postings {
        match (&posting.amount, &posting.currency) {
            (Some(amount), Some(currency)) => {
                let _ = writeln!(out, "  {}  {} {}", posting.account, amount, currency);
            }
            _ => {
                let _ = writeln!(out, "  {}", posting.account);
            }
        }
    }

    out
}

pub fn append_transaction(path: &Path, txn: &NewTransaction) -> Result<()> {
    let formatted = format_transaction(txn);
    // Ensure file ends with a newline before appending
    let existing = std::fs::read_to_string(path)
        .unwrap_or_default();
    let separator = if existing.ends_with('\n') || existing.is_empty() {
        "\n"
    } else {
        "\n\n"
    };
    let content = format!("{}{}", separator, formatted);
    use std::fs::OpenOptions;
    use std::io::Write;
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .with_context(|| format!("Opening beancount file for append: {}", path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Writing transaction to {}", path.display()))?;
    Ok(())
}
