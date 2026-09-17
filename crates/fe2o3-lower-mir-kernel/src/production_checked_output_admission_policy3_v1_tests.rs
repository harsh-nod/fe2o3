use super::*;
use crate::{
    ProductionCheckedOutputAdmissionErrorPolicy3V1 as AdmissionError,
    ProductionCheckedOutputOwnerPolicy3V1 as AdmittedOutput,
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1;
use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;

fn scalar_source(extra_block: bool, borrowed: bool) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture_with_blocks_and_symbol(
        if borrowed {
            Fixture::ElidedBounds
        } else {
            Fixture::Literal(true)
        },
        false,
        |_, _| {
            let statements = if borrowed {
                vec![]
            } else {
                vec![
                    assignment(1, BOOL, SemanticRvalueKindV1::Use(constant(BOOL, 1, 1))),
                    assignment(
                        2,
                        BOOL,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::BitXor,
                            left: value(1, BOOL),
                            right: constant(BOOL, 1, 1),
                        },
                    ),
                ]
            };
            let mut blocks = vec![block(31, statements, SemanticTerminatorKindV1::Return)];
            if extra_block {
                blocks.push(block(32, vec![], SemanticTerminatorKindV1::Return));
            }
            blocks
        },
        |_| "private_array_relation".to_owned(),
        if borrowed { &[] } else { &[BOOL, BOOL] },
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

struct Prepared {
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    bound: VerifiedCanonicalKernelIrModuleV12,
    output: CheckedNeutralKernelIrOwnerPolicy3V1,
    source_storage: usize,
    bound_storage: usize,
    output_storage: usize,
}

// Preparation transfers real owners and their receipts into the next test
// ledger. No constructor or whole-pipeline numeric limit is inferred here.
fn prepare(
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    profile: Profile,
    mutation: Option<fn(&mut Module)>,
) -> Prepared {
    let source_storage = receipt
        .materialized
        .unit_local_source_storage_floor_v1()
        .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(source_storage).unwrap();
    let binding = dialect_amdgcn::bind_production_target_v1(
        receipt.materialized.executable().module(),
        profile,
    )
    .unwrap();
    let (bound, storage) = if let Some(mutate) = mutation {
        // Hostile B components are freshly independently verified; the source
        // owner and checked optimizer owner are never mutated in place.
        let mut candidate = binding.module().clone();
        mutate(&mut candidate);
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &candidate,
            &mut budget,
        )
        .unwrap()
    } else {
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            binding.module(),
            &mut budget,
        )
        .unwrap()
    };
    let bound_storage = storage.retained_storage();
    budget.reserve_storage(bound_storage).unwrap();
    drop(binding);
    let output = optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut budget).unwrap();
    assert_eq!(output.report().passes().len(), 8);
    let output_storage = output.storage().retained_storage();
    budget.reserve_storage(output_storage).unwrap();
    assert_eq!(
        budget.storage(),
        source_storage + bound_storage + output_storage
    );
    Prepared {
        receipt,
        bound,
        output,
        source_storage,
        bound_storage,
        output_storage,
    }
}

fn with_prepared(input: Prepared, check: impl FnOnce(Prepared, &mut AssertOriginBudgetV1<'_>)) {
    let retained = input.source_storage + input.bound_storage + input.output_storage;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR + retained).unwrap();
    check(input, &mut budget);
    assert_eq!(budget.storage(), FLOOR + retained);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn closed_policy3_admission_consumes_real_source_ranked_custody_and_actual_changed_output() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let receipt = array_output_ranked_receipt_v1(scalar_source(false, false));
        with_prepared(prepare(receipt, profile, None), |input, budget| {
            assert_ne!(
                input.bound.canonical().identity(),
                input.output.owner().canonical().identity()
            );
            let expected = *input.output.owner().canonical().identity();
            let owner = AdmittedOutput::try_admit(input.receipt, input.bound, input.output, budget)
                .unwrap();
            assert_eq!(owner.output().canonical().identity(), &expected);
            assert!(std::ptr::eq(owner.output(), owner.checked_output().owner()));
            assert!(!std::ptr::eq(owner.output(), owner.bound()));
            assert!(!std::ptr::eq(
                owner.output(),
                owner.source_semantic_kir().pre_ranked_executable().unwrap()
            ));
            assert!(!owner.grants_artifact_or_launch_authority());
            let [kernel] = owner.kernels() else {
                panic!("one admitted output root")
            };
            assert_eq!(kernel.kernel(), &owner.output().module().kernels[0].id);
            assert_eq!(kernel.entry(), &owner.output().module().kernels[0].entry);
            assert!(kernel.allocations().is_empty());
            assert!(kernel.accesses().is_empty());
            assert!(kernel.bounds_requirements().is_empty());
            assert!(kernel.runtime_alias_requirements().is_empty());
            assert!(kernel.inter_invocation_conflicts().is_empty());
            let incoming = budget.storage();
            owner.verify_equivalence(budget).unwrap();
            assert_eq!(budget.storage(), incoming);
            drop(owner);
        });
    }
}

