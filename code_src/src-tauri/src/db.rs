use chrono::Utc;
use dirs_next::home_dir;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use serde_yaml::{Mapping as YamlMapping, Value as YamlValue};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{fs, sync::Mutex};
use tauri::{AppHandle, Manager};
use thiserror::Error;
use uuid::Uuid;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FileLogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
}

impl FileLogLevel {
    fn from_str(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "debug" => Self::Debug,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
}

#[derive(Debug)]
struct FileLogger {
    enabled: bool,
    level: FileLogLevel,
    retention_days: i64,
    logs_dir: PathBuf,
    last_cleanup_at: Option<chrono::DateTime<Utc>>,
}

impl FileLogger {
    fn new(logs_dir: PathBuf, settings: &AppSettings) -> Self {
        let enabled = settings.enable_log;
        let level = FileLogLevel::from_str(&settings.log_level);
        let retention_days = settings.retention_days.max(1);
        let logger = Self {
            enabled,
            level,
            retention_days,
            logs_dir,
            last_cleanup_at: None,
        };
        let _ = logger.ensure_logs_dir();
        let _ = logger.cleanup_old_files();
        logger
    }

    fn update_settings(&mut self, settings: &AppSettings) {
        self.enabled = settings.enable_log;
        self.level = FileLogLevel::from_str(&settings.log_level);
        self.retention_days = settings.retention_days.max(1);
        let _ = self.ensure_logs_dir();
        let _ = self.cleanup_old_files();
    }

    fn ensure_logs_dir(&self) -> Result<(), DbError> {
        fs::create_dir_all(&self.logs_dir)?;
        Ok(())
    }

    fn current_log_file(&self, now: chrono::DateTime<Utc>) -> PathBuf {
        let filename = format!("{}.log", now.format("%Y-%m-%d_%H-%M"));
        self.logs_dir.join(filename)
    }

