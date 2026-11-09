//! Manual check of the public skin import: `cargo run -p stray-core --example skin_import -- <dir> <player>`
#[tokio::main]
async fn main() {
    let mut a = std::env::args().skip(1);
    let paths = stray_core::paths::Paths::new(a.next().expect("dir").into());
    let name = a.next().expect("player");
    match stray_core::skins::import_player(&paths, &name).await {
        Ok(f) => {
            let s = stray_core::skins::list(&paths).await;
            let skin = s.iter().find(|x| x.file == f).unwrap();
            println!(
                "imported {f}: {}x{} {:?}",
                skin.width, skin.height, skin.variant
            );
        }
        Err(e) => println!("error: {e}"),
    }
}
