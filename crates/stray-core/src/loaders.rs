//! Forge and NeoForge for profiles that pick them. Both are installed by their official
//! installer (`--installClient` / `--install-client`) into the official launcher's folder,
//! which is what the "Stray Client" launcher profile then starts.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::paths::Paths;
use crate::{http, java, mojang, Error, Progress, ProgressFn, Result};

const NEOFORGE_VERSIONS: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";
const FORGE_PROMOS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";

/// The mod loader of a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loader {
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
}

impl Loader {
    /// From the settings value; empty / unknown = Fabric (profiles made before loaders existed).
    pub fn parse(s: &str) -> Self {
        match s {
            "vanilla" => Loader::Vanilla,
            "forge" => Loader::Forge,
            "neoforge" => Loader::NeoForge,
            _ => Loader::Fabric,
        }
    }
    /// Modrinth loader name (also the mod list's loader).
    pub fn modrinth(self) -> &'static str {
        match self {
            Loader::Vanilla | Loader::Fabric => "fabric",
            Loader::Forge => "forge",
            Loader::NeoForge => "neoforge",
        }
    }
    /// CurseForge `modLoaderType`.
    pub fn curseforge(self) -> u32 {
        match self {
            Loader::Forge => 1,
            Loader::NeoForge => 6,
            _ => 4,
        }
    }
}

#[derive(Deserialize)]
struct NeoVersions {
    versions: Vec<String>,
}

/// NeoForge numbers its versions after Minecraft: 26.3 → `26.3.x`, 1.21.1 → `21.1.x`.
pub fn neoforge_prefix(game: &str) -> String {
    let short = game.strip_prefix("1.").unwrap_or(game);
    // 1.21 → "21.0."
    if short.matches('.').count() == 0 {
        format!("{short}.0.")
    } else {
        format!("{short}.")
    }
}

/// Newest stable NeoForge for a game version (betas only if there is nothing else).
pub fn pick_neoforge(versions: &[String], game: &str) -> Option<String> {
    let prefix = neoforge_prefix(game);
    let matching: Vec<&String> = versions.iter().filter(|v| v.starts_with(&prefix)).collect();
    matching
        .iter()
        .rev()
        .find(|v| !v.contains("beta") && !v.contains("alpha"))
        .or(matching.last())
        .map(|v| v.to_string())
}

/// Newest Forge / NeoForge version for `game` (Forge: recommended, else latest).
pub async fn latest(loader: Loader, game: &str) -> Result<String> {
    match loader {
        Loader::NeoForge => {
            let v: NeoVersions = http::get_json(NEOFORGE_VERSIONS).await?;
            pick_neoforge(&v.versions, game)
                .ok_or_else(|| Error::other(format!("no_loader:NeoForge:{game}")))
        }
        Loader::Forge => {
            let v: serde_json::Value = http::get_json(FORGE_PROMOS).await?;
            let promos = &v["promos"];
            promos[format!("{game}-recommended")]
                .as_str()
                .or(promos[format!("{game}-latest")].as_str())
                .map(str::to_string)
                .ok_or_else(|| Error::other(format!("no_loader:Forge:{game}")))
        }
        _ => Err(Error::other("not a Forge loader")),
    }
}

/// Version id the installer creates.
fn version_id(loader: Loader, game: &str, ver: &str) -> String {
    match loader {
        Loader::Forge => format!("{game}-forge-{ver}"),
        _ => format!("neoforge-{ver}"),
    }
}

fn installer_url(loader: Loader, game: &str, ver: &str) -> String {
    match loader {
        Loader::Forge => format!("{FORGE_MAVEN}/{game}-{ver}/forge-{game}-{ver}-installer.jar"),
        _ => format!("{NEOFORGE_MAVEN}/{ver}/neoforge-{ver}-installer.jar"),
    }
}

