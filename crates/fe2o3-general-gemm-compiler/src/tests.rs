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
use fe2o3_llvm_worker_handoff::{
    EXACT_LLD_BUILD_IDENTITY_V1, EXACT_LLD_VERSION_V1, EXACT_LLVM_BUILD_IDENTITY_V1,
    EXACT_LLVM_VERSION_V1,
};
use fe2o3_lower_amdgcn_llvm::lower_amdgcn_to_pliron_llvm_v1;

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
        identity(0x43),
        GeneralGemmSymbolicPlanV1::canonical(),
        GeneralGemmSymbolicKirV1::canonical(),
    )
    .unwrap()
}

fn symbolic_request_for(
    frontend: &GeneralGemmFrontendSemanticBindingV1,
    schedule: GeneralGemmScheduleV1,
) -> CompileRequestV1 {
    let input = StageSnapshotV1::new(
        CompilerStageV1::FrontendInput,
        SnapshotIdentityV1::from_untrusted_bytes(identity(0x17)),
        SnapshotFormatIdentityV1::from_untrusted_bytes(identity(0x18)),
        b"authenticated-general-gemm-symbolic-mir-v1".to_vec(),
    )
    .unwrap();
    let obligations = general_gemm_symbolic_obligation_set_identity_v1(&input, frontend);
    CompileRequestV1::new(
        RequestIdentityV1::from_untrusted_bytes(identity(0x11)),
        KernelInstanceIdentityV1::from_untrusted_bytes(identity(0x12)),
        CompilerProfileIdentityV1::from_untrusted_bytes(identity(0x13)),
        TargetProfileIdentityV1::from_untrusted_bytes(identity(0x14)),
        general_gemm_symbolic_pipeline_configuration_identity_v1(schedule),
        obligations,
        PipelineSelectorV1::PlironV1,
        input,
        CompileLimitsV1::new(16, 16, 16, 4096, 16_384, 4096).unwrap(),
    )
    .unwrap()
}

fn symbolic_unit(schedule: GeneralGemmScheduleV1) -> GeneralGemmSymbolicCompilationUnitV1 {
    symbolic_unit_with_frontend(schedule, frontend_binding())
}

fn symbolic_unit_with_frontend(
    schedule: GeneralGemmScheduleV1,
    frontend: GeneralGemmFrontendSemanticBindingV1,
) -> GeneralGemmSymbolicCompilationUnitV1 {
    let request = symbolic_request_for(&frontend, schedule);
    GeneralGemmSymbolicCompilationUnitV1::checked(
        &request,
        frontend,
        schedule,
        GeneralGemmLoweringLimitsV1::default(),
    )
    .unwrap()
}

fn unit(schedule: GeneralGemmScheduleV1) -> GeneralGemmCompilationUnitV1 {
    unit_with_frontend(schedule, frontend_binding())
}

