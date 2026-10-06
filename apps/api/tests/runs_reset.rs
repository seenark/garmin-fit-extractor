use std::process::Command;

fn cli(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_runs-reset"))
        .args(arguments)
        .env_clear()
        .env(
            "DATABASE_URL",
            "postgres://secret-user:secret-password@invalid/private-db",
        )
        .output()
        .expect("execute standalone reset CLI")
}

#[test]
fn ambient_database_is_never_authority() {
    let result = cli(&[]);
    assert!(!result.status.success());
    assert_eq!(
        String::from_utf8(result.stderr).unwrap().trim(),
        "runs-reset: MISSING_DATABASE_URL"
    );
    assert!(result.stdout.is_empty());
}

#[test]
fn apply_refuses_wrong_confirmation_before_connecting() {
    let base = [
        "--database-url",
        "postgres://secret-user:secret-password@invalid/private-db",
        "--environment",
        "rehearsal",
        "--database",
        "runs_reset_rehearsal",
        "--pgdata-sha256",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--system-id",
        "123",
        "--owner",
        "00000000-0000-0000-0000-000000000001",
        "--apply",
        "--confirm-environment",
        "production",
    ];
    let result = cli(&base);
    assert!(!result.status.success());
    assert_eq!(
        String::from_utf8(result.stderr).unwrap().trim(),
        "runs-reset: CONFIRM_ENVIRONMENT_MISMATCH"
    );
    assert!(result.stdout.is_empty());
}

#[test]
fn ambiguous_scope_and_unknown_flags_refuse_without_echoing_input() {
    for arguments in [
        vec![
            "--owner",
            "00000000-0000-0000-0000-000000000001",
            "--all-runs",
        ],
        vec!["--private-secret-canary"],
    ] {
        let result = cli(&arguments);
        assert!(!result.status.success());
        let error = String::from_utf8(result.stderr).unwrap();
        assert!(error.contains(if arguments.len() == 1 {
            "UNKNOWN_ARGUMENT"
        } else {
            "AMBIGUOUS_SCOPE"
        }));
        assert!(!error.contains("canary"));
        assert!(!error.contains("secret-password"));
    }
}
