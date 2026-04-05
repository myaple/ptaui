# ptaui

A terminal UI for plain text accounting with [beancount](https://beancount.github.io/).

## Features

- **Account dashboard** — hierarchical view of all accounts with live balances
- **Transaction list** — scrollable, reverse-chronological transaction history
- **Add transactions** — guided double-entry form with account autocomplete
- **Reports** — monthly income vs expenses bar chart + summary table
- **bean-check integration** — validates your file after every write

## Quick Start

```bash
cargo build --release
./target/release/ptaui
```

On first run, a default config is created at `~/.config/ptaui/config.json`.

Point it at your beancount file:

```json
{
  "beancount_file": "~/finances/main.beancount",
  "currency": "USD",
  "auto_bean_check": true
}
```

A demo file is provided at `examples/demo.beancount`.

## Keybindings

| Key | Action |
|-----|--------|
| `1` | Accounts & Balances |
| `2` | Transaction List |
| `3` | Add Transaction |
| `4` | Reports |
| `r` | Reload ledger from disk |
| `q` | Quit |
| `↑↓` / `jk` | Scroll |
| `Tab` | Next field / autocomplete account |
| `Shift+Tab` | Previous field |
| `Enter` | Confirm / submit |
| `Esc` | Cancel / back |

## Dependencies

- Rust 1.75+
- `bean-check` (optional, from `pip install beancount`) for validation
