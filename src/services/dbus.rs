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

#[zbus::proxy(
    interface = "xyz.ljones.Platform",
    default_service = "xyz.ljones.Asusd",
    default_path = "/xyz/ljones"
)]
trait Platform {
    #[zbus(property)]
    fn charge_control_end_threshold(&self) -> zbus::Result<u8>;
    #[zbus(property)]
    fn set_charge_control_end_threshold(&self, value: u8) -> zbus::Result<()>;

    #[zbus(property)]
    fn platform_profile(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn set_platform_profile(&self, value: u32) -> zbus::Result<()>;
}

/// Fan/platform power profile exposed by the `asusd` daemon.
///
/// Maps directly to the integer values used by the `platform_profile` D-Bus property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FanProfile {
    /// Balanced power and thermal performance (default, value `0`).
    Balanced = 0,
    /// Maximum CPU/GPU boost, higher fan speed (value `1`).
    Performance = 1,
    /// Reduced fan noise and power draw (value `2`).
    Quiet = 2,
    /// Low-power mode used on TUF laptops in place of Quiet (value `3`).
    LowPower = 3,
}

impl From<u32> for FanProfile {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::Performance,
            2 => Self::Quiet,
            3 => Self::LowPower,
            _ => Self::Balanced,
        }
    }
}

/// Lazily-initialized singleton proxy to the `xyz.ljones.Asusd` D-Bus service.
///
/// The proxy is created once on first use and reused for all subsequent calls,
/// avoiding repeated connection overhead.
static PLATFORM_PROXY: tokio::sync::OnceCell<PlatformProxy<'static>> =
    tokio::sync::OnceCell::const_new();

/// Opens a system D-Bus connection, mapping errors to localised strings.
async fn system_bus_connection() -> Result<zbus::Connection, String> {
    zbus::Connection::system()
        .await
        .map_err(|e| t!("error_dbus_connect", error = e.to_string()).to_string())
}

/// Returns a reference to the shared [`PlatformProxy`], initialising it on first call.
async fn platform_proxy() -> Result<&'static PlatformProxy<'static>, String> {
    PLATFORM_PROXY
        .get_or_try_init(|| async {
            let conn = system_bus_connection().await?;
            PlatformProxy::new(&conn)
                .await
                .map_err(|e| t!("error_dbus_proxy_create", error = e.to_string()).to_string())
        })
        .await
}

/// Returns `true` if the `asusd` D-Bus service is reachable.
///
/// Opens a fresh system bus connection each time to avoid caching a stale result.
/// Does not initialise the shared [`PLATFORM_PROXY`].
pub async fn check_asusd_available() -> bool {
    let conn = match zbus::Connection::system().await {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Ok(proxy) = PlatformProxy::new(&conn).await {
        proxy.platform_profile().await.is_ok()
    } else {
        false
    }
}

/// Reads the current battery charge end-threshold from `asusd` (typically 80 or 100).
pub async fn get_charge_limit() -> Result<u8, String> {
    let proxy = platform_proxy().await?;
    proxy
        .charge_control_end_threshold()
        .await
        .map_err(|e| t!("error_charge_limit_read", error = e.to_string()).to_string())
}

/// Sets the battery charge end-threshold via `asusd` and returns the applied value.
///
/// Pass `80` for maintenance/health mode or `100` for a full charge.
pub async fn set_charge_limit(value: u8) -> Result<u8, String> {
    let proxy = platform_proxy().await?;
    proxy
        .set_charge_control_end_threshold(value)
        .await
        .map_err(|e| t!("error_charge_limit_write", error = e.to_string()).to_string())?;
    Ok(value)
}

/// Reads the active fan/platform profile from `asusd`.
pub async fn get_fan_profile() -> Result<FanProfile, String> {
    let proxy = platform_proxy().await?;
    proxy
        .platform_profile()
        .await
        .map(FanProfile::from)
        .map_err(|e| t!("error_fan_profile_read", error = e.to_string()).to_string())
}

/// Applies a fan/platform profile via `asusd` and returns the applied profile on success.
///
/// If the requested profile is [`FanProfile::Quiet`] and the daemon returns a
/// `NotSupported` error (e.g. on TUF laptops that only expose `low-power`),
/// the function automatically retries with [`FanProfile::LowPower`] and returns
/// that variant on success.
pub async fn set_fan_profile(profile: FanProfile) -> Result<FanProfile, String> {
    let proxy = platform_proxy().await?;
    match proxy.set_platform_profile(profile as u32).await {
        Ok(_) => Ok(profile),
        Err(e) if profile == FanProfile::Quiet && e.to_string().contains("NotSupported") => {
            proxy
                .set_platform_profile(FanProfile::LowPower as u32)
                .await
                .map(|_| FanProfile::LowPower)
                .map_err(|e2| t!("error_fan_profile_write", error = e2.to_string()).to_string())
        }
        Err(e) => Err(t!("error_fan_profile_write", error = e.to_string()).to_string()),
    }
}

#[zbus::proxy(
    interface = "org.supergfxctl.Daemon",
    default_service = "org.supergfxctl.Daemon",
    default_path = "/org/supergfxctl/Gfx"
)]
trait SuperGfx {
    async fn mode(&self) -> zbus::Result<u32>;
    async fn set_mode(&self, mode: u32) -> zbus::Result<u32>;
    async fn supported(&self) -> zbus::Result<Vec<u32>>;
}

