use std::{fs, path::Path};

use rusqlite::{Connection, OptionalExtension, Result as SqlResult};

use super::models::{
    COMMENT_CREDENTIAL_SOURCE_KIND_VALUES, COMMENT_JOB_STATUS_VALUES, COMMENT_RUN_STATUS_VALUES,
};

const COMMENTER_SCHEMA_VERSION: i64 = 9;
pub const COMMENTER_DB_FILE_NAME: &str = "app.db";

pub fn open_in_memory() -> SqlResult<Connection> {
    let conn = Connection::open_in_memory()?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_app_database(data_root: &Path) -> SqlResult<Connection> {
    fs::create_dir_all(data_root)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let conn = Connection::open(data_root.join(COMMENTER_DB_FILE_NAME))?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    if let Err(error) = conn.execute_batch("BEGIN IMMEDIATE;") {
        let _ = conn.execute_batch("PRAGMA foreign_keys=ON;");
        return Err(error);
    }

    let result = (|| -> SqlResult<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS commenter_schema_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            INSERT INTO commenter_schema_meta(key, value) VALUES ('version', '1')
            ON CONFLICT(key) DO UPDATE SET value = excluded.value;
            "#,
        )?;

        ensure_app_settings_table(conn)?;
        ensure_credential_profiles_table(conn)?;
        ensure_profiles_table(conn)?;
        ensure_queue_runs_table(conn)?;
        ensure_file_jobs_table(conn)?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS commenter_artifacts (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id              INTEGER NOT NULL,
                file_job_id         INTEGER,
                kind                TEXT NOT NULL CHECK (kind IN ('input_snapshot','model_response','output_patch','log_bundle')),
                storage_path        TEXT NOT NULL,
                byte_size           INTEGER NOT NULL DEFAULT 0,
                sha256              TEXT,
                created_at          INTEGER NOT NULL,
                FOREIGN KEY(run_id) REFERENCES commenter_queue_runs(id) ON DELETE CASCADE,
                FOREIGN KEY(file_job_id) REFERENCES commenter_file_jobs(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS commenter_run_events (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id              INTEGER NOT NULL,
                file_job_id         INTEGER,
                level               TEXT NOT NULL CHECK (level IN ('info','warn','error')),
                event_type          TEXT NOT NULL,
                message             TEXT NOT NULL,
                created_at          INTEGER NOT NULL,
                FOREIGN KEY(run_id) REFERENCES commenter_queue_runs(id) ON DELETE CASCADE,
                FOREIGN KEY(file_job_id) REFERENCES commenter_file_jobs(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_commenter_runs_profile_status
                ON commenter_queue_runs(profile_id, status, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_commenter_jobs_run_status
                ON commenter_file_jobs(run_id, status, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_commenter_artifacts_run
                ON commenter_artifacts(run_id, kind, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_commenter_events_run
                ON commenter_run_events(run_id, created_at DESC);
            "#,
        )?;

        let violations: i64 =
            conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })?;
        if violations > 0 {
            return Err(rusqlite::Error::InvalidQuery);
        }

        set_schema_version(conn, COMMENTER_SCHEMA_VERSION)?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT;")?;
            conn.execute_batch("PRAGMA foreign_keys=ON;")?;
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK;");
            let _ = conn.execute_batch("PRAGMA foreign_keys=ON;");
            Err(error)
        }
    }
}

fn ensure_app_settings_table(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS commenter_app_settings (
            id                      INTEGER PRIMARY KEY CHECK (id = 1),
            global_max_workers      INTEGER NOT NULL DEFAULT 1,
            api_concurrency_limit   INTEGER NOT NULL DEFAULT 1,
            api_bearer_token        TEXT NOT NULL DEFAULT '',
            created_at              INTEGER NOT NULL DEFAULT 0,
            updated_at              INTEGER NOT NULL DEFAULT 0
        );
        "#,
    )?;

    if !table_has_column(conn, "commenter_app_settings", "api_bearer_token")? {
        conn.execute(
            "ALTER TABLE commenter_app_settings ADD COLUMN api_bearer_token TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }

    conn.execute_batch(
        r#"
        INSERT INTO commenter_app_settings (
            id,
            global_max_workers,
            api_concurrency_limit,
            api_bearer_token,
            created_at,
            updated_at
        )
        VALUES (1, 1, 1, '', 0, 0)
        ON CONFLICT(id) DO NOTHING;
        "#,
    )?;

    Ok(())
}

fn ensure_credential_profiles_table(conn: &Connection) -> SqlResult<()> {
    let Some(sql) = table_sql(conn, "commenter_credential_profiles")? else {
        conn.execute_batch(&credential_profiles_table_sql(
            "commenter_credential_profiles",
        ))?;
        return Ok(());
    };

    if credential_profiles_schema_is_current(conn, &sql)? {
        return Ok(());
    }

    rebuild_credential_profiles_table(conn)
}

fn ensure_profiles_table(conn: &Connection) -> SqlResult<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS commenter_project_profiles (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            project_key         TEXT NOT NULL UNIQUE,
            root_path           TEXT NOT NULL,
            profile_name        TEXT NOT NULL,
            prompt_template     TEXT NOT NULL,
            include_globs_json  TEXT,
            exclude_globs_json  TEXT,
            settings_json       TEXT,
            created_at          INTEGER NOT NULL,
            updated_at          INTEGER NOT NULL
        );
        "#,
    )?;

    if !table_has_column(conn, "commenter_project_profiles", "settings_json")? {
        conn.execute(
            "ALTER TABLE commenter_project_profiles ADD COLUMN settings_json TEXT",
            [],
        )?;
    }

    Ok(())
}

fn ensure_queue_runs_table(conn: &Connection) -> SqlResult<()> {
    let Some(sql) = table_sql(conn, "commenter_queue_runs")? else {
        conn.execute_batch(&queue_runs_table_sql("commenter_queue_runs"))?;
        return Ok(());
    };

    if queue_runs_schema_is_current(conn, &sql)? {
        return Ok(());
    }

    rebuild_queue_runs_table(conn)
}

fn ensure_file_jobs_table(conn: &Connection) -> SqlResult<()> {
    let Some(sql) = table_sql(conn, "commenter_file_jobs")? else {
        conn.execute_batch(&file_jobs_table_sql("commenter_file_jobs"))?;
        return Ok(());
    };

    if file_jobs_schema_is_current(conn, &sql)? {
        return Ok(());
    }

    rebuild_file_jobs_table(conn)
}

fn credential_profiles_schema_is_current(conn: &Connection, sql: &str) -> SqlResult<bool> {
    Ok(
        table_has_column(conn, "commenter_credential_profiles", "profile_key")?
            && table_has_column(conn, "commenter_credential_profiles", "display_name")?
            && table_has_column(conn, "commenter_credential_profiles", "source_kind")?
            && table_has_column(conn, "commenter_credential_profiles", "source_reference")?
            && !table_has_column(conn, "commenter_credential_profiles", "inline_secret")?
            && sql_supports_all_values(sql, COMMENT_CREDENTIAL_SOURCE_KIND_VALUES)
            && sql.contains("profile_key")
            && sql.contains("UNIQUE")
            && sql.contains("source_reference        TEXT NOT NULL"),
    )
}

fn queue_runs_schema_is_current(conn: &Connection, sql: &str) -> SqlResult<bool> {
    Ok(table_has_column(conn, "commenter_queue_runs", "run_settings_json")?
        && sql_supports_all_values(sql, COMMENT_RUN_STATUS_VALUES)
        && sql.contains("run_key")
        && sql.contains("UNIQUE")
        && sql.contains(
            "FOREIGN KEY(profile_id) REFERENCES commenter_project_profiles(id) ON DELETE CASCADE",
        ))
}

fn file_jobs_schema_is_current(_conn: &Connection, sql: &str) -> SqlResult<bool> {
    Ok(sql_supports_all_values(sql, COMMENT_JOB_STATUS_VALUES)
        && sql
            .contains("FOREIGN KEY(run_id) REFERENCES commenter_queue_runs(id) ON DELETE CASCADE")
        && sql.contains("UNIQUE(run_id, relative_path)"))
}

fn rebuild_credential_profiles_table(conn: &Connection) -> SqlResult<()> {
    let row_count = table_row_count(conn, "commenter_credential_profiles")?;
    let has_source_reference =
        table_has_column(conn, "commenter_credential_profiles", "source_reference")?;
    let has_inline_secret =
        table_has_column(conn, "commenter_credential_profiles", "inline_secret")?;

    if row_count > 0 && (!has_source_reference || has_inline_secret) {
        return Err(rusqlite::Error::InvalidQuery);
    }

    let source_reference_expr =
        if table_has_column(conn, "commenter_credential_profiles", "source_reference")? {
            "COALESCE(source_reference, '')"
        } else {
            "''"
        };

    run_rebuild_migration(
        conn,
        &format!(
            r#"
            DROP TABLE IF EXISTS commenter_credential_profiles_new;
            {create_new}
            INSERT INTO commenter_credential_profiles_new (
                id,
                profile_key,
                display_name,
                source_kind,
                source_reference,
                created_at,
                updated_at
            )
            SELECT
                id,
                profile_key,
                display_name,
                source_kind,
                {source_reference_expr},
                created_at,
                updated_at
            FROM commenter_credential_profiles;
            DROP TABLE commenter_credential_profiles;
            ALTER TABLE commenter_credential_profiles_new RENAME TO commenter_credential_profiles;
            "#,
            create_new = credential_profiles_table_sql("commenter_credential_profiles_new"),
            source_reference_expr = source_reference_expr,
        ),
    )
}

fn rebuild_queue_runs_table(conn: &Connection) -> SqlResult<()> {
    let run_settings_expr = if table_has_column(conn, "commenter_queue_runs", "run_settings_json")?
    {
        "run_settings_json"
    } else {
        "NULL AS run_settings_json"
    };

    run_rebuild_migration(
        conn,
        &format!(
            r#"
            DROP INDEX IF EXISTS idx_commenter_runs_profile_status;
            DROP TABLE IF EXISTS commenter_queue_runs_new;
            {create_new}
            INSERT INTO commenter_queue_runs_new (
                id,
                profile_id,
                run_key,
                status,
                requested_by,
                total_jobs,
                completed_jobs,
                run_settings_json,
                created_at,
                updated_at,
                started_at,
                finished_at
            )
            SELECT
                id,
                profile_id,
                run_key,
                CASE status
                    WHEN 'succeeded' THEN 'completed'
                    ELSE status
                END,
                requested_by,
                total_jobs,
                completed_jobs,
                {run_settings_expr},
                created_at,
                updated_at,
                started_at,
                finished_at
            FROM commenter_queue_runs;
            DROP TABLE commenter_queue_runs;
            ALTER TABLE commenter_queue_runs_new RENAME TO commenter_queue_runs;
            "#,
            create_new = queue_runs_table_sql("commenter_queue_runs_new"),
            run_settings_expr = run_settings_expr,
        ),
    )
}

fn rebuild_file_jobs_table(conn: &Connection) -> SqlResult<()> {
    run_rebuild_migration(
        conn,
        &format!(
            r#"
            DROP INDEX IF EXISTS idx_commenter_jobs_run_status;
            DROP TABLE IF EXISTS commenter_file_jobs_new;
            {create_new}
            INSERT INTO commenter_file_jobs_new (
                id,
                run_id,
                relative_path,
                status,
                language_hint,
                retry_count,
                created_at,
                updated_at,
                started_at,
                finished_at,
                error_message
            )
            SELECT
                id,
                run_id,
                relative_path,
                CASE status
                    WHEN 'processing' THEN 'writing'
                    WHEN 'succeeded' THEN 'done'
                    ELSE status
                END,
                language_hint,
                retry_count,
                created_at,
                updated_at,
                started_at,
                finished_at,
                error_message
            FROM commenter_file_jobs;
            DROP TABLE commenter_file_jobs;
            ALTER TABLE commenter_file_jobs_new RENAME TO commenter_file_jobs;
            "#,
            create_new = file_jobs_table_sql("commenter_file_jobs_new"),
        ),
    )
}

fn run_rebuild_migration(conn: &Connection, migration_sql: &str) -> SqlResult<()> {
    conn.execute_batch(migration_sql)
}

fn credential_profiles_table_sql(table_name: &str) -> String {
    format!(
        r#"
        CREATE TABLE IF NOT EXISTS {table_name} (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_key             TEXT NOT NULL UNIQUE,
            display_name            TEXT NOT NULL,
            source_kind             TEXT NOT NULL CHECK (source_kind IN ({source_kinds})),
            source_reference        TEXT NOT NULL,
            created_at              INTEGER NOT NULL,
            updated_at              INTEGER NOT NULL
        );
        "#,
        source_kinds = quoted_values(COMMENT_CREDENTIAL_SOURCE_KIND_VALUES),
    )
}

fn queue_runs_table_sql(table_name: &str) -> String {
    format!(
        r#"
        CREATE TABLE IF NOT EXISTS {table_name} (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id          INTEGER NOT NULL,
            run_key             TEXT NOT NULL UNIQUE,
            status              TEXT NOT NULL CHECK (status IN ({statuses})),
            requested_by        TEXT,
            total_jobs          INTEGER NOT NULL DEFAULT 0,
            completed_jobs      INTEGER NOT NULL DEFAULT 0,
            run_settings_json   TEXT,
            created_at          INTEGER NOT NULL,
            updated_at          INTEGER NOT NULL,
            started_at          INTEGER,
            finished_at         INTEGER,
            FOREIGN KEY(profile_id) REFERENCES commenter_project_profiles(id) ON DELETE CASCADE
        );
        "#,
        statuses = quoted_values(COMMENT_RUN_STATUS_VALUES),
    )
}

fn file_jobs_table_sql(table_name: &str) -> String {
    format!(
        r#"
        CREATE TABLE IF NOT EXISTS {table_name} (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id              INTEGER NOT NULL,
            relative_path       TEXT NOT NULL,
            status              TEXT NOT NULL CHECK (status IN ({statuses})),
            language_hint       TEXT,
            retry_count         INTEGER NOT NULL DEFAULT 0,
            created_at          INTEGER NOT NULL,
            updated_at          INTEGER NOT NULL,
            started_at          INTEGER,
            finished_at         INTEGER,
            error_message       TEXT,
            FOREIGN KEY(run_id) REFERENCES commenter_queue_runs(id) ON DELETE CASCADE,
            UNIQUE(run_id, relative_path)
        );
        "#,
        statuses = quoted_values(COMMENT_JOB_STATUS_VALUES),
    )
}

fn quoted_values(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| format!("'{value}'"))
        .collect::<Vec<_>>()
        .join(",")
}

fn table_sql(conn: &Connection, table_name: &str) -> SqlResult<Option<String>> {
    conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table_name],
        |row| row.get::<_, String>(0),
    )
    .optional()
}

