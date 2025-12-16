mod db;
mod github;

use db::{
    AppDatabase, AppSettings, ApplyModesResult, BackupImportResult, BackupOptions, BackupPayload, GithubRuleRecord,
    IdeInstanceRecord, InstanceModeDiffSummary, InstanceModeItem, InstanceModeUpsertResult, ModeCompareItem, ModeDiffPreview,
    ModeHistoryRecord, ModeHistoryReplayResult, ModeImportReport, ModeMetaRecord, ModeRecord, SyncLogRecord, DbError,
};
use github::{sync_from_github, test_github_token, GithubSyncConfig, GithubSyncResult, GithubTokenTestResult};
use std::sync::Mutex;
use serde_json;
use serde_json::json;
use tauri::{Manager, State};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle();
            let db = AppDatabase::new(&handle)?;
            let settings = GithubSettingsState::load_from_db(&db);
            app.manage(db);
            app.manage(Mutex::new(settings));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            list_modes,
            save_mode,
            delete_mode,
            get_mode_meta,
            list_github_rules,
            save_github_rule,
            delete_github_rule,
            list_ide_instances,
            save_ide_instance,
            delete_ide_instance,
            scan_known_instances,
            scan_all_instances,
            scan_instance_modes,
            diff_instance_modes,
            list_instance_modes,
            get_instance_mode_raw,
            upsert_instance_mode,
            delete_instance_mode,
            preview_mode_diff,
            import_modes_from_text,
            import_instance_modes_to_db,
            apply_modes_to_instances,
            compare_kilo_roo_modes,
            get_app_settings,
            update_app_settings,
            list_mode_history,
            replay_mode_history,
            list_sync_logs,
            clear_sync_logs,
            export_backup,
            import_backup,
            get_github_settings,
            update_github_settings,
            test_github_token_command,
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
fn delete_mode(slug: String, state: State<AppDatabase>) -> Result<(), String> {
    state.delete_mode(&slug).map_err(|err| err.to_string())
}

