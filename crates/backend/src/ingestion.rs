use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, Read},
    path::{Component, Path, PathBuf},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use flate2::read::GzDecoder;
use parquet::file::reader::{FileReader, SerializedFileReader};
use serde_json::Value;
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::sync::{Mutex, OnceLock};
use tokio::io::AsyncWriteExt;
use url::Url;
use walkdir::WalkDir;
use zip::read::ZipArchive;

use crate::{db::Database, job::JobType, source::SourceType};

const DEFAULT_URL_CONNECT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_URL_TOTAL_TIMEOUT_SECS: u64 = 60;
const DEFAULT_URL_MAX_REDIRECTS: usize = 5;
const DEFAULT_URL_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DEFAULT_URL_ALLOWED_CONTENT_TYPES: &[&str] = &[
    "application/octet-stream",
    "audio/mpeg",
    "audio/mp4",
    "audio/x-m4a",
    "audio/wav",
    "audio/x-wav",
    "video/mp4",
    "video/x-matroska",
    "video/webm",
];

const MAX_ARCHIVE_FILES: usize = 10_000;
const MAX_ARCHIVE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationResult {
    pub source_id: uuid::Uuid,
    pub job_id: uuid::Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlImportFailureKind {
    Retryable,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlImportError {
    pub kind: UrlImportFailureKind,
    pub message: String,
}

impl UrlImportError {
    fn retryable(message: impl Into<String>) -> Self {
        Self {
            kind: UrlImportFailureKind::Retryable,
            message: message.into(),
        }
    }

    fn terminal(message: impl Into<String>) -> Self {
        Self {
            kind: UrlImportFailureKind::Terminal,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedMedia {
    pub stored_path: PathBuf,
    pub sha256: String,
    pub byte_size: u64,
    pub content_type: Option<String>,
    pub filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvImportSummary {
    pub imported: u32,
    pub duplicate: u32,
    pub invalid: u32,
    pub failed: u32,
    pub row_failures: Vec<RowFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowFailure {
    pub row: u32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderScanSummary {
    pub discovered: u32,
    pub created: u32,
    pub changed: u32,
    pub removed: u32,
    pub skipped: u32,
    pub source_id: uuid::Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct FolderScanConfig {
    root: PathBuf,
    recursive: bool,
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
    include_hidden: bool,
    follow_symlinks: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrackedFile {
    path: PathBuf,
    size: u64,
    modified_secs: i64,
    is_symlink: bool,
}

pub async fn register_local_file(
    database: &Database,
    project_id: uuid::Uuid,
    path: &Path,
) -> Result<RegistrationResult> {
    validate_regular_file(path)?;
    let normalized_identity = format!(
        "local_file:{}",
        path.to_string_lossy().trim().to_lowercase()
    );
    if let Some(source) = database
        .sources()
        .get_by_identity(project_id, &normalized_identity)
        .await?
    {
        if let Some(job) = database
            .jobs()
            .get_by_logical_key(project_id, Some(source.id), JobType::MediaIngestion)
            .await?
        {
            return Ok(RegistrationResult {
                source_id: source.id,
                job_id: job.id,
            });
        }
    }
    let source = database
        .sources()
        .create(
            project_id,
            SourceType::LocalFile,
            Some(path.to_string_lossy().as_ref()),
            None,
            None,
        )
        .await?;
    let job = database
        .jobs()
        .create(
            project_id,
            Some(source.id),
            JobType::MediaIngestion,
            0,
            Utc::now(),
        )
        .await?;
    Ok(RegistrationResult {
        source_id: source.id,
        job_id: job.id,
    })
}

pub async fn import_local_file_to_staging(
    database: &Database,
    source_id: uuid::Uuid,
    source_path: &Path,
    staging_dir: &Path,
) -> Result<(PathBuf, u64)> {
    fs::create_dir_all(staging_dir)?;
    let source = database
        .sources()
        .get(source_id)
        .await?
        .ok_or_else(|| anyhow!("source not found"))?;
    if let Some(import_metadata) = source.import_metadata.as_deref() {
        let existing = staging_path_from_metadata(import_metadata)?;
        if existing.exists() {
            let size = existing.metadata()?.len();
            return Ok((existing, size));
        }
    }
    let tmp_path = staging_dir.join(format!("{}.part", source_id));
    let final_path = staging_dir.join(format!("{}.bin", source_id));
    fs::copy(source_path, &tmp_path)?;
    fs::rename(&tmp_path, &final_path)?;
    let size = final_path.metadata()?.len();
    let metadata = format!(
        r#"{{"staged_path":"{}","bytes":{}}}"#,
        final_path.display(),
        size
    );
    let _ = database
        .sources()
        .update_import_metadata(source_id, Some(&metadata))
        .await?;
    let _ = database
        .sources()
        .update_ingestion_status(source_id, "staged")
        .await?;
    Ok((final_path, size))
}

pub async fn register_public_url(
    database: &Database,
    project_id: uuid::Uuid,
    url: &str,
) -> Result<RegistrationResult> {
    validate_public_url(url)?;
    let normalized_identity = format!("url:{}", normalize_public_url(url));
    let mut tx = database.pool().begin().await?;
    let source = match sqlx::query(
        r#"
        select id, project_id, source_type, original_url,
               normalized_identity, source_config, failure_reason, import_metadata,
               file_count, byte_count, ingestion_status, created_at, updated_at
        from sources
        where project_id = $1 and normalized_identity = $2
        "#,
    )
    .bind(project_id)
    .bind(&normalized_identity)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(row) => crate::source::source_from_row(&row)?,
        None => {
            let id = uuid::Uuid::new_v4();
            let row = sqlx::query(
                r#"
                insert into sources (
                    id, project_id, source_type, original_url,
                    normalized_identity, source_config, failure_reason, import_metadata,
                    file_count, byte_count, ingestion_status, created_at, updated_at
                )
                values ($1, $2, $3, $4, $5, null, null, null, 0, 0, 'pending', now(), now())
                on conflict (project_id, normalized_identity)
                do update set updated_at = excluded.updated_at
                returning id, project_id, source_type, original_url,
                          normalized_identity, source_config, failure_reason, import_metadata,
                          file_count, byte_count, ingestion_status, created_at, updated_at
                "#,
            )
            .bind(id)
            .bind(project_id)
            .bind(SourceType::PublicUrl.as_str())
            .bind(Some(url))
            .bind(&normalized_identity)
            .fetch_one(&mut *tx)
            .await?;
            crate::source::source_from_row(&row)?
        }
    };
    let job = match sqlx::query(
        r#"
        select id, project_id, source_id, job_type, status, priority,
               attempts, available_at, lease_expires_at, worker_id,
               failure_reason, created_at, updated_at
        from jobs
        where project_id = $1 and source_id = $2 and job_type = $3
        "#,
    )
    .bind(project_id)
    .bind(source.id)
    .bind(JobType::MediaIngestion.as_str())
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(row) => crate::job::job_from_row(&row)?,
        None => {
            let id = uuid::Uuid::new_v4();
            let row = sqlx::query(
                r#"
                insert into jobs (
                    id, project_id, source_id, job_type, status,
                    priority, attempts, available_at, lease_expires_at,
                    worker_id, failure_reason, created_at, updated_at
                )
                values ($1, $2, $3, $4, $5, 0, 0, now(), null, null, null, now(), now())
                on conflict (project_id, source_id, job_type)
                do update set updated_at = excluded.updated_at
                returning id, project_id, source_id, job_type, status, priority,
                          attempts, available_at, lease_expires_at, worker_id,
                          failure_reason, created_at, updated_at
                "#,
            )
            .bind(id)
            .bind(project_id)
            .bind(source.id)
            .bind(JobType::MediaIngestion.as_str())
            .bind("queued")
            .fetch_one(&mut *tx)
            .await?;
            crate::job::job_from_row(&row)?
        }
    };
    tx.commit().await?;
    Ok(RegistrationResult {
        source_id: source.id,
        job_id: job.id,
    })
}

pub async fn download_public_url_to_staging(
    _database: &Database,
    source_id: uuid::Uuid,
    url: &str,
    staging_dir: &Path,
) -> Result<DownloadedMedia, UrlImportError> {
    let temp_path = staging_dir.join(format!("{}.part", source_id));
    let final_path = staging_dir.join(format!("{}.bin", source_id));
    let result = async {
        tokio::fs::create_dir_all(staging_dir)
            .await
            .map_err(|err| UrlImportError::retryable(err.to_string()))?;
        if final_path.exists() {
            let size = tokio::fs::metadata(&final_path)
                .await
                .map_err(|err| UrlImportError::retryable(err.to_string()))?
                .len();
            return Ok(DownloadedMedia {
                stored_path: final_path,
                sha256: String::new(),
                byte_size: size,
                content_type: None,
                filename: safe_filename_from_url(url),
            });
        }
        let mut hasher = Sha256::new();
        let mut total = 0u64;
        let mut current_url = url.to_string();
        let mut redirects = 0usize;
        let mut resolved_filename = safe_filename_from_url(url);
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(default_url_total_timeout_secs());
        loop {
            if redirects > DEFAULT_URL_MAX_REDIRECTS {
                return Err(UrlImportError::terminal("too many redirects"));
            }
            let parsed =
                Url::parse(&current_url).map_err(|_| UrlImportError::terminal("invalid url"))?;
            let validation = validate_resolved_url(&parsed).await?;
            let req = validation
                .client
                .get(parsed.as_str())
                .timeout(Duration::from_secs(default_url_connect_timeout_secs()));
            let response = tokio::time::timeout_at(deadline, req.send())
                .await
                .map_err(|_| UrlImportError::retryable("download timed out"))?
                .map_err(classify_reqwest_error)?;
            if response.status().is_redirection() {
                let next = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| UrlImportError::terminal("redirect missing location"))?;
                let next_url = parsed
                    .join(next)
                    .map_err(|_| UrlImportError::terminal("invalid redirect target"))?;
                validate_public_url(next_url.as_str())
                    .map_err(|err| UrlImportError::terminal(err.to_string()))?;
                current_url = next_url.to_string();
                redirects += 1;
                continue;
            }
            if !response.status().is_success() {
                return Err(UrlImportError::terminal(format!(
                    "unexpected http status {}",
                    response.status()
                )));
            }
            let response_content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            validate_content_type(response_content_type.as_deref())
                .map_err(|err| UrlImportError::terminal(err.to_string()))?;
            if let Some(disposition) = response
                .headers()
                .get(reqwest::header::CONTENT_DISPOSITION)
                .and_then(|value| value.to_str().ok())
            {
                if let Some(filename) = disposition_filename(disposition) {
                    resolved_filename = filename;
                }
            }
            let mut response = response;
            let mut file = tokio::fs::File::create(&temp_path)
                .await
                .map_err(|err| UrlImportError::retryable(err.to_string()))?;
            while let Some(chunk) = tokio::time::timeout_at(deadline, response.chunk())
                .await
                .map_err(|_| UrlImportError::retryable("download timed out"))?
                .map_err(classify_reqwest_error)?
            {
                total = total
                    .checked_add(chunk.len() as u64)
                    .ok_or_else(|| UrlImportError::terminal("response too large"))?;
                if total > default_url_max_bytes() {
                    return Err(UrlImportError::terminal("response too large"));
                }
                hasher.update(&chunk);
                tokio::time::timeout_at(deadline, async {
                    tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await
                })
                .await
                .map_err(|_| UrlImportError::retryable("download timed out"))?
                .map_err(|err| UrlImportError::retryable(err.to_string()))?;
            }
            file.flush()
                .await
                .map_err(|err| UrlImportError::retryable(err.to_string()))?;
            tokio::fs::rename(&temp_path, &final_path)
                .await
                .map_err(|err| UrlImportError::retryable(err.to_string()))?;
            break Ok(DownloadedMedia {
                stored_path: final_path,
                sha256: format!("{:x}", hasher.finalize()),
                byte_size: total,
                content_type: response_content_type,
                filename: resolved_filename,
            });
        }
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temp_path).await;
    }
    result
}

pub async fn scan_folder(
    database: &Database,
    project_id: uuid::Uuid,
    root: &Path,
) -> Result<FolderScanSummary> {
    let root = validate_folder_root(root)?;
    let config = load_folder_scan_config(&root)?;
    let folder_source = ensure_folder_source(database, project_id, &config).await?;
    let existing_files = existing_folder_files(database, project_id, &config.root).await?;
    let mut seen = BTreeSet::new();
    let mut discovered = 0;
    let mut created = 0;
    let mut changed = 0;
    let mut skipped = 0;
    for tracked in walk_folder(&config)? {
        discovered += 1;
        let normalized_identity = local_file_identity(&tracked.path);
        seen.insert(normalized_identity.clone());
        match database
            .sources()
            .get_by_identity(project_id, &normalized_identity)
            .await?
        {
            Some(existing) => {
                if tracked_file_from_source(&existing) == Some(tracked.clone()) {
                    skipped += 1;
                    continue;
                }
                let source_config = tracked_file_config(&config, &tracked)?;
                let _ = database
                    .sources()
                    .update_source_config(existing.id, Some(&source_config))
                    .await?;
                let _ = database
                    .sources()
                    .update_counters(existing.id, tracked.size as i64, tracked.size as i64)
                    .await?;
                let _ = ensure_media_ingestion_job(database, project_id, existing.id).await?;
                changed += 1;
            }
            None => {
                let source_config = tracked_file_config(&config, &tracked)?;
                let source = database
                    .sources()
                    .create(
                        project_id,
                        SourceType::LocalFile,
                        Some(tracked.path.to_string_lossy().as_ref()),
                        Some(&source_config),
                        None,
                    )
                    .await?;
                let _ = database
                    .sources()
                    .update_counters(source.id, tracked.size as i64, tracked.size as i64)
                    .await?;
                let _ = ensure_media_ingestion_job(database, project_id, source.id).await?;
                created += 1;
            }
        }
    }
    let removed = existing_files
        .into_iter()
        .filter(|source| !seen.contains(&source.normalized_identity))
        .count() as u32;
    let summary = FolderScanSummary {
        discovered,
        created,
        changed,
        removed,
        skipped,
        source_id: folder_source.id,
    };
    let _ = database
        .sources()
        .update_import_metadata(
            folder_source.id,
            Some(&serde_json::to_string(&serde_json::json!({
                "source_id": summary.source_id.to_string(),
                "discovered": summary.discovered,
                "created": summary.created,
                "changed": summary.changed,
                "removed": summary.removed,
                "skipped": summary.skipped,
            }))?),
        )
        .await?;
    Ok(summary)
}

fn validate_folder_root(root: &Path) -> Result<PathBuf> {
    let root = root.canonicalize().context("folder must exist")?;
    let approved_roots = approved_roots()?;
    if approved_roots
        .iter()
        .any(|approved| root.starts_with(approved))
    {
        Ok(root)
    } else {
        Err(anyhow!("path is outside approved roots"))
    }
}

fn approved_roots() -> Result<Vec<PathBuf>> {
    let roots = std::env::var("DATA_STUDIO_FOLDER_SCAN_ROOTS")
        .ok()
        .or_else(|| std::env::var("DATA_STUDIO_DATA_DIR").ok())
        .or_else(|| std::env::var("DATA_STUDIO_WORKSPACE_DIR").ok())
        .unwrap_or_else(|| ".".to_string());
    roots
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            PathBuf::from(value)
                .canonicalize()
                .with_context(|| format!("approved root must exist: {value}"))
        })
        .collect()
}

fn load_folder_scan_config(root: &Path) -> Result<FolderScanConfig> {
    Ok(FolderScanConfig {
        root: root.to_path_buf(),
        recursive: true,
        include_patterns: parse_pattern_list("DATA_STUDIO_FOLDER_SCAN_INCLUDE"),
        exclude_patterns: parse_pattern_list("DATA_STUDIO_FOLDER_SCAN_EXCLUDE"),
        include_hidden: false,
        follow_symlinks: false,
    })
}

async fn ensure_folder_source(
    database: &Database,
    project_id: uuid::Uuid,
    config: &FolderScanConfig,
) -> Result<crate::source::Source> {
    let identity = format!(
        "local_folder:{}",
        config.root.to_string_lossy().trim().to_lowercase()
    );
    if let Some(source) = database
        .sources()
        .get_by_identity(project_id, &identity)
        .await?
    {
        return Ok(source);
    }
    database
        .sources()
        .create(
            project_id,
            SourceType::LocalFolder,
            Some(config.root.to_string_lossy().as_ref()),
            Some(&serde_json::to_string(config)?),
            None,
        )
        .await
}

async fn ensure_media_ingestion_job(
    database: &Database,
    project_id: uuid::Uuid,
    source_id: uuid::Uuid,
) -> Result<uuid::Uuid> {
    let job = database
        .jobs()
        .create(
            project_id,
            Some(source_id),
            JobType::MediaIngestion,
            0,
            Utc::now(),
        )
        .await?;
    Ok(job.id)
}

fn local_file_identity(path: &Path) -> String {
    format!(
        "local_file:{}",
        path.to_string_lossy().trim().to_lowercase()
    )
}

fn tracked_file_config(config: &FolderScanConfig, tracked: &TrackedFile) -> Result<String> {
    Ok(serde_json::to_string(&serde_json::json!({
        "scan_root": config.root,
        "path": tracked.path,
        "size": tracked.size,
        "modified_secs": tracked.modified_secs,
        "is_symlink": tracked.is_symlink,
    }))?)
}

fn tracked_file_from_source(source: &crate::source::Source) -> Option<TrackedFile> {
    let value: serde_json::Value = serde_json::from_str(source.source_config.as_ref()?).ok()?;
    Some(TrackedFile {
        path: PathBuf::from(value.get("path")?.as_str()?),
        size: value.get("size")?.as_u64()?,
        modified_secs: value.get("modified_secs")?.as_i64()?,
        is_symlink: value.get("is_symlink")?.as_bool()?,
    })
}

async fn existing_folder_files(
    database: &Database,
    project_id: uuid::Uuid,
    root: &Path,
) -> Result<Vec<crate::source::Source>> {
    let sources = database.sources().list(project_id).await?;
    Ok(sources
        .into_iter()
        .filter(|source| source.source_type == SourceType::LocalFile)
        .filter(|source| {
            source
                .source_config
                .as_deref()
                .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
                .and_then(|value| {
                    value
                        .get("scan_root")
                        .and_then(|v| v.as_str())
                        .map(PathBuf::from)
                })
                .map(|scan_root| scan_root == root)
                .unwrap_or(false)
        })
        .collect())
}

fn walk_folder(config: &FolderScanConfig) -> Result<Vec<TrackedFile>> {
    let mut results = Vec::new();
    let mut walker = WalkDir::new(&config.root).follow_links(config.follow_symlinks);
    if !config.recursive {
        walker = walker.max_depth(1);
    }
    for entry in walker {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path().to_path_buf();
        if !path.starts_with(&config.root) {
            continue;
        }
        let relative = path.strip_prefix(&config.root).unwrap_or(&path);
        if !config.include_hidden && is_hidden(relative) {
            continue;
        }
        if !matches_patterns(relative, &config.include_patterns, &config.exclude_patterns) {
            continue;
        }
        if !is_supported_media(&path) {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        let modified_secs = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|value| value.as_secs() as i64)
            .unwrap_or_default();
        results.push(TrackedFile {
            path,
            size: metadata.len(),
            modified_secs,
            is_symlink: metadata.file_type().is_symlink(),
        });
    }
    results.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(results)
}

fn is_hidden(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(part) => part.to_string_lossy().starts_with('.'),
        _ => false,
    })
}

fn parse_pattern_list(env_key: &str) -> Vec<String> {
    std::env::var(env_key)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn matches_patterns(path: &Path, includes: &[String], excludes: &[String]) -> bool {
    let relative = path.to_string_lossy();
    let included = includes.is_empty()
        || includes
            .iter()
            .any(|pattern| pattern_match(pattern, &relative));
    let excluded = excludes
        .iter()
        .any(|pattern| pattern_match(pattern, &relative));
    included && !excluded
}

fn pattern_match(pattern: &str, value: &str) -> bool {
    wildcard_match(pattern.as_bytes(), value.as_bytes())
}

fn wildcard_match(pattern: &[u8], value: &[u8]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }
    match pattern[0] {
        b'*' => (0..=value.len()).any(|index| wildcard_match(&pattern[1..], &value[index..])),
        b'?' => !value.is_empty() && wildcard_match(&pattern[1..], &value[1..]),
        ch => !value.is_empty() && ch == value[0] && wildcard_match(&pattern[1..], &value[1..]),
    }
}

pub async fn import_csv_urls(
    database: &Database,
    project_id: uuid::Uuid,
    csv_path: &Path,
) -> Result<CsvImportSummary> {
    let file = fs::File::open(csv_path)?;
    let mut reader = csv::Reader::from_reader(BufReader::new(file));
    let headers = reader.headers()?.clone();
    let url_index = headers
        .iter()
        .position(|h| h.trim().eq_ignore_ascii_case("url"))
        .ok_or_else(|| anyhow!("CSV must contain a url column"))?;
    let mut summary = CsvImportSummary {
        imported: 0,
        duplicate: 0,
        invalid: 0,
        failed: 0,
        row_failures: Vec::new(),
    };
    for (index, record) in reader.records().enumerate() {
        let record = match record {
            Ok(record) => record,
            Err(_) => {
                summary.failed += 1;
                summary.row_failures.push(RowFailure {
                    row: (index + 2) as u32,
                    reason: "invalid CSV row".to_string(),
                });
                continue;
            }
        };
        match record
            .get(url_index)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(url) => {
                if validate_public_url(url).is_err() {
                    summary.invalid += 1;
                    summary.row_failures.push(RowFailure {
                        row: (index + 2) as u32,
                        reason: "invalid url".to_string(),
                    });
                    continue;
                }
                let normalized_identity = format!("url:{}", normalize_public_url(url));
                let existed = database
                    .sources()
                    .get_by_identity(project_id, &normalized_identity)
                    .await?
                    .is_some();
                match register_public_url(database, project_id, url).await {
                    Ok(_) if existed => summary.duplicate += 1,
                    Ok(_) => summary.imported += 1,
                    Err(err) => {
                        summary.failed += 1;
                        summary.row_failures.push(RowFailure {
                            row: (index + 2) as u32,
                            reason: err.to_string(),
                        });
                    }
                }
            }
            None => {
                summary.invalid += 1;
                summary.row_failures.push(RowFailure {
                    row: (index + 2) as u32,
                    reason: "missing url".to_string(),
                });
            }
        }
    }
    Ok(summary)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetItemResult {
    pub source_id: uuid::Uuid,
    pub job_id: uuid::Uuid,
    pub metadata: Value,
}

pub async fn import_dataset_manifest(
    database: &Database,
    project_id: uuid::Uuid,
    manifest_path: &Path,
    audio_path_field: &str,
) -> Result<Vec<DatasetItemResult>> {
    let mut results = Vec::new();
    match manifest_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("csv") => {
            let mut reader = csv::Reader::from_path(manifest_path)?;
            let headers = reader.headers()?.clone();
            let audio_index = headers
                .iter()
                .position(|h| h.trim() == audio_path_field)
                .ok_or_else(|| anyhow!("manifest missing audio path field"))?;
            for record in reader.records() {
                let record = record?;
                if let Some(result) = register_manifest_record(
                    database,
                    project_id,
                    manifest_path.parent().unwrap_or_else(|| Path::new(".")),
                    &headers,
                    audio_index,
                    &record,
                )
                .await?
                {
                    results.push(result);
                }
            }
        }
        Some("jsonl") | Some("json") => {
            let file = fs::File::open(manifest_path)?;
            for line in BufReader::new(file).lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let record: Value = serde_json::from_str(&line)?;
                if let Some(result) = register_json_record(
                    database,
                    project_id,
                    manifest_path.parent().unwrap_or_else(|| Path::new(".")),
                    audio_path_field,
                    &record,
                )
                .await?
                {
                    results.push(result);
                }
            }
        }
        Some("parquet") => {
            let file = fs::File::open(manifest_path)?;
            let reader = SerializedFileReader::new(file)?;
            let iter = reader.get_row_iter(None)?;
            for row in iter {
                let row = row?;
                let record = row.to_json_value();
                if let Some(result) = register_json_record(
                    database,
                    project_id,
                    manifest_path.parent().unwrap_or_else(|| Path::new(".")),
                    audio_path_field,
                    &record,
                )
                .await?
                {
                    results.push(result);
                }
            }
        }
        _ => return Err(anyhow!("unsupported manifest format")),
    }
    Ok(results)
}

