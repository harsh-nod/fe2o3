use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use fe2o3_debug_cli::debug_source_map_identity_v1;
use fe2o3_debug_protocol::{
    DebugBackendV1, DebugCapabilityNameV1, DebugResponseV1, DebugResultV1, MemoryAvailabilityV1,
    ProtocolLimitsV1, ScopeStateV1, SessionStateV1, SourceMapProvenanceV1,
    SourceSiteAvailabilityV1, StackValuesAvailabilityV1, StopReasonV1, ValueAvailabilityV1,
    WaveInterpretationV1, decode_response_line_v1,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crate is in workspace/crates")
        .to_owned()
}

fn run_requests(requests: &[u8]) -> std::process::Output {
    run_requests_with_request(
        requests,
        "crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json",
    )
}

fn run_requests_with_request(requests: &[u8], request: &str) -> std::process::Output {
    let root = workspace_root();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .args([
            "sim",
            "--kir-v7",
            "crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir",
            "--request",
            request,
            "--protocol",
            "jsonl",
        ])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn debugger");
    child
        .stdin
        .take()
        .expect("debugger stdin")
        .write_all(requests)
        .expect("write request stream");
    child.wait_with_output().expect("wait for debugger")
}

fn run_requests_with_source_map(requests: &[u8]) -> std::process::Output {
    let root = workspace_root();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .args([
            "sim",
            "--kir-v7",
            "crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir",
            "--request",
            "crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json",
            "--source-map",
            "crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json",
            "--source-bundle-subject",
            "e584497b146b0df95a63a7890e003cd8edf2ce9dfb45dfda1cc62c8529119950",
            "--protocol",
            "jsonl",
        ])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn debugger with source map");
    child
        .stdin
        .take()
        .expect("debugger stdin")
        .write_all(requests)
        .expect("write source request stream");
    child.wait_with_output().expect("wait for debugger")
}

