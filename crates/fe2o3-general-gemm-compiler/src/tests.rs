use super::*;

use fe2o3_compiler_api::{
    CompileLimitsV1, CompilerProfileIdentityV1, KernelInstanceIdentityV1, ObligationSetIdentityV1,
    PipelineConfigurationIdentityV1, RequestIdentityV1, SnapshotFormatIdentityV1,
    SnapshotIdentityV1, StageSnapshotV1, TargetProfileIdentityV1,
};
use fe2o3_compiler_driver::{
    GEMM_REQUIRED_SAFETY_PROPERTIES_V1, GemmObligationFindingV1, GemmObligationOutcomeV1,
    GemmProofReportV1, GemmProofRequirementsV1, admit_proof_required_gemm_v1,
};
use fe2o3_kernel_ir::{
    GeneralGemmPlanSnapshotV1, GeneralGemmPropertyV1, GeneralGemmSemanticMutationV1,
    GeneralGemmVerificationStageV1, general_gemm_semantic_mutation_kir_v1,
};

fn identity(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn plan() -> GeneralGemmPlanFieldsV1 {
    GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
        dimensions: [17, 19, 18],
        strides: [23, 29, 31],
        storage_elements: [386, 512, 515],
        block_counts: [2, 2, 1],
        aql_grid_work_items: [128, 2, 1],
        reduction_phases: 2,
        alpha_bits: 2.0_f32.to_bits(),
        beta_bits: (-1.0_f32).to_bits(),
    })
    .unwrap()
}

fn request_for(kir: &GeneralGemmKirV1, request_byte: u8) -> CompileRequestV1 {
    let input = StageSnapshotV1::new(
        CompilerStageV1::FrontendInput,
        SnapshotIdentityV1::from_untrusted_bytes(identity(0x17)),
        SnapshotFormatIdentityV1::from_untrusted_bytes(identity(0x18)),
        b"authenticated-general-gemm-mir-v1".to_vec(),
    )
    .unwrap();
    let obligations = general_gemm_semantic_obligation_set_identity_v1(input.identity(), kir);
    CompileRequestV1::new(
        RequestIdentityV1::from_untrusted_bytes(identity(request_byte)),
        KernelInstanceIdentityV1::from_untrusted_bytes(identity(0x12)),
        CompilerProfileIdentityV1::from_untrusted_bytes(identity(0x13)),
        TargetProfileIdentityV1::from_untrusted_bytes(identity(0x14)),
        PipelineConfigurationIdentityV1::from_untrusted_bytes(identity(0x15)),
        obligations,
        PipelineSelectorV1::PlironV1,
        input,
        CompileLimitsV1::new(16, 16, 16, 4096, 16_384, 4096).unwrap(),
    )
    .unwrap()
}

fn frontend_binding() -> GeneralGemmFrontendSemanticBindingV1 {
    GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
        identity(0x12),
        identity(0x41),
        identity(0x42),
        GeneralGemmSymbolicPlanV1::canonical(),
        GeneralGemmSymbolicKirV1::canonical(),
    )
    .unwrap()
}

fn unit(schedule: GeneralGemmScheduleV1) -> GeneralGemmCompilationUnitV1 {
    let plan = plan();
    let kir = GeneralGemmKirV1::canonical(plan);
    let request = request_for(&kir, 0x11);
    GeneralGemmCompilationUnitV1::checked(
        &request,
        frontend_binding(),
        plan,
        kir,
        schedule,
        GeneralGemmRuntimeAbiV1::from_plan(plan),
        GeneralGemmLoweringLimitsV1::default(),
    )
    .unwrap()
}

fn admission(request: &CompileRequestV1) -> ProofRequiredGemmAdmissionV1 {
    let requirements = GemmProofRequirementsV1::new(request, Vec::new()).unwrap();
    let findings = GEMM_REQUIRED_SAFETY_PROPERTIES_V1
        .into_iter()
        .map(|property| {
            GemmObligationFindingV1::required(property, GemmObligationOutcomeV1::Discharged, None)
        })
        .collect();
    let report = GemmProofReportV1::new(request.input_obligations_identity(), findings).unwrap();
    admit_proof_required_gemm_v1(request, &requirements, &report).unwrap()
}

