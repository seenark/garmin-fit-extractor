use std::{io::{self, Read, Seek, SeekFrom}, ops::Range, time::{Duration, Instant}};

use axum::{extract::Multipart, http::StatusCode};

use crate::error::ApiError;

pub const MAX_FILES: usize = 10;
pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_EXTRACTED_BYTES: usize = 100 * 1024 * 1024;
pub const MAX_FIT_MEMBERS: usize = 50;
pub const MAX_ZIP_MEMBERS: usize = 1000;
const PARSE_TIME: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct Upload {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputStatus {
    Ready,
    Unsupported,
    Failed,
}

#[derive(Debug)]
pub struct InputItem {
    pub name: String,
    pub bytes: Option<Vec<u8>>,
    pub status: InputStatus,
    pub reason: Option<&'static str>,
    pub warnings: Vec<&'static str>,
}

/// Reads bounded multipart bytes, then stages the whole request before callers can commit.
pub async fn parse(mut multipart: Multipart) -> Result<Vec<InputItem>, ApiError> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let mut uploads = Vec::new();
    let mut rejected_inputs = Vec::new();
    while let Some(mut field) = tokio::time::timeout_at(deadline, multipart.next_field())
        .await.map_err(|_| timeout())?.map_err(multipart_error)?
    {
        if field.name() != Some("files") {
            return Err(ApiError::unknown_field());
        }
        if uploads.len() == MAX_FILES {
            return Err(too_many_files());
        }
        let supplied_name = field.file_name().unwrap_or("upload");
        let mut rejection = invalid_filename(supplied_name).then_some("INVALID_FILE_NAME");
        let name = if rejection.is_some() { receipt_name(supplied_name) } else { supplied_name.to_owned() };
        let mut bytes = Vec::new();
        while let Some(chunk) = tokio::time::timeout_at(deadline, field.chunk())
            .await.map_err(|_| timeout())?.map_err(multipart_error)?
        {
            if rejection.is_none() && bytes.len().saturating_add(chunk.len()) > MAX_FILE_BYTES {
                rejection = Some("FILE_TOO_LARGE");
                bytes = Vec::new();
            }
            if rejection.is_none() {
                bytes.extend_from_slice(&chunk);
            }
        }
        rejected_inputs.push(rejection);
        uploads.push(Upload { name, bytes });
    }
    tokio::task::spawn_blocking(move || stage_uploads(uploads, &rejected_inputs))
        .await.map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "IMPORT_FAILED", "Import staging failed."))?
}

fn multipart_error(error: axum::extract::multipart::MultipartError) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::request_too_large()
    } else {
        ApiError::invalid_multipart()
    }
}

/// Public data seam. Only header identification happens here; FIT decoding belongs in the bounded child.
pub fn stage(uploads: Vec<Upload>) -> Result<Vec<InputItem>, ApiError> {
    stage_uploads(uploads, &[])
}

fn stage_uploads(uploads: Vec<Upload>, rejected_inputs: &[Option<&'static str>]) -> Result<Vec<InputItem>, ApiError> {
    if uploads.is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "EMPTY_BATCH", "Upload at least one FIT or ZIP file."));
    }
    if uploads.len() > MAX_FILES {
        return Err(too_many_files());
    }
    let mut budget = Budget { deadline: Instant::now() + PARSE_TIME, expanded: 0, fits: 0, members: 0 };
    let mut items = Vec::new();
    for (index, upload) in uploads.into_iter().enumerate() {
        budget.check_time()?;
        let name = receipt_name(&upload.name);
        if let Some(reason) = rejected_inputs.get(index).copied().flatten() {
            items.push(rejected(name, reason, false));
        } else if upload.bytes.len() > MAX_FILE_BYTES {
            items.push(rejected(name, "FILE_TOO_LARGE", false));
        } else if invalid_filename(&upload.name) {
            items.push(rejected(name, "INVALID_FILE_NAME", false));
        } else if is_zip(&upload.bytes) || suffix(&upload.name, ".zip") {
            stage_archive(upload.bytes, name, &mut budget, &mut items)?;
        } else {
            budget.add_expanded(upload.bytes.len())?;
            items.push(identify(name, upload.bytes, suffix(&upload.name, ".fit"), &mut budget)?);
        }
    }
    budget.check_time()?;
    Ok(items)
}

struct Budget {
    deadline: Instant,
    expanded: usize,
    fits: usize,
    members: usize,
}

impl Budget {
    fn check_time(&self) -> Result<(), ApiError> {
        if Instant::now() >= self.deadline { Err(timeout()) } else { Ok(()) }
    }

