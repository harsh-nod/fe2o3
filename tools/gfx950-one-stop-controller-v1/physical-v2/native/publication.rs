//! Bounded awaited raw observation output. Only the separate family validator
//! may promote it after cleanup-only ACK and current manager-generation joins.
use super::{clock::Clock, wire::NativePeer};
use fe2o3_private_one_stop_protocol::{Cleanup, ProtocolObservation, Refusal};
use std::io::{self, Write};
const CAP: usize = 18 * 1024 * 1024;
pub(super) struct Buffer {
    bytes: Vec<u8>,
}
impl Buffer {
    pub(super) fn reserve() -> Result<Self, Refusal> {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(CAP).map_err(|_| Refusal::Bound)?;
        Ok(Self { bytes })
    }
}
impl Write for Buffer {
    fn write(&mut self, v: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(v.len())
            .is_none_or(|n| n > CAP)
            || self.bytes.capacity() - self.bytes.len() < v.len()
        {
            return Err(io::Error::other("one-stop raw report cap"));
        }
        self.bytes.extend_from_slice(v);
        Ok(v.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn publish(
    mut buffer: Buffer,
    peer: &NativePeer<'_>,
    result: &Result<ProtocolObservation, (Refusal, Cleanup)>,
    clock: Clock,
) -> Result<(), Refusal> {
    clock.check()?;
    buffer
        .write_all(b"{\"schema\":\"fe2o3-one-stop-native-peer-observation-v2\",\"result\":")
        .map_err(|_| Refusal::Incomplete)?;
    let result_value = match result {
        Ok(value) => serde_json::json!({"status":"observed","observation":value}),
        Err((reason, cleanup)) => {
            serde_json::json!({"status":"refused","reason":format!("{reason:?}"),"cleanup":cleanup})
        }
    };
    serde_json::to_writer(&mut buffer, &result_value).map_err(|_| Refusal::Bound)?;
    buffer
        .write_all(b",\"identity\":")
        .map_err(|_| Refusal::Incomplete)?;
    serde_json::to_writer(&mut buffer, &peer.identity()).map_err(|_| Refusal::Bound)?;
    buffer
        .write_all(b",\"transcript\":")
        .map_err(|_| Refusal::Incomplete)?;
    peer.encode_transcript(&mut buffer)?;
    buffer.write_all(b",\"accepted_by_family\":false,\"native_tuple_independently_replayed\":false,\"general_host_exclusion_proved\":false,\"whole_family_cleanup_proved\":false,\"operational_qualification\":false}\n")
  .map_err(|_|Refusal::Incomplete)?;
    clock.check()?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&buffer.bytes)
        .map_err(|_| Refusal::Incomplete)?;
    stdout.flush().map_err(|_| Refusal::Incomplete)?;
    // A full stdout prefix is not acceptance. Parent MUST require exit 0, timely
    // complete stream, ACK, and current family cleanup; quarantine on any failure.
    clock.check()
}
// Publication failure must not erase the original refusal when the shared
// clock has expired. This is bounded retained evidence only, never a new read,
// protocol transition, cleanup operation, accepted observation, or authority.
const FAILURE_DIAGNOSTIC_CAP: usize = 4096;
const FAILURE_SUFFIX_CAP: usize = 256;

pub(super) struct RetainedTransport<'a> {
    pub sent_consumed: u64,
    pub records_after_cleanup: usize,
    pub eof_after_cleanup: [bool; 2],
    pub closed_after_cleanup: bool,
    pub stdout: &'a [u8],
    pub stderr: &'a [u8],
    pub commands: &'a [u8],
}

pub(super) struct PublicationDiagnostic {
    bytes: [u8; FAILURE_DIAGNOSTIC_CAP],
    used: usize,
}
impl std::fmt::Write for PublicationDiagnostic {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self.used.checked_add(text.len()).ok_or(std::fmt::Error)?;
        if end > self.bytes.len() {
            return Err(std::fmt::Error);
        }
        self.bytes[self.used..end].copy_from_slice(text.as_bytes());
        self.used = end;
        Ok(())
    }
}
impl std::fmt::Display for PublicationDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(std::str::from_utf8(&self.bytes[..self.used]).unwrap_or(
            "one-stop publication diagnostic unavailable: invalid encoding; authority=false",
        ))
    }
}
#[derive(Clone, Copy)]
enum RetainedOutcome {
    ObservedButUnpublished,
    Refused(Refusal),
}
impl RetainedOutcome {
    fn from_result<T>(result: &Result<T, (Refusal, Cleanup)>) -> Self {
        match result {
            Ok(_) => Self::ObservedButUnpublished,
            Err((reason, _)) => Self::Refused(*reason),
        }
    }
}
impl std::fmt::Display for RetainedOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ObservedButUnpublished => f.write_str("observed_but_publication_failed"),
            Self::Refused(reason) => write!(f, "refused({reason:?})"),
        }
    }
}
fn suffix_start(len: usize) -> usize {
    len.saturating_sub(FAILURE_SUFFIX_CAP)
}
struct RetainedSuffix<'a>(&'a [u8]);
impl std::fmt::Display for RetainedSuffix<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let suffix = &self.0[suffix_start(self.0.len())..];
        write!(
            f,
            "total_bytes={},suffix_bytes={},truncated={},suffix_hex=",
            self.0.len(),
            suffix.len(),
            suffix.len() != self.0.len(),
        )?;
        for byte in suffix {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
fn encode_failure(
    out: &mut impl std::fmt::Write,
    publication_refusal: Refusal,
    outcome: RetainedOutcome,
    cleanup: Cleanup,
    retained: &RetainedTransport<'_>,
) -> std::fmt::Result {
    write!(
        out,
        "one-stop raw observation publication refused: {publication_refusal:?}; \
         retained cleanup={cleanup:?}; original_protocol_result={outcome}; \
         transport_after_cleanup={{sent_consumed={},records={},eof={:?},closed={}}}; \
         retained_stdout={{{}}}; retained_stderr={{{}}}; retained_commands={{{}}}; \
         suffixes_are_unparsed_retained_bytes=true; command_consumed_is_not_completion=true; \
         diagnostic_only=true; accepted=false; authority=false",
        retained.sent_consumed,
        retained.records_after_cleanup,
        retained.eof_after_cleanup,
        retained.closed_after_cleanup,
        RetainedSuffix(retained.stdout),
        RetainedSuffix(retained.stderr),
        RetainedSuffix(retained.commands),
    )
}
pub(super) fn failure_diagnostic<T>(
    publication_refusal: Refusal,
    result: &Result<T, (Refusal, Cleanup)>,
    cleanup: Cleanup,
    retained: RetainedTransport<'_>,
) -> PublicationDiagnostic {
    let mut diagnostic = PublicationDiagnostic {
        bytes: [0; FAILURE_DIAGNOSTIC_CAP],
        used: 0,
    };
    if encode_failure(
        &mut diagnostic,
        publication_refusal,
        RetainedOutcome::from_result(result),
        cleanup,
        &retained,
    )
    .is_err()
    {
        // A bounded formatting failure must not escape, allocate, or promote
        // a partial prefix into an observation. Even the fallback is fixed.
        const FALLBACK: &[u8] =
            b"one-stop publication diagnostic unavailable: bound; accepted=false; authority=false";
        diagnostic.bytes[..FALLBACK.len()].copy_from_slice(FALLBACK);
        diagnostic.used = FALLBACK.len();
    }
    diagnostic
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_budget_refuses_before_growth_and_keeps_prior_bytes() {
        let mut b = Buffer::reserve().unwrap();
        b.bytes.resize(CAP - 1, 0);
        b.write_all(b"x").unwrap();
        let old = b.bytes.len();
        assert!(b.write_all(b"y").is_err());
        assert_eq!(b.bytes.len(), old);
    }

    #[cfg(test)]
    mod diagnostic_tests {
        use super::*;
        use std::fmt::Write as _;

        fn context<'a>(
            stdout: &'a [u8],
            stderr: &'a [u8],
            commands: &'a [u8],
        ) -> RetainedTransport<'a> {
            RetainedTransport {
                sent_consumed: 12,
                records_after_cleanup: 37,
                eof_after_cleanup: [true, true],
                closed_after_cleanup: true,
                stdout,
                stderr,
                commands,
            }
        }

        #[test]
        fn late_publication_preserves_distinct_original_refusal() {
            let cleanup = Cleanup {
                debugger_direct_child_reaped: true,
                streams_complete: true,
                reader_threads_joined: true,
                unadmitted_inferior_cleanup_not_proven: true,
                ..Cleanup::default()
            };
            let result = Err::<(), _>((Refusal::Incomplete, cleanup));
            let d = failure_diagnostic(
                Refusal::Deadline,
                &result,
                cleanup,
                context(b"waiting\n", b"", b"12-exec-run\n"),
            );
            let text = d.to_string();
            assert!(text.contains("publication refused: Deadline"));
            assert!(text.contains("original_protocol_result=refused(Incomplete)"));
            assert!(text.contains("unadmitted_inferior_cleanup_not_proven: true"));
            assert!(text.contains("sent_consumed=12,records=37"));
            assert!(text.ends_with("diagnostic_only=true; accepted=false; authority=false"));
        }

        #[test]
        fn successful_protocol_value_is_never_formatted_or_promoted() {
            // This value intentionally has no Debug or Display implementation.
            // The diagnostic must never serialize an unbounded protocol payload.
            struct Opaque;
            let d = failure_diagnostic(
                Refusal::Deadline,
                &Ok::<_, (Refusal, Cleanup)>(Opaque),
                Cleanup::default(),
                context(b"", b"", b""),
            );
            let text = d.to_string();
            assert!(text.contains("original_protocol_result=observed_but_publication_failed"));
            assert!(text.contains("accepted=false; authority=false"));
            assert!(!text.contains("accepted=true"));
        }

        #[test]
        fn every_original_refusal_survives_without_reclassification() {
            for refusal in [
                Refusal::Bound,
                Refusal::Syntax,
                Refusal::Shape,
                Refusal::Token,
                Refusal::Process,
                Refusal::Artifact,
                Refusal::Duplicate,
                Refusal::State,
                Refusal::Stop,
                Refusal::Changed,
                Refusal::Exit,
                Refusal::Incomplete,
                Refusal::Deadline,
            ] {
                let result = Err::<(), _>((refusal, Cleanup::default()));
                let text = failure_diagnostic(
                    Refusal::Incomplete,
                    &result,
                    Cleanup::default(),
                    context(b"", b"", b""),
                )
                .to_string();
                assert!(text.contains(&format!("original_protocol_result=refused({refusal:?})")));
            }
        }

        #[test]
        fn suffix_boundaries_are_exact_and_keep_only_the_tail() {
            for n in [0, 1, 255, 256, 257, 1024] {
                let bytes: Vec<_> = (0..n).map(|i| (i % 251) as u8).collect();
                let text = RetainedSuffix(&bytes).to_string();
                let count = n.min(FAILURE_SUFFIX_CAP);
                assert!(text.starts_with(&format!(
                    "total_bytes={n},suffix_bytes={count},truncated={},suffix_hex=",
                    n > FAILURE_SUFFIX_CAP,
                )));
                let (_, hex) = text.split_once("suffix_hex=").unwrap();
                assert_eq!(hex.len(), 2 * count);
                let mut expected = String::new();
                for v in &bytes[n - count..] {
                    write!(&mut expected, "{v:02x}").unwrap();
                }
                assert_eq!(hex, expected);
            }
        }

        #[test]
        fn hostile_bytes_cannot_inject_lines_or_terminal_controls() {
            let all_bytes: Vec<u8> = (0..=255).collect();
            let text = RetainedSuffix(&all_bytes).to_string();
            let (_, hex) = text.split_once("suffix_hex=").unwrap();
            assert_eq!(hex.len(), 512);
            assert!(hex.bytes().all(|b| b.is_ascii_hexdigit()));
            assert!(!text.contains('\n'));
            assert!(!text.contains('\r'));
            assert!(!text.contains('\u{1b}'));
            assert!(!text.contains('\0'));
        }

        #[test]
        fn maximum_counters_and_three_full_samples_fit_fixed_report() {
            let bytes = [255; FAILURE_SUFFIX_CAP + 1];
            let mut retained = context(&bytes, &bytes, &bytes);
            retained.sent_consumed = u64::MAX;
            retained.records_after_cleanup = usize::MAX;
            let result = Err::<(), _>((Refusal::Incomplete, Cleanup::default()));
            let d = failure_diagnostic(Refusal::Incomplete, &result, Cleanup::default(), retained);
            let text = d.to_string();
            assert!(d.used <= FAILURE_DIAGNOSTIC_CAP);
            assert!(text.starts_with("one-stop raw observation publication refused:"));
            assert_eq!(text.matches("suffix_bytes=256,truncated=true").count(), 3);
            assert_eq!(text.matches(&"ff".repeat(256)).count(), 3);
            assert!(!text.contains("unavailable"));
        }

        #[test]
        fn fixed_writer_refuses_growth_and_preserves_prior_bytes() {
            let mut out = PublicationDiagnostic {
                bytes: [0; FAILURE_DIAGNOSTIC_CAP],
                used: 0,
            };
            out.write_str(&"x".repeat(FAILURE_DIAGNOSTIC_CAP - 1))
                .unwrap();
            out.write_str("y").unwrap();
            assert_eq!(out.used, FAILURE_DIAGNOSTIC_CAP);
            let before = out.bytes;
            assert!(out.write_str("z").is_err());
            assert_eq!(out.used, FAILURE_DIAGNOSTIC_CAP);
            assert_eq!(out.bytes, before);
        }

        #[test]
        fn formatter_error_is_propagated_without_retry_or_extra_samples() {
            struct Refuse {
                calls: usize,
            }
            impl std::fmt::Write for Refuse {
                fn write_str(&mut self, _: &str) -> std::fmt::Result {
                    self.calls += 1;
                    Err(std::fmt::Error)
                }
            }
            let mut out = Refuse { calls: 0 };
            assert!(
                encode_failure(
                    &mut out,
                    Refusal::Deadline,
                    RetainedOutcome::Refused(Refusal::State),
                    Cleanup::default(),
                    &context(b"x", b"y", b"z"),
                )
                .is_err()
            );
            assert_eq!(out.calls, 1);
        }

        #[test]
        fn formatting_preserves_all_cleanup_uncertainty_bits() {
            let cleanup = Cleanup {
                owned_inferior_pidfd_exit_observed: false,
                debugger_direct_child_reaped: true,
                streams_complete: false,
                reader_threads_joined: false,
                unadmitted_inferior_cleanup_not_proven: true,
                inferior_reaped_by_controller: false,
                cleanup_deadline_expired: true,
            };
            let text = failure_diagnostic(
                Refusal::Deadline,
                &Err::<(), _>((Refusal::Process, cleanup)),
                cleanup,
                context(b"", b"", b""),
            )
            .to_string();
            assert!(text.contains(&format!("retained cleanup={cleanup:?}")));
            assert!(text.contains("accepted=false; authority=false"));
        }

        #[test]
        fn diagnostic_does_not_mutate_any_retained_buffer_or_result() {
            let stdout = b"retained out\n".to_vec();
            let stderr = b"retained err\n".to_vec();
            let commands = b"12-exec-run\n".to_vec();
            let before = (stdout.clone(), stderr.clone(), commands.clone());
            let result = Err::<(), _>((Refusal::Changed, Cleanup::default()));
            let _ = failure_diagnostic(
                Refusal::Deadline,
                &result,
                Cleanup::default(),
                context(&stdout, &stderr, &commands),
            );
            assert_eq!((stdout, stderr, commands), before);
            assert_eq!(result, Err((Refusal::Changed, Cleanup::default())));
        }

        #[test]
        fn suffix_and_writer_arithmetic_refuse_overflow() {
            for len in [0, 1, 255, 256, 257, usize::MAX - 1, usize::MAX] {
                let start = suffix_start(len);
                assert!(start <= len);
                assert_eq!(len - start, len.min(FAILURE_SUFFIX_CAP));
            }
            let mut out = PublicationDiagnostic {
                bytes: [0; FAILURE_DIAGNOSTIC_CAP],
                used: usize::MAX,
            };
            assert!(out.write_str("x").is_err());
            assert_eq!(out.used, usize::MAX);
            assert_eq!(out.bytes, [0; FAILURE_DIAGNOSTIC_CAP]);
        }

        #[test]
        fn partial_mi_and_command_bytes_are_retained_without_completion_claims() {
            let result = Err::<(), _>((Refusal::Incomplete, Cleanup::default()));
            let text = failure_diagnostic(
                Refusal::Deadline,
                &result,
                Cleanup::default(),
                context(b"13^don", b"partial error", b"13-interpreter-exec"),
            )
            .to_string();
            assert!(
                text.contains(
                    "total_bytes=6,suffix_bytes=6,truncated=false,suffix_hex=31335e646f6e"
                )
            );
            assert!(text.contains("suffixes_are_unparsed_retained_bytes=true"));
            assert!(text.contains("command_consumed_is_not_completion=true"));
            assert!(text.contains("accepted=false; authority=false"));
        }
    }
}
