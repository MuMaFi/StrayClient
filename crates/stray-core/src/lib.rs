//! Stray Launcher core: everything that does not need a window.
//!
//! * `mojang`   – version manifest, client jar, libraries, assets
//! * `fabric`   – Fabric loader profile merged on top of vanilla
//! * `java`     – Mojang's official Java runtimes (no system Java needed)
//! * `mods`     – Modrinth + CurseForge search / install / dependency resolution
//! * `auth`     – Microsoft device-code login → Minecraft token
//! * `launch`   – argument building and process spawning with tuned JVM flags
//! * `ping`     – Server List Ping for the live player counter

pub mod auth;
pub mod cosmetics;
pub mod download;
pub mod error;
pub mod fabric;
pub mod http;
pub mod java;
pub mod launch;
pub mod mods;
pub mod mojang;
pub mod paths;
pub mod ping;
pub mod settings;
pub mod skins;

pub use error::{Error, Result};

/// The server this launcher is built around.
pub const SERVER_HOST: &str = "play.straysmp.cc";
pub const SERVER_PORT: u16 = 25565;

/// Mods that are always installed and cannot be removed.
pub const CORE_MODS: &[&str] = &["sodium", "ferrite-core"];

/// Optional performance mods offered as one-click toggles (all Modrinth slugs).
pub const PERF_MODS: &[&str] = &["lithium", "immediatelyfast", "entityculling", "modernfix"];

/// Progress reported to the UI while preparing the game.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Progress {
    /// Machine-readable stage key, translated in the UI (`java`, `libraries`, `assets`, …).
    pub stage: &'static str,
    pub done: u64,
    pub total: u64,
}

pub type ProgressFn = std::sync::Arc<dyn Fn(Progress) + Send + Sync>;
