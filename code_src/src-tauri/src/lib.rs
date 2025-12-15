mod db;

use db::{AppDatabase, GithubRuleRecord, IdeInstanceRecord, ModeRecord};
use tauri::State;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle();
            let db = AppDatabase::new(&handle)?;
            app.manage(db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            list_modes,
            save_mode,
            list_github_rules,
            save_github_rule,
            list_ide_instances,
            save_ide_instance
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
