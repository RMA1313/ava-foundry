use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use flate2::read::GzDecoder;
use parquet::file::reader::{FileReader, SerializedFileReader};
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
    use flate2::{write::GzEncoder, Compression};
    use parquet::{
        data_type::{ByteArray, ByteArrayType},
        file::{properties::WriterProperties, writer::SerializedFileWriter},
        schema::parser::parse_message_type,
    };
    use std::{env, fs, fs::File, io::Write, path::PathBuf, sync::Arc};
    use uuid::Uuid;
    use zip::read::ZipArchive;

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
        let admin_pool = sqlx::PgPool::connect(&database_url)
            .await
            .expect("failed to connect to test database");
        sqlx::query(&format!("create schema {}", schema_name))
            .execute(&admin_pool)
            .await
            .expect("failed to create test schema");
        let schema_database_url = format!("{database_url}?options=-csearch_path%3D{schema_name}");
        migrations::run(&schema_database_url)
            .await
            .expect("failed to run migrations");
        sqlx::PgPool::connect(&schema_database_url)
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
    async fn jsonl_manifest_import_preserves_extra_metadata_and_reports_missing_files() {
        clear_env();
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
    async fn archive_import_cleans_up_and_reuses_existing_sources() {
        clear_env();
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
}
