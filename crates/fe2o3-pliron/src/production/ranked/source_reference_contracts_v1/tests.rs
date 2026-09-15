use super::*;
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_functional_proof::{
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2, SafeReferenceKindV2,
    UnsignedFunctionalRefinementReceiptV2,
};

type E = ProductionSourceContractExportErrorV1;
type O = ProductionRankedOperationV1;
type X = ProductionSemanticExpressionV2;

fn id(value: u32) -> ProductionRankedValueIdV1 {
    ProductionRankedValueIdV1::new(value)
}
fn local(value: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(id(value))
}
fn digest(value: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([value; 32])
}
fn f32_constant(value: f32) -> X {
    X::Constant {
        scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
        bits: u64::from(value.to_bits()),
    }
}

// Same policy-staging fixture as production_ranked_pipeline. A signature under
// this test key is intentionally not compiler proof-execution authority.
fn imported(
    binding: FunctionalRefinementBindingV2,
) -> (
    ProductionReferenceProofV2,
    ImportedFunctionalRefinementProofV2,
    ProductionRefinementStagingPolicyV2,
) {
    let signing = SigningKey::from_bytes(&[91; 32]);
    let toolchain =
        VerusToolchainIdentityV2::new(digest(10), digest(11), digest(12), digest(13), digest(14))
            .unwrap();
    let boundary = FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir;
    let policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        boundary,
    )
    .unwrap();
    let production_policy =
        ProductionRefinementStagingPolicyV2::new([policy.signer_identity()], toolchain).unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        digest(20),
        FunctionalRefinementResultV2::Proved,
        boundary,
    )
    .unwrap();
    let wire = unsigned
        .clone()
        .attach_signature(signing.sign(unsigned.signing_bytes()).to_bytes());
    let imported = FunctionalRefinementReceiptImporterV2::new(policy, 1)
        .unwrap()
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    (
        ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding),
        imported,
        production_policy,
    )
}

fn input(expression: X) -> ProductionRankedKernelLoweringInputV1 {
    input_with_read(expression.clone(), expression, false).unwrap()
}

