//! Mod management: Modrinth + CurseForge search, install with required dependencies,
//! enable/disable and removal. Sodium and FerriteCore are locked in.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::download::{self, Job};
use crate::paths::Paths;
use crate::{http, Error, ProgressFn, Result, CORE_MODS};

const MODRINTH: &str = "https://api.modrinth.com/v2";
const CURSEFORGE: &str = "https://api.curseforge.com/v1";
const CF_MINECRAFT: u32 = 432;
const CF_CLASS_MODS: u32 = 6;
const CF_LOADER_FABRIC: u32 = 4;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Modrinth,
    Curseforge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub source: Source,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub version: String,
    pub file_name: String,
    pub icon_url: Option<String>,
    pub enabled: bool,
    /// Core mods (Sodium, FerriteCore) cannot be removed.
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub source: Source,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub installed: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ModList {
    mods: Vec<InstalledMod>,
}

pub fn curseforge_key(user_key: &str) -> Option<String> {
    if !user_key.trim().is_empty() {
        return Some(user_key.trim().to_string());
    }
    option_env!("CURSEFORGE_API_KEY")
        .filter(|k| !k.is_empty())
        .map(String::from)
}

fn mods_dir(paths: &Paths, game: &str) -> PathBuf {
    paths.instance(game).join("mods")
}
fn list_file(paths: &Paths, game: &str) -> PathBuf {
    paths.instance(game).join("stray-mods.json")
}

async fn load_list(paths: &Paths, game: &str) -> ModList {
    match tokio::fs::read(list_file(paths, game)).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => ModList::default(),
    }
}

async fn save_list(paths: &Paths, game: &str, list: &ModList) -> Result<()> {
    tokio::fs::create_dir_all(paths.instance(game)).await?;
    tokio::fs::write(list_file(paths, game), serde_json::to_vec_pretty(list)?).await?;
    Ok(())
}

/// Installed mods; entries whose file vanished (user deleted it by hand) are dropped.
pub async fn installed(paths: &Paths, game: &str) -> Vec<InstalledMod> {
    let mut list = load_list(paths, game).await;
    let dir = mods_dir(paths, game);
    list.mods.retain(|m| {
        dir.join(&m.file_name).exists() || dir.join(format!("{}.disabled", m.file_name)).exists()
    });
    // the jar on disk is the truth (the in-game menu can switch mods too)
    for m in &mut list.mods {
        m.enabled = dir.join(&m.file_name).exists();
    }
    list.mods
}

/// Enable/disable a mod by its jar name in an instance folder (used for changes made in the
/// in-game menu). Locked mods stay on; unknown names outside `mods/` are ignored.
pub async fn set_enabled_file(
    instance: &Path,
    file_name: &str,
    enabled: bool,
) -> Result<Option<String>> {
    if file_name.contains(['/', '\\']) || file_name.contains("..") || !file_name.ends_with(".jar") {
        return Ok(None);
    }
    let list_path = instance.join("stray-mods.json");
    let mut list: ModList = match tokio::fs::read(&list_path).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => ModList::default(),
    };
    if list
        .mods
        .iter()
        .any(|m| m.file_name == file_name && m.locked)
        && !enabled
    {
        return Ok(None);
    }
    let dir = instance.join("mods");
    let on = dir.join(file_name);
    let off = dir.join(format!("{file_name}.disabled"));
    if enabled && off.exists() {
        tokio::fs::rename(&off, &on).await?;
    } else if !enabled && on.exists() {
        tokio::fs::rename(&on, &off).await?;
    }
    let mut slug = None;
    if let Some(m) = list.mods.iter_mut().find(|m| m.file_name == file_name) {
        m.enabled = enabled;
        slug = Some(m.slug.clone());
        tokio::fs::write(&list_path, serde_json::to_vec_pretty(&list)?).await?;
    }
    Ok(slug)
}

// ───────────────────────── Modrinth ─────────────────────────

#[derive(Deserialize)]
struct MrSearch {
    hits: Vec<MrHit>,
}
#[derive(Deserialize)]
struct MrHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    author: String,
    downloads: u64,
    icon_url: Option<String>,
}
#[derive(Deserialize)]
struct MrProject {
    id: String,
    slug: String,
    title: String,
    icon_url: Option<String>,
}
#[derive(Deserialize)]
struct MrVersion {
    version_number: String,
    #[serde(default)]
    game_versions: Vec<String>,
    files: Vec<MrFile>,
    dependencies: Vec<MrDep>,
}
#[derive(Deserialize)]
struct MrFile {
    url: String,
    filename: String,
    primary: bool,
    size: u64,
    hashes: HashMap<String, String>,
}
#[derive(Deserialize)]
struct MrDep {
    project_id: Option<String>,
    dependency_type: String,
}