    fn cleanup_old_files(&self) -> Result<(), DbError> {
        let now = std::time::SystemTime::now();
        let keep_seconds = (self.retention_days.max(1) as u64) * 24 * 60 * 60;
        let cutoff = now
            .checked_sub(std::time::Duration::from_secs(keep_seconds))
            .unwrap_or(now);
        let entries = match fs::read_dir(&self.logs_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(()),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("log") {
                continue;
            }
            let modified = match entry.metadata().and_then(|m| m.modified()) {
                Ok(modified) => modified,
                Err(_) => continue,
            };
            if modified < cutoff {
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    }

    fn maybe_cleanup(&mut self) {
        let now = Utc::now();
        let should = match self.last_cleanup_at {
            None => true,
            Some(last) => (now - last).num_hours() >= 6,
        };
        if should {
            let _ = self.cleanup_old_files();
            self.last_cleanup_at = Some(now);
        }
    }

    fn log(&mut self, level: FileLogLevel, event: &str, message: &str, fields: Option<Value>) -> Result<(), DbError> {
        if !self.enabled || level > self.level {
            return Ok(());
        }
        self.maybe_cleanup();
        let now = Utc::now();
        let record = json!({
            "ts": now.to_rfc3339(),
            "level": level.as_str(),
            "event": event,
            "message": message,
            "fields": fields.unwrap_or(Value::Null),
        });
        let file_path = self.current_log_file(now);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;
        use std::io::Write;
        writeln!(file, "{}", record.to_string())?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("无法获取应用数据目录")]
    ResolvePath,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
}

#[derive(Debug)]
pub struct AppDatabase {
    conn: Mutex<Connection>,
    logger: Mutex<FileLogger>,
}

#[derive(Default)]
struct ModeImportResult {
    discovered: usize,
    saved: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModeImportReport {
    pub discovered: usize,
    pub saved: usize,
    pub skipped_due_to_missing_fields: usize,
    pub duplicate_slug: usize,
    pub duplicate_hash: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModeDiffPreviewItem {
    pub slug: String,
    pub name: String,
    pub content_hash: String,
    pub status: String,
    pub recommended_action: String,
    pub existing_slug: Option<String>,
    pub existing_hash: Option<String>,
    pub rename_suggestion: Option<String>,
    pub missing_fields: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModeDiffPreview {
    pub discovered: usize,
    pub new_modes: usize,
    pub duplicates: usize,
    pub conflicts: usize,
    pub invalid: usize,
    pub items: Vec<ModeDiffPreviewItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApplyModesResult {
    pub total_instances: usize,
    pub updated_instances: usize,
    pub skipped_instances: usize,
    pub errors: Vec<String>,
    pub details: Vec<ApplyInstanceResult>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApplyInstanceResult {
    pub instance_id: String,
    pub alias: String,
    pub path: String,
    pub applied: usize,
    pub overwritten: usize,
    pub renamed: usize,
    pub skipped: usize,
    pub status: String,
    pub messages: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModeCompareItem {
    pub slug: String,
    pub in_kilocode: bool,
    pub in_roocode: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModeMetaRecord {
    pub raw_payload: Option<Value>,
    pub source_path: Option<String>,
    pub source_alias: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstanceModeDiffOnlyItem {
    pub slug: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstanceModeDiffConflictItem {
    pub slug: String,
    pub name: Option<String>,
    pub db_hash: String,
    pub ide_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstanceModeDiffInvalidItem {
    pub slug: Option<String>,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstanceModeDiffSummary {
    pub instance_id: String,
    pub alias: String,
    pub kind: String,
    pub path: String,
    pub file_exists: bool,
    pub status: String,
    pub total_db: usize,
    pub total_ide: usize,
    pub same: usize,
    pub conflicts: Vec<InstanceModeDiffConflictItem>,
    pub ide_only: Vec<InstanceModeDiffOnlyItem>,
    pub invalid: Vec<InstanceModeDiffInvalidItem>,
    pub db_only_total: usize,
    pub db_only_sample: Vec<InstanceModeDiffOnlyItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub enable_log: bool,
    pub log_level: String,
    pub retention_days: i64,
    pub show_role_definition_length: bool,
    pub quality_threshold: i64,
    pub auto_deduplicate: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            enable_log: true,
            log_level: "info".to_string(),
            retention_days: 30,
            show_role_definition_length: true,
            quality_threshold: 800,
            auto_deduplicate: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncLogRecord {
    pub id: String,
    pub sync_kind: String,
    pub rule_id: Option<String>,
    pub rule_name: Option<String>,
    pub target: Option<String>,
    pub status: String,
    pub message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupOptions {
    pub include_modes: bool,
    pub include_rules: bool,
    pub include_instances: bool,
    pub include_settings: bool,
}

impl Default for BackupOptions {
    fn default() -> Self {
        Self {
            include_modes: true,
            include_rules: true,
            include_instances: true,
            include_settings: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupModeRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub groups: Vec<String>,
    pub role_definition: String,
    pub role_definition_length: i64,
    pub source: String,
    pub when_to_use: Option<String>,
    pub custom_instructions: Option<String>,
    pub payload: Option<Value>,
    pub raw_payload: Option<Value>,
    pub source_path: Option<String>,
    pub source_alias: Option<String>,
    pub updated_at: String,
    pub content_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupPayload {
    pub version: i64,
    pub exported_at: String,
    pub options: BackupOptions,
    pub modes: Vec<BackupModeRecord>,
    pub github_rules: Vec<GithubRuleRecord>,
    pub ide_instances: Vec<IdeInstanceRecord>,
    pub github_settings_json: Option<String>,
    pub app_settings_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupImportResult {
    pub imported_modes: usize,
    pub skipped_duplicate_modes: usize,
    pub imported_rules: usize,
    pub imported_instances: usize,
    pub updated_settings: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModeHistoryRecord {
    pub id: String,
    pub mode_id: Option<String>,
    pub instance_id: Option<String>,
    pub instance_alias: Option<String>,
    pub action: String,
    pub before_payload: Option<Value>,
    pub after_payload: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModeHistoryReplayResult {
    pub history_id: String,
    pub instance_id: String,
    pub result: InstanceModeUpsertResult,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstanceModeItem {
    pub slug: String,
    pub name: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstanceModeUpsertResult {
    pub requested_slug: String,
    pub final_slug: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModeRecord {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub groups: Vec<String>,
    pub role_definition: String,
    pub role_definition_length: i64,
    pub source: String,
    pub when_to_use: Option<String>,
    pub custom_instructions: Option<String>,
    pub payload: Option<Value>,
    pub updated_at: String,
    pub hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModeUpsertCandidate {
    record: ModeRecord,
    raw_payload: Option<Value>,
    source_path: Option<String>,
    source_alias: Option<String>,
    content_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GithubRuleRecord {
    pub id: String,
    pub name: String,
    pub query: String,
    pub path_hint: String,
    pub branch: String,
    pub enabled: bool,
    pub delay_sec: i64,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IdeInstanceRecord {
    pub id: String,
    pub alias: String,
    pub kind: String,
    pub path: String,
    pub last_scan_at: Option<String>,
    pub modes_count: i64,
    pub status: String,
    pub selected: bool,
}

impl AppDatabase {
    pub fn new(handle: &AppHandle) -> Result<Self, DbError> {
        let dir = handle
            .path()
            .app_data_dir()
            .map_err(|_| DbError::ResolvePath)?;
        fs::create_dir_all(&dir)?;
        let db_path = dir.join("kilo_modes.db");
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "foreign_keys", &"ON")?;
        Self::run_migrations(&conn)?;
        Self::seed_if_empty(&conn)?;
        let settings = Self::load_app_settings_from_conn(&conn).unwrap_or_default();
        let logger = Mutex::new(FileLogger::new(dir.join("logs"), &settings));
        Ok(Self {
            conn: Mutex::new(conn),
            logger,
        })
    }

    fn load_app_settings_from_conn(conn: &Connection) -> Result<AppSettings, DbError> {
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let raw = stmt
            .query_row(["app_settings"], |row| row.get::<_, String>(0))
            .optional()?;
        Ok(match raw {
            Some(value) => serde_json::from_str(&value).unwrap_or_default(),
            None => AppSettings::default(),
        })
    }

    pub fn log_event(&self, level: &str, event: &str, message: &str, fields: Option<Value>) {
        let level = FileLogLevel::from_str(level);
        let mut logger = match self.logger.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let _ = logger.log(level, event, message, fields);
    }

    fn run_migrations(conn: &Connection) -> Result<(), DbError> {
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS modes (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    groups TEXT NOT NULL,
    role_definition TEXT NOT NULL,
    role_definition_length INTEGER NOT NULL,
    source TEXT NOT NULL,
    when_to_use TEXT,
    custom_instructions TEXT,
    payload TEXT,
    raw_payload TEXT,
    source_path TEXT,
    source_alias TEXT,
    updated_at TEXT NOT NULL,
    hash TEXT NOT NULL,
    content_hash TEXT
);

CREATE TABLE IF NOT EXISTS github_rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    query TEXT NOT NULL,
    path_hint TEXT NOT NULL,
    branch TEXT NOT NULL DEFAULT 'main',
    enabled INTEGER NOT NULL DEFAULT 1,
    delay_sec INTEGER NOT NULL DEFAULT 3,
    last_run_at TEXT
);

CREATE TABLE IF NOT EXISTS ide_instances (
    id TEXT PRIMARY KEY,
    alias TEXT NOT NULL,
    kind TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    last_scan_at TEXT,
    modes_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    selected_for_sync INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_ide_instances_path ON ide_instances(path);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS mode_history (
    id TEXT PRIMARY KEY,
    mode_id TEXT,
    instance_id TEXT,
    action TEXT NOT NULL,
    before_payload TEXT,
    after_payload TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_logs (
    id TEXT PRIMARY KEY,
    sync_kind TEXT NOT NULL,
    rule_id TEXT,
    rule_name TEXT,
    target TEXT,
    status TEXT NOT NULL,
    message TEXT,
    created_at TEXT NOT NULL
);
"#,
        )?;
        Self::ensure_schema(conn)?;
        Ok(())
    }

    fn ensure_schema(conn: &Connection) -> Result<(), DbError> {
        Self::ensure_column(conn, "modes", "raw_payload", "TEXT")?;
        Self::ensure_column(conn, "modes", "source_path", "TEXT")?;
        Self::ensure_column(conn, "modes", "source_alias", "TEXT")?;
        Self::ensure_column(conn, "modes", "content_hash", "TEXT")?;
        Self::ensure_column(conn, "github_rules", "branch", "TEXT")?;

        Self::normalize_existing_sources(conn)?;
        Self::backfill_content_hash(conn)?;

        conn.execute_batch(
            r#"
CREATE UNIQUE INDEX IF NOT EXISTS idx_modes_content_hash ON modes(content_hash);
CREATE INDEX IF NOT EXISTS idx_modes_slug ON modes(slug);
CREATE INDEX IF NOT EXISTS idx_modes_updated_at ON modes(updated_at);

CREATE INDEX IF NOT EXISTS idx_sync_logs_created_at ON sync_logs(created_at);
"#,
        )?;
        Ok(())
    }

    fn ensure_column(conn: &Connection, table: &str, column: &str, ty: &str) -> Result<(), DbError> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let existing = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        if existing.iter().any(|name| name == column) {
            return Ok(());
        }
        conn.execute(
            &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, ty),
            [],
        )?;
        Ok(())
    }

    fn normalize_existing_sources(conn: &Connection) -> Result<(), DbError> {
        let mut stmt = conn.prepare("SELECT id, source FROM modes")?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, source) = row?;
            if source == "local" || source == "github" || source == "ide" {
                continue;
            }
            let mut normalized = "local".to_string();
            let mut alias: Option<String> = None;
            if let Some(rest) = source.strip_prefix("github:") {
                normalized = "github".to_string();
                alias = Some(rest.to_string());
            } else if let Some(rest) = source.strip_prefix("ide:") {
                normalized = "ide".to_string();
                alias = Some(rest.to_string());
            } else if source.starts_with("github") {
                normalized = "github".to_string();
            } else if source.starts_with("ide") {
                normalized = "ide".to_string();
            }

            conn.execute(
                "UPDATE modes SET source = ?1, source_alias = COALESCE(source_alias, ?2) WHERE id = ?3",
                params![normalized, alias, id],
            )?;
        }
        Ok(())
    }

    fn compute_content_hash_from_fields(
        description: &str,
        role_definition: &str,
        groups: &[String],
        when_to_use: Option<&String>,
        custom_instructions: Option<&String>,
    ) -> String {
        let mut groups_sorted = groups.to_vec();
        groups_sorted.sort();
        let when_to_use_value = when_to_use.map(|s| s.as_str()).unwrap_or("");
        let custom_instructions_value = custom_instructions.map(|s| s.as_str()).unwrap_or("");
        let fingerprint = json!({
            "description": description,
            "groups": groups_sorted,
            "roleDefinition": role_definition,
            "whenToUse": when_to_use_value,
            "customInstructions": custom_instructions_value,
        });
        let mut hasher = Sha256::new();
        hasher.update(fingerprint.to_string().as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }

    fn backfill_content_hash(conn: &Connection) -> Result<(), DbError> {
        let mut stmt = conn.prepare(
            "SELECT id, description, groups, role_definition, when_to_use, custom_instructions, hash, content_hash FROM modes",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?;

        let mut by_hash: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for row in rows {
            let (id, description, groups_json, role_definition, when_to_use, custom_instructions, legacy_hash, content_hash) =
                row?;
            if content_hash.is_some() && !content_hash.as_deref().unwrap_or_default().is_empty() {
                continue;
            }
            let groups: Vec<String> = serde_json::from_str(&groups_json).unwrap_or_default();
            let computed = Self::compute_content_hash_from_fields(
                &description,
                &role_definition,
                &groups,
                when_to_use.as_ref(),
                custom_instructions.as_ref(),
            );
            let final_hash = if computed.is_empty() { legacy_hash } else { computed };
            conn.execute(
                "UPDATE modes SET content_hash = ?1 WHERE id = ?2",
                params![final_hash, id],
            )?;
            by_hash.entry(final_hash).or_default().push(id);
        }

        // 去重：若存在相同 content_hash 多条记录，保留 updated_at 最新的一条
        let mut dupes: Vec<String> = Vec::new();
        for (hash, ids) in by_hash {
            if ids.len() <= 1 {
                continue;
            }
            let mut stmt = conn.prepare(
                "SELECT id FROM modes WHERE content_hash = ?1 ORDER BY datetime(updated_at) DESC",
            )?;
            let mut rows = stmt.query([hash])?;
            let mut keep: Option<String> = None;
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                if keep.is_none() {
                    keep = Some(id);
                } else {
                    dupes.push(id);
                }
            }
        }
        for id in dupes {
            conn.execute("DELETE FROM modes WHERE id = ?1", [id])?;
        }
        Ok(())
    }

    fn seed_if_empty(conn: &Connection) -> Result<(), DbError> {
        let rule_count: i64 = conn.query_row("SELECT COUNT(*) FROM github_rules", [], |row| row.get(0))?;
        if rule_count == 0 {
            conn.execute(
                "INSERT INTO github_rules (id, name, query, path_hint, branch, enabled, delay_sec, last_run_at)
                 VALUES (?1, ?2, ?3, ?4, 'main', 1, 3, NULL)",
                params![
                    Uuid::new_v4().to_string(),
                    "示例规则：检索 custom_modes.yaml",
                    "filename:custom_modes.yaml customModes in:file",
                    "customModes[].slug",
                ],
            )?;
        }

        Ok(())
    }

    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("database poisoned")
    }

    pub fn list_modes(&self) -> Result<Vec<ModeRecord>, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, slug, name, description, groups, role_definition, role_definition_length, source, when_to_use, custom_instructions, payload, updated_at, COALESCE(content_hash, hash) AS hash
             FROM modes ORDER BY datetime(updated_at) DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let groups: String = row.get("groups")?;
            let payload: Option<String> = row.get("payload")?;
            Ok(ModeRecord {
                id: row.get("id")?,
                slug: row.get("slug")?,
                name: row.get("name")?,
                description: row.get("description")?,
                groups: serde_json::from_str(&groups).unwrap_or_default(),
                role_definition: row.get("role_definition")?,
                role_definition_length: row.get("role_definition_length")?,
                source: row.get("source")?,
                when_to_use: row.get("when_to_use")?,
                custom_instructions: row.get("custom_instructions")?,
                payload: match payload {
                    Some(raw) => Some(serde_json::from_str(&raw).unwrap_or(Value::Null)),
                    None => None,
                },
                updated_at: row.get("updated_at")?,
                hash: row.get("hash")?,
            })
        })?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    pub fn delete_mode(&self, slug: &str) -> Result<(), DbError> {
        let conn = self.lock_conn();
        conn.execute("DELETE FROM modes WHERE slug = ?1", params![slug])?;
        Ok(())
    }

    pub fn get_mode_meta(&self, slug: &str) -> Result<ModeMetaRecord, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT raw_payload, source_path, source_alias FROM modes WHERE slug = ?1 LIMIT 1",
        )?;
        let row = stmt
            .query_row([slug], |row| {
                let raw_payload: Option<String> = row.get("raw_payload")?;
                Ok(ModeMetaRecord {
                    raw_payload: raw_payload.and_then(|raw| serde_json::from_str(&raw).ok()),
                    source_path: row.get("source_path")?,
                    source_alias: row.get("source_alias")?,
                })
            })
            .optional()?;
        Ok(row.unwrap_or(ModeMetaRecord {
            raw_payload: None,
            source_path: None,
            source_alias: None,
        }))
    }

    pub fn upsert_mode(&self, mut record: ModeRecord) -> Result<ModeRecord, DbError> {
        if record.id.trim().is_empty() {
            record.id = Uuid::new_v4().to_string();
        }
        if record.updated_at.trim().is_empty() {
            record.updated_at = Utc::now().to_rfc3339();
        }
        if record.role_definition_length <= 0 {
            record.role_definition_length = record.role_definition.chars().count() as i64;
        }
        if record.hash.trim().is_empty() {
            record.hash = Self::compute_content_hash_from_fields(
                &record.description,
                &record.role_definition,
                &record.groups,
                record.when_to_use.as_ref(),
                record.custom_instructions.as_ref(),
            );
        }
        let conn = self.lock_conn();
        let groups_json = serde_json::to_string(&record.groups)?;
        let payload_json = record
            .payload
            .as_ref()
            .map(|value| serde_json::to_string(value))
            .transpose()?;
        let existing_raw_payload = conn
            .query_row(
                "SELECT raw_payload FROM modes WHERE slug = ?1 LIMIT 1",
                [record.slug.as_str()],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        let merged_raw_payload_json = existing_raw_payload
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|val| {
                if !val.is_object() {
                    return None;
                }
                let config_source = payload_json
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                    .and_then(|val| val.get("configSource").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    .unwrap_or_else(|| "global".to_string());
                let mut obj = match val {
                    Value::Object(map) => map,
                    _ => return None,
                };
                obj.insert("slug".into(), Value::String(record.slug.clone()));
                obj.insert("name".into(), Value::String(record.name.clone()));
                obj.insert("description".into(), Value::String(record.description.clone()));
                obj.insert("groups".into(), serde_json::to_value(record.groups.clone()).ok()?);
                obj.insert("roleDefinition".into(), Value::String(record.role_definition.clone()));
                obj.insert("source".into(), Value::String(config_source));
                match record.when_to_use.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    Some(value) => {
                        obj.insert("whenToUse".into(), Value::String(value.to_string()));
                    }
                    None => {
                        obj.remove("whenToUse");
                    }
                }
                match record.custom_instructions.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    Some(value) => {
                        obj.insert("customInstructions".into(), Value::String(value.to_string()));
                    }
                    None => {
                        obj.remove("customInstructions");
                    }
                }
                Some(Value::Object(obj))
            })
            .map(|value| serde_json::to_string(&value))
            .transpose()?;

        let inserted = conn.execute(
            "INSERT INTO modes (
                id, slug, name, description, groups, role_definition, role_definition_length,
                source, when_to_use, custom_instructions, payload, raw_payload, updated_at, hash, content_hash
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(slug) DO UPDATE SET
                name=excluded.name,
                description=excluded.description,
                groups=excluded.groups,
                role_definition=excluded.role_definition,
                role_definition_length=excluded.role_definition_length,
                source=excluded.source,
                when_to_use=excluded.when_to_use,
                custom_instructions=excluded.custom_instructions,
                payload=excluded.payload,
                raw_payload=COALESCE(excluded.raw_payload, modes.raw_payload),
                updated_at=excluded.updated_at,
                hash=excluded.hash,
                content_hash=excluded.content_hash",
            params![
                record.id,
                record.slug,
                record.name,
                record.description,
                groups_json,
                record.role_definition,
                record.role_definition_length,
                record.source,
                record.when_to_use,
                record.custom_instructions,
                payload_json,
                merged_raw_payload_json,
                record.updated_at,
                record.hash,
                record.hash
            ],
        );
        if let Err(err) = inserted {
            if Self::is_unique_content_hash_conflict(&err) {
                if let Some(existing) = Self::find_mode_by_content_hash(&conn, &record.hash)? {
                    return Ok(existing);
                }
            }
            return Err(err.into());
        }
        Ok(record)
    }

    fn is_unique_content_hash_conflict(err: &rusqlite::Error) -> bool {
        match err {
            rusqlite::Error::SqliteFailure(inner, _) => inner.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE,
            _ => false,
        }
    }

    fn find_mode_by_content_hash(conn: &Connection, content_hash: &str) -> Result<Option<ModeRecord>, DbError> {
        let mut stmt = conn.prepare(
            "SELECT id, slug, name, description, groups, role_definition, role_definition_length, source, when_to_use, custom_instructions, payload, updated_at, COALESCE(content_hash, hash) AS hash
             FROM modes WHERE content_hash = ?1 LIMIT 1",
        )?;
        let row = stmt
            .query_row([content_hash], |row| {
                let groups: String = row.get("groups")?;
                let payload: Option<String> = row.get("payload")?;
                Ok(ModeRecord {
                    id: row.get("id")?,
                    slug: row.get("slug")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    groups: serde_json::from_str(&groups).unwrap_or_default(),
                    role_definition: row.get("role_definition")?,
                    role_definition_length: row.get("role_definition_length")?,
                    source: row.get("source")?,
                    when_to_use: row.get("when_to_use")?,
                    custom_instructions: row.get("custom_instructions")?,
                    payload: match payload {
                        Some(raw) => Some(serde_json::from_str(&raw).unwrap_or(Value::Null)),
                        None => None,
                    },
                    updated_at: row.get("updated_at")?,
                    hash: row.get("hash")?,
                })
            })
            .optional()?;
        Ok(row)
    }

    fn upsert_mode_candidate(&self, candidate: ModeUpsertCandidate) -> Result<ModeRecord, DbError> {
        let mut record = candidate.record;
        let conn = self.lock_conn();
        let groups_json = serde_json::to_string(&record.groups)?;
        let payload_json = record
            .payload
            .as_ref()
            .map(|value| serde_json::to_string(value))
            .transpose()?;
        let raw_payload_json = candidate
            .raw_payload
            .as_ref()
            .map(|value| serde_json::to_string(value))
            .transpose()?;

        let inserted = conn.execute(
            "INSERT INTO modes (
                id, slug, name, description, groups, role_definition, role_definition_length,
                source, when_to_use, custom_instructions, payload, raw_payload, source_path, source_alias,
                updated_at, hash, content_hash
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT(slug) DO UPDATE SET
                name=excluded.name,
                description=excluded.description,
                groups=excluded.groups,
                role_definition=excluded.role_definition,
                role_definition_length=excluded.role_definition_length,
                source=excluded.source,
                when_to_use=excluded.when_to_use,
                custom_instructions=excluded.custom_instructions,
                payload=excluded.payload,
                raw_payload=COALESCE(excluded.raw_payload, modes.raw_payload),
                source_path=COALESCE(excluded.source_path, modes.source_path),
                source_alias=COALESCE(excluded.source_alias, modes.source_alias),
                updated_at=excluded.updated_at,
                hash=excluded.hash,
                content_hash=excluded.content_hash",
            params![
                record.id,
                record.slug,
                record.name,
                record.description,
                groups_json,
                record.role_definition,
                record.role_definition_length,
                record.source,
                record.when_to_use,
                record.custom_instructions,
                payload_json,
                raw_payload_json,
                candidate.source_path,
                candidate.source_alias,
                record.updated_at,
                record.hash,
                candidate.content_hash
            ],
        );
        if let Err(err) = inserted {
            if Self::is_unique_content_hash_conflict(&err) {
                if let Some(existing) = Self::find_mode_by_content_hash(&conn, &candidate.content_hash)? {
                    return Ok(existing);
                }
            }
            return Err(err.into());
        }
        record.hash = candidate.content_hash;
        Ok(record)
    }

    pub fn upsert_mode_with_meta(
        &self,
        mut record: ModeRecord,
        raw_payload: Option<Value>,
        source_path: Option<String>,
        source_alias: Option<String>,
    ) -> Result<ModeRecord, DbError> {
        if record.id.trim().is_empty() {
            record.id = Uuid::new_v4().to_string();
        }
        if record.updated_at.trim().is_empty() {
            record.updated_at = Utc::now().to_rfc3339();
        }
        if record.role_definition_length <= 0 {
            record.role_definition_length = record.role_definition.chars().count() as i64;
        }
        let content_hash = if record.hash.trim().is_empty() {
            Self::compute_content_hash_from_fields(
                &record.description,
                &record.role_definition,
                &record.groups,
                record.when_to_use.as_ref(),
                record.custom_instructions.as_ref(),
            )
        } else {
            record.hash.clone()
        };
        record.hash = content_hash.clone();
        self.upsert_mode_candidate(ModeUpsertCandidate {
            record,
            raw_payload,
            source_path,
            source_alias,
            content_hash,
        })
    }

    pub fn add_sync_log(
        &self,
        sync_kind: &str,
        rule_id: Option<&str>,
        rule_name: Option<&str>,
        target: Option<&str>,
        status: &str,
        message: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.lock_conn();
        let enable_log = Self::get_setting_with_conn(&conn, "app_settings")?
            .and_then(|raw| serde_json::from_str::<AppSettings>(&raw).ok())
            .map(|settings| settings.enable_log)
            .unwrap_or(true);
        if !enable_log {
            return Ok(());
        }
        conn.execute(
            "INSERT INTO sync_logs (id, sync_kind, rule_id, rule_name, target, status, message, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Uuid::new_v4().to_string(),
                sync_kind,
                rule_id,
                rule_name,
                target,
                status,
                message,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn list_mode_history(
        &self,
        instance_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ModeHistoryRecord>, DbError> {
        let conn = self.lock_conn();
        let mut items = Vec::new();
        if let Some(instance_id) = instance_id {
            let mut stmt = conn.prepare(
                "SELECT h.id, h.mode_id, h.instance_id, i.alias AS instance_alias, h.action, h.before_payload, h.after_payload, h.created_at
                 FROM mode_history h
                 LEFT JOIN ide_instances i ON i.id = h.instance_id
                 WHERE h.instance_id = ?1
                 ORDER BY datetime(h.created_at) DESC
                 LIMIT ?2 OFFSET ?3",
            )?;
            let rows =
                stmt.query_map(params![instance_id, limit as i64, offset as i64], mode_history_from_row)?;
            for row in rows {
                items.push(row?);
            }
            return Ok(items);
        }

        let mut stmt = conn.prepare(
            "SELECT h.id, h.mode_id, h.instance_id, i.alias AS instance_alias, h.action, h.before_payload, h.after_payload, h.created_at
             FROM mode_history h
             LEFT JOIN ide_instances i ON i.id = h.instance_id
             ORDER BY datetime(h.created_at) DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], mode_history_from_row)?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    pub fn replay_mode_history(
        &self,
        history_id: &str,
        conflict_strategy: String,
        save_to_db: bool,
    ) -> Result<ModeHistoryReplayResult, DbError> {
        let row = {
            let conn = self.lock_conn();
            let mut stmt = conn.prepare(
                "SELECT id, instance_id, after_payload FROM mode_history WHERE id = ?1 LIMIT 1",
            )?;
            stmt.query_row([history_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .optional()?
        };

        let Some((id, instance_id, after_payload)) = row else {
            return Err(DbError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "未找到历史记录",
            )));
        };
        let Some(instance_id) = instance_id else {
            return Err(DbError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "历史记录缺少 instance_id",
            )));
        };
        let Some(after_payload) = after_payload else {
            return Err(DbError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "历史记录缺少 after_payload，无法回放",
            )));
        };
        let mode = serde_json::from_str::<Value>(&after_payload)?;
        let result = self.upsert_instance_mode(&instance_id, mode, conflict_strategy, save_to_db)?;
        Ok(ModeHistoryReplayResult {
            history_id: id,
            instance_id,
            result,
        })
    }

    fn add_mode_history(
        &self,
        mode_id: Option<&str>,
        instance_id: Option<&str>,
        action: &str,
        before_payload: Option<&Value>,
        after_payload: Option<&Value>,
    ) -> Result<(), DbError> {
        let conn = self.lock_conn();
        let enable_log = Self::get_setting_with_conn(&conn, "app_settings")?
            .and_then(|raw| serde_json::from_str::<AppSettings>(&raw).ok())
            .map(|settings| settings.enable_log)
            .unwrap_or(true);
        if !enable_log {
            return Ok(());
        }

        let before_json = before_payload.map(|value| serde_json::to_string(value)).transpose()?;
        let after_json = after_payload.map(|value| serde_json::to_string(value)).transpose()?;
        conn.execute(
            "INSERT INTO mode_history (id, mode_id, instance_id, action, before_payload, after_payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                Uuid::new_v4().to_string(),
                mode_id,
                instance_id,
                action,
                before_json,
                after_json,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn list_sync_logs(&self, limit: usize, offset: usize) -> Result<Vec<SyncLogRecord>, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, sync_kind, rule_id, rule_name, target, status, message, created_at
             FROM sync_logs
             ORDER BY datetime(created_at) DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            Ok(SyncLogRecord {
                id: row.get("id")?,
                sync_kind: row.get("sync_kind")?,
                rule_id: row.get("rule_id")?,
                rule_name: row.get("rule_name")?,
                target: row.get("target")?,
                status: row.get("status")?,
                message: row.get("message")?,
                created_at: row.get("created_at")?,
            })
        })?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    pub fn clear_sync_logs(&self) -> Result<(), DbError> {
        let conn = self.lock_conn();
        conn.execute("DELETE FROM sync_logs", [])?;
        Ok(())
    }

    pub fn export_backup(&self, options: BackupOptions) -> Result<BackupPayload, DbError> {
        let mut modes = Vec::new();
        let mut github_rules = Vec::new();
        let mut ide_instances = Vec::new();
        let mut github_settings_json = None;
        let mut app_settings_json = None;

        if options.include_modes {
            let conn = self.lock_conn();
            let mut stmt = conn.prepare(
                "SELECT id, slug, name, description, groups, role_definition, role_definition_length, source,
                        when_to_use, custom_instructions, payload, raw_payload, source_path, source_alias,
                        updated_at, COALESCE(content_hash, hash) AS content_hash
                 FROM modes
                 ORDER BY datetime(updated_at) DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                let groups: String = row.get("groups")?;
                let payload: Option<String> = row.get("payload")?;
                let raw_payload: Option<String> = row.get("raw_payload")?;
                Ok(BackupModeRecord {
                    id: row.get("id")?,
                    slug: row.get("slug")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    groups: serde_json::from_str(&groups).unwrap_or_default(),
                    role_definition: row.get("role_definition")?,
                    role_definition_length: row.get("role_definition_length")?,
                    source: row.get("source")?,
                    when_to_use: row.get("when_to_use")?,
                    custom_instructions: row.get("custom_instructions")?,
                    payload: payload.and_then(|raw| serde_json::from_str(&raw).ok()),
                    raw_payload: raw_payload.and_then(|raw| serde_json::from_str(&raw).ok()),
                    source_path: row.get("source_path")?,
                    source_alias: row.get("source_alias")?,
                    updated_at: row.get("updated_at")?,
                    content_hash: row.get("content_hash")?,
                })
            })?;
            for row in rows {
                modes.push(row?);
            }
        }

        if options.include_rules {
            github_rules = self.list_github_rules()?;
        }

        if options.include_instances {
            ide_instances = self.list_ide_instances()?;
        }

        if options.include_settings {
            github_settings_json = self.get_setting("github_settings")?;
            app_settings_json = self.get_setting("app_settings")?;
        }

        Ok(BackupPayload {
            version: 1,
            exported_at: Utc::now().to_rfc3339(),
            options,
            modes,
            github_rules,
            ide_instances,
            github_settings_json,
            app_settings_json,
        })
    }

    pub fn import_backup(&self, payload: BackupPayload) -> Result<BackupImportResult, DbError> {
        let mut result = BackupImportResult {
            imported_modes: 0,
            skipped_duplicate_modes: 0,
            imported_rules: 0,
            imported_instances: 0,
            updated_settings: false,
            errors: Vec::new(),
        };

        for mode in payload.modes {
            let exists = {
                let conn = self.lock_conn();
                Self::find_mode_by_content_hash(&conn, &mode.content_hash)?.is_some()
            };
            if exists {
                result.skipped_duplicate_modes += 1;
                continue;
            }
            let mut raw_payload = mode.raw_payload;
            let slug = {
                let conn = self.lock_conn();
                let existing_by_slug = Self::find_mode_by_slug(&conn, &mode.slug)?;
                if let Some(existing) = existing_by_slug {
                    if existing.hash != mode.content_hash {
                        self.unique_slug_in_db(&conn, &mode.slug, "-copy")?
                    } else {
                        mode.slug.clone()
                    }
                } else {
                    mode.slug.clone()
                }
            };
            if slug != mode.slug {
                if let Some(Value::Object(obj)) = raw_payload.as_mut() {
                    obj.insert("slug".into(), Value::String(slug.clone()));
                }
            }
            let record = ModeRecord {
                id: mode.id,
                slug,
                name: mode.name,
                description: mode.description,
                groups: mode.groups,
                role_definition: mode.role_definition,
                role_definition_length: mode.role_definition_length,
                source: mode.source,
                when_to_use: mode.when_to_use,
                custom_instructions: mode.custom_instructions,
                payload: mode.payload,
                updated_at: mode.updated_at,
                hash: mode.content_hash,
            };
            if let Err(err) = self.upsert_mode_with_meta(
                record,
                raw_payload,
                mode.source_path,
                mode.source_alias,
            ) {
                result.errors.push(err.to_string());
            } else {
                result.imported_modes += 1;
            }
        }

        for rule in payload.github_rules {
            if let Err(err) = self.upsert_github_rule(rule) {
                result.errors.push(err.to_string());
            } else {
                result.imported_rules += 1;
            }
        }

        for mut instance in payload.ide_instances {
            if let Ok(Some(existing)) = self.find_instance_by_path(&instance.path) {
                instance.id = existing.id;
            }
            if let Err(err) = self.upsert_ide_instance(instance) {
                result.errors.push(err.to_string());
            } else {
                result.imported_instances += 1;
            }
        }

        if let Some(raw) = payload.github_settings_json {
            if let Err(err) = self.set_setting("github_settings", &raw) {
                result.errors.push(err.to_string());
            } else {
                result.updated_settings = true;
            }
        }
        if let Some(raw) = payload.app_settings_json {
            if let Err(err) = self.set_setting("app_settings", &raw) {
                result.errors.push(err.to_string());
            } else {
                result.updated_settings = true;
            }
        }

        Ok(result)
    }

    fn get_setting_with_conn(conn: &Connection, key: &str) -> Result<Option<String>, DbError> {
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let result = stmt
            .query_row([key], |row| row.get::<_, String>(0))
            .optional()?;
        Ok(result)
    }

    pub fn list_github_rules(&self) -> Result<Vec<GithubRuleRecord>, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, query, path_hint, branch, enabled, delay_sec, last_run_at FROM github_rules ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(GithubRuleRecord {
                id: row.get("id")?,
                name: row.get("name")?,
                query: row.get("query")?,
                path_hint: row.get("path_hint")?,
                branch: row
                    .get::<_, Option<String>>("branch")?
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "main".to_string()),
                enabled: row.get::<_, i64>("enabled")? == 1,
                delay_sec: row.get("delay_sec")?,
                last_run_at: row.get("last_run_at")?,
            })
        })?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    pub fn upsert_github_rule(&self, mut record: GithubRuleRecord) -> Result<GithubRuleRecord, DbError> {
        if record.id.trim().is_empty() {
            record.id = Uuid::new_v4().to_string();
        }
        if record.branch.trim().is_empty() {
            record.branch = "main".to_string();
        }
        let conn = self.lock_conn();
        conn.execute(
            "INSERT INTO github_rules (id, name, query, path_hint, branch, enabled, delay_sec, last_run_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name,
                query=excluded.query,
                path_hint=excluded.path_hint,
                branch=excluded.branch,
                enabled=excluded.enabled,
                delay_sec=excluded.delay_sec,
                last_run_at=excluded.last_run_at",
            params![
                record.id,
                record.name,
                record.query,
                record.path_hint,
                record.branch,
                if record.enabled { 1 } else { 0 },
                record.delay_sec,
                record.last_run_at
            ],
        )?;
        Ok(record)
    }

    pub fn delete_github_rule(&self, rule_id: &str) -> Result<(), DbError> {
        let conn = self.lock_conn();
        conn.execute("DELETE FROM github_rules WHERE id = ?1", [rule_id])?;
        Ok(())
    }

    pub fn list_ide_instances(&self) -> Result<Vec<IdeInstanceRecord>, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, alias, kind, path, last_scan_at, modes_count, status, selected_for_sync FROM ide_instances ORDER BY alias ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(IdeInstanceRecord {
                id: row.get("id")?,
                alias: row.get("alias")?,
                kind: row.get("kind")?,
                path: row.get("path")?,
                last_scan_at: row.get("last_scan_at")?,
                modes_count: row.get("modes_count")?,
                status: row.get("status")?,
                selected: row.get::<_, i64>("selected_for_sync")? == 1,
            })
        })?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    pub fn upsert_ide_instance(&self, mut record: IdeInstanceRecord) -> Result<IdeInstanceRecord, DbError> {
        if record.id.trim().is_empty() {
            record.id = Uuid::new_v4().to_string();
        }
        let conn = self.lock_conn();
        conn.execute(
            "INSERT INTO ide_instances (id, alias, kind, path, last_scan_at, modes_count, status, selected_for_sync)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                alias=excluded.alias,
                kind=excluded.kind,
                path=excluded.path,
                last_scan_at=excluded.last_scan_at,
                modes_count=excluded.modes_count,
                status=excluded.status,
                selected_for_sync=excluded.selected_for_sync",
            params![
                record.id,
                record.alias,
                record.kind,
                record.path,
                record.last_scan_at,
                record.modes_count,
                record.status,
                if record.selected { 1 } else { 0 }
            ],
        )?;
        Ok(record)
    }

    pub fn delete_ide_instance(&self, instance_id: &str) -> Result<(), DbError> {
        let conn = self.lock_conn();
        conn.execute("DELETE FROM ide_instances WHERE id = ?1", [instance_id])?;
        Ok(())
    }

    pub fn find_instance_by_path(&self, path: &str) -> Result<Option<IdeInstanceRecord>, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, alias, kind, path, last_scan_at, modes_count, status, selected_for_sync
             FROM ide_instances
             WHERE path = ?1
             LIMIT 1",
        )?;
        let result = stmt
            .query_row([path], |row| {
                Ok(IdeInstanceRecord {
                    id: row.get("id")?,
                    alias: row.get("alias")?,
                    kind: row.get("kind")?,
                    path: row.get("path")?,
                    last_scan_at: row.get("last_scan_at")?,
                    modes_count: row.get("modes_count")?,
                    status: row.get("status")?,
                    selected: row.get::<_, i64>("selected_for_sync")? == 1,
                })
            })
            .optional()?;
        Ok(result)
    }

    pub fn sync_known_instances(&self) -> Result<Vec<IdeInstanceRecord>, DbError> {
        let now = Utc::now().to_rfc3339();
        let mut synced = Vec::new();
        let mut candidates: Vec<(String, String, String)> = known_ide_templates()
            .iter()
            .map(|template| (template.alias.to_string(), template.kind.to_string(), template.path.to_string()))
            .collect();
        candidates.extend(discover_ide_instances());

        let mut seen_paths: HashSet<String> = HashSet::new();
        for (alias, kind, path_display) in candidates {
            if !seen_paths.insert(path_display.clone()) {
                continue;
            }
            let existing = self.find_instance_by_path(&path_display)?;
            let resolved = expand_home_path(&path_display);
            let file_exists = resolved.as_ref().map(|p| p.is_file()).unwrap_or(false);
            let status = if file_exists { "synced" } else { "missing" }.to_string();
            let import_result = if file_exists {
                match self.import_modes_from_file(&alias, &path_display) {
                    Ok(result) => result,
                    Err(err) => {
                        eprintln!("导入模式失败: {} => {}", path_display, err);
                        ModeImportResult::default()
                    }
                }
            } else {
                ModeImportResult::default()
            };
            let mut record = IdeInstanceRecord {
                id: existing
                    .as_ref()
                    .map(|item| item.id.clone())
                    .unwrap_or_default(),
                alias: existing
                    .as_ref()
                    .map(|item| item.alias.clone())
                    .unwrap_or_else(|| alias.to_string()),
                kind: kind.to_string(),
                path: path_display.clone(),
                last_scan_at: Some(now.clone()),
                modes_count: existing.as_ref().map(|item| item.modes_count).unwrap_or(0),
                status,
                selected: existing.as_ref().map(|item| item.selected).unwrap_or(false),
            };
            if file_exists {
                record.modes_count = import_result.discovered as i64;
            }
            record = self.upsert_ide_instance(record)?;
            synced.push(record);
        }
        Ok(synced)
    }

    pub fn sync_all_instances(&self) -> Result<Vec<IdeInstanceRecord>, DbError> {
        let _ = self.sync_known_instances();
        let now = Utc::now().to_rfc3339();
        let instances = self.list_ide_instances()?;
        let mut updated = Vec::new();
        for mut instance in instances {
            let resolved = expand_home_path(&instance.path);
            let file_exists = resolved.as_ref().map(|p| p.is_file()).unwrap_or(false);
            if file_exists {
                let import_result = self
                    .import_modes_from_file(&instance.alias, &instance.path)
                    .unwrap_or_default();
                instance.modes_count = import_result.discovered as i64;
                instance.status = "synced".to_string();
            } else {
                instance.status = "missing".to_string();
            }
            instance.last_scan_at = Some(now.clone());
            updated.push(self.upsert_ide_instance(instance)?);
        }
        Ok(updated)
    }

    pub fn sync_instance_modes(&self, instance_id: &str) -> Result<IdeInstanceRecord, DbError> {
        let now = Utc::now().to_rfc3339();
        let mut instance = self
            .find_instance_by_id(instance_id)?
            .ok_or_else(|| DbError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "未找到实例")))?;
        let resolved = expand_home_path(&instance.path);
        let file_exists = resolved.as_ref().map(|p| p.is_file()).unwrap_or(false);
        if file_exists {
            let import_result = self
                .import_modes_from_file(&instance.alias, &instance.path)
                .unwrap_or_default();
            instance.modes_count = import_result.discovered as i64;
            instance.status = "synced".to_string();
        } else {
            instance.status = "missing".to_string();
        }
        instance.last_scan_at = Some(now);
        self.upsert_ide_instance(instance)
    }

    pub fn diff_instance_modes(&self, instance_id: &str) -> Result<InstanceModeDiffSummary, DbError> {
        let mut instance = self
            .find_instance_by_id(instance_id)?
            .ok_or_else(|| DbError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "未找到实例")))?;

        let Some(resolved) = expand_home_path(&instance.path) else {
            instance.status = "missing".to_string();
            instance.modes_count = 0;
            let _ = self.upsert_ide_instance(instance.clone());
            return Ok(InstanceModeDiffSummary {
                instance_id: instance.id,
                alias: instance.alias,
                kind: instance.kind,
                path: instance.path,
                file_exists: false,
                status: "missing".to_string(),
                total_db: 0,
                total_ide: 0,
                same: 0,
                conflicts: Vec::new(),
                ide_only: Vec::new(),
                invalid: Vec::new(),
                db_only_total: 0,
                db_only_sample: Vec::new(),
            });
        };

        if !resolved.is_file() {
            instance.status = "missing".to_string();
            instance.modes_count = 0;
            let _ = self.upsert_ide_instance(instance.clone());
            return Ok(InstanceModeDiffSummary {
                instance_id: instance.id,
                alias: instance.alias,
                kind: instance.kind,
                path: instance.path,
                file_exists: false,
                status: "missing".to_string(),
                total_db: 0,
                total_ide: 0,
                same: 0,
                conflicts: Vec::new(),
                ide_only: Vec::new(),
                invalid: Vec::new(),
                db_only_total: 0,
                db_only_sample: Vec::new(),
            });
        }

        let conn = self.lock_conn();
        let mut stmt = conn.prepare("SELECT slug, name, COALESCE(content_hash, hash) AS hash FROM modes")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>("slug")?,
                row.get::<_, String>("name")?,
                row.get::<_, String>("hash")?,
            ))
        })?;
        let mut db_map: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new();
        for row in rows {
            let (slug, name, hash) = row?;
            db_map.insert(slug, (name, hash));
        }

        let content = fs::read_to_string(&resolved)?;
        let yaml: YamlValue = serde_yaml::from_str(&content)?;
        let sequence = yaml.get("customModes").and_then(|v| v.as_sequence()).cloned().unwrap_or_default();

        let mut ide_map: std::collections::HashMap<String, (Option<String>, Option<String>)> = std::collections::HashMap::new();
        let mut invalid = Vec::new();
        for node in &sequence {
            let Some(map) = node.as_mapping() else {
                invalid.push(InstanceModeDiffInvalidItem {
                    slug: None,
                    reason: "customModes 内存在非 mapping 项".to_string(),
                });
                continue;
            };
            let slug = yaml_string(map_get(map, "slug"));
            let Some(slug) = slug.filter(|s| !s.trim().is_empty()) else {
                invalid.push(InstanceModeDiffInvalidItem {
                    slug: None,
                    reason: "缺少 slug 字段".to_string(),
                });
                continue;
            };
            let name = yaml_string(map_get(map, "name"));

            let description = yaml_string(map_get(map, "description"));
            let role_definition = yaml_string(map_get(map, "roleDefinition"));
            let groups = extract_groups(map_get(map, "groups"));
            let when_to_use = yaml_string(map_get(map, "whenToUse"));
            let custom_instructions = yaml_string(map_get(map, "customInstructions"));

            let mut missing_fields = Vec::new();
            if name.as_deref().unwrap_or_default().trim().is_empty() {
                missing_fields.push("name");
            }
            if description.as_deref().unwrap_or_default().trim().is_empty() {
                missing_fields.push("description");
            }
            if role_definition.as_deref().unwrap_or_default().trim().is_empty() {
                missing_fields.push("roleDefinition");
            }
            if groups.is_empty() {
                missing_fields.push("groups");
            }

            let content_hash = if missing_fields.is_empty() {
                Some(Self::compute_content_hash_from_fields(
                    description.as_deref().unwrap_or_default(),
                    role_definition.as_deref().unwrap_or_default(),
                    &groups,
                    when_to_use.as_ref(),
                    custom_instructions.as_ref(),
                ))
            } else {
                invalid.push(InstanceModeDiffInvalidItem {
                    slug: Some(slug.clone()),
                    reason: format!("缺少必要字段，无法计算内容哈希：{}", missing_fields.join(", ")),
                });
                None
            };

            if ide_map.contains_key(&slug) {
                invalid.push(InstanceModeDiffInvalidItem {
                    slug: Some(slug.clone()),
                    reason: "IDE 配置内存在重复 slug".to_string(),
                });
                continue;
            }
            ide_map.insert(slug, (name, content_hash));
        }

        let mut same = 0usize;
        let mut conflicts = Vec::new();
        let mut ide_only = Vec::new();

        for (slug, (name, ide_hash)) in &ide_map {
            match db_map.get(slug) {
                None => {
                    ide_only.push(InstanceModeDiffOnlyItem {
                        slug: slug.clone(),
                        name: name.clone(),
                    });
                }
                Some((_db_name, db_hash)) => {
                    if let Some(ide_hash) = ide_hash {
                        if ide_hash == db_hash {
                            same += 1;
                        } else {
                            conflicts.push(InstanceModeDiffConflictItem {
                                slug: slug.clone(),
                                name: name.clone(),
                                db_hash: db_hash.clone(),
                                ide_hash: ide_hash.clone(),
                            });
                        }
                    }
                }
            }
        }

        let mut db_only_total = 0usize;
        let mut db_only_sample = Vec::new();
        for (slug, (name, _hash)) in &db_map {
            if ide_map.contains_key(slug) {
                continue;
            }
            db_only_total += 1;
            if db_only_sample.len() < 50 {
                db_only_sample.push(InstanceModeDiffOnlyItem {
                    slug: slug.clone(),
                    name: Some(name.clone()),
                });
            }
        }

        let status = if conflicts.is_empty() && ide_only.is_empty() && invalid.is_empty() {
            "synced".to_string()
        } else {
            "outdated".to_string()
        };

        instance.status = status.clone();
        instance.modes_count = ide_map.len() as i64;
        let _ = self.upsert_ide_instance(instance.clone());

        Ok(InstanceModeDiffSummary {
            instance_id: instance.id,
            alias: instance.alias,
            kind: instance.kind,
            path: instance.path,
            file_exists: true,
            status,
            total_db: db_map.len(),
            total_ide: ide_map.len(),
            same,
            conflicts,
            ide_only,
            invalid,
            db_only_total,
            db_only_sample,
        })
    }

