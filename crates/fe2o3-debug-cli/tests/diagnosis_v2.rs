use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use fe2o3_debug_protocol::*;
use fe2o3_kernel_ir::{
    AddressSpace, Axis, BarrierSemantics, BasicBlock, BlockId, ComparePredicate, Constant,
    Convergence, DebugSourceMapBindingV1, DebugSourceMapDocumentV1, DebugSourceMapDocumentV2,
    DebugSourceMapFileV1, DebugSourceMapKirSiteV1, DebugSourceMapSiteV1, DebugSourceMapSpanV1,
    Function, IndexKind, IntrinsicKind, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent,
    MemoryAccess, MemoryOrdering, Module, Operation, OperationKind, Signature,
    SimulationCompilerExecutionBindingV1, SimulationProductionKirIdentityV1,
    SimulationSourceLineageV1, SynchronizationScope, TargetCapability, Terminator, Type, ValueDef,
    ValueId, VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV8, VerifiedSimulationBundleV1,
    VerifiedSimulationBundleV2, WorkgroupBarrier, decode_module_v7,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is in workspace/crates")
        .to_owned()
}

fn run_debugger(kernel: &Path, request: &Path, requests: &[u8]) -> std::process::Output {
    run_debugger_with_args(kernel, request, &[], requests)
}

fn run_debugger_with_args(
    kernel: &Path,
    request: &Path,
    args: &[&str],
    requests: &[u8],
) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .args(["sim", "--kir-v7"])
        .arg(kernel)
        .arg("--request")
        .arg(request)
        .args(args)
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

fn run_bundle_debugger(bundle: &Path, request: &Path, requests: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-debug"))
        .args(["sim", "--bundle-v2"])
        .arg(bundle)
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

fn hex_identity(identity: [u8; 32]) -> String {
    identity.iter().map(|byte| format!("{byte:02x}")).collect()
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
    assert_eq!(
        diagnosis.input.configuration_identity,
        session.configuration_identity
    );
    assert!(matches!(
        diagnosis.input.dispatch_request,
        DiagnosisFactV2::Declared { .. }
    ));
    assert!(matches!(
        diagnosis.input.canonical_kir_v7,
        DiagnosisFactV2::Declared { .. }
    ));
    assert!(matches!(
        diagnosis.input.simulation_bundle,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided
        }
    ));
    assert!(matches!(
        diagnosis.input.source_map_v2,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided
        }
    ));
    assert!(matches!(
        diagnosis.source_operation,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided
        }
    ));
    assert!(matches!(
        diagnosis.input.finalized_artifact,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoArtifactAuthority
        }
    ));
    assert!(matches!(
        diagnosis.input.property_proof,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoProofAuthority
        }
    ));
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
    let DiagnosisFactV2::Observed { value: region } = &diagnosis.memory_region else {
        panic!("memory region evidence is missing")
    };
    let DiagnosisFactV2::Declared { value: contract } = &region.allocation_contract else {
        panic!("allocation contract is missing")
    };
    assert_eq!(contract.address_space, AddressSpaceV1::Global);
    assert_eq!(contract.access, DiagnosisAccessModeV2::ReadWrite);
    assert_eq!(contract.alignment, 4);
    assert_eq!(contract.allocation_bytes, 4);
    assert!(matches!(
        contract.abi_arguments.as_slice(),
        [DiagnosisAbiArgumentV2 {
            ordinal: 0,
            backing: None,
            element: DiagnosisScalarTypeV2::U32,
            address_space: AddressSpaceV1::Global,
            access: DiagnosisAccessModeV2::ReadWrite,
            view_offset: 0,
            view_bytes: 4,
            ..
        }]
    ));
    assert!(matches!(
        region.logical_element,
        DiagnosisFactV2::Inferred {
            value: DiagnosisLogicalElementV2 {
                element_index: 1,
                ..
            },
            basis: DiagnosisInferenceBasisV2::AbiViewBounds,
        }
    ));
    assert!(!diagnosis.evidence.citations.is_empty());
    assert!(matches!(
        diagnosis.barrier,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NotApplicable
        }
    ));
}

