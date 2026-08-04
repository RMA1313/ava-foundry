use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgRow, Executor, PgPool, Postgres, Row};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateDecision {
    pub id: Uuid,
    pub audio_asset_low_id: Uuid,
    pub audio_asset_high_id: Uuid,
    pub candidate_fingerprint_low_id: Uuid,
    pub candidate_fingerprint_high_id: Uuid,
    pub algorithm: String,
    pub algorithm_version: String,
    pub duration_delta_seconds: f64,
    pub similarity_score: f64,
    pub decision: String,
    pub confidence: String,
    pub duplicate_group_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DuplicateRepository {
    pool: PgPool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateGroup {
    pub id: Uuid,
    pub project_id: Uuid,
    pub canonical_audio_asset_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateGroupMember {
    pub duplicate_group_id: Uuid,
    pub audio_asset_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateGroupDetails {
    pub group: DuplicateGroup,
    pub members: Vec<DuplicateGroupMember>,
    pub decisions: Vec<DuplicateDecision>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateGroupPage {
    pub items: Vec<DuplicateGroupDetails>,
    pub limit: u32,
    pub offset: u32,
}

impl DuplicateRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_decision<'e, E>(
        &self,
        executor: E,
        request: DuplicateDecisionUpsert,
    ) -> Result<DuplicateDecision>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query(
            r#"
            insert into audio_duplicate_decisions (
                id, audio_asset_low_id, audio_asset_high_id,
                candidate_fingerprint_low_id, candidate_fingerprint_high_id,
                algorithm, algorithm_version, duration_delta_seconds,
                similarity_score, decision, confidence, duplicate_group_id,
                created_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now(), now())
            on conflict (audio_asset_low_id, audio_asset_high_id, algorithm, algorithm_version)
            do update set
                candidate_fingerprint_low_id = excluded.candidate_fingerprint_low_id,
                candidate_fingerprint_high_id = excluded.candidate_fingerprint_high_id,
                duration_delta_seconds = excluded.duration_delta_seconds,
                similarity_score = excluded.similarity_score,
                decision = excluded.decision,
                confidence = excluded.confidence,
                duplicate_group_id = coalesce(audio_duplicate_decisions.duplicate_group_id, excluded.duplicate_group_id),
                updated_at = excluded.updated_at
            returning id, audio_asset_low_id, audio_asset_high_id,
                      candidate_fingerprint_low_id, candidate_fingerprint_high_id,
                      algorithm, algorithm_version, duration_delta_seconds,
                      similarity_score, decision, confidence, duplicate_group_id,
                      created_at, updated_at
            "#,
        )
        .bind(request.id)
        .bind(request.audio_asset_low_id)
        .bind(request.audio_asset_high_id)
        .bind(request.candidate_fingerprint_low_id)
        .bind(request.candidate_fingerprint_high_id)
        .bind(request.algorithm.as_str())
        .bind(request.algorithm_version.as_str())
        .bind(request.duration_delta_seconds)
        .bind(request.similarity_score)
        .bind(request.decision.as_str())
        .bind(request.confidence.as_str())
        .bind(request.duplicate_group_id)
        .fetch_one(executor)
        .await?;
        duplicate_decision_from_row(&row)
    }

    pub async fn get_group_assets(&self, duplicate_group_id: Uuid) -> Result<Vec<Uuid>> {
        let rows = sqlx::query(
            r#"
            select audio_asset_id
            from duplicate_group_memberships
            where duplicate_group_id = $1
            order by audio_asset_id asc
            "#,
        )
        .bind(duplicate_group_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| row.try_get("audio_asset_id"))
            .collect::<std::result::Result<Vec<Uuid>, _>>()
            .map_err(Into::into)
    }

    pub async fn ensure_group_membership(
        &self,
        project_id: Uuid,
        audio_asset_ids: [Uuid; 2],
    ) -> Result<Uuid> {
        let mut ids = vec![audio_asset_ids[0], audio_asset_ids[1]];
        ids.sort();
        let mut tx = self.pool.begin().await?;
        let existing_groups = sqlx::query(
            r#"
            select duplicate_group_id, audio_asset_id
            from duplicate_group_memberships
            where audio_asset_id = any($1)
            order by duplicate_group_id asc, audio_asset_id asc
            "#,
        )
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await?;
        let mut group_ids: Vec<Uuid> = existing_groups
            .iter()
            .map(|row| row.try_get("duplicate_group_id"))
            .collect::<std::result::Result<Vec<Uuid>, _>>()?;
        group_ids.sort();
        group_ids.dedup();
        let target_group_id = group_ids.first().copied().unwrap_or_else(Uuid::new_v4);
        if group_ids.len() > 1 {
            for group_id in group_ids.iter().skip(1) {
                sqlx::query(
                    r#"
                    update duplicate_group_memberships
                    set duplicate_group_id = $1, updated_at = now()
                    where duplicate_group_id = $2
                    "#,
                )
                .bind(target_group_id)
                .bind(group_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query("delete from duplicate_groups where id = $1")
                    .bind(group_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        sqlx::query(
            r#"
            insert into duplicate_groups (id, project_id, created_at, updated_at)
            values ($1, $2, now(), now())
            on conflict (id) do update set updated_at = excluded.updated_at
            "#,
        )
        .bind(target_group_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        for audio_asset_id in ids {
            sqlx::query(
                r#"
                insert into duplicate_group_memberships (
                    duplicate_group_id, audio_asset_id, created_at, updated_at
                )
                values ($1, $2, now(), now())
                on conflict (audio_asset_id) do update set
                    duplicate_group_id = excluded.duplicate_group_id,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(target_group_id)
            .bind(audio_asset_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(target_group_id)
    }

    pub async fn ensure_canonical_audio_asset(
        &self,
        duplicate_group_id: Uuid,
    ) -> Result<Option<Uuid>> {
        let mut tx = self.pool.begin().await?;
        let existing = sqlx::query(
            r#"
            select canonical_audio_asset_id
            from duplicate_groups
            where id = $1
            for update
            "#,
        )
        .bind(duplicate_group_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = existing else {
            tx.commit().await?;
            return Ok(None);
        };
        let current: Option<Uuid> = row.try_get("canonical_audio_asset_id")?;
        if current.is_some() {
            tx.commit().await?;
            return Ok(current);
        }
        let canonical = sqlx::query_scalar(
            r#"
            select audio_asset_id
            from duplicate_group_memberships
            where duplicate_group_id = $1
            order by audio_asset_id asc
            limit 1
            "#,
        )
        .bind(duplicate_group_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(canonical_audio_asset_id) = canonical {
            sqlx::query(
                r#"
                update duplicate_groups
                set canonical_audio_asset_id = $1, updated_at = now()
                where id = $2
                  and canonical_audio_asset_id is null
                "#,
            )
            .bind(canonical_audio_asset_id)
            .bind(duplicate_group_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(Some(canonical_audio_asset_id));
        }
        tx.commit().await?;
        Ok(None)
    }

    pub async fn recover_canonical_audio_assets(&self) -> Result<Vec<(Uuid, Uuid)>> {
        let group_ids = sqlx::query_scalar::<_, Uuid>(
            r#"
            select id
            from duplicate_groups
            where canonical_audio_asset_id is null
            order by created_at asc, id asc
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut recovered = Vec::new();
        for group_id in group_ids {
            if let Some(canonical_audio_asset_id) =
                self.ensure_canonical_audio_asset(group_id).await?
            {
                recovered.push((group_id, canonical_audio_asset_id));
            }
        }
        Ok(recovered)
    }

    pub async fn list_groups(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<DuplicateGroupPage> {
        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);
        let group_rows = sqlx::query(
            r#"
            select id, project_id, canonical_audio_asset_id, created_at, updated_at
            from duplicate_groups
            where project_id = $1
            order by created_at asc, id asc
            limit $2 offset $3
            "#,
        )
        .bind(project_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let mut items = Vec::with_capacity(group_rows.len());
        for row in group_rows {
            let group = DuplicateGroup {
                id: row.try_get("id")?,
                project_id: row.try_get("project_id")?,
                canonical_audio_asset_id: row.try_get("canonical_audio_asset_id")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };
            let member_rows = sqlx::query(
                r#"
                select duplicate_group_id, audio_asset_id, created_at, updated_at
                from duplicate_group_memberships
                where duplicate_group_id = $1
                order by audio_asset_id asc
                "#,
            )
            .bind(group.id)
            .fetch_all(&self.pool)
            .await?;
            let members = member_rows
                .into_iter()
                .map(|row| {
                    Ok(DuplicateGroupMember {
                        duplicate_group_id: row.try_get("duplicate_group_id")?,
                        audio_asset_id: row.try_get("audio_asset_id")?,
                        created_at: row.try_get("created_at")?,
                        updated_at: row.try_get("updated_at")?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let decision_rows = sqlx::query(
                r#"
                select id, audio_asset_low_id, audio_asset_high_id,
                       candidate_fingerprint_low_id, candidate_fingerprint_high_id,
                       algorithm, algorithm_version, duration_delta_seconds,
                       similarity_score, decision, confidence, duplicate_group_id,
                       created_at, updated_at
                from audio_duplicate_decisions
                where duplicate_group_id = $1
                order by created_at asc, id asc
                "#,
            )
            .bind(group.id)
            .fetch_all(&self.pool)
            .await?;
            let decisions = decision_rows
                .iter()
                .map(duplicate_decision_from_row)
                .collect::<Result<Vec<_>>>()?;
            items.push(DuplicateGroupDetails {
                group,
                members,
                decisions,
            });
        }
        Ok(DuplicateGroupPage {
            items,
            limit: limit as u32,
            offset: offset as u32,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DuplicateDecisionUpsert {
    pub id: Uuid,
    pub audio_asset_low_id: Uuid,
    pub audio_asset_high_id: Uuid,
    pub candidate_fingerprint_low_id: Uuid,
    pub candidate_fingerprint_high_id: Uuid,
    pub algorithm: String,
    pub algorithm_version: String,
    pub duration_delta_seconds: f64,
    pub similarity_score: f64,
    pub decision: String,
    pub confidence: String,
    pub duplicate_group_id: Option<Uuid>,
}

fn duplicate_decision_from_row(row: &PgRow) -> Result<DuplicateDecision> {
    Ok(DuplicateDecision {
        id: row.try_get("id")?,
        audio_asset_low_id: row.try_get("audio_asset_low_id")?,
        audio_asset_high_id: row.try_get("audio_asset_high_id")?,
        candidate_fingerprint_low_id: row.try_get("candidate_fingerprint_low_id")?,
        candidate_fingerprint_high_id: row.try_get("candidate_fingerprint_high_id")?,
        algorithm: row.try_get("algorithm")?,
        algorithm_version: row.try_get("algorithm_version")?,
        duration_delta_seconds: row.try_get("duration_delta_seconds")?,
        similarity_score: row.try_get("similarity_score")?,
        decision: row.try_get("decision")?,
        confidence: row.try_get("confidence")?,
        duplicate_group_id: row.try_get("duplicate_group_id")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
