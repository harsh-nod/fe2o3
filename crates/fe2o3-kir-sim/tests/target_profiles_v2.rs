use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent,
    Module, Operation, OperationKind, ScalarType, Signature, TargetCapability, Terminator, Type,
    ValueDef, ValueId, VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV12, WorkgroupMemory,
    WorkgroupMemoryExtent,
};
use fe2o3_kir_sim::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn targets() -> [SimulationTargetV1; 4] {
    [
        SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
        SimulationTargetV1::little_endian(IndexWidthV1::Bits64),
        SimulationTargetV1::amdgpu_from_device_target("gfx942:xnack-").unwrap(),
        SimulationTargetV1::amdgpu_from_device_target("gfx950:xnack-").unwrap(),
    ]
}

fn module(write_only: bool, dynamic: bool, fail: bool) -> Module {
    let access = if write_only {
        AccessMode::WriteOnly
    } else {
        AccessMode::ReadWrite
    };
    let byte = Type::Scalar(ScalarType::U8);
    let mut block = BasicBlock::new(BlockId(0));
    if dynamic {
        block.operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(1),
                Type::pointer(byte.clone(), AddressSpace::Workgroup, AccessMode::ReadWrite),
            ),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: byte.clone(),
                extent: WorkgroupMemoryExtent::Dynamic,
                alignment: 1,
            }),
        ));
    }
    block.terminator = Some(if fail {
        Terminator::Unreachable
    } else {
        Terminator::Return { values: vec![] }
    });
    let mut function = Function::kernel_entry(
        "entry",
        Signature::new(
            vec![Type::pointer(byte, AddressSpace::Global, access)],
            vec![],
        ),
        vec![ValueId(0)],
        vec![block],
    );
    let mut kernel = Kernel::new(
        "profile",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    let mut module = Module::new("sim-tests::target-profile-v2");
    if dynamic {
        module.required_capabilities.extend([
            TargetCapability::WorkgroupMemory,
            TargetCapability::DynamicWorkgroupMemory,
        ]);
        function.required_capabilities = module.required_capabilities.clone();
        kernel.required_capabilities = module.required_capabilities.clone();
    }
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn owner(version: u16, write_only: bool, dynamic: bool, fail: bool) -> AdmittedSimulationModuleV1 {
    let module = module(write_only, dynamic, fail);
    match version {
        7 => AdmittedSimulationModuleV1::admit(
            VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
            SimulationLimitsV1::default(),
        ),
        12 => AdmittedSimulationModuleV1::admit_v12(
            VerifiedCanonicalKernelIrV12::from_module(module).unwrap(),
            SimulationLimitsV1::default(),
        ),
        _ => unreachable!(),
    }
    .unwrap()
}

fn request(write_only: bool) -> SimulationRequestV1 {
    let access = if write_only {
        AccessMode::WriteOnly
    } else {
        AccessMode::ReadWrite
    };
    let byte = ScalarBitsV1::new(ScalarType::U8, 19, targets()[1]).unwrap();
    let buffer = BufferArgumentV1::from_scalars(access, 1, &[byte], targets()[1]).unwrap();
    SimulationRequestV1::new(
        "profile",
        [2, 1, 1],
        [1, 1, 1],
        vec![SimulationArgumentV1::Buffer(buffer)],
    )
}

fn run(
    owner: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    dynamic: bool,
    schedule: SimulationScheduleRequestV1<'_>,
) -> Result<SimulationExecutionV1, SimulationErrorV1> {
    if dynamic {
        owner.simulate_scheduled_with_dynamic_workgroup_memory(
            request,
            DynamicWorkgroupMemoryRequestV1::new(4),
            target,
            SimulationLimitsV1::default(),
            schedule,
        )
    } else {
        owner.simulate_scheduled(request, target, SimulationLimitsV1::default(), schedule)
    }
}

fn record() -> SimulationScheduleRequestV1<'static> {
    SimulationScheduleRequestV1::RecordCanonical { max_decisions: 8 }
}