    pub fn import_instance_modes_to_db(
        &self,
        instance_id: &str,
        mode_slugs: Option<Vec<String>>,
        conflict_strategy: &str,
    ) -> Result<ModeImportReport, DbError> {
        let instance = self
            .find_instance_by_id(instance_id)?
            .ok_or_else(|| DbError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "未找到实例")))?;

        let Some(resolved) = expand_home_path(&instance.path) else {
            return Ok(ModeImportReport {
                discovered: 0,
                saved: 0,
                skipped_due_to_missing_fields: 0,
                duplicate_slug: 0,
                duplicate_hash: 0,
                errors: vec!["无法解析实例路径".to_string()],
            });
        };
        if !resolved.exists() {
            return Ok(ModeImportReport {
                discovered: 0,
                saved: 0,
                skipped_due_to_missing_fields: 0,
                duplicate_slug: 0,
                duplicate_hash: 0,
                errors: vec!["实例配置文件不存在".to_string()],
            });
        }

        let wanted: Option<std::collections::HashSet<String>> = mode_slugs.map(|items| {
            items
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        });

        let content = fs::read_to_string(&resolved)?;
        let yaml: YamlValue = serde_yaml::from_str(&content)?;
        let sequence = yaml.get("customModes").and_then(|v| v.as_sequence()).cloned().unwrap_or_default();

