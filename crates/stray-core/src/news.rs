//! Playtime per profile (written by the Stray Client mod) and the news shown on the home screen.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::paths::Paths;
use crate::settings::Settings;
use crate::{http, Result};

const NEWS_URL: &str = "https://raw.githubusercontent.com/MuMaFi/StrayClient/main/CHANGELOG.json";
/// Shipped with the app: shown offline and before the first download.
const BUNDLED: &str = include_str!("../../../CHANGELOG.json");

async fn seconds_in(instance: &std::path::Path) -> u64 {
    let file = instance
        .join("config")
        .join("strayclient")
        .join("playtime.properties");
    let Ok(text) = tokio::fs::read_to_string(file).await else {
        return 0;
    };
    text.lines()
        .find_map(|l| l.trim().strip_prefix("seconds="))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// Seconds played per profile id ("" = the default profile, all its version folders).
pub async fn playtime(paths: &Paths, settings: &Settings) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let instances = paths.root.join("instances");
    let mut default = 0;
    if let Ok(mut dir) = tokio::fs::read_dir(&instances).await {
        while let Ok(Some(e)) = dir.next_entry().await {
            if !e.file_name().to_string_lossy().starts_with("profile-") {
                default += seconds_in(&e.path()).await;
            }
        }
    }
    out.insert(String::new(), default);
    for p in &settings.profiles {
        out.insert(p.id.clone(), seconds_in(&instances.join(p.dir())).await);
    }
    out
}

/// News entries (newest first): the list on GitHub, else the cached or bundled copy.
pub async fn news(paths: &Paths) -> Result<Value> {
    let cache = paths.cache().join("news.json");
    if let Ok(v) = http::get_json::<Value>(NEWS_URL).await {
        if v.is_array() {
            let _ = tokio::fs::create_dir_all(paths.cache()).await;
            let _ = tokio::fs::write(&cache, serde_json::to_vec(&v)?).await;
            return Ok(v);
        }
    }
    if let Ok(b) = tokio::fs::read(&cache).await {
        if let Ok(v) = serde_json::from_slice::<Value>(&b) {
            return Ok(v);
        }
    }
    Ok(serde_json::from_str(BUNDLED)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_news_is_valid() {
        let v: Value = serde_json::from_str(BUNDLED).unwrap();
        let first = &v.as_array().unwrap()[0];
        assert!(
            first["version"].is_string()
                && first["en"]["items"].is_array()
                && first["ru"]["title"].is_string()
        );
    }

    #[tokio::test]
    async fn playtime_per_profile() {
        let root = std::env::temp_dir().join(format!("stray-pt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::new(root.clone());
        for (dir, secs) in [("26.3", 100), ("1.21.11", 20), ("profile-pvp", 3600)] {
            let d = root.join("instances").join(dir).join("config/strayclient");
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(
                d.join("playtime.properties"),
                format!("#x\nseconds={secs}\n"),
            )
            .unwrap();
        }
        let s = Settings {
            profiles: vec![crate::settings::Profile {
                id: "pvp".into(),
                name: "PvP".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let t = playtime(&paths, &s).await;
        assert_eq!(t[""], 120);
        assert_eq!(t["pvp"], 3600);
        let _ = std::fs::remove_dir_all(root);
    }
}
