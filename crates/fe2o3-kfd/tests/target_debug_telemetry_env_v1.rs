#![cfg(target_os = "linux")]
#![allow(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::Duration;

use fe2o3_kfd::{
    KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V1, KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V1,
    KFD_TARGET_DEBUG_TELEMETRY_NONCE_ENV_V1, KfdDebuggerTelemetryEndpointV1,
    KfdInheritedTargetDebugTelemetryErrorV1, KfdTargetDebugArtifactIdentityV1,
    KfdTargetDebugSessionNonceV1, KfdTargetDebugSessionOutcomeV1, KfdTargetDebugTelemetryDigestV1,
    KfdTargetDebugTelemetryPayloadV1, KfdTargetDebugTelemetryProcessV1,
    KfdTargetDebugTelemetryTransportErrorV1, admit_inherited_kfd_target_debug_telemetry_v1,
    create_kfd_target_debug_telemetry_channel_v1,
};

const CHILD_CASE_ENV: &str = "FE2O3_KFD_TELEMETRY_TEST_CHILD_CASE";
const NONCE_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";

#[derive(Clone, Debug, Default)]
struct ChildAbiEnvironment {
    descriptor: Option<OsString>,
    nonce: Option<OsString>,
    debugger_pid: Option<OsString>,
}

impl ChildAbiEnvironment {
    fn complete(descriptor: impl Into<OsString>) -> Self {
        Self {
            descriptor: Some(descriptor.into()),
            nonce: Some(NONCE_HEX.into()),
            debugger_pid: Some(std::process::id().to_string().into()),
        }
    }
}

