mod db;
mod github;

use db::{AppDatabase, GithubRuleRecord, IdeInstanceRecord, ModeRecord};
use github::{sync_from_github, GithubSyncConfig, GithubSyncResult};
use std::sync::Mutex;
use tauri::{Manager, State};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle();
            let db = AppDatabase::new(&handle)?;
            app.manage(db);
            app.manage(Mutex::new(GithubSettingsState::default()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            list_modes,
            save_mode,
            list_github_rules,
            save_github_rule,
            list_ide_instances,
            save_ide_instance,
            scan_known_instances,
            get_github_settings,
            update_github_settings,
            sync_github_modes
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn list_modes(state: State<AppDatabase>) -> Result<Vec<ModeRecord>, String> {
    state.list_modes().map_err(|err| err.to_string())
}

#[tauri::command]
fn save_mode(payload: ModeRecord, state: State<AppDatabase>) -> Result<ModeRecord, String> {
    state.upsert_mode(payload).map_err(|err| err.to_string())
}

#[tauri::command]
fn list_github_rules(state: State<AppDatabase>) -> Result<Vec<GithubRuleRecord>, String> {
    state.list_github_rules().map_err(|err| err.to_string())
}

#[tauri::command]
fn save_github_rule(
    payload: GithubRuleRecord,
    state: State<AppDatabase>,
) -> Result<GithubRuleRecord, String> {
    state
        .upsert_github_rule(payload)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_ide_instances(state: State<AppDatabase>) -> Result<Vec<IdeInstanceRecord>, String> {
    state
        .list_ide_instances()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn save_ide_instance(
    payload: IdeInstanceRecord,
    state: State<AppDatabase>,
) -> Result<IdeInstanceRecord, String> {
    state
        .upsert_ide_instance(payload)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn scan_known_instances(state: State<AppDatabase>) -> Result<Vec<IdeInstanceRecord>, String> {
    state
        .sync_known_instances()
        .map_err(|err| err.to_string())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GithubSettingsState {
    pub token: String,
    pub proxy: Option<String>,
    pub delay_sec: u64,
    pub last_result: Option<GithubSyncResult>,
}

impl Default for GithubSettingsState {
    fn default() -> Self {
        Self {
            token: String::new(),
            proxy: None,
            delay_sec: 3,
            last_result: None,
        }
    }
}

#[tauri::command]
fn get_github_settings(state: State<'_, Mutex<GithubSettingsState>>) -> Result<GithubSettingsState, String> {
    state
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| "读取 GitHub 设置失败".to_string())
}

#[tauri::command]
fn update_github_settings(
    token: String,
    proxy: Option<String>,
    delay_sec: u64,
    state: State<'_, Mutex<GithubSettingsState>>,
) -> Result<(), String> {
    let mut guard = state
        .lock()
        .map_err(|_| "写入 GitHub 设置失败".to_string())?;
    guard.token = token;
    guard.proxy = proxy;
    guard.delay_sec = delay_sec.max(1);
    Ok(())
}

#[tauri::command]
async fn sync_github_modes(
    query: String,
    path_hint: String,
    state: State<'_, AppDatabase>,
    settings_state: State<'_, Mutex<GithubSettingsState>>,
) -> Result<GithubSyncResult, String> {
    let (token, delay_sec, proxy) = {
        let guard = settings_state
            .lock()
            .map_err(|_| "读取 GitHub 设置失败".to_string())?;
        (guard.token.clone(), guard.delay_sec, guard.proxy.clone())
    };
    let config = GithubSyncConfig {
        token,
        query,
        path_hint,
        delay_sec,
        proxy,
    };
    let result = sync_from_github(config, &state)
        .await
        .map_err(|err| err.to_string())?;
    if let Ok(mut guard) = settings_state.lock() {
        guard.last_result = Some(result.clone());
    }
    Ok(result)
}
