use super::{process, store};
use crate::error::ApiError;
use axum::http::StatusCode;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::time::{Duration, Instant, interval, sleep};
use uuid::Uuid;

fn busy() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "DECODER_BUSY",
        "The decoder is busy. Retry the import later.",
    )
}
fn lost() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "JOB_LEASE_LOST",
        "Processing lease expired or was cancelled.",
    )
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PhysicalIdentity {
    scope: Uuid,
    stage: String,
    runtime: String,
    nonce: Uuid,
    directory_dev: u64,
    directory_ino: u64,
    dev: u64,
    ino: u64,
}

pub(crate) struct PhysicalSlot {
    file: File,
    identity: PhysicalIdentity,
}

/// Keeps the caller's physical capacity inherited by its decoder, not unrelated jobs.
pub struct DecoderCapacity {
    pub(crate) physical: Arc<PhysicalSlot>,
}

impl DecoderCapacity {
    pub(crate) fn descriptor(&self) -> std::os::fd::RawFd {
        self.physical.file.as_raw_fd()
    }
}

impl PhysicalSlot {
    fn runtime() -> Result<String, ApiError> {
        #[cfg(target_os = "linux")]
        {
            let boot =
                std::fs::read_to_string("/proc/sys/kernel/random/boot_id").map_err(|_| lost())?;
            Uuid::parse_str(boot.trim()).map_err(|_| lost())?;
            // Containers on one kernel need distinct physical filesystem authority.
            let namespace = std::fs::metadata("/proc/self/ns/mnt").map_err(|_| lost())?;
            Ok(format!(
                "{}:{}:{}",
                boot.trim(),
                namespace.dev(),
                namespace.ino()
            ))
        }
        #[cfg(target_os = "macos")]
        {
            let mut bytes = [0u8; 128];
            let mut length = bytes.len();
            if unsafe {
                libc::sysctlbyname(
                    c"kern.bootsessionuuid".as_ptr(),
                    bytes.as_mut_ptr().cast(),
                    &mut length,
                    std::ptr::null_mut(),
                    0,
                )
            } != 0
            {
                return Err(lost());
            }
            let runtime = std::str::from_utf8(&bytes[..length.min(bytes.len())])
                .map_err(|_| lost())?
                .trim_end_matches('\0');
            Uuid::parse_str(runtime).map_err(|_| lost())?;
            Ok(runtime.to_owned())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Err(lost())
        }
    }

    fn directory(create: bool) -> Result<File, ApiError> {
        use std::os::unix::fs::DirBuilderExt;
        let path =
            std::env::temp_dir().join(format!("runs-capacity-{}", unsafe { libc::geteuid() }));
        if create {
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            if let Err(error) = builder.create(&path)
                && error.kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(lost());
            }
        }
        let directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| lost())?;
        process::verify_metadata(&directory.metadata().map_err(|_| lost())?, true)?;
        let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::uninit();
        if unsafe { libc::fstatfs(directory.as_raw_fd(), filesystem.as_mut_ptr()) } != 0 {
            return Err(lost());
        }
        let filesystem = unsafe { filesystem.assume_init() };
        #[cfg(target_os = "linux")]
        let local = matches!(
            filesystem.f_type as u64,
            0xEF53 | 0x01021994 | 0x58465342 | 0x9123683E | 0x794C7630
        );
        #[cfg(target_os = "macos")]
        let local = filesystem.f_flags & libc::MNT_LOCAL as u32 != 0;
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let local = false;
        if !local {
            return Err(lost());
        }
        Ok(directory)
    }

    fn marker(
        directory: &File,
        scope: Uuid,
        stage: &str,
        create: bool,
    ) -> Result<(File, bool), ApiError> {
        if !["worker", "import", "decode"].contains(&stage) {
            return Err(lost());
        }
        let name = std::ffi::CString::new(format!("{scope}-{stage}")).map_err(|_| lost())?;
        let flags = libc::O_RDWR | libc::O_CLOEXEC | libc::O_NOFOLLOW;
        let mut created = false;
        let mut descriptor = -1;
        if create {
            descriptor = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    flags | libc::O_CREAT | libc::O_EXCL,
                    0o600,
                )
            };
            if descriptor >= 0 {
                created = true;
            } else if std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists {
                return Err(lost());
            }
        }
        if descriptor < 0 {
            descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        }
        if descriptor < 0 {
            return Err(lost());
        }
        let file = unsafe { File::from_raw_fd(descriptor) };
        let metadata = file.metadata().map_err(|_| lost())?;
        process::verify_metadata(&metadata, false)?;
        if metadata.nlink() != 1 {
            return Err(lost());
        }
        Ok((file, created))
    }

    fn lock(file: &File) -> Result<bool, ApiError> {
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(true);
        }
        if std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock {
            return Ok(false);
        }
        Err(lost())
    }

    fn nonce(file: &mut File) -> Result<Uuid, ApiError> {
        let mut bytes = [0u8; 36];
        file.read_exact(&mut bytes).map_err(|_| lost())?;
        let mut extra = [0u8; 1];
        if file.read(&mut extra).map_err(|_| lost())? != 0 {
            return Err(lost());
        }
        Uuid::parse_str(std::str::from_utf8(&bytes).map_err(|_| lost())?).map_err(|_| lost())
    }

    pub(crate) fn local(scope: Uuid, stage: &str) -> Result<Option<Arc<Self>>, ApiError> {
        let directory = Self::directory(true)?;
        let (mut file, created) = Self::marker(&directory, scope, stage, true)?;
        if !Self::lock(&file)? {
            return Ok(None);
        }
        // A crash before publishing the identity can leave an incomplete nonce.
        // This path is used only for an unowned DB row, with its local lock held.
        let nonce = if created || file.metadata().map_err(|_| lost())?.len() < 36 {
            file.set_len(0).map_err(|_| lost())?;
            let nonce = Uuid::new_v4();
            file.write_all(nonce.to_string().as_bytes())
                .map_err(|_| lost())?;
            file.sync_all().map_err(|_| lost())?;
            nonce
        } else {
            Self::nonce(&mut file)?
        };
        let directory_metadata = directory.metadata().map_err(|_| lost())?;
        let metadata = file.metadata().map_err(|_| lost())?;
        Ok(Some(Arc::new(Self {
            file,
            identity: PhysicalIdentity {
                scope,
                stage: stage.to_owned(),
                runtime: Self::runtime()?,
                nonce,
                directory_dev: directory_metadata.dev(),
                directory_ino: directory_metadata.ino(),
                dev: metadata.dev(),
                ino: metadata.ino(),
            },
        })))
    }

    fn reopen(identity: &PhysicalIdentity) -> Result<Option<Arc<Self>>, ApiError> {
        if identity.runtime != Self::runtime()? {
            return Err(lost());
        }
        let directory = Self::directory(false)?;
        let metadata = directory.metadata().map_err(|_| lost())?;
        if (metadata.dev(), metadata.ino()) != (identity.directory_dev, identity.directory_ino) {
            return Err(lost());
        }
        let (mut file, _) = Self::marker(&directory, identity.scope, &identity.stage, false)?;
        let metadata = file.metadata().map_err(|_| lost())?;
        if (metadata.dev(), metadata.ino()) != (identity.dev, identity.ino)
            || Self::nonce(&mut file)? != identity.nonce
        {
            return Err(lost());
        }
        if !Self::lock(&file)? {
            return Ok(None);
        }
        Ok(Some(Arc::new(Self {
            file,
            identity: identity.clone(),
        })))
    }
}

