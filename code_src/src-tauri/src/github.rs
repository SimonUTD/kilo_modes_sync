use crate::db::{AppDatabase, ModeRecord};
use chrono::Utc;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum GithubSyncError {
    #[error("缺少 GitHub Token")]
    MissingToken,
    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),
    #[error("解析 JSON 失败: {0}")]
    Json(#[from] serde_json::Error),
    #[error("导入数据库失败: {0}")]
    Database(#[from] crate::db::DbError),
    #[error("未找到任何模式文件")]
    NoModeFound,
}

#[derive(Debug, Deserialize)]
struct GithubSearchResponse {
    items: Vec<GithubFileItem>,
}

#[derive(Debug, Deserialize)]
struct GithubFileItem {
    name: String,
    path: String,
    repository: GithubRepository,
    #[serde(default)]
    score: f64,
}

#[derive(Debug, Deserialize)]
struct GithubRepository {
    full_name: String,
    url: String,
}

#[derive(Debug)]
pub struct GithubSyncConfig {
    pub token: String,
    pub query: String,
    pub path_hint: String,
    pub delay_sec: u64,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubSyncResult {
    pub fetched_files: usize,
    pub saved_modes: usize,
    pub skipped_due_to_missing_fields: usize,
    pub errors: Vec<String>,
}

pub async fn sync_from_github(
    config: GithubSyncConfig,
    db: &AppDatabase,
) -> Result<GithubSyncResult, GithubSyncError> {
    if config.token.trim().is_empty() {
        return Err(GithubSyncError::MissingToken);
    }
    let mut result = GithubSyncResult {
        fetched_files: 0,
        saved_modes: 0,
        skipped_due_to_missing_fields: 0,
        errors: Vec::new(),
    };

    let client = build_client(config.proxy.clone())?;
    let search_url = format!(
        "https://api.github.com/search/code?q={}&per_page=10",
        urlencoding::encode(&config.query)
    );
    let search_resp = client
        .get(&search_url)
        .header("User-Agent", "kilo-roo-sync")
        .header("Authorization", format!("token {}", config.token))
        .send()
        .await?
        .error_for_status()?
        .json::<GithubSearchResponse>()
        .await?;

    if search_resp.items.is_empty() {
        return Err(GithubSyncError::NoModeFound);
    }

    result.fetched_files = search_resp.items.len();

    for item in search_resp.items {
        let file_url = format!(
            "https://raw.githubusercontent.com/{}/main/{}",
            item.repository.full_name, item.path
        );
        let raw_resp = client
            .get(&file_url)
            .header("User-Agent", "kilo-roo-sync")
            .send()
            .await?;

        if !raw_resp.status().is_success() {
            result.errors.push(format!("无法下载 {}: {}", file_url, raw_resp.status()));
            continue;
        }
        let content = raw_resp.text().await?;
        match parse_modes_from_text(&content, &item.repository.full_name, &item.path) {
            Ok(mode_list) => {
                for mode in mode_list {
                    match db.upsert_mode(mode) {
                        Ok(_) => result.saved_modes += 1,
                        Err(err) => result.errors.push(format!("写入数据库失败 {}: {}", item.path, err)),
                    }
                }
            }
            Err(err) => {
                result.errors.push(format!("解析 {} 失败: {}", item.path, err));
                result.skipped_due_to_missing_fields += 1;
            }
        }
        sleep(Duration::from_secs(config.delay_sec)).await;
    }

    Ok(result)
}

fn build_client(proxy: Option<String>) -> Result<Client, reqwest::Error> {
    let mut builder = ClientBuilder::new().use_rustls_tls();
    if let Some(proxy_url) = proxy {
        builder = builder.proxy(reqwest::Proxy::all(&proxy_url)?);
    }
    builder.build()
}

fn parse_modes_from_text(
    content: &str,
    repo_name: &str,
    file_path: &str,
) -> Result<Vec<ModeRecord>, serde_yaml::Error> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(content)?;
    let Some(sequence) = yaml.get("customModes").and_then(|v| v.as_sequence()) else {
        return Ok(Vec::new());
    };
    let mut list = Vec::new();
    for node in sequence {
        if let Some(map) = node.as_mapping() {
            let slug = map
                .get(&serde_yaml::Value::String("slug".into()))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let name = map
                .get(&serde_yaml::Value::String("name".into()))
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| slug.as_str())
                .to_string();
            let description = map
                .get(&serde_yaml::Value::String("description".into()))
                .and_then(|v| v.as_str())
                .unwrap_or("来自 GitHub 的模式")
                .to_string();
            let role_definition = map
                .get(&serde_yaml::Value::String("roleDefinition".into()))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if slug.is_empty() || role_definition.is_empty() {
                continue;
            }
            let groups = map
                .get(&serde_yaml::Value::String("groups".into()))
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            let when_to_use = map
                .get(&serde_yaml::Value::String("whenToUse".into()))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let custom_instructions = map
                .get(&serde_yaml::Value::String("customInstructions".into()))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut hasher = Sha256::new();
            hasher.update(slug.as_bytes());
            hasher.update(role_definition.as_bytes());
            let hash = hasher
                .finalize()
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();

            let payload = json!({
                "__repo": repo_name,
                "__path": file_path,
                "__source": "github"
            });

            list.push(ModeRecord {
                id: Uuid::new_v4().to_string(),
                slug,
                name,
                description,
                groups,
                role_definition: role_definition.clone(),
                role_definition_length: role_definition.chars().count() as i64,
                source: format!("github:{}", repo_name),
                when_to_use,
                custom_instructions,
                payload: Some(payload),
                updated_at: Utc::now().to_rfc3339(),
                hash,
            });
        }
    }
    Ok(list)
}
