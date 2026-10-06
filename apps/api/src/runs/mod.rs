pub mod analysis;
pub mod export;
pub mod history;
pub mod import;
pub mod jobs;
pub mod legacy;
pub mod privacy;
pub mod process;
pub mod store;
pub mod stream;
pub mod thresholds;

use crate::error::ApiError;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ReaderProcess {
    scope: String,
    pid: u32,
    incarnation: String,
}

pub(crate) fn reader_process() -> std::io::Result<&'static ReaderProcess> {
    static PROCESS: std::sync::LazyLock<std::io::Result<ReaderProcess>> =
        std::sync::LazyLock::new(|| {
            let pid = std::process::id();
            Ok(ReaderProcess {
                scope: reader_scope()?,
                pid,
                incarnation: process_incarnation(pid)?
                    .ok_or_else(|| std::io::Error::other("reader process disappeared"))?,
            })
        });
    PROCESS
        .as_ref()
        .map_err(|error| std::io::Error::other(error.to_string()))
}

/// Only native process-death proof can recover a crashed transport. Database
/// disconnection and heartbeat expiry cannot prove that retained Bytes are gone.
/// ponytail: Recovery is local to the same boot and PID namespace. Foreign
/// namespaces stay fenced without trusted supervisor proof of process death.
pub(crate) fn reader_is_dead(process: &ReaderProcess) -> bool {
    let Ok(local) = reader_process() else {
        return false;
    };
    if process.scope != local.scope {
        return false;
    }
    match process_incarnation(process.pid) {
        Ok(None) => true,
        Ok(Some(incarnation)) => incarnation != process.incarnation,
        Err(_) => false,
    }
}

