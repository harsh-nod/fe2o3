#![cfg(target_os = "linux")]

use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use fe2o3_debug_protocol::*;
use serde_json::{Value, json};

const AUTHORIZATION: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const WRONG_AUTHORIZATION: &str =
    "3333333333333333333333333333333333333333333333333333333333333333";
static PID_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Session {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
    transcript: Vec<u8>,
    pid_file: PathBuf,
}

struct TestProcess(Child);

impl Drop for TestProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Session {
    fn launch(arguments: &[&str]) -> Self {
        let executable = env!("CARGO_BIN_EXE_fe2o3-debug");
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_rocgdb_mi_v3.py");
        let mut command = Command::new(executable);
        let pid_file = std::env::temp_dir().join(format!(
            "fe2o3-fake-rocgdb-v3-{}-{}",
            std::process::id(),
            PID_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        command
            .args(["live-rocgdb", "--rocgdb"])
            .arg(fixture)
            .args([
                "--authorization",
                AUTHORIZATION,
                "--protocol",
                "jsonl",
                "--wave-width",
                "32",
                "--timeout-ms",
                "5000",
            ])
            .args(arguments)
            .env("FE2O3_FAKE_ROCGDB_PID_FILE", &pid_file)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("launch live-rocgdb");
        let input = BufWriter::new(child.stdin.take().expect("stdin"));
        let output = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            input,
            output,
            transcript: Vec::new(),
            pid_file,
        }
    }

    fn request(&mut self, request: Value) -> RocgdbMiCliResponseV3 {
        serde_json::to_writer(&mut self.input, &request).expect("request JSON");
        self.input.write_all(b"\n").expect("request newline");
        self.read_response()
    }

    fn raw_request(&mut self, request: &[u8]) -> RocgdbMiCliResponseV3 {
        self.input.write_all(request).expect("raw request");
        self.read_response()
    }

    fn read_response(&mut self) -> RocgdbMiCliResponseV3 {
        self.input.flush().expect("flush request");
        let mut line = Vec::new();
        self.output.read_until(b'\n', &mut line).expect("response");
        assert!(!line.is_empty(), "live-rocgdb closed without a response");
        self.transcript.extend_from_slice(&line);
        decode_rocgdb_mi_cli_response_line_v3(&line).expect("valid response")
    }

    fn wait(mut self) -> Vec<u8> {
        drop(self.input);
        let status = self.child.wait().expect("wait live-rocgdb");
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .expect("stderr")
            .read_to_string(&mut stderr)
            .expect("read stderr");
        assert!(status.success(), "live-rocgdb failed: {stderr}");
        assert_processes_gone(&self.pid_file);
        self.transcript
    }
}

fn assert_processes_gone(pid_file: &std::path::Path) {
    let pids = std::fs::read_to_string(pid_file).expect("fixture process identities");
    let deadline = Instant::now() + Duration::from_secs(5);
    for pid in pids.lines() {
        let path = PathBuf::from(format!("/proc/{pid}"));
        while path.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!path.exists(), "fixture process survived cleanup");
    }
    std::fs::remove_file(pid_file).expect("remove fixture pid file");
}

use std::io::Read;

fn ok(response: RocgdbMiCliResponseV3) -> (u64, RocgdbMiCliResultV3) {
    match response {
        RocgdbMiCliResponseV3::Ok {
            revision, result, ..
        } => (revision, *result),
        response => panic!("expected ok response, got {response:?}"),
    }
}