fn q(v: &impl Serialize) -> String {
    urlencoding::encode(&serde_json::to_string(v).unwrap_or_default()).into_owned()
}

async fn modrinth_search(query: &str, game: &str, offset: u32) -> Result<Vec<SearchHit>> {
    let facets = q(&vec![
        vec!["project_type:mod".to_string()],
        vec!["categories:fabric".to_string()],
        vec![format!("versions:{game}")],
    ]);
    let index = if query.is_empty() {
        "downloads"
    } else {
        "relevance"
    };
    let url = format!(
        "{MODRINTH}/search?query={}&facets={facets}&limit=24&offset={offset}&index={index}",
        urlencoding::encode(query)
    );
    let r: MrSearch = http::get_json(&url).await?;
    Ok(r.hits
        .into_iter()
        .map(|h| SearchHit {
            source: Source::Modrinth,
            project_id: h.project_id,
            slug: h.slug,
            title: h.title,
            description: h.description,
            author: h.author,
            downloads: h.downloads,
            icon_url: h.icon_url,
            installed: false,
        })
        .collect())
}

async fn modrinth_versions(project: &str, game: Option<&str>) -> Result<Vec<MrVersion>> {
    let mut url = format!(
        "{MODRINTH}/project/{project}/version?loaders={}",
        q(&["fabric"])
    );
    if let Some(g) = game {
        url.push_str(&format!("&game_versions={}", q(&[g])));
    }
    http::get_json(&url).await
}

/// Every game version a Modrinth project has a Fabric build for.
pub async fn modrinth_game_versions(project: &str) -> Result<HashSet<String>> {
    let fabric: Vec<MrVersion> = modrinth_versions(project, None).await?;
    Ok(fabric.into_iter().flat_map(|v| v.game_versions).collect())
}

// ───────────────────────── CurseForge ─────────────────────────

