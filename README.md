# ptaui

A terminal UI for [beancount](https://beancount.github.io/) plain text accounting.

## Features

- **Accounts dashboard** — hierarchical view of assets and liabilities with color-coded balances and net worth summary
- **Transaction list** — scrollable reverse-chronological history with account filtering and reconciliation mode
- **Add/edit/delete transactions** — double-entry forms with payee memory (auto-fills category and account from last transaction with same payee)
- **CSV import** — 4-step wizard with column mapping, date format configuration, duplicate detection, payee-based category auto-fill, and bulk category assignment
- **Reports** — monthly income vs expenses bar chart, per-category breakdowns with drill-down into individual transactions
- **bean-check** — validates your ledger after every write
- **Git integration** — optional auto-commits after every change to your beancount file

## Quick Start

```bash
cargo build --release
./target/release/ptaui
```

On first run, a config is created at `~/.config/ptaui/config.json`. Edit it to point at your ledger:

```json
{
  "beancount_file": "~/finances/main.beancount",
  "currency": "USD",
  "auto_bean_check": true
}
```

A sample ledger is at `examples/demo.beancount`.

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `1` | Accounts tab |
| `2` | Transactions tab |
| `3` | Reports tab |
| `q` | Quit |
| `r` | Reload ledger from disk |

### Accounts tab

| Key | Action |
|-----|--------|
| `j/k` or arrows | Scroll |
| `a` | Add account |

### Transactions tab

| Key | Action |
|-----|--------|
| `j/k` or arrows | Navigate |
| `a` | Add transaction |
| `e` | Edit transaction |
| `d` | Delete transaction |
| `f` | Filter by account |
| `i` | CSV import |
| `R` (shift) | Reconcile mode |

### Reconcile mode

| Key | Action |
|-----|--------|
| `Space` | Toggle selection |
| `r` | Mark reconciled (`*`) |
| `u` | Mark unreconciled (`!`) |
| `Esc` | Exit reconcile mode |

### Reports tab

| Key | Action |
|-----|--------|
| `Tab` | Toggle monthly / category breakdown |
| `h/l` or arrows | Change period |
| `m` / `y` | Switch month / year view |
| `c` | Filter accounts |
| `Enter` | Drill into category transactions |

### Forms (add/edit transaction, add account)

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Next / previous field |
| `Tab` (on payee/account/category) | Autocomplete |
| `Enter` | Confirm |
| `Esc` | Cancel |

### CSV import wizard

| Key | Action |
|-----|--------|
| `Tab` | Path completion / autocomplete / next field |
| `Left/Right` | Cycle column mapping |
| `Space` | Toggle include row / toggle options |
| `e` | Edit category for row |
| `c` | Apply category to same payee (empty only) |
| `C` | Apply category to all same payee |
| `Enter` | Confirm step / commit import |
| `Esc` | Back / cancel |

## Dependencies

- Rust 1.75+
- `bean-check` (optional, `pip install beancount`) for ledger validation