fn run_fill() -> Vec<u8> {
    let root = workspace_root();
    let requests = fs::read(root.join("crates/fe2o3-debug-cli/tutorial/fill-v1/requests.jsonl"))
        .expect("read request fixture");
    let output = run_requests(&requests);
    assert!(
        output.status.success(),
        "debugger failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    output.stdout
}

fn responses(bytes: &[u8]) -> Vec<DebugResponseV1> {
    bytes
        .split_inclusive(|byte| *byte == b'\n')
        .map(|line| decode_response_line_v1(line, ProtocolLimitsV1::default()).unwrap())
        .collect()
}

#[test]
fn source_map_drives_resolution_stepping_breakpoints_and_captured_stack() {
    let map_bytes =
        fs::read(workspace_root().join("crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json"))
            .unwrap();
    let map_identity: String = debug_source_map_identity_v1(&map_bytes)
        .unwrap()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let requests = format!(
        concat!(
            "{{\"operation\":\"resolve_source\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":1,\"expected_revision\":0,\"site\":{{\"function_ordinal\":0,\"block_ordinal\":0,\"point\":{{\"kind\":\"operation\",\"operation_ordinal\":2}}}}}}\n",
            "{{\"operation\":\"set_breakpoints\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":2,\"expected_revision\":0,\"breakpoints\":[{{\"enabled\":true,\"kind\":{{\"kind\":\"source\",\"source\":{{\"map_identity\":\"{}\",\"provenance\":\"caller_bound\",\"file_identity\":\"8b9da03723f1c1902bc22d282783d38998ecf3ee4fde126135052b17e050e80b\",\"byte_start\":74,\"byte_end\":91}}}}}}]}}\n",
            "{{\"operation\":\"continue\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":3,\"expected_revision\":1,\"max_events\":1024}}\n",
            "{{\"operation\":\"inspect_stack\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":4,\"expected_revision\":2,\"scope\":{{\"level\":\"dispatch\"}},\"page\":{{\"limit\":16}}}}\n",
            "{{\"operation\":\"step\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":5,\"expected_revision\":2,\"direction\":\"forward\",\"granularity\":\"source\",\"count\":1}}\n"
        ),
        map_identity
    );
    let output = run_requests_with_source_map(requests.as_bytes());
    assert!(
        output.status.success(),
        "debugger failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = responses(&output.stdout);
    assert_eq!(responses.len(), 5);
    let DebugResponseV1::Ok { result, .. } = &responses[0] else {
        panic!("source resolve failed")
    };
    let DebugResultV1::Source { site } = result.as_ref() else {
        panic!("wrong source result")
    };
    assert!(matches!(
        site.source,
        SourceSiteAvailabilityV1::Resolved { location }
            if location.provenance == SourceMapProvenanceV1::CallerBound
    ));

    let DebugResponseV1::Ok { result, .. } = &responses[2] else {
        panic!("source breakpoint did not stop")
    };
    assert!(matches!(
        result.as_ref(),
        DebugResultV1::Control { stop: Some(stop), .. }
            if stop.reason == StopReasonV1::Breakpoint
    ));

    let DebugResponseV1::Ok { result, .. } = &responses[3] else {
        panic!("stack query failed")
    };
    let DebugResultV1::Stack { frames, .. } = result.as_ref() else {
        panic!("wrong stack result")
    };
    assert!(!frames.is_empty());
    assert!(frames.iter().all(|frame| frame.frame > 0));
    assert!(
        frames
            .iter()
            .any(|frame| matches!(frame.values, StackValuesAvailabilityV1::Captured { .. }))
    );
    assert!(matches!(responses[4], DebugResponseV1::Ok { .. }));
}

fn project_workbench(bytes: &[u8]) -> Value {
    let rows: Vec<Value> = bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).expect("parse response projection input"))
        .collect();
    assert_eq!(rows.len(), 19);
    let digest = Sha256::digest(bytes);
    let response_sha256: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    json!({
        "schema": "fe2o3-debug-workbench-fixture-v1",
        "source": {
            "kernel": "crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir",
            "request": "crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json",
            "protocol_requests": "crates/fe2o3-debug-cli/tutorial/fill-v1/requests.jsonl",
            "protocol_responses": "crates/fe2o3-debug-cli/tutorial/fill-v1/responses.jsonl",
            "protocol_responses_sha256": response_sha256,
        },
        "capabilities": rows[0],
        "breakpoint_stop": rows[5],
        "hierarchy": rows[6],
        "values": rows[7],
        "watchpoint_stop": rows[9],
        "post_write_step": rows[10],
        "memory": rows[11],
        "events": rows[12],
        "reverse_step": rows[13],
        "limitations": [rows[14].clone(), rows[15].clone()],
        "trace": {
            "session": rows[17]["session"].clone(),
            "trace_identity": rows[17]["result"]["trace_identity"].clone(),
            "canonical_bytes": rows[17]["result"]["canonical_bytes"].clone(),
            "completeness": rows[17]["result"]["completeness"].clone(),
        },
    })
}

