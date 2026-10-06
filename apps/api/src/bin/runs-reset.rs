//! Explicit operator-only Runs reset. No application bootstrap or migrations.
use futures_util::TryStreamExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Connection, PgConnection, Postgres, Row, Transaction};
use std::{collections::BTreeMap, fs::File, io::Read, path::PathBuf};
use uuid::Uuid;

type Result<T, E = &'static str> = std::result::Result<T, E>;
const HELP: &str = "runs-reset [--apply] --database-url URL --environment NAME --database NAME --pgdata-sha256 SHA256 --system-id DECIMAL (--owner UUID | --all-runs) [--include-source-less-legacy] [--backup-file FILE --backup-sha256 SHA256]\nApply also requires --confirm-environment NAME --confirm-scope owner:UUID|all-runs[:source-less-legacy] --confirm-digest SHA256 --confirm-backup --confirm-quiesced.\nDry-run is read-only. Final dry-run must include the actual restricted backup used for apply. Reports never contain database URLs, backup paths, row content, emails, or FIT bytes.";
// Fixed deletion order. Slots are global operational state and NEVER deleted.
const DELETE_ORDER: &[&str] = &[
    "runs_export_chunks",
    "runs_exports",
    "runs_estimate_dependencies",
    "runs_estimates",
    "runs_jobs",
    "runs_revision_chunks",
    "runs_manifests",
    "runs_revisions",
    "activities",
    "runs_activities",
    "runs_sources",
    "runs_import_reports",
    "runs_tombstones",
    "runs_legacy_summaries",
    "extractions",
];
const REQUIRED: &[&str] = &[
    "users",
    "sessions",
    "oauth_states",
    "oauth_login_requests",
    "oauth_authorization_codes",
    "oauth_access_tokens",
    "oauth_refresh_tokens",
    "transcript_entries",
    "legacy_imports",
    "_sqlx_migrations",
    "runs_slots",
];

