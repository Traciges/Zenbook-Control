// Ayuz - Unofficial Control Center for Asus Laptops
// Copyright (C) 2026 Guido Philipp
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see https://www.gnu.org/licenses/.

use rust_i18n::t;

/// Runs a program with arguments on a blocking thread and returns success or an i18n error string.
///
/// Offloads the synchronous [`std::process::Command`] call to a `spawn_blocking` thread so it
/// does not stall the async runtime. Returns `Err` on spawn failure, non-zero exit code, or
/// if the blocking task itself panics.
pub(crate) async fn run_command_blocking(program: &str, args: &[&str]) -> Result<(), String> {
    let program_name = program.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();

    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&program_name)
            .args(&args)
            .status()
    })
    .await;

    match result {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(status)) => Err(t!(
            "error_cmd_exit_code",
            cmd = program,
            code = status.code().unwrap_or(-1).to_string()
        )
        .to_string()),
        Ok(Err(e)) => Err(t!("error_cmd_start", cmd = program, error = e.to_string()).to_string()),
        Err(e) => Err(t!("error_spawn_blocking", error = e.to_string()).to_string()),
    }
}

/// Reads a privileged file using `pkexec cat <path>` and returns its trimmed contents.
///
/// Accepts only `&'static str` paths to prevent dynamic injection.
/// Returns `Err` on spawn failure, non-zero exit code, or task panic.
pub(crate) async fn pkexec_read_file(path: &'static str) -> Result<String, String> {
    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new("pkexec")
            .args(["cat", path])
            .output()
    })
    .await;

    match result {
        Ok(Ok(out)) if out.status.success() => {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        Ok(Ok(out)) => Err(t!(
            "error_cmd_exit_code",
            cmd = "pkexec",
            code = out.status.code().unwrap_or(-1).to_string()
        )
        .to_string()),
        Ok(Err(e)) => {
            Err(t!("error_cmd_start", cmd = "pkexec", error = e.to_string()).to_string())
        }
        Err(e) => Err(t!("error_spawn_blocking", error = e.to_string()).to_string()),
    }
}

/// Writes `value` to a sysfs `path` using `pkexec tee <path>`.
///
/// Both `path` and `value` are `&'static str` to prevent dynamic injection.
/// Returns `Err` on spawn failure, non-zero exit code, or task panic.
pub(crate) async fn pkexec_write_sysfs(path: &'static str, value: &'static str) -> Result<(), String> {
    let result = tokio::task::spawn_blocking(move || {
        use std::io::Write;
        let mut child = std::process::Command::new("pkexec")
            .args(["tee", path])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(format!("{value}\n").as_bytes());
        }
        child.wait()
    })
    .await;

    match result {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(status)) => Err(t!(
            "error_cmd_exit_code",
            cmd = "pkexec",
            code = status.code().unwrap_or(-1).to_string()
        )
        .to_string()),
        Ok(Err(e)) => {
            Err(t!("error_cmd_start", cmd = "pkexec", error = e.to_string()).to_string())
        }
        Err(e) => Err(t!("error_spawn_blocking", error = e.to_string()).to_string()),
    }
}

/// Returns `true` if `program` resolves on `$PATH` (via `which`).
///
/// Runs on `spawn_blocking` so the async runtime is not stalled. A panic or
/// cancellation of the blocking task is logged via `tracing::warn!` and
/// reported as `false`; a clean non-zero exit from `which` is reported as
/// `false` without logging (that's the expected "not installed" path).
pub(crate) async fn which_exists(program: &'static str) -> bool {
    let join = tokio::task::spawn_blocking(move || {
        std::process::Command::new("which")
            .arg(program)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
    .await;

    match join {
        Ok(found) => found,
        Err(e) => {
            tracing::warn!("which {}: spawn_blocking failed: {}", program, e);
            false
        }
    }
}

static QDBUS_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Returns the path to the `qdbus` executable, resolved once and cached.
///
/// Searches `$PATH` for `qdbus`, `qdbus6`, and `qdbus-qt6` in order, then falls back to a
/// set of known fixed locations (Fedora `/usr/bin/qdbus6`, Arch Qt6/Qt5 lib paths).
/// Returns the bare string `"qdbus"` as a last resort if nothing is found.
pub(crate) fn resolve_qdbus_path() -> &'static str {
    QDBUS_PATH.get_or_init(|| {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in path_var.split(':') {
                for name in ["qdbus", "qdbus6", "qdbus-qt6"] {
                    let candidate = std::path::Path::new(dir).join(name);
                    if is_executable(&candidate) {
                        return candidate.to_string_lossy().into_owned();
                    }
                }
            }
        }
        for fixed in [
            "/usr/bin/qdbus6",
            "/usr/lib/qt6/bin/qdbus6",
            "/usr/lib/qt6/bin/qdbus",
            "/usr/lib/qt5/bin/qdbus",
        ] {
            if is_executable(std::path::Path::new(fixed)) {
                return fixed.to_owned();
            }
        }
        "qdbus".to_owned()
    })
}

fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn desktop_is(name: &str) -> bool {
    let name_upper = name.to_uppercase();
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_uppercase().contains(&name_upper))
        .unwrap_or(false)
}

/// Returns `true` if the current desktop session is KDE Plasma.
pub(crate) fn is_kde_desktop() -> bool {
    desktop_is("KDE")
}

/// Returns `true` if the current desktop session is GNOME.
pub(crate) fn is_gnome_desktop() -> bool {
    desktop_is("GNOME")
}
