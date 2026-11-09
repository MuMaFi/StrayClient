//! Skins: a local library of PNG skins plus the official Minecraft skin API, so a skin
//! applied here is what every player sees on every server.

use std::collections::BTreeMap;
use std::path::PathBuf;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::auth;
use crate::cosmetics::png_size;
use crate::paths::Paths;
use crate::{http, Error, Result};

const PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
/// Mojang only accepts these sizes (64×32 is the pre-1.8 layout).
const MAX_SKIN_BYTES: usize = 1024 * 1024;

/// The built-in StraySMP skin (tools/gen_skin.py), added to an empty library on first use.
const DEFAULT_SKIN: &[u8] = include_bytes!("../../../ui/assets/demo-skin.png");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    /// Steve-style arms (4 px).
    #[default]
    Classic,
    /// Alex-style arms (3 px).
    Slim,
}

impl Variant {
    fn api(self) -> &'static str {
        match self {
            Variant::Classic => "classic",
            Variant::Slim => "slim",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    pub file: String,
    pub variant: Variant,
    pub width: u32,
    pub height: u32,
    pub data_url: String,
}

/// The skin currently on the Microsoft account.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentSkin {
    pub variant: Variant,
    pub data_url: Option<String>,
}

fn library(paths: &Paths) -> PathBuf {
    paths.root.join("skins")
}
fn index_file(paths: &Paths) -> PathBuf {
    library(paths).join("skins.json")
}

async fn load_index(paths: &Paths) -> BTreeMap<String, Variant> {
    match tokio::fs::read(index_file(paths)).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => BTreeMap::new(),
    }
}
async fn save_index(paths: &Paths, idx: &BTreeMap<String, Variant>) -> Result<()> {
    tokio::fs::create_dir_all(library(paths)).await?;
    tokio::fs::write(index_file(paths), serde_json::to_vec_pretty(idx)?).await?;
    Ok(())
}

fn data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn validate(bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.len() > MAX_SKIN_BYTES {
        return Err(Error::other("skin_too_big"));
    }
    let (w, h) = png_size(bytes).ok_or_else(|| Error::other("skin_not_png"))?;
    if (w, h) != (64, 64) && (w, h) != (64, 32) {
        return Err(Error::other(format!("skin_bad_size:{w}x{h}")));
    }
    Ok((w, h))
}

fn safe_name(file: &str) -> Result<()> {
    if file.contains('/') || file.contains('\\') || file.contains("..") || !file.ends_with(".png") {
        return Err(Error::other("bad file name"));
    }
    Ok(())
}

pub async fn list(paths: &Paths) -> Vec<Skin> {
    if !library(paths).exists() && tokio::fs::create_dir_all(library(paths)).await.is_ok() {
        let _ = tokio::fs::write(library(paths).join("straysmp.png"), DEFAULT_SKIN).await;
    }
    let idx = load_index(paths).await;
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
            if let Ok((width, height)) = validate(&bytes) {
                out.push(Skin {
                    variant: idx.get(&file).copied().unwrap_or_default(),
                    data_url: data_url(&bytes),
                    file,
                    width,
                    height,
                });
            }
        }
    }
    out
}

pub async fn add(paths: &Paths, name: &str, bytes: &[u8], variant: Variant) -> Result<String> {
    validate(bytes)?;
    let dir = library(paths);
    tokio::fs::create_dir_all(&dir).await?;
    let stem = name
        .trim()
        .trim_end_matches(".png")
        .trim_end_matches(".PNG");
    let mut base: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    base.truncate(40);
    if base.trim_matches('_').is_empty() {
        base = "skin".into();
    }
    let mut file = format!("{base}.png");
    let mut n = 2;
    while dir.join(&file).exists() {
        file = format!("{base}-{n}.png");
        n += 1;
    }
    tokio::fs::write(dir.join(&file), bytes).await?;
    let mut idx = load_index(paths).await;
    idx.insert(file.clone(), variant);
    save_index(paths, &idx).await?;
    Ok(file)
}

pub async fn set_variant(paths: &Paths, file: &str, variant: Variant) -> Result<()> {
    safe_name(file)?;
    let mut idx = load_index(paths).await;
    idx.insert(file.to_string(), variant);
    save_index(paths, &idx).await
}

pub async fn remove(paths: &Paths, file: &str) -> Result<()> {
    safe_name(file)?;
    let _ = tokio::fs::remove_file(library(paths).join(file)).await;
    let mut idx = load_index(paths).await;
    idx.remove(file);
    save_index(paths, &idx).await
}

#[derive(Deserialize)]
struct ProfileSkins {
    #[serde(default)]
    skins: Vec<ProfileSkin>,
}
#[derive(Deserialize)]
struct ProfileSkin {
    state: String,
    url: String,
    #[serde(default)]
    variant: String,
}

fn parse_variant(s: &str) -> Variant {
    if s.eq_ignore_ascii_case("slim") {
        Variant::Slim
    } else {
        Variant::Classic
    }
}

async fn download_png(url: &str) -> Result<Vec<u8>> {
    // Mojang serves textures over http:// in profile data; always fetch over https.
    let url = url.replacen("http://", "https://", 1);
    Ok(http::client()
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?
        .to_vec())
}

