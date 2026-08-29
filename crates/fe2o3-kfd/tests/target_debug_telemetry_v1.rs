#![cfg(target_os = "linux")]

use std::fs::File;
use std::io::{IoSlice, Read};
use std::os::fd::AsFd;
use std::process::{Command, Stdio};

use fe2o3_kfd::{
    KFD_TARGET_DEBUG_TELEMETRY_WIRE_LEN_V1, KfdCooperativeTargetTelemetryEndpointV1,
    KfdDebuggerTelemetryEndpointV1, KfdTargetDebugAllocationPhaseV1,
    KfdTargetDebugArtifactIdentityV1, KfdTargetDebugArtifactRoleV1,
    KfdTargetDebugDiagnosticSeverityV1, KfdTargetDebugDispatchPhaseV1,
    KfdTargetDebugMemoryAccessV1, KfdTargetDebugMemoryKindV1, KfdTargetDebugSessionNonceV1,
    KfdTargetDebugSessionOutcomeV1, KfdTargetDebugTelemetryDigestV1,
    KfdTargetDebugTelemetryPayloadV1, KfdTargetDebugTelemetryProcessV1,
    KfdTargetDebugTelemetryProtocolErrorV1, KfdTargetDebugTelemetryRecordV1,
    KfdTargetDebugTelemetryTransportErrorV1, create_kfd_target_debug_telemetry_channel_v1,
};
use rustix::net::{
    AddressFamily, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType,
    send, sendmsg, socketpair,
};

fn digest(seed: u8) -> KfdTargetDebugTelemetryDigestV1 {
    KfdTargetDebugTelemetryDigestV1::from_bytes([seed; 32]).unwrap()
}

fn nonce(seed: u8) -> KfdTargetDebugSessionNonceV1 {
    KfdTargetDebugSessionNonceV1::from_bytes([seed; 32]).unwrap()
}

fn process() -> KfdTargetDebugTelemetryProcessV1 {
    KfdTargetDebugTelemetryProcessV1::capture(std::process::id()).unwrap()
}

fn executable(seed: u8) -> KfdTargetDebugArtifactIdentityV1 {
    KfdTargetDebugArtifactIdentityV1::new(digest(seed), 4096).unwrap()
}

fn started() -> KfdTargetDebugTelemetryPayloadV1 {
    KfdTargetDebugTelemetryPayloadV1::SessionStarted {
        process_instance: digest(1),
        executable: executable(2),
    }
}

fn endpoint_pair(
    session_nonce: KfdTargetDebugSessionNonceV1,
) -> (
    KfdDebuggerTelemetryEndpointV1,
    KfdCooperativeTargetTelemetryEndpointV1,
) {
    let (debugger, target) = create_kfd_target_debug_telemetry_channel_v1().unwrap();
    let peer = process();
    (
        KfdDebuggerTelemetryEndpointV1::admit(debugger, session_nonce, peer).unwrap(),
        KfdCooperativeTargetTelemetryEndpointV1::admit(target, session_nonce, peer).unwrap(),
    )
}

fn raw_debugger(
    session_nonce: KfdTargetDebugSessionNonceV1,
) -> (KfdDebuggerTelemetryEndpointV1, std::os::fd::OwnedFd) {
    let (debugger, target) = create_kfd_target_debug_telemetry_channel_v1().unwrap();
    (
        KfdDebuggerTelemetryEndpointV1::admit(debugger, session_nonce, process()).unwrap(),
        target,
    )
}

fn send_raw(endpoint: &std::os::fd::OwnedFd, bytes: &[u8]) {
    assert_eq!(
        send(endpoint, bytes, SendFlags::NOSIGNAL).unwrap(),
        bytes.len()
    );
}