#[test]
fn caller_bound_source_map_v2_resolves_exact_oob_operation_without_authority() {
    let root = workspace_root();
    let stem = format!("fe2o3-debug-diagnosis-source-v2-{}", std::process::id());
    let request_path = std::env::temp_dir().join(format!("{stem}-request.json"));
    let map_path = std::env::temp_dir().join(format!("{stem}-source-map.json"));
    fs::write(
        &request_path,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"fill","grid":[4,1,1],"workgroup":[64,1,1],"arguments":[{"kind":"buffer","element":"u32","access":"read_write","alignment":4,"bytes":"0x00000000"}]}"#,
    )
    .unwrap();
    let source_map_v1 = DebugSourceMapDocumentV1::from_json_bytes(
        &fs::read(root.join("crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json")).unwrap(),
    )
    .unwrap();
    let bundle_subject = source_map_v1.binding().bundle_subject_identity();
    let source_map_v2 = DebugSourceMapDocumentV2::new(
        source_map_v1.binding(),
        source_map_v1.files().to_vec(),
        source_map_v1.sites().to_vec(),
        source_map_v1.eliminated().to_vec(),
        vec![],
        vec![],
    )
    .unwrap();
    fs::write(&map_path, source_map_v2.to_canonical_json_bytes().unwrap()).unwrap();
    let map_path_text = map_path.to_str().unwrap();
    let subject_text = hex_identity(bundle_subject);
    let requests = br#"{"operation":"continue","schema":"fe2o3-debug-request-v1","request_id":31,"expected_revision":0,"max_events":1000000}
{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":32,"expected_revision":1,"filter":{"class":"memory_out_of_bounds"},"page":{"limit":1}}
"#;
    let output = run_debugger_with_args(
        &root.join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir"),
        &request_path,
        &[
            "--source-map",
            map_path_text,
            "--source-bundle-subject",
            &subject_text,
        ],
        requests,
    );
    let _ = fs::remove_file(request_path);
    let _ = fs::remove_file(map_path);
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
    let DiagnosisResponseV2::Ok { diagnoses, .. } = response else {
        panic!("diagnosis failed")
    };
    let [diagnosis] = diagnoses.as_slice() else {
        panic!("expected one diagnosis")
    };
    let DiagnosisFactV2::Declared { value: map } = diagnosis.input.source_map_v2 else {
        panic!("source map V2 evidence is missing")
    };
    assert_eq!(map.bundle_subject_identity.as_bytes(), bundle_subject);
    assert_eq!(map.provenance, SourceMapProvenanceV1::CallerBound);
    assert!(matches!(
        diagnosis.input.simulation_bundle,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::InputNotProvided
        }
    ));
    let DiagnosisFactV2::Declared { value: source } = &diagnosis.source_operation else {
        panic!("source operation evidence is missing")
    };
    assert_eq!(source.bundle_subject_identity, map.bundle_subject_identity);
    assert_eq!(
        source.kir_site,
        KirSiteV1 {
            function_ordinal: 0,
            block_ordinal: 0,
            point: KirSitePointV1::Operation {
                operation_ordinal: 3
            }
        }
    );
    assert_eq!(source.location.map_identity, map.identity);
    assert_eq!(
        source.location.provenance,
        SourceMapProvenanceV1::CallerBound
    );
    assert_eq!(source.location.byte_start, 97);
    assert_eq!(source.location.byte_end, 125);
    assert!(matches!(
        diagnosis.input.finalized_artifact,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoArtifactAuthority
        }
    ));
    assert!(matches!(
        diagnosis.input.property_proof,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoProofAuthority
        }
    ));
}

