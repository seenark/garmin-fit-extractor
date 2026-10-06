use garmin_fit_extractor_api::{
    db,
    runs::{history, jobs, store},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{fs, path::PathBuf, process::Command};
use tokio::time::{Duration, sleep, timeout};
use uuid::Uuid;

const FIT: &[u8] = include_bytes!("fixtures/runs/garmin_run.fit");
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn isolated_pool() -> (PgPool, PgPool, String, Uuid) {
    let url = std::env::var("TEST_DATABASE_URL").expect("disposable TEST_DATABASE_URL");
    let expected = std::env::var("PGDATA").expect("disposable PGDATA identity");
    assert!(
        !expected.is_empty(),
        "disposable PGDATA identity must not be empty"
    );
    let admin = PgPool::connect(&url).await.unwrap();
    let actual: String = sqlx::query_scalar("SHOW data_directory")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(actual, expected, "Refuse another PostgreSQL instance");
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert!((180000..190000).contains(&version.parse::<i32>().unwrap()));
    let schema = format!("runs_lifecycle_{}", Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let selected = schema.clone();
    let pool = PgPoolOptions::new().max_connections(6).after_connect(move |connection, _| {
        let schema = selected.clone();
        Box::pin(async move {
            sqlx::query("SELECT set_config('search_path',$1,false),set_config('application_name',$1,false)")
                .bind(schema).execute(connection).await?;
            Ok(())
        })
    }).connect(&url).await.unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let owner = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$1,$2,$3,$3)",
    )
    .bind(owner.to_string())
    .bind(format!("{owner}@example.test"))
    .bind(db::created_at_now())
    .execute(&pool)
    .await
    .unwrap();
    (admin, pool, schema, owner)
}