#[test]
fn complete_private_structural_reports_are_not_final_private_address_admission() {
    let receipt = array_output_ranked_receipt_v1(array_owner(ArrayCase::Initializer {
        values: [11; 8],
        repetitions: 1,
        float: false,
    }));
    with_prepared(prepare(receipt, Profile::Gfx942, None), |input, budget| {
        let report = crate::analyze_checked_output_formal_memory_policy3_v1(&input.output).unwrap();
        assert!(report.kernels()[0].accesses().is_empty());
        drop(report);
        assert!(matches!(
            AdmittedOutput::try_admit(input.receipt, input.bound, input.output, budget),
            Err(AdmissionError::PrivateAddressR2)
        ));
    });
}

#[test]
fn unreachable_source_blocks_and_borrowed_interfaces_remain_outside_the_subset() {
    for (extra_block, borrowed) in [(true, false), (false, true)] {
        let receipt = array_output_ranked_receipt_v1(scalar_source(extra_block, borrowed));
        with_prepared(prepare(receipt, Profile::Gfx942, None), |input, budget| {
            let error = AdmittedOutput::try_admit(input.receipt, input.bound, input.output, budget)
                .unwrap_err();
            assert!(matches!(
                error,
                AdmissionError::Unsupported {
                    phase: "source",
                    ..
                }
            ));
        });
    }
}

#[test]
fn existing_unit_local_receipt_gate_prevents_final_input_construction() {
    let source = array_owner_at_body(ArrayCase::Write { sparse: false }, true).unwrap();
    assert_eq!(
        source.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::UnitLocal
    );
    // This intentionally tests the earlier genuine receipt gate, not a forged
    // final constructor input or a claim that UnitLocal is already admitted.
    let result =
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            source,
            vec![],
        );
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                consumer: "materialized ranked receipt"
            }
        )
    ));
}

#[test]
fn checked_output_with_equal_final_bytes_cannot_replace_the_bound_input_history() {
    let first = array_output_ranked_receipt_v1(scalar_source(false, false));
    let second = array_output_ranked_receipt_v1(scalar_source(false, false));
    let mut input = prepare(first, Profile::Gfx942, None);
    let other = prepare(
        second,
        Profile::Gfx942,
        Some(|module| {
            let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
            let constant = operations
                .iter_mut()
                .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Bool(_))))
                .unwrap();
            let OperationKind::Constant(Constant::Bool(value)) = &mut constant.kind else {
                unreachable!()
            };
            *value = !*value;
        }),
    );
    assert_ne!(
        input.bound.canonical().identity(),
        other.bound.canonical().identity()
    );
    assert_eq!(
        input.output.owner().canonical().canonical_bytes(),
        other.output.owner().canonical().canonical_bytes()
    );
    input.output = other.output;
    input.output_storage = other.output_storage;
    drop((other.receipt, other.bound));
    with_prepared(input, |input, budget| {
        assert!(matches!(
            AdmittedOutput::try_admit(input.receipt, input.bound, input.output, budget),
            Err(AdmissionError::SourceOutput(
                ProductionSourceOutputErrorV1::InputCustody
            ))
        ));
    });
}

