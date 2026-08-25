use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, IndexBinaryKindAttr, MemorySpaceAttr,
    OwnershipCoverageAttr, OwnershipPartitionAttr, SemanticBinaryKindAttr, TensorConvergenceAttr,
};
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2,
    FunctionalRefinementSubjectsV2, ImportedFunctionalRefinementProofV2, SafeReferenceKindV2,
    UnsignedFunctionalRefinementReceiptV2, VerusToolchainIdentityV2,
};
use fe2o3_kernel_analysis::{
    KernelCheckPassKindV1, KernelCheckStatusV1, PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
};
use fe2o3_kernel_ir::{TensorInstructionProfileV1, TensorLayoutContractV1, TensorSymbolicMapV1};
use fe2o3_pliron::{
    DialectRegistration, HARD_MAX_SESSION_OPERATION_TREE_ITEMS, ProductionConstructionV1,
    ProductionEffectRefinementContractV2, ProductionFunctionalRefinementAdmissionErrorV2,
    ProductionFunctionalRefinementTrustPolicyV2, ProductionGpuWriteSiteV2,
    ProductionNumericalContractV2, ProductionOverflowContractV2, ProductionPlironSessionV1,
    ProductionRankedBlockV1, ProductionRankedCompileErrorV1, ProductionRankedCompileErrorV2,
    ProductionRankedKernelErrorV1, ProductionRankedKernelV1, ProductionRankedOperationV1,
    ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    ProductionReferenceOutputSiteV2, ProductionReferenceProofV1, ProductionReferenceProofV2,
    ProductionSemanticBinaryOpV2, ProductionSemanticExpressionErrorV2,
    ProductionSemanticExpressionV2, ProductionSemanticScalarTypeV2, ProductionSessionErrorV1,
    ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
    compile_ranked_kernel_for_lowering_v2, normalized_effect_refinement_hash_for_kernel_v2,
    normalized_functional_refinement_formula_hash_for_kernel_v2,
};
use fe2o3_proof_contracts::DigestV1;

const VIEW: ProductionRankedValueIdV1 = ProductionRankedValueIdV1::new(0);
const INDEX: ProductionRankedValueIdV1 = ProductionRankedValueIdV1::new(1);

fn local(identity: ProductionRankedValueIdV1) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(identity)
}

fn static_kernel(index: u64, extent: u64) -> ProductionRankedKernelV1 {
    ProductionRankedKernelV1::new(
        "static_copy",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::View {
                    result: VIEW,
                    element_width: 32,
                    writable: false,
                    shape: vec![extent],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: INDEX,
                    value: index,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: local(VIEW),
                    indices: vec![local(INDEX)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("valid static recipe")
}

fn dynamic_kernel(guarded: bool) -> ProductionRankedKernelV1 {
    let view = ProductionRankedOperationV1::View {
        result: VIEW,
        element_width: 16,
        writable: false,
        shape: vec![0],
        dynamic_extents: vec![ProductionRankedValueV1::Argument(1)],
        allocation_origin: 1,
        noalias_class: 1,
    };
    let access = ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Read,
        view: local(VIEW),
        indices: vec![ProductionRankedValueV1::Argument(0)],
    };
    let blocks = if guarded {
        vec![
            ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [1, 1, 1],
                        workgroup_extents: [1, 1, 1],
                        subgroup_size: 1,
                        full_physical_workgroups: true,
                    },
                    view,
                ],
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs: ProductionRankedValueV1::Argument(0),
                    rhs: ProductionRankedValueV1::Argument(1),
                    true_block: 1,
                    false_block: 2,
                },
            ),
            ProductionRankedBlockV1::new(vec![access], ProductionRankedTerminatorV1::Return),
            ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
        ]
    } else {
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                view,
                access,
            ],
            ProductionRankedTerminatorV1::Return,
        )]
    };
    ProductionRankedKernelV1::new("dynamic_copy", 2, blocks).expect("valid dynamic recipe")
}

fn construction(kernel: ProductionRankedKernelV1) -> ProductionConstructionV1 {
    ProductionConstructionV1::ranked_kernel("kernel_module", kernel).expect("valid construction")
}

fn tensor_kernel(contract: TensorLayoutContractV1) -> ProductionRankedKernelV1 {
    ProductionRankedKernelV1::new(
        "tensor_contract",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [64, 1, 1],
                    workgroup_extents: [64, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::TensorLayout {
                    contract,
                    convergence: TensorConvergenceAttr::UniformSubgroup,
                    active_lanes: 64,
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("valid tensor recipe")
}

fn deterministic_tensor_kernel() -> ProductionRankedKernelV1 {
    let zero = ProductionRankedValueIdV1::new(0);
    let summary = ProductionRankedValueIdV1::new(1);
    ProductionRankedKernelV1::new(
        "deterministic_tensor_control",
        2,
        vec![
            ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [0, 1, 1],
                        workgroup_extents: [64, 1, 1],
                        subgroup_size: 64,
                        full_physical_workgroups: true,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: zero,
                        value: 0,
                    },
                    ProductionRankedOperationV1::DeterministicJoin {
                        result: summary,
                        dependencies: vec![
                            ProductionRankedValueV1::Argument(0),
                            ProductionRankedValueV1::Argument(1),
                        ],
                    },
                ],
                ProductionRankedTerminatorV1::IndexEqual {
                    lhs: local(summary),
                    rhs: local(zero),
                    true_block: 1,
                    false_block: 2,
                },
            ),
            ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::TensorLayout {
                    contract: TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
                    convergence: TensorConvergenceAttr::UniformSubgroup,
                    active_lanes: 64,
                }],
                ProductionRankedTerminatorV1::Branch { target: 2 },
            ),
            ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
        ],
    )
    .expect("valid deterministic control recipe")
}

fn session(with_kernel_dialect: bool) -> ProductionPlironSessionV1 {
    let registrations = if with_kernel_dialect {
        vec![
            dialect_kernel::dialect_registration().expect("kernel registration"),
            dialect_gpu::dialect_registration().expect("gpu registration"),
        ]
    } else {
        Vec::<DialectRegistration>::new()
    };
    ProductionPlironSessionV1::new(ProductionSessionLimitsV1::default(), registrations)
        .expect("production session")
}

#[test]
fn static_non_gemm_kernel_reaches_safety_verified_lowering_input() {
    let input = compile_ranked_kernel_for_lowering_v1(
        construction(static_kernel(7, 64)),
        ProductionSessionLimitsV1::default(),
    )
    .expect("safe static kernel");

    assert_eq!(input.kernel().function_name(), "static_copy");
    assert_eq!(
        input.production_pipeline_report().pass_order(),
        &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
    );
    assert_eq!(
        input.production_pipeline_report().pass_order()[0],
        KernelCheckPassKindV1::TensorLayout
    );
    assert_eq!(
        input.production_pipeline_report().status(),
        KernelCheckStatusV1::Clean
    );
    assert!(
        !input
            .production_pipeline_report()
            .grants_compiler_refinement_authority()
    );
    assert!(input.bounds_report().is_clean());
    assert!(input.tensor_layout_report().is_clean());
    assert!(input.atomic_report().is_clean());
    assert!(input.race_report().is_clean());
    assert!(input.ownership_report().is_clean());
    assert!(!input.race_report().grants_compiler_refinement_authority());
    assert!(input.barrier_report().is_clean());
    assert!(input.workgroup_report().is_clean());
    assert!(input.semantic_report().is_clean());
    assert!(input.all_mandatory_reports_are_clean());
    assert!(!input.bounds_report().grants_compiler_refinement_authority());
    assert!(!input.grants_artifact_or_launch_authority());
}

