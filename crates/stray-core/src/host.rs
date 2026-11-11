//! "Host": share a singleplayer world with friends anywhere, without a server or port
//! forwarding. The launcher lists the worlds of the active profile, makes sure the e4mc mod
//! (MIT, public relay at e4mc.link) is installed, and leaves a request for the Stray Client
//! mod, which opens the world on start and publishes it. e4mc then prints the public address.

use std::path::Path;
use std::time::UNIX_EPOCH;

use base64::Engine;
use serde::Serialize;

use crate::mods::{self, Source};
use crate::paths::Paths;
use crate::{Error, ProgressFn, Result};

/// Modrinth slug of the mod that gives a LAN world a public address.
pub const HOST_SLUG: &str = "e4mc";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct World {
    /// Folder name in `saves/` (what Minecraft opens).
    pub id: String,
    /// Seconds since the epoch of the last save.
    pub last_played: u64,
    /// `data:image/png;base64,…` of the world icon, if it has one.
    pub icon: Option<String>,
}

fn valid_world(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\']) && !id.contains("..")
}

/// Worlds of the active profile, most recently played first.
pub async fn worlds(paths: &Paths, game: &str) -> Vec<World> {
    let saves = paths.instance(game).join("saves");
    let mut out = Vec::new();
    let Ok(mut dir) = tokio::fs::read_dir(&saves).await else {
        return out;
    };
    while let Ok(Some(entry)) = dir.next_entry().await {
        let path = entry.path();
        let level = path.join("level.dat");
        let Ok(meta) = tokio::fs::metadata(&level).await else {
            continue; // not a world
        };
        let last_played = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let icon = match tokio::fs::read(path.join("icon.png")).await {
            Ok(bytes) if bytes.len() < 512 * 1024 => Some(format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )),
            _ => None,
        };
        out.push(World {
            id: entry.file_name().to_string_lossy().into_owned(),
            last_played,
            icon,
        });
    }
    out.sort_by_key(|w| std::cmp::Reverse(w.last_played));
    out
}

/// Whether hosting works for this game version (e4mc has a build for it).
pub async fn available(game: &str) -> bool {
    mods::modrinth_game_versions(HOST_SLUG)
        .await
        .map(|v| v.iter().any(|g| g == game))
        .unwrap_or(false)
}

/// Prepare the next start to host `world`: install/enable e4mc and leave the request for
/// the Stray Client mod (`config/strayclient/host.properties`, removed once it is read).
pub async fn request(
    paths: &Paths,
    game: &str,
    world: &str,
    name: &str,
    progress: &ProgressFn,
) -> Result<()> {
    let instance = paths.instance(game);
    if !valid_world(world)
        || !instance
            .join("saves")
            .join(world)
            .join("level.dat")
            .exists()
    {
        return Err(Error::other("host_no_world"));
    }
    let installed = mods::installed(paths, game).await;
    match installed.iter().find(|m| m.slug == HOST_SLUG) {
        None => {
            mods::install(paths, Source::Modrinth, HOST_SLUG, game, "", progress)
                .await
                .map_err(|_| Error::other(format!("host_unavailable:{game}")))?;
        }
        Some(m) if !m.enabled => mods::set_enabled(paths, game, &m.project_id, true).await?,
        Some(_) => {}
    }
    write_request(&instance, world, name).await
}

async fn write_request(instance: &Path, world: &str, name: &str) -> Result<()> {
    let dir = instance.join("config").join("strayclient");
    tokio::fs::create_dir_all(&dir).await?;
    // Java .properties: escape backslashes and keep everything on one line.
    let esc = |s: &str| {
        s.chars()
            .filter(|c| !c.is_control())
            .flat_map(|c| match c {
                '\\' => vec!['\\', '\\'],
                c if c.is_ascii() => vec![c],
                c => format!("\\u{:04x}", c as u32).chars().collect(),
            })
            .collect::<String>()
    };
    let name: String = name.trim().chars().take(48).collect();
    let text = format!(
        "#Host request (Stray Client launcher)\nworld={}\nname={}\n",
        esc(world),
        esc(&name)
    );
    tokio::fs::write(dir.join("host.properties"), text).await?;
    Ok(())
}

/// Drop a request that was not used (e.g. the launch failed).
pub async fn cancel(paths: &Paths, game: &str) {
    let file = paths
        .instance(game)
        .join("config")
        .join("strayclient")
        .join("host.properties");
    let _ = tokio::fs::remove_file(file).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lists_worlds_and_writes_the_request() {
        let root = std::env::temp_dir().join(format!("stray-host-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::new(root.clone());
        let inst = paths.instance("26.3");
        for w in ["Survival", "Мой мир"] {
            std::fs::create_dir_all(inst.join("saves").join(w)).unwrap();
            std::fs::write(inst.join("saves").join(w).join("level.dat"), b"x").unwrap();
        }
        std::fs::create_dir_all(inst.join("saves/not-a-world")).unwrap();
        std::fs::write(inst.join("saves/Survival/icon.png"), b"\x89PNG").unwrap();

        let list = worlds(&paths, "26.3").await;
        assert_eq!(list.len(), 2);
        let s = list.iter().find(|w| w.id == "Survival").unwrap();
        assert!(s
            .icon
            .as_deref()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(list.iter().any(|w| w.id == "Мой мир" && w.icon.is_none()));

        write_request(&inst, "Мой мир", "Stray's \\ world\n2")
            .await
            .unwrap();
        let text =
            std::fs::read_to_string(inst.join("config/strayclient/host.properties")).unwrap();
        assert!(text.contains("world=\\u041c\\u043e\\u0439 \\u043c\\u0438\\u0440\n"));
        assert!(text.contains("name=Stray's \\\\ world2\n"));

        let noop: ProgressFn = std::sync::Arc::new(|_| {});
        for bad in ["", "../x", "nope"] {
            assert!(request(&paths, "26.3", bad, "n", &noop).await.is_err());
        }
        cancel(&paths, "26.3").await;
        assert!(!inst.join("config/strayclient/host.properties").exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