/// The skin on the signed-in account (needs a Minecraft token).
pub async fn current(paths: &Paths) -> Result<CurrentSkin> {
    let acc = auth::valid_account(paths).await?;
    let p: ProfileSkins = http::client()
        .get(PROFILE)
        .bearer_auth(&acc.access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let Some(s) = p.skins.into_iter().find(|s| s.state == "ACTIVE") else {
        return Ok(CurrentSkin {
            variant: Variant::Classic,
            data_url: None,
        });
    };
    let bytes = download_png(&s.url).await?;
    Ok(CurrentSkin {
        variant: parse_variant(&s.variant),
        data_url: Some(data_url(&bytes)),
    })
}

/// Upload a library skin to the Microsoft account (official API → visible on every server).
pub async fn apply(paths: &Paths, file: &str) -> Result<()> {
    safe_name(file)?;
    let bytes = tokio::fs::read(library(paths).join(file)).await?;
    validate(&bytes)?;
    let variant = load_index(paths)
        .await
        .get(file)
        .copied()
        .unwrap_or_default();
    let acc = auth::valid_account(paths).await?;
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(|e| Error::other(e.to_string()))?;
    let form = reqwest::multipart::Form::new()
        .text("variant", variant.api())
        .part("file", part);
    let res = http::client()
        .post(format!("{PROFILE}/skins"))
        .bearer_auth(&acc.access_token)
        .multipart(form)
        .send()
        .await?;
    if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(Error::other("skin_rate_limited"));
    }
    if !res.status().is_success() {
        return Err(Error::other(format!(
            "skin_upload_failed:{}",
            res.status().as_u16()
        )));
    }
    Ok(())
}

/// Back to the default Steve/Alex skin.
pub async fn reset(paths: &Paths) -> Result<()> {
    let acc = auth::valid_account(paths).await?;
    http::client()
        .delete(format!("{PROFILE}/skins/active"))
        .bearer_auth(&acc.access_token)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[derive(Deserialize)]
struct NameLookup {
    id: String,
}
#[derive(Deserialize)]
struct SessionProfile {
    properties: Vec<SessionProperty>,
}
#[derive(Deserialize)]
struct SessionProperty {
    name: String,
    value: String,
}

/// Copy another player's current skin into the library (public Mojang API, no login needed).
pub async fn import_player(paths: &Paths, name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty()
        || name.len() > 16
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(Error::other("skin_bad_name"));
    }
    let res = http::client()
        .get(format!(
            "https://api.mojang.com/users/profiles/minecraft/{name}"
        ))
        .send()
        .await?;
    if res.status() == reqwest::StatusCode::NOT_FOUND
        || res.status() == reqwest::StatusCode::NO_CONTENT
    {
        return Err(Error::other(format!("skin_no_player:{name}")));
    }
    let lookup: NameLookup = res.error_for_status()?.json().await?;
    let profile: SessionProfile = http::get_json(&format!(
        "https://sessionserver.mojang.com/session/minecraft/profile/{}",
        lookup.id
    ))
    .await?;
    let textures = profile
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .ok_or_else(|| Error::other(format!("skin_no_player:{name}")))?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&textures.value)
        .map_err(|e| Error::other(e.to_string()))?;
    let v: serde_json::Value = serde_json::from_slice(&decoded)?;
    let skin = &v["textures"]["SKIN"];
    let url = skin["url"]
        .as_str()
        .ok_or_else(|| Error::other(format!("skin_no_player:{name}")))?;
    let variant = parse_variant(skin["metadata"]["model"].as_str().unwrap_or("classic"));
    let bytes = download_png(url).await?;
    add(paths, name, &bytes, variant).await
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

    #[tokio::test]
    async fn library() {
        let root =
            std::env::temp_dir().join(format!("stray-skin-{}", uuid::Uuid::new_v4().simple()));
        let paths = Paths::new(root.clone());
        let first = super::list(&paths).await; // seeds the built-in skin
        assert_eq!(first.len(), 1);
        assert_eq!((first[0].width, first[0].height), (64, 64));
        assert!(add(&paths, "x", &png(64, 64)[..10], Variant::Classic)
            .await
            .is_err());
        assert!(add(&paths, "big", &png(128, 128), Variant::Classic)
            .await
            .is_err());
        let f = add(&paths, "Alex Skin", &png(64, 64), Variant::Slim)
            .await
            .unwrap();
        assert_eq!(f, "alex_skin.png");
        let old = add(&paths, "old", &png(64, 32), Variant::Classic)
            .await
            .unwrap();
        let all = super::list(&paths).await;
        assert_eq!(all.len(), 3);
        assert_eq!(
            all.iter().find(|s| s.file == f).unwrap().variant,
            Variant::Slim
        );
        set_variant(&paths, &old, Variant::Slim).await.unwrap();
        assert_eq!(
            super::list(&paths)
                .await
                .iter()
                .find(|s| s.file == old)
                .unwrap()
                .variant,
            Variant::Slim
        );
        remove(&paths, &f).await.unwrap();
        assert_eq!(super::list(&paths).await.len(), 2);
        assert!(remove(&paths, "../evil.png").await.is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