#[test]
fn runtime_abi_checks_every_dynamic_plan_field() {
    let plan = plan();
    let exact = GeneralGemmRuntimeAbiV1::from_plan(plan).snapshot();
    let mutations = [
        (
            GeneralGemmRuntimeAbiSnapshotV1 {
                a_elements: exact.a_elements + 1,
                ..exact
            },
            GeneralGemmRuntimeAbiErrorV1::StorageElements,
        ),
        (
            GeneralGemmRuntimeAbiSnapshotV1 {
                dimensions: [18, 19, 18],
                ..exact
            },
            GeneralGemmRuntimeAbiErrorV1::Dimensions,
        ),
        (
            GeneralGemmRuntimeAbiSnapshotV1 {
                strides: [24, 29, 31],
                ..exact
            },
            GeneralGemmRuntimeAbiErrorV1::Strides,
        ),
        (
            GeneralGemmRuntimeAbiSnapshotV1 {
                alpha_bits: 3.0_f32.to_bits(),
                ..exact
            },
            GeneralGemmRuntimeAbiErrorV1::Alpha,
        ),
        (
            GeneralGemmRuntimeAbiSnapshotV1 {
                beta_bits: 1.0_f32.to_bits(),
                ..exact
            },
            GeneralGemmRuntimeAbiErrorV1::Beta,
        ),
    ];
    for (mutation, expected) in mutations {
        assert_eq!(
            GeneralGemmRuntimeAbiV1::checked(plan, mutation),
            Err(expected)
        );
    }
}

#[test]
fn schedules_share_one_semantic_body_but_never_an_identity() {
    let reference = unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let vector_a = unit(GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1);

    assert_eq!(reference.kir(), vector_a.kir());
    assert_eq!(reference.plan_identity(), vector_a.plan_identity());
    assert_eq!(
        reference.runtime_abi_identity(),
        vector_a.runtime_abi_identity()
    );
    assert_ne!(reference.schedule_identity(), vector_a.schedule_identity());
    assert_ne!(reference.identity(), vector_a.identity());
    assert!(vector_a.schedule().requires_vectorized_a_isa_confirmation());
    assert!(
        !reference
            .schedule()
            .requires_vectorized_a_isa_confirmation()
    );
    assert_eq!(
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1.a_full_transfer_width_bf16(),
        1
    );
    assert_eq!(
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1.a_full_transfer_width_bf16(),
        4
    );
    assert_eq!(
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
            .a_full_transfer_alignment_bytes(),
        8
    );
    assert_eq!(
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1.b_transfer_width_bf16(),
        1
    );
}

#[test]
fn symbolic_frontend_schemas_are_deterministic_and_bound_to_launch() {
    let plan_schema = GeneralGemmSymbolicPlanV1::canonical();
    let kir_schema = GeneralGemmSymbolicKirV1::canonical();
    assert_eq!(plan_schema, GeneralGemmSymbolicPlanV1::canonical());
    assert_eq!(kir_schema, GeneralGemmSymbolicKirV1::canonical());
    assert_ne!(plan_schema.identity().into_bytes(), [0; 32]);
    assert_ne!(kir_schema.identity().into_bytes(), [0; 32]);
    assert_eq!(plan_schema.expressions().len(), 6);

    let binding = frontend_binding();
    assert_eq!(binding.symbolic_plan(), plan_schema);
    assert_eq!(binding.symbolic_kir(), kir_schema);
    assert_ne!(binding.identity().into_bytes(), [0; 32]);
    assert_eq!(
        GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
            [0; 32],
            identity(0x41),
            identity(0x42),
            plan_schema,
            kir_schema,
        ),
        Err(GeneralGemmFrontendSemanticBindingErrorV1::ZeroIdentity)
    );

    let concrete_plan = plan();
    let kir = GeneralGemmKirV1::canonical(concrete_plan);
    let request = request_for(&kir, 0x11);
    let substituted_kernel =
        GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
            identity(0x55),
            identity(0x41),
            identity(0x42),
            plan_schema,
            kir_schema,
        )
        .unwrap();
    assert_eq!(
        GeneralGemmCompilationUnitV1::checked(
            &request,
            substituted_kernel,
            concrete_plan,
            kir,
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
            GeneralGemmRuntimeAbiV1::from_plan(concrete_plan),
            GeneralGemmLoweringLimitsV1::default(),
        ),
        Err(GeneralGemmCompilationBindingErrorV1::FrontendKernelSubstitution)
    );
}

