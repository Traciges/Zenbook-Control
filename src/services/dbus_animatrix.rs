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
use serde::{Deserialize, Serialize};
use zbus::zvariant::{OwnedValue, Type, Value};

use super::dbus::system_bus_connection;

// ── Hardware detection (DMI, no daemon required) ─────────────────────────────

/// Which AniMatrix panel variant this machine has, detected from DMI board name.
///
/// `Unsupported` means no AniMatrix hardware - the entire UI section is hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatrixHardwareType {
    GA401,
    GA402,
    GU604,
    G635L,
    G835L,
    Unsupported,
}

impl AnimatrixHardwareType {
    /// The string identifier asusd expects in `AnimeDataBuffer.anime` on the wire.
    pub fn as_dbus_str(self) -> &'static str {
        match self {
            Self::GA401 => "GA401",
            Self::GA402 => "GA402",
            Self::GU604 => "GU604",
            Self::G635L => "G635L",
            Self::G835L => "G835L",
            Self::Unsupported => "Unsupported",
        }
    }

}

/// Reads `/sys/class/dmi/id/board_name` and returns the AniMatrix hardware
/// variant for this machine. Returns `Unsupported` on any read error or when
/// the board name does not match a known AniMatrix model.
///
/// This check is completely independent of `asusd` and has no D-Bus overhead.
pub fn detect_animatrix_hardware() -> AnimatrixHardwareType {
    let Ok(board) = std::fs::read_to_string(crate::sys_paths::SYS_BOARD_NAME) else {
        return AnimatrixHardwareType::Unsupported;
    };
    let board = board.trim().to_uppercase();
    if board.contains("GA401I") || board.contains("GA401Q") {
        AnimatrixHardwareType::GA401
    } else if board.contains("GA402R") || board.contains("GA402X") || board.contains("GA402N") {
        AnimatrixHardwareType::GA402
    } else if board.contains("GU604V") {
        AnimatrixHardwareType::GU604
    } else if board.contains("G635L") {
        AnimatrixHardwareType::G635L
    } else if board.contains("G835L") {
        AnimatrixHardwareType::G835L
    } else {
        AnimatrixHardwareType::Unsupported
    }
}

// ── D-Bus status ─────────────────────────────────────────────────────────────

/// Reachability of the `xyz.ljones.Anime` D-Bus interface.
///
/// Hardware presence is checked separately via [`detect_animatrix_hardware`];
/// this enum only reflects whether `asusd` is running and exposing the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatrixStatus {
    Available,
    DaemonNotRunning,
}

// ── D-Bus types ──────────────────────────────────────────────────────────────

/// Four-string struct encoding built-in animation names per power state.
///
/// D-Bus wire type: `(ssss)`. Field order must match rog-anime's `Animations` struct
/// exactly (boot → awake → sleep → shutdown) or values will silently be swapped.
#[derive(Debug, Clone, Type, Value, OwnedValue)]
pub struct BuiltinAnimations {
    pub boot: String,
    pub awake: String,
    pub sleep: String,
    pub shutdown: String,
}

/// Single LED frame sent to asusd via the `Write` D-Bus method.
///
/// D-Bus wire type: `(ays)`. The `data` field contains one frame of raw LED
/// brightness values; `anime_type` is the string variant name (e.g. `"GA401"`).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Value, OwnedValue)]
pub struct DbusAnimeFrame {
    pub data: Vec<u8>,
    pub anime_type: String,
}

// ── Proxy definition ─────────────────────────────────────────────────────────

#[zbus::proxy(
    interface = "xyz.ljones.Anime",
    default_service = "xyz.ljones.Asusd",
    default_path = "/xyz/ljones/aura/anime"
)]
trait Animatrix {
    async fn run_main_loop(&self, start: bool) -> zbus::Result<()>;
    async fn write(&self, input: DbusAnimeFrame) -> zbus::Result<()>;