#[test]
fn fill_stream_is_agent_native_and_truthfully_labeled() {
    let output = run_fill();
    let responses = responses(&output);
    assert_eq!(responses.len(), 19);

    let DebugResponseV1::Ok {
        session, result, ..
    } = &responses[0]
    else {
        panic!("capability response was not ok")
    };
    assert_eq!(session.backend, DebugBackendV1::CpuKirSimulator);
    assert!(session.simulated);
    assert!(!session.hardware_observed);
    assert!(!session.performance_prediction);
    let DebugResultV1::Capabilities { capabilities } = result.as_ref() else {
        panic!("wrong capability result")
    };
    assert!(
        capabilities
            .iter()
            .any(|capability| { capability.name == DebugCapabilityNameV1::DeterministicReplay })
    );

    let DebugResponseV1::Ok { result, .. } = &responses[5] else {
        panic!("breakpoint continue response was not ok")
    };
    let DebugResultV1::Control {
        stop: Some(stop), ..
    } = result.as_ref()
    else {
        panic!("breakpoint continue did not stop")
    };
    assert_eq!(stop.reason, StopReasonV1::Breakpoint);
    assert_eq!(stop.breakpoint_id, Some(1));

    let DebugResponseV1::Ok { result, .. } = &responses[6] else {
        panic!("scope response was not ok")
    };
    let DebugResultV1::Scopes { scopes, .. } = result.as_ref() else {
        panic!("wrong scope result")
    };
    assert!(scopes.iter().any(|scope| matches!(
        scope.scope,
        fe2o3_debug_protocol::ExecutionScopeV1::Wave {
            interpretation: WaveInterpretationV1::LogicalVisualization,
            ..
        }
    )));

    let DebugResponseV1::Ok { result, .. } = &responses[7] else {
        panic!("value response was not ok")
    };
    let DebugResultV1::Values { values, .. } = result.as_ref() else {
        panic!("wrong value result")
    };
    assert!(
        values
            .iter()
            .any(|value| matches!(value.availability, ValueAvailabilityV1::Captured { .. }))
    );

    let DebugResponseV1::Ok { result, .. } = &responses[9] else {
        panic!("watchpoint continue response was not ok")
    };
    let DebugResultV1::Control {
        stop: Some(stop), ..
    } = result.as_ref()
    else {
        panic!("watchpoint continue did not stop")
    };
    assert_eq!(stop.reason, StopReasonV1::Watchpoint);
    assert_eq!(stop.watchpoint_id, Some(1));

    let DebugResponseV1::Ok { result, .. } = &responses[11] else {
        panic!("post-write memory response was not ok")
    };
    let DebugResultV1::Memory { memory, .. } = result.as_ref() else {
        panic!("wrong memory result")
    };
    assert!(matches!(
        &memory.availability,
        MemoryAvailabilityV1::Captured { bytes, .. } if bytes == "0x11000000"
    ));

    assert!(matches!(
        responses[14],
        DebugResponseV1::Unavailable {
            unavailable: fe2o3_debug_protocol::CapabilityUnavailableV1 {
                capability: DebugCapabilityNameV1::RegisterValues,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        responses[15],
        DebugResponseV1::Unavailable {
            unavailable: fe2o3_debug_protocol::CapabilityUnavailableV1 {
                capability: DebugCapabilityNameV1::SourceSites,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        responses[18],
        DebugResponseV1::Ok {
            session: fe2o3_debug_protocol::SessionViewV1 {
                state: SessionStateV1::Terminated,
                ..
            },
            ..
        }
    ));
}

#[test]
fn fill_transcript_matches_the_checked_in_exact_golden() {
    let output = run_fill();
    let golden = workspace_root().join("crates/fe2o3-debug-cli/tutorial/fill-v1/responses.jsonl");
    assert_eq!(
        output,
        fs::read(golden).expect("read exact response golden")
    );
}

#[test]
fn website_projection_is_derived_from_the_exact_protocol_golden() {
    let root = workspace_root();
    let responses = fs::read(root.join("crates/fe2o3-debug-cli/tutorial/fill-v1/responses.jsonl"))
        .expect("read exact response golden");
    let checked_in: Value = serde_json::from_slice(
        &fs::read(root.join("crates/fe2o3-debug-cli/tutorial/fill-v1/workbench-projection.json"))
            .expect("read workbench projection"),
    )
    .expect("parse workbench projection");
    assert_eq!(project_workbench(&responses), checked_in);
}

#[test]
fn stale_revision_is_rejected_without_state_change() {
    let output = run_requests(
        br#"{"operation":"get_state","schema":"fe2o3-debug-request-v1","request_id":91,"expected_revision":7}
"#,
    );
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses = responses(&output.stdout);
    assert!(matches!(
        responses.as_slice(),
        [DebugResponseV1::Error {
            session: Some(fe2o3_debug_protocol::SessionViewV1 { revision: 0, .. }),
            error: fe2o3_debug_protocol::DebugErrorV1 {
                code: fe2o3_debug_protocol::DebugErrorCodeV1::StaleRevision,
                state_changed: false,
                ..
            },
            ..
        }]
    ));
}

#[test]
fn malformed_frame_returns_one_typed_error_and_closes_the_stream() {
    let output = run_requests(b"{}\n");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses = responses(&output.stdout);
    assert!(matches!(
        responses.as_slice(),
        [DebugResponseV1::Error {
            request_id: None,
            error: fe2o3_debug_protocol::DebugErrorV1 {
                code: fe2o3_debug_protocol::DebugErrorCodeV1::InvalidJson,
                state_changed: false,
                ..
            },
            ..
        }]
    ));
}

#[test]
fn frame_aware_over_and_out_are_real_replay_controls() {
    let output = run_requests(
        br#"{"operation":"step","schema":"fe2o3-debug-request-v1","request_id":101,"expected_revision":0,"direction":"forward","granularity":"over","count":1}
{"operation":"step","schema":"fe2o3-debug-request-v1","request_id":102,"expected_revision":1,"direction":"forward","granularity":"out","count":1}
{"operation":"step","schema":"fe2o3-debug-request-v1","request_id":103,"expected_revision":2,"direction":"reverse","granularity":"over","count":1}
"#,
    );
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses = responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    for (response, reason) in responses[..2]
        .iter()
        .zip([StopReasonV1::Step, StopReasonV1::Completed])
    {
        let DebugResponseV1::Ok { result, .. } = response else {
            panic!("frame-aware step was not performed")
        };
        let DebugResultV1::Control {
            stop: Some(stop), ..
        } = result.as_ref()
        else {
            panic!("frame-aware step did not return a stop")
        };
        assert_eq!(stop.reason, reason);
    }
    assert!(matches!(
        responses[2],
        DebugResponseV1::Unavailable {
            unavailable: fe2o3_debug_protocol::CapabilityUnavailableV1 {
                capability: DebugCapabilityNameV1::ReverseStep,
                state_changed: false,
                ..
            },
            ..
        }
    ));
}

#[test]
fn explicit_frame_identity_must_fit_the_simulator_frame_range() {
    let output = run_requests(
        br#"{"operation":"step","schema":"fe2o3-debug-request-v1","request_id":111,"expected_revision":0,"direction":"forward","granularity":"operation","count":1}
{"operation":"inspect_values","schema":"fe2o3-debug-request-v1","request_id":112,"expected_revision":1,"scope":{"level":"dispatch"},"frame":4294967297,"selector":{"selector":"all"},"page":{"limit":1}}
"#,
    );
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses = responses(&output.stdout);
    assert!(matches!(responses[0], DebugResponseV1::Ok { .. }));
    assert!(matches!(
        responses[1],
        DebugResponseV1::Error {
            session: Some(fe2o3_debug_protocol::SessionViewV1 { revision: 1, .. }),
            error: fe2o3_debug_protocol::DebugErrorV1 {
                code: fe2o3_debug_protocol::DebugErrorCodeV1::InvalidRequest,
                state_changed: false,
                ..
            },
            ..
        }
    ));
}

#[test]
fn live_jsonl_flushes_each_response_and_same_cursor_stop_changes_revision() {
    let root = workspace_root();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .args([
            "sim",
            "--kir-v7",
            "crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir",
            "--request",
            "crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json",
            "--protocol",
            "jsonl",
        ])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn live debugger");
    let stdout = child.stdout.take().expect("live debugger stdout");
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = Vec::new();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let decoded = decode_response_line_v1(&line, ProtocolLimitsV1::default());
                    if sender.send(decoded).is_err() {
                        break;
                    }
                }
                Err(error) => panic!("read live debugger response: {error}"),
            }
        }
    });
    let mut stdin = child.stdin.take().expect("live debugger stdin");
    {
        let mut exchange = |request: &Value| {
            serde_json::to_writer(&mut stdin, request).expect("write live debugger request");
            stdin.write_all(b"\n").expect("terminate live request");
            stdin.flush().expect("flush live request");
            receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("debugger did not flush a response")
                .expect("decode live debugger response")
        };

        let set = exchange(&json!({
            "operation": "set_breakpoints",
            "schema": "fe2o3-debug-request-v1",
            "request_id": 121,
            "expected_revision": 0,
            "breakpoints": [{
                "enabled": true,
                "kind": {
                    "kind": "site",
                    "site": {
                        "function_ordinal": 0,
                        "block_ordinal": 0,
                        "point": {"kind": "operation", "operation_ordinal": 2}
                    },
                    "phase": "before_operation"
                }
            }]
        }));
        assert!(matches!(
            set,
            DebugResponseV1::Ok {
                session: fe2o3_debug_protocol::SessionViewV1 { revision: 1, .. },
                ..
            }
        ));
        let stopped = exchange(&json!({
            "operation": "continue",
            "schema": "fe2o3-debug-request-v1",
            "request_id": 122,
            "expected_revision": 1,
            "max_events": 1024
        }));
        let (cursor, revision) = match &stopped {
            DebugResponseV1::Ok {
                session, result, ..
            } => {
                assert!(matches!(
                    result.as_ref(),
                    DebugResultV1::Control {
                        stop: Some(fe2o3_debug_protocol::StopViewV1 {
                            reason: StopReasonV1::Breakpoint,
                            ..
                        }),
                        ..
                    }
                ));
                (session.cursor, session.revision)
            }
            _ => panic!("live continue did not stop at the breakpoint"),
        };
        let seek = exchange(&json!({
            "operation": "seek",
            "schema": "fe2o3-debug-request-v1",
            "request_id": 123,
            "expected_revision": revision,
            "cursor": cursor
        }));
        let next_revision = revision + 1;
        assert!(matches!(
            seek,
            DebugResponseV1::Ok {
                session: fe2o3_debug_protocol::SessionViewV1 { revision, .. },
                result,
                ..
            } if revision == next_revision && matches!(
                result.as_ref(),
                DebugResultV1::Control {
                    stop: Some(fe2o3_debug_protocol::StopViewV1 {
                        reason: StopReasonV1::Step,
                        ..
                    }),
                    ..
                }
            )
        ));
        let terminated = exchange(&json!({
            "operation": "terminate",
            "schema": "fe2o3-debug-request-v1",
            "request_id": 124,
            "expected_revision": next_revision
        }));
        assert!(matches!(terminated, DebugResponseV1::Ok { .. }));
    }
    drop(stdin);
    let output = child.wait_with_output().expect("wait for live debugger");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    reader.join().expect("join live response reader");
}

