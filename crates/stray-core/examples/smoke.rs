//! End-to-end smoke test: resolve versions, download everything, build the command line.
//! `cargo run -p stray-core --example smoke -- <data-dir> [run]`
use std::sync::Arc;
use std::time::Instant;
use stray_core::{auth::Account, launch, mods, paths::Paths, settings::Settings, Progress};

#[tokio::main]
async fn main() -> stray_core::Result<()> {
    let root = std::env::args().nth(1).expect("data dir");
    let run = std::env::args().nth(2).as_deref() == Some("run");
    let paths = Paths::new(root.into());
    let t = Instant::now();
    let versions = mods::supported_versions(&paths).await?;
    println!(
        "supported: {:?} ({:?})",
        &versions[..versions.len().min(8)],
        t.elapsed()
    );
    let game = std::env::var("GAME").unwrap_or_else(|_| versions[0].clone());
    let mut settings = Settings::default();
    if std::env::var("MODULES").is_ok() {
        // also install the HUD modules (PvP Plus, Keystrokes)
        settings.pvp_plus = true;
        settings.keystrokes.enabled = true;
    }
    let last = Arc::new(std::sync::Mutex::new(String::new()));
    let l2 = last.clone();
    let progress = Arc::new(move |p: Progress| {
        let s = format!("{} {}/{}", p.stage, p.done, p.total);
        let mut g = l2.lock().unwrap();
        if p.done == p.total && *g != s {
            println!("  {s}");
        }
        *g = s;
    });
    let t = Instant::now();
    let prepared = launch::prepare(&paths, &game, &settings, &(progress.clone() as _)).await?;
    println!(
        "prepared {game} in {:?}, java {:?}",
        t.elapsed(),
        prepared.java
    );
    let t = Instant::now();
    let _ = launch::prepare(&paths, &game, &settings, &(progress.clone() as _)).await?;
    println!("warm prepare in {:?}", t.elapsed());
    let installed = mods::install(
        &paths,
        mods::Source::Modrinth,
        "lithium",
        &game,
        "",
        &(progress as _),
    )
    .await?;
    println!("installed extra: {installed:?}");
    for m in mods::installed(&paths, &game).await {
        println!("  mod {} {} locked={}", m.title, m.version, m.locked);
    }
    let acc = Account {
        name: "SmokeTest".into(),
        uuid: "00000000000000000000000000000000".into(),
        xuid: "0".into(),
        access_token: "0".into(),
        refresh_token: String::new(),
        expires_at: u64::MAX,
    };
    let args = launch::build_args(&paths, &prepared, &settings, &acc);
    let unresolved: Vec<_> = args.iter().filter(|a| a.contains("${")).collect();
    println!("args: {} (unresolved: {unresolved:?})", args.len());
    println!(
        "main class idx: {:?}",
        args.iter().find(|a| a.contains("KnotClient"))
    );
    if run {
        let mut child = launch::spawn(&paths, &prepared, &settings, &acc).await?;
        tokio::time::sleep(std::time::Duration::from_secs(90)).await;
        println!("still running after 90s: {}", child.try_wait()?.is_none());
        let _ = child.kill().await;
    }
    Ok(())
}