#[test]
fn declared_multi_invocation_domain_checks_constant_write_without_index_use() {
    let kernel = ProductionRankedKernelV1::new(
        "constant_multi_invocation_write",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 2,
                    global_extents: [64, 1, 1],
                    workgroup_extents: [64, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::View {
                    result: VIEW,
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 2,
                    noalias_class: 2,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: INDEX,
                    value: 0,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: local(VIEW),
                    indices: vec![local(INDEX)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("valid constant-write recipe");

    let error = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedRace(_))
    ));
    assert!(error.to_string().contains("error[FE2O3-RACE-001]"));
}

#[test]
fn recipe_rejects_noalias_class_without_authenticated_origin() {
    let error = ProductionRankedKernelV1::new(
        "forged_noalias",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::View {
                result: VIEW,
                element_width: 32,
                writable: true,
                shape: vec![1],
                dynamic_extents: vec![],
                allocation_origin: 0,
                noalias_class: 9,
            }],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap_err();
    assert_eq!(
        error,
        ProductionRankedKernelErrorV1::InvalidAllocationContract
    );
}

#[test]
fn production_allocation_read_effect_reaches_the_full_pipeline() {
    let kernel = ProductionRankedKernelV1::new(
        "allocation_read",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 91,
                    global_extents: [64, 1, 1],
                    workgroup_extents: [64, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::AllocationEffect {
                    kind: AccessKindAttr::Read,
                    memory_space: MemorySpaceAttr::Global,
                    allocation_origin: 91,
                    noalias_class: 92,
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("supported allocation read effect");
    let _ = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .expect("allocation read effect reaches production lowering");
}

#[test]
fn production_allocation_effect_rejects_unchecked_memory_semantics() {
    for (kind, memory_space) in [
        (AccessKindAttr::Write, MemorySpaceAttr::Global),
        (AccessKindAttr::Read, MemorySpaceAttr::Workgroup),
        (AccessKindAttr::Read, MemorySpaceAttr::Private),
    ] {
        let error = ProductionRankedKernelV1::new(
            "unsupported_allocation_effect",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::AllocationEffect {
                    kind,
                    memory_space,
                    allocation_origin: 91,
                    noalias_class: 92,
                }],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap_err();
        assert_eq!(
            error,
            ProductionRankedKernelErrorV1::InvalidAllocationContract
        );
    }
}

#[test]
fn direct_and_independently_transformed_tensor_layouts_reach_production_lowering() {
    for contract in [
        TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
        TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64_lds_xor4(),
        TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64().with_b_lds_xor4(),
        TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
            .with_zero_filled_predicate_inputs(),
    ] {
        let input = compile_ranked_kernel_for_lowering_v1(
            construction(tensor_kernel(contract)),
            ProductionSessionLimitsV1::default(),
        )
        .expect("verified tensor contract");
        assert!(input.tensor_layout_report().is_clean());
        assert!(
            !input
                .tensor_layout_report()
                .grants_artifact_or_launch_authority()
        );
    }
}

#[test]
fn deterministic_control_summary_reaches_all_mandatory_passes_without_authority() {
    let input = compile_ranked_kernel_for_lowering_v1(
        construction(deterministic_tensor_kernel()),
        ProductionSessionLimitsV1::default(),
    )
    .expect("uniform deterministic control is safe for subgroup tensor execution");

    assert!(input.all_mandatory_reports_are_clean());
    assert!(input.tensor_layout_report().is_clean());
    assert!(input.bounds_report().is_clean());
    assert!(!input.grants_artifact_or_launch_authority());
    assert!(
        !input
            .tensor_layout_report()
            .grants_compiler_refinement_authority()
    );
}

#[test]
fn deterministic_control_recipe_rejects_empty_dependencies_and_wrong_edge_arity() {
    let empty = ProductionRankedKernelV1::new(
        "empty_deterministic_join",
        1,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::DeterministicJoin {
                result: ProductionRankedValueIdV1::new(0),
                dependencies: vec![],
            }],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap_err();
    assert_eq!(
        empty,
        ProductionRankedKernelErrorV1::ResourceLimit {
            resource: "deterministic dependency",
            limit: dialect_kernel::MAX_DETERMINISTIC_JOIN_INPUTS_V1,
            actual: 0,
        }
    );

    let wrong_edge = ProductionRankedKernelV1::new(
        "wrong_equality_edge_arity",
        2,
        vec![
            ProductionRankedBlockV1::new(
                vec![],
                ProductionRankedTerminatorV1::IndexEqualArgs {
                    lhs: ProductionRankedValueV1::Argument(0),
                    rhs: ProductionRankedValueV1::Argument(1),
                    true_arguments: vec![],
                    false_arguments: vec![],
                    true_block: 1,
                    false_block: 2,
                },
            ),
            ProductionRankedBlockV1::with_index_arguments(
                1,
                vec![],
                ProductionRankedTerminatorV1::Return,
            ),
            ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
        ],
    )
    .unwrap_err();
    assert_eq!(
        wrong_edge,
        ProductionRankedKernelErrorV1::Materialization(
            "ranked conditional branch arguments do not match successors"
        )
    );
}

#[test]
fn production_tensor_layout_rejects_wrong_mapping_and_fails_closed_on_opaque_forms() {
    let mut wrong = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
    wrong.accumulator.mapping = wrong.a.mapping;
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(tensor_kernel(wrong)),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedTensorLayout(_))
    ));
    assert!(error.to_string().contains("FE2O3-TENSOR-LAYOUT-001"));
    assert!(
        error
            .to_string()
            .contains("Accumulator lane/component mapping")
    );

    let mut opaque = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
    opaque.profile = TensorInstructionProfileV1::Opaque(u32::MAX);
    opaque.a.mapping = TensorSymbolicMapV1::Opaque(u32::MAX);
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(tensor_kernel(opaque)),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedTensorLayout(_))
    ));
    assert!(error.to_string().contains("FE2O3-TENSOR-LAYOUT-002"));
}