#[test]
fn live_rocgdb_jsonl_is_bounded_relative_authorized_and_agent_friendly() {
    let mut session = Session::launch(&["--", "/bin/true", "argument with spaces"]);

    let (revision, bootstrap) = ok(session.request(json!({
        "operation": "get_session",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 1
    })));
    assert_eq!(revision, 1);
    assert!(matches!(
        bootstrap,
        RocgdbMiCliResultV3::Session {
            bootstrap: RocgdbMiControlResultV3 {
                operation: RocgdbMiControlOperationV3::Launch,
                ..
            },
            ..
        }
    ));

    let duplicate = session.request(json!({
        "operation": "get_session",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 1
    }));
    assert!(matches!(
        duplicate,
        RocgdbMiCliResponseV3::Error {
            code: RocgdbMiCliErrorCodeV3::DuplicateRequestId,
            terminal: false,
            ..
        }
    ));

    let (_, capabilities) = ok(session.request(json!({
        "operation": "discover_capabilities",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 2
    })));
    let RocgdbMiCliResultV3::Capabilities { capabilities } = capabilities else {
        panic!("expected capabilities");
    };
    capabilities.validate().expect("capabilities");
    let stopped_wave = capabilities
        .mi
        .capabilities
        .iter()
        .find(|item| item.name == RocgdbMiCapabilityNameV3::StoppedWave)
        .expect("stopped wave capability");
    assert_eq!(
        stopped_wave.unavailable_reason,
        Some(LiveGpuUnavailableReasonV3::NotCaptured)
    );

    ok(session.request(json!({
        "operation": "admit_code_object",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 3,
        "content": {
            "digest": "4444444444444444444444444444444444444444444444444444444444444444",
            "canonical_bytes": 512
        },
        "load_base": "0x1000",
        "byte_len": 512,
        "kernel_entry": "0x1020"
    })));
    ok(session.request(json!({
        "operation": "admit_allocation",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 4,
        "allocation": {"ordinal": 1, "generation": 1},
        "base": "0x2000",
        "byte_len": 256,
        "space": "global"
    })));
    ok(session.request(json!({
        "operation": "admit_source_line",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 5,
        "source": {
            "source_map_identity": "5555555555555555555555555555555555555555555555555555555555555555",
            "file_identity": "6666666666666666666666666666666666666666666666666666666666666666",
            "byte_start": 10,
            "byte_end": 20
        },
        "path": "/private/kernel.fe",
        "line": 7
    })));
    let (_, admitted) = ok(session.request(json!({
        "operation": "admit_gpu_threads",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 6,
        "thread_ordinals": [0]
    })));
    assert!(matches!(
        admitted,
        RocgdbMiCliResultV3::GpuThreadsAdmitted { ref admissions }
            if admissions.len() == 1
    ));

    let (_, running) = ok(session.request(json!({
        "operation": "next_event",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 7,
        "wait_milliseconds": 1000
    })));
    assert!(matches!(
        running,
        RocgdbMiCliResultV3::Event {
            event: RocgdbMiExecutionEventV3::Running { revision: 1 }
        }
    ));
    let (_, stopped) = ok(session.request(json!({
        "operation": "next_event",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 8,
        "wait_milliseconds": 1000
    })));
    let RocgdbMiCliResultV3::Event {
        event: RocgdbMiExecutionEventV3::Stopped { snapshot },
    } = stopped
    else {
        panic!("expected admitted stop");
    };
    assert_eq!(snapshot.revision, 2);
    let thread = &snapshot.threads[0];
    let scope = RocgdbMiStoppedScopeV3 {
        stop_identity: snapshot.stop_identity,
        thread: thread.thread,
        wave: thread.wave,
        lane: None,
    };
    let scope_json = serde_json::to_value(scope).unwrap();

    let (_, registers) = ok(session.request(json!({
        "operation": "inspect_registers",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 9,
        "scope": scope_json
    })));
    assert!(matches!(
        registers,
        RocgdbMiCliResultV3::Registers { ref snapshot }
            if snapshot.registers.iter().any(|register| register.name == "exec")
    ));
    let (_, values) = ok(session.request(json!({
        "operation": "inspect_values",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 10,
        "scope": serde_json::to_value(scope).unwrap()
    })));
    assert!(matches!(
        values,
        RocgdbMiCliResultV3::Values { ref snapshot }
            if matches!(
                snapshot.values[1].value,
                LiveGpuAvailabilityV3::Unavailable {
                    reason: LiveGpuUnavailableReasonV3::OptimizedOut,
                    ..
                }
            )
    ));
    ok(session.request(json!({
        "operation": "evaluate_expression",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 11,
        "scope": serde_json::to_value(scope).unwrap(),
        "value_identity": "7777777777777777777777777777777777777777777777777777777777777777",
        "name": "first",
        "expression": "first"
    })));
    let (_, memory) = ok(session.request(json!({
        "operation": "read_memory",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 12,
        "request": {
            "request_id": 12,
            "expected_revision": 2,
            "scope": serde_json::to_value(scope).unwrap(),
            "allocation": {"ordinal": 1, "generation": 1},
            "byte_offset": 4,
            "byte_len": 2
        }
    })));
    assert!(matches!(
        memory,
        RocgdbMiCliResultV3::Memory {
            memory: RocgdbMiMemoryReadResultV3 {
                memory: LiveGpuMemoryReadV3 {
                    value: LiveGpuAvailabilityV3::Available {
                        value: LiveGpuMemoryBytesV3 { ref bytes, .. },
                        ..
                    },
                    ..
                },
                ..
            }
        } if bytes == "a10f"
    ));

    let (_, stale) = ok(session.request(json!({
        "operation": "control",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 13,
        "control": {
            "operation": "continue",
            "request_id": 13,
            "authorization": {
                "authorization_identity": AUTHORIZATION,
                "expected_revision": 1
            },
            "focus": serde_json::to_value(thread.thread).unwrap()
        }
    })));
    assert!(matches!(
        stale,
        RocgdbMiCliResultV3::Control {
            control: RocgdbMiControlResultV3 {
                outcome: RocgdbMiControlOutcomeV3::Unavailable {
                    reason: RocgdbMiControlUnavailableReasonV3::StaleRevision,
                    effect: RocgdbMiControlEffectV3::None
                },
                ..
            }
        }
    ));
    let (_, unauthorized) = ok(session.request(json!({
        "operation": "control",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 14,
        "control": {
            "operation": "continue",
            "request_id": 14,
            "authorization": {
                "authorization_identity": WRONG_AUTHORIZATION,
                "expected_revision": 2
            },
            "focus": serde_json::to_value(thread.thread).unwrap()
        }
    })));
    assert!(matches!(
        unauthorized,
        RocgdbMiCliResultV3::Control {
            control: RocgdbMiControlResultV3 {
                outcome: RocgdbMiControlOutcomeV3::Unavailable {
                    reason: RocgdbMiControlUnavailableReasonV3::AuthorizationMismatch,
                    ..
                },
                ..
            }
        }
    ));
    let (continued_revision, _) = ok(session.request(json!({
        "operation": "control",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 15,
        "control": {
            "operation": "continue",
            "request_id": 15,
            "authorization": {
                "authorization_identity": AUTHORIZATION,
                "expected_revision": 2
            },
            "focus": serde_json::to_value(thread.thread).unwrap()
        }
    })));
    assert_eq!(continued_revision, 3);

    let bad_terminate = session.request(json!({
        "operation": "terminate",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 16,
        "authorization": {
            "authorization_identity": WRONG_AUTHORIZATION,
            "expected_revision": 3
        }
    }));
    assert!(matches!(
        bad_terminate,
        RocgdbMiCliResponseV3::Error {
            code: RocgdbMiCliErrorCodeV3::AuthorizationMismatch,
            terminal: false,
            ..
        }
    ));
    let (terminated_revision, terminated) = ok(session.request(json!({
        "operation": "terminate",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 17,
        "authorization": {
            "authorization_identity": AUTHORIZATION,
            "expected_revision": 3
        }
    })));
    assert_eq!(terminated_revision, 4);
    assert!(matches!(
        terminated,
        RocgdbMiCliResultV3::Terminated {
            revision: 4,
            effect: RocgdbMiControlEffectV3::Committed
        }
    ));

    let transcript = session.wait();
    let transcript = String::from_utf8(transcript).unwrap();
    for native in [
        "/bin/true",
        "/private/kernel.fe",
        "0x1000",
        "0x1020",
        "0x2000",
        "thread-id",
        "target-id",
        "\"load_base\":",
        "\"kernel_entry\":",
        "ROCgdb private console text",
        "INFERIOR_NATIVE_SECRET",
        "\"pid\"",
        "\"process\"",
        "\"path\":",
    ] {
        assert!(!transcript.contains(native), "response leaked {native}");
    }
}

