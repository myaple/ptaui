use anyhow::Result;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct Account {
    pub name: String,
    #[allow(dead_code)]
    pub open_date: NaiveDate,
    #[allow(dead_code)]
    pub currencies: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Posting {
    pub account: String,
    pub amount: Option<Decimal>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub date: NaiveDate,
    pub flag: char,
    pub payee: Option<String>,
    pub narration: String,
    #[allow(dead_code)]
    pub tags: Vec<String>,
    pub postings: Vec<Posting>,
    /// 0-based line number in the file where this transaction header starts.
    pub line: usize,
}

impl Transaction {
    /// Returns true if this transaction has the `#reconciled` tag.
    pub fn is_reconciled(&self) -> bool {
        self.tags.iter().any(|t| t == "reconciled")
    }
}

#[derive(Debug, Clone, Default)]
pub struct Ledger {
    pub accounts: Vec<Account>,
    pub transactions: Vec<Transaction>,
}

impl Ledger {
    /// Compute balances for all accounts by replaying all transactions.
    pub fn balances(&self) -> HashMap<String, HashMap<String, Decimal>> {
        let mut balances: HashMap<String, HashMap<String, Decimal>> = HashMap::new();
        for txn in &self.transactions {
            for posting in &txn.postings {
                if let (Some(amount), Some(currency)) = (&posting.amount, &posting.currency) {
                    *balances
                        .entry(posting.account.clone())
                        .or_default()
                        .entry(currency.clone())
                        .or_insert(Decimal::ZERO) += amount;
                }
            }
        }
        balances
    }

    /// Group transactions by month, return sorted `(YYYY-MM, income, expenses)`.
    /// Only includes months that have at least one Income or Expenses posting.
    /// When `filter` is `Some`, only postings whose account name is in the set
    /// are counted; `None` means include all accounts.
    pub fn monthly_summary(
        &self,
        currency: &str,
        filter: Option<&std::collections::HashSet<String>>,
    ) -> Vec<(String, Decimal, Decimal)> {
        let mut map: HashMap<String, (Decimal, Decimal)> = HashMap::new();
        for txn in &self.transactions {
            let key = txn.date.format("%Y-%m").to_string();
            for posting in &txn.postings {
                let cur = posting.currency.as_deref().unwrap_or("");
                if cur != currency {
                    continue;
                }
                if let Some(f) = filter {
                    if !f.contains(&posting.account) {
                        continue;
                    }
                }
                if let Some(amount) = posting.amount {
                    let acct = &posting.account;
                    if acct.starts_with("Income") {
                        // Income postings are negative in beancount (credit)
                        let entry = map
                            .entry(key.clone())
                            .or_insert((Decimal::ZERO, Decimal::ZERO));
                        entry.0 += -amount;
                    } else if acct.starts_with("Expenses") {
                        let entry = map
                            .entry(key.clone())
                            .or_insert((Decimal::ZERO, Decimal::ZERO));
                        entry.1 += amount;
                    }
                }
            }
        }
        let mut months: Vec<String> = map.keys().cloned().collect();
        months.sort();
        months
            .into_iter()
            .map(|m| {
                let (income, expenses) = map[&m];
                (m, income, expenses)
            })
            .collect()
    }

    /// Compute per-category totals for transactions whose date is in [from, to).
    /// Returns `(category, amount)` sorted by amount descending.
    /// Expenses:* amounts are positive; Income:* amounts are positive (negated from beancount).
    /// When `filter` is `Some`, only categories in that set are counted.
    pub fn category_breakdown(
        &self,
        currency: &str,
        from: NaiveDate,
        to: NaiveDate,
        filter: Option<&std::collections::HashSet<String>>,
    ) -> Vec<(String, Decimal)> {
        let mut map: HashMap<String, Decimal> = HashMap::new();
        for txn in &self.transactions {
            if txn.date < from || txn.date >= to {
                continue;
            }
            for posting in &txn.postings {
                let cur = posting.currency.as_deref().unwrap_or("");
                if cur != currency {
                    continue;
                }
                if let Some(f) = filter {
                    if !f.contains(&posting.account) {
                        continue;
                    }
                }
                if let Some(amount) = posting.amount {
                    let acct = &posting.account;
                    if acct.starts_with("Expenses") {
                        *map.entry(acct.clone()).or_insert(Decimal::ZERO) += amount;
                    } else if acct.starts_with("Income") {
                        // Income postings are negative in beancount; negate to get positive
                        *map.entry(acct.clone()).or_insert(Decimal::ZERO) += -amount;
                    }
                }
            }
        }
        let mut result: Vec<(String, Decimal)> = map.into_iter().collect();
        // Sort by amount descending (largest expense / largest income first)
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        result
    }

    /// Return all transactions that touch `category` in [from, to) for `currency`,
    /// sorted by the absolute posting amount for that category, descending.
    pub fn transactions_for_category(
        &self,
        currency: &str,
        from: NaiveDate,
        to: NaiveDate,
        category: &str,
    ) -> Vec<Transaction> {
        let mut result: Vec<(Transaction, Decimal)> = Vec::new();
        for txn in &self.transactions {
            if txn.date < from || txn.date >= to {
                continue;
            }
            // Find the posting for this category
            let posting_amount = txn.postings.iter().find_map(|p| {
                if p.account == category && p.currency.as_deref().unwrap_or("") == currency {
                    p.amount
                } else {
                    None
                }
            });
            if let Some(amt) = posting_amount {
                result.push((txn.clone(), amt.abs()));
            }
        }
        // Sort by absolute posting amount descending
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        result.into_iter().map(|(txn, _)| txn).collect()
    }
}

pub fn parse(source: &str) -> Result<Ledger> {
    let mut ledger = Ledger::default();
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim_end();

        // Skip blank lines and comments
        if line.is_empty()
            || line.starts_with(';')
            || line.starts_with("//")
            || line.starts_with('#')
        {
            i += 1;
            continue;
        }

        // Directives start with a date
        if let Some(rest) = try_parse_date_prefix(line) {
            let (date, directive_rest) = rest;
            let parts: Vec<&str> = directive_rest.splitn(2, ' ').collect();
            match parts.first().copied() {
                Some("open") => {
                    if let Some(account_part) = parts.get(1) {
                        let pieces: Vec<&str> = account_part.split_whitespace().collect();
                        if let Some(name) = pieces.first() {
                            let currencies = pieces[1..].iter().map(|s| s.to_string()).collect();
                            ledger.accounts.push(Account {
                                name: name.to_string(),
                                open_date: date,
                                currencies,
                            });
                        }
                    }
                }
                Some("txn") | Some("*") | Some("!") => {
                    let start_line = i;
                    let flag = if parts[0] == "!" { '!' } else { '*' };
                    let (payee, narration, tags) =
                        parse_txn_header(parts.get(1).copied().unwrap_or(""));
                    let mut postings = Vec::new();
                    i += 1;
                    while i < lines.len() {
                        let pline = lines[i];
                        // Posting lines are indented with at least 2 spaces
                        if pline.starts_with("  ") || pline.starts_with('\t') {
                            let pline = pline.trim();
                            if pline.is_empty() || pline.starts_with(';') {
                                i += 1;
                                continue;
                            }
                            if let Some(posting) = parse_posting(pline) {
                                postings.push(posting);
                            }
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    ledger.transactions.push(Transaction {
                        date,
                        flag,
                        payee,
                        narration,
                        tags,
                        postings,
                        line: start_line,
                    });
                    continue;
                }
                _ => {}
            }
        }

        i += 1;
    }

    Ok(ledger)
}

fn try_parse_date_prefix(line: &str) -> Option<(NaiveDate, &str)> {
    if line.len() < 10 {
        return None;
    }
    let date_str = &line[..10];
    let rest = line[10..].trim_start();
    NaiveDate::from_str(date_str).ok().map(|d| (d, rest))
}

fn parse_txn_header(s: &str) -> (Option<String>, String, Vec<String>) {
    // Format: ["Payee"] "Narration" #tag1 #tag2
    let s = s.trim();
    let mut tags = Vec::new();

    // Extract tags
    let mut parts_no_tags = Vec::new();
    for token in s.split_whitespace() {
        if token.starts_with('#') {
            tags.push(token.strip_prefix('#').unwrap().to_string());
        } else {
            parts_no_tags.push(token);
        }
    }
    // Rejoin without tags for quote parsing
    let no_tags: String = parts_no_tags.join(" ");
    let trimmed = no_tags.trim();

    // Count quoted strings
    let quoted: Vec<&str> = extract_quoted_strings(trimmed);
    match quoted.len() {
        0 => (None, trimmed.to_string(), tags),
        1 => (None, quoted[0].to_string(), tags),
        _ => (Some(quoted[0].to_string()), quoted[1].to_string(), tags),
    }
}

fn extract_quoted_strings(s: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '"' {
            let start = i + 1;
            let mut end = start;
            for (j, ch) in chars.by_ref() {
                if ch == '"' {
                    end = j;
                    break;
                }
            }
            result.push(&s[start..end]);
        }
    }
    result
}

fn parse_posting(line: &str) -> Option<Posting> {
    // Beancount uses 2+ spaces between account and amount; split_whitespace
    // handles any amount of whitespace correctly.
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let account = tokens.first()?.to_string();

    // Must start with a capital letter (beancount account type)
    if !account
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
    {
        return None;
    }

    if tokens.len() >= 3 {
        let amount_str = tokens[1].replace(',', "");
        let currency = tokens[2].to_string();
        if let Ok(amount) = Decimal::from_str(&amount_str) {
            return Some(Posting {
                account,
                amount: Some(amount),
                currency: Some(currency),
            });
        }
    }

    // Auto-completing posting (no amount)
    Some(Posting {
        account,
        amount: None,
        currency: None,
    })
}
