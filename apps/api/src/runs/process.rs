use crate::error::{ApiError, FitError};
use axum::http::StatusCode;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    time::{Duration, timeout},
};
use uuid::Uuid;

pub const MAX_INPUT: usize = 20 * 1024 * 1024;
pub const MAX_CONTROL: usize = 64 * 1024;
pub const MAX_SPOOL: u64 = 512 * 1024 * 1024;
const DECODE_TIMEOUT: Duration = Duration::from_secs(60);

pub struct DecodedSpool {
    pub directory: PathBuf,
    pub run: PathBuf,
    pub byte_length: u64,
    pub sha256: String,
    pub source_sha256: String,
    cpu_hold: Option<std::sync::Arc<super::jobs::SlotLease>>,
}
impl DecodedSpool {
    /// Only semantic CPU work retains capacity; decoding stays immediately killable.
    pub fn protect_cpu(&mut self, admission: &super::jobs::LeaseGuard) {
        self.cpu_hold = Some(admission.cpu_hold());
    }
    pub(crate) fn hold_cpu(&mut self, lease: std::sync::Arc<super::jobs::SlotLease>) {
        self.cpu_hold = Some(lease);
    }
}
impl Drop for DecodedSpool {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn failed() -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "DECODE_FAILED",
        "The decoder could not process this FIT file.",
    )
}
fn spool_failed() -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "DECODE_SPOOL_FAILED",
        "Private decoder storage failed.",
    )
}
fn unavailable() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "DECODER_UNAVAILABLE",
        "The isolated decoder is unavailable.",
    )
}