#[test]
fn full_declared_session_round_trips_with_strict_lifecycle() {
    let session_nonce = nonce(7);
    let (mut debugger, mut target) = endpoint_pair(session_nonce);
    assert!(debugger.try_receive().unwrap().is_none());

    let payloads = [
        started(),
        KfdTargetDebugTelemetryPayloadV1::Artifact {
            role: KfdTargetDebugArtifactRoleV1::CodeObject,
            ordinal: 0,
            artifact: KfdTargetDebugArtifactIdentityV1::new(digest(3), 8192).unwrap(),
        },
        KfdTargetDebugTelemetryPayloadV1::Dispatch {
            phase: KfdTargetDebugDispatchPhaseV1::Submitted,
            dispatch: digest(4),
            kernel: digest(5),
            code_object: digest(3),
            logical_queue: digest(6),
            grid: [1024, 4, 1],
            workgroup: [256, 1, 1],
            dynamic_shared_memory_bytes: 2048,
        },
        KfdTargetDebugTelemetryPayloadV1::Allocation {
            phase: KfdTargetDebugAllocationPhaseV1::Created,
            memory_kind: KfdTargetDebugMemoryKindV1::KernelArguments,
            access: KfdTargetDebugMemoryAccessV1::ReadOnly,
            allocation: digest(8),
            logical_scope: digest(4),
            byte_length: 256,
            alignment: 64,
        },
        KfdTargetDebugTelemetryPayloadV1::Diagnostic {
            severity: KfdTargetDebugDiagnosticSeverityV1::Warning,
            stable_code: 17,
            diagnostic: digest(9),
            logical_scope: digest(4),
        },
        KfdTargetDebugTelemetryPayloadV1::SessionEnded {
            outcome: KfdTargetDebugSessionOutcomeV1::Completed,
        },
    ];

    for (sequence, payload) in payloads.into_iter().enumerate() {
        let sent = target.send(payload.clone()).unwrap();
        assert_eq!(sent.sequence(), sequence as u64);
        let received = debugger.receive().unwrap();
        assert_eq!(received, sent);
        assert_eq!(received.payload(), &payload);
    }
    assert!(target.is_finished());
    assert!(debugger.is_finished());
    assert!(matches!(
        target.send(KfdTargetDebugTelemetryPayloadV1::Diagnostic {
            severity: KfdTargetDebugDiagnosticSeverityV1::Note,
            stable_code: 1,
            diagnostic: digest(10),
            logical_scope: digest(4),
        }),
        Err(KfdTargetDebugTelemetryTransportErrorV1::SessionFinished)
    ));
    assert!(matches!(
        debugger.try_receive(),
        Err(KfdTargetDebugTelemetryTransportErrorV1::SessionFinished)
    ));
}

#[test]
fn canonical_serialization_has_zero_padding_and_no_string_or_native_fields() {
    let secret = b"/home/private/kernel.hsaco";
    let record = KfdTargetDebugTelemetryRecordV1::new(
        0,
        nonce(11),
        KfdTargetDebugTelemetryPayloadV1::Artifact {
            role: KfdTargetDebugArtifactRoleV1::CodeObject,
            ordinal: 3,
            artifact: KfdTargetDebugArtifactIdentityV1::new(digest(12), 1234).unwrap(),
        },
    )
    .unwrap();
    let bytes = record.to_wire_bytes();
    assert_eq!(bytes.len(), KFD_TARGET_DEBUG_TELEMETRY_WIRE_LEN_V1);
    assert!(!bytes.windows(secret.len()).any(|window| window == secret));
    assert!(bytes[56 + 48..224].iter().all(|byte| *byte == 0));
    assert_eq!(
        KfdTargetDebugTelemetryRecordV1::from_wire_bytes(&bytes).unwrap(),
        record
    );
    let rendered = format!("{record:?}");
    assert!(!rendered.contains("/home"));
    assert!(!rendered.contains("0x"));
}

#[test]
fn decoder_rejects_wrong_length_and_version_before_interpretation() {
    let record = KfdTargetDebugTelemetryRecordV1::new(0, nonce(13), started()).unwrap();
    let mut bytes = record.to_wire_bytes();
    assert!(matches!(
        KfdTargetDebugTelemetryRecordV1::from_wire_bytes(&bytes[..255]),
        Err(KfdTargetDebugTelemetryProtocolErrorV1::InvalidWireLength {
            expected: 256,
            actual: 255
        })
    ));
    bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        KfdTargetDebugTelemetryRecordV1::from_wire_bytes(&bytes),
        Err(KfdTargetDebugTelemetryProtocolErrorV1::UnsupportedVersion(
            2
        ))
    ));
}

#[test]
fn receiver_rejects_short_and_oversized_packets_and_then_poisoned_state() {
    let session_nonce = nonce(14);
    let (mut debugger, target) = raw_debugger(session_nonce);
    send_raw(&target, &[0_u8; 12]);
    assert!(matches!(
        debugger.receive(),
        Err(
            KfdTargetDebugTelemetryTransportErrorV1::InvalidPacketLength {
                expected: 256,
                actual: 12
            }
        )
    ));
    assert!(matches!(
        debugger.receive(),
        Err(KfdTargetDebugTelemetryTransportErrorV1::Poisoned)
    ));

    let (mut debugger, target) = raw_debugger(session_nonce);
    send_raw(&target, &[0_u8; 257]);
    assert!(matches!(
        debugger.receive(),
        Err(KfdTargetDebugTelemetryTransportErrorV1::PacketTruncated)
    ));
}

#[test]
fn receiver_rejects_nonce_and_sequence_substitution() {
    let expected_nonce = nonce(15);
    let (mut debugger, target) = raw_debugger(expected_nonce);
    let substituted = KfdTargetDebugTelemetryRecordV1::new(0, nonce(16), started())
        .unwrap()
        .to_wire_bytes();
    send_raw(&target, &substituted);
    assert!(matches!(
        debugger.receive(),
        Err(KfdTargetDebugTelemetryTransportErrorV1::NonceMismatch)
    ));

    let (mut debugger, target) = raw_debugger(expected_nonce);
    let skipped = KfdTargetDebugTelemetryRecordV1::new(1, expected_nonce, started())
        .unwrap()
        .to_wire_bytes();
    send_raw(&target, &skipped);
    assert!(matches!(
        debugger.receive(),
        Err(KfdTargetDebugTelemetryTransportErrorV1::SequenceMismatch {
            expected: 0,
            actual: 1
        })
    ));
}

