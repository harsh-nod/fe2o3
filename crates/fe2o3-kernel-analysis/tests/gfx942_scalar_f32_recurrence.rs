#![cfg(target_os = "linux")]

use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_amdgcn_model::{
    ProductionTargetLaunchEvidenceV13, lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1,
};
use fe2o3_compiler_lineage::{
    ExactLlvmPassOccurrenceContentsV1, ExactProductionLlvmPhaseContentsV1,
    FIXED_PRODUCTION_LLVM_PHASES_V1, LlvmPassInvocationV1, LlvmPassIrUnitV1,
    PostLlvmPipelineOccurrenceTranscriptV1, PostLlvmStageCustodyV1,
    StructuredKirControlEdgeLoweringV1, StructuredKirOperationKindV1,
    StructuredKirOperationLoweringV1, StructuredKirToLlvmDerivationPartsV1,
    StructuredKirToLlvmDerivationV1, StructuredKirValueCarrierV1, StructuredKirValueLoweringV1,
    StructuredKirValueTypeV1, StructuredLlvmOpcodeV1, StructuredLlvmTargetV1,
    check_exact_fixed_production_pipeline_contents_v1,
    check_exact_post_llvm_pipeline_occurrence_v1, check_exact_post_llvm_stage_contents_v1,
};
use fe2o3_kernel_analysis::{
    AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
    Gfx942OperationalTranslationUnsupportedV1, Gfx942ScalarF32KirInputV1,
    Gfx942ScalarF32KirRecurrenceErrorV1, Gfx942ScalarF32RecurrenceStepAnalysisErrorV1,
    Gfx942ScalarF32RecurrenceStepArtifactErrorV1, Gfx942ScalarF32RecurrenceStepArtifactV1,
    PhysicalMachineEffectBudgetV1, PhysicalMachineEffectEntryRequestV1,
    audit_authenticated_gfx942_scalar_f32_machine_refinement_inputs_v1,
    bind_gfx942_scalar_f32_kir_and_authenticated_machine_recurrence_v1,
    bind_gfx942_scalar_f32_post_llvm_and_authenticated_machine_v1,
    bind_gfx942_scalar_f32_post_llvm_stage_contents_v1,
    bind_gfx942_scalar_f32_structured_llvm_and_authenticated_machine_recurrence_v1,
    check_authenticated_gfx942_scalar_f32_recurrence_step_v1,
    check_gfx942_scalar_f32_structured_kir_to_llvm_refinement_v1,
    check_verified_canonical_gfx942_scalar_f32_kir_recurrence_v1,
    inspect_physical_machine_effect_worker_candidate_v1,
    require_authenticated_gfx942_scalar_f32_machine_refinement_after_kir_v1,
    require_authenticated_gfx942_scalar_f32_machine_refinement_after_structured_llvm_v1,
    require_authenticated_gfx942_scalar_f32_machine_refinement_v1,
    require_gfx942_scalar_f32_machine_refinement_after_post_llvm_content_v1,
    require_gfx942_scalar_f32_machine_refinement_after_scalar_correspondence_v1,
    verify_authenticated_gfx942_scalar_f32_recurrence_step_artifact_v1,
};
use fe2o3_kernel_ir as kir;
use std::{
    path::Path,
    sync::{Mutex, OnceLock},
    time::Duration,
};

static WORKER_LOCK: Mutex<()> = Mutex::new(());

fn fixture() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_fe2o3-machine-effect-worker-fixture"))
}

fn limits() -> AuthenticatedPhysicalMachineEffectLimitsV1 {
    AuthenticatedPhysicalMachineEffectLimitsV1::new(Duration::from_secs(30), 1024 * 1024, 16 * 1024)
        .unwrap()
}

fn worker() -> &'static AuthenticatedPhysicalMachineEffectWorkerV1 {
    static WORKER: OnceLock<AuthenticatedPhysicalMachineEffectWorkerV1> = OnceLock::new();
    WORKER.get_or_init(|| {
        let candidate =
            inspect_physical_machine_effect_worker_candidate_v1(fixture(), limits()).unwrap();
        AuthenticatedPhysicalMachineEffectWorkerV1::open(fixture(), candidate.policy(), limits())
            .unwrap()
    })
}

fn execution(mode: u8) -> fe2o3_kernel_analysis::AuthenticatedPhysicalMachineAnalysisExecutionV1 {
    let _guard = WORKER_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let payload_len = if mode == 34 { 20 } else { 16 };
    let mut payload = (0_u8..payload_len).collect::<Vec<_>>();
    payload[0] = mode;
    let entry = PhysicalMachineEffectEntryRequestV1::new(
        "scalar_gemm_v1",
        PhysicalMachineEffectBudgetV1::new(0, 0, 0, 1, 0),
    )
    .unwrap();
    worker().analyze(payload, vec![entry], limits()).unwrap()
}

fn positive() -> AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
    check_authenticated_gfx942_scalar_f32_recurrence_step_v1(execution(27), "scalar_gemm_v1")
        .unwrap()
}

#[derive(Clone, Copy, Debug)]
enum KirMutation {
    None,
    LoopTrip,
    PhiSource,
    AddressStride,
    OuterGuard,
    WriteDestination,
    WriteCount,
    InactivePath,
    WriterInjectivity,
}

fn result_operation(id: u32, ty: kir::Type, kind: kir::OperationKind) -> kir::Operation {
    kir::Operation::effect_free(kir::ValueDef::new(kir::ValueId(id), ty), kind)
}

fn binary(id: u32, ty: kir::Type, op: kir::BinaryOp, lhs: u32, rhs: u32) -> kir::Operation {
    result_operation(
        id,
        ty,
        kir::OperationKind::Binary {
            op,
            lhs: kir::ValueId(lhs),
            rhs: kir::ValueId(rhs),
        },
    )
}

fn compare(id: u32, predicate: kir::ComparePredicate, lhs: u32, rhs: u32) -> kir::Operation {
    result_operation(
        id,
        kir::Type::BOOL,
        kir::OperationKind::Compare {
            predicate,
            lhs: kir::ValueId(lhs),
            rhs: kir::ValueId(rhs),
        },
    )
}