fn input_with_read(
    gpu: X,
    reference: X,
    include_read: bool,
) -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV2> {
    let subjects = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        digest(1),
        DigestV1::ZERO,
        digest(2),
        digest(3),
        digest(4),
    )
    .unwrap();
    let effect = ProductionEffectRefinementContractV2::new(
        91,
        ProductionGpuWriteSiteV2::new(0, 7 + u32::from(include_read)),
        ProductionReferenceOutputSiteV2::new(2, 3, 4),
        local(0),
        vec![local(1)],
        vec![local(2)],
        vec![local(2)],
        local(3),
        local(3),
        local(3),
        local(3),
        local(4),
        local(5),
    )
    .unwrap();
    let numerical = ProductionNumericalContractV2::exact_for_expression(&gpu);
    let kernel = ProductionRankedKernelV1::new(
        "pointwise_output",
        0,
        vec![ProductionRankedBlockV1::new(
            {
                let mut operations = vec![
                    O::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [16, 1, 1],
                        workgroup_extents: [1, 1, 1],
                        subgroup_size: 1,
                        full_physical_workgroups: true,
                    },
                    O::ViewInSpace {
                        result: id(0),
                        element_width: u32::from(gpu.scalar().bit_width()),
                        writable: true,
                        shape: vec![16],
                        dynamic_extents: vec![],
                        memory_space: MemorySpaceAttr::Global,
                        allocation_origin: 7,
                        noalias_class: 9,
                    },
                    O::InvocationIndex {
                        result: id(1),
                        dimension: 0,
                        launch_extent: 16,
                    },
                    O::SemanticSymbol {
                        result: id(2),
                        symbol: 0,
                    },
                    O::SemanticConstant {
                        result: id(3),
                        value: 1,
                    },
                    O::SemanticExpression {
                        result: id(4),
                        expression: gpu,
                        numerical_contract: numerical,
                    },
                    O::SemanticExpression {
                        result: id(5),
                        expression: reference,
                        numerical_contract: numerical,
                    },
                    O::ValueAccess {
                        kind: AccessKindAttr::Write,
                        view: local(0),
                        indices: vec![local(1)],
                        value: local(4),
                    },
                    O::OwnershipContract {
                        view: local(0),
                        coverage: OwnershipCoverageAttr::TotalView,
                        partition: OwnershipPartitionAttr::ExactSets,
                    },
                    O::RequestEffectRefinement {
                        contract: effect.clone(),
                        subjects,
                    },
                ];
                if include_read {
                    operations.insert(
                        5,
                        O::Access {
                            kind: AccessKindAttr::Read,
                            view: local(0),
                            indices: vec![local(1)],
                        },
                    );
                }
                operations
            },
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let contract_operation = 9 + usize::from(include_read);
    compile_fixture(kernel, 0, contract_operation, &effect, subjects)
}

fn compile_fixture(
    kernel: ProductionRankedKernelV1,
    block: usize,
    operation: usize,
    effect: &ProductionEffectRefinementContractV2,
    subjects: FunctionalRefinementSubjectsV2,
) -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV2> {
    let hash = normalized_effect_refinement_hash_for_kernel_v2(
        &kernel, block, operation, effect, subjects,
    )
    .unwrap();
    let (request, proof, policy) =
        imported(FunctionalRefinementBindingV2::from_subjects(subjects, hash).unwrap());
    let kernel = kernel
        .bind_functional_refinement_request_v2(block, operation, request)
        .unwrap();
    compile_ranked_kernel_with_policy_checked_refinement_staging_v2(
        ProductionConstructionV1::ranked_kernel("source_contract_test", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
        vec![proof],
        policy,
    )
}

mod guarded_tests;

fn collect(
    input: &ProductionRankedKernelLoweringInputV1,
) -> Result<Vec<ProductionSourceOutputContractFactsV1<'_>>, E> {
    collect_output_facts(
        &input.kernel,
        &input.policy_checked_refinement_staging,
        None,
    )
}

fn memory_expression(
    operation: ProductionSemanticBinaryOpV2,
    read_mode: crate::production::ProductionSemanticReadModeV2,
) -> X {
    let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
    X::Binary {
        operation,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(X::Load(crate::production::ProductionSemanticLoadV2 {
            block: 0,
            operation: 5,
            scalar,
            read_mode,
            allocation_origin: 7,
            view: local(0),
            indices: vec![local(1)].into_boxed_slice(),
        })),
        rhs: Box::new(f32_constant(1.0)),
    }
}

#[test]
fn initial_read_output_contract_requires_live_memory_and_exact_source_replay() {
    use crate::production::ProductionSemanticReadModeV2::UnorderedNonVolatile as Mode;
    // Independently specified component reference: initial input[i] + 1.
    // The test receipt does not authenticate a compiler source/reference ABI.
    let gpu = memory_expression(ProductionSemanticBinaryOpV2::Add, Mode);
    let reference = memory_expression(ProductionSemanticBinaryOpV2::Add, Mode);
    let input = input_with_read(gpu, reference.clone(), true).unwrap();
    assert_eq!(
        collect(&input).unwrap_err(),
        E::UnsupportedLoad {
            block: 0,
            operation: 5
        }
    );
    let exported = input.export_live_source_reference_contracts_v1().unwrap();
    assert_eq!(exported.outputs().len(), 1);
    assert_eq!(exported.outputs()[0].reference_rhs(), &reference);
    assert_ne!(
        exported.outputs()[0].effect().gpu_value(),
        exported.outputs()[0].effect().reference_value()
    );
    assert!(!exported.grants_compiler_refinement_authority());
    assert!(!exported.grants_artifact_or_launch_authority());
    let context = &input._session.inner.context;
    let function = FuncOp::from_operation(input.source_contract_function.operation);
    let producer = function
        .get_entry_block(context)
        .deref(context)
        .iter(context)
        .find(|op| Operation::is_op::<dialect_kernel::SemanticTypedReadOp>(*op, context))
        .unwrap();
    producer.unlink(context);
    assert_eq!(
        input
            .export_live_source_reference_contracts_v1()
            .unwrap_err(),
        E::LiveOwnerChanged
    );
}

#[test]
fn volatile_memory_and_independent_cpu_operator_drift_are_not_contract_proofs() {
    use crate::production::ProductionSemanticReadModeV2 as Mode;
    for volatile in [false, true] {
        let mode = if volatile {
            Mode::UnorderedVolatile
        } else {
            Mode::UnorderedNonVolatile
        };
        let gpu = memory_expression(ProductionSemanticBinaryOpV2::Add, mode);
        let reference = memory_expression(
            if volatile {
                ProductionSemanticBinaryOpV2::Add
            } else {
                ProductionSemanticBinaryOpV2::Subtract
            },
            mode,
        );
        let error = input_with_read(gpu, reference, true)
            .err()
            .expect("volatile reads and changed CPU operators must reject");
        let ProductionRankedCompileErrorV2::Pipeline(ProductionRankedCompileErrorV1::Session(
            ProductionSessionErrorV1::RankedSemantic(error),
        )) = error
        else {
            panic!("volatile={volatile}: {error:?}");
        };
        if volatile {
            assert!(
                matches!(error.report().findings(),
                [fe2o3_kernel_analysis::PlironSemanticRefinementFindingV1::TypedExpressionRejected {
                    reason: "typed read volatility, ordering or predication lacks a proved contract"
                }]),
                "{error:?}"
            );
        } else {
            assert!(error.report().findings().is_empty());
            let effects = error.report().effect_refinement();
            assert_eq!(effects.contract_count(), 1);
            assert_eq!(effects.proved_contract_count(), 0);
            assert!(
                matches!(effects.findings(),
                [fe2o3_kernel_analysis::PlironEffectRefinementFindingV1::ValueMismatch { location, .. }]
                    if location.block() == 0 && location.operation() == 17),
                "{error:?}"
            );
        }
    }
}

fn effect_mut(
    input: &mut ProductionRankedKernelLoweringInputV1,
) -> &mut ProductionEffectRefinementContractV2 {
    let O::RequireEffectRefinement { contract, .. } = &mut input.kernel.blocks[0].operations[9]
    else {
        panic!("effect fixture")
    };
    contract
}

#[test]
fn live_export_retains_independent_reference_and_existing_contract_objects() {
    let input = input(f32_constant(42.5));
    let export = input.export_live_source_reference_contracts_v1().unwrap();
    assert_eq!(export.outputs().len(), 1);
    let output = &export.outputs()[0];
    assert_ne!(
        output.effect().reference_value(),
        output.effect().gpu_value()
    );
    assert!(!std::ptr::eq(
        output.reference_rhs(),
        output.source_gpu_rhs()
    ));
    let O::SemanticExpression { expression, .. } = &input.kernel.blocks[0].operations[6] else {
        panic!("CPU root")
    };
    assert!(std::ptr::eq(output.reference_rhs(), expression));
    assert_eq!(output.reference_rhs(), &f32_constant(42.5));
    assert!(std::ptr::eq(
        output.ownership_contract(),
        &input.kernel.blocks[0].operations[8]
    ));
    assert!(matches!(
        output.view_definition(),
        O::ViewInSpace {
            allocation_origin: 7,
            noalias_class: 9,
            ..
        }
    ));
    assert_eq!(
        output.proof_request().binding(),
        output.policy_checked_staging().binding()
    );
    assert_eq!(
        output.effect().reference_output_site(),
        ProductionReferenceOutputSiteV2::new(2, 3, 4)
    );
    assert_ne!(*export.live_graph_sha256(), [0; 32]);
    assert!(!export.grants_compiler_refinement_authority());
    assert!(!export.grants_artifact_or_launch_authority());
    assert!(
        !output
            .policy_checked_staging()
            .grants_source_to_isa_authority()
    );
    assert_eq!(
        export.live_graph_sha256(),
        input
            .export_live_source_reference_contracts_v1()
            .unwrap()
            .live_graph_sha256()
    );
}

#[test]
fn general_integer_and_float_arithmetic_policies_are_preserved() {
    for scalar in [
        ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        ProductionSemanticScalarTypeV2::Float { bits: 32 },
    ] {
        let expression = X::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(X::Symbol {
                symbol: 0x4000_0000,
                scalar,
            }),
            rhs: Box::new(X::Constant { scalar, bits: 0 }),
        };
        let input = input(expression.clone());
        let export = input.export_live_source_reference_contracts_v1().unwrap();
        assert_eq!(export.outputs()[0].reference_rhs(), &expression);
        assert_eq!(
            export.outputs()[0].numerical_contract(),
            ProductionNumericalContractV2::exact_for_expression(&expression)
        );
    }
}