async fn acquire_slot(
    pool: &PgPool,
    stage: &str,
    holder: &str,
) -> Result<Option<Arc<PhysicalSlot>>, ApiError> {
    let mut tx = pool.begin().await.map_err(store::database_error)?;
    let row = sqlx::query("SELECT lease_owner,physical_owner,physical_scope FROM runs_slots WHERE stage=$1 AND slot=1 AND (lease_owner IS NULL OR lease_until<clock_timestamp()) FOR UPDATE SKIP LOCKED")
        .bind(stage).fetch_optional(&mut *tx).await.map_err(store::database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let scope: Uuid = row
        .try_get("physical_scope")
        .map_err(store::database_error)?;
    let physical = if let Some(value) = row
        .try_get::<Option<Value>, _>("physical_owner")
        .map_err(store::database_error)?
    {
        let identity: PhysicalIdentity = serde_json::from_value(value).map_err(|_| lost())?;
        if identity.scope != scope || identity.stage != stage {
            return Err(lost());
        }
        match PhysicalSlot::reopen(&identity) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        }
    } else {
        if row
            .try_get::<Option<String>, _>("lease_owner")
            .map_err(store::database_error)?
            .is_some()
        {
            return Ok(None);
        }
        PhysicalSlot::local(scope, stage)?
    };
    let Some(physical) = physical else {
        return Ok(None);
    };
    sqlx::query("UPDATE runs_slots SET lease_owner=$2,lease_until=clock_timestamp()+interval '90 seconds',physical_owner=$3 WHERE stage=$1 AND slot=1")
        .bind(stage).bind(holder).bind(serde_json::to_value(&physical.identity).map_err(|_| lost())?)
        .execute(&mut *tx).await.map_err(store::database_error)?;
    tx.commit().await.map_err(store::database_error)?;
    Ok(Some(physical))
}

async fn release_slot(
    pool: &PgPool,
    stage: &str,
    holder: &str,
    identity: &PhysicalIdentity,
) -> Result<(), ApiError> {
    let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs_slots WHERE stage=$1 AND lease_owner=$2 AND physical_owner=$3)")
        .bind(stage).bind(holder).bind(serde_json::to_value(identity).map_err(|_| lost())?)
        .fetch_one(pool).await.map_err(store::database_error)?;
    if !owned {
        return Ok(());
    }
    // Closing a parent descriptor is not completion: its child can still own
    // the same open-file description. Never use LOCK_UN or unlink this marker.
    let completed = loop {
        if let Some(completed) = PhysicalSlot::reopen(identity)? {
            break completed;
        }
        sleep(Duration::from_millis(10)).await;
    };
    sqlx::query("UPDATE runs_slots SET lease_owner=NULL,lease_until=NULL,physical_owner=NULL WHERE stage=$1 AND lease_owner=$2 AND physical_owner=$3")
        .bind(stage).bind(holder).bind(serde_json::to_value(&completed.identity).map_err(|_| lost())?)
        .execute(pool).await.map_err(store::database_error)?;
    Ok(())
}
async fn heartbeat_slot(pool: &PgPool, stage: &str, holder: &str) -> Result<(), ApiError> {
    let updated=sqlx::query("UPDATE runs_slots SET lease_until=now()+interval '90 seconds' WHERE stage=$1 AND lease_owner=$2 AND lease_until>now()")
        .bind(stage).bind(holder).execute(pool).await.map_err(store::database_error)?.rows_affected();
    if updated != 1 {
        return Err(lost());
    }
    Ok(())
}