fn document(
    owner: &AdmittedSimulationModuleV1,
    target: SimulationTargetV1,
    record: &SimulationScheduleRecordV1,
) -> Vec<u8> {
    let artifact = match owner.identity().wire_version() {
        7 => PersistedSimulationScheduleArtifactV1::CanonicalKirV7,
        12 => PersistedSimulationScheduleArtifactV1::CanonicalKirV12,
        _ => unreachable!(),
    };
    PersistedSimulationScheduleDocumentV1::encode_record(
        PersistedSimulationScheduleBindingV1::new(
            artifact,
            *owner.identity(),
            [9; 32],
            317,
            target,
            SimulationLimitsV1::default(),
        ),
        record,
    )
    .unwrap()
}

#[test]
fn exact_profiles_roundtrip_and_replay_across_original_context_routes() {
    for version in [7, 12] {
        for write_only in [false, true]
            .into_iter()
            .filter(|write_only| version != 7 || !*write_only)
        {
            for dynamic in [false, true] {
                let owner = owner(version, write_only, dynamic, false);
                let request = request(write_only);
                let mut contexts = Vec::new();
                for target in targets() {
                    for schedule in [
                        record(),
                        SimulationScheduleRequestV1::RecordSeeded {
                            seed: 17,
                            max_decisions: 8,
                        },
                    ] {
                        let executed = run(&owner, &request, target, dynamic, schedule).unwrap();
                        let record = executed.schedule_record().unwrap();
                        let encoded = document(&owner, target, record);
                        let decoded =
                            PersistedSimulationScheduleDocumentV1::from_canonical_bytes(&encoded)
                                .unwrap();
                        assert_eq!(decoded.binding().target(), target);
                        assert_eq!(decoded.record(), record);
                        assert_eq!(decoded.to_canonical_bytes().unwrap(), encoded);
                        let wire: Value = serde_json::from_slice(&encoded).unwrap();
                        assert_eq!(wire["target"]["identity"], target.identity_tag());
                        let replay = run(
                            &owner,
                            &request,
                            target,
                            dynamic,
                            SimulationScheduleRequestV1::Replay(decoded.record()),
                        )
                        .unwrap();
                        assert_eq!(replay.arguments(), executed.arguments());
                        assert_eq!(
                            replay.schedule_transcript_identity(),
                            executed.schedule_transcript_identity()
                        );
                        for other in targets().into_iter().filter(|other| *other != target) {
                            assert!(matches!(
                                run(
                                    &owner,
                                    &request,
                                    other,
                                    dynamic,
                                    SimulationScheduleRequestV1::Replay(record)
                                ),
                                Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                                    kind: SimulationExecutionErrorKindV1::ScheduleReplay(
                                        SimulationScheduleReplayErrorV1::ContextMismatch
                                    ),
                                    ..
                                }))
                            ));
                        }
                    }
                    contexts.push(
                        *run(&owner, &request, target, dynamic, record())
                            .unwrap()
                            .schedule_record()
                            .unwrap()
                            .context_identity(),
                    );
                }
                for (index, context) in contexts.iter().enumerate() {
                    assert!(!contexts[..index].contains(context));
                }
            }
        }
    }
}

// Independent format/hash oracle pinned to public bfa616c996fa529da67f2f6c32e7829213914e3e.
// It does not call the production target, context, or report hashing helpers.
const LIMIT_KEYS: &[&str] = &[
    "max_canonical_bytes",
    "max_reachable_functions",
    "max_reachable_operations",
    "max_invocations",
    "max_workgroups",
    "max_scheduled_slots",
    "max_steps",
    "max_call_depth",
    "max_ssa_values",
    "max_allocations",
    "max_allocation_bytes",
    "max_total_bytes",
    "max_resident_bytes",
    "max_events",
    "max_memory_access_records",
];

fn bytes(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value);
}

fn unhex(value: &Value) -> Vec<u8> {
    let text = value.as_str().unwrap();
    assert_eq!(text.len(), 64);
    (0..64)
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
        .collect()
}

fn limits(hash: &mut Sha256, value: &Value) {
    for key in LIMIT_KEYS {
        hash.update(value[*key].as_u64().unwrap().to_le_bytes());
    }
}