#[test]
fn production_atomic_recipe_retains_its_contract_and_fails_closed_without_target_context() {
    let legacy = ProductionRankedKernelV1::new(
        "legacy_atomic",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ViewInSpace {
                    result: VIEW,
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                    memory_space: MemorySpaceAttr::Global,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: INDEX,
                    value: 0,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::AtomicWrite,
                    view: local(VIEW),
                    indices: vec![local(INDEX)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    );
    assert_eq!(
        legacy,
        Err(ProductionRankedKernelErrorV1::AtomicContractRequired)
    );

    let kernel = ProductionRankedKernelV1::new(
        "explicit_atomic",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ViewInSpace {
                    result: VIEW,
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                    memory_space: MemorySpaceAttr::Global,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: INDEX,
                    value: 0,
                },
                ProductionRankedOperationV1::AtomicAccess {
                    kind: AccessKindAttr::AtomicWrite,
                    ordering: AtomicOrderingAttr::Release,
                    scope: AtomicScopeAttr::Device,
                    view: local(VIEW),
                    indices: vec![local(INDEX)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("explicit atomic contract is a valid production recipe");
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedAtomic(_))
    ));
    assert!(error.to_string().contains("error[FE2O3-ATOMIC-002]"));
    assert!(error.to_string().contains("no bound target capability"));
}

#[test]
fn declared_expression_mismatch_is_terminal_in_production() {
    let alpha = ProductionRankedValueIdV1::new(0);
    let accumulator = ProductionRankedValueIdV1::new(1);
    let beta = ProductionRankedValueIdV1::new(2);
    let initial = ProductionRankedValueIdV1::new(3);
    let alpha_acc = ProductionRankedValueIdV1::new(4);
    let beta_initial = ProductionRankedValueIdV1::new(5);
    let actual = ProductionRankedValueIdV1::new(6);
    let expected = ProductionRankedValueIdV1::new(7);
    let symbol = |result, symbol| ProductionRankedOperationV1::SemanticSymbol { result, symbol };
    let binary = |result, kind, lhs, rhs| ProductionRankedOperationV1::SemanticBinary {
        result,
        kind,
        lhs: local(lhs),
        rhs: local(rhs),
    };
    let kernel = ProductionRankedKernelV1::new(
        "declared_expression_mismatch",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                symbol(alpha, 0),
                symbol(accumulator, 1),
                symbol(beta, 2),
                symbol(initial, 3),
                binary(
                    alpha_acc,
                    SemanticBinaryKindAttr::Multiply,
                    alpha,
                    accumulator,
                ),
                binary(
                    beta_initial,
                    SemanticBinaryKindAttr::Multiply,
                    beta,
                    initial,
                ),
                binary(actual, SemanticBinaryKindAttr::Add, alpha_acc, initial),
                binary(
                    expected,
                    SemanticBinaryKindAttr::Add,
                    alpha_acc,
                    beta_initial,
                ),
                ProductionRankedOperationV1::RequireEquivalent {
                    actual: local(actual),
                    expected: local(expected),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedSemantic(_))
    ));
    assert!(error.to_string().contains("error[FE2O3-SEMANTIC-001]"));
}

fn reference_proof() -> ProductionReferenceProofV1 {
    ProductionReferenceProofV1::declare_exact(
        [1, 2, 3, 4],
        [5, 6, 7, 8],
        [9, 10, 11, 12],
        [13, 14, 15, 16],
    )
    .expect("exact nonzero proof identities")
}

fn proof_digest(value: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([value; 32])
}

fn functional_subjects(kernel_mir: u8) -> FunctionalRefinementSubjectsV2 {
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        proof_digest(1),
        DigestV1::ZERO,
        proof_digest(2),
        proof_digest(3),
        proof_digest(kernel_mir),
    )
    .unwrap()
}

fn functional_binding(kernel_mir: u8, obligation: DigestV1) -> FunctionalRefinementBindingV2 {
    FunctionalRefinementBindingV2::from_subjects(functional_subjects(kernel_mir), obligation)
        .unwrap()
}

fn imported_reference(
    binding: FunctionalRefinementBindingV2,
    boundary: FunctionalRefinementBoundaryV2,
) -> (
    ProductionReferenceProofV2,
    ImportedFunctionalRefinementProofV2,
    ProductionFunctionalRefinementTrustPolicyV2,
) {
    let signing = SigningKey::from_bytes(&[91; 32]);
    let toolchain = VerusToolchainIdentityV2::new(
        proof_digest(10),
        proof_digest(11),
        proof_digest(12),
        proof_digest(13),
        proof_digest(14),
    )
    .unwrap();
    let policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        boundary,
    )
    .unwrap();
    let signer_identity = policy.signer_identity();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        proof_digest(20),
        FunctionalRefinementResultV2::Proved,
        boundary,
    )
    .unwrap();
    let wire = unsigned
        .clone()
        .attach_signature(signing.sign(unsigned.signing_bytes()).to_bytes());
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
    let imported = importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    let production_policy =
        ProductionFunctionalRefinementTrustPolicyV2::new([signer_identity], toolchain).unwrap();
    (
        ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding),
        imported,
        production_policy,
    )
}

#[test]
fn legacy_declared_proved_reference_is_not_authoritative() {
    let lhs = ProductionRankedValueIdV1::new(0);
    let rhs = ProductionRankedValueIdV1::new(1);
    let actual = ProductionRankedValueIdV1::new(2);
    let expected = ProductionRankedValueIdV1::new(3);
    let kernel = ProductionRankedKernelV1::new(
        "proved_reference",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::SemanticSymbol {
                    result: lhs,
                    symbol: 0,
                },
                ProductionRankedOperationV1::SemanticSymbol {
                    result: rhs,
                    symbol: 1,
                },
                ProductionRankedOperationV1::SemanticBinary {
                    result: actual,
                    kind: SemanticBinaryKindAttr::Add,
                    lhs: local(lhs),
                    rhs: local(rhs),
                },
                ProductionRankedOperationV1::SemanticBinary {
                    result: expected,
                    kind: SemanticBinaryKindAttr::Add,
                    lhs: local(rhs),
                    rhs: local(lhs),
                },
                ProductionRankedOperationV1::RequireReferenceEquivalent {
                    actual: local(actual),
                    expected: local(expected),
                    proof: reference_proof(),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("valid workload-neutral reference recipe");

    let error = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedSemantic(_))
    ));
    assert!(error.to_string().contains("requires exact Proved evidence"));
}