pub(crate) struct SlotLease {
    pool: PgPool,
    holder: String,
    stage: &'static str,
    released: AtomicBool,
    heartbeat: tokio::task::JoinHandle<()>,
    physical: Option<Arc<PhysicalSlot>>,
    identity: PhysicalIdentity,
}
pub struct LeaseGuard {
    lease: std::sync::Arc<SlotLease>,
    cancelled: tokio::sync::watch::Receiver<bool>,
}
impl LeaseGuard {
    pub async fn cancelled(&mut self) {
        cancelled(&mut self.cancelled).await;
    }
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow() || self.cancelled.has_changed().is_err()
    }
    pub fn holder(&self) -> &str {
        &self.lease.holder
    }
    pub fn decoder_hold(&self) -> Result<DecoderCapacity, ApiError> {
        Ok(DecoderCapacity {
            physical: self.lease.physical.as_ref().ok_or_else(lost)?.clone(),
        })
    }
    pub(crate) fn cpu_hold(&self) -> std::sync::Arc<SlotLease> {
        self.lease.clone()
    }
    async fn release(&mut self) -> Result<(), ApiError> {
        if std::sync::Arc::strong_count(&self.lease) != 1 {
            return Err(lost());
        }
        self.lease.heartbeat.abort();
        drop(
            Arc::get_mut(&mut self.lease)
                .ok_or_else(lost)?
                .physical
                .take(),
        );
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            release_slot(
                &self.lease.pool,
                self.lease.stage,
                &self.lease.holder,
                &self.lease.identity,
            ),
        )
        .await
        .map_err(|_| lost())?;
        if result.is_ok() {
            self.lease.released.store(true, Ordering::Release);
        }
        result
    }
}
impl Drop for SlotLease {
    fn drop(&mut self) {
        self.heartbeat.abort();
        drop(self.physical.take());
        if self.released.load(Ordering::Acquire) {
            return;
        }
        let pool = self.pool.clone();
        let holder = self.holder.clone();
        let stage = self.stage;
        let identity = self.identity.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = tokio::time::timeout(
                    Duration::from_secs(5),
                    release_slot(&pool, stage, &holder, &identity),
                )
                .await;
            });
        }
    }
}
pub async fn import_admission(pool: &PgPool) -> Result<LeaseGuard, ApiError> {
    let holder = Uuid::new_v4().to_string();
    let Some(physical) = acquire_slot(pool, "import", &holder).await? else {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "IMPORT_BUSY",
            "Another import is being staged. Retry later.",
        ));
    };
    Ok(lease_guard(pool, "import", holder, physical))
}
fn lease_guard(
    pool: &PgPool,
    stage: &'static str,
    holder: String,
    physical: Arc<PhysicalSlot>,
) -> LeaseGuard {
    let (signal, cancelled) = tokio::sync::watch::channel(false);
    let heartbeat_pool = pool.clone();
    let heartbeat_holder = holder.clone();
    let heartbeat = tokio::spawn(async move {
        let mut timer = interval(Duration::from_secs(10));
        timer.tick().await;
        loop {
            timer.tick().await;
            if heartbeat_slot(&heartbeat_pool, stage, &heartbeat_holder)
                .await
                .is_err()
            {
                let _ = signal.send(true);
            }
        }
    });
    LeaseGuard {
        lease: std::sync::Arc::new(SlotLease {
            pool: pool.clone(),
            holder,
            stage,
            released: AtomicBool::new(false),
            heartbeat,
            identity: physical.identity.clone(),
            physical: Some(physical),
        }),
        cancelled,
    }
}
/// A closed sender means the caller was dropped; cleanup still belongs to the callee.
pub async fn cancelled(signal: &mut tokio::sync::watch::Receiver<bool>) {
    while !*signal.borrow() {
        if signal.changed().await.is_err() {
            return;
        }
    }
}
/// Import, restore, and reprocess share the same PostgreSQL decoder lease.
pub async fn decode_import(
    pool: &PgPool,
    bytes: &[u8],
    cancellation: impl Future<Output = ()>,
    parent: Option<DecoderCapacity>,
) -> Result<process::DecodedSpool, ApiError> {
    if bytes.is_empty() || bytes.len() > process::MAX_INPUT {
        return Err(ApiError::file_too_large());
    }
    tokio::pin!(cancellation);
    let holder = Uuid::new_v4().to_string();
    let deadline = Instant::now() + Duration::from_secs(65);
    let physical = loop {
        tokio::select! {biased;_= &mut cancellation=>return Err(lost()),_=std::future::ready(())=>{}}
        // Drain admission SQL: dropping an in-flight claim can hide capacity it
        // acquired on the server. Once owned, cancellation releases it normally.
        if let Some(physical) = acquire_slot(pool, "decode", &holder).await? {
            break physical;
        }
        if Instant::now() >= deadline {
            return Err(busy());
        }
        tokio::select! {biased;_= &mut cancellation=>return Err(lost()),_=sleep(Duration::from_millis(100))=>{}}
    };
    let mut guard = lease_guard(pool, "decode", holder, physical);
    let capacity = guard.decoder_hold()?;
    if parent
        .as_ref()
        .is_some_and(|parent| parent.physical.identity.scope != capacity.physical.identity.scope)
    {
        return Err(lost());
    }
    let result = process::decode(
        bytes,
        async {
            tokio::select! {biased;_=guard.cancelled()=>{},_= &mut cancellation=>{}}
        },
        capacity,
        parent,
    )
    .await;
    let release = guard.release().await;
    match result {
        Ok(value) => {
            release?;
            Ok(value)
        }
        Err(error) => Err(error),
    }
}

