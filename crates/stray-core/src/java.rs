//! Mojang's official Java runtimes – the same JREs the vanilla launcher uses.
//! Users never need to install Java themselves.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::download::{self, Job};
use crate::paths::Paths;
use crate::{http, Error, ProgressFn, Result};

const ALL_URL: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[derive(Deserialize)]
struct RuntimeEntry {
    manifest: ManifestRef,
}
#[derive(Deserialize)]
struct ManifestRef {
    url: String,
}
#[derive(Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}
#[derive(Deserialize)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    executable: bool,
    downloads: Option<RuntimeDownloads>,
    #[cfg_attr(not(unix), allow(dead_code))]
    target: Option<String>,
}
#[derive(Deserialize)]
struct RuntimeDownloads {
    raw: RawDownload,
}
#[derive(Deserialize)]
struct RawDownload {
    url: String,
    sha1: String,
    size: u64,
}

pub fn platform_key() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "aarch64") => "windows-arm64",
        ("windows", "x86") => "windows-x86",
        ("windows", _) => "windows-x64",
        ("macos", "aarch64") => "mac-os-arm64",
        ("macos", _) => "mac-os",
        ("linux", "x86") => "linux-i386",
        _ => "linux",
    }
}

/// Platform key for a runtime. `x64` asks for the Intel build on an Apple Silicon Mac (it runs
/// through Rosetta): Minecraft before 1.19 only ships x86_64 natives for macOS.
pub fn platform_for(x64: bool) -> &'static str {
    if x64 && platform_key() == "mac-os-arm64" {
        "mac-os"
    } else {
        platform_key()
    }
}

pub fn runtime_dir(paths: &Paths, component: &str) -> PathBuf {
    runtime_dir_for(paths, component, false)
}

fn runtime_dir_for(paths: &Paths, component: &str, x64: bool) -> PathBuf {
    paths.runtimes().join(component).join(platform_for(x64))
}

pub fn java_exe(paths: &Paths, component: &str) -> PathBuf {
    java_exe_for(paths, component, false)
}

fn java_exe_for(paths: &Paths, component: &str, x64: bool) -> PathBuf {
    let base = runtime_dir_for(paths, component, x64);
    if cfg!(target_os = "windows") {
        // javaw = no console window flashing up behind the game.
        base.join("bin").join("javaw.exe")
    } else if cfg!(target_os = "macos") {
        base.join("jre.bundle/Contents/Home/bin/java")
    } else {
        base.join("bin").join("java")
    }
}

/// Ensure the runtime `component` (e.g. `java-runtime-delta`) is installed; returns the java binary.
pub async fn ensure(paths: &Paths, component: &str, progress: &ProgressFn) -> Result<PathBuf> {
    ensure_for(paths, component, false, progress).await
}

/// Like `ensure`, optionally the x86_64 runtime on Apple Silicon (see `platform_for`).
/// Mojang has no Apple Silicon build of some old runtimes (Java 8): then Intel is used too.
pub async fn ensure_for(
    paths: &Paths,
    component: &str,
    x64: bool,
    progress: &ProgressFn,
) -> Result<PathBuf> {
    let exe = java_exe_for(paths, component, x64);
    let marker = runtime_dir_for(paths, component, x64).join(".stray-complete");
    if exe.exists() && marker.exists() {
        return Ok(exe);
    }
    let all: HashMap<String, HashMap<String, Vec<RuntimeEntry>>> = http::get_json(ALL_URL).await?;
    let pick = |key: &str| {
        all.get(key)
            .and_then(|p| p.get(component))
            .and_then(|v| v.first())
    };
    let entry = match pick(platform_for(x64)) {
        Some(e) => e,
        None if !x64 && platform_key() == "mac-os-arm64" => {
            return Box::pin(ensure_for(paths, component, true, progress)).await;
        }
        None => {
            return Err(Error::other(format!(
                "no Java runtime {component} for {}",
                platform_for(x64)
            )))
        }
    };
    let manifest: RuntimeManifest = http::get_json(&entry.manifest.url).await?;
    let base = runtime_dir_for(paths, component, x64);
    let mut jobs = Vec::new();
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut links = Vec::new();
    for (rel, f) in manifest.files {
        let path = base.join(&rel);
        match f.kind.as_str() {
            "directory" => tokio::fs::create_dir_all(&path).await?,
            "file" => {
                if let Some(d) = f.downloads {
                    jobs.push(Job {
                        url: d.raw.url,
                        path,
                        sha1: Some(d.raw.sha1),
                        size: Some(d.raw.size),
                        executable: f.executable,
                    });
                }
            }
            "link" => links.push((path, f.target)),
            _ => {}
        }
    }
    download::run("java", jobs, progress).await?;
    #[cfg(unix)]
    for (path, target) in links {
        if let Some(t) = target {
            let _ = tokio::fs::remove_file(&path).await;
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let _ = tokio::fs::symlink(t, &path).await;
        }
    }
    tokio::fs::write(&marker, b"ok").await?;
    Ok(exe)
}
