# SQL Job Monitor

A native Windows desktop monitor for SQL Server Agent jobs, built with Rust and Slint.

## MVP features

- Current Windows account (SSPI) and SQL Server authentication
- Default TLS certificate validation, with an explicit certificate override
- Host/port and `SERVER\INSTANCE` connections through SQL Server Browser
- Auto-refreshing job state, last result, duration, current step, and next run
- Search, status filters, and sortable name/status/last-run columns
- Recent job and step history
- Guarded start and stop controls
- Last-good-data retention and actionable connection/permission errors

SQL Server Agent is not available in SQL Server Express. Azure Elastic Jobs are outside this
MVP's scope.

## Prerequisites

1. Windows 10 or later.
2. The current stable Rust toolchain from [rustup](https://rustup.rs/).
3. SQL Server TCP/IP enabled. Named instances also need SQL Server Browser and UDP 1434.
4. A SQL Server login or Windows account that can read SQL Server Agent metadata.

Recommended `msdb` role membership:

- `SQLAgentReaderRole` for monitoring jobs visible to the account.
- `SQLAgentOperatorRole` to start/stop eligible local jobs.
- `sysadmin` is not required and should not be granted solely for this application.

SQL Server Agent roles still enforce ownership and visibility rules documented by Microsoft.

## Build and run

```powershell
cargo run
```

Quality checks:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

The portable executable is created at `target\release\sql-job-monitor.exe`.

## Test against a SQL Server

Live tests are ignored by default. Windows authentication only needs the server variable:

```powershell
$env:SQL_JOB_MONITOR_TEST_SERVER = 'SERVER\INSTANCE'
cargo test --test sql_integration -- --ignored
```

For SQL authentication, also set `SQL_JOB_MONITOR_TEST_USER` and
`SQL_JOB_MONITOR_TEST_PASSWORD`. Set `SQL_JOB_MONITOR_TEST_PORT` for a fixed port or
`SQL_JOB_MONITOR_TEST_TRUST_CERT=1` only for a development server whose certificate cannot be
validated.

## Windows MSI

Install the packaging command and WiX Toolset 3.14 once, then build:

```powershell
cargo install cargo-wix
cargo wix
```

The MSI is written under `target\wix`. The installer metadata and stable upgrade identifiers are
in `Cargo.toml` and `wix\main.wxs`. If WiX is kept as portable binaries instead of installed,
pass its directory with `cargo wix --bin-path C:\path\to\wix314`.

## Security

- A connection is saved only when **Remember this connection** is checked and the login succeeds.
- Saved server settings are stored in `%APPDATA%\SQL Job Monitor\connections.json`. SQL passwords are stored in Windows Credential Manager for the current Windows user, not in that file.
- Passwords and full connection strings are never logged.
- TLS encryption and Windows certificate validation are enabled by default.
- “Trust server certificate” is an explicit per-connection override. It encrypts traffic but
  disables identity validation and should be limited to controlled development environments.
- Query and connection operations time out, and UI updates reject stale refresh results.

