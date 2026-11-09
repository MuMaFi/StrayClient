//! Parallel, checksum-verified downloader.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::{stream, StreamExt};
use sha1::{Digest, Sha1};

use crate::{http, Error, Progress, ProgressFn, Result};

/// Concurrent connections. Mojang's CDN handles this fine and it saturates most home links.
const PARALLEL: usize = 48;
const RETRIES: usize = 3;

#[derive(Debug, Clone)]
pub struct Job {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub executable: bool,
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}

/// Existing files: every file was hash-verified when it was written (atomic rename), so on
/// relaunch a size match is enough. This keeps a warm start at a few milliseconds instead
/// of re-hashing ~600 MB of assets. Files without a known size fall back to hashing.
async fn is_valid(job: &Job) -> bool {
    let Ok(meta) = tokio::fs::metadata(&job.path).await else {
        return false;
    };
    if let Some(size) = job.size {
        return meta.len() == size;
    }
    match &job.sha1 {
        Some(expected) => match tokio::fs::read(&job.path).await {
            Ok(bytes) => sha1_hex(&bytes).eq_ignore_ascii_case(expected),
            Err(_) => false,
        },
        None => true,
    }
}

async fn fetch_one(job: &Job) -> Result<()> {
    if is_valid(job).await {
        return Ok(());
    }
    if let Some(parent) = job.path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut last_err = None;
    for attempt in 0..RETRIES {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(400 * attempt as u64)).await;
        }
        match try_fetch(job).await {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| Error::other("download failed")))
}

async fn try_fetch(job: &Job) -> Result<()> {
    let bytes = http::client()
        .get(&job.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if let Some(expected) = &job.sha1 {
        if !sha1_hex(&bytes).eq_ignore_ascii_case(expected) {
            return Err(Error::Checksum(job.url.clone()));
        }
    }
    // Write to a temp file then rename, so a crash never leaves a half-written jar behind.
    let tmp = tmp_path(&job.path);
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, &job.path).await?;
    #[cfg(unix)]
    if job.executable {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&job.path, std::fs::Permissions::from_mode(0o755)).await?;
    }
    Ok(())
}

fn tmp_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".part");
    PathBuf::from(s)
}

/// Download all jobs in parallel, reporting progress under `stage`.
pub async fn run(stage: &'static str, jobs: Vec<Job>, progress: &ProgressFn) -> Result<()> {
    let total = jobs.len() as u64;
    let done = Arc::new(AtomicU64::new(0));
    progress(Progress {
        stage,
        done: 0,
        total,
    });
    let results: Vec<Result<()>> = stream::iter(jobs)
        .map(|job| {
            let done = done.clone();
            let progress = progress.clone();
            async move {
                let r = fetch_one(&job).await;
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                // Throttle UI events: every 1% or the last one.
                if d == total || d.is_multiple_of((total / 100).max(1)) {
                    progress(Progress {
                        stage,
                        done: d,
                        total,
                    });
                }
                r
            }
        })
        .buffer_unordered(PARALLEL)
        .collect()
        .await;
    results.into_iter().collect()
}