fn legacy_context(
    owner: &AdmittedSimulationModuleV1,
    wire: &Value,
    width: u8,
    write_only: bool,
    dynamic: bool,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    let version = owner.identity().wire_version();
    if dynamic {
        hash.update(b"FE2O3/KIR-SIM/DYNAMIC-LDS-SCHEDULE-CONTEXT/V1\0");
        hash.update(version.to_le_bytes());
        hash.update([u8::from(write_only)]);
        hash.update(4_u32.to_le_bytes());
    } else {
        hash.update(match (version != 7, write_only) {
            (false, false) => b"FE2O3/KIR-SIM/SCHEDULE-CONTEXT/V1\0",
            (false, true) => b"FE2O3/KIR-SIM/SCHEDULE-CONTEXT/V2\0",
            (true, false) => b"FE2O3/KIR-SIM/SCHEDULE-CONTEXT/V3\0",
            (true, true) => b"FE2O3/KIR-SIM/SCHEDULE-CONTEXT/V4\0",
        });
        if version != 7 {
            hash.update(version.to_le_bytes());
        }
    }
    hash.update(owner.identity().digest());
    hash.update(owner.identity().canonical_length().to_le_bytes());
    bytes(&mut hash, b"profile");
    for value in [2_u64, 1, 1] {
        hash.update(value.to_le_bytes());
    }
    for value in [1_u32, 1, 1] {
        hash.update(value.to_le_bytes());
    }
    hash.update([0, width]);
    limits(&mut hash, &wire["limits"]);
    hash.update(1_u64.to_le_bytes());
    hash.update([1]); // Buffer argument.
    hash.update([6, if write_only { 2 } else { 1 }]); // U8 and access tags.
    hash.update(1_u32.to_le_bytes());
    bytes(&mut hash, &[19]);
    hash.update(1_u64.to_le_bytes());
    hash.update([1]);
    hash.update(0_u64.to_le_bytes()); // Shared buffers.
    hash.finalize().into()
}

fn ordered_object(value: &Value, keys: &[&str], render: impl Fn(&str, &Value) -> String) -> String {
    assert_eq!(value.as_object().unwrap().len(), keys.len());
    let fields: Vec<_> = keys
        .iter()
        .map(|key| {
            let child = value.get(*key).unwrap();
            format!(
                "{}:{}",
                serde_json::to_string(key).unwrap(),
                render(key, child)
            )
        })
        .collect();
    format!("{{{}}}", fields.join(","))
}

fn primitive(value: &Value) -> String {
    serde_json::to_string(value).unwrap()
}

fn legacy_object(key: &str, value: &Value) -> String {
    if value.is_null() {
        return "null".into();
    }
    let keys: &[&str] = match key {
        "artifact" => &["kind", "kir_sha256", "kir_canonical_bytes"],
        "request" => &["sha256", "bytes"],
        "target" => &["identity", "index_bits", "max_workgroup_invocations"],
        "limits" | "simulation_limits" => LIMIT_KEYS,
        "schedule" if value.get("seed").is_some() => &["identity", "seed"],
        "schedule" => &["identity"],
        "original_schedule" if value.get("seed").is_some() => &["kind", "seed"],
        "original_schedule" => &["kind"],
        "coverage" if value.get("decisions").is_some() => {
            &["decisions", "workgroups", "barrier_releases", "complete"]
        }
        "coverage" => &[
            "attempts",
            "matching_candidates",
            "rejected_candidates",
            "removed_decisions",
            "one_shorter_checked",
        ],
        "reduction_limits" => &[
            "max_attempts",
            "max_decisions_per_schedule",
            "max_retained_decisions",
        ],
        "fingerprint" => &[
            "class",
            "primary_invocation",
            "primary_site",
            "related_invocation",
            "related_site",
            "detail_sha256",
        ],
        "primary_invocation" | "related_invocation" => &[
            "global",
            "workgroup",
            "local",
            "workgroup_size",
            "workgroup_count",
            "launch_extent",
        ],
        "primary_site" | "related_site" => &["function", "block", "operation"],
        "decisions" | "original_decisions" | "minimized_prefix" | "reproducer_schedule"
            if value.is_array() =>
        {
            let decisions: Vec<_> = value
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| {
                    ordered_object(entry, &["workgroup", "phase", "local"], |_, child| {
                        primitive(child)
                    })
                })
                .collect();
            return format!("[{}]", decisions.join(","));
        }
        _ => return primitive(value),
    };
    ordered_object(value, keys, legacy_object)
}

