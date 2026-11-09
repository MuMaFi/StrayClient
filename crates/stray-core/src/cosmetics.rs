//! Capes from PNG files and HUD modules (keystrokes), applied to a game instance.
//!
//! * Capes + StraySMP loading screen: our own `Stray Client` Fabric mod (mods/stray-client, MIT) is embedded in the
//!   launcher. The cape library lives in `<data>/capes`; the selected PNG is copied into
//!   the instance and the mod picks it up (even while the game is running).
//! * Keystrokes: the Modern Keystrokes mod from Modrinth (MIT), configured through its
//!   `config/modern-keystrokes.properties`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;

use crate::mods::{self, Source};
use crate::paths::Paths;
use crate::settings::Settings;
use crate::{Error, ProgressFn, Result};

/// The Stray Client mod (loading screen + capes), built from `mods/stray-client` (see its build.sh).
const CAPES_JAR: &[u8] = include_bytes!("../../../mods/stray-client/dist/stray-client.jar");
const CAPES_JAR_NAME: &str = "stray-client.jar";
/// File name used by older Stray Client versions (removed on update).
const LEGACY_JAR_NAME: &str = "stray-capes.jar";
pub const KEYSTROKES_SLUG: &str = "modern-keystrokes";

/// PvP Plus, made by the Stray Client team (mods/bundled, MIT). Built for Minecraft 26.3.
const PVP_PLUS_JAR: &[u8] = include_bytes!("../../../mods/bundled/pvp-plus-1.0.0.jar");
const PVP_PLUS_JAR_NAME: &str = "pvp-plus-1.0.0.jar";
const FABRIC_API_SLUG: &str = "fabric-api";

/// PvP Plus declares `minecraft: ~26.3` → 26.3 and its patch releases.
pub fn pvp_plus_supported(game: &str) -> bool {
    game == "26.3" || game.starts_with("26.3.")
}

/// Largest cape we accept (HD capes are 2:1, e.g. 2048×1024).
const MAX_CAPE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cape {
    pub file: String,
    pub width: u32,
    pub height: u32,
    /// `data:image/png;base64,…` for the preview in the UI.
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    /// Our cape mod works on the unobfuscated 26.x line.
    pub capes: bool,
    pub keystrokes: bool,
    pub pvp_plus: bool,
}

fn library(paths: &Paths) -> PathBuf {
    paths.root.join("capes")
}

/// `26.3` → true, `1.21.11` → false.
pub fn capes_supported(game: &str) -> bool {
    game.split('.')
        .next()
        .and_then(|m| m.parse::<u32>().ok())
        .is_some_and(|major| major >= 26)
}

pub async fn availability(game: &str) -> Availability {
    let keystrokes = mods::modrinth_game_versions(KEYSTROKES_SLUG)
        .await
        .map(|v| v.contains(game))
        .unwrap_or(false);
    Availability {
        capes: capes_supported(game),
        keystrokes,
        pvp_plus: pvp_plus_supported(game),
    }
}

/// Width and height from a PNG header, or None if it isn't a PNG.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != SIG || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

fn sanitize(name: &str) -> String {
    let stem = name
        .trim()
        .trim_end_matches(".png")
        .trim_end_matches(".PNG");
    let mut s: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    s.truncate(40);
    if s.trim_matches('_').is_empty() {
        s = "cape".into();
    }
    s
}

/// The built-in StraySMP cape (tools/gen_cape.py), added to an empty library on first use.
const DEFAULT_CAPE: &[u8] = include_bytes!("../../../ui/assets/demo-cape.png");

pub async fn list(paths: &Paths) -> Vec<Cape> {
    if !library(paths).exists() && tokio::fs::create_dir_all(library(paths)).await.is_ok() {
        let _ = tokio::fs::write(library(paths).join("straysmp.png"), DEFAULT_CAPE).await;
    }
    let mut out = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(library(paths)).await else {
        return out;
    };
    let mut files = Vec::new();
    while let Ok(Some(e)) = rd.next_entry().await {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.ends_with(".png") {
            files.push(name);
        }
    }
    files.sort();
    for file in files {
        if let Ok(bytes) = tokio::fs::read(library(paths).join(&file)).await {
            if let Some((width, height)) = png_size(&bytes) {
                let data_url = format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&bytes)
                );
                out.push(Cape {
                    file,
                    width,
                    height,
                    data_url,
                });
            }
        }
    }
    out
}

