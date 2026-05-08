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
pub(crate) async fn system_bus_connection() -> Result<zbus::Connection, String> {
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
        Err(e) if profile == FanProfile::Quiet && e.to_string().contains("NotSupported") => proxy
            .set_platform_profile(FanProfile::LowPower as u32)
            .await
            .map(|_| FanProfile::LowPower)
            .map_err(|e2| t!("error_fan_profile_write", error = e2.to_string()).to_string()),
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

// ── Aura RGB keyboard lighting ───────────────────────────────────────────────

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Type, Value};

/// `AuraDeviceType` discriminants exposed by `xyz.ljones.Aura.device_type`.
/// Matches the `AuraDeviceType` enum in `rog-aura/src/lib.rs`.
pub const AURA_DEVICE_TYPE_LAPTOP_2021: u32 = 0;
pub const AURA_DEVICE_TYPE_LAPTOP_PRE_2021: u32 = 1;
pub const AURA_DEVICE_TYPE_TUF: u32 = 2;
pub const AURA_DEVICE_TYPE_SCSI: u32 = 3;
pub const AURA_DEVICE_TYPE_ALLY: u32 = 4;
pub const AURA_DEVICE_TYPE_ANIME_OR_SLASH: u32 = 5;
pub const AURA_DEVICE_TYPE_UNKNOWN: u32 = 255;

/// Active lighting mode. Maps directly to the `AuraModeNum` discriminants in rog-aura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AuraModeNum {
    Static = 0,
    Breathe = 1,
    RainbowCycle = 2,
    RainbowWave = 3,
    Star = 4,
    Rain = 5,
    Highlight = 6,
    Laser = 7,
    Ripple = 8,
    Pulse = 10,
    Comet = 11,
    Flash = 12,
}

impl From<u32> for AuraModeNum {
    fn from(v: u32) -> Self {
        match v {
            1 => Self::Breathe,
            2 => Self::RainbowCycle,
            3 => Self::RainbowWave,
            4 => Self::Star,
            5 => Self::Rain,
            6 => Self::Highlight,
            7 => Self::Laser,
            8 => Self::Ripple,
            10 => Self::Pulse,
            11 => Self::Comet,
            12 => Self::Flash,
            _ => Self::Static,
        }
    }
}

impl AuraModeNum {
    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::Static => "aura_mode_static",
            Self::Breathe => "aura_mode_breathe",
            Self::RainbowCycle => "aura_mode_rainbow_cycle",
            Self::RainbowWave => "aura_mode_rainbow_wave",
            Self::Star => "aura_mode_star",
            Self::Rain => "aura_mode_rain",
            Self::Highlight => "aura_mode_highlight",
            Self::Laser => "aura_mode_laser",
            Self::Ripple => "aura_mode_ripple",
            Self::Pulse => "aura_mode_pulse",
            Self::Comet => "aura_mode_comet",
            Self::Flash => "aura_mode_flash",
        }
    }

    /// Rainbow modes have no user-configurable primary colour.
    pub fn is_colour_irrelevant(self) -> bool {
        matches!(self, Self::RainbowCycle | Self::RainbowWave)
    }

    /// Modes that use a configurable secondary colour. Mirrors `uses_colour2`
    /// behaviour from `rog-aura/src/builtin_modes.rs`.
    pub fn uses_colour2(self) -> bool {
        matches!(
            self,
            Self::Breathe
                | Self::Star
                | Self::Highlight
                | Self::Laser
                | Self::Ripple
                | Self::Pulse
                | Self::Comet
                | Self::Flash
        )
    }

    /// Modes that respect the `speed` setting (animated effects).
    pub fn uses_speed(self) -> bool {
        !matches!(self, Self::Static)
    }

    /// Only the rainbow-wave mode uses `direction`.
    pub fn uses_direction(self) -> bool {
        matches!(self, Self::RainbowWave)
    }
}

/// RGB colour triplet. D-Bus signature: `(yyy)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Value, OwnedValue, Serialize, Deserialize)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Colour {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
}

/// `AuraZone` discriminants exposed by `xyz.ljones.Aura.supported_basic_zones`.
/// Wire signature: `u`. Matches `rog-aura::AuraZone`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AuraZone {
    None = 0,
    Key1 = 1,
    Key2 = 2,
    Key3 = 3,
    Key4 = 4,
    Logo = 5,
    BarLeft = 6,
    BarRight = 7,
}

impl From<u32> for AuraZone {
    fn from(v: u32) -> Self {
        match v {
            1 => Self::Key1,
            2 => Self::Key2,
            3 => Self::Key3,
            4 => Self::Key4,
            5 => Self::Logo,
            6 => Self::BarLeft,
            7 => Self::BarRight,
            _ => Self::None,
        }
    }
}