fn canonical_schedule_wire(value: &Value) -> String {
    ordered_object(
        value,
        &[
            "schema",
            "artifact",
            "request",
            "target",
            "limits",
            "context_sha256",
            "transcript_sha256",
            "record_sha256",
            "schedule",
            "coverage",
            "decisions",
        ],
        legacy_object,
    )
}

fn canonical_report_wire(value: &Value) -> String {
    let keys: Vec<_> = [
        "schema",
        "kir_wire_version",
        "kir_sha256",
        "kir_canonical_bytes",
        "context_sha256",
        "index_bits",
        "target_identity",
        "simulation_limits",
        "reduction_limits",
        "original_schedule",
        "original_decisions",
        "fingerprint",
        "minimized_prefix",
        "reproducer_schedule",
        "coverage",
        "reproducer_sha256",
        "report_sha256",
        "grants_execution_authority",
        "predicts_hardware_timing",
    ]
    .into_iter()
    .filter(|key| *key != "target_identity" || value.get(*key).is_some())
    .collect();
    ordered_object(value, &keys, legacy_object)
}

#[test]
fn legacy_schedule_bytes_and_contexts_match_independent_public_format() {
    for version in [7, 12] {
        for write_only in [false, true]
            .into_iter()
            .filter(|write_only| version != 7 || !*write_only)
        {
            for dynamic in [false, true] {
                let owner = owner(version, write_only, dynamic, false);
                for target in &targets()[..2] {
                    let executed =
                        run(&owner, &request(write_only), *target, dynamic, record()).unwrap();
                    let record = executed.schedule_record().unwrap();
                    let encoded = document(&owner, *target, record);
                    let wire: Value = serde_json::from_slice(&encoded).unwrap();
                    let width = if target.index_width() == IndexWidthV1::Bits32 {
                        32
                    } else {
                        64
                    };
                    assert_eq!(
                        *record.context_identity(),
                        legacy_context(&owner, &wire, width, write_only, dynamic),
                        "V{version}, width={width}, write_only={write_only}, dynamic={dynamic}"
                    );
                    assert_eq!(wire["schema"], "fe2o3-simulation-schedule-v1");
                    assert_eq!(
                        wire["target"]["identity"],
                        if width == 32 {
                            "little_endian_index32_v1"
                        } else {
                            "amdgpu_64_little_endian_v1"
                        }
                    );
                    let expected = canonical_schedule_wire(&wire);
                    assert_eq!(encoded, expected.as_bytes());
                    let legacy = PersistedSimulationScheduleDocumentV1::from_canonical_bytes(
                        expected.as_bytes(),
                    )
                    .unwrap();
                    assert_eq!(legacy.binding().target().amd_profile(), None);
                    assert_eq!(legacy.binding().target(), *target);
                }
            }
        }
    }
}

fn failure(
    owner: &AdmittedSimulationModuleV1,
    target: SimulationTargetV1,
) -> SimulationFailureReductionReportV1 {
    owner
        .reduce_simulation_failure(
            &request(false),
            target,
            SimulationLimitsV1::default(),
            SimulationFailureScheduleV1::Seeded { seed: 17 },
            SimulationFailureReductionLimitsV1::new(10, 8, 24).unwrap(),
        )
        .unwrap()
}

fn decisions(hash: &mut Sha256, value: &Value) {
    let values = value.as_array().unwrap();
    hash.update((values.len() as u64).to_le_bytes());
    for value in values {
        for axis in value["workgroup"].as_array().unwrap() {
            hash.update(axis.as_u64().unwrap().to_le_bytes());
        }
        hash.update(value["phase"].as_u64().unwrap().to_le_bytes());
        for axis in value["local"].as_array().unwrap() {
            hash.update((axis.as_u64().unwrap() as u32).to_le_bytes());
        }
    }
}