    fn add_expanded(&mut self, count: usize) -> Result<(), ApiError> {
        self.expanded = self.expanded.saturating_add(count);
        if self.expanded > MAX_EXTRACTED_BYTES { Err(archive_limit()) } else { Ok(()) }
    }

    fn add_fit(&mut self) -> Result<(), ApiError> {
        self.fits += 1;
        if self.fits > MAX_FIT_MEMBERS { Err(archive_limit()) } else { Ok(()) }
    }
}

fn identify(name: String, bytes: Vec<u8>, expected_fit: bool, budget: &mut Budget) -> Result<InputItem, ApiError> {
    let header = bytes.len() >= 12 && bytes[0] >= 12 && bytes.get(8..12) == Some(b".FIT")
        && bytes.len() >= bytes[0] as usize;
    if header || expected_fit {
        budget.add_fit()?;
    }
    Ok(if header {
        InputItem { name, bytes: Some(bytes), status: InputStatus::Ready, reason: None, warnings: Vec::new() }
    } else {
        rejected(name, if expected_fit { "FIT_HEADER_INVALID" } else { "UNSUPPORTED_FILE" }, !expected_fit)
    })
}

fn stage_archive(bytes: Vec<u8>, archive_name: String, budget: &mut Budget, items: &mut Vec<InputItem>) -> Result<(), ApiError> {
    let directory = match central_directory(&bytes, budget) {
        Ok(directory) => directory,
        Err(CentralError::Limit) => return Err(archive_limit()),
        Err(CentralError::Invalid) => {
            items.push(rejected(archive_name, "INVALID_ZIP", false));
            return Ok(());
        }
    };
    if directory.members.is_empty() {
        items.push(rejected(archive_name, "NO_FIT_FILES", true));
        return Ok(());
    }
    for (index, range) in directory.members.iter().enumerate() {
        budget.check_time()?;
        let central = &bytes[range.clone()];
        let name_length = le16(central, 28) as usize;
        let raw_name = String::from_utf8_lossy(&central[46..46 + name_length]);
        let mut name = format!("{}::{} [member {}]", archive_name, receipt_name(&raw_name), index + 1);
        let expected_fit = suffix(&raw_name, ".fit");
        if unsafe_path(&raw_name) {
            if expected_fit { budget.add_fit()?; }
            items.push(rejected(name, "UNSAFE_ARCHIVE_PATH", false));
            continue;
        }
        if !valid_local_header(&bytes, central, directory.start) {
            if expected_fit { budget.add_fit()?; }
            items.push(rejected(name, "ARCHIVE_MEMBER_INVALID", false));
            continue;
        }
        let mut end = [0_u8; 22];
        end[..4].copy_from_slice(b"PK\x05\x06");
        end[8..10].copy_from_slice(&1_u16.to_le_bytes());
        end[10..12].copy_from_slice(&1_u16.to_le_bytes());
        end[12..16].copy_from_slice(&(central.len() as u32).to_le_bytes());
        end[16..20].copy_from_slice(&(directory.start as u32).to_le_bytes());
        // zip 2 indexes by filename and drops duplicates. A borrowed one-entry view preserves
        // every original central record, local bytes, CRC, name, attributes and extra field.
        let reader = MemberArchive { parts: [&bytes[..directory.start], central, &end], position: 0 };
        let mut archive = match zip::ZipArchive::new(reader) {
            Ok(archive) => archive,
            Err(_) => {
                if expected_fit { budget.add_fit()?; }
                items.push(rejected(name, "ARCHIVE_MEMBER_INVALID", false));
                continue;
            }
        };
        let mut member = match archive.by_index(0) {
            Ok(member) => member,
            Err(_) => {
                if expected_fit { budget.add_fit()?; }
                items.push(rejected(name, "ARCHIVE_MEMBER_INVALID", false));
                continue;
            }
        };
        name = format!("{}::{} [member {}]", archive_name, receipt_name(member.name()), index + 1);
        let expected_fit = expected_fit || suffix(member.name(), ".fit");
        let problem = if unsafe_path(member.name()) {
            Some("UNSAFE_ARCHIVE_PATH")
        } else if member.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            Some("ARCHIVE_SYMLINK")
        } else if suffix(member.name(), ".zip") || suffix(member.name(), ".7z") || suffix(member.name(), ".rar") || suffix(member.name(), ".tar") || suffix(member.name(), ".gz") {
            Some("NESTED_ARCHIVE")
        } else if member.size() > MAX_FILE_BYTES as u64 {
            Some("FILE_TOO_LARGE")
        } else if member.size() > member.compressed_size().saturating_mul(100) {
            Some("ARCHIVE_RATIO_EXCEEDED")
        } else { None };
        if let Some(reason) = problem {
            if expected_fit { budget.add_fit()?; }
            items.push(rejected(name, reason, false));
            continue;
        }
        let compressed = member.compressed_size();
        let declared = member.size();
        let mut data = Vec::new();
        let mut chunk = [0_u8; 64 * 1024];
        let mut problem = None;
        loop {
            budget.check_time()?;
            // Read one extra byte to distinguish exact-limit EOF from an oversized member.
            let remaining = MAX_FILE_BYTES.saturating_add(1).saturating_sub(data.len()).min(chunk.len());
            let count = match member.read(&mut chunk[..remaining]) {
                Ok(0) => break,
                Ok(count) => count,
                Err(_) => { problem = Some("ARCHIVE_MEMBER_INVALID"); break; }
            };
            budget.add_expanded(count)?;
            data.extend_from_slice(&chunk[..count]);
            if data.len() > MAX_FILE_BYTES { problem = Some("FILE_TOO_LARGE"); break; }
            if data.len() as u64 > compressed.saturating_mul(100) { problem = Some("ARCHIVE_RATIO_EXCEEDED"); break; }
        }
        budget.check_time()?;
        if problem.is_none() && data.len() as u64 != declared {
            problem = Some("ARCHIVE_MEMBER_INVALID");
        }
        if problem.is_none() && nested_content(&data) { problem = Some("NESTED_ARCHIVE"); }
        if let Some(reason) = problem {
            if expected_fit { budget.add_fit()?; }
            items.push(rejected(name, reason, false));
        } else if member.is_dir() {
            items.push(rejected(name, if data.is_empty() { "UNSUPPORTED_FILE" } else { "ARCHIVE_MEMBER_INVALID" }, data.is_empty()));
        } else {
            items.push(identify(name, data, expected_fit, budget)?);
        }
    }
    Ok(())
}

