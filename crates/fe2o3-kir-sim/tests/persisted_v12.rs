use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, Constant, Function, IndexKind,
    IntrinsicKind, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module,
    Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV11, VerifiedCanonicalKernelIrV12,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, IndexWidthV1,
    PersistedSimulationScheduleArtifactV1, PersistedSimulationScheduleBindingV1,
    PersistedSimulationScheduleCodecErrorV1, PersistedSimulationScheduleDocumentV1, ScalarBitsV1,
    SimulationArgumentV1, SimulationErrorV1, SimulationExecutionErrorKindV1,
    SimulationExecutionErrorV1, SimulationExecutionV1, SimulationKernelIrIdentityV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationScheduleReplayErrorV1,
    SimulationScheduleRequestV1, SimulationTargetV1,
};

const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

fn module(bias: u32) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let output = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar.clone()),
            OperationKind::Constant(Constant::U32(bias)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), scalar.clone()),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(1),
                rhs: ValueId(3),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), output.clone()),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(5),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("sim-tests::persisted-v12");
    module.functions.push(Function::kernel_entry(
        "write_impl",
        Signature::new(vec![output, scalar], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "write",
        "write_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn admitted(bias: u32) -> AdmittedSimulationModuleV1 {
    AdmittedSimulationModuleV1::admit_v12(
        VerifiedCanonicalKernelIrV12::from_module(module(bias)).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap()
}

fn request(value: u32) -> SimulationRequestV1 {
    let buffer = BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        4,
        &[ScalarBitsV1::u32(u32::MAX); 8],
        TARGET,
    )
    .unwrap();
    SimulationRequestV1::new(
        "write",
        [6, 1, 1],
        [4, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(value)),
        ],
    )
}

fn recorded(
    admitted: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
    schedule: SimulationScheduleRequestV1<'_>,
) -> (SimulationExecutionV1, PersistedSimulationScheduleDocumentV1) {
    let execution = admitted
        .simulate_scheduled(request, target, limits, schedule)
        .unwrap();
    let binding = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::CanonicalKirV12,
        *admitted.identity(),
        [9; 32],
        317,
        target,
        limits,
    );
    let bytes = PersistedSimulationScheduleDocumentV1::encode_record(
        binding,
        execution.schedule_record().unwrap(),
    )
    .unwrap();
    let document = PersistedSimulationScheduleDocumentV1::from_canonical_bytes(&bytes).unwrap();
    assert_eq!(document.binding(), binding);
    assert_eq!(document.to_canonical_bytes().unwrap(), bytes);
    assert_eq!(document.record(), execution.schedule_record().unwrap());
    (execution, document)
}

#[test]
fn exact_v12_persisted_schedules_round_trip_and_replay_with_bounded_decisions() {
    let admitted = admitted(7);
    let request = request(9);
    let limits = SimulationLimitsV1::default();
    for target in [
        TARGET,
        SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
    ] {
        for schedule in [
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 6 },
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 0x5eed,
                max_decisions: 6,
            },
        ] {
            let (execution, document) = recorded(&admitted, &request, target, limits, schedule);
            assert_eq!(document.binding().kir_wire_version(), 12);
            assert_eq!(
                document.binding().kir_sha256(),
                *admitted.identity().digest()
            );
            assert_eq!(document.record().decisions().len(), 6);
            let text = String::from_utf8(document.to_canonical_bytes().unwrap()).unwrap();
            assert!(text.contains("\"kind\":\"canonical_kir_v12\""));
            let replay = admitted
                .simulate_scheduled(
                    &request,
                    target,
                    limits,
                    SimulationScheduleRequestV1::Replay(document.record()),
                )
                .unwrap();
            assert_eq!(replay.identity().wire_version(), 12);
            assert_eq!(replay.arguments(), execution.arguments());
            assert_eq!(replay.schedule_coverage(), execution.schedule_coverage());
            assert_eq!(
                replay.schedule_transcript_identity(),
                execution.schedule_transcript_identity(),
            );
            let output: Vec<_> = replay
                .buffer(0)
                .unwrap()
                .bytes()
                .chunks_exact(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
                .collect();
            assert_eq!(output, [16, 16, 16, 16, 16, 16, u32::MAX, u32::MAX]);
        }
    }
    assert!(matches!(
        admitted.simulate_scheduled(
            &request,
            TARGET,
            limits,
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 5 },
        ),
        Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
            kind: SimulationExecutionErrorKindV1::ScheduleDecisionLimit {
                actual: 6,
                limit: 5,
            },
            ..
        })),
    ));
}