/// Validate and store a cape PNG; returns the stored file name.
pub async fn add(paths: &Paths, name: &str, bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_CAPE_BYTES {
        return Err(Error::other("cape_too_big"));
    }
    let (w, h) = png_size(bytes).ok_or_else(|| Error::other("cape_not_png"))?;
    // Capes are 2:1 (64×32 classic, up to 2048×1024 HD). 22×17 is the old OptiFine layout.
    let ok = (w == 2 * h && (32..=4096).contains(&w)) || (w == 22 && h == 17);
    if !ok {
        return Err(Error::other(format!("cape_bad_size:{w}x{h}")));
    }
    let dir = library(paths);
    tokio::fs::create_dir_all(&dir).await?;
    let base = sanitize(name);
    let mut file = format!("{base}.png");
    let mut n = 2;
    while dir.join(&file).exists() {
        file = format!("{base}-{n}.png");
        n += 1;
    }
    tokio::fs::write(dir.join(&file), bytes).await?;
    Ok(file)
}

pub async fn remove(paths: &Paths, file: &str) -> Result<()> {
    let p = library(paths).join(file);
    // Only plain file names inside the library.
    if file.contains('/') || file.contains('\\') || file.contains("..") {
        return Err(Error::other("bad file name"));
    }
    let _ = tokio::fs::remove_file(p).await;
    Ok(())
}

async fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<()> {
    if tokio::fs::read(path).await.ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, bytes).await?;
    Ok(())
}

/// Bring the instance in line with the settings: cape mod + files, keystrokes mod + config.
pub async fn apply(
    paths: &Paths,
    game: &str,
    settings: &Settings,
    progress: &ProgressFn,
) -> Result<()> {
    let instance = paths.instance(game);

    // ── Stray Client mod: StraySMP loading screen + capes ──
    let jar = instance.join("mods").join(CAPES_JAR_NAME);
    let _ = tokio::fs::remove_file(instance.join("mods").join(LEGACY_JAR_NAME)).await;
    if capes_supported(game) {
        let mode = match settings.cape_mode.as_str() {
            "local" | "none" => settings.cape_mode.as_str(),
            _ => "account",
        };
        let cfg_dir = instance.join("config").join("stray-capes");
        let mut cape = String::new();
        if mode == "local" && !settings.cape.is_empty() {
            let src = library(paths).join(&settings.cape);
            if let Ok(bytes) = tokio::fs::read(&src).await {
                write_if_changed(&cfg_dir.join("capes").join(&settings.cape), &bytes).await?;
                cape = settings.cape.clone();
            }
        }
        let mode = if mode == "local" && cape.is_empty() {
            "account"
        } else {
            mode
        };
        let cfg = format!("# Managed by Stray Client\nmode={mode}\ncape={cape}\n");
        write_if_changed(&cfg_dir.join("config.properties"), cfg.as_bytes()).await?;
        // Always installed on 26.x: it also brings the StraySMP loading screen.
        write_if_changed(&jar, CAPES_JAR).await?;
    } else {
        let _ = tokio::fs::remove_file(&jar).await;
    }

    // ── PvP Plus ──
    let pvp_jar = instance.join("mods").join(PVP_PLUS_JAR_NAME);
    if settings.pvp_plus && pvp_plus_supported(game) {
        // It depends on Fabric API.
        if !mods::installed(paths, game)
            .await
            .iter()
            .any(|m| m.slug == FABRIC_API_SLUG)
        {
            mods::install(paths, Source::Modrinth, FABRIC_API_SLUG, game, "", progress).await?;
        }
        write_if_changed(&pvp_jar, PVP_PLUS_JAR).await?;
    } else {
        let _ = tokio::fs::remove_file(&pvp_jar).await;
    }

    // ── Keystrokes ──
    let ks = &settings.keystrokes;
    let installed = mods::installed(paths, game).await;
    let existing = installed.iter().find(|m| m.slug == KEYSTROKES_SLUG);
    if ks.enabled {
        if existing.is_none() {
            let available = mods::modrinth_game_versions(KEYSTROKES_SLUG)
                .await
                .map(|v| v.contains(game))
                .unwrap_or(false);
            if !available {
                return Err(Error::other(format!("keystrokes_unavailable:{game}")));
            }
            mods::install(paths, Source::Modrinth, KEYSTROKES_SLUG, game, "", progress).await?;
        } else if let Some(m) = existing.filter(|m| !m.enabled) {
            mods::set_enabled(paths, game, &m.project_id, true).await?;
        }
    } else if let Some(m) = existing.filter(|m| m.enabled) {
        mods::set_enabled(paths, game, &m.project_id, false).await?;
    }
    if ks.enabled || existing.is_some() {
        write_keystrokes_config(
            &instance.join("config").join("modern-keystrokes.properties"),
            settings,
            settings.pvp_plus && pvp_plus_supported(game),
        )
        .await?;
    }
    Ok(())
}