struct Directory {
    start: usize,
    members: Vec<Range<usize>>,
}

enum CentralError { Invalid, Limit }

/// Preflight actual central records before zip can allocate from untrusted count metadata.
fn central_directory(bytes: &[u8], budget: &mut Budget) -> Result<Directory, CentralError> {
    if bytes.len() < 22 { return Err(CentralError::Invalid); }
    let start = bytes.len().saturating_sub(22 + u16::MAX as usize);
    let end = (start..=bytes.len() - 22).rev().find(|&index| bytes[index..index + 4] == *b"PK\x05\x06"
        && index + 22 + le16(bytes, index + 20) as usize == bytes.len()).ok_or(CentralError::Invalid)?;
    if le16(bytes, end + 4) != 0 || le16(bytes, end + 6) != 0 { return Err(CentralError::Invalid); }
    let count = le16(bytes, end + 10) as usize;
    if count > MAX_ZIP_MEMBERS || budget.members.saturating_add(count) > MAX_ZIP_MEMBERS { return Err(CentralError::Limit); }
    if le16(bytes, end + 8) as usize != count { return Err(CentralError::Invalid); }
    let directory_start = le32(bytes, end + 16) as usize;
    let size = le32(bytes, end + 12) as usize;
    if directory_start.checked_add(size) != Some(end) { return Err(CentralError::Invalid); }
    let mut cursor = directory_start;
    let mut members = Vec::new();
    while cursor < end {
        if members.len() == MAX_ZIP_MEMBERS || budget.members + members.len() == MAX_ZIP_MEMBERS { return Err(CentralError::Limit); }
        if end - cursor < 46 || bytes[cursor..cursor + 4] != *b"PK\x01\x02" { return Err(CentralError::Invalid); }
        let length = 46 + le16(bytes, cursor + 28) as usize + le16(bytes, cursor + 30) as usize + le16(bytes, cursor + 32) as usize;
        let next = cursor.checked_add(length).filter(|&next| next <= end).ok_or(CentralError::Invalid)?;
        // Multi-disk and ZIP64 offsets/sizes require unsupported layouts, never wrap arithmetic.
        if le16(bytes, cursor + 34) != 0 || [20, 24, 42].iter().any(|offset| le32(bytes, cursor + offset) == u32::MAX) {
            return Err(CentralError::Invalid);
        }
        members.push(cursor..next);
        cursor = next;
    }
    if members.len() != count { return Err(CentralError::Invalid); }
    budget.members += members.len();
    Ok(Directory { start: directory_start, members })
}