#[test]
fn every_bound_function_block_effect_and_non_total_recipe_is_censused_before_optimization_erasure()
{
    let cases: [fn(&mut Module); 4] = [
        |module| {
            let mut block = BasicBlock::new(BlockId(901));
            block.terminator = Some(Terminator::Return { values: vec![] });
            module.functions[0]
                .body
                .as_mut()
                .unwrap()
                .blocks
                .push(block);
        },
        |module| {
            let mut block = BasicBlock::new(BlockId(0));
            block.terminator = Some(Terminator::Return { values: vec![] });
            module.functions.push(Function::internal_helper(
                "unclaimed",
                Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            ));
        },
        |module| {
            module.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .push(Operation::new(
                    vec![],
                    OperationKind::Fence(fe2o3_kernel_ir::Fence {
                        memory_scope: SynchronizationScope::Device,
                        semantics: BarrierSemantics::new(
                            MemoryOrdering::Release,
                            [AddressSpace::Global],
                        ),
                    }),
                ));
        },
        |module| {
            let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
            let ty = Type::Scalar(ScalarType::U32);
            operations.push(Operation::effect_free(
                ValueDef::new(ValueId(900), ty.clone()),
                OperationKind::Constant(Constant::U32(1)),
            ));
            operations.push(Operation::effect_free(
                ValueDef::new(ValueId(901), ty),
                OperationKind::Binary {
                    op: BinaryOp::Divide,
                    lhs: ValueId(900),
                    rhs: ValueId(900),
                },
            ));
        },
    ];
    for mutate in cases {
        let receipt = array_output_ranked_receipt_v1(scalar_source(false, false));
        with_prepared(
            prepare(receipt, Profile::Gfx942, Some(mutate)),
            |input, budget| {
                assert!(matches!(
                    AdmittedOutput::try_admit(input.receipt, input.bound, input.output, budget),
                    Err(AdmissionError::Unsupported { phase: "B", .. })
                ));
            },
        );
    }
}