#[test]
fn receiver_rejects_missing_start_and_duplicate_sequence() {
    let session_nonce = nonce(17);
    let (mut debugger, target) = raw_debugger(session_nonce);
    let artifact = KfdTargetDebugTelemetryRecordV1::new(
        0,
        session_nonce,
        KfdTargetDebugTelemetryPayloadV1::Artifact {
            role: KfdTargetDebugArtifactRoleV1::KernelIr,
            ordinal: 0,
            artifact: executable(18),
        },
    )
    .unwrap()
    .to_wire_bytes();
    send_raw(&target, &artifact);
    assert!(matches!(
        debugger.receive(),
        Err(KfdTargetDebugTelemetryTransportErrorV1::Protocol(
            KfdTargetDebugTelemetryProtocolErrorV1::MissingSessionStart
        ))
    ));

    let (mut debugger, target) = raw_debugger(session_nonce);
    let start = KfdTargetDebugTelemetryRecordV1::new(0, session_nonce, started())
        .unwrap()
        .to_wire_bytes();
    send_raw(&target, &start);
    debugger.receive().unwrap();
    send_raw(&target, &start);
    assert!(matches!(
        debugger.receive(),
        Err(KfdTargetDebugTelemetryTransportErrorV1::SequenceMismatch {
            expected: 1,
            actual: 0
        })
    ));
}

#[test]
fn premature_eof_is_distinct_from_a_clean_session_end() {
    let session_nonce = nonce(19);
    let (mut debugger, target) = raw_debugger(session_nonce);
    drop(target);
    assert!(matches!(
        debugger.receive(),
        Err(KfdTargetDebugTelemetryTransportErrorV1::UnexpectedEof)
    ));
}

#[test]
fn receiver_rejects_file_descriptor_ancillary_data() {
    let session_nonce = nonce(20);
    let (mut debugger, target) = raw_debugger(session_nonce);
    let bytes = KfdTargetDebugTelemetryRecordV1::new(0, session_nonce, started())
        .unwrap()
        .to_wire_bytes();
    let file = File::open("/dev/null").unwrap();
    let descriptors = [file.as_fd()];
    let mut control_storage = [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut control = SendAncillaryBuffer::new(&mut control_storage);
    assert!(control.push(SendAncillaryMessage::ScmRights(&descriptors)));
    assert_eq!(
        sendmsg(
            &target,
            &[IoSlice::new(&bytes)],
            &mut control,
            SendFlags::NOSIGNAL
        )
        .unwrap(),
        bytes.len()
    );
    assert!(matches!(
        debugger.receive(),
        Err(KfdTargetDebugTelemetryTransportErrorV1::ForbiddenAncillaryData)
    ));
}

#[test]
fn receiver_binds_kernel_credentials_to_the_captured_process_instance() {
    let mut other = Command::new("sh")
        .args(["-c", "exec sleep 30"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let other_process = KfdTargetDebugTelemetryProcessV1::capture(other.id()).unwrap();
    let session_nonce = nonce(21);
    let (debugger_fd, target) = create_kfd_target_debug_telemetry_channel_v1().unwrap();
    let mut debugger =
        KfdDebuggerTelemetryEndpointV1::admit(debugger_fd, session_nonce, other_process).unwrap();
    let bytes = KfdTargetDebugTelemetryRecordV1::new(0, session_nonce, started())
        .unwrap()
        .to_wire_bytes();
    send_raw(&target, &bytes);
    assert!(matches!(
        debugger.receive(),
        Err(KfdTargetDebugTelemetryTransportErrorV1::PeerCredentialMismatch)
    ));
    other.kill().unwrap();
    other.wait().unwrap();
}

#[test]
fn endpoint_admission_rejects_non_seqpacket_descriptors() {
    let (debugger, _target) = socketpair(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    assert!(matches!(
        KfdDebuggerTelemetryEndpointV1::admit(debugger, nonce(22), process()),
        Err(KfdTargetDebugTelemetryTransportErrorV1::WrongSocketType)
    ));
}

#[test]
fn proc_stat_parser_handles_the_current_process_identity_stably() {
    let captured = process();
    assert_eq!(captured.pid(), std::process::id());
    assert_ne!(captured.start_time_ticks(), 0);

    let mut status = String::new();
    File::open("/proc/self/status")
        .unwrap()
        .read_to_string(&mut status)
        .unwrap();
    assert!(status.contains("Uid:"));
    assert!(status.contains("Gid:"));
}