#[test]
fn terminal_fault_marks_only_the_matching_scope_chain_failed() {
    let request_path = std::env::temp_dir().join(format!(
        "fe2o3-debug-fault-request-{}.json",
        std::process::id()
    ));
    fs::write(
        &request_path,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"fill","grid":[4,1,1],"workgroup":[64,1,1],"arguments":[{"kind":"buffer","element":"u32","access":"read_write","alignment":4,"bytes":"0x00000000"}]}"#,
    )
    .expect("write fault request");
    let requests = br#"{"operation":"continue","schema":"fe2o3-debug-request-v1","request_id":131,"expected_revision":0,"max_events":1000000}
{"operation":"inspect_scope","schema":"fe2o3-debug-request-v1","request_id":132,"expected_revision":1,"scope":{"level":"dispatch"},"include_children":true,"page":{"limit":16}}
{"operation":"inspect_scope","schema":"fe2o3-debug-request-v1","request_id":133,"expected_revision":1,"scope":{"level":"wave","workgroup":[0,0,0],"wave":0},"include_children":true,"page":{"limit":64}}
"#;
    let output = run_requests_with_request(requests, request_path.to_str().unwrap());
    let _ = fs::remove_file(request_path);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses = responses(&output.stdout);
    assert!(matches!(
        responses[0],
        DebugResponseV1::Ok { ref result, .. } if matches!(
            result.as_ref(),
            DebugResultV1::Control {
                stop: Some(fe2o3_debug_protocol::StopViewV1 {
                    reason: StopReasonV1::Fault,
                    ..
                }),
                ..
            }
        )
    ));
    let DebugResponseV1::Ok { result, .. } = &responses[1] else {
        panic!("dispatch scope inspection failed")
    };
    let DebugResultV1::Scopes { scopes, .. } = result.as_ref() else {
        panic!("dispatch scope result missing")
    };
    assert!(
        scopes
            .iter()
            .all(|scope| scope.state == ScopeStateV1::Failed)
    );

    let DebugResponseV1::Ok { result, .. } = &responses[2] else {
        panic!("wave scope inspection failed")
    };
    let DebugResultV1::Scopes { scopes, .. } = result.as_ref() else {
        panic!("wave scope result missing")
    };
    assert!(scopes.iter().any(|scope| {
        matches!(
            scope.scope,
            fe2o3_debug_protocol::ExecutionScopeV1::Lane { lane: 0, .. }
        ) && scope.state == ScopeStateV1::Completed
    }));
    assert!(scopes.iter().any(|scope| {
        matches!(
            scope.scope,
            fe2o3_debug_protocol::ExecutionScopeV1::Lane { lane: 1, .. }
        ) && scope.state == ScopeStateV1::Failed
    }));
}