#[test]
fn source_cpu_dag_is_not_replaced_by_gpu_rhs() {
    let mut input = input(f32_constant(1.0));
    let O::SemanticExpression { expression, .. } = &mut input.kernel.blocks[0].operations[6] else {
        panic!("CPU root")
    };
    *expression = f32_constant(2.0);
    assert_eq!(
        input
            .export_live_source_reference_contracts_v1()
            .unwrap_err(),
        E::ProofBindingMismatch
    );
}

#[test]
fn gpu_rhs_alias_cannot_serve_as_independent_cpu_reference() {
    let mut input = input(f32_constant(1.0));
    effect_mut(&mut input).reference_value = local(4);
    assert_eq!(
        collect(&input).unwrap_err(),
        E::ReferenceRootAliasesGpuValue
    );
}

#[test]
fn source_output_allocation_and_proof_subject_substitutions_reject() {
    for change in 0..4 {
        let mut input = input(f32_constant(1.0));
        match change {
            0 => {
                effect_mut(&mut input).reference_output_site =
                    ProductionReferenceOutputSiteV2::new(3, 3, 4)
            }
            1 => {
                let O::ViewInSpace {
                    allocation_origin, ..
                } = &mut input.kernel.blocks[0].operations[1]
                else {
                    panic!("view")
                };
                *allocation_origin = 8;
            }
            2 => {
                let O::RequireEffectRefinement { proof, .. } =
                    &mut input.kernel.blocks[0].operations[9]
                else {
                    panic!("proof")
                };
                let subjects = FunctionalRefinementSubjectsV2::new(
                    SafeReferenceKindV2::Mir,
                    digest(1),
                    DigestV1::ZERO,
                    digest(5),
                    digest(3),
                    digest(4),
                )
                .unwrap();
                *proof = ProductionReferenceProofV2::request_exact(
                    proof.receipt_identity(),
                    FunctionalRefinementBindingV2::from_subjects(
                        subjects,
                        proof.binding().normalized_obligation_effect_ir_hash(),
                    )
                    .unwrap(),
                );
            }
            _ => {
                input.policy_checked_refinement_staging[0].boundary =
                    FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron;
            }
        }
        assert!(
            input.export_live_source_reference_contracts_v1().is_err(),
            "substitution {change}"
        );
    }
}

