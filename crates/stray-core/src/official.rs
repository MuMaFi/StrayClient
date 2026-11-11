//! Play through the official Minecraft Launcher.
//!
//! No Microsoft app registration needed: Stray Client prepares Fabric, the mods, capes,
//! keystrokes and tuned JVM flags, writes a "Stray Client" profile into the official
//! launcher (the same way the Fabric installer does) and opens it. The player signs in
//! there, so Mojang itself verifies that they own Minecraft.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};

use crate::paths::Paths;
use crate::settings::Settings;
use crate::{cosmetics, fabric, launch, mods, mojang, Error, Progress, ProgressFn, Result};

pub const PROFILE_KEY: &str = "stray-client";
const ICON: &[u8] = include_bytes!("../../../src-tauri/icons/128x128.png");

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Is the official launcher installed?
    pub launcher_found: bool,
    /// Where the official launcher keeps its data (`.minecraft`).
    pub minecraft_dir: String,
}

/// `.minecraft` of the official launcher.
pub fn minecraft_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".minecraft")
    } else if cfg!(target_os = "macos") {
        dirs::home_dir()
            .unwrap_or_default()
            .join("Library/Application Support/minecraft")
    } else {
        dirs::home_dir().unwrap_or_default().join(".minecraft")
    }
}

/// How to start the official launcher on this machine, if it is installed.
enum Launcher {
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    StoreApp,
    Exe(PathBuf),
}

fn find_launcher() -> Option<Launcher> {
    if cfg!(target_os = "windows") {
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_default();
        if local
            .join("Packages")
            .join("Microsoft.4297127D64EC6_8wekyb3d8bbwe")
            .exists()
        {
            return Some(Launcher::StoreApp);
        }
        for base in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(pf) = std::env::var_os(base) {
                let exe = PathBuf::from(pf)
                    .join("Minecraft Launcher")
                    .join("MinecraftLauncher.exe");
                if exe.exists() {
                    return Some(Launcher::Exe(exe));
                }
            }
        }
        None
    } else if cfg!(target_os = "macos") {
        let home = dirs::home_dir().unwrap_or_default();
        [
            PathBuf::from("/Applications/Minecraft.app"),
            home.join("Applications/Minecraft.app"),
        ]
        .into_iter()
        .find(|p| p.exists())
        .map(Launcher::Exe)
    } else {
        [
            "/usr/bin/minecraft-launcher",
            "/usr/local/bin/minecraft-launcher",
            "/opt/minecraft-launcher/minecraft-launcher",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .map(Launcher::Exe)
    }
}

pub fn status() -> Status {
    Status {
        launcher_found: find_launcher().is_some(),
        minecraft_dir: minecraft_dir().to_string_lossy().into_owned(),
    }
}

fn open_launcher() -> Result<()> {
    let launcher = find_launcher().ok_or_else(|| Error::other("launcher_not_found"))?;
    let spawned = match launcher {
        Launcher::StoreApp => std::process::Command::new("explorer.exe")
            .arg(r"shell:AppsFolder\Microsoft.4297127D64EC6_8wekyb3d8bbwe!Minecraft")
            .spawn(),
        Launcher::Exe(path) if cfg!(target_os = "macos") => {
            std::process::Command::new("/usr/bin/open")
                .arg(path)
                .spawn()
        }
        Launcher::Exe(path) => std::process::Command::new(path).spawn(),
    };
    spawned
        .map(|_| ())
        .map_err(|e| Error::other(format!("launcher_start_failed:{e}")))
}

/// ISO-8601 UTC timestamp as the launcher writes it (`2026-11-09T13:05:00.000Z`).
fn iso_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.000Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Add or update the "Stray Client" profile in one launcher_profiles file.
fn write_profile(file: &Path, version_id: &str, game_dir: &Path, java_args: &str) -> Result<()> {
    let mut root: Value = match std::fs::read(file) {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|_| json!({})),
        Err(_) => json!({}),
    };
    if !root.is_object() {
        root = json!({});
    }
    let obj = root.as_object_mut().unwrap();
    obj.entry("version").or_insert(json!(3));
    obj.entry("settings").or_insert(json!({}));
    let profiles = obj.entry("profiles").or_insert(json!({}));
    if !profiles.is_object() {
        *profiles = json!({});
    }
    let now = iso_now();
    let created = profiles
        .get(PROFILE_KEY)
        .and_then(|p| p.get("created"))
        .cloned()
        .unwrap_or_else(|| json!(now));
    let icon = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(ICON)
    );
    profiles.as_object_mut().unwrap().insert(
        PROFILE_KEY.into(),
        json!({
            "name": "Stray Client",
            "type": "custom",
            "created": created,
            "lastUsed": now, // newest → shown first and pre-selected in the launcher
            "lastVersionId": version_id,
            "gameDir": game_dir.to_string_lossy(),
            "javaArgs": java_args,
            "icon": icon,
        }),
    );
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = file.with_extension("json.stray-tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&root)?)?;
    std::fs::rename(&tmp, file)?;
    Ok(())
}

