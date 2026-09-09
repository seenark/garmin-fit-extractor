use std::fmt;

use thiserror::Error;

pub type Result<T, E = MigratorError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum MigratorError {
    #[error("source {source_name} failed during {operation}")]
    SourceDatabase {
        source_name: String,
        operation: &'static str,
        #[source]
        cause: sqlx::Error,
    },
    #[error("source {source_name} is invalid: {reason}")]
    InvalidSource { source_name: String, reason: String },
    #[error("source snapshot operation failed")]
    SnapshotIo {
        #[source]
        cause: std::io::Error,
    },
    #[error("source snapshot operation failed")]
    SnapshotDatabase {
        #[source]
        cause: sqlx::Error,
    },
    #[error("SQLite backup operation failed during {operation}")]
    SnapshotBackup { operation: &'static str },
    #[error("target database URL must use PostgreSQL")]
    InvalidDatabaseUrl,
    #[error("target database operation failed during {operation}")]
    TargetDatabase {
        operation: &'static str,
        #[source]
        cause: sqlx::Error,
    },
    #[error("target schema is missing or incompatible: {table}")]
    TargetSchema { table: String },
    #[error("target data mismatch in {table}: {reason}")]
    TargetMismatch { table: String, reason: &'static str },
    #[error("source ledger entry for {source_name} does not match this snapshot")]
    LedgerMismatch { source_name: String },
    #[error("source ledger manifest for {source_name} is invalid")]
    LedgerManifest { source_name: String },
    #[error("target transaction aborted")]
    TransactionAborted,
    #[error("report output failed")]
    ReportIo {
        #[source]
        cause: std::io::Error,
    },
    #[error("report serialization failed")]
    ReportSerialization {
        #[source]
        cause: serde_json::Error,
    },
    #[error("invalid command option: {0}")]
    InvalidOption(String),
    #[error("migration failed")]
    MigrationFailed,
}

impl MigratorError {
    pub fn source_database(
        source_name: impl Into<String>,
        operation: &'static str,
        cause: sqlx::Error,
    ) -> Self {
        Self::SourceDatabase {
            source_name: source_name.into(),
            operation,
            cause,
        }
    }

    pub fn target_database(operation: &'static str, cause: sqlx::Error) -> Self {
        Self::TargetDatabase { operation, cause }
    }

    pub fn invalid_source(source_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidSource {
            source_name: source_name.into(),
            reason: reason.into(),
        }
    }
}

/// A display adapter for diagnostics that never includes a database URL, path, SQL text,
/// or row content. The underlying error is retained for `Debug`/operator logs only.
pub struct SafeError<'a>(pub &'a MigratorError);

impl fmt::Display for SafeError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            MigratorError::SourceDatabase {
                source_name,
                operation,
                ..
            } => write!(formatter, "source {source_name} failed during {operation}"),
            MigratorError::InvalidSource {
                source_name,
                reason,
            } => write!(formatter, "source {source_name} is invalid: {reason}"),
            MigratorError::SnapshotIo { .. }
            | MigratorError::SnapshotDatabase { .. }
            | MigratorError::SnapshotBackup { .. } => {
                formatter.write_str("source snapshot operation failed")
            }
            MigratorError::TargetDatabase { operation, .. } => {
                write!(
                    formatter,
                    "target database operation failed during {operation}"
                )
            }
            other => other.fmt(formatter),
        }
    }
}
