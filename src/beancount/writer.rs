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
    pub tags: Vec<String>,
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
    let mut header = match &txn.payee {
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

    for tag in &txn.tags {
        write!(header, " #{}", tag).unwrap();
    }

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

/// Append an `open` directive for a new account.
pub fn append_account_open(path: &Path, date: NaiveDate, account: &str, currencies: &[String]) -> Result<()> {
    let cur_str = if currencies.is_empty() {
        String::new()
    } else {
        format!(" {}", currencies.join(", "))
    };
    let line = format!("{} open {}{}\n", date.format("%Y-%m-%d"), account, cur_str);

    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let separator = if existing.ends_with('\n') || existing.is_empty() { "\n" } else { "\n\n" };
    let content = format!("{}{}", separator, line);

    use std::fs::OpenOptions;
    use std::io::Write;
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .with_context(|| format!("Opening beancount file for append: {}", path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Writing account open to {}", path.display()))?;
    Ok(())
}

/// Replace an existing transaction whose header is at `start_line` (0-based)
/// with `new_txn`. All indented posting lines after the header are considered
/// part of the old transaction and will be replaced.
pub fn replace_transaction(path: &Path, start_line: usize, new_txn: &NewTransaction) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Reading {}", path.display()))?;
    let file_lines: Vec<&str> = content.lines().collect();

    if start_line >= file_lines.len() {
        anyhow::bail!("Transaction start line {} is out of range (file has {} lines)", start_line, file_lines.len());
    }

    // Find the first line after the transaction block (header + indented postings).
    let mut end_line = start_line + 1;
    while end_line < file_lines.len() {
        let l = file_lines[end_line];
        if l.starts_with("  ") || l.starts_with('\t') {
            end_line += 1;
        } else {
            break;
        }
    }

    // Format the replacement transaction (ends with '\n', strip for joining).
    let new_text = format_transaction(new_txn);
    let new_lines: Vec<&str> = new_text.trim_end_matches('\n').lines().collect();

    // Reconstruct: before + new transaction + after.
    let mut result_lines: Vec<&str> = Vec::with_capacity(
        start_line + new_lines.len() + (file_lines.len() - end_line),
    );
    result_lines.extend_from_slice(&file_lines[..start_line]);
    result_lines.extend_from_slice(&new_lines);
    result_lines.extend_from_slice(&file_lines[end_line..]);

    let mut result = result_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }

    std::fs::write(path, &result)
        .with_context(|| format!("Writing updated transaction to {}", path.display()))?;
    Ok(())
}

/// Delete the transaction whose header is at `start_line` (0-based),
/// including all indented posting lines that follow it.
/// A blank line immediately before the transaction is also removed so the
/// file doesn't accumulate extra blank lines.
pub fn delete_transaction(path: &Path, start_line: usize) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Reading {}", path.display()))?;
    let file_lines: Vec<&str> = content.lines().collect();

    if start_line >= file_lines.len() {
        anyhow::bail!("Transaction start line {} is out of range (file has {} lines)", start_line, file_lines.len());
    }

    // Find the first line after the transaction block (header + indented postings).
    let mut end_line = start_line + 1;
    while end_line < file_lines.len() {
        let l = file_lines[end_line];
        if l.starts_with("  ") || l.starts_with('\t') {
            end_line += 1;
        } else {
            break;
        }
    }

    // Also strip a preceding blank line to avoid leaving a double-blank gap.
    let remove_from = if start_line > 0 && file_lines[start_line - 1].trim().is_empty() {
        start_line - 1
    } else {
        start_line
    };

    let mut result_lines: Vec<&str> = Vec::with_capacity(file_lines.len());
    result_lines.extend_from_slice(&file_lines[..remove_from]);
    result_lines.extend_from_slice(&file_lines[end_line..]);

    let mut result = result_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }

    std::fs::write(path, &result)
        .with_context(|| format!("Writing file after deletion to {}", path.display()))?;
    Ok(())
}

/// Update multiple transactions at once. `updates` is a list of `(start_line, new_txn)`.
/// This is more efficient than calling `replace_transaction` repeatedly because it
/// handles line number shifts correctly and performs only one write.
pub fn bulk_update_transactions(
    path: &Path,
    mut updates: Vec<(usize, NewTransaction)>,
) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    // Sort updates by start_line ascending to process the file in order.
    updates.sort_by_key(|u| u.0);

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Reading {}", path.display()))?;
    let file_lines: Vec<&str> = content.lines().collect();

    let mut result_lines: Vec<String> = Vec::new();
    let mut current_line = 0;

    for (start_line, new_txn) in updates {
        if start_line < current_line {
            anyhow::bail!("Overlapping or out-of-order updates");
        }

        // Add lines before the transaction to be replaced
        for i in current_line..start_line {
            result_lines.push(file_lines[i].to_string());
        }

        // Find the end of the transaction block to be replaced
        let mut end_line = start_line + 1;
        while end_line < file_lines.len() {
            let l = file_lines[end_line];
            if l.starts_with("  ") || l.starts_with('\t') {
                end_line += 1;
            } else {
                break;
            }
        }

        // Add the new transaction lines
        let new_text = format_transaction(&new_txn);
        for line in new_text.trim_end_matches('\n').lines() {
            result_lines.push(line.to_string());
        }

        current_line = end_line;
    }

    // Add remaining lines after the last update
    for i in current_line..file_lines.len() {
        result_lines.push(file_lines[i].to_string());
    }

    let mut result = result_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }

    std::fs::write(path, &result)
        .with_context(|| format!("Writing bulk updated transactions to {}", path.display()))?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_transaction_with_tags() {
        let txn = NewTransaction {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            flag: '*',
            payee: Some("Payee".to_string()),
            narration: "Narration".to_string(),
            tags: vec!["reconciled".to_string(), "tag2".to_string()],
            postings: vec![
                NewPosting {
                    account: "Assets:Checking".to_string(),
                    amount: Some(Decimal::new(100, 0)),
                    currency: Some("USD".to_string()),
                },
                NewPosting {
                    account: "Expenses:Food".to_string(),
                    amount: None,
                    currency: None,
                },
            ],
        };
        let formatted = format_transaction(&txn);
        assert!(formatted.contains("#reconciled"));
        assert!(formatted.contains("#tag2"));
        assert!(formatted.starts_with("2024-01-01 * \"Payee\" \"Narration\" #reconciled #tag2"));
    }
}