async fn close_pool(admin: PgPool, pool: PgPool, schema: String) {
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

async fn import_real(pool: &PgPool, owner: Uuid) -> Uuid {
    unsafe {
        std::env::set_var(
            "RUNS_DECODER_EXECUTABLE",
            env!("CARGO_BIN_EXE_garmin-fit-extractor-api"),
        );
    }
    let admission = jobs::import_admission(pool).await.unwrap();
    let mut spool = Box::pin(jobs::decode_import(
        pool,
        FIT,
        std::future::pending(),
        Some(admission.decoder_hold().unwrap()),
    ))
    .await
    .unwrap();
    spool.protect_cpu(&admission);
    let activity = Box::pin(store::accept_spool(
        pool,
        owner,
        FIT,
        spool,
        admission.holder(),
    ))
    .await
    .unwrap()
    .activity_id;
    drop(admission);
    activity
}

struct BlockingChild {
    directory: PathBuf,
    previous: Option<std::ffi::OsString>,
}
impl BlockingChild {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("runs-child-test-{}", Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let source = directory.join("block.c");
        let executable = directory.join("block");
        // This child consumes the public FIT, reports its real PID/workspace, and
        // waits. It never emits a successful decoder document or numerical data.
        fs::write(&source, format!(r#"#include <stdio.h>
#include <unistd.h>
int main(int argc, char **argv) {{
    if (argc != 3) return 2;
    unsigned char input[4096]; size_t size = fread(input, 1, sizeof input, stdin);
    if (size < 12 || input[8] != '.' || input[9] != 'F' || input[10] != 'I' || input[11] != 'T') return 3;
    FILE *ready = fopen("{}/ready", "w"); if (!ready) return 4;
    fprintf(ready, "%ld\n%s\n", (long)getpid(), argv[2]); fclose(ready);
    for (;;) pause();
}}
"#, directory.display())).unwrap();
        assert!(
            Command::new("cc")
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .status()
                .unwrap()
                .success()
        );
        let previous = std::env::var_os("RUNS_DECODER_EXECUTABLE");
        unsafe {
            std::env::set_var("RUNS_DECODER_EXECUTABLE", &executable);
        }
        Self {
            directory,
            previous,
        }
    }
    async fn ready(&self) -> (libc::pid_t, PathBuf) {
        timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(text) = fs::read_to_string(self.directory.join("ready")) {
                    let mut lines = text.lines();
                    if let (Some(pid), Some(path)) = (lines.next(), lines.next()) {
                        return (pid.parse().unwrap(), PathBuf::from(path));
                    }
                }
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("Real decoder child did not start")
    }
}
impl Drop for BlockingChild {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(value) => std::env::set_var("RUNS_DECODER_EXECUTABLE", value),
                None => std::env::remove_var("RUNS_DECODER_EXECUTABLE"),
            }
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn reaped(pid: libc::pid_t) -> bool {
    let mut status = 0;
    let result = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
    result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_slot_loss_reaps_child_even_when_job_heartbeat_is_blocked() {
    let _serial = SERIAL.lock().await;
    let (admin, pool, schema, owner) = isolated_pool().await;
    let activity = Box::pin(import_real(&pool, owner)).await;
    Box::pin(jobs::run_once(&pool)).await.unwrap();
    Box::pin(jobs::run_once(&pool)).await.unwrap();
    Box::pin(store::reprocess(&pool, owner, activity))
        .await
        .unwrap();
    let child = BlockingChild::new();
    let worker_pool = pool.clone();
    let mut worker = tokio::spawn(async move { Box::pin(jobs::run_once(&worker_pool)).await });
    let (pid, workspace) = child.ready().await;
    let mut gate = pool.begin().await.unwrap();
    let job: String = sqlx::query_scalar(
        "SELECT id FROM runs_jobs WHERE activity_id=$1 AND status='processing' FOR UPDATE",
    )
    .bind(activity.to_string())
    .fetch_one(&mut *gate)
    .await
    .unwrap();
    sqlx::query("LOCK TABLE runs_jobs IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *gate)
        .await
        .unwrap();
    sqlx::query("UPDATE runs_slots SET lease_until=clock_timestamp()-interval '1 second' WHERE stage='worker'")
        .execute(&pool).await.unwrap();
    // The worker-slot watch must kill/reap without waiting for the blocked job
    // heartbeat or finish transaction. Capacity stays reserved during cleanup.
    let stopped = timeout(Duration::from_secs(13), async {
        loop {
            if !workspace.exists() {
                return reaped(pid);
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    gate.rollback().await.unwrap();
    let completed = timeout(Duration::from_secs(5), &mut worker).await;
    if completed.is_err() {
        worker.abort();
        let _ = worker.await;
    }
    let mut status = 0;
    if unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) } == 0 {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
    drop(child);
    let visible: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM runs_manifests WHERE activity_id=$1 AND generation=2",
    )
    .bind(activity.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    let state: String = sqlx::query_scalar("SELECT status FROM runs_jobs WHERE id=$1")
        .bind(job)
        .fetch_one(&pool)
        .await
        .unwrap();
    close_pool(admin, pool, schema).await;
    assert_eq!(
        stopped.ok(),
        Some(true),
        "Slot loss did not promptly kill and reap the real child before private cleanup"
    );
    let error = completed.unwrap().unwrap().unwrap_err();
    assert_eq!(error.code(), "JOB_LEASE_LOST");
    assert_eq!(visible, 0, "Lost worker published a new manifest");
    assert_eq!(state, "queued");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_slot_loss_after_dependency_insert_rolls_back_publication() {
    let _serial = SERIAL.lock().await;
    let (admin, pool, schema, owner) = isolated_pool().await;
    let activity = Box::pin(import_real(&pool, owner)).await;
    Box::pin(jobs::run_once(&pool)).await.unwrap();
    let holder = Uuid::new_v4().to_string();
    let generation: i64 =
        sqlx::query_scalar("SELECT history_generation FROM runs_activities WHERE id=$1")
            .bind(activity.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE runs_slots SET lease_owner=$1,lease_until=clock_timestamp()+interval '90 seconds' WHERE stage='worker'")
        .bind(&holder).execute(&pool).await.unwrap();
    sqlx::query("UPDATE runs_jobs SET status='processing',lease_owner=$2,lease_until=clock_timestamp()+interval '90 seconds' WHERE activity_id=$1 AND stage='history' AND desired_generation=$3")
        .bind(activity.to_string()).bind(&holder).bind(generation).execute(&pool).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION runs_history_gate() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(hashtextextended(TG_TABLE_SCHEMA,2)); RETURN NEW; END $$; CREATE TRIGGER runs_history_gate AFTER INSERT ON runs_estimate_dependencies FOR EACH ROW EXECUTE FUNCTION runs_history_gate();")
        .execute(&pool).await.unwrap();
    let mut gate = pool.begin().await.unwrap();
    let gate_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *gate)
        .await
        .unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,2))")
        .bind(&schema)
        .execute(&mut *gate)
        .await
        .unwrap();
    let history_pool = pool.clone();
    let publication = tokio::spawn(async move {
        history::recompute(&history_pool, owner, activity, generation, &holder).await
    });
    timeout(Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND $2=ANY(pg_blocking_pids(pid)))")
                .bind(&schema).bind(gate_pid).fetch_one(&admin).await.unwrap();
            if blocked { break; }
            assert!(!publication.is_finished(), "History did not reach its real dependency write");
            sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("History never reached the dependency publication gate");
    sqlx::query("UPDATE runs_slots SET lease_owner='replacement',lease_until=clock_timestamp()+interval '90 seconds' WHERE stage='worker'")
        .execute(&pool).await.unwrap();
    gate.rollback().await.unwrap();
    let result = timeout(Duration::from_secs(5), publication)
        .await
        .unwrap()
        .unwrap();
    let visible: i64 =
        sqlx::query_scalar("SELECT count(*) FROM runs_estimates WHERE activity_id=$1")
            .bind(activity.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    let dependencies: i64 =
        sqlx::query_scalar("SELECT count(*) FROM runs_estimate_dependencies WHERE activity_id=$1")
            .bind(activity.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    close_pool(admin, pool, schema).await;
    assert_eq!(result.unwrap_err().code(), "JOB_LEASE_LOST");
    assert_eq!(
        visible, 0,
        "Historical estimate escaped its final lease fence"
    );
    assert_eq!(
        dependencies, 0,
        "Stale historical dependencies survived rollback"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn decoder_admission_and_client_drop_cancel_reap_before_releasing_capacity() {
    let _serial = SERIAL.lock().await;
    let (admin, pool, schema, _) = isolated_pool().await;
    let mut outcomes = Vec::new();
    for stage in ["decode", "import", "client"] {
        let child = BlockingChild::new();
        let mut admission = jobs::import_admission(&pool).await.unwrap();
        let admission_holder = admission.holder().to_owned();
        let decoder_capacity = admission.decoder_hold().unwrap();
        let (signal, mut cancellation) = tokio::sync::watch::channel(false);
        let decode_pool = pool.clone();
        let mut decoder = tokio::spawn(async move {
            jobs::decode_import(
                &decode_pool,
                FIT,
                async {
                    tokio::select! {
                        _ = admission.cancelled() => {},
                        _ = jobs::cancelled(&mut cancellation) => {},
                    }
                },
                Some(decoder_capacity),
            )
            .await
        });
        let (pid, workspace) = child.ready().await;
        let holder: Option<String> =
            sqlx::query_scalar("SELECT lease_owner FROM runs_slots WHERE stage='decode'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            holder.is_some(),
            "Real child starts without decoder capacity"
        );
        if stage == "client" {
            drop(signal);
        } else {
            sqlx::query("UPDATE runs_slots SET lease_until=clock_timestamp()-interval '1 second' WHERE stage=$1 AND lease_owner=$2")
                .bind(stage).bind(if stage == "decode" { holder.as_ref().unwrap() } else { &admission_holder })
                .execute(&pool).await.unwrap();
        }
        let stopped = timeout(Duration::from_secs(13), async {
            let mut early_release = false;
            loop {
                tokio::select! {
                    result = &mut decoder => {
                        let result = result.unwrap();
                        let was_reaped = reaped(pid);
                        return (result.err().map(|error| error.code().to_owned()), was_reaped, early_release);
                    },
                    _ = sleep(Duration::from_millis(5)) => {
                        let reserved: bool = sqlx::query_scalar("SELECT lease_owner IS NOT NULL FROM runs_slots WHERE stage='decode'")
                            .fetch_one(&pool).await.unwrap();
                        if (!reserved || !workspace.exists()) && unsafe { libc::kill(pid, 0) } == 0 {
                            early_release = true;
                        }
                    },
                }
            }
        }).await;
        if stopped.is_err() {
            decoder.abort();
            let _ = decoder.await;
        }
        let private_removed = !workspace.exists();
        let released: bool =
            sqlx::query_scalar("SELECT lease_owner IS NULL FROM runs_slots WHERE stage='decode'")
                .fetch_one(&pool)
                .await
                .unwrap();
        outcomes.push((stage, stopped, private_removed, released));
        drop(child);
        timeout(Duration::from_secs(5), async {
            loop {
                let released: bool = sqlx::query_scalar(
                    "SELECT lease_owner IS NULL FROM runs_slots WHERE stage='import'",
                )
                .fetch_one(&pool)
                .await
                .unwrap();
                if released {
                    break;
                }
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("Cancelled caller did not release its admission after draining");
    }
    close_pool(admin, pool, schema).await;
    for (stage, stopped, private_removed, released) in outcomes {
        let (code, was_reaped, early_release) =
            stopped.expect("Cancellation did not promptly complete");
        assert_eq!(
            code.as_deref(),
            Some("JOB_LEASE_LOST"),
            "{stage} cancellation"
        );
        assert!(
            was_reaped,
            "{stage} cancellation returned before reaping the real child"
        );
        assert!(
            !early_release,
            "{stage} cancellation released private storage or capacity before child exit/reaping"
        );
        assert!(
            private_removed,
            "{stage} cancellation retained its private workspace"
        );
        assert!(
            released,
            "{stage} cancellation retained decoder capacity after cleanup"
        );
    }
}
