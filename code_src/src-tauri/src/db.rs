use chrono::Utc;
use dirs_next::home_dir;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use serde_yaml::{Mapping as YamlMapping, Value as YamlValue};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{fs, sync::Mutex};
use tauri::{AppHandle, Manager};
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
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
}

#[derive(Debug)]
pub struct AppDatabase {
    conn: Mutex<Connection>,
}

#[derive(Default)]
struct ModeImportResult {
    discovered: usize,
    saved: usize,
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
    path TEXT NOT NULL UNIQUE,
    last_scan_at TEXT,
    modes_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    selected_for_sync INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_ide_instances_path ON ide_instances(path);
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
        for template in known_ide_templates() {
            let existing = self.find_instance_by_path(template.path)?;
            let path_display = template.path.to_string();
            let resolved = expand_home_path(&path_display);
            let exists = resolved.map(|p| p.exists()).unwrap_or(false);
            let status = if exists { "synced" } else { "missing" }.to_string();
            let import_result = if exists {
                match self.import_modes_from_file(template.alias, &path_display) {
                    Ok(result) => result,
                    Err(err) => {
                        eprintln!("导入模式失败: {} => {}", template.path, err);
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
                    .unwrap_or_else(|| template.alias.to_string()),
                kind: template.kind.to_string(),
                path: path_display.clone(),
                last_scan_at: Some(now.clone()),
                modes_count: existing.as_ref().map(|item| item.modes_count).unwrap_or(0),
                status,
                selected: existing.as_ref().map(|item| item.selected).unwrap_or(false),
            };
            if exists {
                record.modes_count = import_result.discovered as i64;
            }
            record = self.upsert_ide_instance(record)?;
            synced.push(record);
        }
        Ok(synced)
    }

    fn import_modes_from_file(&self, alias: &str, path: &str) -> Result<ModeImportResult, DbError> {
        let mut report = ModeImportResult::default();
        let Some(resolved) = expand_home_path(path) else {
            return Ok(report);
        };
        if !resolved.exists() {
            return Ok(report);
        }
        let content = fs::read_to_string(resolved)?;
        let yaml: YamlValue = serde_yaml::from_str(&content)?;
        let Some(sequence) = yaml.get("customModes").and_then(|v| v.as_sequence()) else {
            return Ok(report);
        };

        for node in sequence {
            report.discovered += 1;
            match Self::mode_from_yaml(alias, path, node) {
                Ok(Some(record)) => {
                    if self.upsert_mode(record).is_ok() {
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

    fn mode_from_yaml(alias: &str, source_path: &str, value: &YamlValue) -> Result<Option<ModeRecord>, DbError> {
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
        let source_field = yaml_string(map_get(map, "source"));
        let payload_value = {
            let mut json_value = serde_json::to_value(value)?;
            if let Value::Object(obj) = &mut json_value {
                obj.insert("__sourcePath".into(), json!(source_path));
                obj.insert("__sourceAlias".into(), json!(alias));
            }
            Some(json_value)
        };

        let mut hasher = Sha256::new();
        hasher.update(slug.as_bytes());
        hasher.update(role_definition.as_bytes());
        hasher.update(description.as_bytes());
        if let Some(custom) = custom_instructions.as_ref() {
            hasher.update(custom.as_bytes());
        }
        let hash = hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        Ok(Some(ModeRecord {
            id: Uuid::new_v4().to_string(),
            slug,
            name,
            description,
            groups,
            role_definition: role_definition.clone(),
            role_definition_length: role_definition.chars().count() as i64,
            source: source_field.unwrap_or_else(|| format!("ide:{}", alias)),
            when_to_use,
            custom_instructions,
                payload: payload_value,
            updated_at: Utc::now().to_rfc3339(),
            hash,
        }))
    }
}

struct KnownInstanceTemplate {
    alias: &'static str,
    kind: &'static str,
    path: &'static str,
}

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

fn expand_home_path(path: &str) -> Option<PathBuf> {
    if path.starts_with("~/") {
        home_dir().map(|home| home.join(path.trim_start_matches("~/")))
    } else {
        Some(Path::new(path).to_path_buf())
    }
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