#[test]
fn compiled_source_and_provider_substitution_change_the_aggregate_identity() {
    let baseline = frontend_binding();
    let source = GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
        identity(0x12),
        identity(0x43),
        identity(0x42),
        GeneralGemmSymbolicPlanV1::canonical(),
        GeneralGemmSymbolicKirV1::canonical(),
    )
    .unwrap();
    let provider =
        GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
            identity(0x12),
            identity(0x41),
            identity(0x44),
            GeneralGemmSymbolicPlanV1::canonical(),
            GeneralGemmSymbolicKirV1::canonical(),
        )
        .unwrap();
    assert_ne!(baseline.identity(), source.identity());
    assert_ne!(baseline.identity(), provider.identity());
    assert_ne!(source.identity(), provider.identity());
}

#[test]
fn verifier_request_is_derived_only_from_the_checked_compilation_unit() {
    for (schedule, expected) in [
        (
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
            GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
        ),
        (
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
            GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
        ),
    ] {
        let unit = unit(schedule);
        let proof = unit.schedule_proof_request().unwrap();
        assert_eq!(proof.schedule(), expected);
        assert_eq!(
            proof.schedule_identity().as_bytes(),
            unit.schedule_identity().as_bytes()
        );
        assert_eq!(
            proof.plan_identity().as_bytes(),
            unit.plan_identity().as_bytes()
        );
        assert_eq!(
            proof.kir_identity().as_bytes(),
            unit.kir_identity().as_bytes()
        );
        assert_eq!(
            proof.compilation_binding_identity().as_bytes(),
            unit.identity().as_bytes()
        );
        assert_eq!(
            proof.compile_request_identity().as_bytes(),
            unit.request().identity().as_bytes()
        );
        assert_eq!(
            proof.obligation_set_identity().as_bytes(),
            unit.request().input_obligations_identity().as_bytes()
        );
        assert_eq!(
            proof.compiler_identity().as_bytes(),
            unit.request().compiler_profile_identity().as_bytes()
        );
        assert_eq!(
            proof.target_identity().as_bytes(),
            unit.request().target_profile_identity().as_bytes()
        );
        assert_eq!(
            proof.toolchain_identity().as_bytes(),
            unit.toolchain_route_identity().as_bytes()
        );
        assert_eq!(
            proof.runtime_abi_identity().as_bytes(),
            unit.runtime_abi_identity().as_bytes()
        );
        assert_eq!(
            proof.source_semantics_identity().as_bytes(),
            unit.frontend_semantic_binding_identity().as_bytes()
        );
        assert_eq!(
            proof.numerical_policy_identity().as_bytes(),
            unit.frontend_semantics().provider_semantics_identity()
        );
    }
}

#[test]
fn hostile_kir_retains_exact_property_stage_and_code() {
    let plan = plan();
    let hostile = general_gemm_semantic_mutation_kir_v1(
        plan,
        GeneralGemmSemanticMutationV1::UnguardedATailLoad,
    );
    let request = request_for(&hostile, 0x11);
    let result = GeneralGemmCompilationUnitV1::checked(
        &request,
        frontend_binding(),
        plan,
        hostile,
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
        GeneralGemmRuntimeAbiV1::from_plan(plan),
        GeneralGemmLoweringLimitsV1::default(),
    );
    let Err(GeneralGemmCompilationBindingErrorV1::SemanticKir(diagnostic)) = result else {
        panic!("hostile KIR must retain its semantic diagnostic");
    };
    assert_eq!(diagnostic.property, GeneralGemmPropertyV1::BoundsSafe);
    assert_eq!(diagnostic.stage, GeneralGemmVerificationStageV1::Tile);
    assert_eq!(diagnostic.code, 0x4647_0102);
}