pub async fn preview_dataset_manifest(
    manifest_path: &Path,
    audio_path_field: &str,
) -> Result<CsvImportSummary> {
    let mut summary = CsvImportSummary {
        imported: 0,
        duplicate: 0,
        invalid: 0,
        failed: 0,
        row_failures: Vec::new(),
    };
    match manifest_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("csv") => {
            let mut reader = csv::Reader::from_path(manifest_path)?;
            let headers = reader.headers()?.clone();
            let audio_index = headers
                .iter()
                .position(|h| h.trim() == audio_path_field)
                .ok_or_else(|| anyhow!("manifest missing audio path field"))?;
            for (row_index, record) in reader.records().enumerate() {
                let row = (row_index + 2) as u32;
                match record {
                    Ok(record) => {
                        if record.get(audio_index).unwrap_or("").trim().is_empty() {
                            summary.invalid += 1;
                            summary.row_failures.push(RowFailure {
                                row,
                                reason: "missing audio path".to_string(),
                            });
                        } else {
                            summary.imported += 1;
                        }
                    }
                    Err(_) => {
                        summary.failed += 1;
                        summary.row_failures.push(RowFailure {
                            row,
                            reason: "invalid CSV row".to_string(),
                        });
                    }
                }
            }
        }
        Some("jsonl") | Some("json") => {
            let file = fs::File::open(manifest_path)?;
            for (row_index, line) in BufReader::new(file).lines().enumerate() {
                let row = (row_index + 1) as u32;
                match line {
                    Ok(line) if !line.trim().is_empty() => {
                        match serde_json::from_str::<Value>(&line) {
                            Ok(record) if record.as_object().is_some() => {
                                let path = record
                                    .get(audio_path_field)
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("")
                                    .trim();
                                if path.is_empty() {
                                    summary.invalid += 1;
                                    summary.row_failures.push(RowFailure {
                                        row,
                                        reason: "missing audio path".to_string(),
                                    });
                                } else {
                                    summary.imported += 1;
                                }
                            }
                            _ => {
                                summary.failed += 1;
                                summary.row_failures.push(RowFailure {
                                    row,
                                    reason: "invalid JSON row".to_string(),
                                });
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {
                        summary.failed += 1;
                        summary.row_failures.push(RowFailure {
                            row,
                            reason: "invalid manifest row".to_string(),
                        });
                    }
                }
            }
        }
        Some("parquet") => {
            let file = fs::File::open(manifest_path)?;
            let reader = SerializedFileReader::new(file)?;
            let iter = reader.get_row_iter(None)?;
            for row in iter {
                let row = row?;
                let record = row.to_json_value();
                let path = record
                    .get(audio_path_field)
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .trim();
                if path.is_empty() {
                    summary.invalid += 1;
                    summary.row_failures.push(RowFailure {
                        row: summary.imported + summary.invalid + summary.failed + 1,
                        reason: "missing audio path".to_string(),
                    });
                } else {
                    summary.imported += 1;
                }
            }
        }
        _ => return Err(anyhow!("unsupported manifest format")),
    }
    Ok(summary)
}

pub async fn import_archive_dataset(
    database: &Database,
    project_id: uuid::Uuid,
    archive_path: &Path,
    selected_manifest: Option<&Path>,
    audio_path_field: &str,
    workspace_dir: &Path,
) -> Result<Vec<DatasetItemResult>> {
    let extract_dir = extract_archive(archive_path, workspace_dir).await?;
    let result = async {
        let manifest_path = match selected_manifest {
            Some(path) if path.is_absolute() => path.to_path_buf(),
            Some(path) => extract_dir.join(path),
            None => find_supported_manifest(&extract_dir)?,
        };
        import_dataset_manifest(database, project_id, &manifest_path, audio_path_field).await
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&extract_dir).await;
    result
}

async fn register_manifest_record(
    database: &Database,
    project_id: uuid::Uuid,
    root: &Path,
    headers: &csv::StringRecord,
    audio_index: usize,
    record: &csv::StringRecord,
) -> Result<Option<DatasetItemResult>> {
    let mut metadata = serde_json::Map::new();
    for (i, header) in headers.iter().enumerate() {
        if i != audio_index {
            metadata.insert(
                header.to_string(),
                Value::String(record.get(i).unwrap_or_default().to_string()),
            );
        }
    }
    let path = record.get(audio_index).unwrap_or("").trim();
    if path.is_empty() {
        return Ok(None);
    }
    register_manifest_path(database, project_id, root, path, Value::Object(metadata)).await
}

async fn register_json_record(
    database: &Database,
    project_id: uuid::Uuid,
    root: &Path,
    audio_path_field: &str,
    record: &Value,
) -> Result<Option<DatasetItemResult>> {
    let obj = match record.as_object() {
        Some(obj) => obj,
        None => return Ok(None),
    };
    let path = obj
        .get(audio_path_field)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let mut metadata = obj.clone();
    metadata.remove(audio_path_field);
    if path.is_empty() {
        return Ok(None);
    }
    register_manifest_path(database, project_id, root, &path, Value::Object(metadata)).await
}

async fn register_manifest_path(
    database: &Database,
    project_id: uuid::Uuid,
    root: &Path,
    relative_path: &str,
    metadata: Value,
) -> Result<Option<DatasetItemResult>> {
    let audio_path = root.join(relative_path);
    if !audio_path.exists() {
        return Ok(None);
    }
    let normalized_identity = format!(
        "existing_dataset:{}",
        audio_path.to_string_lossy().to_lowercase()
    );
    let source = match database
        .sources()
        .get_by_identity(project_id, &normalized_identity)
        .await?
    {
        Some(existing) => {
            if existing.import_metadata.as_deref() != Some(&metadata.to_string()) {
                let _ = database
                    .sources()
                    .update_import_metadata(existing.id, Some(&metadata.to_string()))
                    .await?;
            }
            existing
        }
        None => {
            database
                .sources()
                .create(
                    project_id,
                    SourceType::ExistingDataset,
                    Some(audio_path.to_string_lossy().as_ref()),
                    None,
                    Some(&metadata.to_string()),
                )
                .await?
        }
    };
    let job = match database
        .jobs()
        .get_by_logical_key(project_id, Some(source.id), JobType::MediaIngestion)
        .await?
    {
        Some(existing) => existing,
        None => {
            database
                .jobs()
                .create(
                    project_id,
                    Some(source.id),
                    JobType::MediaIngestion,
                    0,
                    Utc::now(),
                )
                .await?
        }
    };
    Ok(Some(DatasetItemResult {
        source_id: source.id,
        job_id: job.id,
        metadata,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveFormat {
    Zip,
    Tar,
    TarGz,
}

pub async fn extract_archive(archive_path: &Path, output_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(output_dir)?;
    let archive_name = archive_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("archive");
    let extract_dir = output_dir.join(format!("extract-{}", archive_name));
    fs::create_dir_all(&extract_dir)?;
    let result = match detect_archive_format(archive_path)? {
        ArchiveFormat::Zip => {
            let file = fs::File::open(archive_path)?;
            let mut archive = ZipArchive::new(file)?;
            extract_zip(&mut archive, &extract_dir)
        }
        ArchiveFormat::Tar => {
            let file = fs::File::open(archive_path)?;
            let mut archive = tar::Archive::new(file);
            extract_tar(&mut archive, &extract_dir)
        }
        ArchiveFormat::TarGz => {
            let file = fs::File::open(archive_path)?;
            let decoder = GzDecoder::new(file);
            let mut archive = tar::Archive::new(decoder);
            extract_tar(&mut archive, &extract_dir)
        }
    };
    if result.is_err() {
        let _ = fs::remove_dir_all(&extract_dir);
    }
    result?;
    Ok(extract_dir)
}

fn detect_archive_format(archive_path: &Path) -> Result<ArchiveFormat> {
    let mut file = fs::File::open(archive_path)?;
    let mut magic = [0u8; 4];
    let read = file.read(&mut magic)?;
    let ext = archive_path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_ascii_lowercase());
    if read >= 4 && magic == [0x50, 0x4b, 0x03, 0x04] {
        return Ok(ArchiveFormat::Zip);
    }
    if read >= 2 && magic.starts_with(&[0x1f, 0x8b]) {
        return Ok(ArchiveFormat::TarGz);
    }
    match ext.as_deref() {
        Some("tar") => Ok(ArchiveFormat::Tar),
        Some("tgz") => Ok(ArchiveFormat::TarGz),
        _ => Err(anyhow!("unsupported archive format")),
    }
}

fn extract_zip<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    output_dir: &Path,
) -> Result<()> {
    let mut file_count = 0usize;
    let mut total_size = 0u64;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.encrypted() {
            return Err(anyhow!("encrypted archives are not supported"));
        }
        let out = safe_join(output_dir, file.name())?;
        if file.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        file_count += 1;
        if file_count > MAX_ARCHIVE_FILES {
            return Err(anyhow!("archive exceeds file count limit"));
        }
        total_size = total_size
            .checked_add(file.size())
            .ok_or_else(|| anyhow!("archive exceeds size limit"))?;
        if total_size > MAX_ARCHIVE_BYTES {
            return Err(anyhow!("archive exceeds size limit"));
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut outfile = fs::File::create(out)?;
        std::io::copy(&mut file, &mut outfile)?;
    }
    Ok(())
}

fn extract_tar<R: std::io::Read>(archive: &mut tar::Archive<R>, output_dir: &Path) -> Result<()> {
    let mut file_count = 0usize;
    let mut total_size = 0u64;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let out = safe_join(output_dir, path.to_string_lossy().as_ref())?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if !entry_type.is_file() {
            return Err(anyhow!("unsupported archive entry"));
        }
        file_count += 1;
        if file_count > MAX_ARCHIVE_FILES {
            return Err(anyhow!("archive exceeds file count limit"));
        }
        total_size = total_size
            .checked_add(entry.header().size()?)
            .ok_or_else(|| anyhow!("archive exceeds size limit"))?;
        if total_size > MAX_ARCHIVE_BYTES {
            return Err(anyhow!("archive exceeds size limit"));
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        entry.unpack(out)?;
    }
    Ok(())
}

fn safe_join(base: &Path, relative: &str) -> Result<PathBuf> {
    let mut normalized = PathBuf::from(base);
    for component in Path::new(relative).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::ParentDir => {
                if !normalized.pop() || !normalized.starts_with(base) {
                    return Err(anyhow!("archive entry attempted path traversal"));
                }
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(anyhow!("archive entry attempted path traversal"));
            }
        }
    }
    if !normalized.starts_with(base) {
        return Err(anyhow!("archive entry attempted path traversal"));
    }
    Ok(normalized)
}

fn validate_public_url(url: &str) -> Result<()> {
    let parsed = Url::parse(url.trim()).map_err(|_| anyhow!("invalid url"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err(anyhow!("url must start with http:// or https://")),
    }
    if let Some(host) = parsed.host_str() {
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            if is_blocked_ip(&ip) && !is_allowed_test_loopback(&ip) {
                return Err(anyhow!("url target is not allowed"));
            }
        }
    }
    Ok(())
}

fn default_url_connect_timeout_secs() -> u64 {
    std::env::var("DATA_STUDIO_URL_CONNECT_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_URL_CONNECT_TIMEOUT_SECS)
}

fn default_url_total_timeout_secs() -> u64 {
    std::env::var("DATA_STUDIO_URL_TOTAL_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_URL_TOTAL_TIMEOUT_SECS)
}

fn default_url_max_bytes() -> u64 {
    std::env::var("DATA_STUDIO_URL_MAX_BYTES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_URL_MAX_BYTES)
}

struct ValidatedConnection {
    client: reqwest::Client,
}

async fn validate_resolved_url(url: &Url) -> Result<ValidatedConnection, UrlImportError> {
    let host = url
        .host_str()
        .ok_or_else(|| UrlImportError::terminal("url missing host"))?
        .to_string();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| UrlImportError::terminal("url missing port"))?;
    let addrs = tokio::time::timeout(
        Duration::from_secs(default_url_connect_timeout_secs()),
        tokio::net::lookup_host((host.as_str(), port)),
    )
    .await
    .map_err(|_| UrlImportError::retryable("dns lookup timed out"))?
    .map_err(|err| UrlImportError::retryable(err.to_string()))?
    .collect::<Vec<_>>();
    if addrs.is_empty() {
        return Err(UrlImportError::retryable("no dns results"));
    }
    if addrs
        .iter()
        .any(|addr| is_blocked_ip(&addr.ip()) && !is_allowed_test_loopback(&addr.ip()))
    {
        return Err(UrlImportError::terminal("url target is not allowed"));
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(&host, &addrs)
        .build()
        .map_err(|err| UrlImportError::retryable(err.to_string()))?;
    Ok(ValidatedConnection { client })
}

fn classify_reqwest_error(err: reqwest::Error) -> UrlImportError {
    if err.is_timeout() || err.is_connect() || err.is_request() {
        UrlImportError::retryable(err.to_string())
    } else if err.is_status() {
        UrlImportError::terminal(err.to_string())
    } else {
        UrlImportError::retryable(err.to_string())
    }
}

fn validate_content_type(content_type: Option<&str>) -> Result<()> {
    let content_type = match content_type {
        Some(value) => value.split(';').next().unwrap_or(value).trim(),
        None => return Ok(()),
    };
    if DEFAULT_URL_ALLOWED_CONTENT_TYPES
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(content_type))
    {
        Ok(())
    } else {
        Err(anyhow!("content type not allowed"))
    }
}

fn safe_filename_from_url(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .map(|value| value.to_string())
        })
        .filter(|value| !value.trim().is_empty())
        .map(|value| sanitize_filename(&value))
        .unwrap_or_else(|| "download.bin".to_string())
}

fn disposition_filename(disposition: &str) -> Option<String> {
    disposition
        .split(';')
        .find_map(|part| {
            let part = part.trim();
            part.strip_prefix("filename=").map(|value| {
                value
                    .trim_matches('"')
                    .split('/')
                    .next_back()
                    .unwrap_or(value)
                    .to_string()
            })
        })
        .map(|value| sanitize_filename(&value))
}

fn sanitize_filename(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect::<String>();
    if sanitized.is_empty() {
        sanitized = "download.bin".to_string();
    }
    sanitized
}

fn is_blocked_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_unspecified()
        }
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
                || ip.is_unspecified()
        }
    }
}

