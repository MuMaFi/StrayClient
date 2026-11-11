use std::path::{Path, PathBuf};

/// All on-disk locations. Shared game data (libraries, assets, runtimes) is stored once
/// and reused by every instance, so switching versions never re-downloads common files.
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    /// Folder name of the active profile's instance (shared by all clones). `None` = the
    /// default profile, which keeps one instance per game version.
    profile: std::sync::Arc<std::sync::RwLock<Option<String>>>,
}

impl Paths {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            profile: Default::default(),
        }
    }

    /// Switch the instance folder to a profile's own one (`None` = default profile).
    pub fn set_profile(&self, dir: Option<String>) {
        *self.profile.write().unwrap() = dir.map(|d| sanitize(&d));
    }

    /// `%APPDATA%\StrayLauncher` on Windows, `~/Library/Application Support/StrayLauncher` on macOS.
    pub fn default_root() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("StrayLauncher")
    }

    pub fn versions(&self) -> PathBuf {
        self.root.join("meta").join("versions")
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("meta").join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("meta").join("assets")
    }
    pub fn runtimes(&self) -> PathBuf {
        self.root.join("runtime")
    }
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn instance(&self, game_version: &str) -> PathBuf {
        let dir = self.profile.read().unwrap().clone();
        self.root
            .join("instances")
            .join(dir.unwrap_or_else(|| sanitize(game_version)))
    }
    pub fn natives(&self, game_version: &str) -> PathBuf {
        self.instance(game_version).join(".natives")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    /// Single-account file from the pre-1.0 test builds (migrated on first load).
    pub fn account_file(&self) -> PathBuf {
        self.root.join("account.json")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
}

pub fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Maven coordinate `group:artifact:version[:classifier]` → relative jar path.
pub fn maven_path(name: &str) -> Option<String> {
    let (coord, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let (artifact, version) = (parts[1], parts[2]);
    let file = match parts.get(3) {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!("{group}/{artifact}/{version}/{file}"))
}

pub async fn ensure_dir(p: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(p).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maven() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.16.9").unwrap(),
            "net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
    }
}
