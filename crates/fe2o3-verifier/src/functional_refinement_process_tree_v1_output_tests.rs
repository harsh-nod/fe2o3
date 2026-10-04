use super::*;
use std::io::Cursor;

fn capture(bytes: &[u8]) -> Capture {
    Capture {
        bytes: bytes.to_vec(),
        eof: false,
    }
}

#[test]
fn output_overflow_preserves_exact_limit_and_refusal_before_append() {
    for stream in [OutputStream::Stdout, OutputStream::Stderr] {
        let bytes = b"error: type mismatch";
        let mut accepted = capture(&[]);
        drain(&mut Cursor::new(bytes), &mut accepted, bytes.len(), stream).unwrap();
        assert_eq!(accepted.bytes, bytes);
        assert!(accepted.eof);

        let mut refused = capture(&[]);
        let error = drain(
            &mut Cursor::new(bytes),
            &mut refused,
            bytes.len() - 1,
            stream,
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::OutputTooLarge
        );
        let detail = error.to_string();
        assert!(detail.contains(&format!(
            "stream={} limit=19 retained=0 observed_at_least=20",
            stream.name()
        )));
        assert!(detail.contains("prefix=\"error: type mismatch\""));
        assert!(!detail.contains("(truncated)"));
        assert!(refused.bytes.is_empty());
        assert!(!refused.eof);
    }
}

#[test]
fn output_overflow_prefix_handles_zero_limit_and_invalid_utf8() {
    let mut capture = capture(&[]);
    let detail = drain(
        &mut Cursor::new(b"\xff\0\n\r\t\x1b\\\""),
        &mut capture,
        0,
        OutputStream::Stderr,
    )
    .unwrap_err()
    .to_string();
    assert!(detail.contains("stream=stderr limit=0 retained=0 observed_at_least=8"));
    assert!(detail.contains("prefix=\"\\xff\\x00\\n\\r\\t\\x1b\\\\\\\"\""));
    assert!(detail.is_ascii());
    assert!(!detail.chars().any(char::is_control));
    assert!(capture.bytes.is_empty());
    assert_eq!(bounded_output_prefix(&[], &[]), (String::new(), false));
}

#[test]
fn output_overflow_prefix_is_bounded_and_does_not_split_escapes() {
    for remaining in [3, 4] {
        let retained = vec![b'x'; MAX_ESCAPED_OUTPUT_PREFIX_BYTES - remaining];
        let (prefix, truncated) = bounded_output_prefix(&retained, b"\xffHIDDEN_SUFFIX");
        let expected = if remaining == 4 { "\\xff" } else { "" };
        assert_eq!(prefix, format!("{}{expected}", "x".repeat(retained.len())));
        assert!(truncated);
        assert!(!prefix.contains("HIDDEN_SUFFIX"));
    }
    for length in [
        MAX_ESCAPED_OUTPUT_PREFIX_BYTES,
        MAX_ESCAPED_OUTPUT_PREFIX_BYTES + 1,
    ] {
        let (prefix, truncated) = bounded_output_prefix(&[], &vec![b'x'; length]);
        assert_eq!(prefix.len(), MAX_ESCAPED_OUTPUT_PREFIX_BYTES);
        assert_eq!(truncated, length > MAX_ESCAPED_OUTPUT_PREFIX_BYTES);
    }
}

#[test]
fn output_overflow_prefix_uses_only_the_failing_stream_and_retained_prefix() {
    let mut stdout = capture(b"STDOUT_SECRET");
    let mut stderr = capture(b"error: ");
    let error = drain_to_eof(
        &mut Cursor::new(b""),
        &mut Cursor::new(b"proof type mismatch\n"),
        &mut stdout,
        &mut stderr,
        20,
        Instant::now() + Duration::from_secs(1),
    )
    .unwrap_err();
    let detail = error.to_string();
    assert!(detail.contains("stream=stderr limit=20 retained=7 observed_at_least=27"));
    assert!(detail.contains("prefix=\"error: proof type mismatch\\n\""));
    assert!(!detail.contains("STDOUT_SECRET"));
    assert_eq!(stderr.bytes, b"error: ");
    assert_eq!(stdout.bytes, b"STDOUT_SECRET");
}

#[test]
fn output_overflow_first_chunk_bounds_diagnostic_and_preserves_output_cap() {
    let mut bytes = vec![b'x'; 4096];
    bytes.extend_from_slice(b"UNREAD_SUFFIX");
    let mut reader = Cursor::new(bytes);
    let mut capture = capture(&[]);
    let error = drain(&mut reader, &mut capture, 1, OutputStream::Stdout).unwrap_err();
    let detail = error.to_string();
    assert_eq!(reader.position(), 4096);
    assert!(capture.bytes.is_empty());
    assert!(!capture.eof);
    assert!(detail.contains("stream=stdout limit=1 retained=0 observed_at_least=4096"));
    assert!(detail.ends_with(&format!(
        "prefix=\"{}\" (truncated)",
        "x".repeat(MAX_ESCAPED_OUTPUT_PREFIX_BYTES)
    )));
    assert!(!detail.contains("UNREAD_SUFFIX"));
    assert!(detail.len() <= MAX_ESCAPED_OUTPUT_PREFIX_BYTES + 256);
}