#[cfg(target_os = "linux")]
fn reader_scope() -> std::io::Result<String> {
    use std::os::unix::fs::MetadataExt;
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    let namespace = std::fs::metadata("/proc/self/ns/pid")?;
    Ok(format!("{}:{}", boot.trim(), namespace.ino()))
}
#[cfg(target_os = "linux")]
fn process_incarnation(pid: u32) -> std::io::Result<Option<String>> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(std::io::Error::other("invalid reader PID"));
    }
    let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // hidepid can conceal a live process. Missing /proc alone is not
            // death proof; EPERM and an existing PID must remain fenced.
            if unsafe { libc::kill(pid as i32, 0) } == -1
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            {
                return Ok(None);
            }
            return Err(error);
        }
        Err(error) => return Err(error),
    };
    let fields = stat
        .rsplit_once(')')
        .ok_or_else(|| std::io::Error::other("invalid process identity"))?
        .1;
    Ok(Some(
        fields
            .split_whitespace()
            .nth(19)
            .ok_or_else(|| std::io::Error::other("missing process identity"))?
            .to_owned(),
    ))
}
#[cfg(target_os = "macos")]
fn reader_scope() -> std::io::Result<String> {
    let mut identity = [0u8; 16];
    let timeout = libc::timespec {
        tv_sec: 1,
        tv_nsec: 0,
    };
    if unsafe { libc::gethostuuid(identity.as_mut_ptr(), &timeout) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut boot = std::mem::MaybeUninit::<libc::timeval>::uninit();
    let mut size = std::mem::size_of::<libc::timeval>();
    if unsafe {
        libc::sysctlbyname(
            c"kern.boottime".as_ptr(),
            boot.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || size != std::mem::size_of::<libc::timeval>()
    {
        return Err(std::io::Error::last_os_error());
    }
    let boot = unsafe { boot.assume_init() };
    Ok(format!(
        "{}:{}:{}",
        Uuid::from_bytes(identity),
        boot.tv_sec,
        boot.tv_usec
    ))
}
#[cfg(target_os = "macos")]
fn process_incarnation(pid: u32) -> std::io::Result<Option<String>> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(std::io::Error::other("invalid reader PID"));
    }
    // Native proc_bsdinfo from sys/proc_info.h, PROC_PIDTBSDINFO = 3.
    #[repr(C)]
    struct ProcessInfo {
        identity: [u32; 12],
        names: [u8; 48],
        details: [u32; 6],
        seconds: u64,
        microseconds: u64,
    }
    let mut info = std::mem::MaybeUninit::<ProcessInfo>::uninit();
    let size = std::mem::size_of::<ProcessInfo>() as i32;
    let count = unsafe { libc::proc_pidinfo(pid as i32, 3, 0, info.as_mut_ptr().cast(), size) };
    if count != size {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH)
            && unsafe { libc::kill(pid as i32, 0) } == -1
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        {
            return Ok(None);
        }
        return Err(error);
    }
    let info = unsafe { info.assume_init() };
    Ok(Some(format!("{}:{}", info.seconds, info.microseconds)))
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn reader_scope() -> std::io::Result<String> {
    Err(std::io::Error::other("native reader identity unsupported"))
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn process_incarnation(_: u32) -> std::io::Result<Option<String>> {
    Err(std::io::Error::other("native reader identity unsupported"))
}

fn report_counts(items: &[Value]) -> Value {
    let mut counts = json!({"imported":0,"duplicate":0,"unsupported":0,"failed":0});
    for item in items {
        if let Some(status) = item["status"]
            .as_str()
            .filter(|status| matches!(*status, "imported" | "duplicate" | "unsupported" | "failed"))
        {
            counts[status] = json!(counts[status].as_u64().unwrap_or(0) + 1);
        }
    }
    counts
}

pub async fn persist_import_report(
    pool: &PgPool,
    owner: Uuid,
    report: &mut Value,
) -> Result<(), ApiError> {
    let owner = owner.to_string();
    let mut transaction = pool.begin().await.map_err(|_| ApiError::database_error())?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(&owner)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApiError::database_error())?;
    let items = report["items"]
        .as_array_mut()
        .ok_or_else(ApiError::processing_error)?;
    for item in items.iter_mut() {
        if let Some(activity) = item["activityId"].as_str() {
            let live: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM runs_activities WHERE owner_id=$1 AND id=$2)",
            )
            .bind(&owner)
            .bind(activity)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| ApiError::database_error())?;
            if !live {
                *item = json!({"index":item["index"],"name":"deleted-input","status":"failed","reason":"IMPORT_ACTIVITY_DELETED","activityId":null,"warnings":[]});
            }
        }
    }
    report["counts"] = report_counts(items);
    sqlx::query("INSERT INTO runs_import_reports(id,owner_id,payload) VALUES($1,$2,$3::jsonb)")
        .bind(report["batchId"].as_str())
        .bind(&owner)
        .bind(report.to_string())
        .execute(&mut *transaction)
        .await
        .map_err(|_| ApiError::database_error())?;
    transaction
        .commit()
        .await
        .map_err(|_| ApiError::database_error())?;
    Ok(())
}

/// Erase deleted activity provenance without dropping valid sibling import receipts.
pub async fn delete_import_receipts(
    transaction: &mut Transaction<'_, Postgres>,
    owner: &str,
    activity: &str,
) -> Result<(), sqlx::Error> {
    let rows=sqlx::query("SELECT id,payload::text FROM runs_import_reports WHERE owner_id=$1 AND payload @> $2::jsonb FOR UPDATE")
        .bind(owner).bind(json!({"items":[{"activityId":activity}]}).to_string()).fetch_all(&mut **transaction).await?;
    for row in rows {
        let id: String = row.try_get("id")?;
        let text: String = row.try_get("payload")?;
        let mut report: Value = serde_json::from_str(&text)
            .map_err(|_| sqlx::Error::Protocol("Invalid stored import report".into()))?;
        let items = report["items"]
            .as_array_mut()
            .ok_or_else(|| sqlx::Error::Protocol("Invalid stored import items".into()))?;
        items.retain(|item| item["activityId"].as_str() != Some(activity));
        if items.is_empty() {
            sqlx::query("DELETE FROM runs_import_reports WHERE owner_id=$1 AND id=$2")
                .bind(owner)
                .bind(&id)
                .execute(&mut **transaction)
                .await?;
        } else {
            report["counts"] = report_counts(items);
            sqlx::query(
                "UPDATE runs_import_reports SET payload=$3::jsonb WHERE owner_id=$1 AND id=$2",
            )
            .bind(owner)
            .bind(&id)
            .bind(report.to_string())
            .execute(&mut **transaction)
            .await?;
        }
    }
    Ok(())
}