#[test]
fn independently_compiled_extra_ranked_operation_is_not_hidden_by_empty_source_effect_lists() {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let mut receipt = array_output_ranked_receipt_v1(scalar_source(false, false));
    let mut operations = receipt.roots[0].lowering.kernel().blocks()[0]
        .operations()
        .to_vec();
    operations.push(ProductionRankedOperationV1::IndexUnknown {
        result: ProductionRankedValueIdV1::new(0),
    });
    let kernel = ProductionRankedKernelV1::new(
        "private_array_relation",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("private_array_relation", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    assert!(lowering.all_mandatory_reports_are_clean());
    // A private hostile receipt component; this is not emitted by the backend
    // source projector. Its independently compiled ranked graph is still real.
    receipt.roots[0].lowering = lowering;
    with_prepared(prepare(receipt, Profile::Gfx942, None), |input, budget| {
        assert!(matches!(
            AdmittedOutput::try_admit(input.receipt, input.bound, input.output, budget),
            Err(AdmissionError::Unsupported {
                phase: "ranked",
                ..
            })
        ));
    });
}

#[test]
fn entry_work_precedes_missing_floor_and_successful_revalidation_requires_the_retained_floor() {
    for (limit, is_work) in [(7, true), (8, false)] {
        let input = prepare(
            array_output_ranked_receipt_v1(scalar_source(false, false)),
            Profile::Gfx942,
            None,
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let error =
            AdmittedOutput::try_admit(input.receipt, input.bound, input.output, &mut budget)
                .unwrap_err();
        if is_work {
            assert!(matches!(
                error,
                AdmissionError::Resource(AssertOriginResourceV1::Work(_))
            ));
            assert_eq!(budget.work(), 0);
        } else {
            assert!(matches!(
                error,
                AdmissionError::Resource(AssertOriginResourceV1::Accounting)
            ));
            assert_eq!(budget.work(), 8);
        }
        assert_eq!(budget.storage(), FLOOR);
    }
    let input = prepare(
        array_output_ranked_receipt_v1(scalar_source(false, false)),
        Profile::Gfx942,
        None,
    );
    with_prepared(input, |input, budget| {
        let minimum = input.source_storage + input.output_storage;
        let owner =
            AdmittedOutput::try_admit(input.receipt, input.bound, input.output, budget).unwrap();
        let removed = budget.storage() - (minimum - 1);
        budget.release_storage(removed).unwrap();
        assert!(matches!(
            owner.verify_equivalence(budget),
            Err(AdmissionError::Resource(AssertOriginResourceV1::Accounting))
        ));
        assert_eq!(budget.storage(), minimum - 1);
        budget.reserve_storage(removed).unwrap();
        owner.verify_equivalence(budget).unwrap();
        drop(owner);
    });
}

fn ranked_control_receipt(
    control: Vec<(u32, fe2o3_pliron::ProductionRankedTerminatorV1)>,
) -> Result<ProductionMaterializedRankedModuleReceiptV1, fe2o3_pliron::ProductionRankedCompileErrorV1>
{
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
    };
    let mut receipt = array_output_ranked_receipt_v1(scalar_source(false, false));
    let layout = receipt.roots[0].lowering.kernel().blocks()[0].operations()[0].clone();
    let blocks = control
        .into_iter()
        .enumerate()
        .map(|(index, (arguments, terminator))| {
            let operations = if index == 0 {
                vec![
                    layout.clone(),
                    ProductionRankedOperationV1::IndexConstant {
                        result: ProductionRankedValueIdV1::new(0),
                        value: 0,
                    },
                ]
            } else {
                vec![]
            };
            ProductionRankedBlockV1::with_index_arguments(arguments, operations, terminator)
        })
        .collect();
    let kernel = ProductionRankedKernelV1::new("private_array_relation", 0, blocks).unwrap();
    let lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("private_array_relation", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )?;
    assert!(lowering.all_mandatory_reports_are_clean());
    // Private graph components use the real ranked compiler. They are not
    // claimed to be the backend projector's output; its fixture is separate.
    receipt.roots[0].lowering = lowering;
    Ok(receipt)
}

#[test]
fn ranked_linear_prologues_cover_every_block_independent_of_storage_order() {
    use fe2o3_pliron::ProductionRankedTerminatorV1::{Branch, Return};
    for control in [
        vec![(0, Branch { target: 1 }), (0, Return)],
        vec![
            (0, Branch { target: 2 }),
            (0, Return),
            (0, Branch { target: 1 }),
        ],
    ] {
        let count = control.len();
        let receipt = ranked_control_receipt(control).unwrap();
        assert_eq!(receipt.roots[0].lowering.kernel().blocks().len(), count);
        with_prepared(prepare(receipt, Profile::Gfx942, None), |input, budget| {
            let owner = AdmittedOutput::try_admit(input.receipt, input.bound, input.output, budget)
                .unwrap();
            owner.verify_equivalence(budget).unwrap();
            let floor = budget.storage();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(7);
            let mut short = AssertOriginBudgetV1::new(&mut work, STORAGE);
            short.reserve_storage(floor).unwrap();
            assert!(matches!(
                owner.verify_equivalence(&mut short),
                Err(AdmissionError::Resource(AssertOriginResourceV1::Work(_)))
            ));
            assert_eq!(short.storage(), floor);
            assert_eq!(short.work(), 0);
            drop(owner);
        });
    }
}

#[test]
fn ranked_linear_admission_does_not_hide_unvisited_trapping_conditional_or_argument_blocks() {
    use fe2o3_pliron::ProductionRankedTerminatorV1::{
        Branch, BranchArgs, IndexEqual, Return, Trap,
    };
    let zero = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
    for control in [
        vec![(0, Return), (0, Return)],
        vec![(0, Branch { target: 2 }), (0, Return), (0, Return)],
        vec![(0, Branch { target: 1 }), (0, Trap)],
        vec![
            (
                0,
                IndexEqual {
                    lhs: zero,
                    rhs: zero,
                    true_block: 1,
                    false_block: 2,
                },
            ),
            (0, Return),
            (0, Return),
        ],
        vec![
            (
                0,
                BranchArgs {
                    arguments: vec![zero],
                    target: 1,
                },
            ),
            (1, Return),
        ],
        vec![(0, Branch { target: 0 })],
    ] {
        match ranked_control_receipt(control) {
            Ok(receipt) => {
                with_prepared(prepare(receipt, Profile::Gfx942, None), |input, budget| {
                    assert!(matches!(
                        AdmittedOutput::try_admit(input.receipt, input.bound, input.output, budget),
                        Err(AdmissionError::Unsupported {
                            phase: "ranked",
                            ..
                        })
                    ));
                })
            }
            // Some graphs, especially nonterminating ones, may already be
            // refused by mandatory live analyses. This is an earlier boundary,
            // not a claim that an unavailable lowering input reached M5.
            Err(error) => assert!(matches!(
                error,
                fe2o3_pliron::ProductionRankedCompileErrorV1::Session(_)
            )),
        }
    }
}

#[test]
fn out_of_range_ranked_successor_is_refused_before_an_admission_input_exists() {
    use fe2o3_pliron::{
        ProductionRankedBlockV1, ProductionRankedKernelV1, ProductionRankedTerminatorV1,
    };
    let result = ProductionRankedKernelV1::new(
        "invalid_successor",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![],
            ProductionRankedTerminatorV1::Branch { target: 1 },
        )],
    );
    assert!(result.is_err());
}