impl AuraZone {
    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::None => "aura_zone_none",
            Self::Key1 => "aura_zone_key1",
            Self::Key2 => "aura_zone_key2",
            Self::Key3 => "aura_zone_key3",
            Self::Key4 => "aura_zone_key4",
            Self::Logo => "aura_zone_logo",
            Self::BarLeft => "aura_zone_bar_left",
            Self::BarRight => "aura_zone_bar_right",
        }
    }
}

/// `PowerZones` discriminants for `xyz.ljones.Aura.supported_power_zones` /
/// `AuraPowerState.zone`. Wire signature: `u`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Value, OwnedValue, Serialize, Deserialize)]
#[repr(u32)]
#[zvariant(signature = "u")]
pub enum PowerZones {
    Logo = 0,
    Keyboard = 1,
    Lightbar = 2,
    Lid = 3,
    RearGlow = 4,
    KeyboardAndLightbar = 5,
    Ally = 6,
    None = 255,
}

impl From<u32> for PowerZones {
    fn from(v: u32) -> Self {
        match v {
            0 => Self::Logo,
            1 => Self::Keyboard,
            2 => Self::Lightbar,
            3 => Self::Lid,
            4 => Self::RearGlow,
            5 => Self::KeyboardAndLightbar,
            6 => Self::Ally,
            _ => Self::None,
        }
    }
}

impl PowerZones {
    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::Logo => "aura_power_zone_logo",
            Self::Keyboard => "aura_power_zone_keyboard",
            Self::Lightbar => "aura_power_zone_lightbar",
            Self::Lid => "aura_power_zone_lid",
            Self::RearGlow => "aura_power_zone_rear_glow",
            Self::KeyboardAndLightbar => "aura_power_zone_keyboard_and_lightbar",
            Self::Ally => "aura_power_zone_ally",
            Self::None => "aura_power_zone_none",
        }
    }
}

/// Per-zone power-state struct. D-Bus signature follows `rog-aura::AuraPowerState`:
/// `(ubbbb)` (zone u32, then four bools).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Value, OwnedValue, Serialize, Deserialize)]
pub struct AuraPowerState {
    pub zone: PowerZones,
    pub boot: bool,
    pub awake: bool,
    pub sleep: bool,
    pub shutdown: bool,
}

/// Container struct sent to/from `xyz.ljones.Aura.led_power`. Wire signature:
/// `(a(ubbbb))` — a single struct holding a `Vec<AuraPowerState>`.
#[derive(Debug, Clone, PartialEq, Eq, Type, Value, OwnedValue, Serialize, Deserialize)]
pub struct LaptopAuraPower {
    pub states: Vec<AuraPowerState>,
}

/// Full Aura lighting effect sent to / read from `asusd`.
///
/// Uses `u32` for `mode`/`zone` and `String` for `speed`/`direction` to match
/// the rog-aura D-Bus wire format without requiring custom zvariant signature
/// attributes. Field order must exactly match the rog-aura `AuraEffect` struct.
#[derive(Debug, Clone, Type, Value, OwnedValue, Serialize, Deserialize)]
pub struct AuraEffect {
    /// `AuraModeNum` discriminant (e.g. 0 = Static, 1 = Breathe).
    pub mode: u32,
    /// `AuraZone` discriminant (0 = None / all zones).
    pub zone: u32,
    pub colour1: Colour,
    pub colour2: Colour,
    /// Animation speed: `"Low"`, `"Med"`, or `"High"`.
    pub speed: String,
    /// Animation direction: `"Right"`, `"Left"`, `"Up"`, or `"Down"`.
    pub direction: String,
}

#[zbus::proxy(
    interface = "xyz.ljones.Aura",
    default_service = "xyz.ljones.Asusd"
)]
trait Aura {
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn supported_basic_modes(&self) -> zbus::Result<Vec<u32>>;
    #[zbus(property)]
    fn supported_basic_zones(&self) -> zbus::Result<Vec<u32>>;
    #[zbus(property)]
    fn supported_power_zones(&self) -> zbus::Result<Vec<u32>>;
    #[zbus(property)]
    fn supported_brightness(&self) -> zbus::Result<Vec<u32>>;