fn canonical_scalar_gemm(mutation: KirMutation) -> kir::VerifiedCanonicalKernelIrV13 {
    const A: u32 = 0;
    const B: u32 = 1;
    const C: u32 = 2;
    const M: u32 = 3;
    const N: u32 = 4;
    const K: u32 = 5;
    const CONTEXT: u32 = 6;
    const A_CAPABILITY: u32 = 7;
    const B_CAPABILITY: u32 = 8;
    const C_CAPABILITY: u32 = 9;
    const GLOBAL_ID: u32 = 10;
    const ZERO_INDEX: u32 = 11;
    const ZERO_F32: u32 = 12;
    const SAFE_ONE: u32 = 13;
    const N_NONZERO: u32 = 14;
    const SAFE_N: u32 = 15;
    const OUTPUT_EXTENT: u32 = 16;
    const OUTPUT_IN_RANGE: u32 = 17;
    const ACTIVE: u32 = 18;
    const ROW: u32 = 19;
    const COLUMN: u32 = 20;
    const ITERATION: u32 = 21;
    const ACCUMULATOR: u32 = 22;
    const LOOP_CONDITION: u32 = 23;
    const A_STRIDE: u32 = 24;
    const A_INDEX: u32 = 25;
    const A_PROJECTED: u32 = 26;
    const A_LENGTH: u32 = 27;
    const A_GUARD: u32 = 28;
    const A_SAFE_INDEX: u32 = 29;
    const A_DATA: u32 = 30;
    const A_POINTER: u32 = 31;
    const A_VALUE: u32 = 32;
    const B_STRIDE: u32 = 33;
    const B_INDEX: u32 = 34;
    const B_PROJECTED: u32 = 35;
    const B_LENGTH: u32 = 36;
    const B_GUARD: u32 = 37;
    const B_SAFE_INDEX: u32 = 38;
    const B_DATA: u32 = 39;
    const B_POINTER: u32 = 40;
    const B_VALUE: u32 = 41;
    const PRODUCT: u32 = 42;
    const NEXT_ACCUMULATOR: u32 = 43;
    const LOOP_ONE: u32 = 44;
    const NEXT_ITERATION: u32 = 45;
    const C_PROJECTED: u32 = 46;
    const C_LENGTH: u32 = 47;
    const C_GUARD: u32 = 48;
    const C_SAFE_INDEX: u32 = 49;
    const C_DATA: u32 = 50;
    const C_POINTER: u32 = 51;

    let context =
        kir::KernelContextTypeV1::new("scalar_gemm_v1", [0x11; 32], [0x12; 32], [0x13; 32]);
    let source =
        kir::KernelContextSourceIdentityV1::new([0x21; 32], [0x22; 32], [0x23; 32], [0x24; 32]);
    let a_capability = kir::GlobalCapabilityTypeV1::read_only(kir::Type::F32, context.clone());
    let b_capability = kir::GlobalCapabilityTypeV1::read_only(kir::Type::F32, context.clone());
    let write_mapping = if matches!(mutation, KirMutation::WriterInjectivity) {
        kir::GlobalDisjointIndexSpaceV1::ShiftedIndex1d { offset: 1 }
    } else {
        kir::GlobalDisjointIndexSpaceV1::Index1d
    };
    let write_contract = kir::GlobalDisjointIndexContractV1::new([0x31; 32], write_mapping);
    let c_capability = kir::GlobalCapabilityTypeV1::disjoint_write(
        kir::Type::F32,
        context.clone(),
        write_contract,
    );
    let read_pointer = kir::Type::pointer(
        kir::Type::F32,
        kir::AddressSpace::Global,
        kir::AccessMode::ReadOnly,
    );
    let write_pointer = kir::Type::pointer(
        kir::Type::F32,
        kir::AddressSpace::Global,
        kir::AccessMode::WriteOnly,
    );
    let access = kir::MemoryAccess::new(kir::AddressSpace::Global, 4);

    let mut entry = kir::BasicBlock::new(kir::BlockId(0));
    entry.operations = vec![
        kir::Operation::kernel_context_issue(kir::ValueId(CONTEXT), context.clone(), source),
        kir::Operation::global_capability_bind(
            kir::ValueId(A_CAPABILITY),
            a_capability,
            kir::ValueId(CONTEXT),
            kir::ValueId(A),
        ),
        kir::Operation::global_capability_bind(
            kir::ValueId(B_CAPABILITY),
            b_capability,
            kir::ValueId(CONTEXT),
            kir::ValueId(B),
        ),
        kir::Operation::global_capability_bind(
            kir::ValueId(C_CAPABILITY),
            c_capability,
            kir::ValueId(CONTEXT),
            kir::ValueId(C),
        ),
        result_operation(
            GLOBAL_ID,
            kir::Type::INDEX,
            kir::OperationKind::Intrinsic(kir::IntrinsicOperation::global_id_1d()),
        ),
        result_operation(
            ZERO_INDEX,
            kir::Type::INDEX,
            kir::OperationKind::Constant(kir::Constant::Index(0)),
        ),
        result_operation(
            ZERO_F32,
            kir::Type::F32,
            kir::OperationKind::Constant(kir::Constant::F32Bits(0)),
        ),
        result_operation(
            SAFE_ONE,
            kir::Type::INDEX,
            kir::OperationKind::Constant(kir::Constant::Index(1)),
        ),
        compare(N_NONZERO, kir::ComparePredicate::NotEqual, N, ZERO_INDEX),
        result_operation(
            SAFE_N,
            kir::Type::INDEX,
            kir::OperationKind::Select {
                condition: kir::ValueId(N_NONZERO),
                true_value: kir::ValueId(N),
                false_value: kir::ValueId(SAFE_ONE),
            },
        ),
        binary(
            OUTPUT_EXTENT,
            kir::Type::INDEX,
            kir::BinaryOp::Multiply,
            M,
            N,
        ),
        compare(
            OUTPUT_IN_RANGE,
            kir::ComparePredicate::LessThan,
            GLOBAL_ID,
            OUTPUT_EXTENT,
        ),
        binary(
            ACTIVE,
            kir::Type::BOOL,
            kir::BinaryOp::BitAnd,
            N_NONZERO,
            OUTPUT_IN_RANGE,
        ),
        binary(
            ROW,
            kir::Type::INDEX,
            kir::BinaryOp::Divide,
            GLOBAL_ID,
            SAFE_N,
        ),
        binary(
            COLUMN,
            kir::Type::INDEX,
            kir::BinaryOp::Remainder,
            GLOBAL_ID,
            SAFE_N,
        ),
    ];
    entry.terminator = Some(kir::Terminator::ConditionalBranch {
        condition: kir::ValueId(if matches!(mutation, KirMutation::OuterGuard) {
            OUTPUT_IN_RANGE
        } else {
            ACTIVE
        }),
        then_target: kir::BlockId(1),
        then_arguments: vec![kir::ValueId(ZERO_INDEX), kir::ValueId(ZERO_F32)],
        else_target: kir::BlockId(if matches!(mutation, KirMutation::InactivePath) {
            1
        } else {
            4
        }),
        else_arguments: if matches!(mutation, KirMutation::InactivePath) {
            vec![kir::ValueId(ZERO_INDEX), kir::ValueId(ZERO_F32)]
        } else {
            vec![]
        },
    });

    let mut header = kir::BasicBlock::new(kir::BlockId(1));
    header.parameters = vec![
        kir::ValueDef::new(kir::ValueId(ITERATION), kir::Type::INDEX),
        kir::ValueDef::new(kir::ValueId(ACCUMULATOR), kir::Type::F32),
    ];
    header.operations = vec![compare(
        LOOP_CONDITION,
        kir::ComparePredicate::LessThan,
        ITERATION,
        if matches!(mutation, KirMutation::LoopTrip) {
            M
        } else {
            K
        },
    )];
    header.terminator = Some(kir::Terminator::ConditionalBranch {
        condition: kir::ValueId(LOOP_CONDITION),
        then_target: kir::BlockId(2),
        then_arguments: vec![],
        else_target: kir::BlockId(3),
        else_arguments: vec![],
    });

    let mut body = kir::BasicBlock::new(kir::BlockId(2));
    body.operations = vec![
        binary(
            A_STRIDE,
            kir::Type::INDEX,
            kir::BinaryOp::Multiply,
            ROW,
            if matches!(mutation, KirMutation::AddressStride) {
                N
            } else {
                K
            },
        ),
        binary(
            A_INDEX,
            kir::Type::INDEX,
            kir::BinaryOp::Add,
            A_STRIDE,
            ITERATION,
        ),
        kir::Operation::global_capability_index(
            kir::ValueId(A_PROJECTED),
            kir::ValueId(A_CAPABILITY),
            kir::ValueId(A_INDEX),
            None,
        ),
        result_operation(
            A_LENGTH,
            kir::Type::INDEX,
            kir::OperationKind::SliceLength {
                slice: kir::ValueId(A_CAPABILITY),
            },
        ),
        compare(
            A_GUARD,
            kir::ComparePredicate::LessThan,
            A_PROJECTED,
            A_LENGTH,
        ),
        result_operation(
            A_SAFE_INDEX,
            kir::Type::INDEX,
            kir::OperationKind::Select {
                condition: kir::ValueId(A_GUARD),
                true_value: kir::ValueId(A_PROJECTED),
                false_value: kir::ValueId(ZERO_INDEX),
            },
        ),
        result_operation(
            A_DATA,
            read_pointer.clone(),
            kir::OperationKind::SliceData {
                slice: kir::ValueId(A_CAPABILITY),
            },
        ),
        result_operation(
            A_POINTER,
            read_pointer.clone(),
            kir::OperationKind::GetElementPointer {
                base: kir::ValueId(A_DATA),
                offset: kir::ValueId(A_SAFE_INDEX),
            },
        ),
        result_operation(
            A_VALUE,
            kir::Type::F32,
            kir::OperationKind::GuardedLoad {
                pointer: kir::ValueId(A_POINTER),
                predicate: kir::ValueId(A_GUARD),
                fallback: kir::ValueId(ZERO_F32),
                access,
            },
        ),
        binary(
            B_STRIDE,
            kir::Type::INDEX,
            kir::BinaryOp::Multiply,
            ITERATION,
            N,
        ),
        binary(
            B_INDEX,
            kir::Type::INDEX,
            kir::BinaryOp::Add,
            B_STRIDE,
            COLUMN,
        ),
        kir::Operation::global_capability_index(
            kir::ValueId(B_PROJECTED),
            kir::ValueId(B_CAPABILITY),
            kir::ValueId(B_INDEX),
            None,
        ),
        result_operation(
            B_LENGTH,
            kir::Type::INDEX,
            kir::OperationKind::SliceLength {
                slice: kir::ValueId(B_CAPABILITY),
            },
        ),
        compare(
            B_GUARD,
            kir::ComparePredicate::LessThan,
            B_PROJECTED,
            B_LENGTH,
        ),
        result_operation(
            B_SAFE_INDEX,
            kir::Type::INDEX,
            kir::OperationKind::Select {
                condition: kir::ValueId(B_GUARD),
                true_value: kir::ValueId(B_PROJECTED),
                false_value: kir::ValueId(ZERO_INDEX),
            },
        ),
        result_operation(
            B_DATA,
            read_pointer.clone(),
            kir::OperationKind::SliceData {
                slice: kir::ValueId(B_CAPABILITY),
            },
        ),
        result_operation(
            B_POINTER,
            read_pointer,
            kir::OperationKind::GetElementPointer {
                base: kir::ValueId(B_DATA),
                offset: kir::ValueId(B_SAFE_INDEX),
            },
        ),
        result_operation(
            B_VALUE,
            kir::Type::F32,
            kir::OperationKind::GuardedLoad {
                pointer: kir::ValueId(B_POINTER),
                predicate: kir::ValueId(B_GUARD),
                fallback: kir::ValueId(ZERO_F32),
                access,
            },
        ),
        binary(
            PRODUCT,
            kir::Type::F32,
            kir::BinaryOp::Multiply,
            A_VALUE,
            B_VALUE,
        ),
        binary(
            NEXT_ACCUMULATOR,
            kir::Type::F32,
            kir::BinaryOp::Add,
            PRODUCT,
            ACCUMULATOR,
        ),
        result_operation(
            LOOP_ONE,
            kir::Type::INDEX,
            kir::OperationKind::Constant(kir::Constant::Index(1)),
        ),
        binary(
            NEXT_ITERATION,
            kir::Type::INDEX,
            kir::BinaryOp::Add,
            ITERATION,
            LOOP_ONE,
        ),
    ];
    body.terminator = Some(kir::Terminator::Branch {
        target: kir::BlockId(1),
        arguments: vec![
            kir::ValueId(NEXT_ITERATION),
            kir::ValueId(if matches!(mutation, KirMutation::PhiSource) {
                PRODUCT
            } else {
                NEXT_ACCUMULATOR
            }),
        ],
    });

    let mut store = kir::BasicBlock::new(kir::BlockId(3));
    store.operations = vec![
        kir::Operation::global_capability_index(
            kir::ValueId(C_PROJECTED),
            kir::ValueId(C_CAPABILITY),
            kir::ValueId(if matches!(mutation, KirMutation::WriteDestination) {
                ROW
            } else {
                GLOBAL_ID
            }),
            Some(write_contract),
        ),
        result_operation(
            C_LENGTH,
            kir::Type::INDEX,
            kir::OperationKind::SliceLength {
                slice: kir::ValueId(C_CAPABILITY),
            },
        ),
        compare(
            C_GUARD,
            kir::ComparePredicate::LessThan,
            C_PROJECTED,
            C_LENGTH,
        ),
        result_operation(
            C_SAFE_INDEX,
            kir::Type::INDEX,
            kir::OperationKind::Select {
                condition: kir::ValueId(C_GUARD),
                true_value: kir::ValueId(C_PROJECTED),
                false_value: kir::ValueId(ZERO_INDEX),
            },
        ),
        result_operation(
            C_DATA,
            write_pointer.clone(),
            kir::OperationKind::SliceData {
                slice: kir::ValueId(C_CAPABILITY),
            },
        ),
        result_operation(
            C_POINTER,
            write_pointer,
            kir::OperationKind::GetElementPointer {
                base: kir::ValueId(C_DATA),
                offset: kir::ValueId(C_SAFE_INDEX),
            },
        ),
        kir::Operation::new(
            vec![],
            kir::OperationKind::GuardedStore {
                pointer: kir::ValueId(C_POINTER),
                predicate: kir::ValueId(C_GUARD),
                value: kir::ValueId(ACCUMULATOR),
                access,
            },
        ),
    ];
    if matches!(mutation, KirMutation::WriteCount) {
        store.operations.push(store.operations[6].clone());
    }
    store.terminator = Some(kir::Terminator::Branch {
        target: kir::BlockId(4),
        arguments: vec![],
    });

    let mut exit = kir::BasicBlock::new(kir::BlockId(4));
    exit.terminator = Some(kir::Terminator::Return { values: vec![] });

    let input = kir::Type::slice(
        kir::Type::F32,
        kir::AddressSpace::Global,
        kir::AccessMode::ReadOnly,
    );
    let output = kir::Type::slice(
        kir::Type::F32,
        kir::AddressSpace::Global,
        kir::AccessMode::WriteOnly,
    );
    let mut module = kir::Module::new("scalar_gemm_v1");
    module.functions.push(kir::Function::kernel_entry(
        "scalar_gemm_v1",
        kir::Signature::new(
            vec![
                input.clone(),
                input,
                output,
                kir::Type::INDEX,
                kir::Type::INDEX,
                kir::Type::INDEX,
            ],
            vec![],
        ),
        vec![
            kir::ValueId(A),
            kir::ValueId(B),
            kir::ValueId(C),
            kir::ValueId(M),
            kir::ValueId(N),
            kir::ValueId(K),
        ],
        vec![entry, header, body, store, exit],
    ));
    let mut kernel = kir::Kernel::new(
        "scalar_gemm_v1",
        "scalar_gemm_v1",
        kir::LaunchDomain::D1 {
            x: kir::LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(kir::WorkgroupSize::new(256, 1, 1));
    module.kernels.push(kernel);
    kir::VerifiedCanonicalKernelIrV13::from_module(module).unwrap()
}

#[test]
fn full_refinement_gate_reports_every_missing_typed_input_and_retains_custody() {
    let readiness = audit_authenticated_gfx942_scalar_f32_machine_refinement_inputs_v1(
        execution(27),
        "scalar_gemm_v1",
    )
    .unwrap();
    assert_eq!(
        readiness
            .missing_inputs()
            .iter()
            .map(|input| input.diagnostic_name())
            .collect::<Vec<_>>(),
        [
            "verified-canonical-kir-v13-owner",
            "kir-to-llvm-ssa-cfg-phi-abi-replay",
            "pre-post-optimization-bitcode-and-pass-configuration",
            "llvm-text-to-pre-optimization-bitcode-assembly",
            "llvm-optimization-semantic-preservation",
            "llvm-to-gfx942-instruction-selection",
            "generated-relocatable-object",
            "object-to-hsaco-relocation-symbol-section-preservation",
            "gfx942-control-address-operational-semantics",
            "gfx942-ieee-binary32-operational-semantics",
            "machine-loop-effects-writer-injectivity",
        ],
    );
    assert!(!readiness.establishes_kir_to_final_machine_refinement());
    assert!(!readiness.grants_worker_v3_refinement_authority());
    assert!(!readiness.grants_load_or_launch_authority());
    assert!(
        readiness
            .into_authenticated_execution()
            .authenticates_analyzer_execution()
    );

    let incomplete = require_authenticated_gfx942_scalar_f32_machine_refinement_v1(
        execution(27),
        "scalar_gemm_v1",
    )
    .unwrap_err();
    assert!(incomplete.recurrence_error().is_none());
    for name in [
        "verified-canonical-kir-v13-owner",
        "kir-to-llvm-ssa-cfg-phi-abi-replay",
        "object-to-hsaco-relocation-symbol-section-preservation",
        "gfx942-ieee-binary32-operational-semantics",
        "machine-loop-effects-writer-injectivity",
    ] {
        assert!(incomplete.to_string().contains(name));
    }
    assert!(
        incomplete
            .into_authenticated_execution()
            .authenticates_analyzer_execution()
    );
}

#[test]
fn authenticated_recurrence_step_retains_exact_inert_artifact() {
    let checked = positive();
    let artifact = checked.artifact();
    assert_eq!(artifact.function_symbol(), "scalar_gemm_v1");
    assert_eq!(artifact.multiply_offset(), 4);
    assert_eq!(artifact.add_offset(), 8);
    assert_eq!(artifact.product_register(), 1);
    assert_eq!(artifact.accumulator_register(), 0);
    assert_eq!(artifact.result_register(), 0);
    assert_eq!(artifact.product_source_operand_index(), 0);
    assert_eq!(artifact.accumulator_source_operand_index(), 1);
    assert!(artifact.binds_authenticated_trace_and_exact_instruction_encodings());
    assert!(artifact.validates_separate_recurrence_step_dataflow_shape());
    assert!(artifact.excludes_fused_definitions_from_step_inputs());
    assert!(artifact.provides_executable_candidate_numeric_semantics());
    assert!(!artifact.establishes_machine_loop_recurrence());
    assert!(!artifact.establishes_gfx942_instruction_semantics());
    assert!(!artifact.establishes_compiler_refinement());
    assert!(!artifact.grants_worker_v3_refinement_authority());
    assert!(!artifact.grants_load_or_launch_authority());
    assert!(checked.authenticates_analyzer_execution());
    assert!(!checked.establishes_semantic_machine_refinement());
    assert!(!checked.grants_runtime_authority());

    let decoded =
        Gfx942ScalarF32RecurrenceStepArtifactV1::decode_canonical(artifact.canonical_bytes())
            .unwrap();
    assert_eq!(decoded, *artifact);
    assert_eq!(decoded.identity(), artifact.identity());
    assert_eq!(
        decoded.authenticated_execution_identity().0,
        &checked.authenticated_execution_identity().sha256()
    );
}

#[test]
fn persisted_artifact_requires_exact_authenticated_replay() {
    let checked = positive();
    let canonical = checked.artifact().canonical_bytes().to_vec();
    let execution = checked.into_authenticated_execution();
    let replayed = verify_authenticated_gfx942_scalar_f32_recurrence_step_artifact_v1(
        execution,
        "scalar_gemm_v1",
        &canonical,
    )
    .unwrap();
    assert_eq!(replayed.artifact().canonical_bytes(), canonical);

    let checked = positive();
    let mut mutated = checked.artifact().canonical_bytes().to_vec();
    let execution = checked.into_authenticated_execution();
    let final_byte = mutated.last_mut().unwrap();
    *final_byte ^= 1;
    let failure = verify_authenticated_gfx942_scalar_f32_recurrence_step_artifact_v1(
        execution,
        "scalar_gemm_v1",
        &mutated,
    )
    .unwrap_err();
    assert!(matches!(
        failure.error(),
        Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ArtifactMismatch
            | Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::Artifact(
                Gfx942ScalarF32RecurrenceStepArtifactErrorV1::InvalidField
            )
    ));
}

#[test]
fn opcode_dataflow_order_and_contraction_mutations_fail_named_obligations() {
    for (mode, expected) in [
        (28, "multiply-count"),
        (29, "product-dataflow"),
        (30, "fused-dataflow"),
        (31, "dominance"),
        (32, "operand-order"),
        (33, "accumulator-update"),
        (34, "fused-copy-dataflow"),
    ] {
        let failure = check_authenticated_gfx942_scalar_f32_recurrence_step_v1(
            execution(mode),
            "scalar_gemm_v1",
        )
        .unwrap_err();
        let matched = matches!(
            (expected, failure.error()),
            (
                "multiply-count",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::WrongMultiplyCount { actual: 0 },
            ) | (
                "product-dataflow",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ProductDoesNotReachAdd,
            ) | (
                "fused-dataflow",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::FusedDefinitionReachesAdd { .. },
            ) | (
                "fused-copy-dataflow",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::FusedDefinitionReachesAdd {
                    offset: 0,
                },
            ) | (
                "operand-order",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ProductOperandPosition { actual: 1 },
            ) | (
                "accumulator-update",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ResultDoesNotUpdateAccumulator {
                    result: 7,
                    accumulator: 0,
                },
            ) | (
                "dominance",
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::MultiplyDoesNotDominateAdd,
            )
        );
        assert!(matched, "mode {mode} did not fail {expected}: {failure:?}");
        let (execution, _) = failure.into_parts();
        if mode == 28 {
            let incomplete = require_authenticated_gfx942_scalar_f32_machine_refinement_v1(
                execution,
                "scalar_gemm_v1",
            )
            .unwrap_err();
            assert!(matches!(
                incomplete.recurrence_error(),
                Some(
                    Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::WrongMultiplyCount { actual: 0 }
                )
            ));
            assert!(
                incomplete
                    .into_authenticated_execution()
                    .authenticates_analyzer_execution()
            );
        } else {
            assert!(execution.authenticates_analyzer_execution());
        }
    }
}

#[test]
fn exact_canonical_kir_recurrence_produces_inert_move_only_evidence() {
    let checked = check_verified_canonical_gfx942_scalar_f32_kir_recurrence_v1(
        canonical_scalar_gemm(KirMutation::None),
        "scalar_gemm_v1",
    )
    .unwrap();
    assert_eq!(checked.function_symbol(), "scalar_gemm_v1");
    assert!(checked.establishes_exact_kir_recurrence_ssa_cfg_phi_and_effects());
    assert!(!checked.establishes_kir_to_machine_refinement());
    assert!(!checked.grants_runtime_authority());
    assert_eq!(
        checked.evidence_identity().byte_len(),
        checked.canonical_evidence_bytes().len() as u64
    );
    assert_ne!(checked.evidence_identity().sha256(), [0; 32]);
    assert!(checked.into_verified_canonical_kir().revalidate().is_ok());
}

fn lower_scalar_gemm() -> (
    kir::VerifiedCanonicalKernelIrV13,
    Vec<u8>,
    StructuredKirToLlvmDerivationV1,
) {
    let canonical = canonical_scalar_gemm(KirMutation::None);
    let launch = ProductionTargetLaunchEvidenceV13::for_static_launches(&canonical, 1).unwrap();
    let lowered = lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
        &canonical,
        1,
        &launch,
        ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    (
        canonical,
        lowered.llvm_ir().as_bytes().to_vec(),
        lowered.structured_derivation().unwrap().clone(),
    )
}

fn replay_derivation(
    canonical: kir::VerifiedCanonicalKernelIrV13,
    llvm: Vec<u8>,
    derivation: StructuredKirToLlvmDerivationV1,
) -> Result<
    fe2o3_kernel_analysis::CheckedGfx942ScalarF32KirToLlvmRefinementV1,
    Box<fe2o3_kernel_analysis::Gfx942ScalarF32KirToLlvmRefinementFailureV1>,
> {
    let checked =
        check_verified_canonical_gfx942_scalar_f32_kir_recurrence_v1(canonical, "scalar_gemm_v1")
            .unwrap();
    check_gfx942_scalar_f32_structured_kir_to_llvm_refinement_v1(
        checked,
        derivation_llvm(llvm),
        derivation,
    )
}

fn derivation_llvm(llvm: Vec<u8>) -> Box<[u8]> {
    llvm.into_boxed_slice()
}

fn post_llvm_contents(
    lowered_llvm: &[u8],
) -> fe2o3_compiler_lineage::CheckedPostLlvmStageContentsV1 {
    post_llvm_contents_with_final(lowered_llvm, b"exact-final-hsaco")
}

fn post_llvm_contents_with_final(
    lowered_llvm: &[u8],
    hsaco: &[u8],
) -> fe2o3_compiler_lineage::CheckedPostLlvmStageContentsV1 {
    let pre = b"BC\xc0\xde-pre-optimization";
    let post = b"BC\xc0\xde-post-optimization";
    let object = b"exact-relocatable-object";
    let worker: &[u8] = b"exact-production-worker";
    let assembler: &[u8] = b"exact-llvm-text-assembler";
    let assembled = b"BC\xc0\xde-exact-text-assembly-replay";
    let after_strip = b"bitcode-after-strip-debug";
    let after_exports = b"bitcode-after-export-preservation";
    let after_o2 = b"bitcode-after-expanded-default-o2";
    let phases = FIXED_PRODUCTION_LLVM_PHASES_V1
        .into_iter()
        .zip([
            (lowered_llvm, pre.as_slice()),
            (pre.as_slice(), after_strip.as_slice()),
            (after_strip.as_slice(), after_exports.as_slice()),
            (after_exports.as_slice(), after_o2.as_slice()),
            (after_o2.as_slice(), post.as_slice()),
            (post.as_slice(), post.as_slice()),
            (post.as_slice(), object.as_slice()),
            (object.as_slice(), hsaco),
        ])
        .map(|(phase, (input, output))| {
            ExactProductionLlvmPhaseContentsV1::new(phase, input, output).unwrap()
        })
        .collect::<Vec<_>>();
    let pass_occurrences = vec![
        ExactLlvmPassOccurrenceContentsV1::new(
            0,
            1,
            LlvmPassIrUnitV1::Module,
            "default-O2",
            b"verify-each".as_slice(),
            after_exports.as_slice(),
            after_o2.as_slice(),
        )
        .unwrap(),
    ];
    let record = PostLlvmStageCustodyV1::from_exact_stage_bytes(
        lowered_llvm,
        pre,
        post,
        object,
        hsaco,
        "llvmorg-22.0.0-fe2o3",
        vec![LlvmPassInvocationV1::new("default-O2", b"verify-each".to_vec()).unwrap()],
    )
    .unwrap()
    .with_fixed_production_pipeline(worker, "worker-build-v1", &phases)
    .unwrap();
    let checked = check_exact_post_llvm_stage_contents_v1(
        record,
        lowered_llvm.to_vec(),
        pre.to_vec(),
        post.to_vec(),
        object.to_vec(),
        hsaco.to_vec(),
    )
    .unwrap();
    let checked = check_exact_fixed_production_pipeline_contents_v1(
        checked,
        worker,
        "worker-build-v1",
        "llvmorg-22.0.0-fe2o3",
        phases.clone(),
    )
    .unwrap();
    let transcript = PostLlvmPipelineOccurrenceTranscriptV1::capture_unavailable_production_v4(
        [1; 32],
        [2; 32],
        [3; 32],
        worker,
        assembler,
        "worker-build-v1",
        "llvmorg-22.0.0-fe2o3",
        lowered_llvm,
        assembled,
        &phases,
        &pass_occurrences,
    )
    .unwrap();
    check_exact_post_llvm_pipeline_occurrence_v1(
        checked,
        transcript,
        worker,
        assembler,
        lowered_llvm.to_vec(),
        assembled.to_vec(),
        lowered_llvm,
        assembled,
        phases,
        pass_occurrences,
    )
    .unwrap()
}

#[test]
fn production_v13_scalar_lowering_replays_to_move_only_authority_free_receipt() {
    let (canonical, llvm, derivation) = lower_scalar_gemm();
    assert_eq!(derivation.operations().len(), 45);
    assert_eq!(derivation.values().len(), 52);
    assert_eq!(derivation.blocks().len(), 5);
    assert_eq!(derivation.edges().len(), 6);
    let receipt = replay_derivation(canonical, llvm, derivation).unwrap();
    assert!(receipt.establishes_exact_structured_kir_to_llvm_refinement());
    assert!(!receipt.establishes_llvm_to_machine_refinement());
    assert!(!receipt.grants_compiler_or_runtime_authority());
    assert_ne!(receipt.evidence_identity().sha256(), [0; 32]);

    let readiness = bind_gfx942_scalar_f32_structured_llvm_and_authenticated_machine_recurrence_v1(
        receipt,
        positive(),
    )
    .unwrap();
    assert!(readiness.establishes_structured_kir_to_llvm_refinement());
    assert!(!readiness.establishes_llvm_to_machine_refinement());
    assert!(!readiness.grants_authority());
    assert_eq!(
        readiness
            .missing_inputs()
            .iter()
            .map(|input| input.diagnostic_name())
            .collect::<Vec<_>>(),
        [
            "pre-post-optimization-bitcode-and-pass-configuration",
            "llvm-text-to-pre-optimization-bitcode-assembly",
            "llvm-optimization-semantic-preservation",
            "llvm-to-gfx942-instruction-selection",
            "generated-relocatable-object",
            "object-to-hsaco-relocation-symbol-section-preservation",
            "gfx942-control-address-operational-semantics",
            "gfx942-ieee-binary32-operational-semantics",
            "machine-loop-effects-writer-injectivity",
        ]
    );
    let incomplete =
        require_authenticated_gfx942_scalar_f32_machine_refinement_after_structured_llvm_v1(
            readiness,
        )
        .unwrap_err();
    assert_eq!(incomplete.missing_inputs().len(), 9);
    assert!(
        !incomplete
            .missing_inputs()
            .iter()
            .any(|input| input.diagnostic_name() == "kir-to-llvm-ssa-cfg-phi-abi-replay")
    );
}

#[test]
fn exact_post_llvm_content_narrows_custody_but_terminal_refinement_stays_unavailable() {
    let (canonical, llvm, derivation) = lower_scalar_gemm();
    let receipt = replay_derivation(canonical, llvm.clone(), derivation).unwrap();
    let readiness =
        bind_gfx942_scalar_f32_post_llvm_stage_contents_v1(receipt, post_llvm_contents(&llvm))
            .unwrap();
    assert!(readiness.retains_pre_post_optimization_bitcode_and_recorded_passes());
    assert!(readiness.retains_generated_object_and_final_code_object());
    assert!(readiness.retains_complete_pipeline_occurrence_and_assembly_replay());
    assert!(!readiness.establishes_llvm_text_to_bitcode_assembly());
    assert!(!readiness.establishes_llvm_optimization_semantic_preservation());
    assert!(!readiness.establishes_llvm_to_final_machine_refinement());
    assert!(!readiness.grants_authority());
    assert_ne!(readiness.evidence_identity().sha256(), [0; 32]);
    assert_eq!(
        readiness
            .missing_inputs()
            .iter()
            .map(|input| input.diagnostic_name())
            .collect::<Vec<_>>(),
        [
            "pre-post-optimization-bitcode-and-pass-configuration",
            "llvm-text-to-pre-optimization-bitcode-assembly",
            "llvm-optimization-semantic-preservation",
            "llvm-to-gfx942-instruction-selection",
            "object-to-hsaco-relocation-symbol-section-preservation",
            "gfx942-control-address-operational-semantics",
            "gfx942-ieee-binary32-operational-semantics",
            "machine-loop-effects-writer-injectivity",
        ]
    );
    let incomplete =
        require_gfx942_scalar_f32_machine_refinement_after_post_llvm_content_v1(readiness)
            .unwrap_err();
    assert_eq!(incomplete.missing_inputs().len(), 8);
}

#[test]
fn post_llvm_content_rejects_cross_lowering_substitution_and_retains_both_owners() {
    let (canonical, llvm, derivation) = lower_scalar_gemm();
    let receipt = replay_derivation(canonical, llvm, derivation).unwrap();
    let stages = post_llvm_contents(b"different lowered LLVM module");
    let failure = bind_gfx942_scalar_f32_post_llvm_stage_contents_v1(receipt, stages).unwrap_err();
    let (receipt, stages) = failure.into_parts();
    assert_ne!(receipt.llvm_ir(), stages.lowered_llvm_module());
}

#[test]
fn structured_replay_rejects_omission_reorder_opcode_type_operand_cfg_target_and_llvm_mutations() {
    fn reject(mut mutate: impl FnMut(&mut StructuredKirToLlvmDerivationPartsV1)) {
        let (canonical, llvm, derivation) = lower_scalar_gemm();
        let mut parts = derivation.into_parts();
        mutate(&mut parts);
        let hostile = StructuredKirToLlvmDerivationV1::from_parts(parts).unwrap();
        assert!(replay_derivation(canonical, llvm, hostile).is_err());
    }

    reject(|parts| {
        parts.operations = parts.operations[..parts.operations.len() - 1]
            .to_vec()
            .into_boxed_slice();
    });
    reject(|parts| parts.operations.swap(0, 1));
    reject(|parts| {
        let index = parts
            .operations
            .iter()
            .position(|op| op.kind() == StructuredKirOperationKindV1::F32Multiply)
            .unwrap();
        let op = &parts.operations[index];
        parts.operations[index] = StructuredKirOperationLoweringV1::new(
            op.function(),
            op.block(),
            op.operation(),
            op.kind(),
            op.operands().to_vec(),
            op.results().to_vec(),
            op.result_types().to_vec(),
            vec![StructuredLlvmOpcodeV1::AddF32],
        )
        .unwrap();
    });
    reject(|parts| {
        let index = parts
            .values
            .iter()
            .position(|value| value.ty() == StructuredKirValueTypeV1::F32)
            .unwrap();
        let value = parts.values[index];
        parts.values[index] = StructuredKirValueLoweringV1::new(
            value.function(),
            value.value(),
            StructuredKirValueTypeV1::Index,
            value.carrier(),
        );
    });
    reject(|parts| {
        let index = parts
            .values
            .iter()
            .position(|value| matches!(value.carrier(), StructuredKirValueCarrierV1::Phi { .. }))
            .unwrap();
        let value = parts.values[index];
        let StructuredKirValueCarrierV1::Phi { block, ordinal } = value.carrier() else {
            unreachable!()
        };
        parts.values[index] = StructuredKirValueLoweringV1::new(
            value.function(),
            value.value(),
            value.ty(),
            StructuredKirValueCarrierV1::Phi {
                block: block + 1,
                ordinal,
            },
        );
    });
    reject(|parts| {
        let index = parts
            .operations
            .iter()
            .position(|op| op.operands().len() >= 2)
            .unwrap();
        let op = &parts.operations[index];
        let mut operands = op.operands().to_vec();
        operands.swap(0, 1);
        parts.operations[index] = StructuredKirOperationLoweringV1::new(
            op.function(),
            op.block(),
            op.operation(),
            op.kind(),
            operands,
            op.results().to_vec(),
            op.result_types().to_vec(),
            op.llvm_opcodes().to_vec(),
        )
        .unwrap();
    });
    reject(|parts| {
        let edge = &parts.edges[0];
        parts.edges[0] = StructuredKirControlEdgeLoweringV1::new(
            edge.function(),
            edge.predecessor(),
            edge.ordinal(),
            edge.successor() + 1,
            edge.arguments().to_vec(),
            edge.split_for_phi(),
        )
        .unwrap();
    });
    reject(|parts| parts.target = StructuredLlvmTargetV1::AmdGfx950XnackMinus);

    let (canonical, mut llvm, derivation) = lower_scalar_gemm();
    llvm[0] ^= 1;
    assert!(replay_derivation(canonical, llvm, derivation).is_err());
}

#[test]
fn kir_loop_phi_address_guard_write_and_injectivity_mutations_fail_named_obligations() {
    for (mutation, expected) in [
        (KirMutation::LoopTrip, "loop-trip"),
        (KirMutation::PhiSource, "phi-source"),
        (KirMutation::AddressStride, "address-stride"),
        (KirMutation::OuterGuard, "outer-guard"),
        (KirMutation::WriteDestination, "write-destination"),
        (KirMutation::WriteCount, "write-count"),
        (KirMutation::InactivePath, "inactive-path"),
        (KirMutation::WriterInjectivity, "writer-injectivity"),
    ] {
        let failure = check_verified_canonical_gfx942_scalar_f32_kir_recurrence_v1(
            canonical_scalar_gemm(mutation),
            "scalar_gemm_v1",
        )
        .unwrap_err();
        let named_failure = matches!(
            (expected, failure.error()),
            (
                "loop-trip",
                Gfx942ScalarF32KirRecurrenceErrorV1::LoopTripMismatch,
            ) | (
                "phi-source",
                Gfx942ScalarF32KirRecurrenceErrorV1::AccumulatorPhiSourceMismatch,
            ) | (
                "address-stride",
                Gfx942ScalarF32KirRecurrenceErrorV1::AddressStrideMismatch {
                    input: Gfx942ScalarF32KirInputV1::A,
                },
            ) | (
                "outer-guard",
                Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch,
            ) | (
                "write-destination",
                Gfx942ScalarF32KirRecurrenceErrorV1::WriteDestinationMismatch,
            ) | (
                "write-count",
                Gfx942ScalarF32KirRecurrenceErrorV1::WriteCount { actual: 2 },
            ) | (
                "inactive-path",
                Gfx942ScalarF32KirRecurrenceErrorV1::InactivePathMismatch,
            ) | (
                "writer-injectivity",
                Gfx942ScalarF32KirRecurrenceErrorV1::WriterInjectivityMismatch,
            )
        );
        assert!(
            named_failure,
            "mutation {mutation:?} did not fail {expected}: {failure:?}"
        );
        let (canonical, _) = failure.into_parts();
        assert!(canonical.revalidate().is_ok());
    }
}

#[test]
fn authenticated_kir_machine_conjunction_binds_evidence_but_terminal_stays_unavailable() {
    let checked_kir = check_verified_canonical_gfx942_scalar_f32_kir_recurrence_v1(
        canonical_scalar_gemm(KirMutation::None),
        "scalar_gemm_v1",
    )
    .unwrap();
    let readiness =
        bind_gfx942_scalar_f32_kir_and_authenticated_machine_recurrence_v1(checked_kir, positive())
            .unwrap();
    assert!(readiness.binds_exact_kir_and_authenticated_machine_step());
    assert!(!readiness.establishes_kir_to_final_machine_refinement());
    assert!(!readiness.grants_worker_v3_refinement_authority());
    assert!(!readiness.grants_load_or_launch_authority());
    assert_eq!(
        readiness.evidence_identity().byte_len(),
        readiness.canonical_evidence_bytes().len() as u64
    );

    let missing = readiness
        .missing_inputs()
        .iter()
        .map(|input| input.diagnostic_name())
        .collect::<Vec<_>>();
    assert_eq!(
        missing,
        [
            "kir-to-llvm-ssa-cfg-phi-abi-replay",
            "pre-post-optimization-bitcode-and-pass-configuration",
            "llvm-text-to-pre-optimization-bitcode-assembly",
            "llvm-optimization-semantic-preservation",
            "llvm-to-gfx942-instruction-selection",
            "generated-relocatable-object",
            "object-to-hsaco-relocation-symbol-section-preservation",
            "gfx942-control-address-operational-semantics",
            "gfx942-ieee-binary32-operational-semantics",
            "machine-loop-effects-writer-injectivity",
        ]
    );

    // There is intentionally no untyped mutation input for LLVM identity, object evidence, or
    // EXEC behavior. Their only current terminal behavior is this named fail-closed roster.
    for unavailable_mutation in [
        "kir-to-llvm-ssa-cfg-phi-abi-replay",
        "pre-post-optimization-bitcode-and-pass-configuration",
        "llvm-to-gfx942-instruction-selection",
        "generated-relocatable-object",
        "object-to-hsaco-relocation-symbol-section-preservation",
        "gfx942-control-address-operational-semantics",
    ] {
        assert!(missing.contains(&unavailable_mutation));
    }

    let incomplete =
        require_authenticated_gfx942_scalar_f32_machine_refinement_after_kir_v1(readiness)
            .unwrap_err();
    assert_eq!(incomplete.missing_inputs().len(), 10);
    let readiness = incomplete.into_readiness();
    let (kir, machine, evidence, identity) = readiness.into_parts();
    assert_eq!(identity.byte_len(), evidence.len() as u64);
    assert_eq!(identity.sha256().len(), 32);
    assert!(kir.into_verified_canonical_kir().revalidate().is_ok());
    assert!(machine.authenticates_analyzer_execution());
    assert!(!machine.establishes_semantic_machine_refinement());
}

#[test]
fn exact_final_hsaco_and_scalar_def_use_join_authenticated_machine_analysis() {
    let (canonical, llvm, derivation) = lower_scalar_gemm();
    let lowering = replay_derivation(canonical, llvm.clone(), derivation).unwrap();
    let machine = positive();
    let final_hsaco = machine
        .authenticated_execution()
        .request()
        .exact_payload_bytes()
        .to_vec();
    let post_llvm = bind_gfx942_scalar_f32_post_llvm_stage_contents_v1(
        lowering,
        post_llvm_contents_with_final(&llvm, &final_hsaco),
    )
    .unwrap();

    let readiness =
        bind_gfx942_scalar_f32_post_llvm_and_authenticated_machine_v1(post_llvm, machine).unwrap();
    assert!(readiness.binds_exact_final_code_object_to_authenticated_machine_analysis());
    assert!(readiness.validates_admitted_scalar_operation_def_use_correspondence());
    let operational = readiness.operational_translation();
    assert_eq!(operational.structured_operations(), 45);
    assert!(operational.structured_backedges() > 0);
    assert!(operational.validates_admitted_structured_operational_shapes());
    assert!(!operational.establishes_complete_operational_translation());
    assert!(!operational.proves_semantic_equivalence());
    assert!(operational.unsupported().iter().any(|unsupported| matches!(
        unsupported,
        Gfx942OperationalTranslationUnsupportedV1::MachineIeeeBinary32Semantics { .. }
    )));
    assert!(operational.unsupported().iter().any(|unsupported| matches!(
        unsupported,
        Gfx942OperationalTranslationUnsupportedV1::LoopCorrespondence { .. }
    )));
    assert!(!readiness.establishes_llvm_instruction_selection());
    assert!(!readiness.establishes_gfx942_operational_semantics());
    assert!(!readiness.grants_authority());
    assert!(
        readiness
            .missing_inputs()
            .iter()
            .any(|input| input.diagnostic_name() == "llvm-to-gfx942-instruction-selection")
    );

    let incomplete =
        require_gfx942_scalar_f32_machine_refinement_after_scalar_correspondence_v1(readiness)
            .unwrap_err();
    assert!(
        incomplete
            .to_string()
            .contains("llvm-optimization-semantic-preservation")
    );
    let readiness = incomplete.into_readiness();
    let (_, machine, operational, evidence) = readiness.into_parts();
    assert!(!evidence.is_empty());
    assert!(!operational.unsupported().is_empty());
    assert!(machine.authenticates_analyzer_execution());
}

#[test]
fn final_hsaco_one_axis_substitution_fails_and_retains_both_owners() {
    let (canonical, llvm, derivation) = lower_scalar_gemm();
    let lowering = replay_derivation(canonical, llvm.clone(), derivation).unwrap();
    let machine = positive();
    let mut substituted = machine
        .authenticated_execution()
        .request()
        .exact_payload_bytes()
        .to_vec();
    substituted[1] ^= 1;
    let post_llvm = bind_gfx942_scalar_f32_post_llvm_stage_contents_v1(
        lowering,
        post_llvm_contents_with_final(&llvm, &substituted),
    )
    .unwrap();

    let failure = bind_gfx942_scalar_f32_post_llvm_and_authenticated_machine_v1(post_llvm, machine)
        .unwrap_err();
    assert_eq!(
        failure.error().to_string(),
        "gfx942 post-LLVM/machine scalar correspondence rejected: FinalCodeObjectMismatch"
    );
    let (post_llvm, machine, _) = failure.into_parts();
    assert_eq!(post_llvm.stages().final_code_object(), substituted);
    assert!(machine.authenticates_analyzer_execution());
}