#[test]
fn eof_malformed_input_and_timeout_all_cleanup_rocgdb_and_inferior() {
    Session::launch(&["--", "/bin/true"]).wait();

    let mut malformed = Session::launch(&["--", "/bin/true"]);
    let response = malformed.raw_request(b"{not-json}\n");
    assert!(matches!(
        response,
        RocgdbMiCliResponseV3::Error {
            code: RocgdbMiCliErrorCodeV3::InvalidRequest,
            terminal: true,
            ..
        }
    ));
    malformed.wait();

    let mut timed_out = Session::launch(&["--", "/bin/true"]);
    ok(timed_out.request(json!({
        "operation": "next_event",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 1,
        "wait_milliseconds": 1000
    })));
    let response = timed_out.request(json!({
        "operation": "next_event",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 2,
        "wait_milliseconds": 10
    }));
    assert!(matches!(
        response,
        RocgdbMiCliResponseV3::Error {
            code: RocgdbMiCliErrorCodeV3::Timeout,
            terminal: true,
            ..
        }
    ));
    timed_out.wait();
}

#[test]
fn bootstrap_failure_cleans_a_started_inferior() {
    let executable = env!("CARGO_BIN_EXE_fe2o3-debug");
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_rocgdb_mi_v3.py");
    let pid_file = std::env::temp_dir().join(format!(
        "fe2o3-fake-rocgdb-v3-{}-{}",
        std::process::id(),
        PID_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let output = Command::new(executable)
        .args(["live-rocgdb", "--rocgdb"])
        .arg(fixture)
        .args([
            "--authorization",
            AUTHORIZATION,
            "--timeout-ms",
            "1000",
            "--",
            "/bin/true",
        ])
        .env("FE2O3_FAKE_ROCGDB_PID_FILE", &pid_file)
        .env("FE2O3_FAKE_ROCGDB_FAIL_RUN", "1")
        .output()
        .expect("bootstrap failure process");
    assert!(!output.status.success());
    assert_processes_gone(&pid_file);
}

#[test]
fn startup_error_never_echoes_an_unknown_authority() {
    let executable = env!("CARGO_BIN_EXE_fe2o3-debug");
    let secret = "/private/target-native-authority";
    let output = Command::new(executable)
        .args([
            "live-rocgdb",
            "--rocgdb",
            "/unused/rocgdb",
            "--authorization",
            AUTHORIZATION,
            secret,
            "value",
            "--",
            "/private/target-program",
        ])
        .output()
        .expect("invalid startup process");
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains(secret));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(secret));

    let duplicate = Command::new(executable)
        .args([
            "live-rocgdb",
            "--rocgdb",
            "/unused/rocgdb",
            "--authorization",
            AUTHORIZATION,
            "--wave-width",
            "32",
            "--wave-width",
            "64",
            "--",
            "/private/target-program",
        ])
        .output()
        .expect("duplicate startup option");
    assert!(!duplicate.status.success());

    let relative = Command::new(executable)
        .args([
            "live-rocgdb",
            "--rocgdb",
            "rocgdb",
            "--authorization",
            AUTHORIZATION,
            "--",
            "target-program",
        ])
        .output()
        .expect("relative startup authority");
    assert!(!relative.status.success());
}

