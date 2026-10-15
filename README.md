# SQL Job Monitor

A native desktop app for watching and controlling SQL Server Agent jobs on **Windows** and **macOS**. Connect once, then monitor activity, inspect history, and start or stop jobs without opening SQL Server Management Studio.

**[Download the latest installer](https://github.com/keith-k2/sql-job-monitor/releases/latest)**

## What it does

- Connect with a SQL Server login, including named instances. On Windows, the current Windows account is also available.
- See every job’s status, enabled state, current step, last run, duration, next run, owner, and category.
- Filter by status, search, sort, and resize columns. Running jobs stay pinned at the top.
- Open recent history and the steps inside each run.
- Switch to a zoomable timeline that shows how long each job and step took.
- Start a job, start it at a chosen step, stop it, or enable and disable it.
- Refresh manually or on a timer. The last successful connection signs in again the next time the app starts.
- Remember the window layout, theme, and history pane. A pin keeps the window above other windows.

SQL Server Agent is not available in SQL Server Express. Azure Elastic Jobs are outside this app’s scope.

## Install

### Windows

1. Open the [latest release](https://github.com/keith-k2/sql-job-monitor/releases/latest).
2. Download `sql-job-monitor-0.1.0-x86_64.msi`.
3. Run the installer. Windows asks for administrator approval because the app is installed for all users on the computer.
4. Launch **SQL Job Monitor** from the Start menu.

To remove it, use **Apps** in Windows Settings.

### macOS

1. Open the [latest release](https://github.com/keith-k2/sql-job-monitor/releases/latest).
2. Download `sql-job-monitor-0.1.0-macos.dmg`. The disk image is a universal binary for Apple silicon and Intel.
3. Open the disk image and drag **SQL Job Monitor** into Applications.
4. The first launch of an unsigned build needs a right-click on the app, then **Open**.

macOS 11 or later. Use a SQL login; Windows integrated authentication is a Windows-only feature.

## Connect

The login needs permission to read SQL Server Agent metadata in `msdb`.

- `SQLAgentReaderRole` can monitor the jobs that account is allowed to see.
- `SQLAgentOperatorRole` can also start and stop eligible local jobs.
- `sysadmin` is not required.

SQL Server must accept TCP/IP connections. A named instance also needs SQL Server Browser and UDP 1434, unless you enter its fixed port.

The connection is always encrypted with TLS 1.2 through rustls. macOS Secure Transport closes SQL Server's handshake, so this app does not use it.

SQL Server 2012 and most on-prem Agent hosts present a self-signed certificate, which identity checks reject. **Trust server certificate** is on by default so that handshake succeeds; the password and queries still travel over TLS. If that box is unchecked and the certificate is still self-signed, the app retries once with the certificate accepted. Uncheck it only when the server has a certificate issued by a CA that the computer already trusts.

SQL Server 2012 needs a later service pack or cumulative update that enables TLS 1.2 (SP2 CU or newer). Without that update, modern clients cannot complete the encrypted login.

## Build from source

Install the current stable Rust toolchain from [rustup](https://rustup.rs/).

Windows:

```powershell
cargo run --release
```

The executable is written to `target\release\sql-job-monitor.exe`.

macOS:

```bash
cargo run --release
```

The executable is written to `target/release/sql-job-monitor`. Package a drag-install disk image with:

```bash
chmod +x scripts/package-macos.sh
scripts/package-macos.sh
```

`target/macos/sql-job-monitor-0.1.0-macos.dmg` is the installer.

Checks:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Live SQL tests stay ignored unless a server is provided:

```
SQL_JOB_MONITOR_TEST_SERVER=SERVER\INSTANCE
cargo test --test sql_integration -- --ignored
```

For a SQL login, also set `SQL_JOB_MONITOR_TEST_USER` and `SQL_JOB_MONITOR_TEST_PASSWORD`. Set `SQL_JOB_MONITOR_TEST_PORT` for a fixed port. Certificate identity checks are retried automatically when SQL Server 2012 presents its default self-signed certificate.

### Windows MSI

The MSI is built with [cargo-wix](https://github.com/volks73/cargo-wix) and WiX Toolset 3.14:

```powershell
cargo install cargo-wix
cargo wix --bin-path C:\path\to\wix314
```

`target\wix\sql-job-monitor-0.1.0-x86_64.msi` is the installer. Its upgrade identifiers live in `Cargo.toml` and `wix\main.wxs`.

## Security

- A connection is saved only after a successful login when **Remember this connection** is checked.
- Server settings are stored in `%APPDATA%\SQL Job Monitor\connections.json` on Windows, or `~/Library/Application Support/SQL Job Monitor/connections.json` on macOS. SQL passwords go to the OS credential store for the current user, not into that file.
- Passwords and connection strings are not written to logs.
- Connections use TLS. Trusting the server certificate skips identity checks and is the usual setting for SQL Server 2012.
- Queries time out, and a newer refresh replaces an older result that arrives late.

## License

[MIT](LICENSE). The icons in `assets/lucide` are from [Lucide](https://lucide.dev/) and use the ISC license included in that folder.
