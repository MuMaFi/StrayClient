//! Manual check: write the "Stray Client" profile into a test .minecraft folder.
//! `cargo run -p stray-core --example official_install -- <data-dir> <fake-.minecraft>`
use std::sync::Arc;
#[tokio::main]
async fn main() {
    let mut a = std::env::args().skip(1);
    let paths = stray_core::paths::Paths::new(a.next().expect("data dir").into());
    let mc: std::path::PathBuf = a.next().expect(".minecraft dir").into();
    let game = stray_core::mods::supported_versions(&paths).await.unwrap()[0].clone();
    let settings = stray_core::settings::Settings::default();
    let progress: stray_core::ProgressFn = Arc::new(|_| {});
    let id = stray_core::official::install_profile(&paths, &mc, &game, &settings, &progress)
        .await
        .unwrap();
    println!("game {game} → version {id}");
}