#[test]
fn limits_stop_before_pliron_construction() {
    let plan = plan();
    let kir = GeneralGemmKirV1::canonical(plan);
    let request = request_for(&kir, 0x11);
    let kir_len = kir.encode_canonical().len();
    let kir_limits =
        GeneralGemmLoweringLimitsV1::new(kir_len - 1, MAX_GENERAL_GEMM_PLIRON_OPERATIONS_V1)
            .unwrap();
    assert_eq!(
        GeneralGemmCompilationUnitV1::checked(
            &request,
            frontend_binding(),
            plan,
            kir.clone(),
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
            GeneralGemmRuntimeAbiV1::from_plan(plan),
            kir_limits,
        ),
        Err(GeneralGemmCompilationBindingErrorV1::KirBytesLimit {
            actual: kir_len,
            maximum: kir_len - 1,
        })
    );
    let operation_limits = GeneralGemmLoweringLimitsV1::new(
        MAX_GENERAL_GEMM_KIR_BYTES_V1,
        GENERAL_GEMM_PLIRON_OPERATION_COUNT_V1 - 1,
    )
    .unwrap();
    assert_eq!(
        GeneralGemmCompilationUnitV1::checked(
            &request,
            frontend_binding(),
            plan,
            kir,
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
            GeneralGemmRuntimeAbiV1::from_plan(plan),
            operation_limits,
        ),
        Err(
            GeneralGemmCompilationBindingErrorV1::PlironOperationsLimit {
                required: GENERAL_GEMM_PLIRON_OPERATION_COUNT_V1,
                maximum: GENERAL_GEMM_PLIRON_OPERATION_COUNT_V1 - 1,
            }
        )
    );
}

#[test]
fn proof_gate_builds_owner_bound_projection_then_stops_without_artifacts() {
    let unit = unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let request = unit.request().clone();
    let binding_identity = unit.identity();
    let mut backend = GeneralGemmAdmittedBackendV1::new(unit);
    let observation = backend
        .lower_admitted(&request, admission(&request))
        .unwrap();

    assert_eq!(
        observation.projection().compilation_binding_identity(),
        binding_identity
    );
    assert_eq!(
        observation.projection().operation_count(),
        GENERAL_GEMM_PLIRON_OPERATION_COUNT_V1
    );
    assert_eq!(observation.blocker().stage(), CompilerStageV1::Amdgcn);
    assert_eq!(
        observation.blocker().gaps(),
        [
            GeneralGemmMachineRepresentationGapV1::WorkgroupBf16Array256,
            GeneralGemmMachineRepresentationGapV1::Wave64Bf16MfmaM16N16K16,
            GeneralGemmMachineRepresentationGapV1::LoopCarriedF32x4Accumulator,
        ]
    );
    assert_eq!(observation.handoff_v2_identity(), None);
    assert_eq!(observation.llvm_assembly_identity(), None);
    assert_eq!(observation.compiler_handoff_identity(), None);
    assert_eq!(observation.candidate_identity(), None);
    assert!(!observation.projection().grants_artifact_authority());
    assert_eq!(
        backend.lower_admitted(&request, admission(&request)),
        Err(GeneralGemmAdmittedLoweringErrorV1::Replay)
    );
}

#[test]
fn request_substitution_rejects_without_consuming_the_unit() {
    let unit = unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let request = unit.request().clone();
    let substituted = request_for(unit.kir(), 0x21);
    let mut backend = GeneralGemmAdmittedBackendV1::new(unit);

    assert_eq!(
        backend.lower_admitted(&substituted, admission(&substituted)),
        Err(GeneralGemmAdmittedLoweringErrorV1::RequestSubstitution)
    );
    assert!(
        backend
            .lower_admitted(&request, admission(&request))
            .is_ok()
    );
}