#[test]
fn missing_output_contract_is_not_silently_exported() {
    let mut input = input(f32_constant(1.0));
    input.kernel.blocks[0].operations.pop();
    assert_eq!(collect(&input).unwrap_err(), E::MissingOutputContracts);
}

#[test]
fn unconsumed_staging_is_not_silently_exported() {
    let mut first = input(f32_constant(1.0));
    let mut second = input(f32_constant(2.0));
    first
        .policy_checked_refinement_staging
        .append(&mut second.policy_checked_refinement_staging);
    assert_eq!(
        first
            .export_live_source_reference_contracts_v1()
            .unwrap_err(),
        E::ProofBindingMismatch
    );
}

#[test]
fn partial_domain_coverage_and_dynamic_extent_reject_specifically() {
    for change in 0..3 {
        let mut input = input(f32_constant(1.0));
        match change {
            0 => {
                let O::OwnershipContract { coverage, .. } =
                    &mut input.kernel.blocks[0].operations[8]
                else {
                    panic!("ownership")
                };
                *coverage = OwnershipCoverageAttr::ExactEffectDomain;
            }
            1 => {
                let O::SemanticConstant { value, .. } = &mut input.kernel.blocks[0].operations[4]
                else {
                    panic!("domain")
                };
                *value = 0;
            }
            _ => {
                let O::ViewInSpace { shape, .. } = &mut input.kernel.blocks[0].operations[1] else {
                    panic!("view")
                };
                shape[0] = DYNAMIC_EXTENT;
            }
        }
        assert_eq!(collect(&input).unwrap_err(), E::UnsupportedPartialOrFrame);
    }
}

#[test]
fn appended_cfg_block_does_not_reuse_a_compiled_contract() {
    let mut input = input(f32_constant(1.0));
    input.kernel.blocks.push(ProductionRankedBlockV1::new(
        vec![],
        ProductionRankedTerminatorV1::Return,
    ));
    assert!(input.export_live_source_reference_contracts_v1().is_err());
}

#[test]
fn read_load_leaf_and_nested_reserved_load_symbols_reject_specifically() {
    for change in 0..3 {
        let mut input = input(f32_constant(1.0));
        let expected_operation;
        if change == 0 {
            input.kernel.blocks[0].operations.push(O::Access {
                kind: AccessKindAttr::Read,
                view: local(0),
                indices: vec![local(1)],
            });
            expected_operation = 10;
        } else {
            let O::SemanticExpression { expression, .. } =
                &mut input.kernel.blocks[0].operations[6]
            else {
                panic!("CPU root")
            };
            *expression = X::Unary {
                operation: ProductionSemanticUnaryOpV2::Negate,
                scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
                operand: Box::new(if change == 1 {
                    X::Symbol {
                        symbol: 0xc000_0000,
                        scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
                    }
                } else {
                    X::Load(crate::production::ProductionSemanticLoadV2 {
                        block: 0,
                        operation: 10,
                        scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
                        read_mode:
                            crate::production::ProductionSemanticReadModeV2::UnorderedNonVolatile,
                        allocation_origin: 7,
                        view: local(0),
                        indices: vec![local(1)].into_boxed_slice(),
                    })
                }),
            };
            expected_operation = 6;
        }
        assert_eq!(
            collect(&input).unwrap_err(),
            E::UnsupportedLoad {
                block: 0,
                operation: expected_operation
            }
        );
    }
}

