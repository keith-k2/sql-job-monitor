use std::path::PathBuf;

pub fn windows_auth_available() -> bool {
    cfg!(windows)
}

pub fn app_data_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("SQL Job Monitor")
    } else if cfg!(target_os = "macos") {
        home_dir().join("Library/Application Support/SQL Job Monitor")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".config"))
            .join("sql-job-monitor")
        // Secret Service (GNOME Keyring / KWallet) stores the SQL password.
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn open_in_browser(url: &str) {
    let _ = open_in_browser_result(url);
}

#[cfg(windows)]
fn open_in_browser_result(url: &str) -> std::io::Result<()> {
    open_windows(url)
}

#[cfg(target_os = "macos")]
fn open_in_browser_result(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open").arg(url).spawn()?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_in_browser_result(url: &str) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(url).spawn()?;
    Ok(())
}

#[cfg(windows)]
fn open_windows(url: &str) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let operation: Vec<u16> = std::ffi::OsStr::new("open")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let file: Vec<u16> = std::ffi::OsStr::new(url)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // ShellExecuteW hands the URL to the default browser and never creates a console.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result > 32 {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "the default browser could not be opened",
        ))
    }
}

#[cfg(windows)]
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut core::ffi::c_void,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> isize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_settings_under_the_app_name() {
        assert!(app_data_dir()
            .file_name()
            .is_some_and(|name| name == "SQL Job Monitor" || name == "sql-job-monitor"));
    }
}