fn fingerprint(hash: &mut Sha256, value: &Value) {
    bytes(hash, value["class"].as_str().unwrap().as_bytes());
    for prefix in ["primary", "related"] {
        let invocation = &value[format!("{prefix}_invocation")];
        hash.update([u8::from(!invocation.is_null())]);
        if !invocation.is_null() {
            for field in [
                "global",
                "workgroup",
                "local",
                "workgroup_size",
                "workgroup_count",
                "launch_extent",
            ] {
                for axis in invocation[field].as_array().unwrap() {
                    let number = axis.as_u64().unwrap();
                    if matches!(field, "local" | "workgroup_size") {
                        hash.update((number as u32).to_le_bytes());
                    } else {
                        hash.update(number.to_le_bytes());
                    }
                }
            }
        }
        let site = &value[format!("{prefix}_site")];
        hash.update([u8::from(!site.is_null())]);
        if !site.is_null() {
            bytes(hash, site["function"].as_str().unwrap().as_bytes());
            hash.update((site["block"].as_u64().unwrap() as u32).to_le_bytes());
            hash.update(
                site["operation"]
                    .as_u64()
                    .map_or(u32::MAX, |value| value as u32)
                    .to_le_bytes(),
            );
        }
    }
    hash.update(unhex(&value["detail_sha256"]));
}

fn legacy_report_identity(value: &Value) -> [u8; 32] {
    report_identity_oracle(value, None)
}

fn report_identity_oracle(value: &Value, profile: Option<&str>) -> [u8; 32] {
    let mut hash = Sha256::new();
    if let Some(profile) = profile {
        hash.update(b"FE2O3/KIR-SIM/TARGET-PROFILE/V2\0");
        bytes(&mut hash, profile.as_bytes());
    }
    hash.update(b"FE2O3/KIR-SIM/FAILURE-REDUCTION-REPORT/V1\0");
    hash.update((value["kir_wire_version"].as_u64().unwrap() as u16).to_le_bytes());
    hash.update(unhex(&value["kir_sha256"]));
    hash.update(value["kir_canonical_bytes"].as_u64().unwrap().to_le_bytes());
    hash.update(unhex(&value["context_sha256"]));
    hash.update([value["index_bits"].as_u64().unwrap() as u8]);
    limits(&mut hash, &value["simulation_limits"]);
    for key in [
        "max_attempts",
        "max_decisions_per_schedule",
        "max_retained_decisions",
    ] {
        hash.update(
            value["reduction_limits"][key]
                .as_u64()
                .unwrap()
                .to_le_bytes(),
        );
    }
    let schedule = &value["original_schedule"];
    if schedule["kind"] == "seeded" {
        hash.update([1]);
        hash.update(schedule["seed"].as_u64().unwrap().to_le_bytes());
    } else {
        assert_eq!(schedule["kind"], "canonical");
        hash.update([0]);
    }
    decisions(&mut hash, &value["original_decisions"]);
    fingerprint(&mut hash, &value["fingerprint"]);
    decisions(&mut hash, &value["minimized_prefix"]);
    decisions(&mut hash, &value["reproducer_schedule"]);
    for key in [
        "attempts",
        "matching_candidates",
        "rejected_candidates",
        "removed_decisions",
    ] {
        hash.update(value["coverage"][key].as_u64().unwrap().to_le_bytes());
    }
    hash.update([u8::from(
        value["coverage"]["one_shorter_checked"].as_bool().unwrap(),
    )]);
    hash.update(unhex(&value["reproducer_sha256"]));
    hash.finalize().into()
}

#[test]
fn reduction_profiles_are_lossless_and_legacy_reports_keep_original_bytes_and_digest() {
    let owner = owner(12, false, false, true);
    let mut identities = Vec::new();
    for target in targets() {
        let report = failure(&owner, target);
        let encoded = report.to_canonical_bytes().unwrap();
        let wire: Value = serde_json::from_slice(&encoded).unwrap();
        let decoded = SimulationFailureReductionReportV1::from_canonical_bytes(&encoded).unwrap();
        assert_eq!(decoded, report);
        assert_eq!(decoded.to_canonical_bytes().unwrap(), encoded);
        assert_eq!(
            owner
                .replay_simulation_failure_reduction(
                    &request(false),
                    target,
                    SimulationLimitsV1::default(),
                    &decoded
                )
                .unwrap(),
            report.fingerprint().clone()
        );
        assert!(!decoded.grants_execution_authority());
        assert!(!decoded.predicts_hardware_timing());
        for other in targets().into_iter().filter(|other| *other != target) {
            assert!(
                owner
                    .replay_simulation_failure_reduction(
                        &request(false),
                        other,
                        SimulationLimitsV1::default(),
                        &decoded
                    )
                    .is_err()
            );
        }
        if target.amd_profile().is_none() {
            assert_eq!(wire["schema"], "fe2o3-simulation-failure-reduction-v1");
            assert!(wire.get("target_identity").is_none());
            assert_eq!(*report.report_identity(), legacy_report_identity(&wire));
            let expected = canonical_report_wire(&wire);
            assert_eq!(encoded, expected.as_bytes());
        } else {
            assert_eq!(wire["schema"], "fe2o3-simulation-failure-reduction-v2");
            assert_eq!(wire["target_identity"], target.identity_tag());
            assert_ne!(*report.report_identity(), legacy_report_identity(&wire));
            let tag = if target == targets()[2] {
                "amdgpu_gfx942_little_endian_v2"
            } else {
                "amdgpu_gfx950_little_endian_v2"
            };
            assert_eq!(
                *report.report_identity(),
                report_identity_oracle(&wire, Some(tag))
            );
        }
        identities.push(*report.report_identity());
    }
    for (index, identity) in identities.iter().enumerate() {
        assert!(!identities[..index].contains(identity));
    }
}

