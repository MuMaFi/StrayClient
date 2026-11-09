use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::Result;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum GcPreset {
    /// Tuned G1 – best all-rounder.
    #[default]
    Balanced,
    /// Generational ZGC – lowest pause times, wants ≥ 6 GB RAM.
    LowLatency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// `ru` or `en`.
    pub language: String,
    /// Empty = newest version supported by Sodium + FerriteCore.
    pub game_version: String,
    pub memory_mb: u32,
    pub gc: GcPreset,
    pub extra_jvm_args: String,
    /// Optional custom Java binary instead of Mojang's runtime.
    pub java_path: String,
    /// Join play.straysmp.cc directly after the game starts.
    pub auto_join: bool,
    pub close_on_launch: bool,
    /// Optional user-supplied CurseForge API key (the build may embed one).
    pub curseforge_key: String,
    pub width: u32,
    pub height: u32,
    /// Turn off the falling-leaves animation.
    pub reduce_motion: bool,
    /// `account` (Mojang cape), `local` (PNG from the cape library) or `none`.
    pub cape_mode: String,
    /// File name in the cape library used when `cape_mode` is `local`.
    pub cape: String,
    pub keystrokes: Keystrokes,
    /// PvP Plus module (bundled mod, Minecraft 26.3).
    pub pvp_plus: bool,
    /// `auto`: direct Microsoft login when the build has a client ID, otherwise play through
    /// the official Minecraft Launcher. `direct` / `official` force one of the two.
    pub play_mode: String,
}

/// HUD module: WASD, mouse buttons with CPS, space bar, FPS and ping (Modern Keystrokes).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Keystrokes {
    pub enabled: bool,
    pub keys: bool,
    pub mouse: bool,
    pub space: bool,
    pub fps: bool,
    pub ping: bool,
}

impl Default for Keystrokes {
    fn default() -> Self {
        Self {
            enabled: false,
            keys: true,
            mouse: true,
            space: true,
            fps: false,
            ping: false,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "ru".into(),
            game_version: String::new(),
            memory_mb: 4096,
            gc: GcPreset::Balanced,
            extra_jvm_args: String::new(),
            java_path: String::new(),
            auto_join: true,
            close_on_launch: false,
            curseforge_key: String::new(),
            width: 1280,
            height: 720,
            reduce_motion: false,
            cape_mode: "account".into(),
            cape: String::new(),
            keystrokes: Keystrokes::default(),
            pvp_plus: false,
            play_mode: "auto".into(),
        }
    }
}

impl Settings {
    pub async fn load(paths: &Paths) -> Self {
        match tokio::fs::read(paths.settings_file()).await {
            Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub async fn save(&self, paths: &Paths) -> Result<()> {
        tokio::fs::create_dir_all(&paths.root).await?;
        tokio::fs::write(paths.settings_file(), serde_json::to_vec_pretty(self)?).await?;
        Ok(())
    }
}
