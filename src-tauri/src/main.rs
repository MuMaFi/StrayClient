#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use stray_core::auth::{self, Account, AccountList, AccountStore, PublicAccount};
use stray_core::mods::{self, InstalledMod, SearchHit, Source};
use stray_core::paths::Paths;
use stray_core::ping::{self, ServerStatus};
use stray_core::settings::Settings;
use stray_core::{
    cosmetics, launch, official, skins, Progress, ProgressFn, SERVER_HOST, SERVER_PORT,
};
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
    accounts: AccountList,
    total_memory_mb: u64,
    has_client_id: bool,
    has_cf_key: bool,
    app_version: &'static str,
    os: &'static str,
    arch: &'static str,
    /// Apple Silicon performance cores (None elsewhere).
    perf_cores: Option<u32>,
    /// "direct" or "official" (see effective_play_mode).
    play_mode: &'static str,
    official: official::Status,
    data_dir: String,
}

#[tauri::command]
async fn get_init(state: State<'_, AppState>) -> Res<Init> {
    let settings = sync_ingame(&state).await;
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let play_mode = effective_play_mode(&settings);
    Ok(Init {
        has_cf_key: mods::curseforge_key(&settings.curseforge_key).is_some(),
        settings,
        account: Account::load(&state.paths).await.map(|a| a.public()),
        accounts: AccountStore::load(&state.paths).await.list(),
        total_memory_mb: sys.total_memory() / 1024 / 1024,
        has_client_id: auth::client_id().is_some(),
        app_version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        perf_cores: launch::apple_performance_cores(),
        play_mode,
        official: official::status(),
        data_dir: state.paths.root.to_string_lossy().into_owned(),
    })
}

/// Take over changes made in the in-game menu (Right Shift) before showing or using settings.
async fn sync_ingame(state: &AppState) -> Settings {
    let mut settings = state.settings.read().await.clone();
    if cosmetics::take_ingame_changes(&state.paths, &mut settings).await {
        let _ = settings.save(&state.paths).await;
        *state.settings.write().await = settings.clone();
    }
    settings
}

#[tauri::command]
async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Res<bool> {
    settings.save(&state.paths).await.map_err(err)?;
    let has_cf = mods::curseforge_key(&settings.curseforge_key).is_some();
    state.paths.set_profile(settings.profile_dir());
    *state.settings.write().await = settings;
    Ok(has_cf)
}

