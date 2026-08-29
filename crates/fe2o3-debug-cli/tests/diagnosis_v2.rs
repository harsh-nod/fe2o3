use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use fe2o3_debug_protocol::*;
use fe2o3_kernel_ir::{
    AddressSpace, Axis, BarrierSemantics, BasicBlock, BlockId, ComparePredicate, Constant,
    Convergence, Function, IndexKind, IntrinsicKind, IntrinsicOperation, Kernel, LaunchDomain,
    LaunchExtent, MemoryOrdering, Module, Operation, OperationKind, Signature,
    SynchronizationScope, TargetCapability, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrV7, WorkgroupBarrier,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is in workspace/crates")
        .to_owned()
}

fn run_debugger(kernel: &Path, request: &Path, requests: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .args(["sim", "--kir-v7"])
        .arg(kernel)
        .arg("--request")
        .arg(request)
        .args(["--protocol", "jsonl"])
        .current_dir(workspace_root())
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
        .expect("write debugger requests");
    child.wait_with_output().expect("wait for debugger")
}

fn response_lines(bytes: &[u8]) -> Vec<&[u8]> {
    bytes
        .split_inclusive(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect()
}

#[test]
fn seeded_out_of_bounds_diagnosis_is_structured_and_truth_labeled() {
    let root = workspace_root();
    let request_path = std::env::temp_dir().join(format!(
        "fe2o3-debug-diagnosis-oob-request-{}.json",
        std::process::id()
    ));
    fs::write(
        &request_path,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"fill","grid":[4,1,1],"workgroup":[64,1,1],"arguments":[{"kind":"buffer","element":"u32","access":"read_write","alignment":4,"bytes":"0x00000000"}]}"#,
    )
    .unwrap();
    let requests = br#"{"operation":"continue","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0,"max_events":1000000}
{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":2,"expected_revision":1,"filter":{"class":"memory_out_of_bounds"},"page":{"limit":1}}
"#;
    let output = run_debugger(
        &root.join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir"),
        &request_path,
        requests,
    );
    let _ = fs::remove_file(request_path);
    assert!(
        output.status.success(),
        "debugger failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let lines = response_lines(&output.stdout);
    assert_eq!(lines.len(), 2);
    let control = decode_response_line_v1(lines[0], ProtocolLimitsV1::default()).unwrap();
    assert!(matches!(
        control,
        DebugResponseV1::Ok { result, .. }
            if matches!(
                result.as_ref(),
                DebugResultV1::Control {
                    stop: Some(StopViewV1 { reason: StopReasonV1::Fault, .. }),
                    ..
                }
            )
    ));

    let response =
        decode_diagnosis_response_line_v2(lines[1], ProtocolLimitsV1::default()).unwrap();
    let DiagnosisResponseV2::Ok {
        session,
        completeness,
        diagnoses,
        next_cursor,
        ..
    } = response
    else {
        panic!("diagnosis failed")
    };
    assert!(session.simulated);
    assert!(!session.hardware_observed);
    assert_eq!(completeness, CaptureCompletenessV1::Complete);
    assert!(next_cursor.is_none());
    assert_eq!(diagnoses.len(), 1);
    let diagnosis = &diagnoses[0];
    assert_eq!(diagnosis.class, DiagnosisClassV2::MemoryOutOfBounds);
    assert!(matches!(
        diagnosis.context.dispatch,
        DiagnosisFactV2::Declared { .. }
    ));
    assert!(matches!(
        diagnosis.context.workgroup,
        DiagnosisFactV2::Observed { .. }
    ));
    assert!(matches!(
        diagnosis.context.wave,
        DiagnosisFactV2::Inferred {
            basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
            ..
        }
    ));
    assert!(matches!(
        diagnosis.context.lane,
        DiagnosisFactV2::Inferred {
            basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
            ..
        }
    ));
    assert!(matches!(
        diagnosis.memory_region,
        DiagnosisFactV2::Observed {
            value: DiagnosisMemoryRegionV2 {
                requested_offset: 4,
                requested_bytes: 4,
                allocation_bytes: 4,
                ..
            }
        }
    ));
    assert!(matches!(
        diagnosis.barrier,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NotApplicable
        }
    ));
}

fn operation(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn divergent_barrier_module(exit_local: u64) -> Module {
    let barrier = WorkgroupBarrier {
        memory_scope: SynchronizationScope::Workgroup,
        semantics: BarrierSemantics::new(MemoryOrdering::AcquireRelease, [AddressSpace::Workgroup]),
        convergence: Convergence::uniform(SynchronizationScope::Workgroup),
    };
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        operation(
            1,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        operation(
            2,
            Type::INDEX,
            OperationKind::Constant(Constant::Index(exit_local)),
        ),
        operation(
            3,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut exits = BasicBlock::new(BlockId(1));
    exits.terminator = Some(Terminator::Return { values: vec![] });
    let mut waits = BasicBlock::new(BlockId(2));
    waits.operations.push(Operation::new(
        vec![],
        OperationKind::WorkgroupBarrier(barrier),
    ));
    waits.terminator = Some(Terminator::Return { values: vec![] });

    let capability = TargetCapability::WorkgroupBarrier;
    let mut function = Function::kernel_entry(
        "divergent_barrier_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry, exits, waits],
    );
    function.required_capabilities.insert(capability.clone());
    let mut kernel = Kernel::new(
        "divergent_barrier",
        "divergent_barrier_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.required_capabilities.insert(capability.clone());
    let mut module = Module::new("debug-diagnosis-tests::divergent-barrier");
    module.required_capabilities.insert(capability);
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

#[test]
fn seeded_barrier_divergence_names_phase_and_participant_origins() {
    let stem = format!("fe2o3-debug-diagnosis-barrier-{}", std::process::id());
    let kernel_path = std::env::temp_dir().join(format!("{stem}.kir"));
    let request_path = std::env::temp_dir().join(format!("{stem}.json"));
    let canonical = VerifiedCanonicalKernelIrV7::from_module(divergent_barrier_module(0)).unwrap();
    fs::write(&kernel_path, canonical.canonical_bytes()).unwrap();
    fs::write(
        &request_path,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"divergent_barrier","grid":[2,1,1],"workgroup":[2,1,1],"arguments":[]}"#,
    )
    .unwrap();
    let requests = br#"{"operation":"continue","schema":"fe2o3-debug-request-v1","request_id":11,"expected_revision":0,"max_events":1000000}
{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":12,"expected_revision":1,"filter":{"class":"workgroup_barrier_divergence"},"page":{"limit":1}}
"#;
    let output = run_debugger(&kernel_path, &request_path, requests);
    let _ = fs::remove_file(kernel_path);
    let _ = fs::remove_file(request_path);
    assert!(
        output.status.success(),
        "debugger failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let lines = response_lines(&output.stdout);
    assert_eq!(lines.len(), 2);
    let response =
        decode_diagnosis_response_line_v2(lines[1], ProtocolLimitsV1::default()).unwrap();
    let DiagnosisResponseV2::Ok {
        session, diagnoses, ..
    } = response
    else {
        panic!("diagnosis failed")
    };
    assert!(session.simulated);
    assert!(!session.hardware_observed);
    assert_eq!(diagnoses.len(), 1);
    let diagnosis = &diagnoses[0];
    assert_eq!(
        diagnosis.class,
        DiagnosisClassV2::WorkgroupBarrierDivergence
    );
    assert!(matches!(
        diagnosis.site,
        DiagnosisFactV2::Observed {
            value: KirSiteV1 {
                function_ordinal: 0,
                block_ordinal: 2,
                point: KirSitePointV1::Operation {
                    operation_ordinal: 0
                }
            }
        }
    ));
    let DiagnosisFactV2::Observed {
        value:
            DiagnosisBarrierV2::Divergence {
                phase,
                observed_arrivals,
                expected_participants,
                waiting,
                exited,
            },
    } = &diagnosis.barrier
    else {
        panic!("barrier evidence is missing")
    };
    assert!(matches!(phase, DiagnosisFactV2::Observed { value: 0 }));
    assert!(matches!(
        observed_arrivals,
        DiagnosisFactV2::Observed { value: 1 }
    ));
    assert!(matches!(
        expected_participants,
        DiagnosisFactV2::Inferred {
            value: 2,
            basis: DiagnosisInferenceBasisV2::LaunchGeometry
        }
    ));
    for participant in [waiting, exited] {
        assert!(matches!(
            participant.local_workitem,
            DiagnosisFactV2::Observed { .. }
        ));
        assert!(matches!(
            participant.global_workitem,
            DiagnosisFactV2::Inferred {
                basis: DiagnosisInferenceBasisV2::LaunchGeometry,
                ..
            }
        ));
        assert!(matches!(
            participant.lane,
            DiagnosisFactV2::Inferred {
                basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
                ..
            }
        ));
    }
    let text = std::str::from_utf8(lines[1]).unwrap();
    assert!(!text.contains("native_address"));
    assert!(!text.contains("hardware_observation"));
}

#[test]
fn divergence_scope_filter_matches_either_retained_participant() {
    let stem = format!("fe2o3-debug-diagnosis-barrier-scope-{}", std::process::id());
    let kernel_path = std::env::temp_dir().join(format!("{stem}.kir"));
    let request_path = std::env::temp_dir().join(format!("{stem}.json"));
    let canonical = VerifiedCanonicalKernelIrV7::from_module(divergent_barrier_module(64)).unwrap();
    fs::write(&kernel_path, canonical.canonical_bytes()).unwrap();
    fs::write(
        &request_path,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"divergent_barrier","grid":[65,1,1],"workgroup":[65,1,1],"arguments":[]}"#,
    )
    .unwrap();
    let requests = br#"{"operation":"continue","schema":"fe2o3-debug-request-v1","request_id":21,"expected_revision":0,"max_events":1000000}
{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":22,"expected_revision":1,"filter":{"scope":{"level":"lane","workgroup":[0,0,0],"wave":1,"lane":0}},"page":{"limit":1}}
{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":23,"expected_revision":1,"filter":{"scope":{"level":"wave","workgroup":[0,0,0],"wave":1}},"page":{"limit":1}}
{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":24,"expected_revision":1,"filter":{"scope":{"level":"lane","workgroup":[0,0,0],"wave":0,"lane":0}},"page":{"limit":1}}
{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":25,"expected_revision":1,"filter":{"scope":{"level":"lane","workgroup":[1,0,0],"wave":0,"lane":0}},"page":{"limit":1}}
"#;
    let output = run_debugger(&kernel_path, &request_path, requests);
    let _ = fs::remove_file(kernel_path);
    let _ = fs::remove_file(request_path);
    assert!(
        output.status.success(),
        "debugger failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let lines = response_lines(&output.stdout);
    assert_eq!(lines.len(), 5);
    for line in &lines[1..4] {
        let response =
            decode_diagnosis_response_line_v2(line, ProtocolLimitsV1::default()).unwrap();
        let DiagnosisResponseV2::Ok { diagnoses, .. } = response else {
            panic!("diagnosis failed")
        };
        assert_eq!(diagnoses.len(), 1);
    }
    let unrelated =
        decode_diagnosis_response_line_v2(lines[4], ProtocolLimitsV1::default()).unwrap();
    let DiagnosisResponseV2::Ok { diagnoses, .. } = unrelated else {
        panic!("diagnosis failed")
    };
    assert!(diagnoses.is_empty());
}