#[test]
fn authenticated_mir_reference_reaches_the_production_pipeline() {
    let lhs = ProductionRankedValueIdV1::new(0);
    let rhs = ProductionRankedValueIdV1::new(1);
    let actual = ProductionRankedValueIdV1::new(2);
    let expected = ProductionRankedValueIdV1::new(3);
    let kernel = |terminator| {
        ProductionRankedKernelV1::new(
            "authenticated_reference",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::SemanticSymbol {
                        result: lhs,
                        symbol: 0,
                    },
                    ProductionRankedOperationV1::SemanticSymbol {
                        result: rhs,
                        symbol: 1,
                    },
                    ProductionRankedOperationV1::SemanticBinary {
                        result: actual,
                        kind: SemanticBinaryKindAttr::Add,
                        lhs: local(lhs),
                        rhs: local(rhs),
                    },
                    ProductionRankedOperationV1::SemanticBinary {
                        result: expected,
                        kind: SemanticBinaryKindAttr::Add,
                        lhs: local(rhs),
                        rhs: local(lhs),
                    },
                    ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                        actual: local(actual),
                        expected: local(expected),
                        subjects: functional_subjects(4),
                    },
                ],
                terminator,
            )],
        )
        .unwrap()
    };
    let placeholder_kernel = kernel(ProductionRankedTerminatorV1::Return);
    let obligation = normalized_functional_refinement_formula_hash_for_kernel_v2(
        &placeholder_kernel,
        0,
        4,
        local(actual),
        local(expected),
        functional_subjects(4),
    )
    .unwrap();
    let changed_terminator = kernel(ProductionRankedTerminatorV1::Trap);
    assert_ne!(
        obligation,
        normalized_functional_refinement_formula_hash_for_kernel_v2(
            &changed_terminator,
            0,
            4,
            local(actual),
            local(expected),
            functional_subjects(4),
        )
        .unwrap()
    );
    let (_, unbound_imported, unbound_policy) = imported_reference(
        functional_binding(4, obligation),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    );
    let unbound_error = compile_ranked_kernel_for_lowering_v2(
        construction(kernel(ProductionRankedTerminatorV1::Return)),
        ProductionSessionLimitsV1::default(),
        vec![unbound_imported],
        unbound_policy,
    )
    .unwrap_err();
    assert!(matches!(
        unbound_error,
        ProductionRankedCompileErrorV2::Proof(
            ProductionFunctionalRefinementAdmissionErrorV2::UnboundRequest
        )
    ));
    let (proof, imported, policy) = imported_reference(
        functional_binding(4, obligation),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    );
    let bound = placeholder_kernel
        .bind_functional_refinement_request_v2(0, 4, proof)
        .unwrap();
    let input = compile_ranked_kernel_for_lowering_v2(
        construction(bound),
        ProductionSessionLimitsV1::default(),
        vec![imported],
        policy,
    )
    .unwrap();
    assert_eq!(input.semantic_report().reference_obligation_count(), 1);
    assert_eq!(
        input.semantic_report().proved_reference_obligation_count(),
        1
    );
    assert!(
        input
            .semantic_report()
            .all_reference_obligations_are_proved()
    );
    assert!(input.has_retained_functional_refinement_receipts());
    assert!(
        input.retained_functional_refinement_receipts()[0].is_retained_policy_verified_receipt()
    );
    assert!(!input.grants_compiler_refinement_authority());
    assert!(!input.retained_functional_refinement_receipts()[0].grants_source_to_isa_authority());
    assert!(
        !input.retained_functional_refinement_receipts()[0].grants_artifact_or_launch_authority()
    );
}