#[test]
fn transactional_backend_emits_only_the_bounded_blocker_diagnostic() {
    let unit = unit(GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1);
    let request = unit.request().clone();
    let mut backend = GeneralGemmAdmittedBackendV1::new(unit);
    let output = backend
        .compile_admitted(&request, admission(&request))
        .unwrap();

    assert_eq!(output.disposition(), CompileDispositionV1::Rejected);
    assert!(output.candidate().is_none());
    assert!(output.snapshots().is_empty());
    assert!(output.receipts().is_empty());
    assert_eq!(output.diagnostics().len(), 1);
    let diagnostic = &output.diagnostics()[0];
    assert_eq!(
        diagnostic.code().get(),
        GENERAL_GEMM_LOWERING_BLOCKED_CODE_V1
    );
    assert_eq!(diagnostic.stage(), Some(CompilerStageV1::Amdgcn));
    assert_eq!(
        diagnostic.message().as_str(),
        GENERAL_GEMM_LOWERING_BLOCKED_MESSAGE_V1
    );
}

#[test]
fn foreign_context_cannot_validate_projection_ownership() {
    let unit = unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let envelope = project_to_pliron(&unit).unwrap();
    let mut foreign = Context::new();
    ensure_context_identity(&mut foreign).unwrap();

    assert_eq!(
        envelope.validate_owner_in(&foreign),
        Err(ContextIdentityError::CorruptMarker)
    );
    assert_eq!(envelope.validate_owner(), Ok(()));
}

#[test]
fn transplanted_schedule_metadata_cannot_validate_as_the_projection() {
    let unit = unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let envelope = project_to_pliron(&unit).unwrap();
    let binding = envelope.module.get_operation();
    binding.deref_mut(&envelope.context).attributes.set(
        metadata_key(PLIRON_SCHEDULE_ATTR),
        BytesAttr::new(
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
                .identity()
                .into_bytes()
                .to_vec(),
        ),
    );

    assert_eq!(envelope.validate_exact(&unit), Err(()));
}

#[test]
fn zero_request_commitments_and_invalid_limit_configuration_fail_closed() {
    let plan = plan();
    let kir = GeneralGemmKirV1::canonical(plan);
    let input = StageSnapshotV1::new(
        CompilerStageV1::FrontendInput,
        SnapshotIdentityV1::from_untrusted_bytes(identity(0x17)),
        SnapshotFormatIdentityV1::from_untrusted_bytes(identity(0x18)),
        vec![1],
    )
    .unwrap();
    let request = CompileRequestV1::new(
        RequestIdentityV1::from_untrusted_bytes([0; 32]),
        KernelInstanceIdentityV1::from_untrusted_bytes(identity(0x12)),
        CompilerProfileIdentityV1::from_untrusted_bytes(identity(0x13)),
        TargetProfileIdentityV1::from_untrusted_bytes(identity(0x14)),
        PipelineConfigurationIdentityV1::from_untrusted_bytes(identity(0x15)),
        ObligationSetIdentityV1::from_untrusted_bytes(identity(0x16)),
        PipelineSelectorV1::PlironV1,
        input,
        CompileLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        GeneralGemmCompilationUnitV1::checked(
            &request,
            frontend_binding(),
            plan,
            kir,
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
            GeneralGemmRuntimeAbiV1::from_plan(plan),
            GeneralGemmLoweringLimitsV1::default(),
        ),
        Err(GeneralGemmCompilationBindingErrorV1::ZeroRequestCommitment)
    );
    assert_eq!(
        GeneralGemmLoweringLimitsV1::new(0, 1),
        Err(GeneralGemmLoweringLimitErrorV1::KirBytes)
    );
    assert_eq!(
        GeneralGemmLoweringLimitsV1::new(1, MAX_GENERAL_GEMM_PLIRON_OPERATIONS_V1 + 1),
        Err(GeneralGemmLoweringLimitErrorV1::PlironOperations)
    );
}