/// Install the Fabric version + profile into `mc_dir` (testable without a real launcher).
pub async fn install_profile(
    paths: &Paths,
    mc_dir: &Path,
    game: &str,
    settings: &Settings,
    progress: &ProgressFn,
) -> Result<String> {
    progress(Progress {
        stage: "meta",
        done: 0,
        total: 1,
    });
    let loader = launch::loader_for(paths, game).await?;
    let profile = fabric::profile(paths, game, &loader).await?;
    // The Fabric profile is a normal launcher version that inherits from vanilla; the
    // official launcher downloads vanilla, libraries and assets itself.
    let version_id = profile.id.clone();
    let vdir = mc_dir.join("versions").join(&version_id);
    tokio::fs::create_dir_all(&vdir).await?;
    let fabric_json = tokio::fs::read(
        paths
            .versions()
            .join(&version_id)
            .join(format!("{version_id}.json")),
    )
    .await?;
    tokio::fs::write(vdir.join(format!("{version_id}.json")), &fabric_json).await?;
    // Older launchers expect a jar next to the json (the Fabric installer writes it empty too).
    let jar = vdir.join(format!("{version_id}.jar"));
    if !jar.exists() {
        tokio::fs::write(&jar, b"").await?;
    }

    // Mods, capes and HUD modules live in our own instance folder (used as gameDir).
    mods::ensure_core(paths, game, settings.performance_mods(), progress).await?;
    let _ = cosmetics::apply(paths, game, settings, progress).await;

    let java_major = mojang::version(paths, game)
        .await
        .ok()
        .and_then(|v| v.java_version.map(|j| j.major_version))
        .unwrap_or(21);
    let java_args = launch::jvm_flags(settings, java_major).join(" ");
    let game_dir = paths.instance(game);
    tokio::fs::create_dir_all(&game_dir).await?;

    let main = mc_dir.join("launcher_profiles.json");
    write_profile(&main, &version_id, &game_dir, &java_args)?;
    // The Microsoft Store edition keeps its own copy of the profiles.
    let store = mc_dir.join("launcher_profiles_microsoft_store.json");
    if store.exists() {
        write_profile(&store, &version_id, &game_dir, &java_args)?;
    }
    progress(Progress {
        stage: "meta",
        done: 1,
        total: 1,
    });
    Ok(version_id)
}

/// Prepare everything, write the profile and open the official launcher.
pub async fn play(
    paths: &Paths,
    game: &str,
    settings: &Settings,
    progress: &ProgressFn,
) -> Result<String> {
    if find_launcher().is_none() {
        return Err(Error::other("launcher_not_found"));
    }
    let id = install_profile(paths, &minecraft_dir(), game, settings, progress).await?;
    open_launcher()?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_format() {
        let t = iso_now();
        assert_eq!(t.len(), 24);
        assert!(t.starts_with("20") && t.ends_with(".000Z") && &t[10..11] == "T");
    }

    #[test]
    fn profile_is_added_without_touching_others() {
        let dir = std::env::temp_dir().join(format!("stray-off-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("launcher_profiles.json");
        std::fs::write(
            &file,
            r#"{"profiles":{"abc":{"name":"Vanilla","type":"latest-release"}},"settings":{"locale":"de-de"},"version":3}"#,
        )
        .unwrap();
        write_profile(
            &file,
            "fabric-loader-0.20.1-26.3",
            &dir.join("inst"),
            "-Xmx4096M",
        )
        .unwrap();
        write_profile(
            &file,
            "fabric-loader-0.20.2-26.3",
            &dir.join("inst"),
            "-Xmx6144M",
        )
        .unwrap();
        let v: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(v["profiles"]["abc"]["name"], "Vanilla");
        assert_eq!(v["settings"]["locale"], "de-de");
        let p = &v["profiles"][PROFILE_KEY];
        assert_eq!(p["name"], "Stray Client");
        assert_eq!(p["lastVersionId"], "fabric-loader-0.20.2-26.3");
        assert_eq!(p["javaArgs"], "-Xmx6144M");
        assert!(p["icon"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
