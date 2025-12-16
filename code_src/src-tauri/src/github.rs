use crate::db::AppDatabase;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::{sleep, Duration};

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
    #[error("GitHub API 返回错误: {0}")]
    Api(String),
    #[error("未找到任何模式文件")]
    NoModeFound,
}

#[derive(Debug, Deserialize)]
struct GithubSearchResponse {
    items: Vec<GithubFileItem>,
}

#[derive(Debug, Deserialize)]
struct GithubFileItem {
    path: String,
    repository: GithubRepository,
}

#[derive(Debug, Deserialize)]
struct GithubRepository {
    full_name: String,
}

#[derive(Debug)]
pub struct GithubSyncConfig {
    pub token: String,
    pub query: String,
    pub path_hint: String,
    pub delay_sec: u64,
    pub proxy: Option<String>,
    pub rule_id: Option<String>,
    pub rule_name: Option<String>,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubSyncResult {
    pub fetched_files: usize,
    pub saved_modes: usize,
    pub skipped_due_to_missing_fields: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubTokenTestResult {
    pub ok: bool,
    pub status: u16,
    pub remaining: Option<i64>,
    pub reset_at: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct GithubRateLimitResponse {
    resources: GithubRateLimitResources,
}

#[derive(Debug, Deserialize)]
struct GithubRateLimitResources {
    core: GithubRateLimitCore,
}

#[derive(Debug, Deserialize)]
struct GithubRateLimitCore {
    remaining: i64,
    reset: i64,
}

pub async fn test_github_token(
    token: String,
    proxy: Option<String>,
) -> Result<GithubTokenTestResult, GithubSyncError> {
    if token.trim().is_empty() {
        return Err(GithubSyncError::MissingToken);
    }
    let client = build_client(proxy)?;
    let resp = client
        .get("https://api.github.com/rate_limit")
        .header("User-Agent", "kilo-roo-sync")
        .header("Authorization", format!("token {}", token))
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Ok(GithubTokenTestResult {
            ok: false,
            status: status.as_u16(),
            remaining: None,
            reset_at: None,
            message: if body.trim().is_empty() {
                format!("Token 校验失败（HTTP {}）", status)
            } else {
                format!("Token 校验失败（HTTP {}）: {}", status, body)
            },
        });
    }

    let data = resp.json::<GithubRateLimitResponse>().await?;
    let reset_at = chrono::DateTime::<chrono::Utc>::from_timestamp(data.resources.core.reset, 0)
        .map(|dt| dt.to_rfc3339());
    Ok(GithubTokenTestResult {
        ok: true,
        status: status.as_u16(),
        remaining: Some(data.resources.core.remaining),
        reset_at,
        message: "Token 可用，可调用 GitHub API".to_string(),
    })
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
    let mut aggregated: Vec<GithubFileItem> = Vec::new();
    for page in 1..=5 {
        let search_url = format!(
            "https://api.github.com/search/code?q={}&per_page=20&page={}",
            urlencoding::encode(&config.query),
            page
        );
        let resp = client
            .get(&search_url)
            .header("User-Agent", "kilo-roo-sync")
            .header("Authorization", format!("token {}", config.token))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let message = if body.trim().is_empty() {
                format!("搜索失败（HTTP {}）：{}", status.as_u16(), search_url)
            } else {
                format!("搜索失败（HTTP {}）：{} => {}", status.as_u16(), search_url, body)
            };
            let _ = db.add_sync_log(
                "github",
                config.rule_id.as_deref(),
                config.rule_name.as_deref().or(Some(&config.query)),
                Some(&search_url),
                "error",
                Some(&message),
            );
            return Err(GithubSyncError::Api(message));
        }
        let search_resp = resp.json::<GithubSearchResponse>().await?;
        if search_resp.items.is_empty() {
            break;
        }
        aggregated.extend(search_resp.items);
        if aggregated.len() >= 100 {
            break;
        }
        sleep(Duration::from_secs(config.delay_sec)).await;
    }

    if aggregated.is_empty() {
        return Err(GithubSyncError::NoModeFound);
    }
    result.fetched_files = aggregated.len();

    for item in aggregated {
        let file_url = match resolve_download_url(&client, &config.token, &config.branch, &item).await {
            Ok(url) => url,
            Err(err) => {
                let message = format!("解析下载地址失败 {}: {}", item.path, err);
                result.errors.push(message.clone());
                let _ = db.add_sync_log(
                    "github",
                    config.rule_id.as_deref(),
                    config.rule_name.as_deref().or(Some(&config.query)),
                    Some(&item.path),
                    "error",
                    Some(&message),
                );
                continue;
            }
        };
        let raw_resp = client
            .get(&file_url)
            .header("User-Agent", "kilo-roo-sync")
            .send()
            .await?;

        if !raw_resp.status().is_success() {
            let message = format!("无法下载 {}: {}", file_url, raw_resp.status());
            result.errors.push(message.clone());
            let _ = db.add_sync_log(
                "github",
                config.rule_id.as_deref(),
                config.rule_name.as_deref().or(Some(&config.query)),
                Some(&file_url),
                "error",
                Some(&message),
            );
            continue;
        }

        let content = raw_resp.text().await?;
        match db.import_modes_from_text_scoped_with_hint_and_strategy(
            &content,
            "github",
            Some(item.repository.full_name.clone()),
            Some(file_url.clone()),
            Some(&config.path_hint),
            "rename",
        ) {
            Ok(report) => {
                result.saved_modes += report.saved;
                result.skipped_due_to_missing_fields += report.skipped_due_to_missing_fields;
                let status = if report.errors.is_empty() { "success" } else { "warning" };
                for err in &report.errors {
                    result.errors.push(format!("{}: {}", item.path, err));
                }
                let message = format!(
                    "导入 {} 条，跳过 {} 条，重复 hash {} 条",
                    report.saved, report.skipped_due_to_missing_fields, report.duplicate_hash
                );
                let _ = db.add_sync_log(
                    "github",
                    config.rule_id.as_deref(),
                    config.rule_name.as_deref().or(Some(&config.query)),
                    Some(&file_url),
                    status,
                    Some(&message),
                );
            }
            Err(err) => {
                let message = format!("导入 {} 失败: {}", item.path, err);
                result.errors.push(message.clone());
                let _ = db.add_sync_log(
                    "github",
                    config.rule_id.as_deref(),
                    config.rule_name.as_deref().or(Some(&config.query)),
                    Some(&file_url),
                    "error",
                    Some(&message),
                );
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

#[derive(Debug, Deserialize)]
struct GithubContentResponse {
    download_url: Option<String>,
}

async fn resolve_download_url(
    client: &Client,
    token: &str,
    branch: &str,
    item: &GithubFileItem,
) -> Result<String, GithubSyncError> {
    let api_url = format!(
        "https://api.github.com/repos/{}/contents/{}?ref={}",
        item.repository.full_name, item.path
        , urlencoding::encode(branch)
    );
    let resp = client
        .get(&api_url)
        .header("User-Agent", "kilo-roo-sync")
        .header("Authorization", format!("token {}", token))
        .send()
        .await?;
    if resp.status().is_success() {
        let data = resp.json::<GithubContentResponse>().await?;
        if let Some(url) = data.download_url {
            return Ok(url);
        }
    }
    Ok(format!(
        "https://raw.githubusercontent.com/{}/{}/{}",
        item.repository.full_name, branch, item.path
    ))
}
