use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, sync::Mutex};
use tauri::AppHandle;
use thiserror::Error;
use uuid::Uuid;

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
}

#[derive(Debug)]
pub struct AppDatabase {
    conn: Mutex<Connection>,
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
pub struct GithubRuleRecord {
    pub id: String,
    pub name: String,
    pub query: String,
    pub path_hint: String,
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
        let dir = handle.path_resolver().app_data_dir().ok_or(DbError::ResolvePath)?;
        fs::create_dir_all(&dir)?;
        let db_path = dir.join("kilo_modes.db");
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "foreign_keys", &"ON")?;
        Self::run_migrations(&conn)?;
        Self::seed_if_empty(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
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
    updated_at TEXT NOT NULL,
    hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS github_rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    query TEXT NOT NULL,
    path_hint TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    delay_sec INTEGER NOT NULL DEFAULT 3,
    last_run_at TEXT
);

CREATE TABLE IF NOT EXISTS ide_instances (
    id TEXT PRIMARY KEY,
    alias TEXT NOT NULL,
    kind TEXT NOT NULL,
    path TEXT NOT NULL,
    last_scan_at TEXT,
    modes_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    selected_for_sync INTEGER NOT NULL DEFAULT 0
);
"#,
        )?;
        Ok(())
    }

    fn seed_if_empty(conn: &Connection) -> Result<(), DbError> {
        let mode_count: i64 = conn.query_row("SELECT COUNT(*) FROM modes", [], |row| row.get(0))?;
        if mode_count == 0 {
            let groups_json = serde_json::to_string(&vec!["审查", "安全"])?;
            conn.execute(
                "INSERT INTO modes (id, slug, name, description, groups, role_definition, role_definition_length, source, when_to_use, custom_instructions, payload, updated_at, hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, NULL, ?9, ?10)",
                params![
                    Uuid::new_v4().to_string(),
                    "mode-review-pro",
                    "高级 Code Review",
                    "针对大型仓库的安全审查模式",
                    groups_json,
                    "请以安全专家视角审查代码变更……",
                    1200,
                    "github",
                    "2024-03-21T12:00:00Z",
                    "hash-sample-1"
                ],
            )?;
        }

        let rule_count: i64 = conn.query_row("SELECT COUNT(*) FROM github_rules", [], |row| row.get(0))?;
        if rule_count == 0 {
            conn.execute(
                "INSERT INTO github_rules (id, name, query, path_hint, enabled, delay_sec, last_run_at)
                 VALUES (?1, ?2, ?3, ?4, 1, 3, ?5)",
                params![
                    Uuid::new_v4().to_string(),
                    "默认热门模式",
                    "topic:kilocode-mode stars:>10",
                    "customModes[].slug",
                    "2024-03-21T20:00:00Z"
                ],
            )?;
        }

        let instance_count: i64 = conn.query_row("SELECT COUNT(*) FROM ide_instances", [], |row| row.get(0))?;
        if instance_count == 0 {
            conn.execute(
                "INSERT INTO ide_instances (id, alias, kind, path, last_scan_at, modes_count, status, selected_for_sync)
                 VALUES (?1, ?2, ?3, ?4, ?5, 12, 'synced', 1)",
                params![
                    Uuid::new_v4().to_string(),
                    "KiloCode - VSCode 主版",
                    "kilocode",
                    "~/Library/Application Support/Code/User/globalStorage/kilocode.kilo-code/settings/custom_modes.yaml",
                    "2024-03-21T12:00:00Z"
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
            "SELECT id, slug, name, description, groups, role_definition, role_definition_length, source, when_to_use, custom_instructions, payload, updated_at, hash
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

    pub fn upsert_mode(&self, mut record: ModeRecord) -> Result<ModeRecord, DbError> {
        if record.id.trim().is_empty() {
            record.id = Uuid::new_v4().to_string();
        }
        let conn = self.lock_conn();
        let groups_json = serde_json::to_string(&record.groups)?;
        let payload_json = record
            .payload
            .as_ref()
            .map(|value| serde_json::to_string(value))
            .transpose()?;

        conn.execute(
            "INSERT INTO modes (id, slug, name, description, groups, role_definition, role_definition_length, source, when_to_use, custom_instructions, payload, updated_at, hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
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
                updated_at=excluded.updated_at,
                hash=excluded.hash",
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
                record.updated_at,
                record.hash
            ],
        )?;
        Ok(record)
    }

    pub fn list_github_rules(&self) -> Result<Vec<GithubRuleRecord>, DbError> {
        let conn = self.lock_conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, query, path_hint, enabled, delay_sec, last_run_at FROM github_rules ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(GithubRuleRecord {
                id: row.get("id")?,
                name: row.get("name")?,
                query: row.get("query")?,
                path_hint: row.get("path_hint")?,
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
        let conn = self.lock_conn();
        conn.execute(
            "INSERT INTO github_rules (id, name, query, path_hint, enabled, delay_sec, last_run_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name,
                query=excluded.query,
                path_hint=excluded.path_hint,
                enabled=excluded.enabled,
                delay_sec=excluded.delay_sec,
                last_run_at=excluded.last_run_at",
            params![
                record.id,
                record.name,
                record.query,
                record.path_hint,
                if record.enabled { 1 } else { 0 },
                record.delay_sec,
                record.last_run_at
            ],
        )?;
        Ok(record)
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
}
