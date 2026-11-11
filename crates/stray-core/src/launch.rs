//! Prepare everything in parallel and spawn the game with tuned JVM flags.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use crate::auth::Account;
use crate::mojang::{self, Arg, VersionJson};
use crate::paths::Paths;
use crate::settings::{GcPreset, Settings};
use crate::{fabric, java, mods, ProgressFn, Result, SERVER_HOST};

pub struct Prepared {
    pub game: String,
    pub version: VersionJson,
    pub java: PathBuf,
    pub java_major: u32,
}

/// Remember the Fabric loader per instance so launching works offline.
pub async fn loader_for(paths: &Paths, game: &str) -> Result<String> {
    let file = paths.instance(game).join("loader.txt");
    match fabric::latest_loader(game).await {
        Ok(l) => {
            tokio::fs::create_dir_all(paths.instance(game)).await?;
            let _ = tokio::fs::write(&file, &l).await;
            Ok(l)
        }
        Err(e) => tokio::fs::read_to_string(&file)
            .await
            .map(|s| s.trim().to_string())
            .map_err(|_| e),
    }
}

/// Download/verify everything. Java, libraries, assets and mods run concurrently.
pub async fn prepare(
    paths: &Paths,
    game: &str,
    settings: &Settings,
    progress: &ProgressFn,
) -> Result<Prepared> {
    progress(crate::Progress {
        stage: "meta",
        done: 0,
        total: 1,
    });
    let (vanilla, loader) =
        tokio::try_join!(mojang::version(paths, game), loader_for(paths, game))?;
    let profile = fabric::profile(paths, game, &loader).await?;
    let version = fabric::merge(vanilla, profile);

    let (component, major) = version
        .java_version
        .as_ref()
        .map(|j| (j.component.clone(), j.major_version))
        .unwrap_or_else(|| ("java-runtime-delta".into(), 21));

    let java_fut = async {
        if !settings.java_path.trim().is_empty() {
            Ok(PathBuf::from(settings.java_path.trim()))
        } else {
            java::ensure(paths, &component, progress).await
        }
    };
    let (java, _, _, _) = tokio::try_join!(
        java_fut,
        mojang::install_game_files(paths, &version, progress),
        mojang::install_assets(paths, &version, progress),
        mods::ensure_core(paths, game, settings.performance_mods(), progress),
    )?;
    // Capes + HUD modules. A failure here (e.g. keystrokes not built for this version yet)
    // must not stop the game from starting.
    let _ = crate::cosmetics::apply(paths, game, settings, progress).await;
    Ok(Prepared {
        game: game.to_string(),
        version,
        java,
        java_major: major,
    })
}

/// Number of performance cores on Apple Silicon (M1: 4, M1 Pro: 8, …), None elsewhere.
pub fn apple_performance_cores() -> Option<u32> {
    if !(cfg!(target_os = "macos") && cfg!(target_arch = "aarch64")) {
        return None;
    }
    let out = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.perflevel0.physicalcpu"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .ok()
        .filter(|n: &u32| *n > 0)
}

/// Performance JVM flags. G1 settings follow the well-known client tuning (short young-gen
/// pauses, early concurrent marking); ZGC is offered for machines with plenty of RAM.
pub fn jvm_flags(settings: &Settings, java_major: u32) -> Vec<String> {
    let mem = settings.memory_mb.clamp(1024, 65536);
    let mut f = vec![
        format!("-Xmx{mem}M"),
        format!("-Xms{}M", (mem / 2).min(2048)),
    ];
    match settings.gc {
        GcPreset::Balanced => f.extend(
            [
                "-XX:+UnlockExperimentalVMOptions",
                "-XX:+UseG1GC",
                "-XX:+ParallelRefProcEnabled",
                "-XX:MaxGCPauseMillis=37",
                "-XX:G1NewSizePercent=23",
                "-XX:G1MaxNewSizePercent=40",
                "-XX:G1HeapRegionSize=16M",
                "-XX:G1ReservePercent=20",
                "-XX:G1MixedGCCountTarget=3",
                "-XX:InitiatingHeapOccupancyPercent=20",
                "-XX:G1RSetUpdatingPauseTimePercent=0",
                "-XX:SurvivorRatio=32",
                "-XX:MaxTenuringThreshold=1",
                "-XX:+PerfDisableSharedMem",
                "-XX:+DisableExplicitGC",
            ]
            .map(String::from),
        ),
        GcPreset::LowLatency => {
            f.push("-XX:+UseZGC".into());
            if java_major == 21 {
                f.push("-XX:+ZGenerational".into());
            }
            f.extend(["-XX:+PerfDisableSharedMem", "-XX:+DisableExplicitGC"].map(String::from));
        }
    }
    // Apple Silicon: size the GC thread pools to the performance cores. By default the JVM
    // counts all cores, and GC threads parked on efficiency cores make every pause longer.
    if let Some(p) = apple_performance_cores() {
        f.push(format!("-XX:ParallelGCThreads={p}"));
        f.push(format!("-XX:ConcGCThreads={}", (p / 2).max(1)));
    }
    // JDK 25+: 8-byte object headers → ~10-20 % less heap for Minecraft's millions of small objects.
    if java_major >= 25 {
        f.push("-XX:+UseCompactObjectHeaders".into());
    }
    f.push("-XX:+UseCompressedOops".into());
    f.extend(settings.extra_jvm_args.split_whitespace().map(String::from));
    f
}