/// Delete an own profile together with its game folder (mods, worlds, options).
#[tauri::command]
async fn profile_delete(state: State<'_, AppState>, id: String) -> Res<Settings> {
    if state.running.load(Ordering::SeqCst) {
        return Err("already_running".into());
    }
    let mut settings = state.settings.read().await.clone();
    let Some(pos) = settings.profiles.iter().position(|p| p.id == id) else {
        return Ok(settings);
    };
    let profile = settings.profiles.remove(pos);
    if settings.profile == id {
        settings.profile.clear();
    }
    if stray_core::settings::Profile::valid_id(&profile.id) {
        let dir = state.paths.root.join("instances").join(profile.dir());
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
    settings.save(&state.paths).await.map_err(err)?;
    state.paths.set_profile(settings.profile_dir());
    *state.settings.write().await = settings.clone();
    Ok(settings)
}

#[tauri::command]
async fn game_versions(state: State<'_, AppState>) -> Res<Vec<String>> {
    mods::supported_versions(&state.paths).await.map_err(err)
}

async fn current_game(state: &AppState) -> Res<String> {
    let chosen = state.settings.read().await.chosen_game().to_string();
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

/// Lunar-style sign-in: Microsoft's own login page in a separate window. We watch its
/// navigation for the redirect carrying the authorization code, then close it.
#[tauri::command]
async fn login_window(app: AppHandle, state: State<'_, AppState>) -> Res<()> {
    use tauri::{WebviewUrl, WebviewWindowBuilder, WindowEvent};
    const LABEL: &str = "msa-login";

    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.set_focus();
        return Ok(());
    }
    let pkce = auth::Pkce::new();
    let url: tauri::Url = auth::authorize_url(&pkce)
        .map_err(err)?
        .parse()
        .map_err(err)?;
    let done = Arc::new(AtomicBool::new(false));
    let paths = state.paths.clone();
    let title = if state.settings.read().await.language == "en" {
        "Sign in with Microsoft"
    } else {
        "Вход через Microsoft"
    };

    let nav_app = app.clone();
    let nav_done = done.clone();
    let window = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(url))
        .title(title)
        .inner_size(500.0, 680.0)
        .resizable(false)
        .center()
        .focused(true)
        .on_navigation(move |url| {
            let pkce = pkce.clone();
            match auth::parse_redirect(url.as_str(), &pkce) {
                auth::Redirect::NotOurs => true,
                result => {
                    nav_done.store(true, Ordering::SeqCst);
                    let app = nav_app.clone();
                    let paths = paths.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Some(w) = app.get_webview_window(LABEL) {
                            let _ = w.close();
                        }
                        let _ = app.emit("auth-busy", ());
                        let payload = match result {
                            auth::Redirect::Code(code) => match auth::exchange_code(&code, &pkce).await {
                                Ok(acc) => match acc.save(&paths).await {
                                    Ok(()) => serde_json::json!({ "ok": true, "account": acc.public() }),
                                    Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
                                },
                                Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
                            },
                            auth::Redirect::Error(e) if e == "access_denied" => {
                                serde_json::json!({ "ok": false, "error": "cancelled" })
                            }
                            auth::Redirect::Error(e) => serde_json::json!({ "ok": false, "error": e }),
                            auth::Redirect::NotOurs => unreachable!(),
                        };
                        let _ = app.emit("auth", payload);
                    });
                    false
                }
            }
        })
        .build()
        .map_err(err)?;

    // Closed by the user before finishing → tell the UI so it can reset.
    let close_app = app.clone();
    window.on_window_event(move |e| {
        if matches!(e, WindowEvent::Destroyed) && !done.load(Ordering::SeqCst) {
            let _ = close_app.emit(
                "auth",
                serde_json::json!({ "ok": false, "error": "cancelled" }),
            );
        }
    });
    Ok(())
}

#[tauri::command]
fn login_cancel(app: AppHandle) {
    if let Some(w) = app.get_webview_window("msa-login") {
        let _ = w.close();
    }
}

#[tauri::command]
async fn accounts_list(state: State<'_, AppState>) -> Res<AccountList> {
    Ok(AccountStore::load(&state.paths).await.list())
}

#[tauri::command]
async fn account_switch(state: State<'_, AppState>, uuid: String) -> Res<AccountList> {
    let mut store = AccountStore::load(&state.paths).await;
    if !store.switch(&uuid) {
        return Err("unknown account".into());
    }
    store.save(&state.paths).await.map_err(err)?;
    Ok(store.list())
}

#[tauri::command]
async fn account_remove(state: State<'_, AppState>, uuid: String) -> Res<AccountList> {
    let mut store = AccountStore::load(&state.paths).await;
    store.remove(&uuid);
    store.save(&state.paths).await.map_err(err)?;
    Ok(store.list())
}

#[tauri::command]
async fn capes_list(state: State<'_, AppState>) -> Res<Vec<cosmetics::Cape>> {
    Ok(cosmetics::list(&state.paths).await)
}

/// `data` is the PNG as base64 (from a file picker or drag & drop in the UI).
#[tauri::command]
async fn cape_add(state: State<'_, AppState>, name: String, data: String) -> Res<String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| "cape_not_png".to_string())?;
    cosmetics::add(&state.paths, &name, &bytes)
        .await
        .map_err(err)
}

#[tauri::command]
async fn cape_remove(state: State<'_, AppState>, file: String) -> Res<()> {
    cosmetics::remove(&state.paths, &file).await.map_err(err)
}