#[test]
fn authorized_attach_bootstrap_never_echoes_process_authority() {
    let mut attached = TestProcess(
        Command::new("/bin/sleep")
            .arg("60")
            .spawn()
            .expect("borrowed attach target"),
    );
    let attached_identity = attached.0.id().to_string();
    let mut session = Session::launch(&["--attach", &attached_identity]);
    let (_, result) = ok(session.request(json!({
        "operation": "get_session",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 1
    })));
    assert!(matches!(
        result,
        RocgdbMiCliResultV3::Session {
            bootstrap: RocgdbMiControlResultV3 {
                operation: RocgdbMiControlOperationV3::Attach,
                ..
            },
            ..
        }
    ));
    ok(session.request(json!({
        "operation": "terminate",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 2,
        "authorization": {
            "authorization_identity": AUTHORIZATION,
            "expected_revision": 1
        }
    })));
    let transcript = String::from_utf8(session.wait()).unwrap();
    assert!(
        attached
            .0
            .try_wait()
            .expect("inspect attach target")
            .is_none(),
        "debugger cleanup terminated a borrowed attach target"
    );
    for forbidden in ["pid", "process", "target-attach", "descriptor", "path"] {
        assert!(
            !transcript.contains(forbidden),
            "attach response leaked {forbidden}"
        );
    }
}

