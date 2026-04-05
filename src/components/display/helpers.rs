use crate::services::commands::run_command_blocking;
use crate::services::config::AppConfig;
use rust_i18n::t;

pub(crate) const DISPLAY_NAME: &str = "eDP-1";

const SRGB_ICM: &[u8] = include_bytes!("../../../assets/icm/ASUS_sRGB.icm");
const DCIP3_ICM: &[u8] = include_bytes!("../../../assets/icm/ASUS_DCIP3.icm");
const DISPLAYP3_ICM: &[u8] = include_bytes!("../../../assets/icm/ASUS_DisplayP3.icm");

pub(crate) async fn setup_icm_profiles() -> Result<std::path::PathBuf, String> {
    let base = AppConfig::config_dir()
        .ok_or_else(|| t!("error_config_dir").to_string())?
        .join("icm");

    let base_clone = base.clone();
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&base_clone)
            .map_err(|e| t!("error_icm_dir_create", error = e.to_string()).to_string())?;

        for (name, data) in [
            ("ASUS_sRGB.icm", SRGB_ICM),
            ("ASUS_DCIP3.icm", DCIP3_ICM),
            ("ASUS_DisplayP3.icm", DISPLAYP3_ICM),
        ] {
            let path = base_clone.join(name);
            if !path.exists() {
                std::fs::write(&path, data).map_err(|e| {
                    t!("error_icm_write", name = name, error = e.to_string()).to_string()
                })?;
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| t!("error_spawn_blocking", error = e.to_string()).to_string())??;

    Ok(base)
}

pub(crate) async fn reset_icm_profile() -> Result<(), String> {
    let arg = format!("output.{}.colorProfileSource.EDID", DISPLAY_NAME);
    run_command_blocking("kscreen-doctor", &[&arg]).await
}

pub(crate) async fn apply_icm_profile(
    filename: &str,
    base_path: &std::path::Path,
) -> Result<(), String> {
    let arg = format!(
        "output.{}.iccprofile.{}",
        DISPLAY_NAME,
        base_path.join(filename).display()
    );
    run_command_blocking("kscreen-doctor", &[&arg]).await
}

/// Fallback: tries qdbus-qt6, then qdbus.
pub(crate) async fn run_qdbus(args: Vec<String>) -> Result<(), String> {
    let result = tokio::task::spawn_blocking(move || {
        let status = std::process::Command::new("qdbus-qt6").args(&args).status();
        match status {
            Ok(s) => Ok(("qdbus-qt6", s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::process::Command::new("qdbus")
                    .args(&args)
                    .status()
                    .map(|s| ("qdbus", s))
            }
            Err(e) => Err(e),
        }
    })
    .await;

    match result {
        Ok(Ok((_, status))) if status.success() => Ok(()),
        Ok(Ok((cmd, status))) => Err(t!(
            "error_cmd_exit_code",
            cmd = cmd,
            code = status.code().unwrap_or(-1).to_string()
        )
        .to_string()),
        Ok(Err(e)) => Err(t!("error_qdbus_start", error = e.to_string()).to_string()),
        Err(e) => Err(t!("error_spawn_blocking", error = e.to_string()).to_string()),
    }
}