    #[zbus(property)]
    fn enable_display(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_enable_display(&self, value: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn brightness(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn set_brightness(&self, value: u32) -> zbus::Result<()>;

    #[zbus(property)]
    fn builtins_enabled(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_builtins_enabled(&self, value: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn builtin_animations(&self) -> zbus::Result<BuiltinAnimations>;
    #[zbus(property)]
    fn set_builtin_animations(&self, value: BuiltinAnimations) -> zbus::Result<()>;

    #[zbus(property)]
    fn off_when_unplugged(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_off_when_unplugged(&self, value: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn off_when_suspended(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_off_when_suspended(&self, value: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn off_when_lid_closed(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_off_when_lid_closed(&self, value: bool) -> zbus::Result<()>;
}

// ── Singleton proxy ───────────────────────────────────────────────────────────

static ANIMATRIX_PROXY: tokio::sync::OnceCell<AnimatrixProxy<'static>> =
    tokio::sync::OnceCell::const_new();

async fn animatrix_proxy() -> Result<&'static AnimatrixProxy<'static>, String> {
    ANIMATRIX_PROXY
        .get_or_try_init(|| async {
            let conn = system_bus_connection().await?;
            AnimatrixProxy::new(&conn)
                .await
                .map_err(|e| t!("error_dbus_proxy_create", error = e.to_string()).to_string())
        })
        .await
}

// ── Availability check ────────────────────────────────────────────────────────

/// Checks whether `asusd` is running and exposes the `xyz.ljones.Anime` interface.
///
/// Uses a fresh D-Bus connection to avoid caching a stale result.
/// Call only after [`detect_animatrix_hardware`] returns a supported variant.
pub async fn check_animatrix_status() -> AnimatrixStatus {
    let conn = match zbus::Connection::system().await {
        Ok(c) => c,
        Err(_) => return AnimatrixStatus::DaemonNotRunning,
    };

    let manager = match zbus::fdo::ObjectManagerProxy::builder(&conn)
        .destination("xyz.ljones.Asusd")
        .unwrap()
        .path("/")
        .unwrap()
        .build()
        .await
    {
        Ok(m) => m,
        Err(_) => return AnimatrixStatus::DaemonNotRunning,
    };

    let objects = match manager.get_managed_objects().await {
        Ok(o) => o,
        Err(_) => return AnimatrixStatus::DaemonNotRunning,
    };

    let has_anime = objects
        .values()
        .any(|ifaces| ifaces.contains_key("xyz.ljones.Anime"));

    if has_anime {
        AnimatrixStatus::Available
    } else {
        AnimatrixStatus::DaemonNotRunning
    }
}

// ── Public async helpers ──────────────────────────────────────────────────────

pub async fn get_animatrix_enable_display() -> Result<bool, String> {
    animatrix_proxy()
        .await?
        .enable_display()
        .await
        .map_err(|e| t!("error_animatrix_read", error = e.to_string()).to_string())
}

pub async fn set_animatrix_enable_display(value: bool) -> Result<bool, String> {
    animatrix_proxy()
        .await?
        .set_enable_display(value)
        .await
        .map_err(|e| t!("error_animatrix_write", error = e.to_string()).to_string())?;
    Ok(value)
}

pub async fn get_animatrix_brightness() -> Result<u32, String> {
    animatrix_proxy()
        .await?
        .brightness()
        .await
        .map_err(|e| t!("error_animatrix_read", error = e.to_string()).to_string())
}

pub async fn set_animatrix_brightness(value: u32) -> Result<u32, String> {
    animatrix_proxy()
        .await?
        .set_brightness(value)
        .await
        .map_err(|e| t!("error_animatrix_write", error = e.to_string()).to_string())?;
    Ok(value)
}

pub async fn get_animatrix_builtins_enabled() -> Result<bool, String> {
    animatrix_proxy()
        .await?
        .builtins_enabled()
        .await
        .map_err(|e| t!("error_animatrix_read", error = e.to_string()).to_string())
}

pub async fn set_animatrix_builtins_enabled(value: bool) -> Result<bool, String> {
    animatrix_proxy()
        .await?
        .set_builtins_enabled(value)
        .await
        .map_err(|e| t!("error_animatrix_write", error = e.to_string()).to_string())?;
    Ok(value)
}

pub async fn get_animatrix_builtin_animations() -> Result<BuiltinAnimations, String> {
    animatrix_proxy()
        .await?
        .builtin_animations()
        .await
        .map_err(|e| t!("error_animatrix_read", error = e.to_string()).to_string())
}

pub async fn set_animatrix_builtin_animations(value: BuiltinAnimations) -> Result<(), String> {
    animatrix_proxy()
        .await?
        .set_builtin_animations(value)
        .await
        .map_err(|e| t!("error_animatrix_write", error = e.to_string()).to_string())
}

pub async fn get_animatrix_off_when_unplugged() -> Result<bool, String> {
    animatrix_proxy()
        .await?
        .off_when_unplugged()
        .await
        .map_err(|e| t!("error_animatrix_read", error = e.to_string()).to_string())
}

pub async fn set_animatrix_off_when_unplugged(value: bool) -> Result<bool, String> {
    animatrix_proxy()
        .await?
        .set_off_when_unplugged(value)
        .await
        .map_err(|e| t!("error_animatrix_write", error = e.to_string()).to_string())?;
    Ok(value)
}

pub async fn get_animatrix_off_when_suspended() -> Result<bool, String> {
    animatrix_proxy()
        .await?
        .off_when_suspended()
        .await
        .map_err(|e| t!("error_animatrix_read", error = e.to_string()).to_string())
}

pub async fn set_animatrix_off_when_suspended(value: bool) -> Result<bool, String> {
    animatrix_proxy()
        .await?
        .set_off_when_suspended(value)
        .await
        .map_err(|e| t!("error_animatrix_write", error = e.to_string()).to_string())?;
    Ok(value)
}

pub async fn get_animatrix_off_when_lid_closed() -> Result<bool, String> {
    animatrix_proxy()
        .await?
        .off_when_lid_closed()
        .await
        .map_err(|e| t!("error_animatrix_read", error = e.to_string()).to_string())
}

pub async fn set_animatrix_off_when_lid_closed(value: bool) -> Result<bool, String> {
    animatrix_proxy()
        .await?
        .set_off_when_lid_closed(value)
        .await
        .map_err(|e| t!("error_animatrix_write", error = e.to_string()).to_string())?;
    Ok(value)
}

/// Stops (`false`) or restores (`true`) the asusd built-in animation main loop.
///
/// Call `run_main_loop(false)` before streaming GIF frames via [`write_animatrix_frame`],
/// and `run_main_loop(true)` when done to hand control back to asusd.
pub async fn animatrix_run_main_loop(start: bool) -> Result<(), String> {
    animatrix_proxy()
        .await?
        .run_main_loop(start)
        .await
        .map_err(|e| t!("error_animatrix_write", error = e.to_string()).to_string())
}

/// Sends one decoded LED frame to the AniMatrix panel.
pub async fn animatrix_write_frame(frame: DbusAnimeFrame) -> Result<(), String> {
    animatrix_proxy()
        .await?
        .write(frame)
        .await
        .map_err(|e| t!("error_animatrix_write", error = e.to_string()).to_string())
}