#[cfg(test)]
fn test_loopback_allowlist() -> &'static Mutex<std::collections::HashSet<std::net::IpAddr>> {
    static TEST_LOOPBACK_ALLOWLIST: OnceLock<Mutex<std::collections::HashSet<std::net::IpAddr>>> =
        OnceLock::new();
    TEST_LOOPBACK_ALLOWLIST.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

#[cfg(test)]
pub fn register_test_loopback(ip: std::net::IpAddr) {
    if ip.is_loopback() {
        let _ = test_loopback_allowlist().lock().unwrap().insert(ip);
    }
}

#[cfg(test)]
fn is_allowed_test_loopback(ip: &std::net::IpAddr) -> bool {
    ip.is_loopback() && test_loopback_allowlist().lock().unwrap().contains(ip)
}

#[cfg(not(test))]
fn is_allowed_test_loopback(_: &std::net::IpAddr) -> bool {
    false
}

pub fn validate_regular_file(path: &Path) -> Result<()> {
    let metadata =
        fs::metadata(path).with_context(|| format!("path does not exist: {}", path.display()))?;
    if !metadata.is_file() {
        return Err(anyhow!("path is not a regular file: {}", path.display()));
    }
    Ok(())
}

pub fn is_supported_media(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()).map(|value| value.to_ascii_lowercase()),
        Some(ext) if matches!(ext.as_str(), "wav" | "mp3" | "m4a" | "mp4" | "mkv" | "webm" | "flac" | "ogg" | "aac")
    )
}

