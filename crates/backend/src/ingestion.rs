use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use flate2::read::GzDecoder;
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::record::RowAccessor;
use serde_json::Value;
use walkdir::WalkDir;
use zip::read::ZipArchive;

use crate::{db::Database, job::JobType, source::SourceType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationResult {
    pub source_id: uuid::Uuid,
    pub job_id: uuid::Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvImportSummary {
    pub imported: u32,
    pub duplicate: u32,
    pub invalid: u32,
    pub failed: u32,
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
    let normalized_identity = format!("url:{}", { normalize_public_url(url) });
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
        .create(project_id, SourceType::PublicUrl, Some(url), None)
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

pub async fn scan_folder(
    database: &Database,
    project_id: uuid::Uuid,
    root: &Path,
) -> Result<(u32, u32, u32)> {
    let root = root.canonicalize().context("folder must exist")?;
    let mut discovered = 0;
    let mut created = 0;
    let mut skipped = 0;
    for entry in WalkDir::new(&root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !path.starts_with(&root) {
            skipped += 1;
            continue;
        }
        if !is_supported_media(path) {
            skipped += 1;
            continue;
        }
        discovered += 1;
        let normalized_identity = format!(
            "local_file:{}",
            path.to_string_lossy().trim().to_lowercase()
        );
        let existed = database
            .sources()
            .get_by_identity(project_id, &normalized_identity)
            .await?
            .is_some();
        let _ = register_local_file(database, project_id, path).await?;
        if existed {
            skipped += 1;
        } else {
            created += 1;
        }
    }
    Ok((discovered, created, skipped))
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
    };
    for record in reader.records() {
        let record = match record {
            Ok(record) => record,
            Err(_) => {
                summary.failed += 1;
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
                    Err(_) => summary.failed += 1,
                }
            }
            None => summary.invalid += 1,
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
                let mut record = serde_json::Map::new();
                for index in 0..row.len() {
                    let key = format!("col_{index}");
                    let value = row
                        .get_string(index)
                        .map(|value| Value::String(value.to_string()))
                        .unwrap_or(Value::Null);
                    record.insert(key, value);
                }
                let record = Value::Object(record);
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
    let source = database
        .sources()
        .create(
            project_id,
            SourceType::ExistingDataset,
            Some(audio_path.to_string_lossy().as_ref()),
            Some(&metadata.to_string()),
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
    Ok(Some(DatasetItemResult {
        source_id: source.id,
        job_id: job.id,
        metadata,
    }))
}

pub async fn extract_archive(archive_path: &Path, output_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(output_dir)?;
    let archive_name = archive_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("archive");
    let extract_dir = output_dir.join(format!("extract-{}", archive_name));
    fs::create_dir_all(&extract_dir)?;
    match archive_path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_ascii_lowercase())
        .as_deref()
    {
        Some("zip") => {
            let file = fs::File::open(archive_path)?;
            let mut archive = ZipArchive::new(file)?;
            extract_zip(&mut archive, &extract_dir)?;
        }
        Some("tar") => {
            let file = fs::File::open(archive_path)?;
            let mut archive = tar::Archive::new(file);
            extract_tar(&mut archive, &extract_dir)?;
        }
        Some("gz") | Some("tgz") => {
            let file = fs::File::open(archive_path)?;
            let decoder = GzDecoder::new(file);
            let mut archive = tar::Archive::new(decoder);
            extract_tar(&mut archive, &extract_dir)?;
        }
        _ => return Err(anyhow!("unsupported archive format")),
    }
    Ok(extract_dir)
}

fn extract_zip<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    output_dir: &Path,
) -> Result<()> {
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let out = safe_join(output_dir, file.name())?;
        if file.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
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
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let out = safe_join(output_dir, path.to_string_lossy().as_ref())?;
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        entry.unpack(out)?;
    }
    Ok(())
}

fn safe_join(base: &Path, relative: &str) -> Result<PathBuf> {
    let candidate = base.join(relative);
    let normalized = candidate.components().fold(PathBuf::new(), |mut acc, c| {
        acc.push(c);
        acc
    });
    if !normalized.starts_with(base) {
        return Err(anyhow!("archive entry attempted path traversal"));
    }
    Ok(normalized)
}

fn validate_public_url(url: &str) -> Result<()> {
    let trimmed = url.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Ok(())
    } else {
        Err(anyhow!("url must start with http:// or https://"))
    }
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
