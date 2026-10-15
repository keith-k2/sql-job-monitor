# SQL Job Monitor

A native desktop app for watching and controlling SQL Server Agent jobs on **Windows**, **macOS**, and **Linux**. Connect once, then monitor activity, inspect history, and start or stop jobs without opening SQL Server Management Studio.

One codebase, one version in `Cargo.toml`. The same release commit produces every installer.

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

Installers are named from the version in `Cargo.toml` and written to `dist/`.

### Windows

1. Open the [latest release](https://github.com/keith-k2/sql-job-monitor/releases/latest).
2. Download `sql-job-monitor-*-x86_64.msi`.
3. Run the installer. Windows asks for administrator approval because the app is installed for all users on the computer.
4. Launch **SQL Job Monitor** from the Start menu.

To remove it, use **Apps** in Windows Settings.

### macOS

1. Open the [latest release](https://github.com/keith-k2/sql-job-monitor/releases/latest).
2. Download `sql-job-monitor-*-macos.dmg`. The disk image is a universal binary for Apple silicon and Intel.
3. Open the disk image and drag **SQL Job Monitor** into Applications.
4. The first launch of an unsigned build needs a right-click on the app, then **Open**.

macOS 11 or later.

### Linux

1. Open the [latest release](https://github.com/keith-k2/sql-job-monitor/releases/latest).
2. Download `sql-job-monitor-*-linux-x86_64.AppImage`.
3. Mark it executable and run it:

```bash
chmod +x sql-job-monitor-*-linux-x86_64.AppImage
./sql-job-monitor-*-linux-x86_64.AppImage
```

A `.tar.gz` of the same build is also attached if you prefer to unpack it yourself. Saved passwords use the desktop Secret Service (GNOME Keyring, KWallet, or equivalent). Use a SQL login; Windows integrated authentication is a Windows-only feature.

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

Install the current stable Rust toolchain from [rustup](https://rustup.rs/). The UI, SQL, and settings live in one tree; platform differences are isolated in `src/platform.rs` and `packaging/`.

```
cargo run --release
```

| OS | Executable | Installer command | Output in `dist/` |
|---|---|---|---|
| Windows | `target\release\sql-job-monitor.exe` | `scripts\package-windows.ps1` | `sql-job-monitor-<version>-x86_64.msi` |
| macOS | `target/release/sql-job-monitor` | `scripts/package-macos.sh` | `sql-job-monitor-<version>-macos.dmg` |
| Linux | `target/release/sql-job-monitor` | `scripts/package-linux.sh` | `.AppImage` and `.tar.gz` |

Bump the version only in `Cargo.toml`. `scripts/read-version.sh` feeds every packager.

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

The Windows MSI still needs [cargo-wix](https://github.com/volks73/cargo-wix) and WiX Toolset 3.14. The Linux AppImage needs [appimagetool](https://github.com/AppImage/appimagetool) on a Linux host.

## Security

- A connection is saved only after a successful login when **Remember this connection** is checked.
- Server settings are stored in `%APPDATA%\SQL Job Monitor` on Windows, `~/Library/Application Support/SQL Job Monitor` on macOS, or `~/.config/sql-job-monitor` on Linux. SQL passwords go to the OS credential store for the current user, not into that file.
- Passwords and connection strings are not written to logs.
- Connections use TLS. Trusting the server certificate skips identity checks and is the usual setting for SQL Server 2012.
- Queries time out, and a newer refresh replaces an older result that arrives late.

## License

[MIT](LICENSE). The icons in `assets/lucide` are from [Lucide](https://lucide.dev/) and use the ISC license included in that folder.