        let mut report = ModeImportReport {
            discovered: 0,
            saved: 0,
            skipped_due_to_missing_fields: 0,
            duplicate_slug: 0,
            duplicate_hash: 0,
            errors: Vec::new(),
        };

        let conn = self.lock_conn();
        for node in &sequence {
            let slug = node
                .as_mapping()
                .and_then(|map| map_get(map, "slug"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if let Some(wanted) = &wanted {
                if slug.trim().is_empty() || !wanted.contains(&slug) {
                    continue;
                }
            }

            report.discovered += 1;
            let candidate = match Self::mode_candidate_from_yaml(&instance.alias, &instance.path, node) {
                Ok(Some(candidate)) => candidate,
                Ok(None) => {
                    report.skipped_due_to_missing_fields += 1;
                    continue;
                }
                Err(err) => {
                    report.errors.push(format!("解析模式失败 {}: {}", slug, err));
                    continue;
                }
            };

            if Self::find_mode_by_content_hash(&conn, &candidate.content_hash)?.is_some() {
                report.duplicate_hash += 1;
                continue;
            }

            let existing = Self::find_mode_by_slug(&conn, &candidate.record.slug)?;
            let mut candidate = candidate;

            match conflict_strategy {
                "skip" => {
                    if existing.is_some() {
                        report.duplicate_slug += 1;
                        continue;
                    }
                }
                "rename" => {
                    if existing.is_some() {
                        let unique = self.unique_slug_in_db(&conn, &candidate.record.slug, "-copy")?;
                        candidate.record.slug = unique.clone();
                        if let Some(Value::Object(obj)) = candidate.raw_payload.as_mut() {
                            obj.insert("slug".into(), Value::String(unique));
                        }
                    }
                }
                "overwrite" => {}
                other => {
                    report
                        .errors
                        .push(format!("不支持的冲突策略：{}（仅支持 overwrite/rename/skip）", other));
                    continue;
                }
            }

            match self.upsert_mode_candidate(candidate) {
                Ok(_) => report.saved += 1,
                Err(err) => report.errors.push(format!("入库失败 {}: {}", slug, err)),
            }
        }

        Ok(report)
    }

    fn unique_slug_in_db(&self, conn: &Connection, base: &str, suffix: &str) -> Result<String, DbError> {
        let mut candidate = format!("{}{}", base, suffix);
        let mut counter = 2;
        while Self::find_mode_by_slug(conn, &candidate)?.is_some() {
            candidate = format!("{}{}-{}", base, suffix, counter);
            counter += 1;
        }
        Ok(candidate)
    }

    fn import_modes_from_file(&self, alias: &str, path: &str) -> Result<ModeImportResult, DbError> {
        let mut report = ModeImportResult::default();
        let Some(resolved) = expand_home_path(path) else {
            return Ok(report);
        };
        if !resolved.is_file() {
            return Ok(report);
        }
        let content = fs::read_to_string(resolved)?;
        let yaml: YamlValue = serde_yaml::from_str(&content)?;
        let Some(sequence) = yaml.get("customModes").and_then(|v| v.as_sequence()) else {
            return Ok(report);
        };

        for node in sequence {
            report.discovered += 1;
            match Self::mode_candidate_from_yaml(alias, path, node) {
                Ok(Some(candidate)) => {
                    let conn = self.lock_conn();
                    let existing_by_hash = Self::find_mode_by_content_hash(&conn, &candidate.content_hash)?;
                    let existing_by_slug = Self::find_mode_by_slug(&conn, &candidate.record.slug)?;
                    let mut candidate = candidate;
                    if existing_by_hash.is_some() {
                        continue;
                    }
                    if let Some(existing) = existing_by_slug {
                        if existing.hash != candidate.content_hash {
                            let unique = self.unique_slug_in_db(&conn, &candidate.record.slug, "-copy")?;
                            candidate.record.slug = unique.clone();
                            if let Some(Value::Object(obj)) = candidate.raw_payload.as_mut() {
                                obj.insert("slug".into(), Value::String(unique));
                            }
                        }
                    }
                    drop(conn);
                    if self.upsert_mode_candidate(candidate).is_ok() {
                        report.saved += 1;
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    eprintln!("解析模式失败 {}: {}", path, err);
                }
            }
        }

        Ok(report)
    }

    fn mode_candidate_from_yaml(
        alias: &str,
        source_path: &str,
        value: &YamlValue,
    ) -> Result<Option<ModeUpsertCandidate>, DbError> {
        let map = match value.as_mapping() {
            Some(mapping) => mapping,
            None => return Ok(None),
        };

        let slug = yaml_string(map_get(map, "slug"));
        let name = yaml_string(map_get(map, "name"));
        let description = yaml_string(map_get(map, "description"));
        let role_definition = yaml_string(map_get(map, "roleDefinition"));
        let groups = extract_groups(map_get(map, "groups"));

        if slug.as_deref().unwrap_or_default().is_empty()
            || name.as_deref().unwrap_or_default().is_empty()
            || description.as_deref().unwrap_or_default().is_empty()
            || role_definition.as_deref().unwrap_or_default().is_empty()
        {
            return Ok(None);
        }

        let slug = slug.unwrap();
        let name = name.unwrap();
        let description = description.unwrap();
        let role_definition = role_definition.unwrap();

        let when_to_use = yaml_string(map_get(map, "whenToUse"));
        let custom_instructions = yaml_string(map_get(map, "customInstructions"));
        let raw_payload = Some(serde_json::to_value(value)?);
        let content_hash = Self::compute_content_hash_from_fields(
            &description,
            &role_definition,
            &groups,
            when_to_use.as_ref(),
            custom_instructions.as_ref(),
        );

        Ok(Some(ModeUpsertCandidate {
            record: ModeRecord {
                id: Uuid::new_v4().to_string(),
                slug,
                name,
                description,
                groups,
                role_definition: role_definition.clone(),
                role_definition_length: role_definition.chars().count() as i64,
                source: "ide".to_string(),
                when_to_use,
                custom_instructions,
                payload: Some(json!({})),
                updated_at: Utc::now().to_rfc3339(),
                hash: content_hash.clone(),
            },
            raw_payload,
            source_path: Some(source_path.to_string()),
            source_alias: Some(alias.to_string()),
            content_hash,
        }))
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let result = stmt
            .query_row([key], |row| row.get::<_, String>(0))
            .optional()?;
        Ok(result)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        let conn = self.lock_conn();
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_app_settings(&self) -> Result<AppSettings, DbError> {
        match self.get_setting("app_settings")? {
            Some(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            None => Ok(AppSettings::default()),
        }
    }

    pub fn update_app_settings(&self, settings: AppSettings) -> Result<AppSettings, DbError> {
        let json = serde_json::to_string(&settings)?;
        self.set_setting("app_settings", &json)?;
        if let Ok(mut logger) = self.logger.lock() {
            logger.update_settings(&settings);
        }
        Ok(settings)
    }

    pub fn apply_modes_to_instances(
        &self,
        mode_slugs: Vec<String>,
        instance_ids: Vec<String>,
        conflict_strategy: String,
    ) -> Result<ApplyModesResult, DbError> {
        let mut result = ApplyModesResult {
            total_instances: instance_ids.len(),
            updated_instances: 0,
            skipped_instances: 0,
            errors: Vec::new(),
            details: Vec::new(),
        };

        if mode_slugs.is_empty() || instance_ids.is_empty() {
            return Ok(result);
        }

        for instance_id in instance_ids {
            match self.apply_modes_to_instance(&mode_slugs, &instance_id, &conflict_strategy) {
                Ok(detail) => {
                    if detail.applied + detail.overwritten + detail.renamed > 0 {
                        result.updated_instances += 1;
                    } else {
                        result.skipped_instances += 1;
                    }
                    result.details.push(detail);
                }
                Err(err) => {
                    result.errors.push(format!("实例 {} 写入失败: {}", instance_id, err));
                    result.details.push(ApplyInstanceResult {
                        instance_id,
                        alias: String::new(),
                        path: String::new(),
                        applied: 0,
                        overwritten: 0,
                        renamed: 0,
                        skipped: mode_slugs.len(),
                        status: "error".to_string(),
                        messages: vec![err.to_string()],
                    });
                }
            }
        }

        Ok(result)
    }

    pub fn compare_kilo_roo_modes(&self) -> Result<Vec<ModeCompareItem>, DbError> {
        let instances = self.list_ide_instances()?;
        let mut kilo_set: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut roo_set: std::collections::HashSet<String> = std::collections::HashSet::new();

        for instance in instances {
            let slugs = read_mode_slugs_from_instance_path(&instance.path).unwrap_or_default();
            if instance.kind == "kilocode" {
                for slug in slugs {
                    kilo_set.insert(slug);
                }
            } else if instance.kind == "roocode" {
                for slug in slugs {
                    roo_set.insert(slug);
                }
            }
        }

        let mut union: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        union.extend(kilo_set.iter().cloned());
        union.extend(roo_set.iter().cloned());

        Ok(union
            .into_iter()
            .map(|slug| ModeCompareItem {
                in_kilocode: kilo_set.contains(&slug),
                in_roocode: roo_set.contains(&slug),
                slug,
            })
            .collect())
    }

    fn apply_modes_to_instance(
        &self,
        mode_slugs: &[String],
        instance_id: &str,
        conflict_strategy: &str,
    ) -> Result<ApplyInstanceResult, DbError> {
        let instance = self
            .find_instance_by_id(instance_id)?
            .ok_or_else(|| DbError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "未找到实例")))?;

        let resolved_path = expand_home_path(&instance.path)
            .ok_or_else(|| DbError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "无法解析实例路径")))?;
        if resolved_path.exists() && resolved_path.is_dir() {
            return Err(DbError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "实例路径指向目录，请填写配置文件路径（例如 custom_modes.yml）",
            )));
        }

        let mut root = if resolved_path.exists() {
            let content = fs::read_to_string(&resolved_path)?;
            if content.trim().is_empty() {
                YamlValue::Mapping(YamlMapping::new())
            } else {
                serde_yaml::from_str::<YamlValue>(&content).unwrap_or_else(|_| YamlValue::Mapping(YamlMapping::new()))
            }
        } else {
            YamlValue::Mapping(YamlMapping::new())
        };

        let mut messages = Vec::new();
        let root_map = root.as_mapping_mut().ok_or_else(|| {
            DbError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, "配置文件不是 YAML mapping"))
        })?;

        let key = YamlValue::String("customModes".to_string());
        let existing_modes = root_map
            .get(&key)
            .and_then(|v| v.as_sequence())
            .cloned()
            .unwrap_or_else(Vec::new);
        let mut mode_list = existing_modes;

        let mut applied = 0usize;
        let mut overwritten = 0usize;
        let mut renamed = 0usize;
        let mut skipped = 0usize;

        for slug in mode_slugs {
            let Some(mut definition) = self.load_mode_definition_json(slug)? else {
                skipped += 1;
                messages.push(format!("未找到 slug={} 的模式，已跳过", slug));
                continue;
            };

            let mut target_slug = slug.clone();
            let existing_index = mode_list.iter().position(|item| {
                item.as_mapping()
                    .and_then(|map| map_get(map, "slug").and_then(|v| v.as_str()))
                    .map(|s| s == slug)
                    .unwrap_or(false)
            });

            if let Some(index) = existing_index {
                match conflict_strategy {
                    "overwrite" => {
                        let before = serde_json::to_value(&mode_list[index]).unwrap_or(Value::Null);
                        let yaml_node = serde_yaml::to_value(&definition)?;
                        mode_list[index] = yaml_node;
                        overwritten += 1;
                        let mode_id = self.find_mode_id_by_slug(slug)?;
                        let _ = self.add_mode_history(
                            mode_id.as_deref(),
                            Some(&instance.id),
                            "apply_overwrite",
                            Some(&before),
                            Some(&definition),
                        );
                    }
                    "rename" => {
                        target_slug = unique_slug_in_yaml(&mode_list, slug, "-copy");
                        if let Value::Object(obj) = &mut definition {
                            obj.insert("slug".into(), Value::String(target_slug.clone()));
                        }
                        let yaml_node = serde_yaml::to_value(&definition)?;
                        mode_list.push(yaml_node);
                        renamed += 1;
                        let mode_id = self.find_mode_id_by_slug(slug)?;
                        let _ = self.add_mode_history(
                            mode_id.as_deref(),
                            Some(&instance.id),
                            "apply_rename",
                            None,
                            Some(&definition),
                        );
                    }
                    _ => {
                        skipped += 1;
                    }
                }
            } else {
                let yaml_node = serde_yaml::to_value(&definition)?;
                mode_list.push(yaml_node);
                applied += 1;
                let mode_id = self.find_mode_id_by_slug(slug)?;
                let _ = self.add_mode_history(
                    mode_id.as_deref(),
                    Some(&instance.id),
                    "apply_add",
                    None,
                    Some(&definition),
                );
            }

            if target_slug != *slug {
                messages.push(format!("slug 冲突：{} → {}", slug, target_slug));
            }
        }

        root_map.insert(key, YamlValue::Sequence(mode_list.clone()));
        if let Some(parent) = resolved_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let yaml_out = serde_yaml::to_string(&root)?;
        fs::write(&resolved_path, yaml_out)?;

        let mut updated_instance = instance.clone();
        updated_instance.last_scan_at = Some(Utc::now().to_rfc3339());
        updated_instance.modes_count = mode_list.len() as i64;
        updated_instance.status = "synced".to_string();
        self.upsert_ide_instance(updated_instance)?;

        Ok(ApplyInstanceResult {
            instance_id: instance.id,
            alias: instance.alias,
            path: instance.path,
            applied,
            overwritten,
            renamed,
            skipped,
            status: "success".to_string(),
            messages,
        })
    }

    pub fn list_instance_modes(&self, instance_id: &str) -> Result<Vec<InstanceModeItem>, DbError> {
        let instance = self
            .find_instance_by_id(instance_id)?
            .ok_or_else(|| DbError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "未找到实例")))?;
        let Some(resolved) = expand_home_path(&instance.path) else {
            return Ok(Vec::new());
        };
        if !resolved.is_file() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(resolved)?;
        let yaml: YamlValue = serde_yaml::from_str(&content)?;
        let Some(sequence) = yaml.get("customModes").and_then(|v| v.as_sequence()) else {
            return Ok(Vec::new());
        };
        let mut items = Vec::new();
        for node in sequence {
            let slug = node
                .as_mapping()
                .and_then(|map| map_get(map, "slug"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if slug.trim().is_empty() {
                continue;
            }
            let name = node
                .as_mapping()
                .and_then(|map| map_get(map, "name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let raw = serde_json::to_value(node).unwrap_or(Value::Null);
            items.push(InstanceModeItem { slug, name, raw });
        }
        Ok(items)
    }

    pub fn get_instance_mode_raw(&self, instance_id: &str, slug: &str) -> Result<Option<Value>, DbError> {
        if slug.trim().is_empty() {
            return Ok(None);
        }
        let instance = self
            .find_instance_by_id(instance_id)?
            .ok_or_else(|| DbError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "未找到实例")))?;
        let Some(resolved) = expand_home_path(&instance.path) else {
            return Ok(None);
        };
        if !resolved.is_file() {
            return Ok(None);
        }
        let content = fs::read_to_string(resolved)?;
        let yaml: YamlValue = serde_yaml::from_str(&content)?;
        let Some(sequence) = yaml.get("customModes").and_then(|v| v.as_sequence()) else {
            return Ok(None);
        };
        for node in sequence {
            let node_slug = node
                .as_mapping()
                .and_then(|map| map_get(map, "slug"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if node_slug == slug {
                return Ok(serde_json::to_value(node).ok());
            }
        }
        Ok(None)
    }

    pub fn upsert_instance_mode(
        &self,
        instance_id: &str,
        mut mode: Value,
        conflict_strategy: String,
        save_to_db: bool,
    ) -> Result<InstanceModeUpsertResult, DbError> {
        let instance = self
            .find_instance_by_id(instance_id)?
            .ok_or_else(|| DbError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "未找到实例")))?;
        let requested_slug = mode
            .get("slug")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if requested_slug.trim().is_empty() {
            return Err(DbError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "缺少 slug 字段",
            )));
        }

        let resolved_path = expand_home_path(&instance.path)
            .ok_or_else(|| DbError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "无法解析实例路径")))?;
        if resolved_path.exists() && resolved_path.is_dir() {
            return Err(DbError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "实例路径指向目录，请填写配置文件路径（例如 custom_modes.yml）",
            )));
        }

        let mut root = if resolved_path.exists() {
            let content = fs::read_to_string(&resolved_path)?;
            if content.trim().is_empty() {
                YamlValue::Mapping(YamlMapping::new())
            } else {
                serde_yaml::from_str::<YamlValue>(&content).unwrap_or_else(|_| YamlValue::Mapping(YamlMapping::new()))
            }
        } else {
            YamlValue::Mapping(YamlMapping::new())
        };

        let root_map = root.as_mapping_mut().ok_or_else(|| {
            DbError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, "配置文件不是 YAML mapping"))
        })?;
        let key = YamlValue::String("customModes".to_string());
        let mut mode_list = root_map
            .get(&key)
            .and_then(|v| v.as_sequence())
            .cloned()
            .unwrap_or_else(Vec::new);

        let existing_index = mode_list.iter().position(|item| {
            item.as_mapping()
                .and_then(|map| map_get(map, "slug").and_then(|v| v.as_str()))
                .map(|s| s == requested_slug)
                .unwrap_or(false)
        });

        let mut final_slug = requested_slug.clone();
        if let Some(index) = existing_index {
            match conflict_strategy.as_str() {
                "overwrite" => {}
                "rename" => {
                    final_slug = unique_slug_in_yaml(&mode_list, &requested_slug, "-copy");
                    if let Value::Object(obj) = &mut mode {
                        obj.insert("slug".into(), Value::String(final_slug.clone()));
                    }
                    mode_list.push(serde_yaml::to_value(&mode)?);
                    let before_payload = None;
                    let _ = self.add_mode_history(
                        None,
                        Some(&instance.id),
                        "instance_rename",
                        before_payload,
                        Some(&mode),
                    );
                    root_map.insert(key, YamlValue::Sequence(mode_list));
                    if let Some(parent) = resolved_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&resolved_path, serde_yaml::to_string(&root)?)?;
                    if save_to_db {
                        self.try_upsert_mode_from_instance_payload(&instance, &mode)?;
                    }
                    return Ok(InstanceModeUpsertResult {
                        requested_slug,
                        final_slug,
                    });
                }
                _ => {
                    return Ok(InstanceModeUpsertResult {
                        requested_slug,
                        final_slug,
                    });
                }
            }

            let before_payload = serde_json::to_value(&mode_list[index]).ok();
            mode_list[index] = serde_yaml::to_value(&mode)?;
            let _ = self.add_mode_history(
                None,
                Some(&instance.id),
                "instance_overwrite",
                before_payload.as_ref(),
                Some(&mode),
            );
        } else {
            mode_list.push(serde_yaml::to_value(&mode)?);
            let _ = self.add_mode_history(
                None,
                Some(&instance.id),
                "instance_add",
                None,
                Some(&mode),
            );
        }

        root_map.insert(key, YamlValue::Sequence(mode_list.clone()));
        if let Some(parent) = resolved_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&resolved_path, serde_yaml::to_string(&root)?)?;

        let mut updated_instance = instance.clone();
        updated_instance.last_scan_at = Some(Utc::now().to_rfc3339());
        updated_instance.modes_count = mode_list.len() as i64;
        updated_instance.status = "synced".to_string();
        self.upsert_ide_instance(updated_instance)?;

        if save_to_db {
            self.try_upsert_mode_from_instance_payload(&instance, &mode)?;
        }

        Ok(InstanceModeUpsertResult {
            requested_slug,
            final_slug,
        })
    }

    pub fn delete_instance_mode(&self, instance_id: &str, slug: &str) -> Result<(), DbError> {
        let instance = self
            .find_instance_by_id(instance_id)?
            .ok_or_else(|| DbError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "未找到实例")))?;
        let Some(resolved_path) = expand_home_path(&instance.path) else {
            return Ok(());
        };
        if !resolved_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&resolved_path)?;
        let mut root: YamlValue =
            serde_yaml::from_str::<YamlValue>(&content).unwrap_or_else(|_| YamlValue::Mapping(YamlMapping::new()));
        let root_map = root.as_mapping_mut().ok_or_else(|| {
            DbError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, "配置文件不是 YAML mapping"))
        })?;
        let key = YamlValue::String("customModes".to_string());
        let mut mode_list = root_map
            .get(&key)
            .and_then(|v| v.as_sequence())
            .cloned()
            .unwrap_or_else(Vec::new);
        let mut removed: Option<Value> = None;
        mode_list.retain(|item| {
            let matches = item
                .as_mapping()
                .and_then(|map| map_get(map, "slug").and_then(|v| v.as_str()))
                .map(|s| s == slug)
                .unwrap_or(false);
            if matches && removed.is_none() {
                removed = serde_json::to_value(item).ok();
            }
            !matches
        });
        if let Some(before) = removed.as_ref() {
            let _ = self.add_mode_history(
                None,
                Some(&instance.id),
                "instance_delete",
                Some(before),
                None,
            );
        }
        root_map.insert(key, YamlValue::Sequence(mode_list.clone()));
        fs::write(&resolved_path, serde_yaml::to_string(&root)?)?;

        let mut updated_instance = instance.clone();
        updated_instance.last_scan_at = Some(Utc::now().to_rfc3339());
        updated_instance.modes_count = mode_list.len() as i64;
        updated_instance.status = "synced".to_string();
        self.upsert_ide_instance(updated_instance)?;

        Ok(())
    }

    fn try_upsert_mode_from_instance_payload(&self, instance: &IdeInstanceRecord, mode: &Value) -> Result<(), DbError> {
        let yaml_node = serde_yaml::to_value(mode)?;
        let candidate = candidate_from_document_node(
            &yaml_node,
            "ide",
            Some(instance.alias.clone()),
            Some(instance.path.clone()),
        )
        .map_err(|missing| DbError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!(
            "缺少字段: {}",
            missing.join(", ")
        ))))?;
        if let Some(candidate) = candidate {
            let _ = self.upsert_mode_candidate(candidate)?;
        }
        Ok(())
    }

    fn find_mode_id_by_slug(&self, slug: &str) -> Result<Option<String>, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare("SELECT id FROM modes WHERE slug = ?1 LIMIT 1")?;
        let result = stmt
            .query_row([slug], |row| row.get::<_, String>(0))
            .optional()?;
        Ok(result)
    }

    fn find_instance_by_id(&self, id: &str) -> Result<Option<IdeInstanceRecord>, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, alias, kind, path, last_scan_at, modes_count, status, selected_for_sync
             FROM ide_instances
             WHERE id = ?1
             LIMIT 1",
        )?;
        let result = stmt
            .query_row([id], |row| {
                Ok(IdeInstanceRecord {
                    id: row.get("id")?,
                    alias: row.get("alias")?,
                    kind: row.get("kind")?,
                    path: row.get("path")?,
                    last_scan_at: row.get("last_scan_at")?,
                    modes_count: row.get("modes_count")?,
                    status: row.get("status")?,
                    selected: row.get::<_, i64>("selected_for_sync")? == 1,
                })
            })
            .optional()?;
        Ok(result)
    }

    fn load_mode_definition_json(&self, slug: &str) -> Result<Option<Value>, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT slug, name, description, groups, role_definition, when_to_use, custom_instructions, payload, raw_payload
             FROM modes WHERE slug = ?1 LIMIT 1",
        )?;
        let row = stmt
            .query_row([slug], |row| {
                Ok((
                    row.get::<_, String>("slug")?,
                    row.get::<_, String>("name")?,
                    row.get::<_, String>("description")?,
                    row.get::<_, String>("groups")?,
                    row.get::<_, String>("role_definition")?,
                    row.get::<_, Option<String>>("when_to_use")?,
                    row.get::<_, Option<String>>("custom_instructions")?,
                    row.get::<_, Option<String>>("payload")?,
                    row.get::<_, Option<String>>("raw_payload")?,
                ))
            })
            .optional()?;

        let Some((
            slug,
            name,
            description,
            groups_json,
            role_definition,
            when_to_use,
            custom_instructions,
            payload_json,
            raw_json,
        )) = row
        else {
            return Ok(None);
        };

        let config_source = payload_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|val| val.get("configSource").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| "global".to_string());
        let groups: Vec<String> = serde_json::from_str(&groups_json).unwrap_or_default();
        if let Some(raw) = raw_json {
            if let Ok(Value::Object(mut obj)) = serde_json::from_str::<Value>(&raw) {
                obj.insert("slug".into(), Value::String(slug.clone()));
                obj.insert("name".into(), Value::String(name.clone()));
                obj.insert("description".into(), Value::String(description.clone()));
                obj.insert("groups".into(), serde_json::to_value(groups.clone())?);
                obj.insert("roleDefinition".into(), Value::String(role_definition.clone()));
                obj.insert("source".into(), Value::String(config_source));
                match when_to_use.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    Some(value) => {
                        obj.insert("whenToUse".into(), Value::String(value.to_string()));
                    }
                    None => {
                        obj.remove("whenToUse");
                    }
                }
                match custom_instructions.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    Some(value) => {
                        obj.insert("customInstructions".into(), Value::String(value.to_string()));
                    }
                    None => {
                        obj.remove("customInstructions");
                    }
                }
                return Ok(Some(Value::Object(obj)));
            }
        }

        let mut obj = serde_json::Map::new();
        obj.insert("slug".into(), Value::String(slug));
        obj.insert("name".into(), Value::String(name));
        obj.insert("description".into(), Value::String(description));
        obj.insert("groups".into(), serde_json::to_value(groups)?);
        obj.insert("roleDefinition".into(), Value::String(role_definition));
        obj.insert("source".into(), Value::String(config_source));
        if let Some(value) = when_to_use {
            obj.insert("whenToUse".into(), Value::String(value));
        }
        if let Some(value) = custom_instructions {
            obj.insert("customInstructions".into(), Value::String(value));
        }
        Ok(Some(Value::Object(obj)))
    }

    pub fn preview_mode_diff(&self, text: &str) -> Result<ModeDiffPreview, DbError> {
        let parsed = parse_modes_document(text).unwrap_or_else(|_| Vec::new());
        let mut preview = ModeDiffPreview {
            discovered: parsed.len(),
            new_modes: 0,
            duplicates: 0,
            conflicts: 0,
            invalid: 0,
            items: Vec::new(),
        };
        let conn = self.lock_conn();

        for node in parsed {
            let candidate = candidate_from_document_node(&node, "local", None, None);
            match candidate {
                Ok(Some(candidate)) => {
                    let existing_by_slug = Self::find_mode_by_slug(&conn, &candidate.record.slug)?;
                    let existing_by_hash = Self::find_mode_by_content_hash(&conn, &candidate.content_hash)?;

                    let mut status = "new".to_string();
                    let mut action = "create".to_string();
                    let mut existing_slug = None;
                    let mut existing_hash = None;
                    let mut rename_suggestion = None;

                    if let Some(existing) = existing_by_hash.as_ref() {
                        preview.duplicates += 1;
                        status = "duplicateContent".to_string();
                        action = "ignore".to_string();
                        existing_slug = Some(existing.slug.clone());
                        existing_hash = Some(existing.hash.clone());
                    } else if let Some(existing) = existing_by_slug.as_ref() {
                        if existing.hash == candidate.content_hash {
                            preview.duplicates += 1;
                            status = "same".to_string();
                            action = "ignore".to_string();
                            existing_slug = Some(existing.slug.clone());
                            existing_hash = Some(existing.hash.clone());
                        } else {
                            preview.conflicts += 1;
                            status = "slugConflict".to_string();
                            action = "overwrite".to_string();
                            existing_slug = Some(existing.slug.clone());
                            existing_hash = Some(existing.hash.clone());
                            rename_suggestion = Some(format!("{}-copy", candidate.record.slug));
                        }
                    } else {
                        preview.new_modes += 1;
                    }

                    preview.items.push(ModeDiffPreviewItem {
                        slug: candidate.record.slug,
                        name: candidate.record.name,
                        content_hash: candidate.content_hash,
                        status,
                        recommended_action: action,
                        existing_slug,
                        existing_hash,
                        rename_suggestion,
                        missing_fields: Vec::new(),
                    });
                }
                Ok(None) => {
                    preview.invalid += 1;
                }
                Err(missing) => {
                    preview.invalid += 1;
                    preview.items.push(ModeDiffPreviewItem {
                        slug: node_slug_fallback(&node),
                        name: node_name_fallback(&node),
                        content_hash: String::new(),
                        status: "invalid".to_string(),
                        recommended_action: "ignore".to_string(),
                        existing_slug: None,
                        existing_hash: None,
                        rename_suggestion: None,
                        missing_fields: missing,
                    });
                }
            }
        }

        Ok(preview)
    }

    pub fn import_modes_from_text_with_strategy(
        &self,
        text: &str,
        conflict_strategy: &str,
    ) -> Result<ModeImportReport, DbError> {
        self.import_modes_from_text_scoped_with_hint_and_strategy(
            text,
            "local",
            None,
            None,
            None,
            conflict_strategy,
        )
    }

    pub fn import_modes_from_text_scoped_with_hint_and_strategy(
        &self,
        text: &str,
        source: &str,
        source_alias: Option<String>,
        source_path: Option<String>,
        path_hint: Option<&str>,
        conflict_strategy: &str,
    ) -> Result<ModeImportReport, DbError> {
        let parsed = parse_modes_document_with_hint(text, path_hint).unwrap_or_else(|_| Vec::new());
        let mut report = ModeImportReport {
            discovered: parsed.len(),
            saved: 0,
            skipped_due_to_missing_fields: 0,
            duplicate_slug: 0,
            duplicate_hash: 0,
            errors: Vec::new(),
        };

        for node in parsed {
            let candidate = candidate_from_document_node(
                &node,
                source,
                source_alias.clone(),
                source_path.clone(),
            );
            match candidate {
                Ok(Some(candidate)) => {
                    let conn = self.lock_conn();
                    let existing_by_slug = Self::find_mode_by_slug(&conn, &candidate.record.slug)?;
                    let existing_by_hash = Self::find_mode_by_content_hash(&conn, &candidate.content_hash)?;
                    let mut candidate = candidate;
                    drop(conn);

                    if existing_by_hash.is_some() {
                        report.duplicate_hash += 1;
                        continue;
                    }
                    if let Some(existing) = existing_by_slug {
                        if existing.hash != candidate.content_hash {
                            report.duplicate_slug += 1;
                            match conflict_strategy {
                                "skip" => {
                                    continue;
                                }
                                "rename" => {
                                    let conn = self.lock_conn();
                                    let unique = self.unique_slug_in_db(&conn, &candidate.record.slug, "-copy")?;
                                    drop(conn);
                                    candidate.record.slug = unique.clone();
                                    if let Some(Value::Object(obj)) = candidate.raw_payload.as_mut() {
                                        obj.insert("slug".into(), Value::String(unique));
                                    }
                                }
                                "overwrite" => {}
                                other => {
                                    report.errors.push(format!(
                                        "不支持的冲突策略：{}（仅支持 overwrite/rename/skip）",
                                        other
                                    ));
                                    continue;
                                }
                            }
                        }
                    }

                    match self.upsert_mode_candidate(candidate) {
                        Ok(_) => report.saved += 1,
                        Err(err) => report.errors.push(err.to_string()),
                    }
                }
                Ok(None) => {
                    report.skipped_due_to_missing_fields += 1;
                }
                Err(missing) => {
                    report.skipped_due_to_missing_fields += 1;
                    report
                        .errors
                        .push(format!("缺少字段: {}", missing.join(", ")));
                }
            }
        }
        Ok(report)
    }
}