/// Install Forge/NeoForge for `game` into `mc_dir` (the official launcher's folder) and
/// return the version id to start. Cheap when already installed; offline it reuses the last
/// installed version.
pub async fn install(
    paths: &Paths,
    loader: Loader,
    game: &str,
    mc_dir: &Path,
    progress: &ProgressFn,
) -> Result<String> {
    progress(Progress {
        stage: "loader",
        done: 0,
        total: 1,
    });
    let remember = paths
        .instance(game)
        .join(format!("{}.txt", loader.modrinth()));
    let ver = match latest(loader, game).await {
        Ok(v) => v,
        Err(e) if e.to_string().starts_with("no_loader") => return Err(e),
        Err(e) => tokio::fs::read_to_string(&remember)
            .await
            .map(|s| s.trim().to_string())
            .map_err(|_| e)?,
    };
    let id = version_id(loader, game, &ver);
    let json = mc_dir.join("versions").join(&id).join(format!("{id}.json"));
    if !json.exists() {
        let jar = download_installer(paths, loader, game, &ver).await?;
        // The installers refuse to run without a launcher profile file.
        tokio::fs::create_dir_all(mc_dir).await?;
        let profiles = mc_dir.join("launcher_profiles.json");
        if !profiles.exists() {
            tokio::fs::write(&profiles, br#"{"profiles":{}}"#).await?;
        }
        let component = mojang::version(paths, game)
            .await
            .ok()
            .and_then(|v| v.java_version.map(|j| j.component))
            .unwrap_or_else(|| "java-runtime-delta".into());
        let java = java::ensure(paths, &component, progress).await?;
        let flag = if loader == Loader::Forge {
            "--installClient"
        } else {
            "--install-client"
        };
        let log = paths
            .logs()
            .join(format!("{}-installer.log", loader.modrinth()));
        tokio::fs::create_dir_all(paths.logs()).await?;
        let out = tokio::process::Command::new(&java)
            .arg("-jar")
            .arg(&jar)
            .arg(flag)
            .arg(mc_dir)
            .current_dir(jar.parent().unwrap_or(mc_dir))
            .output()
            .await?;
        let mut text = out.stdout;
        text.extend_from_slice(&out.stderr);
        let _ = tokio::fs::write(&log, &text).await;
        if !out.status.success() || !json.exists() {
            return Err(Error::other(format!(
                "loader_install_failed:{}",
                log.display()
            )));
        }
    }
    tokio::fs::create_dir_all(paths.instance(game)).await?;
    let _ = tokio::fs::write(&remember, &ver).await;
    progress(Progress {
        stage: "loader",
        done: 1,
        total: 1,
    });
    Ok(id)
}

async fn download_installer(
    paths: &Paths,
    loader: Loader,
    game: &str,
    ver: &str,
) -> Result<PathBuf> {
    let dir = paths.cache().join("installers");
    tokio::fs::create_dir_all(&dir).await?;
    let file = dir.join(format!("{}-{game}-{ver}-installer.jar", loader.modrinth()));
    if !file.exists() {
        let bytes = http::client()
            .get(installer_url(loader, game, ver))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        tokio::fs::write(&file, &bytes).await?;
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_versions_follow_minecraft() {
        let v: Vec<String> = [
            "21.1.200",
            "21.1.209",
            "26.3.0.112",
            "26.3.0.113",
            "26.3.1.2-beta",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(pick_neoforge(&v, "26.3").as_deref(), Some("26.3.0.113"));
        assert_eq!(pick_neoforge(&v, "1.21.1").as_deref(), Some("21.1.209"));
        assert_eq!(neoforge_prefix("1.21"), "21.0.");
        assert!(pick_neoforge(&v, "1.20.1").is_none());
    }

    #[test]
    fn ids_and_urls() {
        assert_eq!(
            version_id(Loader::Forge, "26.3", "66.1.8"),
            "26.3-forge-66.1.8"
        );
        assert_eq!(
            version_id(Loader::NeoForge, "26.3", "26.3.0.113"),
            "neoforge-26.3.0.113"
        );
        assert!(installer_url(Loader::Forge, "26.3", "66.1.8")
            .ends_with("/26.3-66.1.8/forge-26.3-66.1.8-installer.jar"));
        assert!(installer_url(Loader::NeoForge, "26.3", "26.3.0.113")
            .ends_with("/26.3.0.113/neoforge-26.3.0.113-installer.jar"));
        assert_eq!(Loader::parse(""), Loader::Fabric);
        assert_eq!(Loader::parse("neoforge").curseforge(), 6);
    }
}