fn table_has_column(conn: &Connection, table_name: &str, column_name: &str) -> SqlResult<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let existing_name: String = row.get(1)?;
        if existing_name == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

fn table_row_count(conn: &Connection, table_name: &str) -> SqlResult<i64> {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table_name}"), [], |row| {
        row.get(0)
    })
}

fn sql_supports_all_values(sql: &str, values: &[&str]) -> bool {
    values
        .iter()
        .all(|value| sql.contains(&format!("'{value}'")))
}

fn set_schema_version(conn: &Connection, version: i64) -> SqlResult<()> {
    conn.execute(
        "INSERT INTO commenter_schema_meta(key, value) VALUES('version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [version.to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::commenter::models::{
        CommentProjectSettings, CommentRunMode, CommentRunSettingsSnapshot, JsonHandlingStrategy,
    };
    use rusqlite::{Connection, Result};

    use super::*;

    fn list_table_names(conn: &Connection) -> Result<Vec<String>> {
        let mut stmt =
            conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    fn list_column_names(conn: &Connection, table_name: &str) -> Result<Vec<String>> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table_name})"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        rows.collect()
    }

    #[test]
    fn creates_profiles_runs_and_jobs_tables() {
        let conn = open_in_memory().expect("in-memory db");
        migrate(&conn).expect("migrate");

        let tables = list_table_names(&conn).expect("table list");
        assert!(tables.contains(&"commenter_app_settings".to_string()));
        assert!(tables.contains(&"commenter_credential_profiles".to_string()));
        assert!(tables.contains(&"commenter_project_profiles".to_string()));
        assert!(tables.contains(&"commenter_queue_runs".to_string()));
        assert!(tables.contains(&"commenter_file_jobs".to_string()));
        assert!(tables.contains(&"commenter_artifacts".to_string()));
        assert!(tables.contains(&"commenter_run_events".to_string()));
    }

    #[test]
    fn schema_accepts_approved_run_and_job_statuses() {
        let conn = open_in_memory().expect("in-memory db");

        conn.execute(
            "INSERT INTO commenter_project_profiles (
                project_key, root_path, profile_name, prompt_template, settings_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                "project-a",
                "C:\\repo",
                "default",
                "prompt",
                Some("{\"credential_profile_key\":\"team-default\",\"default_run_mode\":\"review\"}"),
                1_i64,
                1_i64,
            ),
        )
        .expect("insert profile");

        conn.execute(
            "INSERT INTO commenter_queue_runs (
                profile_id, run_key, status, requested_by, total_jobs, completed_jobs, run_settings_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                1_i64,
                "run-1",
                "stopped_by_limit",
                Some("spec-test"),
                2_i64,
                1_i64,
                Some("{\"credential_profile_key\":\"team-default\",\"run_mode\":\"review\"}"),
                1_i64,
                1_i64,
            ),
        )
        .expect("insert run");

        conn.execute(
            "INSERT INTO commenter_file_jobs (
                run_id, relative_path, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)",
            (1_i64, "src/main.rs", "review_needed", 1_i64, 1_i64),
        )
        .expect("insert file job");
    }

    #[test]
    fn schema_accepts_artifact_and_event_contract_values() {
        let conn = open_in_memory().expect("in-memory db");

        conn.execute(
            "INSERT INTO commenter_project_profiles (
                project_key, root_path, profile_name, prompt_template, settings_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                "project-artifact",
                "C:\\repo",
                "default",
                "prompt",
                Some("{\"credential_profile_key\":\"team-default\",\"default_run_mode\":\"review\"}"),
                1_i64,
                1_i64,
            ),
        )
        .expect("insert profile");

        conn.execute(
            "INSERT INTO commenter_queue_runs (
                profile_id, run_key, status, total_jobs, completed_jobs, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (1_i64, "run-artifact", "queued", 1_i64, 0_i64, 1_i64, 1_i64),
        )
        .expect("insert run");

        conn.execute(
            "INSERT INTO commenter_file_jobs (
                run_id, relative_path, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)",
            (1_i64, "src/main.rs", "done", 1_i64, 1_i64),
        )
        .expect("insert file job");

        conn.execute(
            "INSERT INTO commenter_artifacts (
                run_id, file_job_id, kind, storage_path, byte_size, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                1_i64,
                1_i64,
                "model_response",
                "runs/run-artifact/response/1.json",
                128_i64,
                1_i64,
            ),
        )
        .expect("insert artifact");

        conn.execute(
            "INSERT INTO commenter_run_events (
                run_id, file_job_id, level, event_type, message, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                1_i64,
                1_i64,
                "warn",
                "artifact_written",
                "artifact stored",
                1_i64,
            ),
        )
        .expect("insert event");
    }

    #[test]
    fn schema_exposes_settings_carrier_columns_on_profiles_and_runs() {
        let conn = open_in_memory().expect("in-memory db");

        let profile_columns =
            list_column_names(&conn, "commenter_project_profiles").expect("profile columns");
        let run_columns = list_column_names(&conn, "commenter_queue_runs").expect("run columns");

        assert!(profile_columns.contains(&"settings_json".to_string()));
        assert!(run_columns.contains(&"run_settings_json".to_string()));
    }

    #[test]
    fn schema_creates_app_settings_and_credential_profile_carriers() {
        let conn = open_in_memory().expect("in-memory db");

        let tables = list_table_names(&conn).expect("table list");
        assert!(tables.contains(&"commenter_app_settings".to_string()));
        assert!(tables.contains(&"commenter_credential_profiles".to_string()));

        let app_settings_columns =
            list_column_names(&conn, "commenter_app_settings").expect("app settings columns");
        assert!(app_settings_columns.contains(&"api_bearer_token".to_string()));
        assert!(!app_settings_columns.contains(&"request_mode".to_string()));

        let credential_profile_columns = list_column_names(&conn, "commenter_credential_profiles")
            .expect("credential profile columns");
        assert!(credential_profile_columns.contains(&"profile_key".to_string()));
        assert!(credential_profile_columns.contains(&"source_kind".to_string()));
        assert!(credential_profile_columns.contains(&"source_reference".to_string()));
        assert!(!credential_profile_columns.contains(&"inline_secret".to_string()));
    }

    #[test]
    fn settings_json_fields_round_trip_through_sqlite_as_non_secret_references() {
        let conn = open_in_memory().expect("in-memory db");

        conn.execute(
            "INSERT INTO commenter_credential_profiles (
                profile_key, display_name, source_kind, source_reference, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                "default-openai",
                "Default OpenAI",
                "env_var",
                "OPENAI_API_KEY",
                9_i64,
                9_i64,
            ),
        )
        .expect("insert credential profile");

        let project_settings_json = serde_json::to_string(&CommentProjectSettings {
            credential_profile_key: "default-openai".to_string(),
            default_run_mode: CommentRunMode::Review,
            default_max_workers: 4,
            default_max_retries: 2,
            default_max_files: 50,
            allow_light_rewrite: true,
            json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            api_base_url: "https://example.com".to_string(),
            api_model: "glm-5.0".to_string(),
            api_bearer_token: String::new(),
            request_timeout_secs: 600,
        })
        .expect("encode project settings");

        let run_settings_json = serde_json::to_string(&CommentRunSettingsSnapshot {
            credential_profile_key: "default-openai".to_string(),
            run_mode: CommentRunMode::Auto,
            max_workers: 8,
            max_retries: 3,
            max_files: 100,
            allow_light_rewrite: false,
            json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
        })
        .expect("encode run settings");

        conn.execute(
            "INSERT INTO commenter_project_profiles (
                project_key, root_path, profile_name, prompt_template, settings_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                "project-b",
                "C:\\repo",
                "default",
                "prompt",
                Some(project_settings_json.clone()),
                10_i64,
                10_i64,
            ),
        )
        .expect("insert profile");

        conn.execute(
            "INSERT INTO commenter_queue_runs (
                profile_id, run_key, status, total_jobs, completed_jobs, run_settings_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                1_i64,
                "run-2",
                "queued",
                0_i64,
                0_i64,
                Some(run_settings_json.clone()),
                11_i64,
                11_i64,
            ),
        )
        .expect("insert run");

        let stored_project_settings: Option<String> = conn
            .query_row(
                "SELECT settings_json FROM commenter_project_profiles WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("select profile settings");
        let stored_run_settings: Option<String> = conn
            .query_row(
                "SELECT run_settings_json FROM commenter_queue_runs WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("select run settings");

        let decoded_project_settings: CommentProjectSettings = serde_json::from_str(
            stored_project_settings
                .as_deref()
                .expect("stored project settings"),
        )
        .expect("decode project settings");
        let decoded_run_settings: CommentRunSettingsSnapshot =
            serde_json::from_str(stored_run_settings.as_deref().expect("stored run settings"))
                .expect("decode run settings");

        assert_eq!(
            stored_project_settings.as_deref(),
            Some(project_settings_json.as_str())
        );
        assert_eq!(
            stored_run_settings.as_deref(),
            Some(run_settings_json.as_str())
        );
        assert_eq!(
            decoded_project_settings.credential_profile_key,
            "default-openai"
        );
        assert_eq!(
            decoded_run_settings.credential_profile_key,
            "default-openai"
        );
        assert!(!stored_project_settings
            .as_deref()
            .unwrap_or_default()
            .contains("credential_source"));
        assert!(!stored_run_settings
            .as_deref()
            .unwrap_or_default()
            .contains("credential_source"));
        assert!(!stored_project_settings
            .as_deref()
            .unwrap_or_default()
            .contains("inline_secret"));
        assert!(!stored_run_settings
            .as_deref()
            .unwrap_or_default()
            .contains("inline_secret"));
    }

    #[test]
    fn app_settings_round_trip_as_global_limits_and_token_row() {
        let conn = open_in_memory().expect("in-memory db");

        conn.execute(
            "UPDATE commenter_app_settings
             SET global_max_workers = ?1,
                 api_concurrency_limit = ?2,
                 api_bearer_token = ?3,
                 updated_at = ?4
             WHERE id = 1",
            (12_i64, 6_i64, "Bearer global-token", 9_i64),
        )
        .expect("update app settings");

        let row = conn
            .query_row(
                "SELECT global_max_workers, api_concurrency_limit, api_bearer_token, updated_at
                 FROM commenter_app_settings WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .expect("select app settings");

        assert_eq!(row, (12, 6, "Bearer global-token".to_string(), 9));
    }

    #[test]
    fn migrate_upgrades_legacy_run_and_job_tables_in_place() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE commenter_schema_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO commenter_schema_meta(key, value) VALUES ('version', '2');

            CREATE TABLE commenter_project_profiles (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                project_key         TEXT NOT NULL UNIQUE,
                root_path           TEXT NOT NULL,
                profile_name        TEXT NOT NULL,
                prompt_template     TEXT NOT NULL,
                include_globs_json  TEXT,
                exclude_globs_json  TEXT,
                created_at          INTEGER NOT NULL,
                updated_at          INTEGER NOT NULL
            );

            CREATE TABLE commenter_queue_runs (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                profile_id          INTEGER NOT NULL,
                run_key             TEXT NOT NULL UNIQUE,
                status              TEXT NOT NULL CHECK (status IN ('queued','running','succeeded','failed','cancelled')),
                requested_by        TEXT,
                total_jobs          INTEGER NOT NULL DEFAULT 0,
                completed_jobs      INTEGER NOT NULL DEFAULT 0,
                created_at          INTEGER NOT NULL,
                updated_at          INTEGER NOT NULL,
                started_at          INTEGER,
                finished_at         INTEGER,
                FOREIGN KEY(profile_id) REFERENCES commenter_project_profiles(id) ON DELETE CASCADE
            );

            CREATE TABLE commenter_file_jobs (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id              INTEGER NOT NULL,
                relative_path       TEXT NOT NULL,
                status              TEXT NOT NULL CHECK (status IN ('pending','processing','succeeded','failed','skipped')),
                language_hint       TEXT,
                retry_count         INTEGER NOT NULL DEFAULT 0,
                created_at          INTEGER NOT NULL,
                updated_at          INTEGER NOT NULL,
                started_at          INTEGER,
                finished_at         INTEGER,
                error_message       TEXT,
                FOREIGN KEY(run_id) REFERENCES commenter_queue_runs(id) ON DELETE CASCADE,
                UNIQUE(run_id, relative_path)
            );

            INSERT INTO commenter_project_profiles (
                id, project_key, root_path, profile_name, prompt_template, created_at, updated_at
            ) VALUES (1, 'legacy-project', 'C:\repo', 'legacy', 'prompt', 1, 1);

            INSERT INTO commenter_queue_runs (
                id, profile_id, run_key, status, total_jobs, completed_jobs, created_at, updated_at
            ) VALUES (1, 1, 'legacy-run', 'succeeded', 1, 1, 1, 1);

            INSERT INTO commenter_file_jobs (
                id, run_id, relative_path, status, retry_count, created_at, updated_at
            ) VALUES (1, 1, 'src/main.rs', 'processing', 1, 1, 1);
            "#,
        )
        .expect("seed legacy schema");

        migrate(&conn).expect("upgrade legacy schema");

        let queue_columns =
            list_column_names(&conn, "commenter_queue_runs").expect("queue run columns");
        let credential_columns = list_column_names(&conn, "commenter_credential_profiles")
            .expect("credential profile columns");
        assert!(queue_columns.contains(&"run_settings_json".to_string()));
        assert!(credential_columns.contains(&"source_reference".to_string()));
        assert!(!credential_columns.contains(&"inline_secret".to_string()));

        let counts = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM commenter_project_profiles),
                    (SELECT COUNT(*) FROM commenter_queue_runs),
                    (SELECT COUNT(*) FROM commenter_file_jobs)",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .expect("count upgraded rows");
        assert_eq!(counts, (1, 1, 1));

        let migrated_statuses = conn
            .query_row(
                "SELECT
                    (SELECT status FROM commenter_queue_runs WHERE id = 1),
                    (SELECT status FROM commenter_file_jobs WHERE id = 1)",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("select migrated statuses");
        assert_eq!(
            migrated_statuses,
            ("completed".to_string(), "writing".to_string())
        );

        conn.execute(
            "INSERT INTO commenter_queue_runs (
                profile_id, run_key, status, total_jobs, completed_jobs, run_settings_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                1_i64,
                "upgraded-run",
                "stopped_by_limit",
                0_i64,
                0_i64,
                Some("{\"credential_profile_key\":\"team-default\",\"run_mode\":\"review\"}"),
                2_i64,
                2_i64,
            ),
        )
        .expect("insert upgraded run");

        conn.execute(
            "INSERT INTO commenter_file_jobs (
                run_id, relative_path, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)",
            (2_i64, "src/other.rs", "review_needed", 2_i64, 2_i64),
        )
        .expect("insert upgraded file job");
    }

    #[test]
    fn migrate_rejects_legacy_inline_secret_profiles_without_safe_reference() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            r#"
            CREATE TABLE commenter_schema_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO commenter_schema_meta(key, value) VALUES ('version', '3');

            CREATE TABLE commenter_credential_profiles (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                profile_key      TEXT NOT NULL UNIQUE,
                display_name     TEXT NOT NULL,
                source_kind      TEXT NOT NULL,
                inline_secret    TEXT,
                created_at       INTEGER NOT NULL,
                updated_at       INTEGER NOT NULL
            );

            INSERT INTO commenter_credential_profiles (
                id, profile_key, display_name, source_kind, inline_secret, created_at, updated_at
            ) VALUES (1, 'legacy-inline', 'Legacy Inline', 'inline_secret', 'secret-token', 1, 1);
            "#,
        )
        .expect("seed legacy credential schema");

        let result = migrate(&conn);
        assert!(result.is_err(), "unsafe credential migration should fail");

        let tables = list_table_names(&conn).expect("table list after failed migrate");
        assert!(tables.contains(&"commenter_credential_profiles".to_string()));
        assert!(!tables.contains(&"commenter_app_settings".to_string()));

        let columns = list_column_names(&conn, "commenter_credential_profiles")
            .expect("credential columns after failed migrate");
        assert!(columns.contains(&"inline_secret".to_string()));
        assert!(!columns.contains(&"source_reference".to_string()));

        let row = conn
            .query_row(
                "SELECT profile_key, inline_secret FROM commenter_credential_profiles WHERE id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("legacy credential row preserved");
        assert_eq!(
            row,
            ("legacy-inline".to_string(), "secret-token".to_string())
        );

        let schema_version: String = conn
            .query_row(
                "SELECT value FROM commenter_schema_meta WHERE key = 'version'",
                [],
                |row| row.get(0),
            )
            .expect("schema version preserved");
        assert_eq!(schema_version, "3");
    }
}