fn parse_modes_document(text: &str) -> Result<Vec<YamlValue>, DbError> {
    parse_modes_document_with_hint(text, None)
}

fn parse_modes_document_with_hint(text: &str, path_hint: Option<&str>) -> Result<Vec<YamlValue>, DbError> {
    let yaml: Result<YamlValue, _> = serde_yaml::from_str(text);
    let doc = match yaml {
        Ok(value) => value,
        Err(_) => {
            let json: Value = serde_json::from_str(text)?;
            serde_yaml::to_value(json)?
        }
    };
    let hint = path_hint
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    Ok(match hint {
        Some(hint) => extract_mode_nodes_by_hint(&doc, hint),
        None => extract_mode_nodes(&doc),
    })
}

fn extract_mode_nodes(doc: &YamlValue) -> Vec<YamlValue> {
    if let Some(seq) = doc.get("customModes").and_then(|v| v.as_sequence()) {
        return seq.clone();
    }
    if let Some(seq) = doc.get("custom_modes").and_then(|v| v.as_sequence()) {
        return seq.clone();
    }
    if let Some(seq) = doc.as_sequence() {
        return seq.clone();
    }
    if doc.as_mapping().is_some() {
        // 单个 Mode 直接粘贴
        return vec![doc.clone()];
    }
    Vec::new()
}