#[test]
fn typed_semantic_commitments_reach_all_mandatory_v2_passes() {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    let expression = ProductionSemanticExpressionV2::Binary {
        operation: ProductionSemanticBinaryOpV2::Add,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(ProductionSemanticExpressionV2::Symbol { symbol: 9, scalar }),
        rhs: Box::new(ProductionSemanticExpressionV2::Constant { scalar, bits: 4 }),
    };
    let actual = ProductionRankedValueIdV1::new(0);
    let expected = ProductionRankedValueIdV1::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "typed_authenticated_reference",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::SemanticExpression {
                    result: actual,
                    expression: expression.clone(),
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::SemanticExpression {
                    result: expected,
                    expression,
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                    actual: local(actual),
                    expected: local(expected),
                    subjects: functional_subjects(4),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let obligation = normalized_functional_refinement_formula_hash_for_kernel_v2(
        &kernel,
        0,
        2,
        local(actual),
        local(expected),
        functional_subjects(4),
    )
    .unwrap();
    let (proof, imported, policy) = imported_reference(
        functional_binding(4, obligation),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    );
    let bound = kernel
        .bind_functional_refinement_request_v2(0, 2, proof)
        .unwrap();
    let input = compile_ranked_kernel_for_lowering_v2(
        construction(bound),
        ProductionSessionLimitsV1::default(),
        vec![imported],
        policy,
    )
    .unwrap();
    assert!(input.all_mandatory_reports_are_clean());
    assert_eq!(
        input.semantic_report().proved_reference_obligation_count(),
        1
    );
    let summary = fe2o3_pliron::typed_semantic_obligation_summary_v2(input.kernel()).unwrap();
    assert!(summary.is_non_vacuous());
    assert_eq!(summary.expression_roots, 2);
    assert_eq!(summary.exact_bitvector_operator_congruence_roots, 2);
}

#[test]
fn undefined_typed_expressions_are_rejected_before_external_receipt_import() {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    let symbol = || ProductionSemanticExpressionV2::Symbol { symbol: 9, scalar };
    let constant = |bits| ProductionSemanticExpressionV2::Constant { scalar, bits };
    let (proof, _imported, _policy) = imported_reference(
        functional_binding(4, proof_digest(44)),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    );
    for expression in [
        ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar,
            overflow: ProductionOverflowContractV2::Checked,
            lhs: Box::new(symbol()),
            rhs: Box::new(constant(1)),
        },
        ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Divide,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(symbol()),
            rhs: Box::new(symbol()),
        },
    ] {
        let actual = ProductionRankedValueIdV1::new(0);
        let expected = ProductionRankedValueIdV1::new(1);
        let error = ProductionRankedKernelV1::new(
            "external_receipt_cannot_bypass_definedness",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::SemanticExpression {
                        result: actual,
                        expression,
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::SemanticExpression {
                        result: expected,
                        expression: constant(0),
                        numerical_contract:
                            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                    },
                    ProductionRankedOperationV1::RequireAuthenticatedReferenceEquivalent {
                        actual: local(actual),
                        expected: local(expected),
                        proof,
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap_err();
        assert_eq!(
            error,
            ProductionRankedKernelErrorV1::InvalidSemanticExpression(
                ProductionSemanticExpressionErrorV2::IncompleteDomain,
            ),
        );
    }
}

#[test]
fn functional_refinement_transcript_binds_control_dependencies() {
    let actual = ProductionRankedValueIdV1::new(0);
    let expected = ProductionRankedValueIdV1::new(1);
    let kernel = |dependency| {
        ProductionRankedKernelV1::new(
            "control_dependency_binding",
            2,
            vec![
                ProductionRankedBlockV1::new(
                    vec![
                        ProductionRankedOperationV1::SemanticSymbol {
                            result: actual,
                            symbol: 0,
                        },
                        ProductionRankedOperationV1::SemanticSymbol {
                            result: expected,
                            symbol: 0,
                        },
                        ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
                            actual: local(actual),
                            expected: local(expected),
                            subjects: functional_subjects(4),
                        },
                    ],
                    ProductionRankedTerminatorV1::AnalysisSplit {
                        control_dependencies: vec![ProductionRankedValueV1::Argument(dependency)],
                        first_block: 1,
                        second_block: 2,
                    },
                ),
                ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
                ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
            ],
        )
        .unwrap()
    };
    let first = kernel(0);
    let second = kernel(1);
    let digest = |kernel: &ProductionRankedKernelV1| {
        normalized_functional_refinement_formula_hash_for_kernel_v2(
            kernel,
            0,
            2,
            local(actual),
            local(expected),
            functional_subjects(4),
        )
        .unwrap()
    };
    assert_ne!(digest(&first), digest(&second));
}

#[test]
fn authenticated_reference_admission_rejects_missing_wrong_boundary_and_stale_binding() {
    fn kernel(proof: ProductionReferenceProofV2) -> ProductionRankedKernelV1 {
        let value = ProductionRankedValueIdV1::new(0);
        ProductionRankedKernelV1::new(
            "reference_admission_negative",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::SemanticSymbol {
                        result: value,
                        symbol: 0,
                    },
                    ProductionRankedOperationV1::RequireAuthenticatedReferenceEquivalent {
                        actual: local(value),
                        expected: local(value),
                        proof,
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap()
    }

    let (request, _, policy) = imported_reference(
        functional_binding(4, proof_digest(5)),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    );
    let error = compile_ranked_kernel_for_lowering_v2(
        construction(kernel(request)),
        ProductionSessionLimitsV1::default(),
        vec![],
        policy,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV2::Proof(
            ProductionFunctionalRefinementAdmissionErrorV2::MissingImportedReceipt(_)
        )
    ));

    let (request, imported, policy) = imported_reference(
        functional_binding(4, proof_digest(5)),
        FunctionalRefinementBoundaryV2::SafeReferenceSourceToKernelMir,
    );
    let error = compile_ranked_kernel_for_lowering_v2(
        construction(kernel(request)),
        ProductionSessionLimitsV1::default(),
        vec![imported],
        policy,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV2::Proof(
            ProductionFunctionalRefinementAdmissionErrorV2::WrongBoundary(_)
        )
    ));

    let (request, _, _) = imported_reference(
        functional_binding(4, proof_digest(5)),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    );
    let (_, stale, policy) = imported_reference(
        functional_binding(6, proof_digest(5)),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    );
    let error = compile_ranked_kernel_for_lowering_v2(
        construction(kernel(ProductionReferenceProofV2::request_exact(
            stale.receipt_identity(),
            request.binding(),
        ))),
        ProductionSessionLimitsV1::default(),
        vec![stale],
        policy,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV2::Proof(
            ProductionFunctionalRefinementAdmissionErrorV2::BindingMismatch(_)
        )
    ));
}

#[test]
fn authenticated_effect_refinement_reaches_the_same_production_pipeline() {
    let view = ProductionRankedValueIdV1::new(0);
    let index = ProductionRankedValueIdV1::new(1);
    let formula = ProductionRankedValueIdV1::new(2);
    let alternate_formula = ProductionRankedValueIdV1::new(3);
    let kernel = |allocation_origin,
                  grid_identity,
                  reference_statement,
                  alternate_coordinate,
                  alternate_rhs| {
        let contract = ProductionEffectRefinementContractV2::new(
            91,
            ProductionGpuWriteSiteV2::new(0, 5),
            ProductionReferenceOutputSiteV2::new(2, 3, reference_statement),
            local(view),
            vec![local(index)],
            vec![local(if alternate_coordinate {
                alternate_formula
            } else {
                formula
            })],
            vec![local(formula)],
            local(formula),
            local(formula),
            local(formula),
            local(formula),
            local(if alternate_rhs {
                alternate_formula
            } else {
                formula
            }),
            local(formula),
        )
        .unwrap();
        ProductionRankedKernelV1::new(
            "authenticated_effect",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity,
                        global_extents: [1, 1, 1],
                        workgroup_extents: [1, 1, 1],
                        subgroup_size: 1,
                        full_physical_workgroups: true,
                    },
                    ProductionRankedOperationV1::View {
                        result: view,
                        element_width: 32,
                        writable: true,
                        shape: vec![1],
                        dynamic_extents: vec![],
                        allocation_origin,
                        noalias_class: 1,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: index,
                        value: 0,
                    },
                    ProductionRankedOperationV1::SemanticConstant {
                        result: formula,
                        value: 1,
                    },
                    ProductionRankedOperationV1::SemanticConstant {
                        result: alternate_formula,
                        value: 2,
                    },
                    ProductionRankedOperationV1::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: local(view),
                        indices: vec![local(index)],
                        value: local(if alternate_rhs {
                            alternate_formula
                        } else {
                            formula
                        }),
                    },
                    ProductionRankedOperationV1::OwnershipContract {
                        view: local(view),
                        coverage: OwnershipCoverageAttr::ExactView,
                        partition: OwnershipPartitionAttr::ExactSets,
                    },
                    ProductionRankedOperationV1::RequestEffectRefinement {
                        contract,
                        subjects: functional_subjects(4),
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap()
    };
    let skeleton = kernel(1, 1, 4, false, false);
    let ProductionRankedOperationV1::RequestEffectRefinement { contract, .. } =
        &skeleton.blocks()[0].operations()[7]
    else {
        unreachable!()
    };
    let obligation = normalized_effect_refinement_hash_for_kernel_v2(
        &skeleton,
        0,
        7,
        contract,
        functional_subjects(4),
    )
    .unwrap();
    let mutated = kernel(2, 1, 4, false, false);
    let ProductionRankedOperationV1::RequestEffectRefinement {
        contract: mutated_contract,
        ..
    } = &mutated.blocks()[0].operations()[7]
    else {
        unreachable!()
    };
    assert_ne!(
        obligation,
        normalized_effect_refinement_hash_for_kernel_v2(
            &mutated,
            0,
            7,
            mutated_contract,
            functional_subjects(4),
        )
        .unwrap()
    );
    for changed in [
        kernel(1, 2, 4, false, false),
        kernel(1, 1, 5, false, false),
        kernel(1, 1, 4, true, false),
        kernel(1, 1, 4, false, true),
    ] {
        let ProductionRankedOperationV1::RequestEffectRefinement {
            contract: changed_contract,
            ..
        } = &changed.blocks()[0].operations()[7]
        else {
            unreachable!()
        };
        assert_ne!(
            obligation,
            normalized_effect_refinement_hash_for_kernel_v2(
                &changed,
                0,
                7,
                changed_contract,
                functional_subjects(4),
            )
            .unwrap()
        );
    }
    let (proof, imported, policy) = imported_reference(
        functional_binding(4, obligation),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    );
    let bound = skeleton
        .bind_functional_refinement_request_v2(0, 7, proof)
        .unwrap();
    let input = compile_ranked_kernel_for_lowering_v2(
        construction(bound),
        ProductionSessionLimitsV1::default(),
        vec![imported],
        policy,
    )
    .unwrap();
    assert!(input.semantic_report().effect_refinement().is_clean());
    assert_eq!(
        input.semantic_report().effect_refinement().contract_count(),
        1
    );
    assert_eq!(
        input
            .semantic_report()
            .effect_refinement()
            .proved_contract_count(),
        1
    );
}

#[test]
fn proved_reference_identity_cannot_hide_a_wrong_expression() {
    let actual = ProductionRankedValueIdV1::new(0);
    let expected = ProductionRankedValueIdV1::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "wrong_proved_reference",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::SemanticSymbol {
                    result: actual,
                    symbol: 0,
                },
                ProductionRankedOperationV1::SemanticSymbol {
                    result: expected,
                    symbol: 1,
                },
                ProductionRankedOperationV1::RequireReferenceEquivalent {
                    actual: local(actual),
                    expected: local(expected),
                    proof: reference_proof(),
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("structurally valid reference recipe");

    let error = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedSemantic(_))
    ));
    assert!(error.to_string().contains("error[FE2O3-SEMANTIC-001]"));
}

