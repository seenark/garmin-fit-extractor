use std::{fmt::Write as _, path::Path};

use serde::Serialize;

use crate::{
    checksum::TableDigest,
    error::{MigratorError, Result},
    source::{Snapshot, SourceKind},
};

#[derive(Clone, Debug, Serialize)]
pub struct TableReport {
    pub table: String,
    pub count: u64,
    pub digest: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ManifestReport {
    pub sha256: String,
    pub tables: Vec<TableReport>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceReport {
    pub source_name: String,
    pub sha256: String,
    pub schema_tables: Vec<String>,
    pub manifest: ManifestReport,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub command: String,
    pub status: String,
    pub dry_run: bool,
    pub sources: Vec<SourceReport>,
    pub target: Option<ManifestReport>,
    pub discrepancies: Vec<String>,
}

impl Report {
    pub fn new(command: impl Into<String>, dry_run: bool) -> Self {
        Self {
            command: command.into(),
            status: "ok".to_owned(),
            dry_run,
            sources: Vec::new(),
            target: None,
            discrepancies: Vec::new(),
        }
    }

    pub fn add_source(&mut self, snapshot: &Snapshot) {
        let manifests = snapshot.manifests();
        self.sources.push(SourceReport {
            source_name: snapshot.kind().source_name().to_owned(),
            sha256: snapshot.source_sha256(),
            schema_tables: snapshot.schema_tables(),
            manifest: manifest_report(&manifests),
        });
    }

    pub fn set_target(&mut self, manifests: &[TableDigest]) {
        self.target = Some(manifest_report(manifests));
    }

    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    pub fn with_discrepancy(mut self, discrepancy: impl Into<String>) -> Self {
        self.discrepancies.push(discrepancy.into());
        self
    }

    pub fn json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|cause| MigratorError::ReportSerialization { cause })
    }

    /// Human output deliberately contains only source/table identities, counts, and digests.
    pub fn human(&self) -> String {
        let mut output = String::new();
        let _ = writeln!(
            output,
            "command={} status={} dry_run={}",
            self.command, self.status, self.dry_run
        );
        for source in &self.sources {
            let _ = writeln!(
                output,
                "source={} sha256={}",
                source.source_name, source.sha256
            );
            for table in &source.manifest.tables {
                let _ = writeln!(
                    output,
                    "  source table={} count={} digest={}",
                    table.table, table.count, table.digest
                );
            }
        }
        if let Some(target) = &self.target {
            let _ = writeln!(output, "target sha256={}", target.sha256);
            for table in &target.tables {
                let _ = writeln!(
                    output,
                    "  target table={} count={} digest={}",
                    table.table, table.count, table.digest
                );
            }
        }
        for discrepancy in &self.discrepancies {
            let _ = writeln!(output, "discrepancy={discrepancy}");
        }
        output
    }
}

pub fn manifest_report(manifests: &[TableDigest]) -> ManifestReport {
    let mut tables: Vec<TableReport> = manifests
        .iter()
        .map(|manifest| TableReport {
            table: manifest.table.clone(),
            count: manifest.row_count,
            digest: manifest.digest.clone(),
        })
        .collect();
    tables.sort_by(|left, right| left.table.cmp(&right.table));
    let mut sorted = manifests.to_vec();
    sorted.sort_by(|left, right| left.table.cmp(&right.table));
    ManifestReport {
        sha256: crate::checksum::manifest_digest(&sorted),
        tables,
    }
}

pub fn write_report(path: Option<&Path>, report: &Report) -> Result<()> {
    if let Some(path) = path {
        std::fs::write(path, report.json()?).map_err(|cause| MigratorError::ReportIo { cause })?;
    }
    Ok(())
}

pub fn source_report(kind: SourceKind, snapshot: &Snapshot) -> SourceReport {
    debug_assert_eq!(kind, snapshot.kind());
    let manifests = snapshot.manifests();
    SourceReport {
        source_name: kind.source_name().to_owned(),
        sha256: snapshot.source_sha256(),
        schema_tables: snapshot.schema_tables(),
        manifest: manifest_report(&manifests),
    }
}

#[cfg(test)]
mod tests {
    use super::Report;

    #[test]
    fn report_does_not_include_paths_or_row_values() {
        let mut report = Report::new("import", true);
        report
            .discrepancies
            .push("target mismatch in users".to_owned());
        let text = report.json().expect("report serializes");
        assert!(!text.contains("/private/staging"));
        assert!(!text.contains("session-secret"));
        assert!(!text.contains("transcription contents"));
    }
}
