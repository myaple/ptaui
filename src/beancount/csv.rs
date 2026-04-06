use anyhow::{Context, Result};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::path::Path;
use std::str::FromStr;

use super::parser::Transaction;

/// Read a CSV file, returning headers and rows.
pub fn read_csv(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut reader = csv::Reader::from_path(path)
        .with_context(|| format!("Opening CSV file: {}", path.display()))?;

    let headers: Vec<String> = reader
        .headers()
        .context("Reading CSV headers")?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let mut rows = Vec::new();
    for result in reader.records() {
        let record = result.context("Reading CSV row")?;
        let row: Vec<String> = record.iter().map(|f| f.to_string()).collect();
        rows.push(row);
    }

    Ok((headers, rows))
}

/// Parse a raw amount string, handling currency symbols, commas, and parentheses for negatives.
fn parse_amount(s: &str) -> Result<Decimal> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Decimal::ZERO);
    }

    // Check for parenthetical negatives: (123.45)
    let (negative, s) = if s.starts_with('(') && s.ends_with(')') {
        (true, &s[1..s.len() - 1])
    } else {
        (false, s)
    };

    // Strip currency symbols and whitespace
    let cleaned: String = s
        .chars()
        .filter(|c| *c == '-' || *c == '.' || c.is_ascii_digit())
        .collect();

    let mut amount =
        Decimal::from_str(&cleaned).with_context(|| format!("Parsing amount: {:?}", s))?;

    if negative {
        amount = -amount;
    }

    Ok(amount)
}

/// Column mapping for CSV import.
pub struct ColumnMapping {
    pub date_col: usize,
    pub payee_col: usize,
    pub amount_col: usize,
    pub debit_col: Option<usize>,
    pub credit_col: Option<usize>,
}

/// A parsed CSV row ready for import.
#[derive(Debug, Clone)]
pub struct CsvRow {
    pub date: NaiveDate,
    pub payee: String,
    pub amount: Decimal,
    pub is_duplicate: bool,
    pub include: bool,
    pub category: String,
}

/// Parse raw CSV rows into structured import rows.
pub fn parse_rows(
    raw_rows: &[Vec<String>],
    mapping: &ColumnMapping,
    date_format: &str,
    negate: bool,
) -> Result<Vec<CsvRow>> {
    let mut rows = Vec::with_capacity(raw_rows.len());

    for (i, raw) in raw_rows.iter().enumerate() {
        let date_str = raw
            .get(mapping.date_col)
            .map(|s| s.trim())
            .unwrap_or_default();

        if date_str.is_empty() {
            continue; // skip empty rows
        }

        let date = NaiveDate::parse_from_str(date_str, date_format).with_context(|| {
            format!(
                "Row {}: parsing date {:?} with format {:?}",
                i + 1,
                date_str,
                date_format
            )
        })?;

        let payee = raw
            .get(mapping.payee_col)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let amount = if let Some(debit_col) = mapping.debit_col {
            // Separate debit/credit columns; credit column is optional
            let debit_str = raw.get(debit_col).map(|s| s.trim()).unwrap_or_default();
            let debit = if debit_str.is_empty() {
                Decimal::ZERO
            } else {
                parse_amount(debit_str).with_context(|| format!("Row {}: parsing debit", i + 1))?
            };
            let credit = if let Some(credit_col) = mapping.credit_col {
                let credit_str = raw.get(credit_col).map(|s| s.trim()).unwrap_or_default();
                if credit_str.is_empty() {
                    Decimal::ZERO
                } else {
                    parse_amount(credit_str)
                        .with_context(|| format!("Row {}: parsing credit", i + 1))?
                }
            } else {
                Decimal::ZERO
            };
            // Credits are positive (money in), debits are negative (money out)
            credit - debit
        } else {
            let amount_str = raw
                .get(mapping.amount_col)
                .map(|s| s.trim())
                .unwrap_or_default();
            parse_amount(amount_str).with_context(|| format!("Row {}: parsing amount", i + 1))?
        };

        let amount = if negate { -amount } else { amount };

        rows.push(CsvRow {
            date,
            payee,
            amount,
            is_duplicate: false,
            include: true,
            category: String::new(),
        });
    }

    Ok(rows)
}