/// GPU graphics mode exposed by the `supergfxctl` daemon.
///
/// Maps directly to the integer values used in the D-Bus `Mode`/`SetMode` methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GfxMode {
    /// PRIME/Optimus hybrid rendering - iGPU renders, dGPU handles compute (value `0`).
    Hybrid = 0,
    /// Only the integrated GPU is active; dGPU is powered off (value `1`).
    Integrated = 1,
    /// Nvidia driver loaded without kernel modesetting (value `2`).
    NvidiaNoModeset = 2,
    /// dGPU passed through to a virtual machine via VFIO (value `3`).
    Vfio = 3,
    /// Ayuz external GPU dock mode (value `4`).
    AsusEgpu = 4,
    /// Ayuz MUX switch set to discrete-only output (value `5`).
    AsusMuxDiscreet = 5,
}

impl From<u32> for GfxMode {
    fn from(v: u32) -> Self {
        match v {
            1 => Self::Integrated,
            2 => Self::NvidiaNoModeset,
            3 => Self::Vfio,
            4 => Self::AsusEgpu,
            5 => Self::AsusMuxDiscreet,
            _ => Self::Hybrid,
        }
    }
}

impl GfxMode {
    /// Returns the i18n key used to look up the localised display name for this mode.
    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::Hybrid => "gpu_mode_hybrid",
            Self::Integrated => "gpu_mode_integrated",
            Self::NvidiaNoModeset => "gpu_mode_nvidia_no_modeset",
            Self::Vfio => "gpu_mode_vfio",
            Self::AsusEgpu => "gpu_mode_asus_egpu",
            Self::AsusMuxDiscreet => "gpu_mode_asus_mux_discreet",
        }
    }
}

/// Lazily-initialized singleton proxy to the `org.supergfxctl.Daemon` D-Bus service.
static SUPERGFX_PROXY: tokio::sync::OnceCell<SuperGfxProxy<'static>> =
    tokio::sync::OnceCell::const_new();

/// Returns a reference to the shared [`SuperGfxProxy`], initialising it on first call.
async fn supergfx_proxy() -> Result<&'static SuperGfxProxy<'static>, String> {
    SUPERGFX_PROXY
        .get_or_try_init(|| async {
            let conn = system_bus_connection().await?;
            SuperGfxProxy::new(&conn)
                .await
                .map_err(|e| t!("error_dbus_proxy_create", error = e.to_string()).to_string())
        })
        .await
}

