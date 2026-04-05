# CLAUDE.md

This file provides guidance to Claude Code when working with the ptaui repository.

## Project Overview

`ptaui` is a terminal UI (TUI) for plain text accounting powered by [beancount](https://beancount.github.io/). It's written in Rust and uses `ratatui` for the terminal interface.

## Build & Test Commands

```bash
# Build
cargo build

# Build release
cargo build --release

# Run
cargo run

# Run tests
cargo test

# Lint (check for warnings and errors)
cargo clippy

# Format code
cargo fmt

# Format check (no modifications)
cargo fmt --check
```

## Architecture

```
src/
  main.rs              # Entry point, event loop
  app.rs               # Application state machine (App struct)
  config.rs            # Config loading/saving (~/.config/ptaui/config.json)
  events.rs            # Terminal event handling
  git.rs               # Optional git integration
  beancount/
    mod.rs             # Module re-exports
    parser.rs          # Beancount file parser
    validator.rs       # bean-check integration
    writer.rs          # Writing transactions to beancount file
  ui/
    mod.rs             # UI module re-exports
    startup.rs         # Startup/loading screen
    dashboard.rs       # Account balances tab (key: 1)
    transactions.rs    # Transaction list tab (key: 2)
    add_tx.rs          # Add transaction form (key: a)
    add_account.rs     # Add account form
    reports.rs         # Income/expense reports tab (key: 4)
    category_transactions.rs  # Category breakdown view
    account_filter.rs  # Account filter modal
examples/
  demo.beancount       # Sample beancount file for testing
```

## Key Patterns

- **App state** is centralized in `app.rs` (`App` struct). All UI tabs read from and write to this struct.
- **Tabs** are selected with number keys (`1`-`4`); each tab has its own UI module under `src/ui/`.
- **beancount file** is read on startup and on `r` keypress; written when adding transactions/accounts.
- After every write, `bean-check` is optionally invoked for validation (controlled by `auto_bean_check` config).

## External Dependencies

- `bean-check`: `pip install beancount` — used for ledger validation after writes

## Configuration

Default config location: `~/.config/ptaui/config.json`

```json
{
  "beancount_file": "~/finances/main.beancount",
  "currency": "USD",
  "auto_bean_check": true
}
```
