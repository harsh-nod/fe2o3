use std::os::unix::process::ExitStatusExt as _;

fn captured_v84(bytes: &[u8]) -> File {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(bytes).unwrap();
    file
}

#[test]
fn provisioning_capture_requires_both_success_streams_silent() {
    let success = std::process::ExitStatus::from_raw(0);
    finish_provisioning_capture_v84(success, captured_v84(b""), captured_v84(b"")).unwrap();
    for (stdout, stderr) in [
        (b"unexpected".as_slice(), b"".as_slice()),
        (b"", b"unexpected"),
    ] {
        let error =
            finish_provisioning_capture_v84(success, captured_v84(stdout), captured_v84(stderr))
                .unwrap_err();
        assert_eq!(
            error.kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationProvisioning
        );
        assert!(error.to_string().contains("emitted unexpected"));
    }
}

#[test]
fn provisioning_capture_escapes_failure_and_keeps_nonzero_status() {
    let error = finish_provisioning_capture_v84(
        std::process::ExitStatus::from_raw(7 << 8),
        captured_v84(b"systemd 255 (255.4-1ubuntu8.17)\n"),
        captured_v84(b"bad\n\r\0\xff\"\\record"),
    )
    .unwrap_err();
    let message = error.to_string();
    assert_eq!(
        error.kind(),
        DeploymentVerificationErrorKindV1::InvalidQualificationProvisioning
    );
    assert!(message.contains("exit_code=Some(7)"));
    assert!(message.contains("stderr=\"bad\\n\\r\\x00\\xff\\\"\\\\record\""));
    assert!(!message.contains('\n') && !message.contains('\r') && message.is_ascii());
    assert!(!message.starts_with("systemd 255 "));
}

#[test]
fn provisioning_capture_bounds_escaped_prefix_without_full_read() {
    let file = captured_v84(&[0xff; 4096]);
    let error = finish_provisioning_capture_v84(
        std::process::ExitStatus::from_raw(1 << 8),
        file,
        captured_v84(b""),
    )
    .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("[truncated]"));
    assert!(message.len() < 2304 && message.is_ascii());
    assert!(!message.contains('\n'));
}

#[test]
fn provisioning_capture_refuses_combined_output_before_reading() {
    for status in [0, 1 << 8] {
        for (stdout_bytes, stderr_bytes) in [(65536, 1), (1, 65536), (65537, 0)] {
            let stdout = captured_v84(b"");
            let stderr = captured_v84(b"");
            stdout.set_len(stdout_bytes).unwrap();
            stderr.set_len(stderr_bytes).unwrap();
            let error = finish_provisioning_capture_v84(
                std::process::ExitStatus::from_raw(status),
                stdout,
                stderr,
            )
            .unwrap_err();
            assert_eq!(
                error.kind(),
                DeploymentVerificationErrorKindV1::InvalidQualificationProvisioning
            );
            assert!(
                error
                    .to_string()
                    .contains("combined production provisioner output exceeds")
            );
        }
    }
}

#[test]
fn provisioning_pid1_diagnostic_is_bounded_failure_only() {
    let error = provisioning_invalid("\n\r\0\"\\".repeat(4096));
    let diagnostic = compiler_execution_provisioning_pid1_error_v84(&error);
    assert!(
        diagnostic.starts_with("FE2O3_PROVISIONING_PID1_ERROR stage=\"provisioning-v1\" cause=\"")
    );
    assert!(diagnostic.ends_with("[truncated]\"\n"));
    assert_eq!(diagnostic.bytes().filter(|byte| *byte == b'\n').count(), 1);
    assert!(diagnostic.is_ascii() && diagnostic.len() < 1024);
    assert!(!diagnostic.contains('\r') && !diagnostic.contains('\0'));
}

#[test]
#[ignore = "requires a reviewed private root namespace, actual static qualification tool and static BusyBox fixture"]
fn qualification_provisioning_pid1_proc_and_parent_death_are_isolated() {
    let tool = std::env::var_os("FE2O3_QUALIFICATION_TEST_BINARY").expect("actual static tool");
    let busybox = std::env::var_os("FE2O3_BUSYBOX_TEST_BINARY").expect("static test BusyBox");
    let script = std::env::var_os("FE2O3_PROVISIONING_NAMESPACE_TEST_SCRIPT").expect("test script");
    assert_eq!(
        std::fs::read(&script).unwrap(),
        include_bytes!("provisioning_namespace_v84_probe.py")
    );
    let output = Command::new("/usr/bin/python3")
        .args(["-I", "-B"])
        .arg(script)
        .arg(tool)
        .arg(busybox)
        .output()
        .unwrap();
    assert!(output.stdout.len() <= 65536 && output.stderr.len() <= 65536);
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    print!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| *line == "qualification_provisioning_namespace_v84: PASS")
            .count(),
        1
    );
}
