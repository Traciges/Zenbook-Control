// Asus Hub - Unofficial Control Center for Asus Laptops
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

/// Runs a shell command with elevated privileges via `pkexec sh -c <command>`.
///
/// Prompts the user for authentication through the system's PolicyKit agent.
/// Prefer this over embedding `sudo` calls directly in command strings.
pub(crate) async fn pkexec_shell(command: &str) -> Result<(), String> {
    run_command_blocking("pkexec", &["sh", "-c", command]).await
}

static QDBUS_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Returns the path to the `qdbus` executable, resolved once and cached.
///
/// Checks in order: `qdbus` in `$PATH`, `/usr/lib/qt6/bin/qdbus` (Arch Linux Qt6),
/// `/usr/lib/qt5/bin/qdbus` (Arch Linux Qt5). Falls back to `"qdbus"` if none are found.
pub(crate) fn resolve_qdbus_path() -> &'static str {
    QDBUS_PATH.get_or_init(|| {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in path_var.split(':') {
                let candidate = std::path::Path::new(dir).join("qdbus");
                if is_executable(&candidate) {
                    return candidate.to_string_lossy().into_owned();
                }
            }
        }
        if is_executable(std::path::Path::new("/usr/lib/qt6/bin/qdbus")) {
            return "/usr/lib/qt6/bin/qdbus".to_owned();
        }
        if is_executable(std::path::Path::new("/usr/lib/qt5/bin/qdbus")) {
            return "/usr/lib/qt5/bin/qdbus".to_owned();
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

/// Returns `true` if the current desktop session is KDE Plasma.
///
/// Checks the `XDG_CURRENT_DESKTOP` environment variable for the substring `"KDE"` (case-insensitive).
pub(crate) fn is_kde_desktop() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_uppercase().contains("KDE"))
        .unwrap_or(false)
}