fn private_directory() -> Result<PathBuf, ApiError> {
    let directory = std::env::temp_dir().join(format!("runs-decode-{}", Uuid::new_v4()));
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&directory).map_err(|_| spool_failed())?;
    Ok(directory)
}
fn verify_private(path: &Path, directory: bool) -> Result<(), ApiError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| spool_failed())?;
    verify_metadata(&metadata, directory)
}
pub(crate) fn verify_metadata(
    metadata: &std::fs::Metadata,
    directory: bool,
) -> Result<(), ApiError> {
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(spool_failed());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o777 != if directory { 0o700 } else { 0o600 }
        {
            return Err(spool_failed());
        }
    }
    Ok(())
}
/// Only the decoder and its actual caller inherit capacity. The parent keeps
/// CLOEXEC; clearing it happens in the forked child immediately before exec.
pub(crate) fn inherit_capacity(
    command: &mut Command,
    capacity: &super::jobs::DecoderCapacity,
    parent: Option<&super::jobs::DecoderCapacity>,
) {
    let descriptors = [
        Some(capacity.descriptor()),
        parent.map(|value| value.descriptor()),
    ];
    unsafe {
        command.pre_exec(move || {
            for descriptor in descriptors.into_iter().flatten() {
                let flags = libc::fcntl(descriptor, libc::F_GETFD);
                if flags < 0
                    || libc::fcntl(descriptor, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}

/// Called only while jobs::decode_import owns the PostgreSQL decoder lease.
pub async fn decode(
    bytes: &[u8],
    cancellation: impl Future<Output = ()>,
    capacity: super::jobs::DecoderCapacity,
    parent: Option<super::jobs::DecoderCapacity>,
) -> Result<DecodedSpool, ApiError> {
    if bytes.is_empty() || bytes.len() > MAX_INPUT {
        return Err(ApiError::file_too_large());
    }
    let directory = private_directory()?;
    let mut spool = DecodedSpool {
        run: directory.join("run.json"),
        directory,
        byte_length: 0,
        sha256: String::new(),
        source_sha256: format!("{:x}", Sha256::digest(bytes)),
        cpu_hold: None,
    };
    let executable = std::env::var_os("RUNS_DECODER_EXECUTABLE")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_exe)
        .map_err(|_| unavailable())?;
    let mut command = Command::new(executable);
    command
        .arg("--runs-decode-child")
        .arg(&spool.directory)
        .env("TMPDIR", &spool.directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    inherit_capacity(&mut command, &capacity, parent.as_ref());
    #[cfg(target_os = "linux")]
    unsafe {
        command.pre_exec(|| {
            for (resource, limit) in [
                (libc::RLIMIT_AS, 512 * 1024 * 1024),
                (libc::RLIMIT_CPU, 60),
                (libc::RLIMIT_NPROC, 1),
                (libc::RLIMIT_FSIZE, MAX_SPOOL),
                (libc::RLIMIT_CORE, 0),
            ] {
                let bounds = libc::rlimit {
                    rlim_cur: limit,
                    rlim_max: limit,
                };
                if libc::setrlimit(resource, &bounds) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = command.spawn().map_err(|_| unavailable())?;
    let result = {
        let work = async {
            let mut input = child.stdin.take().ok_or_else(unavailable)?;
            let output = child.stdout.take().ok_or_else(unavailable)?;
            let write = async move {
                input.write_all(bytes).await?;
                input.shutdown().await?;
                drop(input);
                Ok::<_, std::io::Error>(())
            };
            let read = async {
                let mut data = Vec::new();
                output
                    .take((MAX_CONTROL + 1) as u64)
                    .read_to_end(&mut data)
                    .await?;
                Ok::<_, std::io::Error>(data)
            };
            let (_, data) = tokio::try_join!(write, read).map_err(|_| failed())?;
            if data.len() > MAX_CONTROL {
                return Err(failed());
            }
            let status = child.wait().await.map_err(|_| failed())?;
            if !status.success() {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    if status.signal().is_some_and(|signal| {
                        matches!(
                            signal,
                            libc::SIGXCPU | libc::SIGXFSZ | libc::SIGABRT | libc::SIGKILL
                        )
                    }) {
                        return Err(ApiError::new(
                            StatusCode::UNPROCESSABLE_ENTITY,
                            "DECODE_RESOURCE_LIMIT",
                            "The decoder exceeded its resource limit.",
                        ));
                    }
                }
                let value: Value = serde_json::from_slice(&data).map_err(|_| failed())?;
                return Err(child_error(&value));
            }
            let value: Value = serde_json::from_slice(&data).map_err(|_| failed())?;
            if value["schemaVersion"] != "runs-spool-1"
                || value["sourceSha256"] != spool.source_sha256
                || value["document"]["name"] != "run.json"
            {
                return Err(failed());
            }
            let length = value["document"]["byteLength"]
                .as_u64()
                .filter(|n| *n > 0 && *n <= MAX_SPOOL)
                .ok_or_else(failed)?;
            let expected = value["document"]["sha256"]
                .as_str()
                .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or_else(failed)?;
            verify_private(&spool.directory, true)?;
            verify_private(&spool.run, false)?;
            let mut options = tokio::fs::OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                options.custom_flags(libc::O_NOFOLLOW);
            }
            let mut file = options.open(&spool.run).await.map_err(|_| spool_failed())?;
            let metadata = file.metadata().await.map_err(|_| spool_failed())?;
            verify_metadata(&metadata, false)?;
            if metadata.len() != length {
                return Err(failed());
            }
            let mut hash = Sha256::new();
            let mut read_bytes = 0u64;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let count = file.read(&mut buffer).await.map_err(|_| spool_failed())?;
                if count == 0 {
                    break;
                }
                read_bytes += count as u64;
                if read_bytes > length {
                    return Err(failed());
                }
                hash.update(&buffer[..count]);
            }
            let digest = format!("{:x}", hash.finalize());
            if read_bytes != length || digest != expected {
                return Err(failed());
            }
            spool.byte_length = length;
            spool.sha256 = digest;
            Ok(())
        };
        tokio::select! {biased;
            _=cancellation=>Err(ApiError::new(StatusCode::SERVICE_UNAVAILABLE,"JOB_LEASE_LOST","Processing lease expired or was cancelled.")),
            result=timeout(DECODE_TIMEOUT,work)=>match result{Ok(result)=>result,Err(_)=>Err(ApiError::new(StatusCode::UNPROCESSABLE_ENTITY,"DECODE_TIMEOUT","The FIT decoder exceeded its time limit."))},
        }
    };
    if let Err(error) = result {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err(error);
    }
    Ok(spool)
}
fn child_error(value: &Value) -> ApiError {
    match value["error"]["code"].as_str() {
        Some("UNSUPPORTED_MANUFACTURER") => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "UNSUPPORTED_MANUFACTURER",
            "Only Garmin activity FIT files are supported.",
        ),
        Some("UNSUPPORTED_SPORT") => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "UNSUPPORTED_SPORT",
            "Only running activities are supported.",
        ),
        Some("UNSUPPORTED_SESSION_LAYOUT") => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "UNSUPPORTED_SESSION_LAYOUT",
            "Exactly one supported running session is required.",
        ),
        Some("INVALID_FIT") => ApiError::invalid_fit(),
        Some("DECODE_SPOOL_FAILED") => spool_failed(),
        Some("DECODE_SPOOL_LIMIT") => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "DECODE_SPOOL_LIMIT",
            "The decoded document exceeds the private storage limit.",
        ),
        Some("DECODE_RESOURCE_LIMIT") => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "DECODE_RESOURCE_LIMIT",
            "The decoder exceeded its resource limit.",
        ),
        _ => failed(),
    }
}
/// Invoke before constructing Tokio, reading configuration, or opening a DB.
pub fn child_mode() -> Option<i32> {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new("--runs-decode-child")) {
        return None;
    }
    Some(run_child())
}
fn run_child() -> i32 {
    let Some(directory) = std::env::args_os().nth(2).map(PathBuf::from) else {
        return emit_error("DECODE_SPOOL_FAILED");
    };
    if verify_private(&directory, true).is_err() {
        return emit_error("DECODE_SPOOL_FAILED");
    }
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.is_empty()
        || bytes.len() > MAX_INPUT
    {
        return emit_error("INVALID_FIT");
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let Ok(file) = options.open(directory.join("run.json")) else {
        return emit_error("DECODE_SPOOL_FAILED");
    };
    // Buffer serde's tiny writes before quota accounting and SHA-256 updates.
    let mut writer = std::io::BufWriter::with_capacity(
        64 * 1024,
        SpoolWriter {
            inner: file,
            written: 0,
            hash: Sha256::new(),
            limit_exceeded: false,
        },
    );
    let decoded = crate::fit::runs::decode_run_to_writer(&bytes, &mut writer);
    if writer.get_ref().limit_exceeded {
        return emit_error("DECODE_SPOOL_LIMIT");
    }
    if let Err(error) = decoded {
        return match error {
            FitError::UnsupportedRun { code, .. } | FitError::ProcessingFailed { code, .. } => {
                emit_error(code)
            }
            FitError::InvalidFit => emit_error("INVALID_FIT"),
        };
    }
    if writer.flush().is_err() {
        return emit_error(if writer.get_ref().limit_exceeded {
            "DECODE_SPOOL_LIMIT"
        } else {
            "DECODE_SPOOL_FAILED"
        });
    }
    // No descriptor can describe bytes still buffered or a failed final flush.
    let (writer, _) = writer.into_parts();
    let control = json!({"schemaVersion":"runs-spool-1","sourceSha256":format!("{:x}",Sha256::digest(&bytes)),"document":{"name":"run.json","byteLength":writer.written,"sha256":format!("{:x}",writer.hash.finalize())}});
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    if serde_json::to_writer(&mut output, &control).is_err() || output.flush().is_err() {
        return 1;
    }
    0
}
struct SpoolWriter<W> {
    inner: W,
    written: u64,
    hash: Sha256,
    limit_exceeded: bool,
}
impl<W: Write> Write for SpoolWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let remaining = MAX_SPOOL.saturating_sub(self.written);
        if remaining == 0 {
            self.limit_exceeded = true;
            return Err(std::io::Error::other(
                "Private decoder storage limit exceeded",
            ));
        }
        let count = self
            .inner
            .write(&bytes[..bytes.len().min(remaining as usize)])?;
        self.hash.update(&bytes[..count]);
        self.written += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
fn emit_error(code: &str) -> i32 {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let _ = serde_json::to_writer(&mut output, &json!({"error":{"code":code}}));
    let _ = output.flush();
    1
}
/// Sweep only this binary's generated, private, owned spool directories.
pub fn sweep_spools() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(suffix) = name.strip_prefix("runs-decode-") else {
            continue;
        };
        if Uuid::parse_str(suffix).is_err() {
            continue;
        }
        let path = entry.path();
        if verify_private(&path, true).is_err() {
            continue;
        }
        let old = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > std::time::Duration::from_secs(3600));
        if old {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spool_hash_tracks_partial_writes_at_quota() {
        struct PartialWriter(Vec<u8>);
        impl Write for PartialWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                let count = bytes.len().min(2);
                self.0.extend_from_slice(&bytes[..count]);
                Ok(count)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        // Exercise the actual quota boundary without allocating 512 MiB.
        let mut writer = SpoolWriter {
            inner: PartialWriter(Vec::new()),
            written: MAX_SPOOL - 5,
            hash: Sha256::new(),
            limit_exceeded: false,
        };
        writer.write_all(b"abcde").unwrap();
        assert_eq!(writer.inner.0, b"abcde");
        assert_eq!(writer.written, MAX_SPOOL);
        assert_eq!(
            format!("{:x}", writer.hash.clone().finalize()),
            format!("{:x}", Sha256::digest(b"abcde"))
        );
        assert_eq!(writer.write(b"").unwrap(), 0);
        assert!(!writer.limit_exceeded);
        assert!(writer.write_all(b"f").is_err());
        assert!(writer.limit_exceeded);
        assert_eq!(writer.inner.0, b"abcde");
        assert_eq!(writer.written, MAX_SPOOL);
        assert_eq!(
            format!("{:x}", writer.hash.finalize()),
            format!("{:x}", Sha256::digest(b"abcde"))
        );
    }

    #[test]
    fn spool_final_flush_records_quota_failure() {
        let spool = SpoolWriter {
            inner: Vec::new(),
            written: MAX_SPOOL - 3,
            hash: Sha256::new(),
            limit_exceeded: false,
        };
        let mut writer = std::io::BufWriter::with_capacity(64 * 1024, spool);
        writer.write_all(b"four").unwrap();
        assert!(writer.get_ref().inner.is_empty());
        assert!(writer.flush().is_err());
        let (spool, buffer) = writer.into_parts();
        assert!(spool.limit_exceeded);
        assert_eq!(spool.inner, b"fou");
        assert_eq!(spool.written, MAX_SPOOL);
        assert_eq!(
            format!("{:x}", spool.hash.finalize()),
            format!("{:x}", Sha256::digest(b"fou"))
        );
        assert_eq!(buffer.unwrap(), b"r");
    }

    #[test]
    fn spool_successful_flush_hashes_exact_document() {
        let spool = SpoolWriter {
            inner: Vec::new(),
            written: 0,
            hash: Sha256::new(),
            limit_exceeded: false,
        };
        let mut writer = std::io::BufWriter::with_capacity(64 * 1024, spool);
        let document =
            json!({"samples":[{"speedMps":2.5},{"speedMps":null}],"activityName":"A \"run\""});
        serde_json::to_writer(&mut writer, &document).unwrap();
        writer.flush().unwrap();
        let (spool, buffer) = writer.into_parts();
        assert!(buffer.unwrap().is_empty());
        assert_eq!(spool.written, spool.inner.len() as u64);
        assert_eq!(
            format!("{:x}", spool.hash.finalize()),
            format!("{:x}", Sha256::digest(&spool.inner))
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&spool.inner).unwrap(),
            document
        );
    }

    #[tokio::test]
    #[ignore = "Requires RUNS_DECODER_EXECUTABLE pointing to the rebuilt real API binary."]
    async fn genuine_small_fit_closes_stdin_before_waiting_for_child_output() {
        std::env::var_os("RUNS_DECODER_EXECUTABLE").expect("real RUNS_DECODER_EXECUTABLE");
        let bytes = include_bytes!("../../tests/fixtures/runs/garmin_run.fit");
        let physical = super::super::jobs::PhysicalSlot::local(Uuid::new_v4(), "decode")
            .unwrap()
            .unwrap();
        let capacity = super::super::jobs::DecoderCapacity { physical };
        let spool = timeout(
            Duration::from_secs(5),
            decode(bytes, std::future::pending(), capacity, None),
        )
        .await
        .expect("Child waited for unclosed stdin")
        .unwrap();
        assert_eq!(
            spool.source_sha256,
            "16f00e938171283656a36f0e41e4fda43368f662f28a2abde8c1379b87876232"
        );
        let document: Value =
            serde_json::from_reader(std::fs::File::open(&spool.run).unwrap()).unwrap();
        assert_eq!(document["normalized"]["sport"], "running");
        let speeds: Vec<_> = document["normalized"]["samples"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sample| sample["speedMps"].as_f64())
            .collect();
        assert_eq!(
            speeds,
            vec![Some(2.0), Some(4.0), Some(0.0), Some(0.0)],
            "Public enhanced-speed and recorded-zero oracle"
        );
        let directory = spool.directory.clone();
        drop(spool);
        assert!(
            !directory.exists(),
            "Completed child left its private workspace"
        );
    }
}