#[test]
fn malformed_reference_identities_are_rejected_before_pliron_allocation() {
    assert_eq!(
        ProductionReferenceProofV1::declare_exact(
            [0; 4],
            [5, 6, 7, 8],
            [9, 10, 11, 12],
            [13, 14, 15, 16],
        ),
        Err(ProductionRankedKernelErrorV1::InvalidReferenceContract)
    );
    assert_eq!(
        ProductionReferenceProofV1::declare_exact(
            [1, 2, 3, 4],
            [1, 2, 3, 4],
            [9, 10, 11, 12],
            [13, 14, 15, 16],
        ),
        Err(ProductionRankedKernelErrorV1::InvalidReferenceContract)
    );
}

#[test]
fn divergent_barrier_is_terminal_in_the_closed_production_pipeline() {
    let invocation = ProductionRankedValueIdV1::new(0);
    let two = ProductionRankedValueIdV1::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "divergent_barrier",
        0,
        vec![
            ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: 7,
                        global_extents: [4, 1, 1],
                        workgroup_extents: [4, 1, 1],
                        subgroup_size: 4,
                        full_physical_workgroups: true,
                    },
                    ProductionRankedOperationV1::InvocationIndex {
                        result: invocation,
                        dimension: 0,
                        launch_extent: 4,
                    },
                    ProductionRankedOperationV1::IndexConstant {
                        result: two,
                        value: 2,
                    },
                ],
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs: local(invocation),
                    rhs: local(two),
                    true_block: 1,
                    false_block: 2,
                },
            ),
            ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::Barrier {
                    execution_scope: HierarchyAttr::Workgroup,
                    memory_scope: MemoryScopeAttr::Workgroup,
                    address_space: AddressSpaceAttr::Workgroup,
                    order: MemoryOrderAttr::AcquireRelease,
                }],
                ProductionRankedTerminatorV1::Branch { target: 2 },
            ),
            ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
        ],
    )
    .unwrap();
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedBarrier(_))
    ));
    assert!(error.to_string().contains("error[FE2O3-BARRIER-001]"));
}

#[test]
fn uninitialized_workgroup_read_is_terminal_before_lowering() {
    let view = ProductionRankedValueIdV1::new(0);
    let invocation = ProductionRankedValueIdV1::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "uninitialized_workgroup_read",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 7,
                    global_extents: [8, 1, 1],
                    workgroup_extents: [8, 1, 1],
                    subgroup_size: 8,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::ViewInSpace {
                    result: view,
                    element_width: 32,
                    writable: true,
                    shape: vec![8],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                    memory_space: MemorySpaceAttr::Workgroup,
                },
                ProductionRankedOperationV1::InvocationIndex {
                    result: invocation,
                    dimension: 0,
                    launch_extent: 8,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: local(view),
                    indices: vec![local(invocation)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedWorkgroup(_))
    ));
    assert!(error.to_string().contains("error[FE2O3-WORKGROUP-001]"));
}

fn concurrent_write_kernel(indexed_by_invocation: bool) -> ProductionRankedKernelV1 {
    let invocation = ProductionRankedOperationV1::InvocationIndex {
        result: INDEX,
        dimension: 0,
        launch_extent: 64,
    };
    let address = ProductionRankedValueIdV1::new(2);
    let constant = (!indexed_by_invocation).then_some(ProductionRankedOperationV1::IndexConstant {
        result: address,
        value: 0,
    });
    let access_index = if indexed_by_invocation {
        INDEX
    } else {
        address
    };
    let mut operations = vec![
        ProductionRankedOperationV1::ViewInSpace {
            result: VIEW,
            element_width: 32,
            writable: true,
            shape: vec![64],
            dynamic_extents: vec![],
            allocation_origin: 1,
            noalias_class: 1,
            memory_space: MemorySpaceAttr::Global,
        },
        invocation,
    ];
    operations.extend(constant);
    operations.push(ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Write,
        view: local(VIEW),
        indices: vec![local(access_index)],
    });
    ProductionRankedKernelV1::new(
        "concurrent_write",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("valid concurrent recipe")
}

#[test]
fn invocation_owned_output_reaches_lowering_after_race_verification() {
    let input = compile_ranked_kernel_for_lowering_v1(
        construction(concurrent_write_kernel(true)),
        ProductionSessionLimitsV1::default(),
    )
    .expect("invocation-owned output is disjoint");
    assert!(input.bounds_report().is_clean());
    assert!(input.race_report().is_clean());
}

fn hierarchy_owned_output_kernel(has_holes: bool) -> ProductionRankedKernelV1 {
    let two = ProductionRankedValueIdV1::new(2);
    let address = ProductionRankedValueIdV1::new(3);
    let mut operations = vec![
        ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 9,
            global_extents: [64, 1, 1],
            workgroup_extents: [32, 1, 1],
            subgroup_size: 16,
            full_physical_workgroups: true,
        },
        ProductionRankedOperationV1::ViewInSpace {
            result: VIEW,
            element_width: 32,
            writable: true,
            shape: vec![if has_holes { 128 } else { 64 }],
            dynamic_extents: vec![],
            allocation_origin: 1,
            noalias_class: 1,
            memory_space: MemorySpaceAttr::Global,
        },
        ProductionRankedOperationV1::OwnershipContract {
            view: local(VIEW),
            coverage: OwnershipCoverageAttr::TotalView,
            partition: if has_holes {
                OwnershipPartitionAttr::ExactSets
            } else {
                OwnershipPartitionAttr::DenseRectangles
            },
        },
        ProductionRankedOperationV1::InvocationIndex {
            result: INDEX,
            dimension: 0,
            launch_extent: 64,
        },
    ];
    let access_index = if has_holes {
        operations.push(ProductionRankedOperationV1::IndexConstant {
            result: two,
            value: 2,
        });
        operations.push(ProductionRankedOperationV1::IndexBinary {
            result: address,
            kind: IndexBinaryKindAttr::Multiply,
            lhs: local(INDEX),
            rhs: local(two),
        });
        address
    } else {
        INDEX
    };
    operations.push(ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Write,
        view: local(VIEW),
        indices: vec![local(access_index)],
    });
    ProductionRankedKernelV1::new(
        "hierarchy_owned_output",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("valid hierarchy ownership recipe")
}