#[test]
fn production_bundle_v2_oob_binds_envelope_kir_abi_lineage_and_exact_map_member() {
    let root = workspace_root();
    let stem = format!("fe2o3-debug-diagnosis-bundle-v2-{}", std::process::id());
    let request_path = std::env::temp_dir().join(format!("{stem}-request.json"));
    let bundle_path = std::env::temp_dir().join(format!("{stem}.fe2sim"));
    fs::write(
        &request_path,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"fill","grid":[4,1,1],"workgroup":[64,1,1],"arguments":[{"kind":"buffer","element":"u32","access":"read_write","alignment":4,"bytes":"0x00000000"}]}"#,
    )
    .unwrap();
    let kir = fs::read(root.join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir")).unwrap();
    let canonical_v7 = VerifiedCanonicalKernelIrV7::from_canonical_bytes(kir.clone()).unwrap();
    let production_v8 =
        VerifiedCanonicalKernelIrV8::from_module(decode_module_v7(&kir).unwrap()).unwrap();
    let inner = VerifiedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([3; 32], 33, [4; 32], 44).unwrap(),
        SimulationProductionKirIdentityV1::v8(
            *production_v8.identity().digest(),
            production_v8.identity().canonical_length(),
        )
        .unwrap(),
        "gfx942:xnack-",
        canonical_v7,
        None,
    )
    .unwrap();
    let source_v1 = DebugSourceMapDocumentV1::from_json_bytes(
        &fs::read(root.join("crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json")).unwrap(),
    )
    .unwrap();
    let binding = DebugSourceMapBindingV1::new(
        *inner.subject_identity(),
        *inner.canonical_kir_v7_identity().digest(),
        inner.canonical_kir_v7_identity().canonical_length(),
    )
    .unwrap();
    let source_v2 = DebugSourceMapDocumentV2::new(
        binding,
        source_v1.files().to_vec(),
        source_v1.sites().to_vec(),
        source_v1.eliminated().to_vec(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let bundle = VerifiedSimulationBundleV2::new(inner, source_v2).unwrap();
    let map_identity = OpaqueIdentityV1::new(*bundle.debug_map_identity()).unwrap();
    let envelope_identity = OpaqueIdentityV1::new(*bundle.identity().as_bytes()).unwrap();
    let subject_identity = OpaqueIdentityV1::new(*bundle.subject_identity()).unwrap();
    assert_ne!(envelope_identity, subject_identity);
    fs::write(&bundle_path, bundle.canonical_bytes()).unwrap();

    let requests = br#"{"operation":"continue","schema":"fe2o3-debug-request-v1","request_id":41,"expected_revision":0,"max_events":1000000}
{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":42,"expected_revision":1,"filter":{"class":"memory_out_of_bounds"},"page":{"limit":1}}
"#;
    let output = run_bundle_debugger(&bundle_path, &request_path, requests);
    let _ = fs::remove_file(request_path);
    let _ = fs::remove_file(bundle_path);
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
    let [diagnosis] = diagnoses.as_slice() else {
        panic!("expected one diagnosis")
    };
    assert!(session.simulated);
    assert!(!session.hardware_observed);
    assert_eq!(
        diagnosis.input.configuration_identity,
        session.configuration_identity
    );
    assert!(matches!(
        diagnosis.input.simulation_bundle,
        DiagnosisFactV2::Declared {
            value: DiagnosisBundleReferenceV2 {
                envelope_version: 2,
                identity,
                subject_identity: subject,
            }
        } if identity == envelope_identity && subject == subject_identity
    ));
    assert!(matches!(
        diagnosis.input.production_kir,
        DiagnosisFactV2::Declared {
            value: DiagnosisVersionedContentReferenceV2 { version: 8, .. }
        }
    ));
    assert!(matches!(
        diagnosis.input.kernel_abi_identity,
        DiagnosisFactV2::Declared { .. }
    ));
    assert!(matches!(
        diagnosis.input.source_lineage,
        DiagnosisFactV2::Declared { .. }
    ));
    let DiagnosisFactV2::Declared { value: map } = diagnosis.input.source_map_v2 else {
        panic!("bundle source map is missing")
    };
    assert_eq!(map.identity, map_identity);
    assert_eq!(map.bundle_subject_identity, subject_identity);
    assert_eq!(map.provenance, SourceMapProvenanceV1::CompilerBundleBound);
    let DiagnosisFactV2::Declared { value: source } = &diagnosis.source_operation else {
        panic!("exact source operation is missing")
    };
    assert_eq!(source.bundle_subject_identity, subject_identity);
    assert_eq!(source.location.map_identity, map_identity);
    assert_eq!(
        source.location.provenance,
        SourceMapProvenanceV1::CompilerBundleBound
    );
    assert_eq!(source.membership.member_count, map.operation_members);
    assert!(matches!(
        diagnosis.input.finalized_artifact,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoArtifactAuthority
        }
    ));
    assert!(matches!(
        diagnosis.input.property_proof,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoProofAuthority
        }
    ));
    let DiagnosisFactV2::Observed { value: region } = &diagnosis.memory_region else {
        panic!("memory region is missing")
    };
    assert!(matches!(
        region.legal_bounds,
        DiagnosisFactV2::Inferred {
            value: DiagnosisLegalBoundsPropertyV2 {
                satisfied: false,
                ..
            },
            basis: DiagnosisInferenceBasisV2::AbiViewBounds,
        }
    ));
    assert!(diagnosis.evidence.citations.iter().any(|citation| {
        citation.source == DiagnosisEvidenceSourceV2::SourceMapOperationRecord
            && citation.source_record_identity == source.membership.member_identity
    }));
}

