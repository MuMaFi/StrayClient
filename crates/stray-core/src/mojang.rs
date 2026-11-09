//! Vanilla Minecraft metadata from Mojang's piston-meta.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::download::{self, Job};
use crate::paths::{maven_path, Paths};
use crate::{http, ProgressFn, Result};

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub versions: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    pub main_class: Option<String>,
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub arguments: Arguments,
    #[serde(default)]
    pub libraries: Vec<Library>,
    pub asset_index: Option<AssetIndexRef>,
    pub assets: Option<String>,
    pub downloads: Option<VersionDownloads>,
    pub java_version: Option<JavaVersion>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

impl VersionJson {
    /// The vanilla id (the client jar lives under it, even for a merged Fabric profile).
    pub fn vanilla_id(&self) -> &str {
        self.inherits_from.as_deref().unwrap_or(&self.id)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Arg>,
    #[serde(default)]
    pub jvm: Vec<Arg>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Ruled { rules: Vec<Rule>, value: OneOrMany },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn into_vec(self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s],
            OneOrMany::Many(v) => v,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    /// Maven repository base (Fabric-style libraries).
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub rules: Option<Vec<Rule>>,
    /// Legacy (<1.19) natives map — those versions are not supported.
    pub natives: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Artifact {
    pub path: Option<String>,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionDownloads {
    pub client: Artifact,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Debug, Deserialize)]
struct AssetIndex {
    objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

/// Mojang rule semantics: start disallowed, last matching rule wins.
pub fn rules_allow(rules: &[Rule], features: &HashMap<&str, bool>) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let os_ok = rule.os.as_ref().is_none_or(|os| {
            os.name.as_deref().is_none_or(|n| n == os_name())
                && os
                    .arch
                    .as_deref()
                    .is_none_or(|a| a != "x86" || cfg!(target_pointer_width = "32"))
        });
        let feat_ok = rule.features.as_ref().is_none_or(|f| {
            f.iter()
                .all(|(k, v)| features.get(k.as_str()).copied().unwrap_or(false) == *v)
        });
        if os_ok && feat_ok {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

pub async fn manifest(paths: &Paths) -> Result<Manifest> {
    let cache = paths.cache().join("version_manifest_v2.json");
    match http::get_json::<Manifest>(MANIFEST_URL).await {
        Ok(m) => {
            let _ = tokio::fs::create_dir_all(paths.cache()).await;
            let _ = tokio::fs::write(&cache, serde_json::to_vec(&m)?).await;
            Ok(m)
        }
        // Offline: use the last manifest we saw.
        Err(e) => match tokio::fs::read(&cache).await {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(_) => Err(e),
        },
    }
}

/// Download (or load cached) vanilla version JSON.
pub async fn version(paths: &Paths, id: &str) -> Result<VersionJson> {
    let file = paths.versions().join(id).join(format!("{id}.json"));
    if let Ok(bytes) = tokio::fs::read(&file).await {
        if let Ok(v) = serde_json::from_slice(&bytes) {
            return Ok(v);
        }
    }
    let m = manifest(paths).await?;
    let entry = m
        .versions
        .iter()
        .find(|v| v.id == id)
        .ok_or_else(|| crate::Error::other(format!("unknown version {id}")))?;
    let bytes = http::client()
        .get(&entry.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if download::sha1_hex(&bytes) != entry.sha1 {
        return Err(crate::Error::Checksum(entry.url.clone()));
    }
    tokio::fs::create_dir_all(file.parent().unwrap()).await?;
    tokio::fs::write(&file, &bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn client_jar(paths: &Paths, id: &str) -> PathBuf {
    paths.versions().join(id).join(format!("{id}.jar"))
}

/// Resolve every library that applies to this OS into (download job, classpath entry).
pub fn library_jobs(paths: &Paths, libs: &[Library]) -> Vec<Job> {
    let no_features = HashMap::new();
    let mut jobs = Vec::new();
    for lib in libs {
        if lib.natives.is_some() {
            continue;
        }
        if let Some(rules) = &lib.rules {
            if !rules_allow(rules, &no_features) {
                continue;
            }
        }
        if let Some(art) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let rel = art.path.clone().or_else(|| maven_path(&lib.name));
            if let Some(rel) = rel {
                jobs.push(Job {
                    url: art.url.clone(),
                    path: paths.libraries().join(rel),
                    sha1: art.sha1.clone(),
                    size: art.size,
                    executable: false,
                });
            }
        } else if let (Some(base), Some(rel)) = (&lib.url, maven_path(&lib.name)) {
            let base = base.trim_end_matches('/');
            jobs.push(Job {
                url: format!("{base}/{rel}"),
                path: paths.libraries().join(&rel),
                sha1: lib.sha1.clone(),
                size: lib.size,
                executable: false,
            });
        }
    }
    jobs
}

/// Client jar + libraries.
pub async fn install_game_files(
    paths: &Paths,
    v: &VersionJson,
    progress: &ProgressFn,
) -> Result<()> {
    let mut jobs = library_jobs(paths, &v.libraries);
    if let Some(d) = &v.downloads {
        jobs.push(Job {
            url: d.client.url.clone(),
            path: client_jar(paths, v.vanilla_id()),
            sha1: d.client.sha1.clone(),
            size: d.client.size,
            executable: false,
        });
    }
    download::run("libraries", jobs, progress).await
}

pub async fn install_assets(paths: &Paths, v: &VersionJson, progress: &ProgressFn) -> Result<()> {
    let Some(idx) = &v.asset_index else {
        return Ok(());
    };
    let index_path = paths
        .assets()
        .join("indexes")
        .join(format!("{}.json", idx.id));
    download::run(
        "assets",
        vec![Job {
            url: idx.url.clone(),
            path: index_path.clone(),
            sha1: Some(idx.sha1.clone()),
            size: idx.size,
            executable: false,
        }],
        progress,
    )
    .await?;
    let index: AssetIndex = serde_json::from_slice(&tokio::fs::read(&index_path).await?)?;
    let objects = paths.assets().join("objects");
    let mut seen = std::collections::HashSet::new();
    let jobs = index
        .objects
        .into_values()
        .filter(|o| seen.insert(o.hash.clone()))
        .map(|o| {
            let prefix = &o.hash[..2];
            Job {
                url: format!("{RESOURCES_URL}/{prefix}/{}", o.hash),
                path: objects.join(prefix).join(&o.hash),
                sha1: Some(o.hash.clone()),
                size: Some(o.size),
                executable: false,
            }
        })
        .collect();
    download::run("assets", jobs, progress).await
}