#[test]
fn production_pipeline_enforces_complete_gpu_hierarchy_ownership() {
    let input = compile_ranked_kernel_for_lowering_v1(
        construction(hierarchy_owned_output_kernel(false)),
        ProductionSessionLimitsV1::default(),
    )
    .expect("complete disjoint hierarchy ownership");
    assert!(input.ownership_report().is_clean());
    assert!(!input.ownership_report().regions().is_empty());
    assert!(
        input
            .ownership_report()
            .all_total_view_contracts_are_proved()
    );
    assert_eq!(
        input
            .ownership_report()
            .coverage_summary()
            .total_view_declared(),
        1
    );
}

#[test]
fn v2_lowering_input_retains_non_vacuous_total_output_coverage() {
    let (_, _, policy) = imported_reference(
        functional_binding(4, proof_digest(5)),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
    );
    let input = compile_ranked_kernel_for_lowering_v2(
        construction(hierarchy_owned_output_kernel(false)),
        ProductionSessionLimitsV1::default(),
        vec![],
        policy,
    )
    .expect("V2 mandatory pipeline retains total-output coverage");
    assert!(
        input
            .ownership_report()
            .all_total_view_contracts_are_proved()
    );
    assert_eq!(
        input
            .ownership_report()
            .coverage_summary()
            .total_view_proved(),
        1
    );
}

#[test]
fn hierarchy_coverage_hole_is_a_terminal_compile_time_diagnostic() {
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(hierarchy_owned_output_kernel(true)),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedOwnership(_))
    ));
    assert!(error.to_string().contains("error[FE2O3-OWN-006]"));
    assert!(error.to_string().contains("logical coordinate [1]"));
}

#[test]
fn duplicate_output_ownership_is_a_terminal_compile_time_diagnostic() {
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(concurrent_write_kernel(false)),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedRace(_))
    ));
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("error[FE2O3-RACE-001]"));
    assert!(diagnostic.contains("invocation [0]"));
    assert!(diagnostic.contains("invocation [1]"));
    assert!(
        diagnostic
            .contains("distinct concurrent invocations do not imply disjoint memory coordinates")
    );
}

#[test]
fn production_fence_does_not_authorize_cross_workgroup_plain_overlap() {
    let view = ProductionRankedValueIdV1::new(0);
    let invocation = ProductionRankedValueIdV1::new(1);
    let zero = ProductionRankedValueIdV1::new(2);
    let kernel = ProductionRankedKernelV1::new(
        "fence_is_not_grid_barrier",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 19,
                    global_extents: [128, 1, 1],
                    workgroup_extents: [64, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::ViewInSpace {
                    result: view,
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                    memory_space: MemorySpaceAttr::Global,
                },
                ProductionRankedOperationV1::InvocationIndex {
                    result: invocation,
                    dimension: 0,
                    launch_extent: 128,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: zero,
                    value: 0,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: local(view),
                    indices: vec![local(zero)],
                },
                ProductionRankedOperationV1::Fence {
                    memory_scope: MemoryScopeAttr::Device,
                    address_space: AddressSpaceAttr::Global,
                    order: MemoryOrderAttr::AcquireRelease,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: local(view),
                    indices: vec![local(zero)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedRace(_))
    ));
    assert!(error.to_string().contains("error[FE2O3-RACE-002]"));
    assert!(error.to_string().contains("fence alone"));
}

#[test]
fn execution_layout_is_unique_canonical_and_checked() {
    let contradictory_domain = ProductionRankedKernelV1::new(
        "contradictory_domain",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::ExecutionLayout {
                grid_identity: 1,
                global_extents: [65, 1, 1],
                workgroup_extents: [64, 1, 1],
                subgroup_size: 64,
                full_physical_workgroups: true,
            }],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap_err();
    assert!(matches!(
        contradictory_domain,
        ProductionRankedKernelErrorV1::InvalidExecutionLayout
    ));

    let invalid = ProductionRankedKernelV1::new(
        "invalid_layout",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::ExecutionLayout {
                grid_identity: 1,
                global_extents: [96, 1, 1],
                workgroup_extents: [96, 1, 1],
                subgroup_size: 64,
                full_physical_workgroups: true,
            }],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap_err();
    assert!(matches!(
        invalid,
        ProductionRankedKernelErrorV1::InvalidExecutionLayout
    ));

    let noncanonical = ProductionRankedKernelV1::new(
        "late_layout",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [64, 1, 1],
                    workgroup_extents: [64, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 2,
                    global_extents: [64, 1, 1],
                    workgroup_extents: [64, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap_err();
    assert!(matches!(
        noncanonical,
        ProductionRankedKernelErrorV1::InvalidExecutionLayout
    ));
}

#[test]
fn static_oob_is_a_terminal_compile_time_diagnostic_before_lowering() {
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(static_kernel(64, 64)),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();

    let ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedBounds(bounds)) =
        &error
    else {
        panic!("static out-of-bounds access must stop in ranked bounds");
    };
    assert_eq!(bounds.report().status(), KernelCheckStatusV1::Rejected);
    assert_eq!(
        error.to_string(),
        "error[FE2O3-BOUNDS-001]: statically out-of-bounds Read at block 0 op 3; access: v0 dimension 0; required: 64 < 64",
    );
}

#[test]
fn dynamic_access_requires_a_dominating_exact_bound() {
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(dynamic_kernel(false)),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    let ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedBounds(bounds)) =
        &error
    else {
        panic!("unproved dynamic access must stop in ranked bounds");
    };
    assert_eq!(bounds.report().status(), KernelCheckStatusV1::Incomplete);
    assert!(error.to_string().contains("error[FE2O3-BOUNDS-002]"));
    assert!(error.to_string().contains("dimension 0"));
    assert!(error.to_string().contains("unproven bound:"));

    let guarded = compile_ranked_kernel_for_lowering_v1(
        construction(dynamic_kernel(true)),
        ProductionSessionLimitsV1::default(),
    )
    .expect("dominating guard proves dynamic access");
    assert!(guarded.bounds_report().is_clean());
}

#[test]
fn arbitrary_control_flow_split_never_manufactures_a_bounds_fact() {
    let view = ProductionRankedOperationV1::View {
        result: VIEW,
        element_width: 32,
        writable: false,
        shape: vec![0],
        dynamic_extents: vec![ProductionRankedValueV1::Argument(1)],
        allocation_origin: 1,
        noalias_class: 1,
    };
    let access = ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Read,
        view: local(VIEW),
        indices: vec![ProductionRankedValueV1::Argument(0)],
    };
    let kernel = ProductionRankedKernelV1::new(
        "arbitrary_split",
        2,
        vec![
            ProductionRankedBlockV1::new(
                vec![view],
                ProductionRankedTerminatorV1::AnalysisSplit {
                    control_dependencies: vec![],
                    first_block: 1,
                    second_block: 2,
                },
            ),
            ProductionRankedBlockV1::new(vec![access], ProductionRankedTerminatorV1::Return),
            ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
        ],
    )
    .unwrap();
    let error = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedBounds(_))
    ));
    assert!(error.to_string().contains("error[FE2O3-BOUNDS-002]"));
}