#[tauri::command]
async fn modules_availability(
    state: State<'_, AppState>,
    game: Option<String>,
) -> Res<cosmetics::Availability> {
    let game = match game {
        Some(g) if !g.is_empty() => g,
        _ => current_game(&state).await?,
    };
    Ok(cosmetics::availability(&game).await)
}

/// Apply cape + module settings to the instance right away (also done on every launch).
#[tauri::command]
async fn modules_apply(app: AppHandle, state: State<'_, AppState>) -> Res<()> {
    let settings = sync_ingame(&state).await;
    let game = current_game(&state).await?;
    cosmetics::apply(&state.paths, &game, &settings, &progress_emitter(&app))
        .await
        .map_err(err)
}

#[tauri::command]
async fn skins_list(state: State<'_, AppState>) -> Res<Vec<skins::Skin>> {
    Ok(skins::list(&state.paths).await)
}

#[tauri::command]
async fn skin_add(
    state: State<'_, AppState>,
    name: String,
    data: String,
    variant: skins::Variant,
) -> Res<String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| "skin_not_png".to_string())?;
    skins::add(&state.paths, &name, &bytes, variant)
        .await
        .map_err(err)
}

#[tauri::command]
async fn skin_variant(
    state: State<'_, AppState>,
    file: String,
    variant: skins::Variant,
) -> Res<()> {
    skins::set_variant(&state.paths, &file, variant)
        .await
        .map_err(err)
}

#[tauri::command]
async fn skin_remove(state: State<'_, AppState>, file: String) -> Res<()> {
    skins::remove(&state.paths, &file).await.map_err(err)
}

#[tauri::command]
async fn skin_apply(state: State<'_, AppState>, file: String) -> Res<()> {
    skins::apply(&state.paths, &file).await.map_err(err)
}

#[tauri::command]
async fn skin_current(state: State<'_, AppState>) -> Res<skins::CurrentSkin> {
    skins::current(&state.paths).await.map_err(err)
}

#[tauri::command]
async fn skin_reset(state: State<'_, AppState>) -> Res<()> {
    skins::reset(&state.paths).await.map_err(err)
}

#[tauri::command]
async fn skin_import(state: State<'_, AppState>, name: String) -> Res<String> {
    skins::import_player(&state.paths, &name).await.map_err(err)
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
        "skins" => state.paths.root.join("skins"),
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

/// `auto` → direct login when this build has a Microsoft client ID, else the official launcher.
fn effective_play_mode(settings: &Settings) -> &'static str {
    let direct = match settings.play_mode.as_str() {
        "direct" => true,
        "official" => false,
        _ => auth::DIRECT_LOGIN_BY_DEFAULT,
    };
    if direct && auth::client_id().is_some() {
        "direct"
    } else {
        "official"
    }
}

#[tauri::command]
fn official_status() -> official::Status {
    official::status()
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
    let settings = sync_ingame(state).await;
    let game = current_game(state).await?;
    let paths = state.paths.clone();
    if effective_play_mode(&settings) == "official" {
        // Through the official Minecraft Launcher: it signs the player in and starts the game.
        let version = official::play(&paths, &game, &settings, &progress_emitter(app))
            .await
            .map_err(err)?;
        state.running.store(false, Ordering::SeqCst);
        let _ = app.emit(
            "game",
            serde_json::json!({ "state": "official", "version": version }),
        );
        return Ok(());
    }
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
    paths.set_profile(settings.profile_dir());

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
            profile_delete,
            game_versions,
            server_status,
            login_start,
            login_window,
            login_cancel,
            accounts_list,
            official_status,
            capes_list,
            skins_list,
            skin_add,
            skin_variant,
            skin_remove,
            skin_apply,
            skin_current,
            skin_reset,
            skin_import,
            cape_add,
            cape_remove,
            modules_availability,
            modules_apply,
            account_switch,
            account_remove,
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