/// Changes made in the in-game menu (Right Shift), written by the Stray Client mod to
/// `config/strayclient/ingame.properties` in each instance. They are taken over into the
/// launcher settings (so the next launch keeps them), mods are switched, and the file is removed.
/// Returns true when the settings changed.
pub async fn take_ingame_changes(paths: &Paths, settings: &mut Settings) -> bool {
    let mut changed = false;
    let Ok(mut dirs) = tokio::fs::read_dir(paths.root.join("instances")).await else {
        return false;
    };
    while let Ok(Some(entry)) = dirs.next_entry().await {
        let instance = entry.path();
        let file = instance
            .join("config")
            .join("strayclient")
            .join("ingame.properties");
        let Ok(text) = tokio::fs::read_to_string(&file).await else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim().replace("\\", ""), v.trim());
            match k.as_str() {
                "keystrokes" => {
                    let on = v == "true";
                    changed |= settings.keystrokes.enabled != on;
                    settings.keystrokes.enabled = on;
                }
                "cape" if v == "local" || v == "account" || v == "none" => {
                    changed |= settings.cape_mode != v;
                    settings.cape_mode = v.to_string();
                }
                _ => {
                    if let Some(jar) = k.strip_prefix("mod.") {
                        let _ = mods::set_enabled_file(&instance, jar, v == "on").await;
                    }
                }
            }
        }
        let _ = tokio::fs::remove_file(&file).await;
    }
    changed
}

/// Default Keystrokes position (GUI pixels, top-left anchored).
const KS_Y_DEFAULT: &str = "10";
/// Below the PvP Plus info HUD, which is fixed at the top left (≈ 70 px tall).
const KS_Y_BELOW_PVP: &str = "80";

