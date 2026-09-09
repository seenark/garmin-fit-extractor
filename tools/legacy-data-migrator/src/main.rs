use std::{path::PathBuf, str::FromStr};

use clap::{Parser, Subcommand};
use legacy_data_migrator::{
    SafeError,
    error::MigratorError,
    report::{Report, write_report},
    source::{SourceKind, read_snapshot, write_snapshot},
    target,
};

#[derive(Debug, Parser)]
#[command(
    name = "legacy-data-migrator",
    about = "Safely migrate stopped SQLite snapshots into PostgreSQL"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Snapshot {
        #[arg(long, value_name = "garmin|intake")]
        source_name: String,
        #[arg(long, value_name = "SQLITE_FILE")]
        input: PathBuf,
        #[arg(long, value_name = "SQLITE_FILE")]
        output: PathBuf,
        #[arg(long, value_name = "JSON_FILE")]
        report: Option<PathBuf>,
    },
    PrepareTarget {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long, value_name = "JSON_FILE")]
        report: Option<PathBuf>,
    },
    Import {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long, value_name = "SQLITE_FILE")]
        garmin_snapshot: PathBuf,
        #[arg(long, value_name = "SQLITE_FILE")]
        intake_snapshot: PathBuf,
        #[arg(long)]
        apply: bool,
        #[arg(long, value_name = "JSON_FILE")]
        report: Option<PathBuf>,
    },
    Verify {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long, value_name = "SQLITE_FILE")]
        garmin_snapshot: PathBuf,
        #[arg(long, value_name = "SQLITE_FILE")]
        intake_snapshot: PathBuf,
        #[arg(long, value_name = "JSON_FILE")]
        report: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("{}", SafeError(&error));
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> legacy_data_migrator::Result<()> {
    match cli.command {
        Command::Snapshot {
            source_name,
            input,
            output,
            report: report_path,
        } => {
            let kind = SourceKind::from_str(&source_name)?;
            let snapshot = write_snapshot(&input, &output, kind).await?;
            let mut report = Report::new("snapshot", false);
            report.add_source(&snapshot);
            write_report(report_path.as_deref(), &report)?;
            print_report(report);
        }
        Command::PrepareTarget {
            database_url,
            report: report_path,
        } => {
            let report = target::prepare_target(&database_url).await?;
            write_report(report_path.as_deref(), &report)?;
            print_report(report);
        }
        Command::Import {
            database_url,
            garmin_snapshot,
            intake_snapshot,
            apply,
            report: report_path,
        } => {
            target::ensure_postgres_url(&database_url)?;
            let garmin = read_snapshot(&garmin_snapshot, SourceKind::Garmin).await?;
            let intake = read_snapshot(&intake_snapshot, SourceKind::Intake).await?;
            let (
                legacy_data_migrator::source::Snapshot::Garmin(garmin),
                legacy_data_migrator::source::Snapshot::Intake(intake),
            ) = (garmin, intake)
            else {
                return Err(MigratorError::MigrationFailed);
            };
            let report = target::import(&database_url, &garmin, &intake, apply).await?;
            write_report(report_path.as_deref(), &report)?;
            print_report(report);
        }
        Command::Verify {
            database_url,
            garmin_snapshot,
            intake_snapshot,
            report: report_path,
        } => {
            target::ensure_postgres_url(&database_url)?;
            let garmin = read_snapshot(&garmin_snapshot, SourceKind::Garmin).await?;
            let intake = read_snapshot(&intake_snapshot, SourceKind::Intake).await?;
            let (
                legacy_data_migrator::source::Snapshot::Garmin(garmin),
                legacy_data_migrator::source::Snapshot::Intake(intake),
            ) = (garmin, intake)
            else {
                return Err(MigratorError::MigrationFailed);
            };
            let report = target::verify(&database_url, &garmin, &intake).await?;
            write_report(report_path.as_deref(), &report)?;
            print_report(report);
        }
    }
    Ok(())
}

fn print_report(report: Report) {
    println!("{}", report.human());
}
