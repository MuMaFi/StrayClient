//! Fabric loader: fetch the launcher profile and merge it over the vanilla version.

use std::collections::HashSet;

use serde::Deserialize;

use crate::mojang::{Library, VersionJson};
use crate::paths::Paths;
use crate::{http, Result};

const META: &str = "https://meta.fabricmc.net/v2";

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
    let list: Vec<LoaderEntry> = http::get_json(&format!("{META}/versions/loader/{game}")).await?;
    list.iter()
        .find(|e| e.loader.stable)
        .or(list.first())
        .map(|e| e.loader.version.clone())
        .ok_or_else(|| crate::Error::other(format!("Fabric does not support {game} yet")))
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
    let mut seen: HashSet<String> = HashSet::new();
    let mut libraries: Vec<Library> = Vec::new();
    for lib in fabric.libraries.into_iter().chain(vanilla.libraries) {
        if seen.insert(lib_key(&lib.name)) {
            libraries.push(lib);
        }
    }
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
    }
}