#[test]
#[ignore = "requires installed ROCgdb"]
fn installed_rocgdb_runs_through_the_public_jsonl_entrypoint() {
    let executable = env!("CARGO_BIN_EXE_fe2o3-debug");
    let rocgdb = std::env::var_os("FE2O3_ROCGDB").unwrap_or_else(|| "/usr/bin/rocgdb".into());
    let mut child = Command::new(executable)
        .args(["live-rocgdb", "--rocgdb"])
        .arg(rocgdb)
        .args([
            "--authorization",
            AUTHORIZATION,
            "--timeout-ms",
            "15000",
            "--",
            "/bin/sleep",
            "60",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("installed ROCgdb CLI");
    let mut input = BufWriter::new(child.stdin.take().unwrap());
    let mut output = BufReader::new(child.stdout.take().unwrap());
    {
        let mut exchange = |request: Value| {
            serde_json::to_writer(&mut input, &request).unwrap();
            input.write_all(b"\n").unwrap();
            input.flush().unwrap();
            let mut line = Vec::new();
            output.read_until(b'\n', &mut line).unwrap();
            decode_rocgdb_mi_cli_response_line_v3(&line).unwrap()
        };
        let (_, capabilities) = ok(exchange(json!({
            "operation": "discover_capabilities",
            "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
            "request_id": 1
        })));
        let RocgdbMiCliResultV3::Capabilities { capabilities } = capabilities else {
            panic!("expected capabilities");
        };
        for required in [
            RocgdbMiCapabilityNameV3::Launch,
            RocgdbMiCapabilityNameV3::StructuredThreads,
            RocgdbMiCapabilityNameV3::Breakpoints,
            RocgdbMiCapabilityNameV3::Continue,
            RocgdbMiCapabilityNameV3::Pause,
            RocgdbMiCapabilityNameV3::Step,
        ] {
            assert!(capabilities.mi.capabilities.iter().any(|capability| {
                capability.name == required
                    && capability.availability == LiveGpuCapabilityAvailabilityV3::Available
            }));
        }
        ok(exchange(json!({
        "operation": "terminate",
        "schema": ROCGDB_MI_CLI_REQUEST_SCHEMA_V3,
        "request_id": 2,
        "authorization": {
            "authorization_identity": AUTHORIZATION,
            "expected_revision": 1
        }
        })));
    }
    drop(input);
    let status = child.wait().unwrap();
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(status.success(), "installed ROCgdb CLI failed: {stderr}");
}