struct Job {
    id: String,
    owner: String,
    activity: String,
    stage: String,
    generation: i64,
    holder: String,
    attempts: i32,
    analysis_running: AtomicBool,
    decoder_running: AtomicBool,
    cancelled: AtomicBool,
}
pub fn start_worker(pool: PgPool) {
    tokio::spawn(async move {
        let _ = tokio::task::spawn_blocking(process::sweep_spools).await;
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            super::export::recover_readers(&pool, None),
        )
        .await;
        let mut next_sweep = Instant::now() + Duration::from_secs(3600);
        let mut next_scan = Instant::now();
        loop {
            if Instant::now() >= next_sweep {
                let _ = tokio::task::spawn_blocking(process::sweep_spools).await;
                next_sweep = Instant::now() + Duration::from_secs(3600);
            }
            if Instant::now() >= next_scan {
                if let Err(error) = scan_versions(&pool).await {
                    tracing::warn!(code = error.code(), "Runs version scan failed");
                }
                next_scan = Instant::now() + Duration::from_secs(60);
            }
            match run_once(&pool).await {
                Ok(true) => {}
                Ok(false) => sleep(Duration::from_secs(1)).await,
                Err(error) => {
                    tracing::warn!(code = error.code(), "Runs job failed");
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });
}
async fn recover(pool: &PgPool, lease: &SlotLease) -> Result<(), ApiError> {
    if lease.stage != "worker" || lease.physical.is_none() {
        return Err(lost());
    }
    let mut tx = pool.begin().await.map_err(store::database_error)?;
    sqlx::query("UPDATE runs_jobs SET status=CASE WHEN attempts<3 THEN 'queued' ELSE 'failed' END,error_code='WORKER_LEASE_EXPIRED',lease_owner=NULL,lease_until=NULL,next_attempt_at=now()+make_interval(secs=>least(30,attempts*attempts)) WHERE status='processing' AND lease_until<now()")
        .execute(&mut *tx).await.map_err(store::database_error)?;
    sqlx::query("UPDATE runs_activities a SET processing_status='failed',error_code='WORKER_LEASE_EXPIRED' WHERE EXISTS(SELECT 1 FROM runs_jobs j WHERE j.activity_id=a.id AND j.desired_generation=a.desired_generation AND j.stage<>'history' AND j.status='failed' AND j.error_code='WORKER_LEASE_EXPIRED')")
        .execute(&mut *tx).await.map_err(store::database_error)?;
    tx.commit().await.map_err(store::database_error)
}
async fn claim(pool: &PgPool, holder: &str) -> Result<Option<Job>, ApiError> {
    let mut tx = pool.begin().await.map_err(store::database_error)?;
    let row=sqlx::query("SELECT j.id,j.owner_id,j.activity_id,j.stage,j.desired_generation,j.attempts FROM runs_jobs j JOIN runs_activities a ON a.id=j.activity_id AND (CASE WHEN j.stage='history' THEN a.history_generation ELSE a.desired_generation END)=j.desired_generation WHERE j.status='queued' AND j.next_attempt_at<=now() AND j.attempts<3 AND NOT EXISTS(SELECT 1 FROM runs_tombstones t WHERE t.activity_id=j.activity_id) ORDER BY j.created_at,j.id FOR UPDATE OF j SKIP LOCKED LIMIT 1")
        .fetch_optional(&mut *tx).await.map_err(store::database_error)?;
    let Some(row) = row else { return Ok(None) };
    let job = Job {
        id: row.get("id"),
        owner: row.get("owner_id"),
        activity: row.get("activity_id"),
        stage: row.get("stage"),
        generation: row.get("desired_generation"),
        holder: holder.to_owned(),
        attempts: row.get::<i32, _>("attempts") + 1,
        analysis_running: AtomicBool::new(false),
        decoder_running: AtomicBool::new(false),
        cancelled: AtomicBool::new(false),
    };
    sqlx::query("UPDATE runs_jobs SET status='processing',attempts=attempts+1,lease_owner=$2,lease_until=now()+interval '90 seconds',heartbeat_at=now(),error_code=NULL WHERE id=$1")
        .bind(&job.id).bind(holder).execute(&mut *tx).await.map_err(store::database_error)?;
    if job.stage != "history" {
        sqlx::query("UPDATE runs_activities SET processing_status='processing' WHERE id=$1 AND desired_generation=$2").bind(&job.activity).bind(job.generation).execute(&mut *tx).await.map_err(store::database_error)?;
    }
    tx.commit().await.map_err(store::database_error)?;
    Ok(Some(job))
}
async fn heartbeat_job(pool: &PgPool, job: &Job) -> Result<(), ApiError> {
    let updated=sqlx::query("UPDATE runs_jobs j SET lease_until=now()+interval '90 seconds',heartbeat_at=now() WHERE j.id=$1 AND j.lease_owner=$2 AND j.status='processing' AND j.lease_until>clock_timestamp() AND EXISTS(SELECT 1 FROM runs_slots WHERE stage='worker' AND lease_owner=$2 AND lease_until>clock_timestamp()) AND EXISTS(SELECT 1 FROM runs_activities a WHERE a.id=j.activity_id AND (CASE WHEN j.stage='history' THEN a.history_generation ELSE a.desired_generation END)=j.desired_generation) AND NOT EXISTS(SELECT 1 FROM runs_tombstones t WHERE t.activity_id=j.activity_id)")
        .bind(&job.id).bind(&job.holder).execute(pool).await.map_err(store::database_error)?.rows_affected();
    if updated != 1 {
        return Err(lost());
    }
    Ok(())
}
pub async fn run_once(pool: &PgPool) -> Result<bool, ApiError> {
    let holder = Uuid::new_v4().to_string();
    let Some(physical) = acquire_slot(pool, "worker", &holder).await? else {
        return Ok(false);
    };
    let mut guard = lease_guard(pool, "worker", holder.clone(), physical);
    // Owning the physical worker marker proves the previous worker has drained.
    recover(pool, &guard.lease).await?;
    let claim = claim(pool, &holder).await;
    let job = match claim {
        Ok(Some(job)) => job,
        Ok(None) => {
            guard.release().await?;
            return Ok(false);
        }
        Err(error) => {
            let _ = guard.release().await;
            return Err(error);
        }
    };
    let result = {
        let (signal, mut cancellation) = tokio::sync::watch::channel(false);
        let lease = guard.cpu_hold();
        let future = execute(pool, &job, &lease, &mut cancellation);
        tokio::pin!(future);
        // Keep execution polled while its publication row lock delays a heartbeat.
        let heartbeat = async {
            let mut timer = interval(Duration::from_secs(10));
            timer.tick().await;
            loop {
                timer.tick().await;
                if let Err(error) = heartbeat_job(pool, &job).await {
                    break error;
                }
            }
        };
        tokio::pin!(heartbeat);
        let execution = tokio::select! {biased;
            _=guard.cancelled()=>Err(lost()),
            result=&mut future=>Ok(result),
            error=&mut heartbeat=>Err(error),
        };
        match execution {
            Ok(result) => result,
            Err(error) => {
                job.cancelled.store(true, Ordering::Release);
                let _ = signal.send(true);
                // Drive the owned child through kill/wait. Non-abortable CPU
                // work also drains before its original capacity can be released.
                if job.decoder_running.load(Ordering::Acquire)
                    || job.analysis_running.load(Ordering::Acquire)
                {
                    let _ = (&mut future).await;
                }
                Err(error)
            }
        }
    };
    let finish = finish(pool, &job, result.as_ref().err()).await;
    let release = guard.release().await;
    finish?;
    release?;
    match result {
        Ok(()) => Ok(true),
        Err(error) => Err(error),
    }
}
async fn finish(pool: &PgPool, job: &Job, error: Option<&ApiError>) -> Result<(), ApiError> {
    let transient = error.is_some_and(|e| {
        matches!(
            e.code(),
            "DECODER_UNAVAILABLE" | "DECODER_BUSY" | "RUNS_STORAGE_FAILED" | "JOB_LEASE_LOST"
        )
    });
    let state = if error.is_none() {
        "ready"
    } else if transient && job.attempts < 3 {
        "queued"
    } else {
        "failed"
    };
    let mut tx = pool.begin().await.map_err(store::database_error)?;
    let changed=sqlx::query("UPDATE runs_jobs SET status=$3,error_code=$4,lease_owner=NULL,lease_until=NULL,next_attempt_at=now()+make_interval(secs=>$5) WHERE id=$1 AND lease_owner=$2 AND status='processing'")
        .bind(&job.id).bind(&job.holder).bind(state).bind(error.map(ApiError::code)).bind(f64::from(job.attempts*job.attempts)).execute(&mut *tx).await.map_err(store::database_error)?.rows_affected();
    if changed == 1 && job.stage != "history" && error.is_some() {
        sqlx::query("UPDATE runs_activities SET processing_status=$3,error_code=$4 WHERE id=$1 AND desired_generation=$2")
        .bind(&job.activity).bind(job.generation).bind(state).bind(error.map(ApiError::code)).execute(&mut *tx).await.map_err(store::database_error)?;
    }
    tx.commit().await.map_err(store::database_error)
}
async fn execute(
    pool: &PgPool,
    job: &Job,
    lease: &std::sync::Arc<SlotLease>,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<(), ApiError> {
    if job.stage == "history" {
        let owner = Uuid::parse_str(&job.owner).map_err(|_| lost())?;
        let activity = Uuid::parse_str(&job.activity).map_err(|_| lost())?;
        return crate::runs::history::recompute(pool, owner, activity, job.generation, &job.holder)
            .await;
    }
    let staged=sqlx::query("SELECT d.id AS decoded_id,n.id AS normalized_id,n.payload::text AS normalized,n.legacy_projection::text AS legacy FROM runs_revisions n JOIN runs_revisions d ON d.id=n.input_revision_ids->>0 WHERE n.activity_id=$1 AND n.stage='normalized' AND n.generation=$2")
        .bind(&job.activity).bind(job.generation).fetch_optional(pool).await.map_err(store::database_error)?;
    let (decoded_id, normalized_id, metadata, legacy) = if let Some(row) = staged {
        let metadata = store::parse(
            row.try_get::<&str, _>("normalized")
                .map_err(store::database_error)?,
        )?;
        let legacy: crate::model::Analysis = serde_json::from_str(
            row.try_get::<&str, _>("legacy")
                .map_err(store::database_error)?,
        )
        .map_err(|_| store::invalid())?;
        (
            row.try_get::<String, _>("decoded_id")
                .map_err(store::database_error)?,
            row.try_get::<String, _>("normalized_id")
                .map_err(store::database_error)?,
            metadata,
            legacy,
        )
    } else if job.stage == "analysis" {
        let row=sqlx::query("SELECT m.decoded_revision_id,m.normalized_revision_id,n.payload::text AS normalized,n.legacy_projection::text AS legacy FROM runs_activities a JOIN runs_manifests m ON m.id=a.current_manifest_id JOIN runs_revisions n ON n.id=m.normalized_revision_id WHERE a.id=$1 AND a.desired_generation=$2")
            .bind(&job.activity).bind(job.generation).fetch_optional(pool).await.map_err(store::database_error)?.ok_or_else(lost)?;
        let metadata = store::parse(
            row.try_get::<&str, _>("normalized")
                .map_err(store::database_error)?,
        )?;
        let legacy: crate::model::Analysis = serde_json::from_str(
            row.try_get::<&str, _>("legacy")
                .map_err(store::database_error)?,
        )
        .map_err(|_| store::invalid())?;
        (
            row.try_get("decoded_revision_id")
                .map_err(store::database_error)?,
            row.try_get("normalized_revision_id")
                .map_err(store::database_error)?,
            metadata,
            legacy,
        )
    } else {
        let row=sqlx::query("SELECT s.bytes,s.sha256,s.size_bytes FROM runs_sources s JOIN runs_activities a ON a.source_id=s.id AND a.owner_id=s.owner_id WHERE a.id=$1 AND a.desired_generation=$2")
            .bind(&job.activity).bind(job.generation).fetch_optional(pool).await.map_err(store::database_error)?.ok_or_else(lost)?;
        let bytes: &[u8] = row.try_get("bytes").map_err(store::database_error)?;
        let hash: &[u8] = row.try_get("sha256").map_err(store::database_error)?;
        let actual = Sha256::digest(bytes);
        if row
            .try_get::<i64, _>("size_bytes")
            .map_err(store::database_error)?
            != bytes.len() as i64
            || actual[..] != *hash
        {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "SOURCE_INTEGRITY_FAILED",
                "Stored source integrity verification failed.",
            ));
        }
        let source_hash = format!("{actual:x}");
        job.decoder_running.store(true, Ordering::Release);
        let parent = DecoderCapacity {
            physical: lease.physical.as_ref().ok_or_else(lost)?.clone(),
        };
        let decoded = decode_import(pool, bytes, cancelled(cancellation), Some(parent)).await;
        job.decoder_running.store(false, Ordering::Release);
        let mut spool = decoded?;
        spool.hold_cpu(lease.clone());
        job.analysis_running.store(true, Ordering::Release);
        let (_spool, staged) = tokio::task::spawn_blocking(move || {
            let staged = super::stream::split_run(&spool.run, &spool.directory)?;
            Ok::<_, ApiError>((spool, staged))
        })
        .await
        .map_err(|_| store::invalid())??;
        job.analysis_running.store(false, Ordering::Release);
        if job.cancelled.load(Ordering::Acquire) {
            return Err(lost());
        }
        let mut tx = pool.begin().await.map_err(store::database_error)?;
        store::owner_lock(&mut tx, &job.owner)
            .await
            .map_err(store::database_error)?;
        fence(&mut tx, job).await?;
        let (decoded, normalized) = store::stage_documents(
            &mut tx,
            &job.owner,
            &job.activity,
            job.generation,
            &staged,
            &source_hash,
        )
        .await?;
        let mut metadata = staged.normalized_metadata;
        metadata["sourceHash"] = json!(source_hash);
        metadata["documents"] = json!({"archive":staged.normalized.metadata,"analysisInput":staged.analysis_input.metadata});
        metadata["projectionVersion"] = json!(crate::runs::thresholds::PROJECTION_VERSION);
        fence(&mut tx, job).await?;
        tx.commit().await.map_err(store::database_error)?;
        (decoded, normalized, metadata, staged.legacy)
    };
    let normalized_hash = metadata["documents"]["archive"]["sha256"]
        .as_str()
        .ok_or_else(store::invalid)?
        .to_owned();
    if metadata["projectionVersion"] != crate::runs::thresholds::PROJECTION_VERSION {
        return Err(store::invalid());
    }
    let descriptor = metadata["documents"]["analysisInput"].clone();
    let owner = job.owner.clone();
    let revision = normalized_id.clone();
    let read_pool = pool.clone();
    let cpu_hold = lease.clone();
    let source_hash = metadata["sourceHash"]
        .as_str()
        .ok_or_else(store::invalid)?
        .to_owned();
    job.analysis_running.store(true, Ordering::Release);
    let (analysis, input_hash) = tokio::task::spawn_blocking(move || {
        let _cpu_hold = cpu_hold;
        let reader = super::stream::RevisionReader::new(
            read_pool,
            owner,
            revision.clone(),
            "analysisInput",
            &descriptor,
        )?;
        let mut input: Value = serde_json::from_reader(reader).map_err(|error| {
            if error.io_error_kind() == Some(std::io::ErrorKind::ConnectionAborted) {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "RUNS_STORAGE_FAILED",
                    "Run storage operation failed.",
                )
            } else {
                store::invalid()
            }
        })?;
        input["sourceRevision"] = json!(revision);
        input["sourceHash"] = json!(source_hash);
        input["normalizedDocumentHash"] = json!(normalized_hash);
        input["projectionVersion"] = json!(crate::runs::thresholds::PROJECTION_VERSION);
        let mut hash = Sha256::new();
        serde_json::to_writer(&mut hash, &input).map_err(|_| store::invalid())?;
        let digest = format!("{:x}", hash.finalize());
        let analysis = crate::runs::analysis::analyze(&input);
        Ok::<_, ApiError>((analysis, digest))
    })
    .await
    .map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ANALYSIS_FAILED",
            "Run analysis failed.",
        )
    })??;
    job.analysis_running.store(false, Ordering::Release);
    if job.cancelled.load(Ordering::Acquire) {
        return Err(lost());
    }
    validate_analysis(&analysis, &metadata)?;
    let normalized_hash = metadata["documents"]["archive"]["sha256"]
        .as_str()
        .ok_or_else(store::invalid)?;
    let config = store::versions()["config"]
        .as_str()
        .ok_or_else(store::invalid)?
        .to_owned();
    for (target, method) in [("lt1", "running-dfa-a1-075"), ("lt2", "running-dfa-a1-050")] {
        let value = &analysis["thresholds"][target];
        if value["trace"]["inputHash"] != input_hash
            || value["trace"]["inputRevision"] != normalized_id
            || value["trace"]["inputNormalizedHash"] != normalized_hash
            || value["trace"]["inputProjectionVersion"]
                != crate::runs::thresholds::PROJECTION_VERSION
            || value["method"]["id"] != method
            || value["method"]["version"] != crate::runs::thresholds::METHOD_VERSION
            || value["method"]["configurationHash"] != config
        {
            return Err(store::invalid());
        }
    }
    let validation = json!({"status":"validated","inputHash":input_hash,"projectionVersion":crate::runs::thresholds::PROJECTION_VERSION,"normalizedRevisionId":normalized_id,"normalizedDocumentHash":normalized_hash,"methodVersion":crate::runs::thresholds::METHOD_VERSION,"configurationHash":config});
    let mut tx = pool.begin().await.map_err(store::database_error)?;
    store::owner_lock(&mut tx, &job.owner)
        .await
        .map_err(store::database_error)?;
    fence(&mut tx, job).await?;
    let analysis_id = store::insert_revision(
        &mut tx,
        (&job.owner, &job.activity, job.generation),
        "analysis",
        &json!([normalized_id]),
        &analysis,
        None,
        &validation,
    )
    .await?;
    let manifest = Uuid::now_v7().to_string();
    sqlx::query("INSERT INTO runs_manifests(id,owner_id,activity_id,decoded_revision_id,normalized_revision_id,analysis_revision_id,generation,versions) VALUES($1,$2,$3,$4,$5,$6,$7,$8::jsonb)")
        .bind(&manifest).bind(&job.owner).bind(&job.activity).bind(&decoded_id).bind(&normalized_id).bind(analysis_id).bind(job.generation).bind(store::versions().to_string()).execute(&mut *tx).await.map_err(store::database_error)?;
    let (fingerprint, group, evidence) =
        store::observation_identity(&mut tx, &job.owner, &job.activity, &metadata).await?;
    let changed=sqlx::query("UPDATE runs_activities SET current_manifest_id=$3,summary=$4::jsonb,start_time=$5,end_time=$6,start_order=$5::timestamptz,end_order=$6::timestamptz,subtype=$7,observation_fingerprint=$8,observation_group_id=$9,duplicate_evidence=$10::jsonb,processing_status='ready',error_code=NULL WHERE id=$1 AND desired_generation=$2 AND NOT EXISTS(SELECT 1 FROM runs_tombstones WHERE activity_id=$1)")
        .bind(&job.activity).bind(job.generation).bind(manifest).bind(metadata["summary"].to_string()).bind(metadata["startTime"].as_str()).bind(metadata["endTime"].as_str()).bind(metadata["subtype"].as_str()).bind(fingerprint).bind(group).bind(evidence.to_string()).execute(&mut *tx).await.map_err(store::database_error)?.rows_affected();
    if changed != 1 {
        return Err(lost());
    }
    sqlx::query("INSERT INTO activities(id,owner_id,sport,started_at,activity_data,created_at) VALUES($1,$2,'running',$3,$4,$5) ON CONFLICT(id) DO UPDATE SET sport=EXCLUDED.sport,started_at=EXCLUDED.started_at,activity_data=EXCLUDED.activity_data WHERE activities.owner_id=EXCLUDED.owner_id")
        .bind(&job.activity).bind(&job.owner).bind(metadata["startTime"].as_str()).bind(serde_json::to_string(&legacy).map_err(|_|lost())?).bind(crate::db::created_at_now()).execute(&mut *tx).await.map_err(store::database_error)?;
    store::invalidate_cutoffs(
        &mut tx,
        &job.owner,
        metadata["endTime"].as_str().ok_or_else(lost)?,
    )
    .await
    .map_err(store::database_error)?;
    let completed=sqlx::query("UPDATE runs_jobs j SET status='ready',error_code=NULL,lease_owner=NULL,lease_until=NULL WHERE j.id=$1 AND j.lease_owner=$2 AND j.status='processing' AND j.lease_until>clock_timestamp() AND EXISTS(SELECT 1 FROM runs_activities a WHERE a.id=j.activity_id AND a.desired_generation=j.desired_generation) AND EXISTS(SELECT 1 FROM runs_slots s WHERE s.stage='worker' AND s.lease_owner=$2 AND s.lease_until>clock_timestamp()) AND NOT EXISTS(SELECT 1 FROM runs_tombstones t WHERE t.activity_id=j.activity_id)")
        .bind(&job.id).bind(&job.holder).execute(&mut *tx).await.map_err(store::database_error)?.rows_affected();
    if completed != 1 || job.cancelled.load(Ordering::Acquire) {
        return Err(lost());
    }
    tx.commit().await.map_err(store::database_error)
}
async fn fence(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, job: &Job) -> Result<(), ApiError> {
    let live=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM runs_activities a JOIN runs_jobs j ON j.activity_id=a.id WHERE a.id=$1 AND a.desired_generation=$2 AND j.id=$3 AND j.status='processing' AND j.lease_owner=$4 AND j.lease_until>clock_timestamp() AND EXISTS(SELECT 1 FROM runs_slots s WHERE s.stage='worker' AND s.lease_owner=$4 AND s.lease_until>clock_timestamp()) AND NOT EXISTS(SELECT 1 FROM runs_tombstones t WHERE t.activity_id=a.id))")
        .bind(&job.activity).bind(job.generation).bind(&job.id).bind(&job.holder).fetch_one(&mut **tx).await.map_err(store::database_error)?;
    if !live {
        return Err(lost());
    }
    Ok(())
}
fn validate_analysis(analysis: &Value, normalized: &Value) -> Result<(), ApiError> {
    let error = || {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_ANALYSIS_DOCUMENT",
            "Analysis failed semantic validation.",
        )
    };
    if analysis["schemaVersion"] != "2.0.0"
        || !analysis["quality"].is_object()
        || !analysis["thresholds"].is_object()
        || !analysis["transformations"].is_array()
    {
        return Err(error());
    }
    let duration = (store::timestamp(&normalized["endTime"])?
        - store::timestamp(&normalized["startTime"])?)
    .num_milliseconds() as f64
        / 1000.0;
    for segment in analysis["segments"].as_array().ok_or_else(error)? {
        let start = segment["startElapsedSeconds"].as_f64().ok_or_else(error)?;
        let end = segment["endElapsedSeconds"].as_f64().ok_or_else(error)?;
        if !start.is_finite()
            || !end.is_finite()
            || start < 0.0
            || end < start
            || end > duration + 0.001
        {
            return Err(error());
        }
    }
    for target in ["lt1", "lt2"] {
        let target = &analysis["thresholds"][target];
        if !matches!(
            target["status"].as_str(),
            Some("estimated" | "low_confidence" | "insufficient_data")
        ) {
            return Err(error());
        }
        if !target["value"].is_null()
            && target["value"]["heartRateBpm"]
                .as_f64()
                .is_none_or(|n| !n.is_finite() || n <= 0.0)
        {
            return Err(error());
        }
    }
    Ok(())
}
pub async fn scan_versions(pool: &PgPool) -> Result<(), ApiError> {
    let rows=sqlx::query("SELECT id,owner_id,current_manifest_id FROM runs_activities WHERE desired_versions<>$1::jsonb ORDER BY id LIMIT 64")
        .bind(store::versions().to_string()).fetch_all(pool).await.map_err(store::database_error)?;
    for row in rows {
        let id: String = row.get("id");
        let owner: String = row.get("owner_id");
        let manifest: Option<String> = row.get("current_manifest_id");
        let mut tx = pool.begin().await.map_err(store::database_error)?;
        store::owner_lock(&mut tx, &owner)
            .await
            .map_err(store::database_error)?;
        let current=sqlx::query("SELECT desired_versions::text AS versions,desired_generation FROM runs_activities WHERE id=$1 AND desired_versions<>$2::jsonb FOR UPDATE")
            .bind(&id).bind(store::versions().to_string()).fetch_optional(&mut *tx).await.map_err(store::database_error)?;
        let Some(current) = current else { continue };
        let old = store::parse(current.get::<&str, _>("versions"))?;
        let desired = store::versions();
        let reuse = manifest.is_some()
            && ["schema", "decoder", "observationRule", "projectionVersion"]
                .iter()
                .all(|key| old[*key] == desired[*key]);
        let history_only = desired.as_object().is_some_and(|versions| {
            versions.iter().all(|(key, value)| {
                key == "history" || key == "historySourceHash" || old[key] == *value
            })
        });
        if history_only {
            let generation=sqlx::query_scalar::<_,i64>("UPDATE runs_activities SET desired_versions=$2::jsonb,history_generation=history_generation+1 WHERE id=$1 RETURNING history_generation")
                .bind(&id).bind(desired.to_string()).fetch_one(&mut *tx).await.map_err(store::database_error)?;
            sqlx::query("UPDATE runs_jobs SET status='cancelled',lease_owner=NULL,lease_until=NULL WHERE activity_id=$1 AND stage='history' AND status IN ('queued','processing')").bind(&id).execute(&mut *tx).await.map_err(store::database_error)?;
            if manifest.is_some() {
                store::enqueue(&mut tx, &owner, &id, "history", generation, &json!([]))
                    .await
                    .map_err(store::database_error)?;
            }
            tx.commit().await.map_err(store::database_error)?;
            continue;
        }
        let generation = current.get::<i64, _>("desired_generation") + 1;
        sqlx::query("UPDATE runs_jobs SET status='cancelled',lease_owner=NULL,lease_until=NULL WHERE activity_id=$1 AND stage<>'history' AND status IN ('queued','processing')").bind(&id).execute(&mut *tx).await.map_err(store::database_error)?;
        sqlx::query("UPDATE runs_activities SET desired_generation=$2,desired_versions=$3::jsonb,processing_status='queued',error_code=NULL WHERE id=$1")
            .bind(&id).bind(generation).bind(desired.to_string()).execute(&mut *tx).await.map_err(store::database_error)?;
        store::enqueue(
            &mut tx,
            &owner,
            &id,
            if reuse { "analysis" } else { "process" },
            generation,
            &json!([]),
        )
        .await
        .map_err(store::database_error)?;
        tx.commit().await.map_err(store::database_error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "Requires the verified disposable Runs PostgreSQL environment."]
    async fn cpu_hold_retains_original_slot_until_blocking_work_finishes() {
        let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
        let expected = std::env::var("PGDATA").expect("PGDATA");
        assert!(
            !expected.is_empty(),
            "disposable PGDATA identity must not be empty"
        );
        let root = PgPool::connect(&url).await.unwrap();
        let actual: String = sqlx::query_scalar("SHOW data_directory")
            .fetch_one(&root)
            .await
            .unwrap();
        assert_eq!(actual, expected);
        let version: String = sqlx::query_scalar("SHOW server_version_num")
            .fetch_one(&root)
            .await
            .unwrap();
        assert!((180000..190000).contains(&version.parse::<i32>().unwrap()));
        let schema = format!("runs_lease_{}", Uuid::new_v4().simple());
        sqlx::QueryBuilder::<sqlx::Postgres>::new(format!("CREATE SCHEMA {schema}"))
            .build()
            .execute(&root)
            .await
            .unwrap();
        let selected = schema.clone();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(3)
            .after_connect(move |connection, _| {
                let schema = selected.clone();
                Box::pin(async move {
                    sqlx::query("SELECT set_config('search_path',$1,false)")
                        .bind(schema)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE runs_slots(stage TEXT PRIMARY KEY,slot INTEGER NOT NULL,lease_owner TEXT,lease_until TIMESTAMPTZ,physical_scope UUID NOT NULL DEFAULT gen_random_uuid(),physical_owner JSONB)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO runs_slots(stage,slot) VALUES('worker',1)")
            .execute(&pool)
            .await
            .unwrap();
        let original = Uuid::new_v4().to_string();
        let physical = acquire_slot(&pool, "worker", &original)
            .await
            .unwrap()
            .unwrap();
        let identity = physical.identity.clone();
        let admission = lease_guard(&pool, "worker", original.clone(), physical);
        assert_eq!(admission.holder(), original);
        let hold = admission.cpu_hold();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (finish, gate) = std::sync::mpsc::channel();
        let cpu = tokio::task::spawn_blocking(move || {
            let _hold = hold;
            started.send(()).unwrap();
            gate.recv().unwrap();
        });
        ready.await.unwrap();
        drop(admission);
        let expired = sqlx::query("UPDATE runs_slots SET lease_until=clock_timestamp()-interval '1 second' WHERE stage='worker' AND lease_owner=$1")
            .bind(&original).execute(&pool).await.unwrap();
        assert_eq!(expired.rows_affected(), 1);
        let replacement = Uuid::new_v4().to_string();
        assert!(
            acquire_slot(&pool, "worker", &replacement)
                .await
                .unwrap()
                .is_none(),
            "Expired lease admitted replacement while original blocking work remained alive"
        );
        let holder: Option<String> =
            sqlx::query_scalar("SELECT lease_owner FROM runs_slots WHERE stage='worker'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(holder.as_deref(), Some(original.as_str()));
        finish.send(()).unwrap();
        cpu.await.unwrap();
        let next = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(physical) = acquire_slot(&pool, "worker", &replacement).await.unwrap() {
                    break lease_guard(&pool, "worker", replacement.clone(), physical);
                }
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("CPU cleanup did not release admission");
        release_slot(&pool, "worker", &original, &identity)
            .await
            .unwrap();
        let holder: Option<String> =
            sqlx::query_scalar("SELECT lease_owner FROM runs_slots WHERE stage='worker'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            holder.as_deref(),
            Some(next.holder()),
            "Old cleanup released a new holder"
        );
        // A real native process inherits both its decoder slot and caller slot.
        // Dropping every parent holder must not free either capacity before exit.
        sqlx::query("INSERT INTO runs_slots(stage,slot,physical_scope) SELECT 'decode',1,physical_scope FROM runs_slots WHERE stage='worker'")
            .execute(&pool).await.unwrap();
        let decoder_holder = Uuid::new_v4().to_string();
        let physical = acquire_slot(&pool, "decode", &decoder_holder)
            .await
            .unwrap()
            .unwrap();
        let decoder = lease_guard(&pool, "decode", decoder_holder.clone(), physical);
        let capacity = decoder.decoder_hold().unwrap();
        let parent = next.decoder_hold().unwrap();
        let mut command = tokio::process::Command::new("/bin/cat");
        command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        process::inherit_capacity(&mut command, &capacity, Some(&parent));
        let mut child = command.spawn().unwrap();
        drop(capacity);
        drop(parent);
        drop(decoder);
        drop(next);
        sqlx::query("UPDATE runs_slots SET lease_until=clock_timestamp()-interval '1 second' WHERE lease_owner IN ($1,$2)")
            .bind(&replacement).bind(&decoder_holder).execute(&pool).await.unwrap();
        let successor = Uuid::new_v4().to_string();
        assert!(
            acquire_slot(&pool, "worker", &successor)
                .await
                .unwrap()
                .is_none(),
            "Inherited child admitted replacement worker after parent holders dropped"
        );
        assert!(
            acquire_slot(&pool, "decode", &successor)
                .await
                .unwrap()
                .is_none(),
            "Inherited child admitted replacement decoder after parent holders dropped"
        );
        drop(child.stdin.take());
        assert!(child.wait().await.unwrap().success());
        for stage in ["worker", "decode"] {
            let physical = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if let Some(physical) = acquire_slot(&pool, stage, &successor).await.unwrap() {
                        break physical;
                    }
                    sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .expect("Reaped child did not release physical capacity");
            let mut guard = lease_guard(&pool, stage, successor.clone(), physical);
            guard.release().await.unwrap();
        }
        pool.close().await;
        sqlx::QueryBuilder::<sqlx::Postgres>::new(format!("DROP SCHEMA {schema} CASCADE"))
            .build()
            .execute(&root)
            .await
            .unwrap();
        root.close().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "Requires the verified disposable Runs PostgreSQL environment and decoder binary."]
    async fn published_generation_survives_worker_loss_before_finish() {
        let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
        let expected = std::env::var("PGDATA").expect("PGDATA");
        assert!(
            !expected.is_empty(),
            "disposable PGDATA identity must not be empty"
        );
        let root = PgPool::connect(&url).await.unwrap();
        let actual: String = sqlx::query_scalar("SHOW data_directory")
            .fetch_one(&root)
            .await
            .unwrap();
        assert_eq!(actual, expected);
        let version: String = sqlx::query_scalar("SHOW server_version_num")
            .fetch_one(&root)
            .await
            .unwrap();
        assert!((180000..190000).contains(&version.parse::<i32>().unwrap()));
        let schema = format!("runs_publish_{}", Uuid::new_v4().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&root)
            .await
            .unwrap();
        let selected = schema.clone();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .after_connect(move |connection, _| {
                let schema = selected.clone();
                Box::pin(async move {
                    sqlx::query("SELECT set_config('search_path',$1,false)")
                        .bind(schema)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let owner = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,google_subject,email,created_at,updated_at) VALUES($1,$2,$3,$4,$4)")
            .bind(owner.to_string()).bind(format!("publish-test:{owner}")).bind(format!("{owner}@example.test")).bind(crate::db::created_at_now()).execute(&pool).await.unwrap();
        let bytes = include_bytes!("../../tests/fixtures/runs/garmin_run.fit");
        let document = decode_import(&pool, bytes, std::future::pending(), None)
            .await
            .unwrap();
        let mut admission = import_admission(&pool).await.unwrap();
        let imported = store::accept_spool(&pool, owner, bytes, document, admission.holder())
            .await
            .unwrap();
        admission.release().await.unwrap();
        let holder = Uuid::new_v4().to_string();
        let physical = acquire_slot(&pool, "worker", &holder)
            .await
            .unwrap()
            .unwrap();
        let mut guard = lease_guard(&pool, "worker", holder.clone(), physical);
        let job = claim(&pool, &holder).await.unwrap().unwrap();
        assert_eq!(job.activity, imported.activity_id.to_string());
        let hold = guard.cpu_hold();
        let (_signal, mut cancellation) = tokio::sync::watch::channel(false);
        execute(&pool, &job, &hold, &mut cancellation)
            .await
            .unwrap();
        drop(hold);
        // Simulate a dead original worker after publication, without ever calling finish.
        guard.release().await.unwrap();
        sqlx::query("UPDATE runs_jobs SET lease_until=clock_timestamp()-interval '1 second' WHERE id=$1 AND status='processing' AND lease_owner=$2")
            .bind(&job.id).bind(&holder).execute(&pool).await.unwrap();
        let recovery_holder = Uuid::new_v4().to_string();
        let physical = acquire_slot(&pool, "worker", &recovery_holder)
            .await
            .unwrap()
            .unwrap();
        let mut recovery = lease_guard(&pool, "worker", recovery_holder, physical);
        recover(&pool, &recovery.lease).await.unwrap();
        recovery.release().await.unwrap();
        sqlx::query("UPDATE runs_jobs SET next_attempt_at=clock_timestamp() WHERE status='queued'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            run_once(&pool).await.unwrap(),
            "Recovery must compute real history, not retry the published analysis"
        );
        assert!(
            !run_once(&pool).await.unwrap(),
            "Published generation must not be requeued"
        );
        let row=sqlx::query("SELECT a.current_manifest_id,a.processing_status,j.status AS job_status,j.lease_owner FROM runs_activities a JOIN runs_jobs j ON j.activity_id=a.id WHERE j.id=$1")
            .bind(&job.id).fetch_one(&pool).await.unwrap();
        assert_eq!(row.get::<String, _>("processing_status"), "ready");
        assert_eq!(row.get::<String, _>("job_status"), "ready");
        assert!(row.get::<Option<String>, _>("lease_owner").is_none());
        let history = super::super::history::for_manifest(
            &pool,
            owner,
            imported.activity_id,
            row.get::<Option<String>, _>("current_manifest_id")
                .as_deref(),
            sqlx::query_scalar("SELECT history_generation FROM runs_activities WHERE id=$1")
                .bind(&job.activity)
                .fetch_one(&pool)
                .await
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(history["activityId"], job.activity);
        assert_eq!(history["lt1"]["status"], "insufficient_data");
        let request = super::super::export::ExportRequest {
            activity_ids: vec![imported.activity_id],
            mode: super::super::export::ExportMode::Full,
            include_location: false,
            include_device_identifiers: false,
        };
        let created = super::super::export::create(&pool, owner, request)
            .await
            .unwrap();
        let response =
            super::super::export::serve(&pool, owner, created["token"].as_str().unwrap())
                .await
                .unwrap();
        let exported: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            exported["activities"][0]["historicalThresholds"]["computedAt"],
            history["computedAt"]
        );
        pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
            .execute(&root)
            .await
            .unwrap();
        root.close().await;
    }
}