fn extract_mode_nodes_by_hint(doc: &YamlValue, path_hint: &str) -> Vec<YamlValue> {
    let hint = path_hint.trim();
    if hint.is_empty() {
        return extract_mode_nodes(doc);
    }

    let tokens = hint
        .split('.')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>();

    // 若形如 settings.customModes[].slug，则定位到 customModes 数组即可
    if let Some((index, array_token)) = tokens.iter().enumerate().find(|(_, t)| t.ends_with("[]")) {
        let array_key = array_token.trim_end_matches("[]");
        let mut current = doc;
        for token in tokens.iter().take(index) {
            if let Some(next) = current.get(*token) {
                current = next;
            } else {
                current = doc;
                break;
            }
        }
        if let Some(seq) = current.get(array_key).and_then(|v| v.as_sequence()) {
            return seq.clone();
        }
    }

    // 逐层 key 下降，直到拿到 sequence 或 mapping
    let mut current = doc;
    for token in &tokens {
        let key = token.trim_end_matches("[]");
        if let Some(next) = current.get(key) {
            current = next;
            if token.ends_with("[]") {
                break;
            }
        } else {
            return extract_mode_nodes(doc);
        }
    }

    if let Some(seq) = current.as_sequence() {
        return seq.clone();
    }
    extract_mode_nodes(current)
}