fn oob_module(
    kernel: &str,
    abi_access: fe2o3_kernel_ir::AccessMode,
    parameter_count: usize,
) -> Module {
    let scalar = Type::Scalar(fe2o3_kernel_ir::ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, abi_access);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        operation(2, scalar.clone(), OperationKind::Constant(Constant::U32(1))),
        operation(
            3,
            pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(2),
            },
        ),
        operation(
            4,
            scalar,
            OperationKind::Load {
                pointer: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry_name = format!("{kernel}_impl");
    let entry = Function::kernel_entry(
        entry_name.clone(),
        Signature::new(vec![pointer; parameter_count], vec![]),
        (0..parameter_count)
            .map(|ordinal| ValueId(u32::try_from(ordinal).unwrap()))
            .collect(),
        vec![block],
    );
    let mut module = Module::new(format!("debug-tests::{kernel}"));
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        kernel,
        entry_name,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn production_bundle_v2_oob_diagnosis(
    suffix: &str,
    module: Module,
    request: &[u8],
) -> DiagnosisViewV2 {
    let stem = format!(
        "fe2o3-debug-diagnosis-{suffix}-bundle-v2-{}",
        std::process::id()
    );
    let request_path = std::env::temp_dir().join(format!("{stem}-request.json"));
    let bundle_path = std::env::temp_dir().join(format!("{stem}.fe2sim"));
    let canonical_v7 = VerifiedCanonicalKernelIrV7::from_module(module.clone()).unwrap();
    let production_v8 = VerifiedCanonicalKernelIrV8::from_module(module).unwrap();
    let inner = VerifiedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([7; 32], 77, [8; 32], 88).unwrap(),
        SimulationProductionKirIdentityV1::v8(
            *production_v8.identity().digest(),
            production_v8.identity().canonical_length(),
        )
        .unwrap(),
        "gfx942:xnack-",
        canonical_v7,
        None,
    )
    .unwrap();
    let binding = DebugSourceMapBindingV1::new(
        *inner.subject_identity(),
        *inner.canonical_kir_v7_identity().digest(),
        inner.canonical_kir_v7_identity().canonical_length(),
    )
    .unwrap();
    let file = DebugSourceMapFileV1::new([0x66; 32], 24, "oob.fe".to_owned()).unwrap();
    let span = DebugSourceMapSpanV1::new([0x66; 32], 0, 12, 1, 1).unwrap();
    let site =
        DebugSourceMapSiteV1::new(DebugSourceMapKirSiteV1::operation(0, 0, 2), vec![span]).unwrap();
    let source_v2 = DebugSourceMapDocumentV2::new(
        binding,
        vec![file],
        vec![site],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let bundle = VerifiedSimulationBundleV2::new(inner, source_v2).unwrap();
    fs::write(&bundle_path, bundle.canonical_bytes()).unwrap();
    fs::write(&request_path, request).unwrap();
    let requests = br#"{"operation":"continue","schema":"fe2o3-debug-request-v1","request_id":61,"expected_revision":0,"max_events":1000000}
{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":62,"expected_revision":1,"filter":{"class":"memory_out_of_bounds"},"page":{"limit":1}}
"#;
    let output = run_bundle_debugger(&bundle_path, &request_path, requests);
    let _ = fs::remove_file(request_path);
    let _ = fs::remove_file(bundle_path);
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
    let DiagnosisResponseV2::Ok { diagnoses, .. } = response else {
        panic!("diagnosis failed")
    };
    assert_eq!(diagnoses.len(), 1);
    diagnoses.into_iter().next().unwrap()
}

#[test]
fn production_bundle_v2_shared_backing_preserves_narrow_faulting_view() {
    let diagnosis = production_bundle_v2_oob_diagnosis(
        "alias",
        oob_module(
            "aliased_view_oob",
            fe2o3_kernel_ir::AccessMode::ReadWrite,
            2,
        ),
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"aliased_view_oob","grid":[1,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer_view","backing":9,"element":"u32","access":"read_write","alignment":4,"byte_offset":4,"elements":1},{"kind":"buffer_view","backing":9,"element":"u32","access":"read_write","alignment":4,"byte_offset":0,"elements":3}],"shared_buffers":[{"id":9,"element":"u32","access":"read_write","alignment":4,"bytes":"0x0a000000140000001e000000"}]}"#,
    );
    let DiagnosisFactV2::Observed { value: region } = &diagnosis.memory_region else {
        panic!("memory diagnosis is unavailable")
    };
    assert_eq!(region.requested_offset, 8);
    assert_eq!(region.requested_bytes, 4);
    assert_eq!(region.legal_offset, 4);
    assert_eq!(region.legal_bytes, 4);
    assert_eq!(region.allocation_bytes, 12);
    assert!(region.requested_offset + region.requested_bytes <= region.allocation_bytes);
    let DiagnosisFactV2::Declared { value: contract } = &region.allocation_contract else {
        panic!("allocation contract is unavailable")
    };
    assert_eq!(contract.abi_arguments.len(), 2);
    assert_eq!(contract.abi_arguments[0].backing, Some(9));
    assert_eq!(contract.abi_arguments[1].backing, Some(9));
    assert_ne!(
        contract.abi_arguments[0].view_bytes,
        contract.abi_arguments[1].view_bytes
    );
    assert!(matches!(
        region.abi_argument,
        DiagnosisFactV2::Declared {
            value: DiagnosisAbiArgumentV2 {
                ordinal: 0,
                backing: Some(9),
                view_offset: 4,
                view_bytes: 4,
                ..
            }
        }
    ));
    assert!(diagnosis.evidence.retained.is_some());
}

struct ExpectedOobAccessContract {
    backing: Option<u32>,
    legal_offset: u64,
    legal_bytes: u64,
    allocation_bytes: u64,
    required: DiagnosisAccessModeV2,
    supplied: DiagnosisAccessModeV2,
    backing_access: DiagnosisAccessModeV2,
}

fn assert_oob_access_contract(diagnosis: &DiagnosisViewV2, expected: ExpectedOobAccessContract) {
    let DiagnosisFactV2::Observed { value: region } = &diagnosis.memory_region else {
        panic!("memory diagnosis is unavailable")
    };
    assert_eq!(region.allocation.ordinal, 1);
    assert_eq!(region.allocation.generation, 0);
    assert_eq!(
        region.requested_offset,
        expected.legal_offset + expected.legal_bytes
    );
    assert_eq!(region.requested_bytes, 4);
    assert_eq!(region.legal_offset, expected.legal_offset);
    assert_eq!(region.legal_bytes, expected.legal_bytes);
    assert_eq!(region.allocation_bytes, expected.allocation_bytes);
    let DiagnosisFactV2::Declared { value: contract } = &region.allocation_contract else {
        panic!("allocation contract is unavailable")
    };
    assert_eq!(contract.access, expected.backing_access);
    let DiagnosisFactV2::Declared { value: argument } = &region.abi_argument else {
        panic!("ABI argument is unavailable")
    };
    assert_eq!(argument.ordinal, 0);
    assert_eq!(argument.backing, expected.backing);
    assert_eq!(argument.access, expected.required);
    assert_eq!(argument.supplied_access, expected.supplied);
    assert_eq!(argument.view_offset, expected.legal_offset);
    assert_eq!(argument.view_bytes, expected.legal_bytes);
    assert_eq!(contract.abi_arguments[0], *argument);
    assert!(diagnosis.evidence.retained.is_some());
}

#[test]
fn production_bundle_v2_preserves_abi_view_and_backing_access_narrowing() {
    let ordinary = production_bundle_v2_oob_diagnosis(
        "ordinary-rw-ro-abi",
        oob_module(
            "ordinary_rw_ro_abi_oob",
            fe2o3_kernel_ir::AccessMode::ReadOnly,
            1,
        ),
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"ordinary_rw_ro_abi_oob","grid":[1,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer","element":"u32","access":"read_write","alignment":4,"bytes":"0x0a000000"}]}"#,
    );
    assert_oob_access_contract(
        &ordinary,
        ExpectedOobAccessContract {
            backing: None,
            legal_offset: 0,
            legal_bytes: 4,
            allocation_bytes: 4,
            required: DiagnosisAccessModeV2::ReadOnly,
            supplied: DiagnosisAccessModeV2::ReadWrite,
            backing_access: DiagnosisAccessModeV2::ReadWrite,
        },
    );

    let read_write_view = production_bundle_v2_oob_diagnosis(
        "rw-view-ro-abi",
        oob_module(
            "rw_view_ro_abi_oob",
            fe2o3_kernel_ir::AccessMode::ReadOnly,
            2,
        ),
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"rw_view_ro_abi_oob","grid":[1,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer_view","backing":9,"element":"u32","access":"read_write","alignment":4,"byte_offset":4,"elements":1},{"kind":"buffer_view","backing":9,"element":"u32","access":"read_write","alignment":4,"byte_offset":0,"elements":3}],"shared_buffers":[{"id":9,"element":"u32","access":"read_write","alignment":4,"bytes":"0x0a000000140000001e000000"}]}"#,
    );
    assert_oob_access_contract(
        &read_write_view,
        ExpectedOobAccessContract {
            backing: Some(9),
            legal_offset: 4,
            legal_bytes: 4,
            allocation_bytes: 12,
            required: DiagnosisAccessModeV2::ReadOnly,
            supplied: DiagnosisAccessModeV2::ReadWrite,
            backing_access: DiagnosisAccessModeV2::ReadWrite,
        },
    );

    let read_only_view = production_bundle_v2_oob_diagnosis(
        "ro-view-rw-backing",
        oob_module(
            "ro_view_rw_backing_oob",
            fe2o3_kernel_ir::AccessMode::ReadOnly,
            2,
        ),
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"ro_view_rw_backing_oob","grid":[1,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer_view","backing":9,"element":"u32","access":"read_only","alignment":4,"byte_offset":4,"elements":1},{"kind":"buffer_view","backing":9,"element":"u32","access":"read_only","alignment":4,"byte_offset":0,"elements":3}],"shared_buffers":[{"id":9,"element":"u32","access":"read_write","alignment":4,"bytes":"0x0a000000140000001e000000"}]}"#,
    );
    assert_oob_access_contract(
        &read_only_view,
        ExpectedOobAccessContract {
            backing: Some(9),
            legal_offset: 4,
            legal_bytes: 4,
            allocation_bytes: 12,
            required: DiagnosisAccessModeV2::ReadOnly,
            supplied: DiagnosisAccessModeV2::ReadOnly,
            backing_access: DiagnosisAccessModeV2::ReadWrite,
        },
    );
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
                semantics,
                lds_epoch,
                observed_arrivals,
                expected_participants,
                expected_participant_set,
                arrived_participants,
                waiting_participants,
                exited_participants,
                ..
            },
    } = &diagnosis.barrier
    else {
        panic!("barrier evidence is missing")
    };
    assert!(matches!(phase, DiagnosisFactV2::Observed { value: 0 }));
    assert!(matches!(
        semantics,
        DiagnosisFactV2::Declared {
            value: DiagnosisBarrierSemanticsV2 {
                memory_scope: DiagnosisSynchronizationScopeV2::Workgroup,
                ordering: DiagnosisMemoryOrderingV2::AcquireRelease,
                address_spaces,
            }
        } if address_spaces == &[AddressSpaceV1::Workgroup]
    ));
    assert!(matches!(
        lds_epoch.current,
        DiagnosisFactV2::Inferred {
            value: 0,
            basis: DiagnosisInferenceBasisV2::BarrierPhase
        }
    ));
    assert!(matches!(
        lds_epoch.after_release,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::BarrierNotReleased
        }
    ));
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
    let DiagnosisFactV2::Inferred {
        value: expected_set,
        ..
    } = expected_participant_set
    else {
        panic!("expected participant set is missing")
    };
    let DiagnosisFactV2::Observed { value: arrived } = arrived_participants else {
        panic!("arrived participant set is missing")
    };
    let DiagnosisFactV2::Observed { value: waiting } = waiting_participants else {
        panic!("waiting participant set is missing")
    };
    let DiagnosisFactV2::Observed { value: exited } = exited_participants else {
        panic!("exited participant set is missing")
    };
    assert_eq!(expected_set.len(), 2);
    assert!(expected_set.iter().all(|participant| matches!(
        participant.local_workitem,
        DiagnosisFactV2::Inferred {
            basis: DiagnosisInferenceBasisV2::LaunchGeometry,
            ..
        }
    )));
    assert_eq!(arrived, waiting);
    assert_eq!(waiting.len(), 1);
    assert_eq!(exited.len(), 1);
    for participant in waiting.iter().chain(exited) {
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
fn production_bundle_v2_barrier_binds_scheduled_operation_and_participant_sets() {
    let stem = format!(
        "fe2o3-debug-diagnosis-barrier-bundle-v2-{}",
        std::process::id()
    );
    let request_path = std::env::temp_dir().join(format!("{stem}-request.json"));
    let bundle_path = std::env::temp_dir().join(format!("{stem}.fe2sim"));
    let module = divergent_barrier_module(0);
    let canonical_v7 = VerifiedCanonicalKernelIrV7::from_module(module.clone()).unwrap();
    let production_v8 = VerifiedCanonicalKernelIrV8::from_module(module).unwrap();
    let inner = VerifiedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([5; 32], 55, [6; 32], 66).unwrap(),
        SimulationProductionKirIdentityV1::v8(
            *production_v8.identity().digest(),
            production_v8.identity().canonical_length(),
        )
        .unwrap(),
        "gfx942:xnack-",
        canonical_v7,
        None,
    )
    .unwrap();
    let binding = DebugSourceMapBindingV1::new(
        *inner.subject_identity(),
        *inner.canonical_kir_v7_identity().digest(),
        inner.canonical_kir_v7_identity().canonical_length(),
    )
    .unwrap();
    let file = DebugSourceMapFileV1::new([0x55; 32], 32, "barrier.fe".to_owned()).unwrap();
    let span = DebugSourceMapSpanV1::new([0x55; 32], 0, 10, 1, 1).unwrap();
    let site =
        DebugSourceMapSiteV1::new(DebugSourceMapKirSiteV1::operation(0, 2, 0), vec![span]).unwrap();
    let source_v2 = DebugSourceMapDocumentV2::new(
        binding,
        vec![file],
        vec![site],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let bundle = VerifiedSimulationBundleV2::new(inner, source_v2).unwrap();
    let envelope_identity = OpaqueIdentityV1::new(*bundle.identity().as_bytes()).unwrap();
    let subject_identity = OpaqueIdentityV1::new(*bundle.subject_identity()).unwrap();
    let map_identity = OpaqueIdentityV1::new(*bundle.debug_map_identity()).unwrap();
    fs::write(&bundle_path, bundle.canonical_bytes()).unwrap();
    fs::write(
        &request_path,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"divergent_barrier","grid":[2,1,1],"workgroup":[2,1,1],"arguments":[]}"#,
    )
    .unwrap();
    let requests = br#"{"operation":"continue","schema":"fe2o3-debug-request-v1","request_id":51,"expected_revision":0,"max_events":1000000}
{"operation":"diagnose","schema":"fe2o3-debug-diagnosis-request-v2","request_id":52,"expected_revision":1,"filter":{"class":"workgroup_barrier_divergence"},"page":{"limit":1}}
"#;
    let output = run_bundle_debugger(&bundle_path, &request_path, requests);
    let _ = fs::remove_file(request_path);
    let _ = fs::remove_file(bundle_path);
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
    let DiagnosisResponseV2::Ok { diagnoses, .. } = response else {
        panic!("diagnosis failed")
    };
    let [diagnosis] = diagnoses.as_slice() else {
        panic!("expected one diagnosis")
    };
    assert!(matches!(
        diagnosis.input.simulation_bundle,
        DiagnosisFactV2::Declared {
            value: DiagnosisBundleReferenceV2 {
                envelope_version: 2,
                identity,
                subject_identity: subject,
            }
        } if identity == envelope_identity && subject == subject_identity
    ));
    let DiagnosisFactV2::Declared { value: map } = diagnosis.input.source_map_v2 else {
        panic!("bundle source map is missing")
    };
    assert_eq!(map.identity, map_identity);
    assert_eq!(map.bundle_subject_identity, subject_identity);
    assert_eq!(map.provenance, SourceMapProvenanceV1::CompilerBundleBound);
    let DiagnosisFactV2::Declared { value: source } = &diagnosis.source_operation else {
        panic!("scheduled source operation is missing")
    };
    assert_eq!(source.bundle_subject_identity, subject_identity);
    assert_eq!(source.location.map_identity, map_identity);
    assert_eq!(source.location.file_identity.as_bytes(), [0x55; 32]);
    assert_eq!(source.location.byte_start, 0);
    assert_eq!(source.location.byte_end, 10);
    assert_eq!(
        source.kir_site,
        KirSiteV1 {
            function_ordinal: 0,
            block_ordinal: 2,
            point: KirSitePointV1::Operation {
                operation_ordinal: 0
            }
        }
    );
    let DiagnosisFactV2::Observed {
        value:
            DiagnosisBarrierV2::Divergence {
                expected_participant_set,
                arrived_participants,
                waiting_participants,
                exited_participants,
                ..
            },
    } = &diagnosis.barrier
    else {
        panic!("barrier participant evidence is missing")
    };
    let DiagnosisFactV2::Inferred {
        value: expected, ..
    } = expected_participant_set
    else {
        panic!("expected participant inventory is missing")
    };
    let DiagnosisFactV2::Observed { value: arrived } = arrived_participants else {
        panic!("arrival inventory is missing")
    };
    let DiagnosisFactV2::Observed { value: waiting } = waiting_participants else {
        panic!("waiting inventory is missing")
    };
    let DiagnosisFactV2::Observed { value: exited } = exited_participants else {
        panic!("exited inventory is missing")
    };
    assert_eq!(expected.len(), 2);
    assert!(expected.iter().all(|participant| matches!(
        participant.local_workitem,
        DiagnosisFactV2::Inferred {
            basis: DiagnosisInferenceBasisV2::LaunchGeometry,
            ..
        }
    )));
    assert_eq!(arrived, waiting);
    assert_eq!(waiting.len(), 1);
    assert_eq!(exited.len(), 1);
    assert!(matches!(
        diagnosis.input.finalized_artifact,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoArtifactAuthority
        }
    ));
    assert!(matches!(
        diagnosis.input.property_proof,
        DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::NoProofAuthority
        }
    ));
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