struct Options {
    values: BTreeMap<String, String>,
    apply: bool,
    legacy: bool,
    owner: Option<String>,
    backup: Option<Backup>,
}
struct Backup {
    file: File,
    manifest: Value,
}
impl Options {
    fn value(&self, key: &str) -> &str {
        self.values.get(key).map(String::as_str).unwrap_or("")
    }
    fn scope(&self) -> String {
        let mut scope = self
            .owner
            .as_ref()
            .map(|owner| format!("owner:{owner}"))
            .unwrap_or_else(|| "all-runs".into());
        if self.legacy {
            scope.push_str(":source-less-legacy");
        }
        scope
    }
    fn parse() -> Result<Option<Self>> {
        let mut values = BTreeMap::new();
        let mut flags = std::collections::BTreeSet::new();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--help" && values.is_empty() && flags.is_empty() && args.next().is_none() {
                println!("{HELP}");
                return Ok(None);
            }
            match arg.as_str() {
                "--apply"
                | "--all-runs"
                | "--include-source-less-legacy"
                | "--confirm-backup"
                | "--confirm-quiesced" => {
                    if !flags.insert(arg) {
                        return Err("DUPLICATE_ARGUMENT");
                    }
                }
                "--database-url"
                | "--environment"
                | "--database"
                | "--pgdata-sha256"
                | "--system-id"
                | "--owner"
                | "--confirm-environment"
                | "--confirm-scope"
                | "--confirm-digest"
                | "--backup-file"
                | "--backup-sha256" => {
                    let value = args
                        .next()
                        .filter(|value| !value.is_empty() && !value.starts_with("--"))
                        .ok_or("MISSING_ARGUMENT_VALUE")?;
                    if values.insert(arg, value).is_some() {
                        return Err("DUPLICATE_ARGUMENT");
                    }
                }
                _ => return Err("UNKNOWN_ARGUMENT"),
            }
        }
        if values.contains_key("--owner") && flags.contains("--all-runs") {
            return Err("AMBIGUOUS_SCOPE");
        }
        for (key, error) in [
            ("--database-url", "MISSING_DATABASE_URL"),
            ("--environment", "MISSING_ENVIRONMENT"),
            ("--database", "MISSING_DATABASE"),
            ("--pgdata-sha256", "MISSING_PGDATA_FINGERPRINT"),
            ("--system-id", "MISSING_SYSTEM_ID"),
        ] {
            if !values.contains_key(key) {
                return Err(error);
            }
        }
        for key in ["--environment", "--database"] {
            let value = &values[key];
            if value.len() > 128
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            {
                return Err("INVALID_TARGET_LABEL");
            }
        }
        if !is_hash(&values["--pgdata-sha256"]) {
            return Err("INVALID_PGDATA_FINGERPRINT");
        }
        if values["--system-id"].parse::<u64>().is_err() {
            return Err("INVALID_SYSTEM_ID");
        }
        let owner = values
            .get("--owner")
            .map(|owner| {
                Uuid::parse_str(owner)
                    .map(|id| id.to_string())
                    .map_err(|_| "INVALID_OWNER")
            })
            .transpose()?;
        if owner.is_none() && !flags.contains("--all-runs") {
            return Err("MISSING_SCOPE");
        }
        let mut options = Self {
            values,
            apply: flags.contains("--apply"),
            legacy: flags.contains("--include-source-less-legacy"),
            owner,
            backup: None,
        };
        if options.apply {
            if options.value("--confirm-environment") != options.value("--environment") {
                return Err("CONFIRM_ENVIRONMENT_MISMATCH");
            }
            if options.value("--confirm-scope") != options.scope() {
                return Err("CONFIRM_SCOPE_MISMATCH");
            }
            if !is_hash(options.value("--confirm-digest")) {
                return Err("INVALID_CONFIRM_DIGEST");
            }
            if !flags.contains("--confirm-backup") {
                return Err("BACKUP_CONFIRMATION_REQUIRED");
            }
            if !flags.contains("--confirm-quiesced") {
                return Err("QUIESCE_CONFIRMATION_REQUIRED");
            }
            if options.value("--backup-file").is_empty() {
                return Err("BACKUP_REQUIRED");
            }
        } else if flags.contains("--confirm-backup")
            || flags.contains("--confirm-quiesced")
            || options
                .values
                .keys()
                .any(|key| key.starts_with("--confirm-"))
        {
            return Err("CONFIRMATION_REQUIRES_APPLY");
        }
        if options.values.contains_key("--backup-file")
            != options.values.contains_key("--backup-sha256")
        {
            return Err("BACKUP_FILE_AND_HASH_REQUIRED");
        }
        if options.values.contains_key("--backup-file") {
            options.backup = Some(Backup::open(
                options.value("--backup-file"),
                options.value("--backup-sha256"),
            )?);
        }
        Ok(Some(options))
    }
}
fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn file_hash(file: &mut File) -> Result<String> {
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "ARTIFACT_READ_FAILED")?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let length = file.read(&mut buffer).map_err(|_| "ARTIFACT_READ_FAILED")?;
        if length == 0 {
            break;
        }
        hash.update(&buffer[..length]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
impl Backup {
    fn open(path: &str, expected: &str) -> Result<Self> {
        if !is_hash(expected) {
            return Err("INVALID_BACKUP_HASH");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)
                .map_err(|_| "BACKUP_UNREADABLE")?;
            let metadata = file.metadata().map_err(|_| "BACKUP_UNREADABLE")?;
            if !metadata.is_file()
                || metadata.len() == 0
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.mode() & 0o077 != 0
                || metadata.nlink() != 1
            {
                return Err("BACKUP_NOT_OWNED_RESTRICTED_REGULAR_FILE");
            }
            if file_hash(&mut file)? != expected {
                return Err("BACKUP_HASH_MISMATCH");
            }
            let manifest = json!({"sha256":expected,"bytes":metadata.len(),"device":metadata.dev(),"inode":metadata.ino(),"owner":metadata.uid(),"mode":metadata.mode() & 0o777});
            Ok(Self { file, manifest })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err("BACKUP_PLATFORM_UNSUPPORTED")
        }
    }
    fn verify(&mut self) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = self.file.metadata().map_err(|_| "BACKUP_UNREADABLE")?;
            if !metadata.is_file()
                || metadata.nlink() != 1
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.mode() & 0o077 != 0
                || metadata.len() != self.manifest["bytes"].as_u64().ok_or("BACKUP_CHANGED")?
                || file_hash(&mut self.file)?
                    != self.manifest["sha256"].as_str().ok_or("BACKUP_CHANGED")?
            {
                return Err("BACKUP_CHANGED");
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            Err("BACKUP_PLATFORM_UNSUPPORTED")
        }
    }
}
fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
fn selected(table: &str) -> String {
    let owner = "($1::text IS NULL OR t.owner_id=$1)";
    match table {
        "extractions" => "$2::boolean AND ($1::text IS NULL OR t.user_id=$1) AND lower(t.activity_type)='running' AND NOT EXISTS(SELECT 1 FROM public.runs_activities a WHERE a.id=t.id)".into(),
        "runs_legacy_summaries" => format!("$2::boolean AND {owner} AND EXISTS(SELECT 1 FROM public.extractions e WHERE e.id=t.id AND e.user_id=t.owner_id AND lower(e.activity_type)='running') AND NOT EXISTS(SELECT 1 FROM public.runs_activities a WHERE a.id=t.id)"),
        "activities" => format!("{owner} AND (EXISTS(SELECT 1 FROM public.runs_activities a WHERE a.id=t.id AND a.owner_id=t.owner_id) OR ($2::boolean AND lower(t.sport)='running' AND EXISTS(SELECT 1 FROM public.extractions e WHERE e.id=t.id AND e.user_id=t.owner_id AND lower(e.activity_type)='running')))"),
        "runs_export_chunks" => "($2::boolean OR NOT $2::boolean) AND EXISTS(SELECT 1 FROM public.runs_exports e WHERE e.token=t.token AND ($1::text IS NULL OR e.owner_id=$1))".into(),
        "runs_slots" => "false AND $1::text IS NULL AND $2::boolean".into(),
        _ if DELETE_ORDER.contains(&table) => format!("{owner} AND ($2::boolean OR NOT $2::boolean)"),
        _ => "false AND $1::text IS NULL AND $2::boolean".into(),
    }
}
async fn execute(tx: &mut Transaction<'_, Postgres>, statement: &str) -> Result<()> {
    // Only fixed statements and quoted catalog table names reach this helper.
    sqlx::query(sqlx::AssertSqlSafe(statement))
        .execute(&mut **tx)
        .await
        .map_err(|_| "DATABASE_OPERATION_REFUSED")?;
    Ok(())
}
async fn tables(tx: &mut Transaction<'_, Postgres>) -> Result<Vec<String>> {
    let tables = sqlx::query_scalar::<_, String>("SELECT c.relname::text FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind IN ('r','p') ORDER BY c.relname COLLATE \"C\"")
        .fetch_all(&mut **tx).await.map_err(|_| "SCHEMA_READ_FAILED")?;
    for name in DELETE_ORDER.iter().chain(REQUIRED.iter()) {
        if !tables.iter().any(|table| table == name) {
            return Err("REQUIRED_SCHEMA_MISSING");
        }
    }
    Ok(tables)
}
async fn identity(tx: &mut Transaction<'_, Postgres>, options: &Options) -> Result<Value> {
    let row = sqlx::query("SELECT current_database()::text AS database, current_setting('data_directory') AS pgdata, current_setting('server_version_num') AS version, (SELECT system_identifier::text FROM pg_control_system()) AS system_id, pg_is_in_recovery() AS recovery")
        .fetch_one(&mut **tx).await.map_err(|_| "TARGET_IDENTITY_UNAVAILABLE")?;
    let database: String = row
        .try_get("database")
        .map_err(|_| "TARGET_IDENTITY_UNAVAILABLE")?;
    let pgdata: String = row
        .try_get("pgdata")
        .map_err(|_| "TARGET_IDENTITY_UNAVAILABLE")?;
    let version: String = row
        .try_get("version")
        .map_err(|_| "TARGET_IDENTITY_UNAVAILABLE")?;
    let system_id: String = row
        .try_get("system_id")
        .map_err(|_| "TARGET_IDENTITY_UNAVAILABLE")?;
    if database != options.value("--database")
        || sha(pgdata.as_bytes()) != options.value("--pgdata-sha256")
        || system_id != options.value("--system-id")
        || version
            .parse::<u32>()
            .map_err(|_| "TARGET_IDENTITY_MISMATCH")?
            / 10000
            != 18
        || row
            .try_get::<bool, _>("recovery")
            .map_err(|_| "TARGET_IDENTITY_UNAVAILABLE")?
    {
        return Err("TARGET_IDENTITY_MISMATCH");
    }
    Ok(
        json!({"database":database,"pgdataSha256":sha(pgdata.as_bytes()),"systemId":system_id,"postgresVersion":version}),
    )
}
async fn supported_schema(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    let allowlist: Vec<&str> = DELETE_ORDER.to_vec();
    let unsafe_schema: bool = sqlx::query_scalar(r#"SELECT
        EXISTS(SELECT 1 FROM pg_constraint f JOIN pg_class target ON target.oid=f.confrelid JOIN pg_namespace tn ON tn.oid=target.relnamespace JOIN pg_class child ON child.oid=f.conrelid JOIN pg_namespace cn ON cn.oid=child.relnamespace WHERE f.contype='f' AND tn.nspname='public' AND target.relname=ANY($1::text[]) AND (cn.nspname<>'public' OR NOT child.relname=ANY($1::text[])))
        OR EXISTS(SELECT 1 FROM pg_constraint f JOIN pg_class c ON c.oid=f.conrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND NOT f.convalidated)
        OR EXISTS(SELECT 1 FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND (t.tgenabled<>'O' OR (NOT t.tgisinternal AND NOT (t.tgname=ANY(ARRAY['runs_sources_immutable','runs_revisions_immutable','runs_manifests_immutable','runs_revision_chunks_immutable','runs_export_chunks_immutable']) AND t.tgtype=19))))
        OR EXISTS(SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND (c.relrowsecurity OR c.relforcerowsecurity OR (c.relname=ANY($1::text[]) AND (c.relkind<>'r' OR c.relispartition))))
        OR EXISTS(SELECT 1 FROM pg_inherits i JOIN pg_class c ON c.oid=i.inhparent JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relname=ANY($1::text[]))"#)
        .bind(&allowlist).fetch_one(&mut **tx).await.map_err(|_| "SCHEMA_GUARD_FAILED")?;
    if unsafe_schema {
        return Err("UNSUPPORTED_SCHEMA_DEPENDENCY_OR_TRIGGER");
    }
    Ok(())
}
async fn schema(tx: &mut Transaction<'_, Postgres>) -> Result<Value> {
    let migrations = sqlx::query("SELECT version,success,checksum AS checksum_bytes,encode(checksum,'hex') AS checksum FROM public._sqlx_migrations ORDER BY version")
        .fetch_all(&mut **tx).await.map_err(|_| "MIGRATION_READ_FAILED")?;
    let expected = sqlx::migrate!();
    if migrations.len() != expected.iter().count() {
        return Err("APPLICATION_SCHEMA_MISMATCH");
    }
    let mut versions = Vec::new();
    for (row, migration) in migrations.iter().zip(expected.iter()) {
        let version: i64 = row
            .try_get("version")
            .map_err(|_| "MIGRATION_READ_FAILED")?;
        let checksum: String = row
            .try_get("checksum")
            .map_err(|_| "MIGRATION_READ_FAILED")?;
        if version != migration.version
            || row
                .try_get::<&[u8], _>("checksum_bytes")
                .map_err(|_| "MIGRATION_READ_FAILED")?
                != migration.checksum.as_ref()
            || !row
                .try_get::<bool, _>("success")
                .map_err(|_| "MIGRATION_READ_FAILED")?
        {
            return Err("APPLICATION_SCHEMA_MISMATCH");
        }
        versions.push(json!({"version":version,"checksum":checksum}));
    }
    let definitions = sqlx::query_scalar::<_,String>(r#"SELECT definition FROM (
        SELECT 'column:'||n.nspname||'.'||c.relname||':'||a.attnum||':'||a.attname||':'||format_type(a.atttypid,a.atttypmod)||':'||a.attnotnull||':'||coalesce(pg_get_expr(d.adbin,d.adrelid),'') AS definition FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace LEFT JOIN pg_attrdef d ON d.adrelid=a.attrelid AND d.adnum=a.attnum WHERE n.nspname='public' AND a.attnum>0 AND NOT a.attisdropped
        UNION ALL SELECT 'constraint:'||c.conrelid::regclass::text||':'||c.conname||':'||pg_get_constraintdef(c.oid) FROM pg_constraint c JOIN pg_namespace n ON n.oid=c.connamespace WHERE n.nspname='public'
        UNION ALL SELECT 'index:'||indexname||':'||indexdef FROM pg_indexes WHERE schemaname='public'
        UNION ALL SELECT 'trigger:'||t.tgrelid::regclass::text||':'||t.tgenabled::text||':'||pg_get_triggerdef(t.oid) FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public'
        UNION ALL SELECT 'function:'||p.oid::regprocedure::text||':'||pg_get_functiondef(p.oid) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='public' AND p.prokind='f'
        UNION ALL SELECT 'relation:'||c.relname||':'||c.relkind::text||':'||c.relrowsecurity||':'||c.relforcerowsecurity FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public'
        UNION ALL SELECT 'policy:'||tablename||':'||policyname||':'||coalesce(qual,'')||':'||coalesce(with_check,'') FROM pg_policies WHERE schemaname='public'
    ) definitions ORDER BY definition COLLATE "C""#).fetch_all(&mut **tx).await.map_err(|_| "SCHEMA_READ_FAILED")?;
    Ok(
        json!({"migrations":versions,"sha256":sha(serde_json::to_vec(&definitions).map_err(|_| "MANIFEST_ENCODING_FAILED")?.as_slice())}),
    )
}
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    table: &str,
    predicate: &str,
    options: &Options,
) -> Result<Value> {
    // Hash inside PostgreSQL: private content never leaves the database or reaches reports.
    let query = format!(
        "SELECT hash,bytes FROM (SELECT encode(sha256(convert_to(to_jsonb(t)::text,'UTF8')),'hex') AS hash,pg_column_size(t)::bigint AS bytes FROM public.{} t WHERE {predicate}) hashed ORDER BY hash COLLATE \"C\"",
        quote(table)
    );
    // Identifiers are catalog-derived and SQL-quoted; predicates are fixed source clauses, values are bound.
    let mut rows = sqlx::query(sqlx::AssertSqlSafe(query.as_str()))
        .bind(options.owner.as_deref())
        .bind(options.legacy)
        .fetch(&mut **tx);
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut bytes = 0u64;
    while let Some(row) = rows.try_next().await.map_err(|_| "SNAPSHOT_READ_FAILED")? {
        let digest: String = row.try_get("hash").map_err(|_| "SNAPSHOT_READ_FAILED")?;
        let size: i64 = row.try_get("bytes").map_err(|_| "SNAPSHOT_READ_FAILED")?;
        hash.update(digest.as_bytes());
        count += 1;
        bytes = bytes
            .checked_add(size.try_into().map_err(|_| "SNAPSHOT_READ_FAILED")?)
            .ok_or("SNAPSHOT_TOO_LARGE")?;
    }
    Ok(json!({"rows":count,"rowBytes":bytes,"contentSha256":format!("{:x}",hash.finalize())}))
}
async fn leases(tx: &mut Transaction<'_, Postgres>, options: &Options) -> Result<Value> {
    let active_jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM public.runs_jobs WHERE ($1::text IS NULL OR owner_id=$1) AND lease_owner IS NOT NULL AND lease_until>clock_timestamp()")
        .bind(options.owner.as_deref()).fetch_one(&mut **tx).await.map_err(|_| "LEASE_READ_FAILED")?;
    let global_slots: i64 = sqlx::query_scalar("SELECT count(*) FROM public.runs_slots WHERE lease_owner IS NOT NULL AND lease_until>clock_timestamp()")
        .fetch_one(&mut **tx).await.map_err(|_| "LEASE_READ_FAILED")?;
    // Missing registry means empty. Any nonempty or malformed registry blocks reset;
    // crash leftovers must not be silently cleared while another replica could stream.
    let guarded_exports: i64 = sqlx::query_scalar("SELECT count(*) FROM public.runs_exports WHERE ($1::text IS NULL OR owner_id=$1) AND coalesce(meta->'activeReads','[]'::jsonb)<>'[]'::jsonb")
        .bind(options.owner.as_deref()).fetch_one(&mut **tx).await.map_err(|_| "LEASE_READ_FAILED")?;
    let statuses = sqlx::query("SELECT status,count(*)::bigint AS count FROM public.runs_jobs WHERE ($1::text IS NULL OR owner_id=$1) GROUP BY status ORDER BY status")
        .bind(options.owner.as_deref()).fetch_all(&mut **tx).await.map_err(|_| "LEASE_READ_FAILED")?;
    let mut jobs = BTreeMap::new();
    for row in statuses {
        jobs.insert(
            row.try_get::<String, _>("status")
                .map_err(|_| "LEASE_READ_FAILED")?,
            row.try_get::<i64, _>("count")
                .map_err(|_| "LEASE_READ_FAILED")?,
        );
    }
    Ok(
        json!({"activeAffectedJobLeases":active_jobs,"activeGlobalSlots":global_slots,"affectedExportsWithActiveReads":guarded_exports,"jobsByStatus":jobs}),
    )
}
async fn manifest(
    tx: &mut Transaction<'_, Postgres>,
    options: &Options,
    names: &[String],
    executable: &str,
) -> Result<Value> {
    let mut selected_rows = BTreeMap::new();
    let mut protected = BTreeMap::new();
    for table in names {
        let predicate = selected(table);
        if DELETE_ORDER.contains(&table.as_str()) {
            selected_rows.insert(
                table,
                snapshot(
                    tx,
                    table,
                    &format!("COALESCE(({predicate}),false)"),
                    options,
                )
                .await?,
            );
        }
        protected.insert(
            table,
            snapshot(
                tx,
                table,
                &format!("NOT COALESCE(({predicate}),false)"),
                options,
            )
            .await?,
        );
    }
    let source_bytes: i64 = sqlx::query_scalar("SELECT coalesce(sum(size_bytes),0)::bigint FROM public.runs_sources WHERE ($1::text IS NULL OR owner_id=$1)")
        .bind(options.owner.as_deref()).fetch_one(&mut **tx).await.map_err(|_| "SNAPSHOT_READ_FAILED")?;
    let export_bytes: i64 = sqlx::query_scalar("SELECT coalesce(sum(byte_length),0)::bigint FROM public.runs_exports WHERE ($1::text IS NULL OR owner_id=$1)")
        .bind(options.owner.as_deref()).fetch_one(&mut **tx).await.map_err(|_| "SNAPSHOT_READ_FAILED")?;
    Ok(
        json!({"manifestVersion":1,"environment":options.value("--environment"),"target":identity(tx,options).await?,"scope":options.scope(),"allowlist":DELETE_ORDER,"schema":schema(tx).await?,"application":{"packageVersion":env!("CARGO_PKG_VERSION"),"executableSha256":executable,"runsVersions":garmin_fit_extractor_api::runs::store::versions()},"backup":options.backup.as_ref().map(|backup| &backup.manifest),"selected":selected_rows,"sourceBytes":source_bytes,"exportBytes":export_bytes,"leases":leases(tx,options).await?,"protected":protected}),
    )
}
fn manifest_digest(value: &Value) -> Result<String> {
    Ok(sha(
        &serde_json::to_vec(value).map_err(|_| "MANIFEST_ENCODING_FAILED")?
    ))
}
async fn invariants(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    // FKs cover native children. These references deliberately lack FKs.
    let invalid: bool = sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM public.activities p WHERE NOT EXISTS(SELECT 1 FROM public.runs_activities a WHERE a.id=p.id AND a.owner_id=p.owner_id) AND NOT EXISTS(SELECT 1 FROM public.extractions e WHERE e.id=p.id AND e.user_id=p.owner_id))
        OR EXISTS(SELECT 1 FROM public.runs_exports e CROSS JOIN LATERAL unnest(e.activity_ids,e.manifest_ids) pin(activity_id,manifest_id) WHERE NOT EXISTS(SELECT 1 FROM public.runs_manifests m WHERE m.id=pin.manifest_id AND m.activity_id=pin.activity_id AND m.owner_id=e.owner_id))
        OR EXISTS(SELECT 1 FROM public.runs_activities a CROSS JOIN LATERAL jsonb_array_elements(a.duplicate_evidence) evidence WHERE evidence->>'activityId' IS NOT NULL AND NOT EXISTS(SELECT 1 FROM public.runs_activities peer WHERE peer.id=evidence->>'activityId' AND peer.owner_id=a.owner_id))
        OR EXISTS(SELECT 1 FROM public.runs_sources s WHERE NOT EXISTS(SELECT 1 FROM public.runs_activities a WHERE a.source_id=s.id AND a.owner_id=s.owner_id))
        OR EXISTS(SELECT 1 FROM public.runs_import_reports r CROSS JOIN LATERAL jsonb_array_elements(coalesce(r.payload->'items','[]'::jsonb)) item WHERE item->>'activityId' IS NOT NULL AND NOT EXISTS(SELECT 1 FROM public.runs_activities a WHERE a.id=item->>'activityId' AND a.owner_id=r.owner_id))
        OR EXISTS(SELECT 1 FROM public.runs_sources WHERE sha256<>pg_catalog.sha256(bytes) OR size_bytes<>octet_length(bytes))"#)
        .fetch_one(&mut **tx).await.map_err(|_| "INVARIANT_READ_FAILED")?;
    if invalid {
        return Err("ORPHAN_OR_SOURCE_INTEGRITY_FAILED");
    }
    Ok(())
}
async fn lock(
    tx: &mut Transaction<'_, Postgres>,
    options: &Options,
    names: &[String],
) -> Result<()> {
    let owners: Vec<String> = if let Some(owner) = &options.owner {
        vec![owner.clone()]
    } else {
        sqlx::query_scalar("SELECT id FROM public.users ORDER BY id COLLATE \"C\"")
            .fetch_all(&mut **tx)
            .await
            .map_err(|_| "OWNER_READ_FAILED")?
    };
    for owner in &owners {
        let acquired: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
                .bind(owner)
                .fetch_one(&mut **tx)
                .await
                .map_err(|_| "OWNER_LOCK_FAILED")?;
        if !acquired {
            return Err("OWNER_NOT_QUIESCENT");
        }
    }
    // ponytail: global table locks favor maintenance safety; per-owner locking can replace them only with a proved writer protocol.
    for table in names {
        let mode = if DELETE_ORDER.contains(&table.as_str()) || table == "runs_slots" {
            "EXCLUSIVE"
        } else {
            "SHARE"
        };
        execute(
            tx,
            &format!("LOCK TABLE public.{} IN {mode} MODE NOWAIT", quote(table)),
        )
        .await?;
    }
    let current: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM public.users WHERE ($1::text IS NULL OR id=$1) ORDER BY id COLLATE \"C\"",
    )
    .bind(options.owner.as_deref())
    .fetch_all(&mut **tx)
    .await
    .map_err(|_| "OWNER_READ_FAILED")?;
    if current != owners {
        return Err("OWNER_SET_CHANGED_OR_UNKNOWN");
    }
    if tables(tx).await? != names {
        return Err("SCHEMA_CHANGED");
    }
    Ok(())
}
async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    options: &mut Options,
    plan: &Value,
    names: &[String],
    executable: &str,
) -> Result<Value> {
    if plan["leases"]["activeAffectedJobLeases"] != 0
        || plan["leases"]["activeGlobalSlots"] != 0
        || plan["leases"]["affectedExportsWithActiveReads"] != 0
    {
        return Err("ACTIVE_LEASES_NOT_QUIESCENT");
    }
    invariants(tx).await?;
    options.backup.as_mut().ok_or("BACKUP_REQUIRED")?.verify()?;
    sqlx::query("UPDATE public.runs_jobs SET status='cancelled',lease_owner=NULL,lease_until=NULL WHERE ($1::text IS NULL OR owner_id=$1) AND status IN ('queued','processing')")
        .bind(options.owner.as_deref()).execute(&mut **tx).await.map_err(|_| "JOB_FENCE_FAILED")?;
    sqlx::query(
        "UPDATE public.runs_exports SET revoked=true WHERE ($1::text IS NULL OR owner_id=$1)",
    )
    .bind(options.owner.as_deref())
    .execute(&mut **tx)
    .await
    .map_err(|_| "EXPORT_FENCE_FAILED")?;
    sqlx::query("UPDATE public.runs_activities SET current_manifest_id=NULL,desired_generation=desired_generation+1,history_generation=history_generation+1 WHERE ($1::text IS NULL OR owner_id=$1)")
        .bind(options.owner.as_deref()).execute(&mut **tx).await.map_err(|_| "GENERATION_FENCE_FAILED")?;
    let mut deleted = BTreeMap::new();
    // Projections are removed before native activities used by their scope predicates.
    for table in DELETE_ORDER {
        let predicate = selected(table);
        let query = format!(
            "DELETE FROM public.{} t WHERE COALESCE(({predicate}),false)",
            quote(table)
        );
        // Table names and predicates come only from the fixed deletion allowlist.
        let rows = sqlx::query(sqlx::AssertSqlSafe(query.as_str()))
            .bind(options.owner.as_deref())
            .bind(options.legacy)
            .execute(&mut **tx)
            .await
            .map_err(|_| "DELETE_REFUSED")?
            .rows_affected();
        if rows
            != plan["selected"][*table]["rows"]
                .as_u64()
                .ok_or("MANIFEST_INVALID")?
        {
            return Err("DELETE_COUNT_MISMATCH");
        }
        deleted.insert(*table, rows);
    }
    execute(tx, "SET CONSTRAINTS ALL IMMEDIATE").await?;
    invariants(tx).await?;
    let after = manifest(tx, options, names, executable).await?;
    if after["protected"] != plan["protected"] {
        return Err("PROTECTED_CONTENT_CHANGED");
    }
    if after["schema"] != plan["schema"]
        || after["target"] != plan["target"]
        || tables(tx).await? != names
    {
        return Err("SCHEMA_OR_TARGET_CHANGED");
    }
    for table in DELETE_ORDER {
        if after["selected"][*table]["rows"] != 0 {
            return Err("APPROVED_ROWS_REMAIN");
        }
    }
    options.backup.as_mut().ok_or("BACKUP_REQUIRED")?.verify()?;
    Ok(
        json!({"deleted":deleted,"protected":after["protected"],"schema":after["schema"],"target":after["target"]}),
    )
}
async fn run(mut options: Options) -> Result<()> {
    let executable: PathBuf =
        std::env::current_exe().map_err(|_| "EXECUTABLE_FINGERPRINT_FAILED")?;
    let executable_hash =
        file_hash(&mut File::open(executable).map_err(|_| "EXECUTABLE_FINGERPRINT_FAILED")?)?;
    let mut connection = PgConnection::connect(options.value("--database-url"))
        .await
        .map_err(|_| "DATABASE_CONNECTION_FAILED")?;
    let mut tx = connection.begin().await.map_err(|_| "TRANSACTION_FAILED")?;
    if options.apply {
        // Identity reads precede locks. READ COMMITTED ensures the held-lock recompute sees all intervening commits.
        execute(
            &mut tx,
            "SET TRANSACTION ISOLATION LEVEL READ COMMITTED READ WRITE",
        )
        .await?;
    } else {
        execute(
            &mut tx,
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .await?;
    }
    execute(&mut tx, "SET LOCAL search_path=pg_catalog,public").await?;
    execute(&mut tx, "SET LOCAL timezone='UTC'").await?;
    execute(&mut tx, "SET LOCAL datestyle='ISO, YMD'").await?;
    execute(&mut tx, "SET LOCAL row_security=off").await?;
    execute(&mut tx, "SET LOCAL lock_timeout='2s'").await?;
    identity(&mut tx, &options).await?;
    let names = tables(&mut tx).await?;
    if options.apply {
        lock(&mut tx, &options, &names).await?;
    }
    supported_schema(&mut tx).await?;
    if let Some(owner) = &options.owner {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.users WHERE id=$1)")
                .bind(owner)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| "OWNER_READ_FAILED")?;
        if !exists {
            return Err("UNKNOWN_OWNER");
        }
    }
    let plan = manifest(&mut tx, &options, &names, &executable_hash).await?;
    let digest = manifest_digest(&plan)?;
    if options.apply {
        if digest != options.value("--confirm-digest") {
            return Err("STALE_OR_WRONG_MANIFEST_DIGEST");
        }
        let result = apply(&mut tx, &mut options, &plan, &names, &executable_hash).await?;
        tx.commit().await.map_err(|_| "COMMIT_FAILED")?;
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"mode":"applied","digest":digest,"manifest":plan,"result":result})
            )
            .map_err(|_| "MANIFEST_ENCODING_FAILED")?
        );
    } else {
        tx.rollback().await.map_err(|_| "TRANSACTION_FAILED")?;
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"mode":"dry-run","digest":digest,"manifest":plan})
            )
            .map_err(|_| "MANIFEST_ENCODING_FAILED")?
        );
    }
    Ok(())
}
#[tokio::main]
async fn main() {
    let result = match Options::parse() {
        Ok(Some(options)) => run(options).await,
        Ok(None) => Ok(()),
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        eprintln!("runs-reset: {error}");
        std::process::exit(2);
    }
}