fn node_slug_fallback(node: &YamlValue) -> String {
    node.as_mapping()
        .and_then(|map| map_get(map, "slug").and_then(|v| v.as_str().map(|s| s.to_string())))
        .unwrap_or_default()
}

fn node_name_fallback(node: &YamlValue) -> String {
    node.as_mapping()
        .and_then(|map| map_get(map, "name").and_then(|v| v.as_str().map(|s| s.to_string())))
        .unwrap_or_default()
}

fn candidate_from_document_node(
    node: &YamlValue,
    source: &str,
    source_alias: Option<String>,
    source_path: Option<String>,
) -> Result<Option<ModeUpsertCandidate>, Vec<String>> {
    let Some(map) = node.as_mapping() else {
        return Ok(None);
    };

    let slug = yaml_string(map_get(map, "slug")).unwrap_or_default();
    let name = yaml_string(map_get(map, "name")).unwrap_or_default();
    let description = yaml_string(map_get(map, "description")).unwrap_or_default();
    let role_definition = yaml_string(map_get(map, "roleDefinition")).unwrap_or_default();
    let groups = extract_groups(map_get(map, "groups"));
    let config_source = yaml_string(map_get(map, "source")).unwrap_or_default();

    let mut missing = Vec::new();
    if slug.trim().is_empty() {
        missing.push("slug".to_string());
    }
    if name.trim().is_empty() {
        missing.push("name".to_string());
    }
    if description.trim().is_empty() {
        missing.push("description".to_string());
    }
    if role_definition.trim().is_empty() {
        missing.push("roleDefinition".to_string());
    }
    if groups.is_empty() {
        missing.push("groups".to_string());
    }
    if config_source.trim().is_empty() {
        missing.push("source".to_string());
    }
    if !missing.is_empty() {
        return Err(missing);
    }

    let when_to_use = yaml_string(map_get(map, "whenToUse"));
    let custom_instructions = yaml_string(map_get(map, "customInstructions"));
    let raw_payload = Some(serde_json::to_value(node).map_err(|_| vec!["rawPayload".to_string()])?);
    let content_hash = AppDatabase::compute_content_hash_from_fields(
        &description,
        &role_definition,
        &groups,
        when_to_use.as_ref(),
        custom_instructions.as_ref(),
    );

    Ok(Some(ModeUpsertCandidate {
        record: ModeRecord {
            id: Uuid::new_v4().to_string(),
            slug,
            name,
            description,
            groups,
            role_definition: role_definition.clone(),
            role_definition_length: role_definition.chars().count() as i64,
            source: source.to_string(),
            when_to_use,
            custom_instructions,
            payload: Some(json!({ "configSource": config_source })),
            updated_at: Utc::now().to_rfc3339(),
            hash: content_hash.clone(),
        },
        raw_payload,
        source_path,
        source_alias,
        content_hash,
    }))
}

