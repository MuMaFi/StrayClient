//! Fabric loader: fetch the launcher profile and merge it over the vanilla version.

use std::collections::HashSet;

use serde::Deserialize;

use crate::mojang::{Library, VersionJson};
use crate::paths::Paths;
use crate::{http, Result};

const META: &str = "https://meta.fabricmc.net/v2";
/// Error text when Fabric has no loader for a version (it then runs as vanilla).
pub const NO_FABRIC: &str = "no_fabric";

#[derive(Debug, Deserialize)]
struct LoaderEntry {
    loader: LoaderInfo,
}
#[derive(Debug, Deserialize)]
struct LoaderInfo {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
pub struct GameVersion {
    pub version: String,
    pub stable: bool,
}

pub async fn game_versions() -> Result<Vec<GameVersion>> {
    http::get_json(&format!("{META}/versions/game")).await
}

/// Newest stable loader for a game version.
pub async fn latest_loader(game: &str) -> Result<String> {
    let resp = http::client()
        .get(format!("{META}/versions/loader/{game}"))
        .send()
        .await?;
    // Fabric answers 400/404 for versions it doesn't know (very old ones, brand-new snapshots).
    if matches!(resp.status().as_u16(), 400 | 404) {
        return Err(crate::Error::other(format!("{NO_FABRIC} {game}")));
    }
    let list: Vec<LoaderEntry> = resp.error_for_status()?.json().await?;
    list.iter()
        .find(|e| e.loader.stable)
        .or(list.first())
        .map(|e| e.loader.version.clone())
        .ok_or_else(|| crate::Error::other(format!("{NO_FABRIC} {game}")))
}

/// Profile JSON for game+loader, cached on disk so we can launch offline.
pub async fn profile(paths: &Paths, game: &str, loader: &str) -> Result<VersionJson> {
    let id = format!("fabric-loader-{loader}-{game}");
    let file = paths.versions().join(&id).join(format!("{id}.json"));
    if let Ok(bytes) = tokio::fs::read(&file).await {
        if let Ok(v) = serde_json::from_slice(&bytes) {
            return Ok(v);
        }
    }
    let url = format!("{META}/versions/loader/{game}/{loader}/profile/json");
    let bytes = http::client()
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    tokio::fs::create_dir_all(file.parent().unwrap()).await?;
    tokio::fs::write(&file, &bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// `group:artifact[:classifier]` – the version is left out so the loader's newer ASM etc. win.
fn lib_key(name: &str) -> String {
    let p: Vec<&str> = name.split(':').collect();
    match p.as_slice() {
        [g, a, _v, c, ..] => format!("{g}:{a}:{c}"),
        [g, a, ..] => format!("{g}:{a}"),
        _ => name.to_string(),
    }
}

/// Fabric profile on top of vanilla: fabric libraries first, its main class and extra args.
pub fn merge(vanilla: VersionJson, fabric: VersionJson) -> VersionJson {
    // Fabric's libraries replace vanilla's of the same artifact. Vanilla entries are not
    // de-duplicated among themselves: older versions list one library twice with OS rules
    // (e.g. lwjgl-glfw 3.2.1 for macOS, 3.2.2 for the rest) and both must stay.
    let fabric_keys: HashSet<String> = fabric.libraries.iter().map(|l| lib_key(&l.name)).collect();
    let mut libraries: Vec<Library> = fabric.libraries;
    libraries.extend(
        vanilla
            .libraries
            .into_iter()
            .filter(|l| !fabric_keys.contains(&lib_key(&l.name))),
    );
    let mut arguments = vanilla.arguments;
    arguments.jvm.extend(fabric.arguments.jvm);
    arguments.game.extend(fabric.arguments.game);
    VersionJson {
        id: fabric.id,
        main_class: fabric.main_class.or(vanilla.main_class),
        inherits_from: Some(vanilla.id),
        arguments,
        libraries,
        asset_index: vanilla.asset_index,
        assets: vanilla.assets,
        downloads: vanilla.downloads,
        java_version: vanilla.java_version,
        kind: vanilla.kind,
        minecraft_arguments: vanilla.minecraft_arguments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib(name: &str) -> Library {
        serde_json::from_value(serde_json::json!({ "name": name })).unwrap()
    }

    #[test]
    fn merge_keeps_per_os_duplicates_and_lets_fabric_win() {
        let vanilla = VersionJson {
            id: "1.16.5".into(),
            libraries: vec![
                lib("org.lwjgl:lwjgl-glfw:3.2.1"),
                lib("org.lwjgl:lwjgl-glfw:3.2.2"),
                lib("org.ow2.asm:asm:7.0"),
            ],
            minecraft_arguments: None,
            ..Default::default()
        };
        let fabric = VersionJson {
            id: "fabric-loader-0.20.1-1.16.5".into(),
            libraries: vec![lib("org.ow2.asm:asm:9.8")],
            ..Default::default()
        };
        let m = merge(vanilla, fabric);
        let names: Vec<_> = m.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "org.ow2.asm:asm:9.8",
                "org.lwjgl:lwjgl-glfw:3.2.1",
                "org.lwjgl:lwjgl-glfw:3.2.2"
            ]
        );
        assert_eq!(m.vanilla_id(), "1.16.5");
    }
}
