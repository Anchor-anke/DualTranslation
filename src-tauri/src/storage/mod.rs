use crate::{
    error::{AppError, AppResult},
    types::{
        AppSettings, ConversionMode, ConversionResponse, HistoryRecord, HistorySummary,
        HistoryVersion, OutputLanguage, ProviderProfile, TargetAgent,
    },
};
use chrono::Utc;
use serde_json::Value;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const MIGRATION: &str = include_str!("../../migrations/001_initial.sql");

#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
}

pub struct NewConversion<'a> {
    pub mode: &'a ConversionMode,
    pub original_input: &'a str,
    pub sensitive: bool,
    pub target_agent: &'a TargetAgent,
    pub output_language: &'a OutputLanguage,
    pub response: &'a ConversionResponse,
    pub rendered_text: &'a str,
    pub history_limit: u32,
}

impl Storage {
    pub async fn initialize(app: &AppHandle) -> AppResult<Self> {
        let directory = app.path().app_config_dir().map_err(AppError::internal)?;
        std::fs::create_dir_all(&directory).map_err(AppError::internal)?;
        Self::initialize_at(directory.join("dualtranslation.db")).await
    }

    async fn initialize_at(path: PathBuf) -> AppResult<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(AppError::internal)?;
        sqlx::raw_sql(MIGRATION)
            .execute(&pool)
            .await
            .map_err(AppError::internal)?;
        let storage = Self { pool };
        storage.ensure_default_settings().await?;
        Ok(storage)
    }

    async fn ensure_default_settings(&self) -> AppResult<()> {
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM settings WHERE key = 'app'")
                .fetch_one(&self.pool)
                .await
                .map_err(AppError::internal)?;
        if exists == 0 {
            self.update_settings(&AppSettings::default()).await?;
        }
        Ok(())
    }

    pub async fn list_provider_profiles(&self) -> AppResult<Vec<ProviderProfile>> {
        let rows = sqlx::query(
            "SELECT id, name, base_url, model, timeout_ms, credential_ref FROM provider_profiles ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::internal)?;

        rows.into_iter()
            .map(|row| {
                let timeout_ms = row
                    .try_get::<i64, _>("timeout_ms")
                    .map_err(AppError::internal)?;
                Ok(ProviderProfile {
                    id: row.try_get("id").map_err(AppError::internal)?,
                    name: row.try_get("name").map_err(AppError::internal)?,
                    base_url: row.try_get("base_url").map_err(AppError::internal)?,
                    model: row.try_get("model").map_err(AppError::internal)?,
                    timeout_ms: timeout_ms.try_into().map_err(AppError::internal)?,
                    has_credential: !row
                        .try_get::<String, _>("credential_ref")
                        .map_err(AppError::internal)?
                        .is_empty(),
                })
            })
            .collect()
    }

    pub async fn get_provider_profile(&self, id: &str) -> AppResult<ProviderProfile> {
        let row = sqlx::query(
            "SELECT id, name, base_url, model, timeout_ms, credential_ref FROM provider_profiles WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| {
            AppError::new(
                "PROVIDER_NOT_CONFIGURED",
                "找不到当前模型配置。",
                "请在设置中选择或新增模型配置。",
            )
        })?;
        let timeout_ms = row
            .try_get::<i64, _>("timeout_ms")
            .map_err(AppError::internal)?;
        Ok(ProviderProfile {
            id: row.try_get("id").map_err(AppError::internal)?,
            name: row.try_get("name").map_err(AppError::internal)?,
            base_url: row.try_get("base_url").map_err(AppError::internal)?,
            model: row.try_get("model").map_err(AppError::internal)?,
            timeout_ms: timeout_ms.try_into().map_err(AppError::internal)?,
            has_credential: true,
        })
    }

    pub async fn upsert_provider_profile(&self, profile: &ProviderProfile) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO provider_profiles
               (id, name, base_url, model, timeout_ms, credential_ref, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 base_url = excluded.base_url,
                 model = excluded.model,
                 timeout_ms = excluded.timeout_ms,
                 credential_ref = excluded.credential_ref,
                 updated_at = excluded.updated_at"#,
        )
        .bind(&profile.id)
        .bind(&profile.name)
        .bind(&profile.base_url)
        .bind(&profile.model)
        .bind(i64::try_from(profile.timeout_ms).map_err(AppError::internal)?)
        .bind(&profile.id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(AppError::internal)?;
        Ok(())
    }

    pub async fn get_settings(&self) -> AppResult<AppSettings> {
        let value =
            sqlx::query_scalar::<_, String>("SELECT value_json FROM settings WHERE key = 'app'")
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::internal)?;
        match value {
            Some(value) => serde_json::from_str(&value).map_err(AppError::internal),
            None => Ok(AppSettings::default()),
        }
    }

    pub async fn update_settings(&self, settings: &AppSettings) -> AppResult<AppSettings> {
        let value = serde_json::to_string(settings).map_err(AppError::internal)?;
        sqlx::query(
            r#"INSERT INTO settings(key, value_json, updated_at) VALUES('app', ?, ?)
               ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at"#,
        )
        .bind(value)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(AppError::internal)?;
        self.prune_history(settings.history_limit).await?;
        Ok(settings.clone())
    }

    pub async fn save_conversion(&self, conversion: NewConversion<'_>) -> AppResult<()> {
        let conversion_id = conversion.response.request_id.clone();
        let now = Utc::now().to_rfc3339();
        let structured = serde_json::to_string(conversion.response).map_err(AppError::internal)?;
        let mut transaction = self.pool.begin().await.map_err(AppError::internal)?;
        sqlx::query(
            r#"INSERT INTO conversions
               (id, mode, original_input, sensitive, target_agent, output_language, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&conversion_id)
        .bind(conversion.mode.as_str())
        .bind(conversion.original_input)
        .bind(i64::from(conversion.sensitive))
        .bind(conversion.target_agent.as_str())
        .bind(conversion.output_language.as_str())
        .bind(&now)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
        sqlx::query(
            r#"INSERT INTO conversion_versions
               (id, conversion_id, version_no, structured_json, rendered_text, adjustment_text, changed_fields_json, created_at)
               VALUES (?, ?, 1, ?, ?, NULL, '[]', ?)"#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&conversion_id)
        .bind(structured)
        .bind(conversion.rendered_text)
        .bind(&now)
        .execute(&mut *transaction)
        .await
        .map_err(AppError::internal)?;
        transaction.commit().await.map_err(AppError::internal)?;
        self.prune_history(conversion.history_limit).await
    }

    pub async fn list_history(&self) -> AppResult<Vec<HistorySummary>> {
        let rows = sqlx::query(
            r#"SELECT c.id, c.mode, c.target_agent, c.output_language, c.created_at,
                      COALESCE((SELECT substr(v.rendered_text, 1, 160)
                                FROM conversion_versions v
                                WHERE v.conversion_id = c.id
                                ORDER BY v.version_no DESC LIMIT 1), '') AS preview,
                      (SELECT COUNT(*) FROM conversion_versions v WHERE v.conversion_id = c.id) AS version_count
               FROM conversions c
               ORDER BY c.created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::internal)?;
        rows.into_iter().map(history_summary_from_row).collect()
    }

    pub async fn append_conversion_version(
        &self,
        conversion_id: &str,
        response: &ConversionResponse,
        rendered_text: &str,
        adjustment_text: &str,
        changed_fields: &[String],
    ) -> AppResult<Option<u32>> {
        let current = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(version_no) FROM conversion_versions WHERE conversion_id = ?",
        )
        .bind(conversion_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::internal)?;
        let Some(current) = current else {
            return Ok(None);
        };
        let next = current + 1;
        sqlx::query(
            r#"INSERT INTO conversion_versions
               (id, conversion_id, version_no, structured_json, rendered_text, adjustment_text, changed_fields_json, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(conversion_id)
        .bind(next)
        .bind(serde_json::to_string(response).map_err(AppError::internal)?)
        .bind(rendered_text)
        .bind(adjustment_text)
        .bind(serde_json::to_string(changed_fields).map_err(AppError::internal)?)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(AppError::internal)?;
        Ok(Some(next.try_into().map_err(AppError::internal)?))
    }

    pub async fn get_history(&self, id: &str) -> AppResult<HistoryRecord> {
        let row = sqlx::query(
            r#"SELECT c.id, c.mode, c.original_input, c.sensitive, c.target_agent, c.output_language, c.created_at,
                      COALESCE((SELECT substr(v.rendered_text, 1, 160)
                                FROM conversion_versions v
                                WHERE v.conversion_id = c.id
                                ORDER BY v.version_no DESC LIMIT 1), '') AS preview,
                      (SELECT COUNT(*) FROM conversion_versions v WHERE v.conversion_id = c.id) AS version_count
               FROM conversions c WHERE c.id = ?"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| {
            AppError::new(
                "NOT_FOUND",
                "找不到这条历史记录。",
                "它可能已被历史上限清理或手动删除。",
            )
        })?;
        let original_input = row.try_get("original_input").map_err(AppError::internal)?;
        let sensitive = row
            .try_get::<i64, _>("sensitive")
            .map_err(AppError::internal)?
            != 0;
        let summary = history_summary_from_row(row)?;
        let version_rows = sqlx::query(
            r#"SELECT version_no, rendered_text, structured_json, adjustment_text,
                      changed_fields_json, created_at
               FROM conversion_versions WHERE conversion_id = ? ORDER BY version_no ASC"#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::internal)?;
        let versions = version_rows
            .into_iter()
            .map(|row| {
                let structured_text: String =
                    row.try_get("structured_json").map_err(AppError::internal)?;
                let changed_text: String = row
                    .try_get("changed_fields_json")
                    .map_err(AppError::internal)?;
                Ok(HistoryVersion {
                    version_no: row
                        .try_get::<i64, _>("version_no")
                        .map_err(AppError::internal)?
                        .try_into()
                        .map_err(AppError::internal)?,
                    rendered_text: row.try_get("rendered_text").map_err(AppError::internal)?,
                    structured_data: serde_json::from_str(&structured_text)
                        .map_err(AppError::internal)?,
                    adjustment_text: row.try_get("adjustment_text").map_err(AppError::internal)?,
                    changed_fields: serde_json::from_str(&changed_text)
                        .map_err(AppError::internal)?,
                    created_at: row.try_get("created_at").map_err(AppError::internal)?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        Ok(HistoryRecord {
            summary,
            original_input,
            sensitive,
            versions,
        })
    }

    pub async fn delete_history(&self, id: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM conversions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::internal)?;
        Ok(())
    }

    pub async fn clear_history(&self) -> AppResult<()> {
        sqlx::query("DELETE FROM conversions")
            .execute(&self.pool)
            .await
            .map_err(AppError::internal)?;
        Ok(())
    }

    pub async fn increment_metric(&self, event_name: &str) -> AppResult<()> {
        let allowed = [
            "generate",
            "copy",
            "minor_edit_copy",
            "major_edit_copy",
            "conversion_failed",
            "sensitive_hit",
            "cancel",
            "history_copy",
        ];
        if !allowed.contains(&event_name) {
            return Err(AppError::internal("invalid metric name"));
        }
        sqlx::query(
            r#"INSERT INTO local_metric_events(event_name, count, updated_at) VALUES(?, 1, ?)
               ON CONFLICT(event_name) DO UPDATE SET count = count + 1, updated_at = excluded.updated_at"#,
        )
        .bind(event_name)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(AppError::internal)?;
        Ok(())
    }

    pub async fn get_metrics(&self) -> AppResult<Value> {
        let rows =
            sqlx::query("SELECT event_name, count FROM local_metric_events ORDER BY event_name")
                .fetch_all(&self.pool)
                .await
                .map_err(AppError::internal)?;
        let mut result = serde_json::Map::new();
        for row in rows {
            let name: String = row.try_get("event_name").map_err(AppError::internal)?;
            let count: i64 = row.try_get("count").map_err(AppError::internal)?;
            result.insert(name, Value::from(count));
        }
        Ok(Value::Object(result))
    }

    pub async fn clear_metrics(&self) -> AppResult<()> {
        sqlx::query("DELETE FROM local_metric_events")
            .execute(&self.pool)
            .await
            .map_err(AppError::internal)?;
        Ok(())
    }

    async fn prune_history(&self, limit: u32) -> AppResult<()> {
        sqlx::query(
            "DELETE FROM conversions WHERE id IN (SELECT id FROM conversions ORDER BY created_at DESC LIMIT -1 OFFSET ?)",
        )
        .bind(i64::from(limit))
        .execute(&self.pool)
        .await
        .map_err(AppError::internal)?;
        Ok(())
    }
}

fn history_summary_from_row(row: sqlx::sqlite::SqliteRow) -> AppResult<HistorySummary> {
    let mode = match row
        .try_get::<String, _>("mode")
        .map_err(AppError::internal)?
        .as_str()
    {
        "write" => ConversionMode::Write,
        "explain" => ConversionMode::Explain,
        _ => return Err(AppError::internal("invalid stored conversion mode")),
    };
    let target_agent = match row
        .try_get::<String, _>("target_agent")
        .map_err(AppError::internal)?
        .as_str()
    {
        "generic" => TargetAgent::Generic,
        "cursor" => TargetAgent::Cursor,
        "codex" => TargetAgent::Codex,
        _ => return Err(AppError::internal("invalid stored target agent")),
    };
    let output_language = match row
        .try_get::<String, _>("output_language")
        .map_err(AppError::internal)?
        .as_str()
    {
        "zh" => OutputLanguage::Zh,
        "en" => OutputLanguage::En,
        "bilingual" => OutputLanguage::Bilingual,
        _ => return Err(AppError::internal("invalid stored output language")),
    };
    Ok(HistorySummary {
        id: row.try_get("id").map_err(AppError::internal)?,
        mode,
        preview: row.try_get("preview").map_err(AppError::internal)?,
        target_agent,
        output_language,
        created_at: row.try_get("created_at").map_err(AppError::internal)?,
        version_count: row
            .try_get::<i64, _>("version_count")
            .map_err(AppError::internal)?
            .try_into()
            .map_err(AppError::internal)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn storage() -> Storage {
        let directory = tempdir().unwrap();
        let path = directory.keep().join("test.db");
        Storage::initialize_at(path).await.unwrap()
    }

    #[tokio::test]
    async fn migrations_are_idempotent_and_defaults_exist() {
        let storage = storage().await;
        assert_eq!(storage.get_settings().await.unwrap().history_limit, 20);
        sqlx::raw_sql(MIGRATION)
            .execute(&storage.pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn profile_table_never_contains_api_key_column() {
        let storage = storage().await;
        let columns = sqlx::query("PRAGMA table_info(provider_profiles)")
            .fetch_all(&storage.pool)
            .await
            .unwrap();
        let names = columns
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();
        assert!(!names.iter().any(|name| name.contains("api_key")));
    }

    #[tokio::test]
    async fn history_limit_prunes_oldest_records_and_versions() {
        let storage = storage().await;
        for index in 0..3 {
            let response = ConversionResponse {
                schema_version: 1,
                kind: "write_completed".into(),
                request_id: format!("req_{index}"),
                data: serde_json::json!({}),
            };
            let input = format!("input {index}");
            let rendered = format!("rendered {index}");
            storage
                .save_conversion(NewConversion {
                    mode: &ConversionMode::Write,
                    original_input: &input,
                    sensitive: false,
                    target_agent: &TargetAgent::Codex,
                    output_language: &OutputLanguage::Zh,
                    response: &response,
                    rendered_text: &rendered,
                    history_limit: 2,
                })
                .await
                .unwrap();
        }
        let history = storage.list_history().await.unwrap();
        assert_eq!(history.len(), 2);
        let orphan_versions = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM conversion_versions v LEFT JOIN conversions c ON c.id = v.conversion_id WHERE c.id IS NULL",
        )
        .fetch_one(&storage.pool)
        .await
        .unwrap();
        assert_eq!(orphan_versions, 0);
    }

    #[tokio::test]
    async fn history_restores_sensitive_flag_and_all_versions() {
        let storage = storage().await;
        let initial = ConversionResponse {
            schema_version: 1,
            kind: "write_completed".into(),
            request_id: "versioned".into(),
            data: serde_json::json!({ "renderedPrompt": "initial" }),
        };
        storage
            .save_conversion(NewConversion {
                mode: &ConversionMode::Write,
                original_input: "token=secret",
                sensitive: true,
                target_agent: &TargetAgent::Codex,
                output_language: &OutputLanguage::Zh,
                response: &initial,
                rendered_text: "initial",
                history_limit: 20,
            })
            .await
            .unwrap();
        let adjusted = ConversionResponse {
            data: serde_json::json!({ "renderedPrompt": "adjusted" }),
            ..initial
        };
        let fields = vec!["data.renderedPrompt".to_owned()];
        let version = storage
            .append_conversion_version(
                "versioned",
                &adjusted,
                "adjusted",
                "make it shorter",
                &fields,
            )
            .await
            .unwrap();

        assert_eq!(version, Some(2));
        let restored = storage.get_history("versioned").await.unwrap();
        assert!(restored.sensitive);
        assert_eq!(restored.versions.len(), 2);
        assert_eq!(
            restored.versions[1].adjustment_text.as_deref(),
            Some("make it shorter")
        );
        assert_eq!(restored.versions[1].changed_fields, fields);

        storage.delete_history("versioned").await.unwrap();
        assert!(storage.get_history("versioned").await.is_err());
    }

    #[tokio::test]
    async fn local_metrics_store_counts_only_and_can_be_cleared() {
        let storage = storage().await;
        storage.increment_metric("generate").await.unwrap();
        storage.increment_metric("generate").await.unwrap();
        storage.increment_metric("history_copy").await.unwrap();

        let metrics = storage.get_metrics().await.unwrap();
        assert_eq!(metrics["generate"], 2);
        assert_eq!(metrics["history_copy"], 1);
        assert!(storage.increment_metric("input text").await.is_err());

        storage.clear_metrics().await.unwrap();
        assert_eq!(storage.get_metrics().await.unwrap(), serde_json::json!({}));
    }
}
