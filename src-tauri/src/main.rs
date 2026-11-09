#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use stray_core::auth::{self, Account, PublicAccount};
use stray_core::mods::{self, InstalledMod, SearchHit, Source};
use stray_core::paths::Paths;
use stray_core::ping::{self, ServerStatus};
use stray_core::settings::Settings;
use stray_core::{launch, Progress, ProgressFn, SERVER_HOST, SERVER_PORT};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::RwLock;

struct AppState {
    paths: Paths,
    settings: RwLock<Settings>,
    running: AtomicBool,
}

type Res<T> = Result<T, String>;

fn err(e: impl ToString) -> String {
    e.to_string()
}

fn progress_emitter(app: &AppHandle) -> ProgressFn {
    let app = app.clone();
    Arc::new(move |p: Progress| {
        let _ = app.emit("progress", p);
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Init {
    settings: Settings,
    account: Option<PublicAccount>,
    total_memory_mb: u64,
    has_client_id: bool,
    has_cf_key: bool,
    app_version: &'static str,
    os: &'static str,
    data_dir: String,
}

#[tauri::command]
async fn get_init(state: State<'_, AppState>) -> Res<Init> {
    let settings = state.settings.read().await.clone();
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    Ok(Init {
        has_cf_key: mods::curseforge_key(&settings.curseforge_key).is_some(),
        settings,
        account: Account::load(&state.paths).await.map(|a| a.public()),
        total_memory_mb: sys.total_memory() / 1024 / 1024,
        has_client_id: auth::client_id().is_some(),
        app_version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        data_dir: state.paths.root.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Res<bool> {
    settings.save(&state.paths).await.map_err(err)?;
    let has_cf = mods::curseforge_key(&settings.curseforge_key).is_some();
    *state.settings.write().await = settings;
    Ok(has_cf)
}

#[tauri::command]
async fn game_versions(state: State<'_, AppState>) -> Res<Vec<String>> {
    mods::supported_versions(&state.paths).await.map_err(err)
}

async fn current_game(state: &AppState) -> Res<String> {
    let chosen = state.settings.read().await.game_version.clone();
    if !chosen.is_empty() {
        return Ok(chosen);
    }
    mods::supported_versions(&state.paths)
        .await
        .map_err(err)?
        .into_iter()
        .next()
        .ok_or_else(|| "no_versions".to_string())
}

#[tauri::command]
async fn server_status() -> Res<ServerStatus> {
    Ok(ping::status(SERVER_HOST, SERVER_PORT)
        .await
        .unwrap_or_default())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginCode {
    user_code: String,
    verification_uri: String,
}

#[tauri::command]
async fn login_start(app: AppHandle, state: State<'_, AppState>) -> Res<LoginCode> {
    let code = auth::start_device_code().await.map_err(err)?;
    let paths = state.paths.clone();
    let out = LoginCode {
        user_code: code.user_code.clone(),
        verification_uri: code.verification_uri.clone(),
    };
    tauri::async_runtime::spawn(async move {
        let result = match auth::poll_device_code(&code).await {
            Ok(acc) => match acc.save(&paths).await {
                Ok(()) => serde_json::json!({ "ok": true, "account": acc.public() }),
                Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
            },
            Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
        };
        let _ = app.emit("auth", result);
    });
    Ok(out)
}

#[tauri::command]
async fn logout(state: State<'_, AppState>) -> Res<()> {
    Account::delete(&state.paths).await;
    Ok(())
}

#[tauri::command]
async fn mods_list(state: State<'_, AppState>, game: Option<String>) -> Res<Vec<InstalledMod>> {
    let game = match game {
        Some(g) if !g.is_empty() => g,
        _ => current_game(&state).await?,
    };
    Ok(mods::installed(&state.paths, &game).await)
}

#[tauri::command]
async fn mods_search(
    state: State<'_, AppState>,
    source: Source,
    query: String,
    game: String,
    offset: u32,
) -> Res<Vec<SearchHit>> {
    let key = state.settings.read().await.curseforge_key.clone();
    mods::search(&state.paths, source, &query, &game, offset, &key)
        .await
        .map_err(err)
}

#[tauri::command]
async fn mod_install(
    app: AppHandle,
    state: State<'_, AppState>,
    source: Source,
    project_id: String,
    game: String,
) -> Res<Vec<String>> {
    let key = state.settings.read().await.curseforge_key.clone();
    mods::install(
        &state.paths,
        source,
        &project_id,
        &game,
        &key,
        &progress_emitter(&app),
    )
    .await
    .map_err(err)
}

#[tauri::command]
async fn mod_toggle(
    state: State<'_, AppState>,
    game: String,
    project_id: String,
    enabled: bool,
) -> Res<()> {
    mods::set_enabled(&state.paths, &game, &project_id, enabled)
        .await
        .map_err(err)
}

#[tauri::command]
async fn mod_remove(state: State<'_, AppState>, game: String, project_id: String) -> Res<()> {
    mods::remove(&state.paths, &game, &project_id)
        .await
        .map_err(err)
}

#[tauri::command]
async fn open_dir(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    game: Option<String>,
) -> Res<()> {
    let game = game.unwrap_or_default();
    let p = match kind.as_str() {
        "mods" => state.paths.instance(&game).join("mods"),
        "instance" => state.paths.instance(&game),
        "logs" => state.paths.logs(),
        _ => state.paths.root.clone(),
    };
    tokio::fs::create_dir_all(&p).await.map_err(err)?;
    app.opener()
        .open_path(p.to_string_lossy(), None::<&str>)
        .map_err(err)
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Res<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("bad url".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(err)
}

#[tauri::command]
async fn play(app: AppHandle, state: State<'_, AppState>) -> Res<()> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("already_running".into());
    }
    let result = play_inner(&app, &state).await;
    if result.is_err() {
        state.running.store(false, Ordering::SeqCst);
    }
    result
}

async fn play_inner(app: &AppHandle, state: &AppState) -> Res<()> {
    let settings = state.settings.read().await.clone();
    let game = current_game(state).await?;
    let paths = state.paths.clone();
    // Check the account first so the user isn't kept waiting for downloads only to fail.
    let account = auth::valid_account(&paths).await.map_err(err)?;
    let prepared = launch::prepare(&paths, &game, &settings, &progress_emitter(app))
        .await
        .map_err(err)?;
    let mut child = launch::spawn(&paths, &prepared, &settings, &account)
        .await
        .map_err(err)?;
    let _ = app.emit(
        "game",
        serde_json::json!({ "state": "running", "version": game }),
    );
    let window = app.get_webview_window("main");
    if settings.close_on_launch {
        if let Some(w) = &window {
            let _ = w.minimize();
        }
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let code = child.wait().await.ok().and_then(|s| s.code());
        app2.state::<AppState>()
            .running
            .store(false, Ordering::SeqCst);
        if let Some(w) = app2.get_webview_window("main") {
            let _ = w.unminimize();
        }
        let _ = app2.emit(
            "game",
            serde_json::json!({ "state": "exited", "code": code }),
        );
    });
    Ok(())
}

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");
    tauri::async_runtime::set(rt.handle().clone());
    let paths = Paths::new(Paths::default_root());
    let settings = rt.block_on(Settings::load(&paths));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            paths,
            settings: RwLock::new(settings),
            running: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            get_init,
            save_settings,
            game_versions,
            server_status,
            login_start,
            logout,
            mods_list,
            mods_search,
            mod_install,
            mod_toggle,
            mod_remove,
            open_dir,
            open_url,
            play
        ])
        .run(tauri::generate_context!())
        .expect("error while running Stray Launcher");
}
