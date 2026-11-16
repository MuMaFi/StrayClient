//! Launch check for any version: `cargo run -p stray-core --example version_check -- <data> <game> [secs]`
use std::sync::Arc;
use stray_core::{auth::Account, launch, mods, paths::Paths, settings::Settings, Progress};

#[tokio::main]
async fn main() -> stray_core::Result<()> {
    let root = std::env::args().nth(1).expect("data dir");
    let game = std::env::args().nth(2).expect("game version");
    let secs: u64 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(40);
    let paths = Paths::new(root.into());
    let list = mods::all_versions(&paths).await?;
    let entry = list.iter().find(|v| v.id == game);
    println!("listed: {entry:?} (of {} versions)", list.len());
    let settings = Settings::default();
    let progress = Arc::new(|p: Progress| {
        if p.done == p.total {
            println!("  {} {}/{}", p.stage, p.done, p.total);
        }
    });
    let t = std::time::Instant::now();
    let prepared = launch::prepare(&paths, &game, &settings, &(progress as _)).await?;
    println!(
        "prepared {game} in {:?}: java {:?} (major {}), main {:?}",
        t.elapsed(),
        prepared.java,
        prepared.java_major,
        prepared.version.main_class
    );
    let acc = Account {
        name: "VersionCheck".into(),
        uuid: "00000000000000000000000000000000".into(),
        xuid: "0".into(),
        access_token: "0".into(),
        refresh_token: String::new(),
        expires_at: u64::MAX,
    };
    let args = launch::build_args(&paths, &prepared, &settings, &acc);
    let unresolved: Vec<_> = args.iter().filter(|a| a.contains("${")).collect();
    println!("args: {} (unresolved: {unresolved:?})", args.len());
    let mut child = launch::spawn(&paths, &prepared, &settings, &acc).await?;
    let natives = std::fs::read_dir(paths.natives(&game))
        .map(|d| d.count())
        .unwrap_or(0);
    println!("natives unpacked: {natives}");
    tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
    println!(
        "still running after {secs}s: {}",
        child.try_wait()?.is_none()
    );
    let _ = child.kill().await;
    Ok(())
}