#[test]
fn persisted_target_corruption_and_unknown_tags_refuse_without_legacy_promotion() {
    let owner = owner(12, false, false, false);
    let target = targets()[2];
    let executed = run(&owner, &request(false), target, false, record()).unwrap();
    let bytes = document(&owner, target, executed.schedule_record().unwrap());
    let original: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(canonical_schedule_wire(&original).as_bytes(), bytes);
    for (field, bad) in [
        ("identity", json!("amdgpu_gfx999_little_endian_v2")),
        ("index_bits", json!(32)),
        ("max_workgroup_invocations", json!(2048)),
        ("identity", Value::Null),
        ("identity", json!("x".repeat(129))),
    ] {
        let mut wire = original.clone();
        wire["target"][field] = bad;
        assert!(
            PersistedSimulationScheduleDocumentV1::from_canonical_bytes(
                canonical_schedule_wire(&wire).as_bytes()
            )
            .is_err()
        );
    }
    let text = std::str::from_utf8(&bytes).unwrap();
    for corrupted in [
        text.replacen(
            "\"index_bits\":64",
            "\"index_bits\":64,\"index_bits\":64",
            1,
        ),
        text.replacen("\"index_bits\":64", "\"index_bits\":64,\"extra\":true", 1),
        text.replacen(
            "fe2o3-simulation-schedule-v1",
            "fe2o3-simulation-schedule-v999",
            1,
        ),
        format!(" {text}"),
        text[..text.len() - 1].into(),
    ] {
        assert!(
            PersistedSimulationScheduleDocumentV1::from_canonical_bytes(corrupted.as_bytes())
                .is_err()
        );
    }
    let legacy_bytes = text.replace(target.identity_tag(), "amdgpu_64_little_endian_v1");
    let legacy =
        PersistedSimulationScheduleDocumentV1::from_canonical_bytes(legacy_bytes.as_bytes())
            .unwrap();
    assert_eq!(legacy.binding().target(), targets()[1]);
    assert_ne!(legacy.binding().target(), target);
    assert!(
        run(
            &owner,
            &request(false),
            legacy.binding().target(),
            false,
            SimulationScheduleRequestV1::Replay(legacy.record())
        )
        .is_err()
    );
    assert!(SimulationTargetV1::amdgpu_from_device_target("gfx950").is_none());
    assert!(SimulationTargetV1::amdgpu_from_device_target("gfx942:xnack+").is_none());
}