#[test]
fn exact_v12_binding_refuses_every_older_artifact_route_and_older_identity() {
    let admitted = admitted(7);
    let limits = SimulationLimitsV1::default();
    let (_, document) = recorded(
        &admitted,
        &request(9),
        TARGET,
        limits,
        SimulationScheduleRequestV1::RecordCanonical { max_decisions: 6 },
    );
    for artifact in [
        PersistedSimulationScheduleArtifactV1::CanonicalKirV7,
        PersistedSimulationScheduleArtifactV1::CanonicalKirV9,
        PersistedSimulationScheduleArtifactV1::CanonicalKirV10,
        PersistedSimulationScheduleArtifactV1::CanonicalKirV11,
        PersistedSimulationScheduleArtifactV1::SimulationBundleV1 {
            bundle_sha256: [1; 32],
            subject_sha256: [2; 32],
        },
        PersistedSimulationScheduleArtifactV1::SimulationBundleV5 {
            bundle_sha256: [1; 32],
            subject_sha256: [2; 32],
        },
        PersistedSimulationScheduleArtifactV1::SimulationBundleV6 {
            bundle_sha256: [1; 32],
            subject_sha256: [2; 32],
        },
    ] {
        let binding = PersistedSimulationScheduleBindingV1::new(
            artifact,
            *admitted.identity(),
            [9; 32],
            317,
            TARGET,
            limits,
        );
        assert_eq!(
            PersistedSimulationScheduleDocumentV1::encode_record(binding, document.record())
                .unwrap_err(),
            PersistedSimulationScheduleCodecErrorV1::InvalidBinding,
        );
    }
    let v7 = VerifiedCanonicalKernelIrV7::from_module(module(7)).unwrap();
    let v11 = VerifiedCanonicalKernelIrV11::from_module(module(7)).unwrap();
    for identity in [
        SimulationKernelIrIdentityV1::from(*v7.identity()),
        SimulationKernelIrIdentityV1::from(*v11.identity()),
    ] {
        let binding = PersistedSimulationScheduleBindingV1::new(
            PersistedSimulationScheduleArtifactV1::CanonicalKirV12,
            identity,
            [9; 32],
            317,
            TARGET,
            limits,
        );
        assert_eq!(
            PersistedSimulationScheduleDocumentV1::new(binding, document.record().clone())
                .unwrap_err(),
            PersistedSimulationScheduleCodecErrorV1::InvalidBinding,
        );
    }
}

