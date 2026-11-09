fn main() {
    println!("cargo:rerun-if-env-changed=STRAY_MSA_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=CURSEFORGE_API_KEY");
    tauri_build::build()
}