#[derive(Deserialize)]
struct CfList<T> {
    data: Vec<T>,
}
#[derive(Deserialize)]
struct CfOne<T> {
    data: T,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u64,
    slug: String,
    name: String,
    summary: String,
    download_count: f64,
    logo: Option<CfLogo>,
    #[serde(default)]
    authors: Vec<CfAuthor>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLogo {
    thumbnail_url: String,
}
#[derive(Deserialize)]
struct CfAuthor {
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    display_name: String,
    file_name: String,
    file_length: u64,
    download_url: Option<String>,
    #[serde(default)]
    hashes: Vec<CfHash>,
    #[serde(default)]
    dependencies: Vec<CfDep>,
}
#[derive(Deserialize)]
struct CfHash {
    value: String,
    algo: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfDep {
    mod_id: u64,
    relation_type: u32,
}

async fn cf_get<T: serde::de::DeserializeOwned>(key: &str, url: &str) -> Result<T> {
    Ok(http::client()
        .get(url)
        .header("x-api-key", key)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn curseforge_search(
    key: &str,
    query: &str,
    game: &str,
    offset: u32,
) -> Result<Vec<SearchHit>> {
    let url = format!(
        "{CURSEFORGE}/mods/search?gameId={CF_MINECRAFT}&classId={CF_CLASS_MODS}&modLoaderType={CF_LOADER_FABRIC}\
         &gameVersion={game}&searchFilter={}&sortField={}&sortOrder=desc&pageSize=24&index={offset}",
        urlencoding::encode(query),
        if query.is_empty() { 6 } else { 2 } // 6 = total downloads, 2 = popularity
    );
    let r: CfList<CfMod> = cf_get(key, &url).await?;
    Ok(r.data
        .into_iter()
        .map(|m| SearchHit {
            source: Source::Curseforge,
            project_id: m.id.to_string(),
            slug: m.slug,
            title: m.name,
            description: m.summary,
            author: m
                .authors
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_default(),
            downloads: m.download_count as u64,
            icon_url: m.logo.map(|l| l.thumbnail_url),
            installed: false,
        })
        .collect())
}

// ───────────────────────── public API ─────────────────────────

pub async fn search(
    paths: &Paths,
    source: Source,
    query: &str,
    game: &str,
    offset: u32,
    cf_user_key: &str,
) -> Result<Vec<SearchHit>> {
    let mut hits = match source {
        Source::Modrinth => modrinth_search(query, game, offset).await?,
        Source::Curseforge => {
            let key = curseforge_key(cf_user_key).ok_or_else(|| Error::other("cf_no_key"))?;
            curseforge_search(&key, query, game, offset).await?
        }
    };
    let installed: HashSet<(Source, String)> = installed(paths, game)
        .await
        .into_iter()
        .map(|m| (m.source, m.project_id))
        .collect();
    for h in &mut hits {
        h.installed = installed.contains(&(h.source, h.project_id.clone()));
    }
    Ok(hits)
}

struct Resolved {
    meta: InstalledMod,
    job: Job,
    deps: Vec<String>,
}

async fn resolve_modrinth(project: &str, game: &str) -> Result<Resolved> {
    let p: MrProject = http::get_json(&format!("{MODRINTH}/project/{project}")).await?;
    let versions = modrinth_versions(&p.id, Some(game)).await?;
    let v = versions
        .into_iter()
        .next()
        .ok_or_else(|| Error::other(format!("no_version:{}", p.title)))?;
    let file = v
        .files
        .iter()
        .find(|f| f.primary)
        .or(v.files.first())
        .ok_or_else(|| Error::other("no file"))?;
    let deps = v
        .dependencies
        .iter()
        .filter(|d| d.dependency_type == "required")
        .filter_map(|d| d.project_id.clone())
        .collect();
    Ok(Resolved {
        job: Job {
            url: file.url.clone(),
            path: PathBuf::from(&file.filename),
            sha1: file.hashes.get("sha1").cloned(),
            size: Some(file.size),
            executable: false,
        },
        meta: InstalledMod {
            source: Source::Modrinth,
            project_id: p.id,
            slug: p.slug.clone(),
            title: p.title,
            version: v.version_number,
            file_name: file.filename.clone(),
            icon_url: p.icon_url,
            enabled: true,
            locked: CORE_MODS.contains(&p.slug.as_str()),
        },
        deps,
    })
}

async fn resolve_curseforge(key: &str, id: &str, game: &str) -> Result<Resolved> {
    let m: CfOne<CfMod> = cf_get(key, &format!("{CURSEFORGE}/mods/{id}")).await?;
    let files: CfList<CfFile> = cf_get(
        key,
        &format!("{CURSEFORGE}/mods/{id}/files?gameVersion={game}&modLoaderType={CF_LOADER_FABRIC}&pageSize=1"),
    )
    .await?;
    let f = files
        .data
        .into_iter()
        .next()
        .ok_or_else(|| Error::other(format!("no_version:{}", m.data.name)))?;
    // Some authors disable third-party downloads; CurseForge then returns no URL.
    let url = f
        .download_url
        .ok_or_else(|| Error::other(format!("cf_no_download:{}", m.data.name)))?;
    Ok(Resolved {
        job: Job {
            url,
            path: PathBuf::from(&f.file_name),
            sha1: f
                .hashes
                .iter()
                .find(|h| h.algo == 1)
                .map(|h| h.value.clone()),
            size: Some(f.file_length),
            executable: false,
        },
        meta: InstalledMod {
            source: Source::Curseforge,
            project_id: m.data.id.to_string(),
            slug: m.data.slug,
            title: m.data.name,
            version: f.display_name,
            file_name: f.file_name,
            icon_url: m.data.logo.map(|l| l.thumbnail_url),
            enabled: true,
            locked: false,
        },
        deps: f
            .dependencies
            .iter()
            .filter(|d| d.relation_type == 3)
            .map(|d| d.mod_id.to_string())
            .collect(),
    })
}

/// Install a mod and (recursively) its required dependencies from the same platform.
/// Returns the titles of everything newly installed.
pub async fn install(
    paths: &Paths,
    source: Source,
    project: &str,
    game: &str,
    cf_user_key: &str,
    progress: &ProgressFn,
) -> Result<Vec<String>> {
    let cf_key = curseforge_key(cf_user_key);
    let mut list = load_list(paths, game).await;
    let have: HashSet<String> = list
        .mods
        .iter()
        .flat_map(|m| [m.project_id.clone(), m.slug.clone()])
        .collect();
    let mut queue = vec![project.to_string()];
    let mut seen = HashSet::new();
    let mut resolved = Vec::new();
    while let Some(id) = queue.pop() {
        if !seen.insert(id.clone()) || (id != project && have.contains(&id)) {
            continue;
        }
        let r = match source {
            Source::Modrinth => resolve_modrinth(&id, game).await?,
            Source::Curseforge => {
                let key = cf_key.as_deref().ok_or_else(|| Error::other("cf_no_key"))?;
                resolve_curseforge(key, &id, game).await?
            }
        };
        if have.contains(&r.meta.project_id) && id != project {
            continue;
        }
        queue.extend(r.deps.iter().cloned());
        resolved.push(r);
    }

    let dir = mods_dir(paths, game);
    tokio::fs::create_dir_all(&dir).await?;
    let jobs = resolved
        .iter()
        .map(|r| Job {
            path: dir.join(&r.job.path),
            ..r.job.clone()
        })
        .collect();
    download::run("mods", jobs, progress).await?;

    let mut titles = Vec::new();
    for r in resolved {
        // Replace an older file of the same project (update).
        if let Some(old) = list.mods.iter().find(|m| m.project_id == r.meta.project_id) {
            if old.file_name != r.meta.file_name {
                let _ = tokio::fs::remove_file(dir.join(&old.file_name)).await;
                let _ =
                    tokio::fs::remove_file(dir.join(format!("{}.disabled", old.file_name))).await;
            }
        }
        list.mods.retain(|m| m.project_id != r.meta.project_id);
        titles.push(r.meta.title.clone());
        list.mods.push(r.meta);
    }
    list.mods.sort_by(|a, b| {
        b.locked
            .cmp(&a.locked)
            .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    save_list(paths, game, &list).await?;
    Ok(titles)
}

pub async fn set_enabled(paths: &Paths, game: &str, project_id: &str, enabled: bool) -> Result<()> {
    let mut list = load_list(paths, game).await;
    let dir = mods_dir(paths, game);
    let m = list
        .mods
        .iter_mut()
        .find(|m| m.project_id == project_id)
        .ok_or_else(|| Error::other("not installed"))?;
    if m.locked && !enabled {
        return Err(Error::other("locked"));
    }
    let on = dir.join(&m.file_name);
    let off = dir.join(format!("{}.disabled", m.file_name));
    if enabled && off.exists() {
        tokio::fs::rename(&off, &on).await?;
    } else if !enabled && on.exists() {
        tokio::fs::rename(&on, &off).await?;
    }
    m.enabled = enabled;
    save_list(paths, game, &list).await
}

pub async fn remove(paths: &Paths, game: &str, project_id: &str) -> Result<()> {
    let mut list = load_list(paths, game).await;
    let dir = mods_dir(paths, game);
    if let Some(m) = list.mods.iter().find(|m| m.project_id == project_id) {
        if m.locked {
            return Err(Error::other("locked"));
        }
        let _ = tokio::fs::remove_file(dir.join(&m.file_name)).await;
        let _ = tokio::fs::remove_file(dir.join(format!("{}.disabled", m.file_name))).await;
    }
    list.mods.retain(|m| m.project_id != project_id);
    save_list(paths, game, &list).await
}

/// Make sure Sodium + FerriteCore are present for `game`. Cheap when already installed.
pub async fn ensure_core(paths: &Paths, game: &str, progress: &ProgressFn) -> Result<()> {
    let have: HashSet<String> = installed(paths, game)
        .await
        .into_iter()
        .map(|m| m.slug)
        .collect();
    for slug in CORE_MODS {
        if !have.contains(*slug) {
            install(paths, Source::Modrinth, slug, game, "", progress).await?;
        }
    }
    // Core mods are always enabled.
    let list = load_list(paths, game).await;
    for m in list.mods.iter().filter(|m| m.locked && !m.enabled) {
        set_enabled(paths, game, &m.project_id, true).await?;
    }
    Ok(())
}

/// Release versions (newest first) that Fabric, Sodium and FerriteCore all support.
pub async fn supported_versions(paths: &Paths) -> Result<Vec<String>> {
    let cache = paths.cache().join("supported_versions.json");
    let fetched: Result<Vec<String>> = async {
        let (manifest, fabric, sodium, ferrite) = tokio::try_join!(
            crate::mojang::manifest(paths),
            crate::fabric::game_versions(),
            modrinth_game_versions(CORE_MODS[0]),
            modrinth_game_versions(CORE_MODS[1]),
        )?;
        let fabric: HashSet<String> = fabric
            .into_iter()
            .filter(|g| g.stable)
            .map(|g| g.version)
            .collect();
        let mut out = Vec::new();
        for v in manifest.versions.iter().filter(|v| v.kind == "release") {
            if fabric.contains(&v.id) && sodium.contains(&v.id) && ferrite.contains(&v.id) {
                out.push(v.id.clone());
            }
            // Modern natives layout only (LWJGL 3.3+, 1.20+).
            if v.id == "1.20" {
                break;
            }
        }
        Ok(out)
    }
    .await;
    match fetched {
        Ok(list) if !list.is_empty() => {
            let _ = tokio::fs::create_dir_all(paths.cache()).await;
            let _ = tokio::fs::write(&cache, serde_json::to_vec(&list)?).await;
            Ok(list)
        }
        other => match tokio::fs::read(&cache).await {
            Ok(b) => Ok(serde_json::from_slice(&b)?),
            Err(_) => other,
        },
    }
}
