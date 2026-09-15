#[test]
fn failure_retains_exit_and_both_output_streams_without_reclassifying_the_result() {
    let observed = output(
        1,
        b"verification results:: 4 verified, 1 errors\n",
        b"error: assertion failed\n",
    );
    let error = validate_proved_output(&observed).unwrap_err();
    assert_eq!(
        error.kind(),
        FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult,
    );
    let message = error.to_string();
    assert!(message.contains("exit_code=Some(1), signal=None"));
    assert!(message.contains("stdout_bytes=44, stdout_truncated=false"));
    assert!(message.contains("stdout=\"verification results:: 4 verified, 1 errors\\n\""));
    assert!(message.contains("stderr_bytes=24, stderr_truncated=false"));
    assert!(message.contains("stderr=\"error: assertion failed\\n\""));
    assert_eq!(
        message,
        validate_proved_output(&observed).unwrap_err().to_string()
    );
}

#[test]
fn signal_and_missing_exit_do_not_accept_success_text() {
    for (exit_code, signal) in [(None, None), (None, Some(9)), (Some(0), Some(9))] {
        let mut observed = output(0, b"verification results:: 1 verified, 0 errors\n", b"");
        observed.exit_code = exit_code;
        observed.signal = signal;
        let error = validate_proved_output(&observed).unwrap_err();
        assert_eq!(
            error.kind(),
            FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult
        );
        assert!(
            error
                .to_string()
                .contains(&format!("exit_code={exit_code:?}, signal={signal:?}"))
        );
    }
}

#[test]
fn raw_bytes_and_terminal_controls_are_escaped_without_utf8_decoding() {
    let bytes = b"\x1b[31m\n\r\t\0\xff\"\\";
    let message = validate_proved_output(&output(1, bytes, bytes))
        .unwrap_err()
        .to_string();
    assert!(message.is_ascii());
    assert!(!message.bytes().any(|byte| byte.is_ascii_control()));
    assert_eq!(
        message
            .matches("\\x1b[31m\\n\\r\\t\\x00\\xff\\\"\\\\")
            .count(),
        2
    );
}

#[test]
fn excerpts_report_exact_lengths_and_truncation_at_each_boundary() {
    for length in [
        0,
        1023,
        1024,
        1025,
        MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2,
    ] {
        for stderr in [false, true] {
            let bytes = vec![b'x'; length];
            let observed = if stderr {
                output(1, b"", &bytes)
            } else {
                output(1, &bytes, b"")
            };
            let message = validate_proved_output(&observed).unwrap_err().to_string();
            let stream = if stderr { "stderr" } else { "stdout" };
            assert!(message.contains(&format!(
                "{stream}_bytes={length}, {stream}_truncated={}",
                length > 1024,
            )));
            assert!(message.contains(&format!("{stream}=\"{}\"", "x".repeat(length.min(1024)))));
        }
    }
}

#[test]
fn worst_case_escaping_is_bounded_and_does_not_read_the_omitted_tail() {
    let mut bytes = vec![0xff; MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2];
    let message = validate_proved_output(&output(1, &bytes, &bytes))
        .unwrap_err()
        .to_string();
    assert!(message.len() <= 2 * 4 * 1024 + 256);
    assert_eq!(message.matches("\\xff").count(), 2 * 1024);
    assert_eq!(message.matches("_truncated=true").count(), 2);
    bytes[1024..].fill(0);
    assert_eq!(
        message,
        validate_proved_output(&output(1, &bytes, &bytes))
            .unwrap_err()
            .to_string()
    );
}
