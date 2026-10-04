// Kept outside src/bin so Cargo does not discover a second worker binary.
use super::{WorkerMode, parse_args};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn legacy_worker_arguments_keep_the_default_wait_policy() {
    assert_eq!(
        parse_args(&args(&[
            "--device-unique-id",
            "7",
            "--allow-unauthenticated-machine-code"
        ])),
        Some((7, WorkerMode::Default))
    );
}

#[test]
fn active_polling_requires_the_exact_explicit_fourth_argument() {
    assert_eq!(
        parse_args(&args(&[
            "--device-unique-id",
            "7",
            "--allow-unauthenticated-machine-code",
            "--diagnostic-active-poll-10ms"
        ])),
        Some((7, WorkerMode::ActivePoll))
    );
}

#[test]
fn unknown_duplicate_and_reordered_flags_fail_closed() {
    for tail in [
        "--active-poll",
        "--diagnostic-active-poll-1ms",
        "--diagnostic-active-poll-10ms=1",
    ] {
        assert!(
            parse_args(&args(&[
                "--device-unique-id",
                "7",
                "--allow-unauthenticated-machine-code",
                tail
            ]))
            .is_none()
        );
    }
    assert!(
        parse_args(&args(&[
            "--device-unique-id",
            "7",
            "--allow-unauthenticated-machine-code",
            "--diagnostic-active-poll-10ms",
            "--diagnostic-active-poll-10ms"
        ]))
        .is_none()
    );
    assert!(
        parse_args(&args(&[
            "--device-unique-id",
            "7",
            "--diagnostic-active-poll-10ms",
            "--allow-unauthenticated-machine-code"
        ]))
        .is_none()
    );
}

#[test]
fn opt_in_does_not_remove_device_or_machine_code_acknowledgement() {
    for invalid in [
        args(&["--device-unique-id", "7", "--diagnostic-active-poll-10ms"]),
        args(&[
            "--device-unique-id",
            "not-a-number",
            "--allow-unauthenticated-machine-code",
            "--diagnostic-active-poll-10ms",
        ]),
        args(&[
            "--device-index",
            "7",
            "--allow-unauthenticated-machine-code",
            "--diagnostic-active-poll-10ms",
        ]),
    ] {
        assert!(parse_args(&invalid).is_none());
    }
}

#[test]
fn token_program_requires_its_exact_diagnostic_flag() {
    assert_eq!(
        parse_args(&args(&[
            "--device-unique-id",
            "7",
            "--allow-unauthenticated-machine-code",
            "--diagnostic-token-program-v1"
        ])),
        Some((7, WorkerMode::TokenProgram))
    );
    for flag in [
        "--token-program",
        "--diagnostic-token-program-v2",
        "--diagnostic-token-program-v1=1",
    ] {
        assert!(
            parse_args(&args(&[
                "--device-unique-id",
                "7",
                "--allow-unauthenticated-machine-code",
                flag
            ]))
            .is_none()
        );
    }
}

#[test]
fn token_program_cannot_be_combined_with_active_poll_or_missing_acknowledgement() {
    for flags in [
        args(&[
            "--device-unique-id",
            "7",
            "--allow-unauthenticated-machine-code",
            "--diagnostic-token-program-v1",
            "--diagnostic-active-poll-10ms",
        ]),
        args(&[
            "--device-unique-id",
            "7",
            "--allow-unauthenticated-machine-code",
            "--diagnostic-token-program-v1",
            "--diagnostic-token-program-v1",
        ]),
        args(&["--device-unique-id", "7", "--diagnostic-token-program-v1"]),
    ] {
        assert!(parse_args(&flags).is_none());
    }
}