    #[zbus(property)]
    fn brightness(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn set_brightness(&self, value: u32) -> zbus::Result<()>;

    #[zbus(property)]
    fn led_mode(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn set_led_mode(&self, value: u32) -> zbus::Result<()>;

    #[zbus(property)]
    fn led_mode_data(&self) -> zbus::Result<AuraEffect>;
    #[zbus(property)]
    fn set_led_mode_data(&self, value: AuraEffect) -> zbus::Result<()>;

    #[zbus(property)]
    fn led_power(&self) -> zbus::Result<LaptopAuraPower>;
    #[zbus(property)]
    fn set_led_power(&self, value: LaptopAuraPower) -> zbus::Result<()>;

    fn all_mode_data(&self) -> zbus::Result<BTreeMap<u32, AuraEffect>>;
}

/// Builds an `AuraProxy` for a specific object path. Each call constructs a
/// fresh proxy because Aura device paths are dynamic (e.g. `tuf`,
/// `<product_id>_<num>_<devpath>`) — caching a single proxy would lock us to
/// one device.
async fn aura_proxy_for(path: &str) -> Result<AuraProxy<'static>, String> {
    let conn = system_bus_connection().await?;
    let object_path = ObjectPath::try_from(path.to_string())
        .map_err(|e| t!("error_dbus_proxy_create", error = e.to_string()).to_string())?
        .into_owned();
    AuraProxy::builder(&conn)
        .path(object_path)
        .map_err(|e| t!("error_dbus_proxy_create", error = e.to_string()).to_string())?
        .build()
        .await
        .map_err(|e| t!("error_dbus_proxy_create", error = e.to_string()).to_string())
}

/// Coarse classification used by the UI to label each Aura device.
/// Derived from `device_type` plus the trailing path segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuraDeviceKind {
    /// USB-HID keyboard (LaptopKeyboard2021 / LaptopKeyboardPre2021).
    Keyboard,
    /// I2C-connected TUF keyboard (`/xyz/ljones/aura/tuf`). Single-zone.
    KeyboardTuf,
    /// AniMe matrix lid panel (`/xyz/ljones/aura/anime`).
    AniMe,
    /// ROG Slash strip (`/xyz/ljones/aura/slash`).
    Slash,
    /// External SCSI-attached LED device.
    ScsiExt,
    /// Asus Ally handheld.
    Ally,
    /// Anything else, including device_type == Unknown.
    Other,
}

impl AuraDeviceKind {
    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::Keyboard => "aura_device_keyboard",
            Self::KeyboardTuf => "aura_device_keyboard_tuf",
            Self::AniMe => "aura_device_anime",
            Self::Slash => "aura_device_slash",
            Self::ScsiExt => "aura_device_scsi",
            Self::Ally => "aura_device_ally",
            Self::Other => "aura_device_other",
        }
    }

    /// True for kinds that the per-profile config should drive.
    pub fn is_keyboard(self) -> bool {
        matches!(self, Self::Keyboard | Self::KeyboardTuf)
    }
}

fn classify_aura(device_type: u32, path: &str) -> AuraDeviceKind {
    let suffix = path.rsplit('/').next().unwrap_or("");
    match device_type {
        AURA_DEVICE_TYPE_TUF => AuraDeviceKind::KeyboardTuf,
        AURA_DEVICE_TYPE_LAPTOP_2021 | AURA_DEVICE_TYPE_LAPTOP_PRE_2021 => AuraDeviceKind::Keyboard,
        AURA_DEVICE_TYPE_SCSI => AuraDeviceKind::ScsiExt,
        AURA_DEVICE_TYPE_ALLY => AuraDeviceKind::Ally,
        AURA_DEVICE_TYPE_ANIME_OR_SLASH => match suffix {
            "anime" => AuraDeviceKind::AniMe,
            "slash" => AuraDeviceKind::Slash,
            _ => AuraDeviceKind::Other,
        },
        _ => match suffix {
            "tuf" => AuraDeviceKind::KeyboardTuf,
            "anime" => AuraDeviceKind::AniMe,
            "slash" => AuraDeviceKind::Slash,
            _ => AuraDeviceKind::Other,
        },
    }
}

/// Snapshot of one Aura device discovered on the bus.
#[derive(Debug, Clone)]
pub struct AuraDeviceInfo {
    pub path: String,
    pub kind: AuraDeviceKind,
}

/// Returns the daemon's running state. Re-uses [`PlatformProxy`] as a probe
/// so a missing daemon and a daemon-without-Aura can be distinguished.
pub async fn is_asusd_running() -> bool {
    let conn = match zbus::Connection::system().await {
        Ok(c) => c,
        Err(_) => return false,
    };
    matches!(
        PlatformProxy::new(&conn).await,
        Ok(p) if p.platform_profile().await.is_ok()
    )
}