#[test]
fn persisted_v12_replay_rejects_body_version_request_target_and_limit_substitution() {
    let admitted = admitted(7);
    let request = request(9);
    let limits = SimulationLimitsV1::default();
    let (_, document) = recorded(
        &admitted,
        &request,
        TARGET,
        limits,
        SimulationScheduleRequestV1::RecordSeeded {
            seed: 0x5eed,
            max_decisions: 6,
        },
    );
    let changed_body = self::admitted(8);
    let old_version = AdmittedSimulationModuleV1::admit_v11(
        VerifiedCanonicalKernelIrV11::from_module(module(7)).unwrap(),
        limits,
    )
    .unwrap();
    let replay = SimulationScheduleRequestV1::Replay(document.record());
    for result in [
        changed_body.simulate_scheduled(&request, TARGET, limits, replay),
        old_version.simulate_scheduled(&request, TARGET, limits, replay),
        admitted.simulate_scheduled(&self::request(10), TARGET, limits, replay),
        admitted.simulate_scheduled(
            &request,
            SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
            limits,
            replay,
        ),
        admitted.simulate_scheduled(
            &request,
            TARGET,
            SimulationLimitsV1 {
                max_events: limits.max_events - 1,
                ..limits
            },
            replay,
        ),
    ] {
        assert!(matches!(
            result,
            Err(SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                invocation: None,
                site: None,
                kind: SimulationExecutionErrorKindV1::ScheduleReplay(
                    SimulationScheduleReplayErrorV1::ContextMismatch,
                ),
                ..
            })),
        ));
    }
}

#[test]
fn persisted_v12_codec_retains_strict_wire_integrity_and_external_binding_checks() {
    let admitted = admitted(7);
    let limits = SimulationLimitsV1::default();
    let (_, document) = recorded(
        &admitted,
        &request(9),
        TARGET,
        limits,
        SimulationScheduleRequestV1::RecordSeeded {
            seed: 0x5eed,
            max_decisions: 6,
        },
    );
    let bytes = document.to_canonical_bytes().unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    for corrupt in [
        text.replacen("canonical_kir_v12", "canonical_kir_v012", 1),
        text.replacen("canonical_kir_v12", "canonical_kir_v13", 1),
        text.replacen("\"artifact\":{", "\"artifact\":{\"unknown\":0,", 1),
    ] {
        assert_eq!(
            PersistedSimulationScheduleDocumentV1::from_canonical_bytes(corrupt.as_bytes())
                .unwrap_err(),
            PersistedSimulationScheduleCodecErrorV1::JsonStructure,
        );
    }
    let mut noncanonical = bytes;
    noncanonical.push(b'\n');
    assert_eq!(
        PersistedSimulationScheduleDocumentV1::from_canonical_bytes(&noncanonical).unwrap_err(),
        PersistedSimulationScheduleCodecErrorV1::NonCanonical,
    );
    let corrupt_seed = text.replacen("\"seed\":24301", "\"seed\":24302", 1);
    assert_eq!(
        PersistedSimulationScheduleDocumentV1::from_canonical_bytes(corrupt_seed.as_bytes())
            .unwrap_err(),
        PersistedSimulationScheduleCodecErrorV1::InvalidRecordIntegrity,
    );
    let binding = PersistedSimulationScheduleBindingV1::new(
        PersistedSimulationScheduleArtifactV1::CanonicalKirV12,
        *admitted.identity(),
        [9; 32],
        317,
        TARGET,
        SimulationLimitsV1 {
            max_events: 0,
            ..limits
        },
    );
    assert_eq!(
        PersistedSimulationScheduleDocumentV1::encode_record(binding, document.record())
            .unwrap_err(),
        PersistedSimulationScheduleCodecErrorV1::InvalidLimits,
    );

    // A codec cannot authenticate inputs it was not supplied. Consumers must
    // compare this exact binding before replaying against their admitted owner.
    let wrong_route = text.replacen("canonical_kir_v12", "canonical_kir_v11", 1);
    let substituted =
        PersistedSimulationScheduleDocumentV1::from_canonical_bytes(wrong_route.as_bytes())
            .unwrap();
    assert_eq!(substituted.binding().kir_wire_version(), 11);
    assert_ne!(substituted.binding(), document.binding());
    for (request_digest, request_bytes) in [([8; 32], 317), ([9; 32], 318)] {
        let external = PersistedSimulationScheduleBindingV1::new(
            PersistedSimulationScheduleArtifactV1::CanonicalKirV12,
            *admitted.identity(),
            request_digest,
            request_bytes,
            TARGET,
            limits,
        );
        assert_ne!(external, document.binding());
    }
}