impl AppDatabase {
    fn find_mode_by_slug(conn: &Connection, slug: &str) -> Result<Option<ModeRecord>, DbError> {
        let mut stmt = conn.prepare(
            "SELECT id, slug, name, description, groups, role_definition, role_definition_length, source, when_to_use, custom_instructions, payload, updated_at, COALESCE(content_hash, hash) AS hash
             FROM modes WHERE slug = ?1 LIMIT 1",
        )?;
        let row = stmt
            .query_row([slug], |row| {
                let groups: String = row.get("groups")?;
                let payload: Option<String> = row.get("payload")?;
                Ok(ModeRecord {
                    id: row.get("id")?,
                    slug: row.get("slug")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    groups: serde_json::from_str(&groups).unwrap_or_default(),
                    role_definition: row.get("role_definition")?,
                    role_definition_length: row.get("role_definition_length")?,
                    source: row.get("source")?,
                    when_to_use: row.get("when_to_use")?,
                    custom_instructions: row.get("custom_instructions")?,
                    payload: match payload {
                        Some(raw) => Some(serde_json::from_str(&raw).unwrap_or(Value::Null)),
                        None => None,
                    },
                    updated_at: row.get("updated_at")?,
                    hash: row.get("hash")?,
                })
            })
            .optional()?;
        Ok(row)
    }
}

fn unique_slug_in_yaml(items: &[YamlValue], base: &str, suffix: &str) -> String {
    let mut candidate = format!("{}{}", base, suffix);
    let mut counter = 2;
    while items.iter().any(|item| {
        item.as_mapping()
            .and_then(|map| map_get(map, "slug").and_then(|v| v.as_str()))
            .map(|slug| slug == candidate)
            .unwrap_or(false)
    }) {
        candidate = format!("{}{}-{}", base, suffix, counter);
        counter += 1;
    }
    candidate
}

fn read_mode_slugs_from_instance_path(path: &str) -> Result<Vec<String>, DbError> {
    let Some(resolved) = expand_home_path(path) else {
        return Ok(Vec::new());
    };
    if !resolved.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(resolved)?;
    let yaml: YamlValue = serde_yaml::from_str(&content)?;
    let Some(sequence) = yaml.get("customModes").and_then(|v| v.as_sequence()) else {
        return Ok(Vec::new());
    };
    Ok(sequence
        .iter()
        .filter_map(|node| {
            node.as_mapping()
                .and_then(|map| map_get(map, "slug"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect())
}

fn mode_history_from_row(row: &rusqlite::Row<'_>) -> Result<ModeHistoryRecord, rusqlite::Error> {
    let before_raw: Option<String> = row.get("before_payload")?;
    let after_raw: Option<String> = row.get("after_payload")?;
    Ok(ModeHistoryRecord {
        id: row.get("id")?,
        mode_id: row.get("mode_id")?,
        instance_id: row.get("instance_id")?,
        instance_alias: row.get("instance_alias")?,
        action: row.get("action")?,
        before_payload: before_raw.and_then(|raw| serde_json::from_str(&raw).ok()),
        after_payload: after_raw.and_then(|raw| serde_json::from_str(&raw).ok()),
        created_at: row.get("created_at")?,
    })
}

struct KnownInstanceTemplate {
    alias: &'static str,
    kind: &'static str,
    path: &'static str,
}

#[cfg(target_os = "windows")]
fn known_ide_templates() -> &'static [KnownInstanceTemplate] {
    &[
        KnownInstanceTemplate {
            alias: "KiloCode - VSCode 主版",
            kind: "kilocode",
            path: "%APPDATA%/Code/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "KiloCode - VSCode Insiders",
            kind: "kilocode",
            path: "%APPDATA%/Code - Insiders/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "KiloCode - Cursor",
            kind: "kilocode",
            path: "%APPDATA%/Cursor/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "RooCode - VSCode 主版",
            kind: "roocode",
            path: "%APPDATA%/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "RooCode - VSCode Insiders",
            kind: "roocode",
            path: "%APPDATA%/Code - Insiders/User/globalStorage/rooveterinaryinc.roo-cline/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "RooCode - Cursor",
            kind: "roocode",
            path: "%APPDATA%/Cursor/User/globalStorage/rooveterinaryinc.roo-cline/settings/custom_modes.yaml",
        },
    ]
}

#[cfg(target_os = "macos")]
fn known_ide_templates() -> &'static [KnownInstanceTemplate] {
    &[
        KnownInstanceTemplate {
            alias: "KiloCode - VSCode 主版",
            kind: "kilocode",
            path: "~/Library/Application Support/Code/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "KiloCode - Trae 国服",
            kind: "kilocode",
            path: "~/Library/Application Support/Trae CN/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "KiloCode - Trae 国际版",
            kind: "kilocode",
            path: "~/Library/Application Support/Trae/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "RooCode - VSCode 主版",
            kind: "roocode",
            path: "~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "RooCode - Trae 国服",
            kind: "roocode",
            path: "~/Library/Application Support/Trae CN/User/globalStorage/rooveterinaryinc.roo-cline/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "RooCode - Trae 国际版",
            kind: "roocode",
            path: "~/Library/Application Support/Trae/User/globalStorage/rooveterinaryinc.roo-cline/settings/custom_modes.yaml",
        },
    ]
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn known_ide_templates() -> &'static [KnownInstanceTemplate] {
    &[
        KnownInstanceTemplate {
            alias: "KiloCode - VSCode 主版",
            kind: "kilocode",
            path: "~/.config/Code/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml",
        },
        KnownInstanceTemplate {
            alias: "RooCode - VSCode 主版",
            kind: "roocode",
            path: "~/.config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/custom_modes.yaml",
        },
    ]
}

fn expand_home_path(path: &str) -> Option<PathBuf> {
    if path.contains("%APPDATA%") {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Some(PathBuf::from(path.replace("%APPDATA%", &appdata)));
        }
    }
    if path.contains("%LOCALAPPDATA%") {
        if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(path.replace("%LOCALAPPDATA%", &appdata)));
        }
    }
    if path.starts_with("~/") {
        home_dir().map(|home| home.join(path.trim_start_matches("~/")))
    } else {
        Some(Path::new(path).to_path_buf())
    }
}

fn build_custom_modes_path(root: &Path, extension_id: &str) -> PathBuf {
    root.join("User")
        .join("globalStorage")
        .join(extension_id)
        .join("settings")
        .join("custom_modes.yaml")
}

fn discover_ide_instances_in_dir(base_dir: &Path) -> Vec<(String, String, String)> {
    let mut results = Vec::new();
    let entries = match fs::read_dir(base_dir) {
        Ok(entries) => entries,
        Err(_) => return results,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry
            .file_name()
            .to_string_lossy()
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }

        let kilo_path = build_custom_modes_path(&path, "kilocode.kilo-code");
        if kilo_path.exists() {
            results.push((
                format!("KiloCode - {}", name),
                "kilocode".to_string(),
                kilo_path.to_string_lossy().to_string(),
            ));
        }
        let roo_path = build_custom_modes_path(&path, "rooveterinaryinc.roo-cline");
        if roo_path.exists() {
            results.push((
                format!("RooCode - {}", name),
                "roocode".to_string(),
                roo_path.to_string_lossy().to_string(),
            ));
        }
    }

    results
}

#[cfg(target_os = "windows")]
fn discover_ide_instances() -> Vec<(String, String, String)> {
    let Ok(appdata) = std::env::var("APPDATA") else {
        return Vec::new();
    };
    discover_ide_instances_in_dir(Path::new(&appdata))
}

#[cfg(target_os = "macos")]
fn discover_ide_instances() -> Vec<(String, String, String)> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    discover_ide_instances_in_dir(&home.join("Library").join("Application Support"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn discover_ide_instances() -> Vec<(String, String, String)> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    discover_ide_instances_in_dir(&home.join(".config"))
}

fn map_get<'a>(map: &'a YamlMapping, key: &str) -> Option<&'a YamlValue> {
    map.get(&YamlValue::String(key.to_string()))
}

fn yaml_string(value: Option<&YamlValue>) -> Option<String> {
    value.and_then(|v| match v {
        YamlValue::String(s) => Some(s.clone()),
        YamlValue::Number(num) => Some(num.to_string()),
        YamlValue::Bool(flag) => Some(flag.to_string()),
        _ => None,
    })
}

fn extract_groups(value: Option<&YamlValue>) -> Vec<String> {
    let mut groups = Vec::new();
    if let Some(YamlValue::Sequence(seq)) = value {
        for item in seq {
            match item {
                YamlValue::String(text) => groups.push(text.clone()),
                YamlValue::Sequence(inner) => {
                    if let Some(YamlValue::String(text)) = inner.first() {
                        groups.push(text.clone());
                    }
                }
                YamlValue::Mapping(map) => {
                    if let Some(YamlValue::String(text)) = map.get(&YamlValue::String("name".into())) {
                        groups.push(text.clone());
                    }
                }
                _ => {}
            }
        }
    }
    groups
}