fn normalize_public_url(url: &str) -> String {
    let trimmed = url.trim();
    if let Some(rest) = trimmed.strip_prefix("https://youtu.be/") {
        return format!("youtube:{}", rest.trim_matches('/'));
    }
    if let Some(rest) = trimmed.strip_prefix("https://www.youtube.com/watch?v=") {
        return format!("youtube:{}", rest.split('&').next().unwrap_or(rest));
    }
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    without_query.trim_end_matches('/').to_lowercase()
}

fn staging_path_from_metadata(metadata: &str) -> Result<PathBuf> {
    let value: serde_json::Value = serde_json::from_str(metadata)?;
    let path = value
        .get("staged_path")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("missing staged_path"))?;
    Ok(PathBuf::from(path))
}

fn find_supported_manifest(root: &Path) -> Result<PathBuf> {
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        match path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("csv") | Some("jsonl") | Some("json") | Some("parquet") => {
                return Ok(path.to_path_buf())
            }
            _ => {}
        }
    }
    Err(anyhow!("no supported manifest found"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use axum::{
        body::Body,
        http::{HeaderMap, HeaderValue, Uri},
        response::{IntoResponse, Redirect},
        routing::get,
        Router,
    };
    use flate2::{write::GzEncoder, Compression};
    use parquet::{
        data_type::{ByteArray, ByteArrayType},
        file::{properties::WriterProperties, writer::SerializedFileWriter},
        schema::parser::parse_message_type,
    };
    use std::sync::OnceLock;
    use std::{env, fs, fs::File, io::Write, path::PathBuf, sync::Arc};
    use uuid::Uuid;
    use zip::read::ZipArchive;

    static DB_TEST_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

    async fn lock_db_tests() -> tokio::sync::MutexGuard<'static, ()> {
        DB_TEST_LOCK
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await
    }

    async fn db_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
        lock_db_tests().await
    }

    async fn url_import_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
        static URL_IMPORT_TEST_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        URL_IMPORT_TEST_LOCK
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await
    }

    fn clear_env() {
        for key in [
            "DATA_STUDIO_DATABASE_URL",
            "DATA_STUDIO_WORKSPACE_DIR",
            "DATA_STUDIO_DATA_DIR",
            "DATA_STUDIO_TEMP_DIR",
            "DATA_STUDIO_HOST",
            "DATA_STUDIO_PORT",
        ] {
            std::env::remove_var(key);
        }
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ingestion_manifest_test_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    async fn test_pool() -> sqlx::PgPool {
        let database_url = env::var("DATA_STUDIO_TEST_DATABASE_URL")
            .or_else(|_| env::var("DATA_STUDIO_DATABASE_URL"))
            .unwrap_or_else(|_| {
                "postgresql://postgres:postgres@localhost:55432/data_studio".to_string()
            });
        let schema_name = format!("ingestion_manifest_test_{}", Uuid::new_v4().simple());
        let admin_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("failed to connect to test database");
        sqlx::query(&format!("create schema {}", schema_name))
            .execute(&admin_pool)
            .await
            .expect("failed to create test schema");
        drop(admin_pool);
        let schema_database_url = format!("{database_url}?options=-csearch_path%3D{schema_name}");
        migrations::run(&schema_database_url)
            .await
            .expect("failed to run migrations");
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&schema_database_url)
            .await
            .expect("failed to connect to schema-scoped test database")
    }

    async fn seed_project(pool: &sqlx::PgPool) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into projects (id, name, description, created_at, updated_at)
            values ($1, $2, $3, now(), now())
            "#,
        )
        .bind(id)
        .bind("Research")
        .bind(Option::<String>::None)
        .execute(pool)
        .await
        .expect("project should insert");
        id
    }

    async fn seed_public_source(pool: &sqlx::PgPool, project_id: Uuid, url: &str) -> Uuid {
        let source_id = Uuid::new_v4();
        sqlx::query(
            r#"
            insert into sources (
                id, project_id, source_type, original_url, normalized_identity,
                source_config, failure_reason, import_metadata, file_count,
                byte_count, ingestion_status, created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, null, null, null, 0, 0, 'pending', now(), now())
            "#,
        )
        .bind(source_id)
        .bind(project_id)
        .bind("public_url")
        .bind(Some(url))
        .bind(format!("url:{}", normalize_public_url(url)))
        .execute(pool)
        .await
        .expect("source should insert");
        source_id
    }

    async fn start_http_server() -> std::net::SocketAddr {
        async fn file() -> impl IntoResponse {
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            (headers, Body::from("hello world"))
        }

        async fn html() -> impl IntoResponse {
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("text/html"),
            );
            (headers, Body::from("<html></html>"))
        }

        async fn big() -> impl IntoResponse {
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            (headers, Body::from(vec![b'a'; 32]))
        }

        async fn slow() -> impl IntoResponse {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            file().await
        }

        async fn redirect(uri: Uri) -> impl IntoResponse {
            let target = uri.query().unwrap_or("/file");
            Redirect::temporary(target)
        }

        let app = Router::new()
            .route("/file", get(file))
            .route("/html", get(html))
            .route("/big", get(big))
            .route("/slow", get(slow))
            .route("/redirect", get(redirect));
        let listener = tokio::net::TcpListener::bind("127.0.0.2:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    fn write_parquet_fixture(path: &Path) {
        let schema = Arc::new(
            parse_message_type(
                r#"
                message dataset {
                    REQUIRED BYTE_ARRAY audio_path (UTF8);
                    OPTIONAL BYTE_ARRAY speaker (UTF8);
                    OPTIONAL BYTE_ARRAY note (UTF8);
                }
                "#,
            )
            .expect("schema"),
        );
        let file = File::create(path).expect("fixture file");
        let props = Arc::new(WriterProperties::builder().build());
        let mut writer = SerializedFileWriter::new(file, schema, props).expect("writer");
        let mut row_group = writer.next_row_group().expect("row group");

        let mut audio_col = row_group.next_column().expect("col").expect("audio col");
        audio_col
            .typed::<ByteArrayType>()
            .write_batch(
                &[
                    ByteArray::from("audio/a.wav"),
                    ByteArray::from("audio/b.wav"),
                ],
                None,
                None,
            )
            .expect("audio data");
        audio_col.close().expect("close audio");

        let mut speaker_col = row_group.next_column().expect("col").expect("speaker col");
        speaker_col
            .typed::<ByteArrayType>()
            .write_batch(
                &[ByteArray::from("spk-1"), ByteArray::from("spk-2")],
                Some(&[1, 1]),
                None,
            )
            .expect("speaker data");
        speaker_col.close().expect("close speaker");

        let mut note_col = row_group.next_column().expect("col").expect("note col");
        note_col
            .typed::<ByteArrayType>()
            .write_batch(
                &[ByteArray::from("first"), ByteArray::from("second")],
                Some(&[1, 1]),
                None,
            )
            .expect("note data");
        note_col.close().expect("close note");

        row_group.close().expect("close row group");
        writer.close().expect("close file");
    }

    #[tokio::test]
    async fn csv_manifest_import_is_incremental_and_idempotent() {
        clear_env();
        let _guard = db_test_guard().await;
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let temp_dir = temp_dir();
        let audio = temp_dir.join("audio.wav");
        fs::write(&audio, b"audio").expect("audio");
        let manifest = temp_dir.join("manifest.csv");
        fs::write(
            &manifest,
            "audio_path,speaker,notes\n\
             audio.wav,spk-1,hello\n",
        )
        .expect("manifest");

        let results = import_dataset_manifest(&database, project_id, &manifest, "audio_path")
            .await
            .expect("csv import");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata["speaker"], "spk-1");
        assert_eq!(results[0].metadata["notes"], "hello");
        assert!(database
            .sources()
            .get(results[0].source_id)
            .await
            .unwrap()
            .is_some());
        assert!(database
            .jobs()
            .get(results[0].job_id)
            .await
            .unwrap()
            .is_some());

        let repeated = import_dataset_manifest(&database, project_id, &manifest, "audio_path")
            .await
            .expect("csv import again");
        assert_eq!(repeated.len(), 1);
        let source_count = database.sources().list(project_id).await.unwrap().len();
        let job_count = database.jobs().list().await.unwrap().len();
        assert_eq!(source_count, 1);
        assert_eq!(job_count, 1);
    }

    #[tokio::test]
    async fn csv_manifest_preview_reports_row_failures_without_writing_rows() {
        clear_env();
        let _guard = db_test_guard().await;
        let pool = test_pool().await;
        let temp_dir = temp_dir();
        let manifest = temp_dir.join("preview.csv");
        fs::write(&manifest, "audio_path,title\nexists.wav,Hello\n,Missing\n").expect("manifest");

        let preview = preview_dataset_manifest(&manifest, "audio_path")
            .await
            .expect("preview");
        assert_eq!(preview.imported, 1);
        assert_eq!(preview.invalid, 1);
        assert_eq!(preview.failed, 0);
        assert_eq!(preview.row_failures.len(), 1);
        let count = sqlx::query_scalar::<_, i64>("select count(*) from sources")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn jsonl_manifest_import_preserves_extra_metadata_and_reports_missing_files() {
        clear_env();
        let _guard = db_test_guard().await;
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let temp_dir = temp_dir();
        fs::write(temp_dir.join("present.wav"), b"audio").expect("audio");
        let manifest = temp_dir.join("manifest.jsonl");
        fs::write(
            &manifest,
            r#"{"audio":"present.wav","speaker":"spk-1","extra":{"kind":"field"}}"#.to_string()
                + "\n"
                + r#"{"audio":"missing.wav","speaker":"spk-2"}"#,
        )
        .expect("manifest");

        let results = import_dataset_manifest(&database, project_id, &manifest, "audio")
            .await
            .expect("jsonl import");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata["speaker"], "spk-1");
        assert_eq!(results[0].metadata["extra"]["kind"], "field");
    }

    #[tokio::test]
    async fn parquet_manifest_import_preserves_metadata() {
        clear_env();
        let _guard = db_test_guard().await;
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let temp_dir = temp_dir();
        fs::create_dir_all(temp_dir.join("audio")).expect("dir");
        fs::write(temp_dir.join("audio/a.wav"), b"audio").expect("audio");
        let manifest = temp_dir.join("manifest.parquet");
        write_parquet_fixture(&manifest);

        let results = import_dataset_manifest(&database, project_id, &manifest, "audio_path")
            .await
            .expect("parquet import");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata["speaker"], "spk-1");
        assert_eq!(results[0].metadata["note"], "first");
    }

    #[tokio::test]
    async fn folder_scan_rejects_paths_outside_approved_roots() {
        clear_env();
        let _guard = db_test_guard().await;
        let temp_dir = temp_dir();
        std::env::set_var(
            "DATA_STUDIO_FOLDER_SCAN_ROOTS",
            temp_dir.to_string_lossy().to_string(),
        );
        let outside = std::env::temp_dir().join(format!("outside_{}", Uuid::new_v4()));
        fs::create_dir_all(&outside).expect("outside dir");
        let err = validate_folder_root(&outside).unwrap_err();
        assert!(err.to_string().contains("approved roots"));
    }

    #[tokio::test]
    async fn folder_scan_is_recursive_idempotent_and_persists_summary() {
        clear_env();
        let _guard = db_test_guard().await;
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let temp_dir = temp_dir();
        std::env::set_var(
            "DATA_STUDIO_FOLDER_SCAN_ROOTS",
            temp_dir.to_string_lossy().to_string(),
        );
        std::env::set_var("DATA_STUDIO_FOLDER_SCAN_EXCLUDE", "*/skip.wav");
        let nested = temp_dir.join("nested");
        fs::create_dir_all(&nested).expect("nested dir");
        fs::write(temp_dir.join("keep.wav"), b"keep").expect("keep");
        fs::write(nested.join("child.mp3"), b"child").expect("child");
        fs::write(nested.join("skip.wav"), b"skip").expect("skip");

        let summary = scan_folder(&database, project_id, &temp_dir)
            .await
            .expect("scan");
        assert_eq!(summary.discovered, 2);
        assert_eq!(summary.created, 2);
        assert_eq!(summary.changed, 0);
        assert_eq!(summary.removed, 0);
        assert_eq!(summary.skipped, 0);

        let repeated = scan_folder(&database, project_id, &temp_dir)
            .await
            .expect("repeat scan");
        assert_eq!(repeated.created, 0);
        assert_eq!(repeated.changed, 0);
        assert_eq!(repeated.discovered, 2);

        let sources = database.sources().list(project_id).await.expect("sources");
        assert_eq!(
            sources
                .iter()
                .filter(|source| source.source_type == SourceType::LocalFile)
                .count(),
            2
        );
        assert!(sources
            .iter()
            .any(|source| source.source_type == SourceType::LocalFolder));
        let jobs = database.jobs().list().await.expect("jobs");
        assert_eq!(jobs.len(), 2);
    }

    #[tokio::test]
    async fn archive_import_cleans_up_and_reuses_existing_sources() {
        clear_env();
        let _guard = db_test_guard().await;
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let temp_dir = temp_dir();
        let audio_root = temp_dir.join("dataset");
        fs::create_dir_all(&audio_root).expect("audio dir");
        fs::write(audio_root.join("clip.wav"), b"audio").expect("audio");
        let manifest_path = audio_root.join("manifest.jsonl");
        fs::write(&manifest_path, r#"{"audio":"clip.wav","speaker":"spk-1"}"#).expect("manifest");

        let archive_root = temp_dir.join("archives");
        fs::create_dir_all(&archive_root).expect("archive dir");
        let zip_path = archive_root.join("dataset.zip");
        {
            let file = File::create(&zip_path).expect("zip file");
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("manifest.jsonl", options).expect("manifest");
            zip.write_all(br#"{"audio":"clip.wav","speaker":"spk-1"}"#)
                .expect("write manifest");
            zip.start_file("clip.wav", options).expect("clip");
            zip.write_all(b"audio").expect("write audio");
            zip.finish().expect("finish zip");
        }

        let workspace_dir = temp_dir.join("workspace");
        let first = import_archive_dataset(
            &database,
            project_id,
            &zip_path,
            None,
            "audio",
            &workspace_dir,
        )
        .await
        .expect("archive import");
        assert_eq!(first.len(), 1);
        assert!(!workspace_dir.join("extract-dataset.zip").exists());

        let second = import_archive_dataset(
            &database,
            project_id,
            &zip_path,
            None,
            "audio",
            &workspace_dir,
        )
        .await
        .expect("archive import again");
        assert_eq!(second.len(), 1);

        assert_eq!(database.sources().list(project_id).await.unwrap().len(), 1);
        assert_eq!(database.jobs().list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn archive_import_supports_tar_and_tgz() {
        clear_env();
        let _guard = db_test_guard().await;
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let temp_dir = temp_dir();
        let archive_root = temp_dir.join("archives");
        fs::create_dir_all(&archive_root).expect("archive dir");

        let tar_path = archive_root.join("dataset.tar");
        {
            let file = File::create(&tar_path).expect("tar file");
            let mut builder = tar::Builder::new(file);
            let manifest = br#"{"audio":"clip.wav","speaker":"spk-1"}"#;
            let mut header = tar::Header::new_gnu();
            header.set_size(manifest.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, "manifest.jsonl", &manifest[..])
                .expect("manifest");
            let audio = b"audio";
            let mut header = tar::Header::new_gnu();
            header.set_size(audio.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, "clip.wav", &audio[..])
                .expect("audio");
            builder.finish().expect("finish tar");
        }

        let tgz_path = archive_root.join("dataset.tar.gz");
        {
            let file = File::create(&tgz_path).expect("tgz file");
            let encoder = GzEncoder::new(file, Compression::default());
            let mut builder = tar::Builder::new(encoder);
            let manifest = br#"{"audio":"clip.wav","speaker":"spk-1"}"#;
            let mut header = tar::Header::new_gnu();
            header.set_size(manifest.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, "manifest.jsonl", &manifest[..])
                .expect("manifest");
            let audio = b"audio";
            let mut header = tar::Header::new_gnu();
            header.set_size(audio.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, "clip.wav", &audio[..])
                .expect("audio");
            let encoder = builder.into_inner().expect("builder");
            encoder.finish().expect("finish tgz");
        }

        for archive in [&tar_path, &tgz_path] {
            let results = import_archive_dataset(
                &database,
                project_id,
                archive,
                None,
                "audio",
                temp_dir.as_path(),
            )
            .await
            .expect("archive import");
            assert_eq!(results.len(), 1);
            assert_eq!(database.sources().list(project_id).await.unwrap().len(), 1);
            assert_eq!(database.jobs().list().await.unwrap().len(), 1);
            sqlx::query("truncate table media_assets, sources, jobs restart identity cascade")
                .execute(database.pool())
                .await
                .expect("reset tables");
        }
    }

    #[tokio::test]
    async fn archive_path_traversal_is_rejected() {
        clear_env();
        let temp_dir = temp_dir();
        let zip_path = temp_dir.join("evil.zip");
        {
            let file = File::create(&zip_path).expect("zip file");
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("../evil.txt", options).expect("file");
            zip.write_all(b"boom").expect("write");
            zip.finish().expect("finish zip");
        }

        let file = File::open(&zip_path).expect("zip file");
        let mut archive = ZipArchive::new(file).expect("archive");
        let err = extract_zip(&mut archive, &temp_dir).unwrap_err();
        assert!(err.to_string().contains("path traversal"));
    }

    #[tokio::test]
    async fn archive_import_rejects_corrupt_and_unsupported_files_without_leaking_temp_dirs() {
        clear_env();
        let _guard = db_test_guard().await;
        let temp_dir = temp_dir();
        let workspace = temp_dir.join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let bad = temp_dir.join("bad.bin");
        fs::write(&bad, b"not an archive").expect("bad file");
        let err = extract_archive(&bad, &workspace).await.unwrap_err();
        assert!(err.to_string().contains("unsupported archive format"));
    }

    #[tokio::test]
    async fn archive_import_rejects_archive_bombs_by_file_count() {
        clear_env();
        let _guard = db_test_guard().await;
        let temp_dir = temp_dir();
        let archive_path = temp_dir.join("bomb.zip");
        {
            let file = File::create(&archive_path).expect("zip file");
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            for i in 0..(MAX_ARCHIVE_FILES + 1) {
                zip.start_file(format!("file-{i}.txt"), options)
                    .expect("file");
                zip.write_all(b"x").expect("write");
            }
            zip.finish().expect("finish");
        }
        let err = extract_archive(&archive_path, &temp_dir).await.unwrap_err();
        assert!(err.to_string().contains("file count limit"));
    }

    #[tokio::test]
    async fn public_url_download_follows_redirects_and_is_idempotent() {
        clear_env();
        let _url_guard = url_import_test_guard().await;
        let _guard = db_test_guard().await;
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let server = start_http_server().await;
        register_test_loopback(server.ip());
        let server_url = format!("http://{}", server);
        let source_id =
            seed_public_source(&pool, project_id, &format!("{server_url}/redirect?/file")).await;
        let staging = temp_dir();

        let first = download_public_url_to_staging(
            &database,
            source_id,
            &format!("{server_url}/redirect?/file"),
            &staging,
        )
        .await
        .expect("download");
        let second = download_public_url_to_staging(
            &database,
            source_id,
            &format!("{server_url}/redirect?/file"),
            &staging,
        )
        .await
        .expect("download again");

        assert_eq!(first.stored_path, second.stored_path);
        assert!(first.byte_size > 0);
        assert!(!first.sha256.is_empty());
        assert!(first.stored_path.exists());
    }

    #[tokio::test]
    async fn public_url_download_rejects_private_targets_and_invalid_content_type() {
        clear_env();
        let _url_guard = url_import_test_guard().await;
        let _guard = db_test_guard().await;
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let server = start_http_server().await;
        register_test_loopback(server.ip());
        let server_url = format!("http://{}", server);
        let private_source =
            seed_public_source(&pool, project_id, "http://127.0.0.1/private").await;
        let staging = temp_dir();

        let private_err = download_public_url_to_staging(
            &database,
            private_source,
            "http://127.0.0.1/private",
            &staging,
        )
        .await
        .unwrap_err();
        assert!(matches!(private_err.kind, UrlImportFailureKind::Terminal));

        let html_source =
            seed_public_source(&pool, project_id, &format!("{server_url}/html")).await;
        let html_err = download_public_url_to_staging(
            &database,
            html_source,
            &format!("{server_url}/html"),
            &staging,
        )
        .await
        .unwrap_err();
        assert!(matches!(html_err.kind, UrlImportFailureKind::Terminal));
    }

    #[tokio::test]
    async fn public_url_download_retries_on_timeout_and_cleans_up_partial_files() {
        clear_env();
        let _url_guard = url_import_test_guard().await;
        let _guard = db_test_guard().await;
        std::env::set_var("DATA_STUDIO_URL_TOTAL_TIMEOUT_SECS", "0");
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let server = start_http_server().await;
        register_test_loopback(server.ip());
        let server_url = format!("http://{}", server);
        let source_id = seed_public_source(&pool, project_id, &format!("{server_url}/slow")).await;
        let staging = temp_dir();

        let err = download_public_url_to_staging(
            &database,
            source_id,
            &format!("{server_url}/slow"),
            &staging,
        )
        .await
        .unwrap_err();
        assert!(matches!(err.kind, UrlImportFailureKind::Retryable));
        assert!(!staging.join(format!("{}.part", source_id)).exists());
        std::env::remove_var("DATA_STUDIO_URL_TOTAL_TIMEOUT_SECS");
    }

    #[tokio::test]
    async fn public_url_download_enforces_size_limits() {
        clear_env();
        let _url_guard = url_import_test_guard().await;
        let _guard = db_test_guard().await;
        std::env::set_var("DATA_STUDIO_URL_MAX_BYTES", "8");
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let server = start_http_server().await;
        register_test_loopback(server.ip());
        let server_url = format!("http://{}", server);
        let source_id = seed_public_source(&pool, project_id, &format!("{server_url}/big")).await;
        let staging = temp_dir();

        let err = download_public_url_to_staging(
            &database,
            source_id,
            &format!("{server_url}/big"),
            &staging,
        )
        .await
        .unwrap_err();
        assert!(matches!(err.kind, UrlImportFailureKind::Terminal));
        assert!(!staging.join(format!("{}.part", source_id)).exists());
        std::env::remove_var("DATA_STUDIO_URL_MAX_BYTES");
    }

    #[tokio::test]
    async fn public_url_download_revalidates_redirect_targets_and_retries_idempotently() {
        clear_env();
        let _url_guard = url_import_test_guard().await;
        let _guard = db_test_guard().await;
        let pool = test_pool().await;
        let project_id = seed_project(&pool).await;
        let database = Database::from_pool(pool.clone());
        let redirect_server = start_http_server().await;
        register_test_loopback(redirect_server.ip());
        let redirect_url = format!("http://{}", redirect_server);
        let blocked_target = "http://127.0.0.1:65535/file";
        let redirect_source = seed_public_source(
            &pool,
            project_id,
            &format!("{redirect_url}/redirect?{blocked_target}"),
        )
        .await;
        let staging = temp_dir();

        let blocked = download_public_url_to_staging(
            &database,
            redirect_source,
            &format!("{redirect_url}/redirect?{blocked_target}"),
            &staging,
        )
        .await
        .unwrap_err();
        assert!(matches!(blocked.kind, UrlImportFailureKind::Terminal));
        assert!(!staging.join(format!("{}.part", redirect_source)).exists());

        let file_server = start_http_server().await;
        register_test_loopback(file_server.ip());
        let file_url = format!("http://{}", file_server);
        let source_id = seed_public_source(&pool, project_id, &format!("{file_url}/file")).await;
        let first = download_public_url_to_staging(
            &database,
            source_id,
            &format!("{file_url}/file"),
            &staging,
        )
        .await
        .expect("download should succeed");
        let second = download_public_url_to_staging(
            &database,
            source_id,
            &format!("{file_url}/file"),
            &staging,
        )
        .await
        .expect("download should be idempotent");
        assert_eq!(first.stored_path, second.stored_path);
        assert_eq!(first.byte_size, second.byte_size);
        assert!(first.stored_path.exists());
        assert!(!first.sha256.is_empty());
    }
}