fn expand(
    args: &[Arg],
    features: &HashMap<&str, bool>,
    vars: &HashMap<&str, String>,
) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        let values = match a {
            Arg::Plain(s) => vec![s.clone()],
            Arg::Ruled { rules, value } => {
                if !mojang::rules_allow(rules, features) {
                    continue;
                }
                value.clone().into_vec()
            }
        };
        for v in values {
            let mut s = v;
            // Substitute every ${key}; unknown keys are kept as-is.
            while let Some(start) = s.find("${") {
                let Some(end_rel) = s[start..].find('}') else {
                    break;
                };
                let key = &s[start + 2..start + end_rel];
                match vars.get(key) {
                    Some(val) => s.replace_range(start..start + end_rel + 1, val),
                    None => break,
                }
            }
            out.push(s);
        }
    }
    out
}

/// Full command line (without the java binary).
pub fn build_args(paths: &Paths, p: &Prepared, settings: &Settings, acc: &Account) -> Vec<String> {
    let v = &p.version;
    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut cp: Vec<String> = Vec::new();
    for job in mojang::library_jobs(paths, &v.libraries) {
        let s = job.path.to_string_lossy().into_owned();
        if !cp.contains(&s) {
            cp.push(s);
        }
    }
    cp.push(
        mojang::client_jar(paths, v.vanilla_id())
            .to_string_lossy()
            .into_owned(),
    );

    let game_dir = paths.instance(&p.game);
    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", acc.name.clone()),
        ("version_name", p.game.clone()),
        ("game_directory", game_dir.to_string_lossy().into_owned()),
        ("assets_root", paths.assets().to_string_lossy().into_owned()),
        (
            "assets_index_name",
            v.asset_index
                .as_ref()
                .map(|a| a.id.clone())
                .or(v.assets.clone())
                .unwrap_or_default(),
        ),
        ("auth_uuid", acc.uuid.clone()),
        ("auth_access_token", acc.access_token.clone()),
        (
            "clientid",
            crate::auth::client_id().unwrap_or("").to_string(),
        ),
        ("auth_xuid", acc.xuid.clone()),
        ("user_type", "msa".into()),
        ("version_type", "StraySMP".into()),
        (
            "natives_directory",
            paths.natives(&p.game).to_string_lossy().into_owned(),
        ),
        ("launcher_name", "StrayLauncher".into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("classpath", cp.join(sep)),
        ("classpath_separator", sep.into()),
        (
            "library_directory",
            paths.libraries().to_string_lossy().into_owned(),
        ),
        ("resolution_width", settings.width.to_string()),
        ("resolution_height", settings.height.to_string()),
        ("quickPlayMultiplayer", SERVER_HOST.into()),
    ]);
    let features: HashMap<&str, bool> = HashMap::from([
        ("has_custom_resolution", true),
        ("is_quick_play_multiplayer", settings.auto_join),
    ]);

    let mut args = jvm_flags(settings, p.java_major);
    args.extend(expand(&v.arguments.jvm, &features, &vars));
    args.push(
        v.main_class
            .clone()
            .unwrap_or_else(|| "net.minecraft.client.main.Main".into()),
    );
    args.extend(expand(&v.arguments.game, &features, &vars));
    args
}

/// Spawn the game. stdout/stderr go to `logs/latest-launch.log`.
pub async fn spawn(
    paths: &Paths,
    p: &Prepared,
    settings: &Settings,
    acc: &Account,
) -> Result<tokio::process::Child> {
    let game_dir = paths.instance(&p.game);
    tokio::fs::create_dir_all(&game_dir).await?;
    tokio::fs::create_dir_all(paths.natives(&p.game)).await?;
    tokio::fs::create_dir_all(paths.logs()).await?;
    let log = std::fs::File::create(paths.logs().join("latest-launch.log"))?;
    let args = build_args(paths, p, settings, acc);
    let mut cmd = tokio::process::Command::new(&p.java);
    cmd.args(&args)
        .current_dir(&game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    Ok(cmd.spawn()?)
}
