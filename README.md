# SQL Job Monitor

A native Windows desktop app for watching and controlling SQL Server Agent jobs. Connect once, then monitor activity, inspect history, and start or stop jobs without opening SQL Server Management Studio.

**[Download the Windows installer](https://github.com/keith-k2/sql-job-monitor/releases/latest)**

Windows 10 or later. The installer adds SQL Job Monitor to the Start menu.

## What it does

- Connect with the current Windows account or a SQL Server login, including named instances.
- See every job’s status, enabled state, current step, last run, duration, next run, owner, and category.
- Filter by status, search, sort, and resize columns. Running jobs stay pinned at the top.
- Open recent history and the steps inside each run.
- Switch to a zoomable timeline that shows how long each job and step took.
- Start a job, start it at a chosen step, stop it, or enable and disable it.
- Refresh manually or on a timer. The last successful connection signs in again the next time the app starts.
- Remember the window layout, theme, and history pane. A pin keeps the window above other windows.

SQL Server Agent is not available in SQL Server Express. Azure Elastic Jobs are outside this app’s scope.

## Install

1. Open the [latest release](https://github.com/keith-k2/sql-job-monitor/releases/latest).
2. Download `sql-job-monitor-0.1.0-x86_64.msi`.
3. Run the installer. Windows asks for administrator approval because the app is installed for all users on the computer.
4. Launch **SQL Job Monitor** from the Start menu.

To remove it, use **Apps** in Windows Settings.

## Connect

The login needs permission to read SQL Server Agent metadata in `msdb`.

- `SQLAgentReaderRole` can monitor the jobs that account is allowed to see.
- `SQLAgentOperatorRole` can also start and stop eligible local jobs.
- `sysadmin` is not required.

SQL Server must accept TCP/IP connections. A named instance also needs SQL Server Browser and UDP 1434, unless you enter its fixed port.

Certificate validation stays on unless **Trust server certificate** is checked for that connection. Use that override only for a server whose certificate cannot be validated.

## Build from source

Install the current stable Rust toolchain from [rustup](https://rustup.rs/), then:

```powershell
cargo run --release
```

The executable is written to `target\release\sql-job-monitor.exe`.

Checks:

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Live SQL tests stay ignored unless a server is provided:

```powershell
$env:SQL_JOB_MONITOR_TEST_SERVER = 'SERVER\INSTANCE'
cargo test --test sql_integration -- --ignored
```

For a SQL login, also set `SQL_JOB_MONITOR_TEST_USER` and `SQL_JOB_MONITOR_TEST_PASSWORD`. Set `SQL_JOB_MONITOR_TEST_PORT` for a fixed port, or `SQL_JOB_MONITOR_TEST_TRUST_CERT=1` only for a development server.

### Installer

The MSI is built with [cargo-wix](https://github.com/volks73/cargo-wix) and WiX Toolset 3.14:

```powershell
cargo install cargo-wix
cargo wix --bin-path C:\path\to\wix314
```

`target\wix\sql-job-monitor-0.1.0-x86_64.msi` is the installer. Its upgrade identifiers live in `Cargo.toml` and `wix\main.wxs`.

## Security

- A connection is saved only after a successful login when **Remember this connection** is checked.
- Server settings are stored in `%APPDATA%\SQL Job Monitor\connections.json`. SQL passwords go to Windows Credential Manager for the current Windows user, not into that file.
- Passwords and connection strings are not written to logs.
- Connections use TLS. Trusting the server certificate is an explicit, per-connection choice.
- Queries time out, and a newer refresh replaces an older result that arrives late.

## License

[MIT](LICENSE). The icons in `assets/lucide` are from [Lucide](https://lucide.dev/) and use the ISC license included in that folder.