/// Enumerates every `xyz.ljones.Aura` object exported by `asusd` via
/// `org.freedesktop.DBus.ObjectManager`. Returns an empty `Vec` when no
/// keyboard / lightbar / LED hardware is registered.
pub async fn discover_aura_devices() -> Result<Vec<AuraDeviceInfo>, String> {
    let conn = system_bus_connection().await?;
    let om = zbus::fdo::ObjectManagerProxy::builder(&conn)
        .destination("xyz.ljones.Asusd")
        .map_err(|e| t!("error_dbus_proxy_create", error = e.to_string()).to_string())?
        .path("/")
        .map_err(|e| t!("error_dbus_proxy_create", error = e.to_string()).to_string())?
        .build()
        .await
        .map_err(|e| t!("error_dbus_proxy_create", error = e.to_string()).to_string())?;

    let managed = om
        .get_managed_objects()
        .await
        .map_err(|e| t!("error_aura_read", error = e.to_string()).to_string())?;

    let mut out = Vec::new();
    for (object_path, ifaces) in managed {
        if !ifaces.contains_key("xyz.ljones.Aura") {
            continue;
        }
        let path: OwnedObjectPath = object_path;
        let path_str = path.as_str().to_string();
        if !path_str.starts_with("/xyz/ljones/aura/") {
            continue;
        }
        let device_type = match aura_proxy_for(&path_str).await {
            Ok(p) => p.device_type().await.unwrap_or(AURA_DEVICE_TYPE_UNKNOWN),
            Err(_) => AURA_DEVICE_TYPE_UNKNOWN,
        };
        let kind = classify_aura(device_type, &path_str);
        out.push(AuraDeviceInfo {
            path: path_str,
            kind,
        });
    }
    // Stable ordering: keyboards first, then lightbar/lid/anime/slash/other.
    out.sort_by_key(|d| match d.kind {
        AuraDeviceKind::Keyboard | AuraDeviceKind::KeyboardTuf => 0,
        AuraDeviceKind::AniMe => 1,
        AuraDeviceKind::Slash => 2,
        AuraDeviceKind::Ally => 3,
        AuraDeviceKind::ScsiExt => 4,
        AuraDeviceKind::Other => 5,
    });
    Ok(out)
}

// ── Per-device Aura service functions ───────────────────────────────────────

pub async fn get_aura_brightness(path: &str) -> Result<u32, String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .brightness()
        .await
        .map_err(|e| t!("error_aura_read", error = e.to_string()).to_string())
}

pub async fn set_aura_brightness(path: &str, value: u32) -> Result<u32, String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .set_brightness(value)
        .await
        .map_err(|e| t!("error_aura_write", error = e.to_string()).to_string())?;
    Ok(value)
}

pub async fn get_aura_effect(path: &str) -> Result<AuraEffect, String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .led_mode_data()
        .await
        .map_err(|e| t!("error_aura_read", error = e.to_string()).to_string())
}

pub async fn set_aura_effect(path: &str, effect: AuraEffect) -> Result<(), String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .set_led_mode_data(effect)
        .await
        .map_err(|e| t!("error_aura_write", error = e.to_string()).to_string())
}

pub async fn get_aura_supported_modes(path: &str) -> Result<Vec<u32>, String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .supported_basic_modes()
        .await
        .map_err(|e| t!("error_aura_read", error = e.to_string()).to_string())
}

pub async fn get_aura_supported_zones(path: &str) -> Result<Vec<u32>, String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .supported_basic_zones()
        .await
        .map_err(|e| t!("error_aura_read", error = e.to_string()).to_string())
}

pub async fn get_aura_supported_power_zones(path: &str) -> Result<Vec<u32>, String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .supported_power_zones()
        .await
        .map_err(|e| t!("error_aura_read", error = e.to_string()).to_string())
}

pub async fn get_aura_supported_brightness(path: &str) -> Result<Vec<u32>, String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .supported_brightness()
        .await
        .map_err(|e| t!("error_aura_read", error = e.to_string()).to_string())
}

pub async fn get_aura_all_mode_data(path: &str) -> Result<BTreeMap<u32, AuraEffect>, String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .all_mode_data()
        .await
        .map_err(|e| t!("error_aura_read", error = e.to_string()).to_string())
}

pub async fn get_aura_led_power(path: &str) -> Result<LaptopAuraPower, String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .led_power()
        .await
        .map_err(|e| t!("error_aura_read", error = e.to_string()).to_string())
}

pub async fn set_aura_led_power(path: &str, power: LaptopAuraPower) -> Result<(), String> {
    let proxy = aura_proxy_for(path).await?;
    proxy
        .set_led_power(power)
        .await
        .map_err(|e| t!("error_aura_write", error = e.to_string()).to_string())
}