/// Returns `true` if the `supergfxctl` D-Bus service is reachable.
///
/// Opens a fresh system bus connection each time to avoid caching a stale result.
pub async fn check_supergfxctl_available() -> bool {
    let conn = match zbus::Connection::system().await {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Ok(proxy) = SuperGfxProxy::new(&conn).await {
        proxy.mode().await.is_ok()
    } else {
        false
    }
}

/// Reads the active GPU mode from `supergfxctl`.
pub async fn get_gpu_mode() -> Result<GfxMode, String> {
    let proxy = supergfx_proxy().await?;
    proxy
        .mode()
        .await
        .map(GfxMode::from)
        .map_err(|e| t!("error_gpu_mode_read", error = e.to_string()).to_string())
}

/// Returns the GPU modes that `supergfxctl` reports as available for switching.
///
/// Note: the daemon may omit the currently active mode from this list.
pub async fn get_supported_gpu_modes() -> Result<Vec<GfxMode>, String> {
    let proxy = supergfx_proxy().await?;
    proxy
        .supported()
        .await
        .map(|v| v.into_iter().map(GfxMode::from).collect())
        .map_err(|e| t!("error_gpu_mode_read", error = e.to_string()).to_string())
}

/// Requests a GPU mode switch via `supergfxctl` and returns the requested mode on success.
///
/// The daemon queues the change and returns the *currently active* mode (not the requested one),
/// because the switch only takes effect after a reboot or display-server restart.
/// We therefore ignore the return value and echo back `mode` to avoid confusing log output.
pub async fn set_gpu_mode(mode: GfxMode) -> Result<GfxMode, String> {
    let proxy = supergfx_proxy().await?;
    proxy
        .set_mode(mode as u32)
        .await
        .map(|_| mode)
        .map_err(|e| t!("error_gpu_mode_write", error = e.to_string()).to_string())
}

#[zbus::proxy(
    interface = "xyz.ljones.AsusArmoury",
    default_service = "xyz.ljones.Asusd",
    default_path = "/xyz/ljones/asus_armoury/apu_mem"
)]
trait AsusArmoury {
    #[zbus(property)]
    fn current_value(&self) -> zbus::Result<i32>;
    #[zbus(property)]
    fn set_current_value(&self, value: i32) -> zbus::Result<()>;
    #[zbus(property)]
    fn possible_values(&self) -> zbus::Result<Vec<i32>>;
}

/// Lazily-initialized singleton proxy to the `apu_mem` AsusArmoury D-Bus object.
static ASUS_ARMOURY_APU_MEM_PROXY: tokio::sync::OnceCell<AsusArmouryProxy<'static>> =
    tokio::sync::OnceCell::const_new();

/// Returns a reference to the shared [`AsusArmouryProxy`] for `apu_mem`, initialising it on first call.
async fn asus_armoury_apu_mem_proxy() -> Result<&'static AsusArmouryProxy<'static>, String> {
    ASUS_ARMOURY_APU_MEM_PROXY
        .get_or_try_init(|| async {
            let conn = system_bus_connection().await?;
            AsusArmouryProxy::new(&conn)
                .await
                .map_err(|e| t!("error_dbus_proxy_create", error = e.to_string()).to_string())
        })
        .await
}

/// Reads the current APU memory (UMA frame buffer) size from `asusd`.
pub async fn get_apu_mem() -> Result<i32, String> {
    let proxy = asus_armoury_apu_mem_proxy().await?;
    proxy
        .current_value()
        .await
        .map_err(|e| t!("error_apu_mem_read", error = e.to_string()).to_string())
}

/// Sets the APU memory (UMA frame buffer) size via `asusd` and returns the applied value.
pub async fn set_apu_mem(value: i32) -> Result<i32, String> {
    let proxy = asus_armoury_apu_mem_proxy().await?;
    proxy
        .set_current_value(value)
        .await
        .map_err(|e| t!("error_apu_mem_write", error = e.to_string()).to_string())?;
    Ok(value)
}

/// Returns the list of allowed APU memory values from `asusd` (e.g. `[0, 1, 2, 4, 8]`).
///
/// Returns an error if asusd is unreachable or if the laptop's BIOS does not expose this attribute.
pub async fn get_apu_mem_options() -> Result<Vec<i32>, String> {
    let proxy = asus_armoury_apu_mem_proxy().await?;
    proxy
        .possible_values()
        .await
        .map_err(|e| t!("error_apu_mem_read", error = e.to_string()).to_string())
}