#[test]
fn numerical_policy_is_not_repaired_or_inferred_during_export() {
    let mut input = input(f32_constant(1.0));
    let O::SemanticExpression {
        numerical_contract, ..
    } = &mut input.kernel.blocks[0].operations[6]
    else {
        panic!("CPU root")
    };
    *numerical_contract = ProductionNumericalContractV2::ExactBitVectorOperatorCongruence;
    assert_eq!(collect(&input).unwrap_err(), E::UnsupportedNumericalPolicy);
}

fn live_constant(input: &ProductionRankedKernelLoweringInputV1) -> SemanticTypedConstantOp {
    let context = &input._session.inner.context;
    let block =
        FuncOp::from_operation(input.source_contract_function.operation).get_entry_block(context);
    let pointer = block
        .deref(context)
        .iter(context)
        .find(|op| Operation::is_op::<SemanticTypedConstantOp>(*op, context))
        .unwrap();
    SemanticTypedConstantOp::from_operation(pointer)
}

#[test]
fn live_mutation_invalidates_export_even_if_recipe_and_cached_report_are_unchanged() {
    let mut input = input(f32_constant(1.0));
    let op = live_constant(&input);
    op.set_attr_kernel_semantic_typed_constant_bits(
        &input._session.inner.context,
        dialect_kernel::SemanticConstantAttr(u64::from(2.0_f32.to_bits())),
    );
    assert_eq!(
        input
            .export_live_source_reference_contracts_v1()
            .unwrap_err(),
        E::LiveOwnerChanged
    );
    // Exercise structural replay independently of the mutation-epoch check.
    input.source_contract_mutation_epoch = Some(
        input
            ._session
            .inner
            .context
            .ir_mutation_attempt_epoch()
            .unwrap()
            .value(),
    );
    assert!(matches!(
        input
            .export_live_source_reference_contracts_v1()
            .unwrap_err(),
        E::LiveGraphChanged | E::LiveAnalysisRejected
    ));
}

#[test]
fn net_zero_live_mutation_still_invalidates_epoch() {
    let input = input(f32_constant(1.0));
    let op = live_constant(&input);
    let context = &input._session.inner.context;
    let bits = op.bits(context).unwrap();
    op.set_attr_kernel_semantic_typed_constant_bits(
        context,
        dialect_kernel::SemanticConstantAttr(bits ^ 1),
    );
    op.set_attr_kernel_semantic_typed_constant_bits(
        context,
        dialect_kernel::SemanticConstantAttr(bits),
    );
    assert_eq!(
        input
            .export_live_source_reference_contracts_v1()
            .unwrap_err(),
        E::LiveOwnerChanged
    );
}

#[test]
fn foreign_context_function_and_staging_reject() {
    let mut first = input(f32_constant(1.0));
    let second = input(f32_constant(1.0));
    // Reproduce the same-slot collision in the pinned arena-index-only Ptr.
    assert_eq!(
        first.source_contract_function.operation,
        second.source_contract_function.operation
    );
    assert_ne!(
        first.source_contract_function.owner,
        second.source_contract_function.owner
    );
    first.source_contract_function = second.source_contract_function;
    assert_eq!(
        first
            .export_live_source_reference_contracts_v1()
            .unwrap_err(),
        E::LiveOwnerChanged
    );
    let mut first = input(f32_constant(1.0));
    // Correct context, wrong live operation must also reject before analysis.
    first.source_contract_function.operation = live_constant(&first).get_operation();
    assert_eq!(
        first
            .export_live_source_reference_contracts_v1()
            .unwrap_err(),
        E::LiveOwnerChanged
    );
    let mut first = input(f32_constant(1.0));
    first.policy_checked_refinement_staging.clear();
    assert_eq!(
        first
            .export_live_source_reference_contracts_v1()
            .unwrap_err(),
        E::ProofBindingMismatch
    );
}