#[test]
fn typed_analysis_split_materializes_and_recursively_verifies_exact_segments() {
    let kernel = ProductionRankedKernelV1::new(
        "typed_analysis_split",
        1,
        vec![
            ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [64, 1, 1],
                    workgroup_extents: [64, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                }],
                ProductionRankedTerminatorV1::AnalysisSplitArgs {
                    control_dependencies: vec![ProductionRankedValueV1::Argument(0)],
                    first_arguments: vec![ProductionRankedValueV1::Argument(0)],
                    second_arguments: vec![ProductionRankedValueV1::Argument(0)],
                    first_block: 1,
                    second_block: 2,
                },
            ),
            ProductionRankedBlockV1::with_index_arguments(
                1,
                vec![],
                ProductionRankedTerminatorV1::Return,
            ),
            ProductionRankedBlockV1::with_index_arguments(
                1,
                vec![],
                ProductionRankedTerminatorV1::Return,
            ),
        ],
    )
    .unwrap();

    let lowered = compile_ranked_kernel_for_lowering_v1(
        construction(kernel),
        ProductionSessionLimitsV1::default(),
    )
    .expect("typed split survives construction and recursive verification");
    assert!(lowered.all_mandatory_reports_are_clean());
}

#[test]
fn rank_two_static_shapes_are_checked_without_gemm_semantics() {
    let row = ProductionRankedValueIdV1::new(1);
    let column = ProductionRankedValueIdV1::new(2);
    let kernel = ProductionRankedKernelV1::new(
        "image_tile",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::View {
                    result: VIEW,
                    element_width: 8,
                    writable: true,
                    shape: vec![32, 64],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: row,
                    value: 31,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: column,
                    value: 63,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: local(VIEW),
                    indices: vec![local(row), local(column)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("rank-two recipe");
    assert!(
        compile_ranked_kernel_for_lowering_v1(
            construction(kernel),
            ProductionSessionLimitsV1::default(),
        )
        .is_ok()
    );
}

#[test]
fn ranked_construction_requires_the_typed_kernel_registration() {
    let mut session = session(false);
    let registered = session
        .register_construction(construction(static_kernel(0, 1)))
        .expect("register data recipe");
    assert!(matches!(
        session.construct_registered(registered),
        Err(ProductionSessionErrorV1::RankedRecipe(
            ProductionRankedKernelErrorV1::MissingKernelDialect
        ))
    ));
    assert!(session.is_poisoned());
}

#[test]
fn builtin_module_cannot_be_relabelled_as_kernel_checks_verified() {
    let mut session = session(false);
    let registered = session
        .register_construction(
            ProductionConstructionV1::builtin_module("empty").expect("module recipe"),
        )
        .expect("registration");
    let (stage, root) = session
        .construct_registered(registered)
        .expect("empty module");
    assert!(matches!(
        session.verify_production_ranked_kernel_pipeline(stage, root),
        Err(ProductionSessionErrorV1::WrongConstructionKind)
    ));
}

#[test]
fn same_session_stage_root_substitution_is_rejected_before_analysis() {
    let mut session = session(true);
    let first = session
        .register_construction(
            ProductionConstructionV1::ranked_kernel("first", static_kernel(0, 1)).unwrap(),
        )
        .unwrap();
    let second = session
        .register_construction(
            ProductionConstructionV1::ranked_kernel("second", static_kernel(0, 1)).unwrap(),
        )
        .unwrap();
    let (first_stage, _) = session.construct_registered(first).unwrap();
    let (_, second_root) = session.construct_registered(second).unwrap();
    assert!(matches!(
        session.verify_production_ranked_kernel_pipeline(first_stage, second_root),
        Err(ProductionSessionErrorV1::StageRootMismatch)
    ));
    assert!(!session.is_poisoned());
}

#[test]
fn recipe_rejects_undefined_duplicate_and_non_entry_values() {
    let undefined = ProductionRankedKernelV1::new(
        "undefined",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                view: local(VIEW),
                indices: vec![local(INDEX)],
            }],
            ProductionRankedTerminatorV1::Return,
        )],
    );
    assert!(matches!(
        undefined,
        Err(ProductionRankedKernelErrorV1::UndefinedValue(_))
    ));

    let duplicate = ProductionRankedKernelV1::new(
        "duplicate",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::IndexConstant {
                    result: VIEW,
                    value: 0,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: VIEW,
                    value: 1,
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    );
    assert_eq!(
        duplicate,
        Err(ProductionRankedKernelErrorV1::NonCanonicalValueId {
            expected: 1,
            actual: 0,
        })
    );

    let non_entry = ProductionRankedKernelV1::new(
        "non_entry",
        0,
        vec![
            ProductionRankedBlockV1::new(
                vec![],
                ProductionRankedTerminatorV1::Branch { target: 1 },
            ),
            ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::IndexConstant {
                    result: INDEX,
                    value: 0,
                }],
                ProductionRankedTerminatorV1::Return,
            ),
        ],
    );
    assert_eq!(
        non_entry,
        Err(ProductionRankedKernelErrorV1::NonEntryDefinition { block: 1 })
    );
}

#[test]
fn recipe_rejects_shape_rank_and_access_type_mismatches() {
    let bad_dynamic = ProductionRankedKernelV1::new(
        "bad_dynamic",
        1,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::View {
                result: VIEW,
                element_width: 32,
                writable: false,
                shape: vec![0, 4],
                dynamic_extents: vec![],
                allocation_origin: 1,
                noalias_class: 1,
            }],
            ProductionRankedTerminatorV1::Return,
        )],
    );
    assert_eq!(
        bad_dynamic,
        Err(ProductionRankedKernelErrorV1::DynamicExtentCountMismatch {
            expected: 1,
            actual: 0,
        })
    );

    let write_read_only = ProductionRankedKernelV1::new(
        "readonly",
        1,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::View {
                    result: VIEW,
                    element_width: 32,
                    writable: false,
                    shape: vec![4],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: local(VIEW),
                    indices: vec![ProductionRankedValueV1::Argument(0)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    );
    assert_eq!(
        write_read_only,
        Err(ProductionRankedKernelErrorV1::WriteThroughReadOnlyView)
    );
}

#[test]
fn dense_value_and_exact_tree_work_bounds_reject_before_materialization() {
    let maximum_constants = (HARD_MAX_SESSION_OPERATION_TREE_ITEMS - 9) / 2;
    let build = |count: usize| {
        let operations = (0..count)
            .map(|identity| ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(identity as u32),
                value: identity as u64,
            })
            .collect();
        ProductionRankedKernelV1::new(
            "resource_boundary",
            0,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
    };

    assert!(build(maximum_constants).is_ok());
    assert_eq!(
        build(maximum_constants + 1),
        Err(ProductionRankedKernelErrorV1::ResourceLimit {
            resource: "operation tree work",
            limit: HARD_MAX_SESSION_OPERATION_TREE_ITEMS,
            actual: HARD_MAX_SESSION_OPERATION_TREE_ITEMS + 1,
        })
    );
}