#[tauri::command]
fn get_mode_meta(slug: String, state: State<AppDatabase>) -> Result<ModeMetaRecord, String> {
    state.get_mode_meta(&slug).map_err(|err| err.to_string())
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
fn delete_github_rule(rule_id: String, state: State<AppDatabase>) -> Result<(), String> {
    state
        .delete_github_rule(&rule_id)
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
fn delete_ide_instance(instance_id: String, state: State<AppDatabase>) -> Result<(), String> {
    state
        .delete_ide_instance(&instance_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn scan_known_instances(state: State<AppDatabase>) -> Result<Vec<IdeInstanceRecord>, String> {
    state
        .sync_known_instances()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn scan_all_instances(state: State<AppDatabase>) -> Result<Vec<IdeInstanceRecord>, String> {
    state.sync_all_instances().map_err(|err| err.to_string())
}

#[tauri::command]
fn scan_instance_modes(instance_id: String, state: State<AppDatabase>) -> Result<IdeInstanceRecord, String> {
    state
        .sync_instance_modes(&instance_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn diff_instance_modes(instance_id: String, state: State<AppDatabase>) -> Result<InstanceModeDiffSummary, String> {
    state.log_event("info", "diff_instance_modes", "开始计算实例差异", Some(json!({ "instanceId": instance_id })));
    state
        .diff_instance_modes(&instance_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_instance_modes(instance_id: String, state: State<AppDatabase>) -> Result<Vec<InstanceModeItem>, String> {
    state
        .list_instance_modes(&instance_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_instance_mode_raw(
    instance_id: String,
    slug: String,
    state: State<AppDatabase>,
) -> Result<Option<serde_json::Value>, String> {
    state
        .get_instance_mode_raw(&instance_id, &slug)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn upsert_instance_mode(
    instance_id: String,
    mode: serde_json::Value,
    conflict_strategy: String,
    save_to_db: bool,
    state: State<AppDatabase>,
) -> Result<InstanceModeUpsertResult, String> {
    state.log_event(
        "info",
        "upsert_instance_mode",
        "写回实例模式",
        Some(json!({ "instanceId": instance_id, "conflictStrategy": conflict_strategy, "saveToDb": save_to_db })),
    );
    state
        .upsert_instance_mode(&instance_id, mode, conflict_strategy, save_to_db)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn delete_instance_mode(instance_id: String, slug: String, state: State<AppDatabase>) -> Result<(), String> {
    state.log_event(
        "info",
        "delete_instance_mode",
        "删除实例模式",
        Some(json!({ "instanceId": instance_id, "slug": slug })),
    );
    state
        .delete_instance_mode(&instance_id, &slug)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn preview_mode_diff(text: String, state: State<AppDatabase>) -> Result<ModeDiffPreview, String> {
    state.log_event("info", "preview_mode_diff", "预览导入差异", Some(json!({ "textBytes": text.len() })));
    state.preview_mode_diff(&text).map_err(|err| err.to_string())
}

#[tauri::command]
fn import_modes_from_text(
    text: String,
    conflict_strategy: Option<String>,
    state: State<AppDatabase>,
) -> Result<ModeImportReport, String> {
    state.log_event("info", "import_modes_from_text", "批量导入文本到本地库", Some(json!({ "textBytes": text.len() })));
    let strategy = conflict_strategy.unwrap_or_else(|| "overwrite".to_string());
    state
        .import_modes_from_text_with_strategy(&text, &strategy)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn import_instance_modes_to_db(
    instance_id: String,
    mode_slugs: Option<Vec<String>>,
    conflict_strategy: String,
    state: State<AppDatabase>,
) -> Result<ModeImportReport, String> {
    state.log_event(
        "info",
        "import_instance_modes_to_db",
        "从实例导入模式到本地库",
        Some(json!({ "instanceId": instance_id, "modeSlugsCount": mode_slugs.as_ref().map(|v| v.len()).unwrap_or(0), "conflictStrategy": conflict_strategy })),
    );
    state
        .import_instance_modes_to_db(&instance_id, mode_slugs, &conflict_strategy)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn apply_modes_to_instances(
    mode_slugs: Vec<String>,
    instance_ids: Vec<String>,
    conflict_strategy: String,
    state: State<AppDatabase>,
) -> Result<ApplyModesResult, String> {
    state.log_event(
        "info",
        "apply_modes_to_instances",
        "写回模式到实例",
        Some(json!({ "modeSlugsCount": mode_slugs.len(), "instanceIdsCount": instance_ids.len(), "conflictStrategy": conflict_strategy })),
    );
    state
        .apply_modes_to_instances(mode_slugs, instance_ids, conflict_strategy)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn compare_kilo_roo_modes(state: State<AppDatabase>) -> Result<Vec<ModeCompareItem>, String> {
    state.compare_kilo_roo_modes().map_err(|err| err.to_string())
}

#[tauri::command]
fn get_app_settings(state: State<AppDatabase>) -> Result<AppSettings, String> {
    state.get_app_settings().map_err(|err| err.to_string())
}

#[tauri::command]
fn update_app_settings(payload: AppSettings, state: State<AppDatabase>) -> Result<AppSettings, String> {
    state.update_app_settings(payload).map_err(|err| err.to_string())
}

#[tauri::command]
fn list_mode_history(
    instance_id: Option<String>,
    limit: Option<u64>,
    offset: Option<u64>,
    state: State<AppDatabase>,
) -> Result<Vec<ModeHistoryRecord>, String> {
    let limit = limit.unwrap_or(50).min(200) as usize;
    let offset = offset.unwrap_or(0) as usize;
    state
        .list_mode_history(instance_id.as_deref(), limit, offset)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn replay_mode_history(
    history_id: String,
    conflict_strategy: String,
    save_to_db: bool,
    state: State<AppDatabase>,
) -> Result<ModeHistoryReplayResult, String> {
    state
        .replay_mode_history(&history_id, conflict_strategy, save_to_db)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_sync_logs(limit: Option<u64>, offset: Option<u64>, state: State<AppDatabase>) -> Result<Vec<SyncLogRecord>, String> {
    let limit = limit.unwrap_or(50).min(200) as usize;
    let offset = offset.unwrap_or(0) as usize;
    state
        .list_sync_logs(limit, offset)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn clear_sync_logs(state: State<AppDatabase>) -> Result<(), String> {
    state.clear_sync_logs().map_err(|err| err.to_string())
}

#[tauri::command]
fn export_backup(options: Option<BackupOptions>, state: State<AppDatabase>) -> Result<BackupPayload, String> {
    state.log_event("info", "export_backup", "导出备份", None);
    state
        .export_backup(options.unwrap_or_default())
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn import_backup(payload: BackupPayload, state: State<AppDatabase>) -> Result<BackupImportResult, String> {
    state.log_event("info", "import_backup", "导入备份", None);
    state.import_backup(payload).map_err(|err| err.to_string())
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

impl GithubSettingsState {
    fn load_from_db(db: &AppDatabase) -> Self {
        match db.get_setting("github_settings") {
            Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
            Ok(None) | Err(_) => Self::default(),
        }
    }

    fn persist_to_db(&self, db: &AppDatabase) -> Result<(), DbError> {
        let json = serde_json::to_string(self)?;
        db.set_setting("github_settings", &json)
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
    db: State<'_, AppDatabase>,
) -> Result<(), String> {
    let mut guard = state
        .lock()
        .map_err(|_| "写入 GitHub 设置失败".to_string())?;
    guard.token = token;
    guard.proxy = proxy;
    guard.delay_sec = delay_sec.max(1);
    guard.persist_to_db(&db).map_err(|err| err.to_string())
}

#[tauri::command]
async fn test_github_token_command(
    settings_state: State<'_, Mutex<GithubSettingsState>>,
) -> Result<GithubTokenTestResult, String> {
    let (token, proxy) = {
        let guard = settings_state
            .lock()
            .map_err(|_| "读取 GitHub 设置失败".to_string())?;
        (guard.token.clone(), guard.proxy.clone())
    };
    test_github_token(token, proxy)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn sync_github_modes(
    query: String,
    path_hint: String,
    rule_id: Option<String>,
    rule_name: Option<String>,
    delay_sec: Option<u64>,
    branch: Option<String>,
    state: State<'_, AppDatabase>,
    settings_state: State<'_, Mutex<GithubSettingsState>>,
) -> Result<GithubSyncResult, String> {
    let (token, delay_sec_default, proxy) = {
        let guard = settings_state
            .lock()
            .map_err(|_| "读取 GitHub 设置失败".to_string())?;
        (guard.token.clone(), guard.delay_sec, guard.proxy.clone())
    };
    let config = GithubSyncConfig {
        token,
        query,
        path_hint,
        delay_sec: delay_sec.unwrap_or(delay_sec_default),
        proxy,
        rule_id,
        rule_name,
        branch: branch
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "main".to_string()),
    };
    state.log_event(
        "info",
        "sync_github_modes",
        "开始 GitHub 同步",
        Some(json!({ "query": config.query, "pathHint": config.path_hint, "ruleId": config.rule_id, "ruleName": config.rule_name, "delaySec": config.delay_sec, "branch": config.branch })),
    );
    let result = sync_from_github(config, &state)
        .await
        .map_err(|err| err.to_string())?;
    state.log_event(
        "info",
        "sync_github_modes",
        "GitHub 同步完成",
        Some(json!({ "fetchedFiles": result.fetched_files, "savedModes": result.saved_modes, "skippedDueToMissingFields": result.skipped_due_to_missing_fields, "errors": result.errors.len() })),
    );
    if let Ok(mut guard) = settings_state.lock() {
        guard.last_result = Some(result.clone());
        if let Err(err) = guard.persist_to_db(&state) {
            eprintln!("Failed to persist github settings: {}", err);
        }
    }
    Ok(result)
}