/// Detect duplicates by matching (date, absolute amount) against existing
/// transactions that involve the destination account.
pub fn detect_duplicates(rows: &mut [CsvRow], existing: &[Transaction], dest_account: &str) {
    // Build a lookup of (date, abs_amount) -> list of payees for the destination account
    let mut existing_set: Vec<(NaiveDate, Decimal, Vec<String>)> = Vec::new();

    for txn in existing {
        // Check if this transaction involves the destination account
        let mut involves_dest = false;
        let mut dest_amount = Decimal::ZERO;
        for posting in &txn.postings {
            if posting.account == dest_account {
                involves_dest = true;
                if let Some(amt) = posting.amount {
                    dest_amount = amt;
                }
            }
        }
        if involves_dest {
            let payee = txn.payee.clone().unwrap_or_default();
            existing_set.push((txn.date, dest_amount.abs(), vec![payee]));
        }
    }

    for row in rows.iter_mut() {
        let row_abs = row.amount.abs();
        for (date, amt, payees) in &existing_set {
            if row.date == *date && row_abs == *amt {
                row.is_duplicate = true;
                row.include = false;
                // Check payee similarity for stronger confidence (not used for gating, just detection)
                let _payee_match = payees.iter().any(|p| {
                    let p_lower = p.to_lowercase();
                    let row_lower = row.payee.to_lowercase();
                    p_lower.contains(&row_lower) || row_lower.contains(&p_lower)
                });
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_amount_simple() {
        assert_eq!(
            parse_amount("123.45").unwrap(),
            Decimal::from_str("123.45").unwrap()
        );
        assert_eq!(
            parse_amount("-42.00").unwrap(),
            Decimal::from_str("-42.00").unwrap()
        );
    }

    #[test]
    fn test_parse_amount_currency_symbols() {
        assert_eq!(
            parse_amount("$1,234.56").unwrap(),
            Decimal::from_str("1234.56").unwrap()
        );
        assert_eq!(
            parse_amount("$-50.00").unwrap(),
            Decimal::from_str("-50.00").unwrap()
        );
    }

    #[test]
    fn test_parse_amount_parentheses() {
        assert_eq!(
            parse_amount("(100.00)").unwrap(),
            Decimal::from_str("-100.00").unwrap()
        );
    }

    #[test]
    fn test_parse_amount_empty() {
        assert_eq!(parse_amount("").unwrap(), Decimal::ZERO);
        assert_eq!(parse_amount("  ").unwrap(), Decimal::ZERO);
    }

    #[test]
    fn test_parse_rows_basic() {
        let raw = vec![
            vec![
                "01/15/2024".to_string(),
                "Grocery Store".to_string(),
                "-50.00".to_string(),
            ],
            vec![
                "01/16/2024".to_string(),
                "Gas Station".to_string(),
                "-30.00".to_string(),
            ],
        ];
        let mapping = ColumnMapping {
            date_col: 0,
            payee_col: 1,
            amount_col: 2,
            debit_col: None,
            credit_col: None,
        };
        let rows = parse_rows(&raw, &mapping, "%m/%d/%Y", false).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].payee, "Grocery Store");
        assert_eq!(rows[0].amount, Decimal::from_str("-50.00").unwrap());
        assert_eq!(rows[1].date, NaiveDate::from_ymd_opt(2024, 1, 16).unwrap());
    }

    #[test]
    fn test_detect_duplicates() {
        use crate::beancount::parser::{Posting, Transaction};

        let existing = vec![Transaction {
            date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            flag: '*',
            payee: Some("Grocery Store".to_string()),
            narration: "Groceries".to_string(),
            tags: vec![],
            postings: vec![
                Posting {
                    account: "Assets:Checking".to_string(),
                    amount: Some(Decimal::from_str("-50.00").unwrap()),
                    currency: Some("USD".to_string()),
                },
                Posting {
                    account: "Expenses:Food".to_string(),
                    amount: Some(Decimal::from_str("50.00").unwrap()),
                    currency: Some("USD".to_string()),
                },
            ],
            line: 0,
        }];

        let mut rows = vec![
            CsvRow {
                date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                payee: "Grocery Store".to_string(),
                amount: Decimal::from_str("-50.00").unwrap(),
                is_duplicate: false,
                include: true,
                category: String::new(),
            },
            CsvRow {
                date: NaiveDate::from_ymd_opt(2024, 1, 17).unwrap(),
                payee: "Coffee Shop".to_string(),
                amount: Decimal::from_str("-5.00").unwrap(),
                is_duplicate: false,
                include: true,
                category: String::new(),
            },
        ];

        detect_duplicates(&mut rows, &existing, "Assets:Checking");

        assert!(rows[0].is_duplicate);
        assert!(!rows[0].include);
        assert!(!rows[1].is_duplicate);
        assert!(rows[1].include);
    }
}
