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
    /// Standard mods (like in Lunar) switched on/off by the player; missing → the default.
    pub extras: std::collections::BTreeMap<String, bool>,
    /// Own profiles: each has its own game folder (mods, worlds, options).
    pub profiles: Vec<Profile>,
    /// Id of the active profile; empty = the default profile.
    pub profile: String,
    /// `new` or `classic` home screen.
    pub home_layout: String,
    /// Skip the start-up animation.
    pub skip_intro: bool,
}

/// A player-made profile with its own mods.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Profile {
    /// `[a-z0-9]`, made by the UI; the instance folder is `instances/profile-<id>`.
    pub id: String,
    pub name: String,
    /// Empty = newest supported version.
    pub game_version: String,
    /// Sodium + FerriteCore.
    pub performance_mods: bool,
    /// The standard mods and HUD modules (Freelook, AppleSkin, Keystrokes, PvP Plus, …).
    pub standard_mods: bool,
}

impl Profile {
    pub fn valid_id(id: &str) -> bool {
        !id.is_empty() && id.len() <= 24 && id.chars().all(|c| c.is_ascii_alphanumeric())
    }
    pub fn dir(&self) -> String {
        format!("profile-{}", self.id.to_ascii_lowercase())
    }
}

impl Settings {
    /// The active own profile, if any (the default profile is `None`).
    pub fn active_profile(&self) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.id == self.profile && Profile::valid_id(&p.id))
    }
    /// Instance folder of the active profile for `Paths::set_profile`.
    pub fn profile_dir(&self) -> Option<String> {
        self.active_profile().map(Profile::dir)
    }
    /// The game version picked for the active profile (empty = newest).
    pub fn chosen_game(&self) -> &str {
        match self.active_profile() {
            Some(p) => &p.game_version,
            None => &self.game_version,
        }
    }
    /// Sodium + FerriteCore wanted in the active profile.
    pub fn performance_mods(&self) -> bool {
        self.active_profile().is_none_or(|p| p.performance_mods)
    }
    /// Standard mods and HUD modules wanted in the active profile.
    pub fn standard_mods(&self) -> bool {
        self.active_profile().is_none_or(|p| p.standard_mods)
    }

    /// Whether a standard mod (see `cosmetics::EXTRAS`) is on.
    pub fn extra_enabled(&self, id: &str) -> bool {
        self.extras.get(id).copied().unwrap_or_else(|| {
            crate::cosmetics::EXTRAS
                .iter()
                .any(|e| e.id == id && e.default_on)
        })
    }
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
            language: system_language().into(),
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
            extras: Default::default(),
            profiles: Vec::new(),
            profile: String::new(),
            home_layout: "new".into(),
            skip_intro: false,
        }
    }
}

/// First-start language from the OS: Russian or Ukrainian systems get their language,
/// everyone else English.
pub fn system_language() -> &'static str {
    language_for(sys_locale::get_locale().as_deref().unwrap_or("en"))
}

fn language_for(locale: &str) -> &'static str {
    let l = locale.to_ascii_lowercase();
    if l.starts_with("uk") {
        "uk"
    } else if l.starts_with("ru") || l.starts_with("be") || l.starts_with("kk") {
        "ru"
    } else {
        "en"
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

#[cfg(test)]
mod tests {
    #[test]
    fn profiles() {
        use super::{Profile, Settings};
        use crate::paths::Paths;
        let mut s = Settings {
            game_version: "26.2".into(),
            ..Default::default()
        };
        // default profile: everything on, one instance per game version
        assert!(s.active_profile().is_none());
        assert!(s.performance_mods() && s.standard_mods());
        assert_eq!(s.chosen_game(), "26.2");
        let paths = Paths::new("/data".into());
        paths.set_profile(s.profile_dir());
        assert!(paths.instance("26.2").ends_with("instances/26.2"));

        s.profiles.push(Profile {
            id: "pAbc1".into(),
            name: "PvP".into(),
            game_version: "26.3".into(),
            performance_mods: true,
            standard_mods: false,
        });
        s.profile = "pAbc1".into();
        assert_eq!(s.chosen_game(), "26.3");
        assert!(s.performance_mods() && !s.standard_mods());
        // its own folder, shared by every clone of Paths
        let clone = paths.clone();
        paths.set_profile(s.profile_dir());
        assert!(clone.instance("26.3").ends_with("instances/profile-pabc1"));

        // unknown or unsafe ids fall back to the default profile
        s.profile = "../x".into();
        assert!(s.active_profile().is_none());
        assert!(!Profile::valid_id("../x") && !Profile::valid_id(""));
        // old settings files without the new fields still load
        let old: Settings =
            serde_json::from_str(r#"{"language":"en","gameVersion":"26.3"}"#).unwrap();
        assert!(old.profiles.is_empty() && old.home_layout == "new");
    }

    #[test]
    fn languages() {
        assert_eq!(super::language_for("uk-UA"), "uk");
        assert_eq!(super::language_for("ru_RU"), "ru");
        assert_eq!(super::language_for("de-DE"), "en");
        assert_eq!(super::language_for("en-US"), "en");
    }
}