fn valid_local_header(bytes: &[u8], central: &[u8], directory_start: usize) -> bool {
    let local = le32(central, 42) as usize;
    if local.checked_add(30).is_none_or(|offset| offset > directory_start)
        || bytes.get(local..local + 4) != Some(b"PK\x03\x04") { return false; }
    let local_name = le16(bytes, local + 26) as usize;
    let local_extra = le16(bytes, local + 28) as usize;
    let Some(data_start) = local.checked_add(30 + local_name + local_extra) else { return false; };
    if data_start.checked_add(le32(central, 20) as usize).is_none_or(|offset| offset > directory_start) { return false; }
    let central_name = le16(central, 28) as usize;
    bytes.get(local + 30..local + 30 + local_name) == central.get(46..46 + central_name)
        && le16(bytes, local + 6) == le16(central, 8)
        && le16(bytes, local + 8) == le16(central, 10)
}

/// Seekable borrowed ZIP view: original local-data area, one unchanged central record, EOCD.
struct MemberArchive<'a> {
    parts: [&'a [u8]; 3],
    position: u64,
}

impl Read for MemberArchive<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut offset = self.position;
        for part in self.parts {
            if offset < part.len() as u64 {
                let offset = offset as usize;
                let count = buffer.len().min(part.len() - offset);
                buffer[..count].copy_from_slice(&part[offset..offset + count]);
                self.position += count as u64;
                return Ok(count);
            }
            offset = offset.saturating_sub(part.len() as u64);
        }
        Ok(0)
    }
}

impl Seek for MemberArchive<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let length = self.parts.iter().map(|part| part.len() as u64).sum::<u64>();
        let position = match from {
            SeekFrom::Start(position) => position as i128,
            SeekFrom::Current(offset) => self.position as i128 + offset as i128,
            SeekFrom::End(offset) => length as i128 + offset as i128,
        };
        if !(0..=u64::MAX as i128).contains(&position) { return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid seek")); }
        self.position = position as u64;
        Ok(self.position)
    }
}

fn le16(bytes: &[u8], index: usize) -> u16 { u16::from_le_bytes([bytes[index], bytes[index + 1]]) }
fn le32(bytes: &[u8], index: usize) -> u32 { u32::from_le_bytes([bytes[index], bytes[index + 1], bytes[index + 2], bytes[index + 3]]) }
fn suffix(name: &str, suffix: &str) -> bool { name.get(name.len().saturating_sub(suffix.len())..).is_some_and(|tail| tail.eq_ignore_ascii_case(suffix)) }
fn is_zip(bytes: &[u8]) -> bool { bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") || bytes.starts_with(b"PK\x07\x08") }
fn nested_content(bytes: &[u8]) -> bool { is_zip(bytes) || bytes.starts_with(b"7z\xbc\xaf\x27\x1c") || bytes.starts_with(b"Rar!\x1a\x07") || bytes.starts_with(&[0x1f, 0x8b]) || bytes.get(257..262) == Some(b"ustar") }
fn unsafe_path(name: &str) -> bool {
    name.is_empty() || name.starts_with('/') || name.starts_with('\\') || name.contains('\\') || name.contains(':')
        || name.chars().any(|character| character.is_control()) || name.split('/').any(|part| part == ".." || part == ".")
}
fn receipt_name(name: &str) -> String {
    let basename = name.rsplit(['/', '\\']).next().unwrap_or("upload");
    let name: String = basename.chars().take(160).map(|character| if character.is_control() || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') { '_' } else { character }).collect();
    if name.is_empty() { "upload".to_owned() } else { name }
}
fn rejected(name: String, reason: &'static str, unsupported: bool) -> InputItem {
    InputItem { name, bytes: None, status: if unsupported { InputStatus::Unsupported } else { InputStatus::Failed }, reason: Some(reason), warnings: Vec::new() }
}
fn too_many_files() -> ApiError { ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "TOO_MANY_FILES", "Upload at most 10 files.") }
fn archive_limit() -> ApiError { ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "ARCHIVE_LIMIT_EXCEEDED", "The request exceeds archive member or extracted byte limits.") }
fn timeout() -> ApiError { ApiError::new(StatusCode::REQUEST_TIMEOUT, "IMPORT_TIMEOUT", "Import parsing exceeded its time limit.") }
fn invalid_filename(name: &str) -> bool { name.len() > 255 || name.chars().any(|character| character.is_ascii_control()) }