fn command_for_child(
    case: &str,
    environment: &ChildAbiEnvironment,
    inherited: Option<&OwnedFd>,
) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            OsStr::new("--exact"),
            OsStr::new("inherited_environment_child_case"),
            OsStr::new("--nocapture"),
            OsStr::new("--test-threads=1"),
        ])
        .env(CHILD_CASE_ENV, case)
        .env_remove(KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V1)
        .env_remove(KFD_TARGET_DEBUG_TELEMETRY_NONCE_ENV_V1)
        .env_remove(KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V1)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(value) = &environment.descriptor {
        command.env(KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V1, value);
    }
    if let Some(value) = &environment.nonce {
        command.env(KFD_TARGET_DEBUG_TELEMETRY_NONCE_ENV_V1, value);
    }
    if let Some(value) = &environment.debugger_pid {
        command.env(KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V1, value);
    }
    if let Some(inherited) = inherited {
        let raw = inherited.as_raw_fd();
        // SAFETY: the source is owned by the parent through spawn. The child hook changes only
        // the close-on-exec bit on that exact scalar descriptor before exec.
        unsafe {
            command.pre_exec(move || {
                let flags = libc::fcntl(raw, libc::F_GETFD);
                if flags < 0 || libc::fcntl(raw, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    command
}

fn run_child(case: &str, environment: ChildAbiEnvironment, inherited: Option<&OwnedFd>) -> Output {
    let output = command_for_child(case, &environment, inherited)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child case {case} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn spawn_child(
    case: &str,
    environment: &ChildAbiEnvironment,
    inherited: Option<&OwnedFd>,
) -> Child {
    command_for_child(case, environment, inherited)
        .spawn()
        .unwrap()
}

fn assert_child_success(case: &str, child: Child) {
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "child case {case} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn inherited_environment_child_case() {
    let Some(case) = std::env::var_os(CHILD_CASE_ENV) else {
        return;
    };
    let case = case.into_string().unwrap();
    let inherited_raw = std::env::var(KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V1)
        .ok()
        .and_then(|value| value.parse::<i32>().ok());
    let debugger_pid_text = std::env::var(KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V1).ok();
    if case == "positive" {
        // Give the parent time to capture this process instance before the target can exit.
        thread::sleep(Duration::from_millis(200));
    }
    let result = admit_inherited_kfd_target_debug_telemetry_v1();
    match case.as_str() {
        "absent" => assert!(result.unwrap().is_none()),
        "incomplete" => assert!(matches!(
            result,
            Err(KfdInheritedTargetDebugTelemetryErrorV1::IncompleteEnvironment)
        )),
        "invalid_descriptor" => assert!(matches!(
            result,
            Err(KfdInheritedTargetDebugTelemetryErrorV1::InvalidDescriptor)
        )),
        "invalid_nonce" => assert!(matches!(
            result,
            Err(KfdInheritedTargetDebugTelemetryErrorV1::InvalidNonce)
        )),
        "invalid_debugger" => assert!(matches!(
            result,
            Err(KfdInheritedTargetDebugTelemetryErrorV1::InvalidDebuggerProcess)
        )),
        "unavailable_descriptor" => match result {
            Err(KfdInheritedTargetDebugTelemetryErrorV1::DuplicateDescriptor(error)) => {
                assert_eq!(error.raw_os_error(), libc::EBADF)
            }
            _ => panic!("unavailable descriptor was not rejected at duplication"),
        },
        "wrong_type" => {
            assert!(matches!(
                result,
                Err(KfdInheritedTargetDebugTelemetryErrorV1::Admit(_))
            ));
            let raw = inherited_raw.unwrap();
            // A failed admission must not mutate an unrelated inherited descriptor.
            let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
            assert!(flags >= 0);
            assert_eq!(flags & libc::FD_CLOEXEC, 0);
        }
        "wrong_peer" => assert!(matches!(
            result,
            Err(KfdInheritedTargetDebugTelemetryErrorV1::Admit(
                KfdTargetDebugTelemetryTransportErrorV1::PeerCredentialMismatch
            ))
        )),
        "positive" => {
            let mut endpoint = result.unwrap().expect("telemetry endpoint");
            let raw = inherited_raw.unwrap();
            let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
            assert!(flags >= 0);
            assert_ne!(flags & libc::FD_CLOEXEC, 0);
            assert_eq!(
                std::env::var(KFD_TARGET_DEBUG_TELEMETRY_FD_ENV_V1).unwrap(),
                raw.to_string()
            );
            assert_eq!(
                std::env::var(KFD_TARGET_DEBUG_TELEMETRY_NONCE_ENV_V1).unwrap(),
                NONCE_HEX
            );
            assert_eq!(
                std::env::var(KFD_TARGET_DEBUG_TELEMETRY_DEBUGGER_PID_ENV_V1).unwrap(),
                debugger_pid_text.unwrap()
            );
            endpoint
                .send(KfdTargetDebugTelemetryPayloadV1::SessionStarted {
                    process_instance: KfdTargetDebugTelemetryDigestV1::from_bytes([0x21; 32])
                        .unwrap(),
                    executable: KfdTargetDebugArtifactIdentityV1::new(
                        KfdTargetDebugTelemetryDigestV1::from_bytes([0x22; 32]).unwrap(),
                        4096,
                    )
                    .unwrap(),
                })
                .unwrap();
            endpoint
                .send(KfdTargetDebugTelemetryPayloadV1::SessionEnded {
                    outcome: KfdTargetDebugSessionOutcomeV1::Completed,
                })
                .unwrap();
        }
        _ => panic!("unknown child case"),
    }
}

#[test]
fn absent_environment_is_an_optional_disabled_channel() {
    run_child("absent", ChildAbiEnvironment::default(), None);
}

#[test]
fn every_incomplete_environment_combination_is_rejected() {
    for mask in 1_u8..7 {
        let environment = ChildAbiEnvironment {
            descriptor: (mask & 1 != 0).then(|| "19".into()),
            nonce: (mask & 2 != 0).then(|| NONCE_HEX.into()),
            debugger_pid: (mask & 4 != 0).then(|| std::process::id().to_string().into()),
        };
        run_child("incomplete", environment, None);
    }
}

#[test]
fn descriptor_environment_requires_canonical_nonstdio_i32() {
    let hostile = [
        OsString::from(""),
        OsString::from("0"),
        OsString::from("1"),
        OsString::from("2"),
        OsString::from("03"),
        OsString::from("+3"),
        OsString::from(" 3"),
        OsString::from("3 "),
        OsString::from("-3"),
        OsString::from("2147483648"),
        OsString::from_vec(vec![b'3', 0xff]),
    ];
    for descriptor in hostile {
        run_child(
            "invalid_descriptor",
            ChildAbiEnvironment::complete(descriptor),
            None,
        );
    }
}

#[test]
fn nonce_environment_requires_exact_nonzero_lowercase_hex() {
    let hostile = [
        OsString::from(""),
        OsString::from("1".repeat(63)),
        OsString::from("1".repeat(65)),
        OsString::from("0".repeat(64)),
        OsString::from("A".repeat(64)),
        OsString::from(format!("{}g", "1".repeat(63))),
        OsString::from_vec(vec![b'1'; 63].into_iter().chain([0xff]).collect::<Vec<_>>()),
    ];
    for nonce in hostile {
        let mut environment = ChildAbiEnvironment::complete("19");
        environment.nonce = Some(nonce);
        run_child("invalid_nonce", environment, None);
    }
}

#[test]
fn debugger_pid_environment_requires_canonical_nonzero_u32() {
    let hostile = [
        OsString::from(""),
        OsString::from("0"),
        OsString::from("01"),
        OsString::from("+1"),
        OsString::from(" 1"),
        OsString::from("1 "),
        OsString::from("-1"),
        OsString::from("4294967296"),
        OsString::from_vec(vec![b'1', 0xff]),
    ];
    for debugger_pid in hostile {
        let mut environment = ChildAbiEnvironment::complete("19");
        environment.debugger_pid = Some(debugger_pid);
        run_child("invalid_debugger", environment, None);
    }
}

#[test]
fn unavailable_or_wrong_descriptor_never_becomes_an_endpoint() {
    run_child(
        "unavailable_descriptor",
        ChildAbiEnvironment::complete(i32::MAX.to_string()),
        None,
    );

    let file: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
    run_child(
        "wrong_type",
        ChildAbiEnvironment::complete(file.as_raw_fd().to_string()),
        Some(&file),
    );
}

#[test]
fn socket_peer_must_be_the_declared_live_debugger_process() {
    let mut unrelated = Command::new("sh")
        .args(["-c", "exec sleep 30"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (_debugger, target) = create_kfd_target_debug_telemetry_channel_v1().unwrap();
    let mut environment = ChildAbiEnvironment::complete(target.as_raw_fd().to_string());
    environment.debugger_pid = Some(unrelated.id().to_string().into());
    run_child("wrong_peer", environment, Some(&target));
    unrelated.kill().unwrap();
    unrelated.wait().unwrap();
}

#[test]
fn positive_inherited_channel_emits_records_and_protects_original() {
    let session_nonce = KfdTargetDebugSessionNonceV1::from_bytes([0x11; 32]).unwrap();
    let (debugger_fd, target) = create_kfd_target_debug_telemetry_channel_v1().unwrap();
    let environment = ChildAbiEnvironment::complete(target.as_raw_fd().to_string());
    let child = spawn_child("positive", &environment, Some(&target));
    drop(target);
    let target_process = KfdTargetDebugTelemetryProcessV1::capture(child.id()).unwrap();
    let mut debugger =
        KfdDebuggerTelemetryEndpointV1::admit(debugger_fd, session_nonce, target_process).unwrap();
    assert!(matches!(
        debugger.receive().unwrap().payload(),
        KfdTargetDebugTelemetryPayloadV1::SessionStarted { .. }
    ));
    assert!(matches!(
        debugger.receive().unwrap().payload(),
        KfdTargetDebugTelemetryPayloadV1::SessionEnded {
            outcome: KfdTargetDebugSessionOutcomeV1::Completed
        }
    ));
    assert_child_success("positive", child);
}