#[test]
fn reduction_schema_profile_matrix_and_identity_tampering_refuse() {
    let owner = owner(12, false, false, true);
    let encoded = failure(&owner, targets()[2]).to_canonical_bytes().unwrap();
    let original: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(canonical_report_wire(&original).as_bytes(), encoded);
    for (field, value) in [
        ("schema", json!("fe2o3-simulation-failure-reduction-v1")),
        ("schema", json!("fe2o3-simulation-failure-reduction-v999")),
        ("target_identity", json!("amdgpu_64_little_endian_v1")),
        ("target_identity", json!("amdgpu_gfx999_little_endian_v2")),
        ("target_identity", Value::Null),
        ("index_bits", json!(32)),
        ("grants_execution_authority", json!(true)),
    ] {
        let mut wire = original.clone();
        wire[field] = value;
        assert!(
            SimulationFailureReductionReportV1::from_canonical_bytes(
                canonical_report_wire(&wire).as_bytes()
            )
            .is_err()
        );
    }
    let mut missing = original.clone();
    missing.as_object_mut().unwrap().remove("target_identity");
    assert!(
        SimulationFailureReductionReportV1::from_canonical_bytes(
            canonical_report_wire(&missing).as_bytes()
        )
        .is_err()
    );
    let text = std::str::from_utf8(&encoded).unwrap();
    let other = text.replace(targets()[2].identity_tag(), targets()[3].identity_tag());
    assert_ne!(other.as_bytes(), encoded);
    assert!(matches!(
        SimulationFailureReductionReportV1::from_canonical_bytes(other.as_bytes()),
        Err(SimulationFailureReductionCodecErrorV1::InvalidIdentity)
    ));
    let legacy = failure(&owner, targets()[1]).to_canonical_bytes().unwrap();
    let legacy = std::str::from_utf8(&legacy).unwrap();
    let explicit_null = legacy.replacen(
        "\"index_bits\":64",
        "\"index_bits\":64,\"target_identity\":null",
        1,
    );
    assert!(
        SimulationFailureReductionReportV1::from_canonical_bytes(explicit_null.as_bytes()).is_err()
    );
    let downgrade = text.replace(
        "fe2o3-simulation-failure-reduction-v2",
        "fe2o3-simulation-failure-reduction-v1",
    );
    assert!(
        SimulationFailureReductionReportV1::from_canonical_bytes(downgrade.as_bytes()).is_err()
    );
    for corrupted in [
        text.replacen(
            "\"index_bits\":64",
            "\"index_bits\":64,\"index_bits\":64",
            1,
        ),
        text.replacen("\"index_bits\":64", "\"index_bits\":64,\"extra\":true", 1),
        text.replacen(targets()[2].identity_tag(), &"x".repeat(16 * 1024 + 1), 1),
        format!(" {text}"),
        text[..text.len() - 1].into(),
    ] {
        assert!(
            SimulationFailureReductionReportV1::from_canonical_bytes(corrupted.as_bytes()).is_err()
        );
    }
}

// This is the exact closed nested grammar from the public V1 schedule reader.
#[derive(serde::Deserialize)]
enum OldTargetIdentityV1 {
    #[serde(rename = "little_endian_index32_v1")]
    LittleEndianIndex32V1,
    #[serde(rename = "amdgpu_64_little_endian_v1")]
    Amdgpu64LittleEndianV1,
}

#[test]
fn old_target_grammar_refuses_explicit_profiles_in_actual_schedule_documents() {
    let owner = owner(12, false, false, false);
    for (index, target) in targets().into_iter().enumerate() {
        let executed = run(&owner, &request(false), target, false, record()).unwrap();
        let encoded = document(&owner, target, executed.schedule_record().unwrap());
        let wire: Value = serde_json::from_slice(&encoded).unwrap();
        let old = serde_json::from_value::<OldTargetIdentityV1>(wire["target"]["identity"].clone());
        assert_eq!(old.is_ok(), index < 2);
        assert!(matches!(
            PersistedSimulationScheduleDocumentV1::from_canonical_bytes(
                &encoded[..encoded.len() - 1]
            ),
            Err(PersistedSimulationScheduleCodecErrorV1::JsonSyntax)
        ));
        assert_eq!(
            PersistedSimulationScheduleDocumentV1::from_canonical_bytes(&encoded)
                .unwrap()
                .binding()
                .target(),
            target
        );
        assert!(matches!(
            run(
                &owner,
                &request(false),
                target,
                false,
                SimulationScheduleRequestV1::RecordCanonical { max_decisions: 1 }
            ),
            Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                kind: SimulationExecutionErrorKindV1::ScheduleDecisionLimit { .. },
                ..
            }))
        ));
    }
}

#[test]
fn exact_profiles_do_not_bypass_legacy_write_only_encoding_limits() {
    for dynamic in [false, true] {
        assert!(matches!(
            VerifiedCanonicalKernelIrV7::from_module(module(true, dynamic, false)),
            Err(fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV7::Encode(
                fe2o3_kernel_ir::KernelIrEncodeError::UnsupportedInVersion {
                    version: 7,
                    feature: "write-only pointer and slice types",
                }
            ))
        ));
    }
}