fn unit_with_frontend(
    schedule: GeneralGemmScheduleV1,
    frontend: GeneralGemmFrontendSemanticBindingV1,
) -> GeneralGemmCompilationUnitV1 {
    let plan = plan();
    let kir = GeneralGemmKirV1::canonical(plan);
    let request = request_for(&kir, 0x11);
    GeneralGemmCompilationUnitV1::checked(
        &request,
        frontend,
        plan,
        kir,
        schedule,
        GeneralGemmRuntimeAbiV1::from_plan(plan),
        GeneralGemmLoweringLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn complete_general_gemm_handoffs_lower_into_live_pliron_llvm_graphs() {
    for schedule in [
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    ] {
        let unit = unit(schedule);
        let machine = lower_general_gemm_structural_machine_v1(&unit).unwrap();
        let source = machine.handoff();
        let lowered = lower_amdgcn_to_pliron_llvm_v1(source).unwrap();
        let inspection = lowered.inspect_live_graph().unwrap();
        let expected_operations = source
            .module()
            .functions()
            .iter()
            .flat_map(|function| function.blocks())
            .map(|block| {
                block
                    .instructions()
                    .iter()
                    .filter(|instruction| {
                        !matches!(
                            instruction.kind(),
                            fe2o3_llvm_handoff::InstructionKindV2::Phi { .. }
                        )
                    })
                    .count()
                    + 1
            })
            .sum::<usize>();

        assert_eq!(lowered.source_identity(), source.identity());
        assert_eq!(inspection, lowered.construction_inspection());
        assert_eq!(
            inspection.global_count() as usize,
            source.module().globals().len()
        );
        assert_eq!(
            inspection.intrinsic_count() as usize,
            source.module().intrinsics().len()
        );
        assert_eq!(
            inspection.function_count() as usize,
            source.module().functions().len()
        );
        assert_eq!(inspection.operation_count() as usize, expected_operations);
        assert!(inspection.strict_float());
        assert!(inspection.exact_memory_alignment());
        assert!(!lowered.grants_artifact_authority());
        let boundary = machine.compiler_boundary();
        assert_eq!(
            boundary.serialization_receipt().graph_inspection(),
            inspection
        );
        assert_eq!(
            boundary.worker_admission().handoff_identity(),
            source.identity()
        );
        assert_ne!(boundary.identity().as_bytes(), &[0; 32]);
        assert!(!boundary.grants_artifact_authority());
    }
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
fn symbolic_compilation_binds_template_and_closed_schedule_without_launch_values() {
    let reference = symbolic_unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let vector = symbolic_unit(GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1);

    assert_eq!(
        reference.symbolic_plan_identity(),
        GeneralGemmSymbolicPlanV1::canonical().identity()
    );
    assert_eq!(
        reference.symbolic_kir_identity(),
        GeneralGemmSymbolicKirV1::canonical().identity()
    );
    assert_ne!(reference.schedule_identity(), vector.schedule_identity());
    assert_ne!(reference.identity(), vector.identity());
    assert!(!reference.grants_artifact_authority());
}

#[test]
fn symbolic_compilation_rejects_concrete_obligations_and_schedule_relabeling() {
    let schedule = GeneralGemmScheduleV1::ReferenceWave64Xor4V1;
    let frontend = frontend_binding();
    let concrete_plan = plan();
    let concrete_kir = GeneralGemmKirV1::canonical(concrete_plan);
    let concrete_request = request_for(&concrete_kir, 0x11);
    let wrong_obligations_request = CompileRequestV1::new(
        concrete_request.identity(),
        concrete_request.kernel_instance_identity(),
        concrete_request.compiler_profile_identity(),
        concrete_request.target_profile_identity(),
        general_gemm_symbolic_pipeline_configuration_identity_v1(schedule),
        concrete_request.input_obligations_identity(),
        PipelineSelectorV1::PlironV1,
        concrete_request.input().clone(),
        concrete_request.limits(),
    )
    .unwrap();
    assert_eq!(
        GeneralGemmSymbolicCompilationUnitV1::checked(
            &wrong_obligations_request,
            frontend,
            schedule,
            GeneralGemmLoweringLimitsV1::default(),
        )
        .unwrap_err(),
        GeneralGemmSymbolicCompilationErrorV1::SymbolicObligationSetSubstitution
    );

    let frontend = frontend_binding();
    let vector_request = symbolic_request_for(
        &frontend,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    );
    assert_eq!(
        GeneralGemmSymbolicCompilationUnitV1::checked(
            &vector_request,
            frontend,
            schedule,
            GeneralGemmLoweringLimitsV1::default(),
        )
        .unwrap_err(),
        GeneralGemmSymbolicCompilationErrorV1::ScheduleSelectionSubstitution
    );
}

#[test]
fn symbolic_compilation_limits_fail_before_projection() {
    let schedule = GeneralGemmScheduleV1::ReferenceWave64Xor4V1;
    let frontend = frontend_binding();
    let request = symbolic_request_for(&frontend, schedule);
    let limits = GeneralGemmLoweringLimitsV1::new(MAX_GENERAL_GEMM_KIR_BYTES_V1, 10).unwrap();
    assert_eq!(
        GeneralGemmSymbolicCompilationUnitV1::checked(&request, frontend, schedule, limits,)
            .unwrap_err(),
        GeneralGemmSymbolicCompilationErrorV1::PlironOperationsLimit {
            required: GENERAL_GEMM_SYMBOLIC_LOWERED_OPERATION_COUNT_V1,
            maximum: 10,
        }
    );
}

#[test]
fn checked_launch_instantiation_binds_every_runtime_value_to_symbolic_artifact() {
    let unit = symbolic_unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let plan = plan();
    let kir = GeneralGemmKirV1::canonical(plan);
    let artifact = GeneralGemmSymbolicArtifactIdentityV1(identity(0x55));
    let launch = GeneralGemmCheckedLaunchInstantiationV1::checked(
        &unit,
        artifact,
        plan,
        kir,
        GeneralGemmRuntimeAbiV1::from_plan(plan).snapshot(),
    )
    .unwrap();

    assert_eq!(launch.symbolic_compilation_identity(), unit.identity());
    assert_eq!(launch.symbolic_artifact_identity(), artifact);
    assert_eq!(launch.plan_identity(), plan_identity(plan));
    assert_eq!(
        launch.kir_identity(),
        GeneralGemmKirV1::canonical(plan).identity()
    );
    assert!(!launch.grants_launch_authority());
}

#[test]
fn checked_launch_instantiation_rejects_artifact_and_runtime_substitution() {
    let unit = symbolic_unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let plan = plan();
    let kir = GeneralGemmKirV1::canonical(plan);
    let snapshot = GeneralGemmRuntimeAbiV1::from_plan(plan).snapshot();
    assert_eq!(
        GeneralGemmCheckedLaunchInstantiationV1::checked(
            &unit,
            GeneralGemmSymbolicArtifactIdentityV1([0; 32]),
            plan,
            kir.clone(),
            snapshot,
        )
        .unwrap_err(),
        GeneralGemmCheckedLaunchInstantiationErrorV1::ZeroArtifactIdentity
    );
    assert_eq!(
        GeneralGemmCheckedLaunchInstantiationV1::checked(
            &unit,
            GeneralGemmSymbolicArtifactIdentityV1(identity(0x55)),
            plan,
            kir,
            GeneralGemmRuntimeAbiSnapshotV1 {
                strides: [
                    snapshot.strides[0] + 1,
                    snapshot.strides[1],
                    snapshot.strides[2]
                ],
                ..snapshot
            },
        )
        .unwrap_err(),
        GeneralGemmCheckedLaunchInstantiationErrorV1::RuntimeAbi(
            GeneralGemmRuntimeAbiErrorV1::Strides,
        )
    );
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
            identity(0x43),
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
            identity(0x43),
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

fn independently_derived_source_schema() -> GeneralGemmDerivedSourceSchemaV1 {
    GeneralGemmDerivedSourceSchemaV1::checked(
        [
            GeneralGemmSymbolicPlanExpressionV1::CheckedRowMajorExtent {
                rows: GeneralGemmAbiArgumentV1::M,
                columns: GeneralGemmAbiArgumentV1::K,
                stride: GeneralGemmAbiArgumentV1::Lda,
            },
            GeneralGemmSymbolicPlanExpressionV1::CheckedRowMajorExtent {
                rows: GeneralGemmAbiArgumentV1::K,
                columns: GeneralGemmAbiArgumentV1::N,
                stride: GeneralGemmAbiArgumentV1::Ldb,
            },
            GeneralGemmSymbolicPlanExpressionV1::CheckedRowMajorExtent {
                rows: GeneralGemmAbiArgumentV1::M,
                columns: GeneralGemmAbiArgumentV1::N,
                stride: GeneralGemmAbiArgumentV1::Ldc,
            },
            GeneralGemmSymbolicPlanExpressionV1::CeilDiv16(GeneralGemmAbiArgumentV1::K),
            GeneralGemmSymbolicPlanExpressionV1::OutputBlockCounts,
            GeneralGemmSymbolicPlanExpressionV1::AqlGridWorkItems,
        ],
        [
            GeneralGemmDerivedKirBehaviorV1::Wave64GridXy16,
            GeneralGemmDerivedKirBehaviorV1::GuardedAbCheckedRowMajorZeroTail,
            GeneralGemmDerivedKirBehaviorV1::Xor4SingleBufferPublishReadMfmaReuse,
            GeneralGemmDerivedKirBehaviorV1::CarriedF32x4PhaseAccumulator,
            GeneralGemmDerivedKirBehaviorV1::GuardedDisjointCAlphaAccPlusBetaC,
        ],
    )
    .unwrap()
}

#[test]
fn independently_derived_source_schema_reproduces_closed_identities_without_authority() {
    let schema = independently_derived_source_schema();
    let plan = GeneralGemmSymbolicPlanV1::from_derived_source_schema(&schema).unwrap();
    let kir = GeneralGemmSymbolicKirV1::from_derived_source_schema(&schema).unwrap();
    assert_eq!(plan, GeneralGemmSymbolicPlanV1::canonical());
    assert_eq!(kir, GeneralGemmSymbolicKirV1::canonical());
    assert!(!schema.grants_authority());
}

#[test]
fn derived_source_schema_rejects_plan_and_kir_reordering() {
    let schema = independently_derived_source_schema();
    let mut plan = schema.plan_expressions();
    plan.swap(0, 1);
    assert_eq!(
        GeneralGemmDerivedSourceSchemaV1::checked(plan, schema.kir_behaviors()),
        Err(GeneralGemmDerivedSourceSchemaErrorV1::PlanExpressions)
    );
    let mut behaviors = schema.kir_behaviors();
    behaviors.swap(2, 3);
    assert_eq!(
        GeneralGemmDerivedSourceSchemaV1::checked(schema.plan_expressions(), behaviors),
        Err(GeneralGemmDerivedSourceSchemaErrorV1::KirBehaviors)
    );
}

#[test]
fn compiled_source_and_provider_substitution_change_the_aggregate_identity() {
    let baseline = frontend_binding();
    let source = GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
        identity(0x12),
        identity(0x44),
        identity(0x42),
        identity(0x43),
        GeneralGemmSymbolicPlanV1::canonical(),
        GeneralGemmSymbolicKirV1::canonical(),
    )
    .unwrap();
    let provider =
        GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
            identity(0x12),
            identity(0x41),
            identity(0x45),
            identity(0x43),
            GeneralGemmSymbolicPlanV1::canonical(),
            GeneralGemmSymbolicKirV1::canonical(),
        )
        .unwrap();
    assert_ne!(baseline.identity(), source.identity());
    assert_ne!(baseline.identity(), provider.identity());
    assert_ne!(source.identity(), provider.identity());
}

#[test]
fn verifier_request_is_derived_only_from_the_symbolic_compilation_unit() {
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
        let unit = symbolic_unit(schedule);
        let proof = unit.symbolic_schedule_proof_request().unwrap();
        assert_eq!(proof.schedule(), expected);
        assert_eq!(
            proof.schedule_identity().as_bytes(),
            unit.schedule_identity().as_bytes()
        );
        assert_eq!(
            proof.symbolic_plan_identity().as_bytes(),
            unit.symbolic_plan_identity().as_bytes()
        );
        assert_eq!(
            proof.symbolic_kir_identity().as_bytes(),
            unit.symbolic_kir_identity().as_bytes()
        );
        assert_eq!(
            proof.symbolic_compilation_identity().as_bytes(),
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
            proof.source_template_identity().as_bytes(),
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
    assert_eq!(observation.blocker().stage(), CompilerStageV1::Llvm);
    assert_eq!(
        observation.blocker().gaps(),
        [GeneralGemmProductionGapV1::AuthorityJoin]
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
    assert_eq!(diagnostic.stage(), Some(CompilerStageV1::Llvm));
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
fn descriptor_source_is_exactly_bound_to_projection_schedule_and_safe_abi() {
    let reference = unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let reference_projection = project_to_pliron(&reference).unwrap().receipt;
    let reference_source =
        derive_general_gemm_descriptor_source_v1(&reference, reference_projection).unwrap();
    let table = reference_source.table();
    assert_eq!(table.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(
        table.device_target().to_string(),
        GENERAL_GEMM_DEVICE_TARGET_V1
    );
    let [kernel] = table.kernels() else {
        panic!("general GEMM descriptor must contain exactly one kernel");
    };
    assert_eq!(
        kernel.kernel_id().as_bytes(),
        reference.identity().as_bytes()
    );
    assert_eq!(kernel.entry_name().as_str(), GENERAL_GEMM_KERNEL_SYMBOL_V1);
    assert_eq!(
        kernel.descriptor_symbol().as_str(),
        GENERAL_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1
    );
    assert_eq!(
        kernel.abi_layout().explicit_argument_size(),
        GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1
    );
    assert_eq!(
        kernel.abi_layout().kernarg_segment_size(),
        GENERAL_GEMM_TOTAL_KERNARG_BYTES_V1
    );
    assert_eq!(kernel.arguments().len(), 11);
    assert_eq!(kernel.launch().rank(), 2);
    assert_eq!(
        kernel.launch().block_size(),
        BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap())
    );
    assert_eq!(kernel.launch().max_flat_workgroup_size(), 64);
    assert_eq!(
        kernel.launch().static_shared_memory_bytes(),
        GENERAL_GEMM_STATIC_LDS_BYTES_V1
    );
    let components = kernel
        .arguments()
        .iter()
        .flat_map(LogicalArgumentV1::physical_components)
        .map(|(_, offset, size, _)| (offset, size))
        .collect::<Vec<_>>();
    assert_eq!(
        components,
        [
            (0, 8),
            (8, 8),
            (16, 8),
            (24, 8),
            (32, 8),
            (40, 8),
            (48, 4),
            (52, 4),
            (56, 4),
            (60, 4),
            (64, 4),
            (68, 4),
            (72, 4),
            (76, 4),
        ]
    );
    assert_eq!(
        kernel.source_evidence().identity().as_bytes(),
        reference.frontend_semantic_binding_identity().as_bytes()
    );
    assert_eq!(
        kernel.executable_ir_evidence().identity().as_bytes(),
        reference_projection.identity().as_bytes()
    );
    assert!(
        reference_source
            .identity()
            .matches(reference_source.canonical_bytes())
    );

    let optimized = unit(GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1);
    let optimized_projection = project_to_pliron(&optimized).unwrap().receipt;
    let optimized_source =
        derive_general_gemm_descriptor_source_v1(&optimized, optimized_projection).unwrap();
    assert_ne!(reference_source.identity(), optimized_source.identity());
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

#[test]
fn structural_machine_lowers_both_schedules_without_artifact_authority() {
    for schedule in [
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    ] {
        let unit = unit(schedule);
        let machine = lower_general_gemm_structural_machine_v1(&unit).unwrap();
        assert_eq!(
            machine.projection().compilation_binding_identity(),
            unit.identity()
        );
        assert_eq!(
            machine
                .handoff()
                .base()
                .stage_identities()
                .schedule()
                .as_bytes(),
            unit.schedule_identity().as_bytes()
        );
        assert_eq!(
            machine.assembly().source_identity(),
            machine.graph_handoff().identity()
        );
        assert_eq!(
            machine.worker_admission().handoff_identity(),
            machine.graph_handoff().identity()
        );
        assert_ne!(
            machine.worker_admission().admission_identity().as_bytes(),
            &[0; 32]
        );
        assert_eq!(
            machine
                .compiler_boundary()
                .serialization_receipt()
                .graph_handoff_identity(),
            machine.graph_handoff().identity()
        );
        assert!(!machine.compiler_boundary().grants_artifact_authority());
        assert!(
            !machine
                .worker_admission()
                .authenticates_worker_measurement()
        );
        assert!(!machine.worker_admission().grants_object_authority());
        assert!(!machine.worker_admission().grants_link_authority());
        assert!(!machine.worker_admission().grants_publication_authority());
        assert!(!machine.worker_admission().grants_load_authority());
        assert!(!machine.worker_admission().grants_launch_authority());
        assert!(machine.assembly().has_embedded_source_identity());
        assert!(!machine.grants_artifact_authority());
        assert!(!machine.compiler_handoff().grants_compiler_authority());
        assert!(!machine.compiler_handoff().grants_worker_authority());
        assert!(!machine.compiler_handoff().grants_link_authority());
        assert!(!machine.compiler_handoff().grants_load_authority());
        assert!(!machine.compiler_handoff().grants_launch_authority());
    }
}

#[test]
fn compiler_boundary_retains_only_post_serialization_graph_evidence() {
    let machine = lower_general_gemm_structural_machine_v1(&unit(
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
    ))
    .unwrap();
    let boundary = machine.compiler_boundary();
    let receipt = boundary.serialization_receipt();

    assert_eq!(
        boundary.worker_admission().handoff_identity(),
        receipt.graph_handoff_identity()
    );
    assert_eq!(machine.assembly().sha256(), receipt.assembly_sha256());
    assert!(!boundary.grants_artifact_authority());
    assert!(!boundary.worker_admission().grants_object_authority());
}

#[test]
fn compiler_boundary_retains_exact_worker_build_policy() {
    let machine = lower_general_gemm_structural_machine_v1(&unit(
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
    ))
    .unwrap();
    let build = machine.worker_admission().build_identity();
    assert_eq!(build.llvm_version(), EXACT_LLVM_VERSION_V1);
    assert_eq!(build.llvm_build_identity(), EXACT_LLVM_BUILD_IDENTITY_V1);
    assert_eq!(build.lld_version(), EXACT_LLD_VERSION_V1);
    assert_eq!(build.lld_build_identity(), EXACT_LLD_BUILD_IDENTITY_V1);
    assert!(build.in_process_lld());
}

fn occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

#[test]
fn structural_machine_text_has_the_exact_tiled_gemm_profile() {
    let reference = lower_general_gemm_structural_machine_v1(&unit(
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
    ))
    .unwrap();
    let optimized = lower_general_gemm_structural_machine_v1(&unit(
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    ))
    .unwrap();

    for machine in [&reference, &optimized] {
        let text = machine.assembly().as_str();
        assert_eq!(
            occurrences(text, "addrspace(3) global [256 x i16] undef, align 16"),
            2
        );
        assert_eq!(occurrences(text, "call void @llvm.amdgcn.s.barrier()"), 2);
        assert_eq!(
            occurrences(
                text,
                "call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k"
            ),
            1
        );
        assert_eq!(occurrences(text, "  store float "), 4);
        assert!(text.contains("!reqd_work_group_size"));
        assert!(text.contains("!{i32 64, i32 1, i32 1}"));
        assert!(text.contains("section \".fe2o3.kd.v1\", align 8"));
        assert!(text.contains("section \".fe2o3.general-gemm.binding.v1\", align 16"));
        assert!(text.contains("@llvm.compiler.used = appending global [2 x ptr]"));
        assert!(text.contains("@general_gemm_descriptor_source to ptr"));
        assert!(text.contains("@general_gemm_compilation_binding to ptr"));
        assert_eq!(
            occurrences(text, "load <4 x i16>"),
            usize::from(core::ptr::eq(machine, &optimized))
        );

        let function = &machine.handoff().module().functions()[0];
        let mut vector_load_bases = Vec::new();
        let instructions = function
            .blocks()
            .iter()
            .flat_map(|block| block.instructions())
            .collect::<Vec<_>>();
        for instruction in &instructions {
            let fe2o3_llvm_handoff::InstructionKindV2::VectorLoad4 { pointer, .. } =
                instruction.kind()
            else {
                continue;
            };
            let producer = instructions
                .iter()
                .find(|candidate| {
                    candidate
                        .result()
                        .is_some_and(|result| result.id() == *pointer)
                })
                .expect("vector pointer must be defined by the same function");
            let fe2o3_llvm_handoff::InstructionKindV2::GetElementPtr { base, .. } = producer.kind()
            else {
                panic!("vector load pointer must be derived by GEP");
            };
            vector_load_bases.push(*base);
        }
        let expected = if core::ptr::eq(machine, &optimized) {
            vec![fe2o3_llvm_handoff::ValueIdV2::new(1)]
        } else {
            vec![]
        };
        assert_eq!(vector_load_bases, expected, "B must remain scalar");
    }
}

#[test]
fn machine_sections_and_identities_reject_schedule_and_frontend_abi_substitution() {
    let reference_unit = unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let reference = lower_general_gemm_structural_machine_v1(&reference_unit).unwrap();
    let repeated = lower_general_gemm_structural_machine_v1(&reference_unit).unwrap();
    assert_eq!(
        reference.handoff().identity(),
        repeated.handoff().identity()
    );
    assert_eq!(reference.assembly().sha256(), repeated.assembly().sha256());
    assert_eq!(
        reference.compiler_handoff().identity(),
        repeated.compiler_handoff().identity()
    );
    assert_eq!(reference.binding_section(), repeated.binding_section());
    assert_eq!(reference.compiler_boundary(), repeated.compiler_boundary());

    let optimized = lower_general_gemm_structural_machine_v1(&unit(
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    ))
    .unwrap();
    assert_ne!(
        reference.binding_section().identity(),
        optimized.binding_section().identity()
    );
    assert_ne!(
        reference.handoff().identity(),
        optimized.handoff().identity()
    );
    assert_ne!(
        reference.compiler_boundary().identity(),
        optimized.compiler_boundary().identity()
    );
    assert_ne!(reference.assembly().sha256(), optimized.assembly().sha256());

    let substituted_frontend =
        GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
            identity(0x12),
            identity(0x41),
            identity(0x42),
            identity(0x99),
            GeneralGemmSymbolicPlanV1::canonical(),
            GeneralGemmSymbolicKirV1::canonical(),
        )
        .unwrap();
    let substituted = lower_general_gemm_structural_machine_v1(&unit_with_frontend(
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
        substituted_frontend,
    ))
    .unwrap();
    assert_ne!(
        reference.binding_section().identity(),
        substituted.binding_section().identity()
    );
    assert_ne!(
        reference.compiler_boundary().identity(),
        substituted.compiler_boundary().identity()
    );
    assert_ne!(
        reference.handoff().identity(),
        substituted.handoff().identity()
    );

    let decoded = fe2o3_llvm_handoff::Gfx942HandoffV2::decode_canonical(
        reference.handoff().encode_canonical().as_bytes(),
    )
    .unwrap();
    assert_eq!(decoded, *reference.handoff());
    let globals = reference.handoff().module().globals();
    assert_eq!(
        globals
            .iter()
            .filter(|global| {
                global.address_space() == fe2o3_llvm_handoff::AddressSpaceV1::Local
                    && global.array_elements() == Some(256)
            })
            .count(),
        2
    );
    let descriptor = globals
        .iter()
        .find(|global| global.section() == Some(fe2o3_llvm_handoff::KERNEL_DESCRIPTOR_SECTION_V2))
        .unwrap();
    assert_eq!(
        descriptor.byte_initializer(),
        Some(reference.descriptor_source().canonical_bytes())
    );
    let binding = globals
        .iter()
        .find(|global| {
            global.section() == Some(fe2o3_llvm_handoff::GENERAL_GEMM_BINDING_SECTION_V2)
        })
        .unwrap();
    assert_eq!(
        binding.byte_initializer(),
        Some(reference.binding_section().canonical_bytes())
    );
}

#[test]
fn symbolic_machine_lowers_dynamic_body_without_concrete_witness_plan() {
    for schedule in [
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    ] {
        let unit = symbolic_unit(schedule);
        let machine = lower_general_gemm_symbolic_structural_machine_v1(&unit).unwrap();
        assert_eq!(machine.projection().compilation_identity(), unit.identity());
        assert_eq!(
            machine.projection().symbolic_plan_identity(),
            unit.symbolic_plan_identity()
        );
        assert_eq!(
            machine.projection().symbolic_kir_identity(),
            unit.symbolic_kir_identity()
        );
        assert_eq!(
            machine.projection().operation_count(),
            GENERAL_GEMM_SYMBOLIC_LOWERED_OPERATION_COUNT_V1
        );
        assert_ne!(
            machine.projection().source_operation_identity().as_bytes(),
            &[0; 32]
        );
        assert_ne!(
            machine.projection().lowered_operation_identity().as_bytes(),
            &[0; 32]
        );
        assert_ne!(
            machine.projection().transformation_identity().as_bytes(),
            &[0; 32]
        );
        assert_eq!(
            machine
                .handoff()
                .base()
                .stage_identities()
                .schedule()
                .as_bytes(),
            unit.schedule_identity().as_bytes()
        );
        assert_eq!(
            machine.assembly().source_identity(),
            machine.handoff().identity()
        );
        assert!(!machine.grants_artifact_authority());
        assert_ne!(machine.artifact_identity().as_bytes(), &[0; 32]);
        let text = machine.assembly().as_str();
        assert_eq!(occurrences(text, "call void @llvm.amdgcn.s.barrier()"), 2);
        assert_eq!(
            occurrences(text, "load <4 x i16>"),
            usize::from(schedule.requires_vectorized_a_isa_confirmation())
        );
    }
}

#[test]
fn symbolic_machine_identity_rejects_schedule_and_frontend_substitution() {
    let reference = lower_general_gemm_symbolic_structural_machine_v1(&symbolic_unit(
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
    ))
    .unwrap();
    let vector = lower_general_gemm_symbolic_structural_machine_v1(&symbolic_unit(
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    ))
    .unwrap();
    assert_ne!(reference.artifact_identity(), vector.artifact_identity());
    assert_ne!(
        reference.compiler_boundary().identity(),
        vector.compiler_boundary().identity()
    );
    assert_ne!(
        reference.binding_section().identity(),
        vector.binding_section().identity()
    );
    assert_ne!(reference.assembly().sha256(), vector.assembly().sha256());

    let substituted_frontend =
        GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
            identity(0x12),
            identity(0x91),
            identity(0x42),
            identity(0x43),
            GeneralGemmSymbolicPlanV1::canonical(),
            GeneralGemmSymbolicKirV1::canonical(),
        )
        .unwrap();
    let substituted =
        lower_general_gemm_symbolic_structural_machine_v1(&symbolic_unit_with_frontend(
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
            substituted_frontend,
        ))
        .unwrap();
    assert_ne!(
        reference.artifact_identity(),
        substituted.artifact_identity()
    );
    assert_ne!(
        reference.binding_section().identity(),
        substituted.binding_section().identity()
    );
    assert_ne!(
        reference.compiler_boundary().identity(),
        substituted.compiler_boundary().identity()
    );
}

fn lowered_operation_pointers(
    envelope: &GeneralGemmSymbolicPlironEnvelope,
) -> Vec<pliron::context::Ptr<Operation>> {
    envelope
        .module
        .get_body(&envelope.context, 0)
        .deref(&envelope.context)
        .iter(&envelope.context)
        .collect()
}

#[test]
fn symbolic_pre_pass_rejects_schedule_substitution() {
    let unit = symbolic_unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let (context, module) = build_symbolic_source_module(&unit).unwrap();
    let operations: Vec<_> = module
        .get_body(&context, 0)
        .deref(&context)
        .iter(&context)
        .collect();
    let schedule = Operation::get_op::<GeneralGemmPlanOp>(operations[3], &context).unwrap();
    schedule.set_attr_general_gemm_kind(
        &context,
        GeneralGemmScheduleAttr::VectorizedAOnlyBf16GlobalTransferV1,
    );
    assert!(verify_operation(module.get_operation(), &context).is_err());
    assert!(validate_symbolic_source_operations(&context, &module, unit.schedule()).is_err());
}

#[test]
fn symbolic_post_pass_rejects_abi_payload_omission() {
    let unit = symbolic_unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let envelope = project_symbolic_to_pliron_inner(&unit).unwrap();
    let operations = lowered_operation_pointers(&envelope);
    operations[0]
        .deref_mut(&envelope.context)
        .attributes
        .0
        .clear();
    assert!(envelope.into_verified_lowered(&unit).is_err());
}

#[test]
fn symbolic_post_pass_rejects_epilogue_omission() {
    let unit = symbolic_unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let envelope = project_symbolic_to_pliron_inner(&unit).unwrap();
    let operations = lowered_operation_pointers(&envelope);
    operations[13]
        .deref_mut(&envelope.context)
        .attributes
        .0
        .clear();
    assert!(envelope.into_verified_lowered(&unit).is_err());
}

#[test]
fn symbolic_post_pass_rejects_epoch_reordering() {
    let unit = symbolic_unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let envelope = project_symbolic_to_pliron_inner(&unit).unwrap();
    let operations = lowered_operation_pointers(&envelope);
    let publish =
        Operation::get_op::<GeneralGemmEpochOp>(operations[7], &envelope.context).unwrap();
    let reuse = Operation::get_op::<GeneralGemmEpochOp>(operations[11], &envelope.context).unwrap();
    publish.set_attr_general_gemm_epoch(
        &envelope.context,
        GeneralGemmEpochAttr::ReuseWorkgroupAcquireReleaseV1,
    );
    reuse.set_attr_general_gemm_epoch(
        &envelope.context,
        GeneralGemmEpochAttr::PublishWorkgroupAcquireReleaseV1,
    );
    assert!(envelope.into_verified_lowered(&unit).is_err());
}

#[test]
fn symbolic_post_pass_rejects_cross_schedule_a_transfer() {
    let unit = symbolic_unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1);
    let envelope = project_symbolic_to_pliron_inner(&unit).unwrap();
    let operations = lowered_operation_pointers(&envelope);
    let a_transfer =
        Operation::get_op::<GeneralGemmGlobalTransferOp>(operations[3], &envelope.context).unwrap();
    a_transfer.set_attr_general_gemm_global_transfer(
        &envelope.context,
        GeneralGemmGlobalTransferAttr::AVector4AlignedFullScalarFallbackZeroFillV1,
    );
    assert!(envelope.into_verified_lowered(&unit).is_err());
}