/// Java properties file; keeps the HUD position/scale the player set in game (`/keystrokes`).
/// With PvP Plus active the HUD moves below the PvP Plus HUD (unless the player placed it
/// themselves) and FPS/ping are left to PvP Plus, which already shows them.
async fn write_keystrokes_config(path: &Path, settings: &Settings, pvp_plus: bool) -> Result<()> {
    let mut props: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(text) = tokio::fs::read_to_string(path).await {
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with('!') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                props.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    props.entry("x".into()).or_insert_with(|| "25".into());
    let y = props
        .entry("y".into())
        .or_insert_with(|| KS_Y_DEFAULT.into());
    if *y == KS_Y_DEFAULT || *y == KS_Y_BELOW_PVP {
        *y = if pvp_plus {
            KS_Y_BELOW_PVP
        } else {
            KS_Y_DEFAULT
        }
        .into();
    }
    props.entry("scale".into()).or_insert_with(|| "1.0".into());
    let ks = &settings.keystrokes;
    for (k, v) in [
        ("isEnabled", ks.enabled),
        ("showKeys", ks.keys),
        ("showCPS", ks.mouse),
        ("showSpace", ks.space),
        ("showFPS", ks.fps && !pvp_plus),
        ("showPing", ks.ping && !pvp_plus),
    ] {
        props.insert(k.into(), v.to_string());
    }
    let mut out = String::from("#Modern Keystrokes Configuration (managed by Stray Client)\n");
    for (k, v) in props {
        out.push_str(&format!("{k}={v}\n"));
    }
    write_if_changed(path, out.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        b.extend(w.to_be_bytes());
        b.extend(h.to_be_bytes());
        b.extend([8, 6, 0, 0, 0]);
        b
    }

    #[test]
    fn versions() {
        assert!(capes_supported("26.3"));
        assert!(capes_supported("26.1.2"));
        assert!(!capes_supported("1.21.11"));
    }

    #[tokio::test]
    async fn default_cape_on_first_use() {
        let root =
            std::env::temp_dir().join(format!("stray-cos-{}", uuid::Uuid::new_v4().simple()));
        let caps = list(&Paths::new(root.clone())).await;
        assert_eq!(caps.len(), 1);
        assert_eq!(
            (caps[0].file.as_str(), caps[0].width, caps[0].height),
            ("straysmp.png", 64, 32)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pvp_plus_versions_and_jar() {
        assert!(pvp_plus_supported("26.3") && pvp_plus_supported("26.3.1"));
        assert!(!pvp_plus_supported("26.2") && !pvp_plus_supported("1.21.11"));
        assert_eq!(&PVP_PLUS_JAR[..2], b"PK");
    }

    #[test]
    fn embedded_mod_is_a_jar() {
        assert_eq!(&CAPES_JAR[..2], b"PK");
    }

    #[tokio::test]
    async fn add_list_apply() {
        let root =
            std::env::temp_dir().join(format!("stray-cos-{}", uuid::Uuid::new_v4().simple()));
        let paths = Paths::new(root.clone());
        assert!(add(&paths, "x.png", b"nope").await.is_err());
        assert!(add(&paths, "bad", &png(64, 64)).await.is_err());
        let f = add(&paths, "My Cape!.png", &png(64, 32)).await.unwrap();
        assert_eq!(f, "my_cape_.png");
        assert_eq!(
            add(&paths, "My Cape!.png", &png(128, 64)).await.unwrap(),
            "my_cape_-2.png"
        );
        let caps = list(&paths).await;
        assert_eq!(caps.len(), 2); // created before list() → no default cape added
        let first = caps.iter().find(|c| c.file == f).unwrap();
        assert_eq!((first.width, first.height), (64, 32));
        assert!(first.data_url.starts_with("data:image/png;base64,"));

        let mut s = Settings {
            cape_mode: "local".into(),
            cape: f.clone(),
            ..Default::default()
        };
        let noop: ProgressFn = std::sync::Arc::new(|_| {});
        apply(&paths, "26.3", &s, &noop).await.unwrap();
        let inst = paths.instance("26.3");
        assert!(inst.join("mods/stray-client.jar").exists());
        assert!(inst.join("config/stray-capes/capes").join(&f).exists());
        let cfg =
            std::fs::read_to_string(inst.join("config/stray-capes/config.properties")).unwrap();
        assert!(cfg.contains("mode=local") && cfg.contains(&format!("cape={f}")));

        s.cape_mode = "account".into();
        apply(&paths, "26.3", &s, &noop).await.unwrap();
        // the mod stays (loading screen); the cape config switches back to the account cape
        assert!(inst.join("mods/stray-client.jar").exists());
        let cfg =
            std::fs::read_to_string(inst.join("config/stray-capes/config.properties")).unwrap();
        assert!(cfg.contains("mode=account"));

        // keystrokes config keeps the in-game position
        let ks = inst.join("config/modern-keystrokes.properties");
        std::fs::write(&ks, "x=300\ny=40\nscale=0.8\n").unwrap();
        s.keystrokes = crate::settings::Keystrokes {
            enabled: false,
            keys: true,
            mouse: false,
            ..Default::default()
        };
        write_keystrokes_config(&ks, &s, false).await.unwrap();
        let text = std::fs::read_to_string(&ks).unwrap();
        assert!(text.contains("x=300") && text.contains("scale=0.8"));
        assert!(
            text.contains("showKeys=true")
                && text.contains("showCPS=false")
                && text.contains("isEnabled=false")
        );
        // a position the player chose survives PvP Plus
        write_keystrokes_config(&ks, &s, true).await.unwrap();
        assert!(std::fs::read_to_string(&ks).unwrap().contains("y=40"));

        // default position moves below the PvP Plus HUD and back; FPS/ping go to PvP Plus
        std::fs::write(&ks, "x=25\ny=10\n").unwrap();
        s.keystrokes.fps = true;
        s.keystrokes.ping = true;
        write_keystrokes_config(&ks, &s, true).await.unwrap();
        let text = std::fs::read_to_string(&ks).unwrap();
        assert!(
            text.contains("y=80")
                && text.contains("showFPS=false")
                && text.contains("showPing=false")
        );
        write_keystrokes_config(&ks, &s, false).await.unwrap();
        let text = std::fs::read_to_string(&ks).unwrap();
        assert!(text.contains("y=10") && text.contains("showFPS=true"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn ingame_menu_changes_are_taken_over() {
        let root = std::env::temp_dir().join(format!("stray-ingame-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = Paths::new(root.clone());
        let inst = paths.instance("26.3");
        std::fs::create_dir_all(inst.join("mods")).unwrap();
        std::fs::create_dir_all(inst.join("config/strayclient")).unwrap();
        std::fs::write(inst.join("mods/lithium.jar"), b"PK").unwrap();
        std::fs::write(
            inst.join("stray-mods.json"),
            r#"{"mods":[{"source":"modrinth","projectId":"x","slug":"lithium","title":"Lithium","version":"1","fileName":"lithium.jar","iconUrl":null,"enabled":true,"locked":false}]}"#,
        )
        .unwrap();
        std::fs::write(
            inst.join("config/strayclient/ingame.properties"),
            "#Changes\nkeystrokes=true\ncape=local\nmod.lithium.jar=off\n",
        )
        .unwrap();
        let mut s = Settings::default();
        assert!(take_ingame_changes(&paths, &mut s).await);
        assert!(s.keystrokes.enabled);
        assert_eq!(s.cape_mode, "local");
        assert!(inst.join("mods/lithium.jar.disabled").exists());
        assert!(!inst.join("config/strayclient/ingame.properties").exists());
        let list = mods::installed(&paths, "26.3").await;
        assert!(!list[0].enabled);
        // nothing pending → nothing changes
        assert!(!take_ingame_changes(&paths, &mut s).await);
        let _ = std::fs::remove_dir_all(root);
    }
}
