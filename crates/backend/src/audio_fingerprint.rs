use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgRow, Executor, PgPool, Postgres, Row};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioFingerprint {
    pub id: Uuid,
    pub audio_asset_id: Uuid,
    pub algorithm: String,
    pub algorithm_version: String,
    pub processing_status: String,
    pub duration_seconds: f64,
    pub storage_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioFingerprintUpsert {
    pub id: Uuid,
    pub audio_asset_id: Uuid,
    pub algorithm: String,
    pub algorithm_version: String,
    pub processing_status: String,
    pub duration_seconds: f64,
    pub storage_path: String,
}

#[derive(Debug, Clone)]
pub struct AudioFingerprintRepository {
    pool: PgPool,
}

impl AudioFingerprintRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert<'e, E>(
        &self,
        executor: E,
        request: AudioFingerprintUpsert,
    ) -> Result<AudioFingerprint>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query(
            r#"
            insert into audio_fingerprints (
                id, audio_asset_id, algorithm, algorithm_version, processing_status,
                duration_seconds, storage_path, created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, now(), now())
            on conflict (audio_asset_id, algorithm, algorithm_version)
            do update set
                processing_status = excluded.processing_status,
                duration_seconds = excluded.duration_seconds,
                storage_path = excluded.storage_path,
                updated_at = excluded.updated_at
            returning id, audio_asset_id, algorithm, algorithm_version, processing_status,
                      duration_seconds, storage_path, created_at, updated_at
            "#,
        )
        .bind(request.id)
        .bind(request.audio_asset_id)
        .bind(request.algorithm.as_str())
        .bind(request.algorithm_version.as_str())
        .bind(request.processing_status.as_str())
        .bind(request.duration_seconds)
        .bind(request.storage_path.as_str())
        .fetch_one(executor)
        .await?;
        audio_fingerprint_from_row(&row)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<AudioFingerprint>> {
        let row = sqlx::query(
            r#"
            select id, audio_asset_id, algorithm, algorithm_version, processing_status,
                   duration_seconds, storage_path, created_at, updated_at
            from audio_fingerprints
            where id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(audio_fingerprint_from_row).transpose()
    }
}

fn audio_fingerprint_from_row(row: &PgRow) -> Result<AudioFingerprint> {
    Ok(AudioFingerprint {
        id: row.try_get("id")?,
        audio_asset_id: row.try_get("audio_asset_id")?,
        algorithm: row.try_get("algorithm")?,
        algorithm_version: row.try_get("algorithm_version")?,
        processing_status: row.try_get("processing_status")?,
        duration_seconds: row.try_get("duration_seconds")?,
        storage_path: row.try_get("storage_path")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
