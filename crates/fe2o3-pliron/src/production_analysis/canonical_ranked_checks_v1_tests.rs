use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV1 as Inventory, CanonicalRankedMetadataV1 as Metadata,
    build_canonical_ranked_candidate_v1, with_checked_canonical_ranked_view_v1,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CheckedBinaryOperator, Constant, Function, Kernel, LaunchDomain, LaunchExtent, Module,
    Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};
use pliron::{
    builtin::attributes::StringAttr,
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    operation::Operation as LiveOperation,
};

const WORK: usize = 1 << 48;
const STORAGE: usize = 1 << 32;
std::thread_local! {
    static MUTATE_BETWEEN: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn between_functions(projection: &Projection<'_>, ordinal: usize) {
    if ordinal == 0 && MUTATE_BETWEEN.with(|flag| flag.replace(false)) {
        projection.test_live(|context, root| {
            let original = root.deref(context).attributes.clone();
            root.deref_mut(context).attributes = original;
        });
    }
}

pub(super) fn noop() -> Module {
    let mut entry = BasicBlock::new(BlockId(91));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("canonical-native-checks");
    module.functions.push(Function::kernel_entry(
        "zeta",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    module.kernels.push(Kernel::new(
        "zeta",
        "zeta",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}
pub(super) fn owner(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, storage.retained_storage())
}
pub(super) fn with_checked<T>(
    module: &Module,
    run: impl FnOnce(&mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>, &mut Budget<'_>) -> T,
) -> T {
    let (owner, storage) = owner(module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(storage).unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let metadata = Metadata::new(&owner, &[]);
    let metadata_storage = metadata.storage_extent(&mut budget).unwrap();
    budget.reserve_storage(metadata_storage).unwrap();
    let (candidate, candidate_storage) =
        build_canonical_ranked_candidate_v1(&inventory, &metadata, &mut budget).unwrap();
    budget
        .reserve_storage(candidate_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let result = with_checked_canonical_ranked_view_v1(
        &inventory,
        &metadata,
        &candidate,
        &mut budget,
        |checked, budget| Ok::<_, CanonicalRankedViewErrorV1>(run(checked, budget)),
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    drop(candidate);
    budget
        .release_storage(candidate_storage.retained_storage())
        .unwrap();
    drop(metadata);
    budget.release_storage(metadata_storage).unwrap();
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), 0);
    result
}

fn payload_module(duplicate: bool, cycle: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(17));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(11), Type::BOOL),
            OperationKind::Constant(Constant::Bool(true)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(12), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(8)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(13), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(9)),
        ),
    ];
    entry.terminator = Some(if duplicate {
        Terminator::ConditionalBranch {
            condition: ValueId(11),
            then_target: BlockId(19),
            then_arguments: vec![ValueId(12)],
            else_target: BlockId(19),
            else_arguments: vec![ValueId(13)],
        }
    } else {
        Terminator::Branch {
            target: BlockId(19),
            arguments: vec![ValueId(12)],
        }
    });
    let mut end = BasicBlock::new(BlockId(19));
    end.parameters
        .push(ValueDef::new(ValueId(14), Type::Scalar(ScalarType::U32)));
    end.terminator = Some(if cycle {
        Terminator::Branch {
            target: BlockId(19),
            arguments: vec![ValueId(14)],
        }
    } else {
        Terminator::Return {
            values: vec![ValueId(14)],
        }
    });
    let mut module = Module::new("typed-native-return");
    module.functions.push(Function::internal_helper(
        "canonical_name_not_kir_fn_0",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
        vec![],
        vec![entry, end],
    ));
    module
}

#[test]
fn actual_native_controls_reach_all_nine_and_keep_all_obligations_pending() {
    for module in [
        noop(),
        payload_module(false, false),
        payload_module(true, false),
    ] {
        with_checked(&module, |checked, budget| {
            let expected = checked.inventory(budget).unwrap().owner();
            with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
                assert!(std::ptr::eq(policies.owner(budget)?, expected));
                assert_eq!(policies.function_count(budget)?, 1);
                let report = policies.report(0, budget)?;
                assert_eq!(
                    report.pass_order(),
                    &super::super::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
                );
                assert!(report.is_clean());
                assert_eq!(policies.pending_obligations().iter().count(), 19);
                assert!(!policies.ranked_verification_is_complete());
                assert!(!policies.grants_artifact_or_launch_authority());
                assert!(policies.observation(budget)?.work_upper_bound() > 0);
                Ok(())
            })
            .unwrap();
        });
    }
}

#[test]
fn empty_module_is_not_a_fabricated_nine_pass_report() {
    with_checked(&Module::new("empty"), |checked, budget| {
        with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
            assert_eq!(policies.function_count(budget)?, 0);
            assert_eq!(policies.observation(budget)?.work_upper_bound(), 0);
            assert_eq!(policies.pending_obligations().iter().count(), 19);
            Ok(())
        })
        .unwrap();
    });
}

pub(super) fn two_roots() -> Module {
    let mut module = noop();
    let mut end = BasicBlock::new(BlockId(701));
    end.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "alpha",
        Signature::new(vec![], vec![]),
        vec![],
        vec![end],
    ));
    module.kernels.push(Kernel::new(
        "alpha",
        "alpha",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(32),
        },
    ));
    module
}

#[test]
fn all_functions_share_one_cumulative_analysis_allowance_and_ordered_subject() {
    let module = two_roots();
    with_checked(&module, |checked, budget| {
        with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
            assert_eq!(policies.owner(budget)?.module().kernels, module.kernels);
            assert_eq!(policies.function_count(budget)?, 2);
            let first = policies.history(0, budget)?;
            let second = policies.history(1, budget)?;
            assert_eq!(first.floor().work_upper_bound(), 0);
            assert_eq!(
                second.floor().work_upper_bound(),
                first.invocation().work_upper_bound()
            );
            assert_eq!(
                second.floor().retained_storage_units(),
                first.invocation().retained_storage_units()
            );
            assert_eq!(
                policies.observation(budget)?.work_upper_bound(),
                first.invocation().work_upper_bound() + second.invocation().work_upper_bound()
            );
            assert!(policies.report(0, budget)?.is_clean());
            assert!(policies.report(1, budget)?.is_clean());
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn native_reachable_cycle_remains_real_typed_progress_refusal() {
    let mut module = noop();
    let function = &mut module.functions[0];
    function.signature.parameters = vec![Type::BOOL];
    let body = function.body.as_mut().unwrap();
    body.parameters = vec![ValueId(0)];
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(92),
        then_arguments: vec![],
        else_target: BlockId(93),
        else_arguments: vec![],
    });
    let mut cycle = BasicBlock::new(BlockId(92));
    cycle.terminator = Some(Terminator::Branch {
        target: BlockId(92),
        arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(93));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([cycle, exit]);
    with_checked(&module, |checked, budget| {
        let error = with_canonical_ranked_policy_checks_v1(
            checked,
            budget,
            |_, _| -> Result<(), Failure> {
                panic!("cyclic native CFG must not obtain restricted policy success");
            },
        )
        .unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::Analysis { function: 0, .. }
        ));
        assert!(error.observation().work_upper_bound() > 0);
        assert_eq!(error.last_invocation().unwrap().function(), 0);
        let Failure::Analysis {
            cause: ProductionPlironPreloweringErrorV2::Semantic(cause),
            ..
        } = error.failure()
        else {
            panic!("expected the actual semantic progress refusal: {error:?}");
        };
        assert!(cause.report().progress().findings().iter().any(|finding| {
            matches!(
                finding,
                crate::PlironProgressFindingV1::ProgressIncomplete { .. }
            )
        }));
    });
}

#[test]
fn ordered_kernel_roster_is_joined_independently_of_physical_function_order() {
    let mut module = two_roots();
    module.kernels.reverse();
    with_checked(&module, |checked, budget| {
        with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
            assert_eq!(policies.owner(budget)?.module().kernels, module.kernels);
            assert_eq!(policies.function_count(budget)?, 2);
            for ordinal in 0..2 {
                assert_eq!(policies.history(ordinal, budget)?.function(), ordinal);
                assert_eq!(policies.report(ordinal, budget)?.pass_order().len(), 9);
            }
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn checked_integer_shape_and_ieee_constant_bits_are_exact_projection_data() {
    let mut module = payload_module(false, false);
    let entry = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    entry.operations.push(Operation::new(
        vec![
            ValueDef::new(ValueId(22), Type::Scalar(ScalarType::U32)),
            ValueDef::new(ValueId(23), Type::BOOL),
        ],
        OperationKind::Binary {
            op: BinaryOp::Checked(CheckedBinaryOperator::Add),
            lhs: ValueId(12),
            rhs: ValueId(13),
        },
    ));
    for (id, bits) in [(24, 0x8000_0000), (25, 0x7fc0_0042)] {
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::F32),
            OperationKind::Constant(Constant::F32Bits(bits)),
        ));
    }
    with_checked(&module, |checked, budget| {
        with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
            assert_eq!(policies.owner(budget)?.module(), &module);
            assert!(policies.report(0, budget)?.is_clean());
            Ok(())
        })
        .unwrap();
    });
}

fn first_function(context: &Context, root: Ptr<LiveOperation>) -> Ptr<LiveOperation> {
    let region = root.deref(context).get_region(0);
    let block = region.deref(context).iter(context).next().unwrap();
    block.deref(context).iter(context).next().unwrap()
}

pub(super) fn with_projection<T>(
    module: &Module,
    run: impl FnOnce(&mut Projection<'_>, &mut Budget<'_>) -> T,
) -> T {
    let (owner, storage) = owner(module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(storage).unwrap();
    let result = resources::protected(&mut budget, |budget| {
        let mut projection = Projection::import(&owner, budget)?;
        Ok(run(&mut projection, budget))
    })
    .unwrap();
    assert_eq!(budget.storage(), storage);
    drop(owner);
    budget.release_storage(storage).unwrap();
    result
}

#[test]
fn symbols_extra_semantic_attributes_and_native_block_metadata_refuse_independently() {
    for which in 0..4 {
        with_projection(&noop(), |projection, budget| {
            projection.test_live(|context, root| {
                let function = first_function(context, root);
                match which {
                    0 => {
                        function.deref_mut(context).attributes.set(
                            "sym_name".try_into().unwrap(),
                            pliron::builtin::attributes::IdentifierAttr::new(
                                "kir_fn_00".try_into().unwrap(),
                            ),
                        );
                    }
                    1 => function.deref_mut(context).attributes.set(
                        "unexpected_semantics".try_into().unwrap(),
                        StringAttr::new("claim".into()),
                    ),
                    2 => {
                        let region = function.deref(context).get_region(0);
                        let block = region.deref(context).iter(context).next().unwrap();
                        block.deref_mut(context).attributes.set(
                            "unexpected_semantics".try_into().unwrap(),
                            StringAttr::new("claim".into()),
                        );
                    }
                    _ => root.deref_mut(context).attributes.set(
                        "unexpected_semantics".try_into().unwrap(),
                        StringAttr::new("claim".into()),
                    ),
                }
            });
            assert!(matches!(projection.check(budget), Err(Failure::Mutation)));
            projection.test_rebase_epoch();
            assert!(matches!(
                projection.check(budget),
                Err(Failure::NativeSchema)
            ));
        });
    }
}

#[test]
fn same_typed_constant_substitution_cannot_pass_complete_byte_comparison() {
    with_projection(&payload_module(false, false), |projection, budget| {
        projection.test_live(|context, root| {
            let function = first_function(context, root);
            let region = function.deref(context).get_region(0);
            let block = region.deref(context).iter(context).next().unwrap();
            let operations = block.deref(context).iter(context).collect::<Vec<_>>();
            let old = operations[1].deref(context).attributes.clone();
            operations[1].deref_mut(context).attributes =
                operations[2].deref(context).attributes.clone();
            assert_ne!(old, operations[1].deref(context).attributes);
        });
        projection.test_rebase_epoch();
        assert!(matches!(
            projection.check(budget),
            Err(Failure::Bridge(_) | Failure::ExactGraph)
        ));
    });
}

#[test]
fn duplicate_target_payload_order_is_part_of_the_exact_relation() {
    with_projection(&payload_module(true, false), |projection, budget| {
        projection.test_live(|context, root| {
            let function = first_function(context, root);
            let region = function.deref(context).get_region(0);
            let entry = region.deref(context).iter(context).next().unwrap();
            let branch = entry.deref(context).get_terminator(context).unwrap();
            let first = branch.deref(context).get_operand(1);
            let second = branch.deref(context).get_operand(2);
            assert_ne!(first, second);
            LiveOperation::replace_operand(branch, context, 1, second);
            LiveOperation::replace_operand(branch, context, 2, first);
        });
        assert!(matches!(projection.check(budget), Err(Failure::Mutation)));
        projection.test_rebase_epoch();
        assert!(matches!(
            projection.check(budget),
            Err(Failure::Bridge(_) | Failure::ExactGraph)
        ));
    });
}

#[test]
fn native_wrong_return_type_is_refused_by_the_real_ordinary_verifier() {
    with_projection(&payload_module(true, false), |projection, budget| {
        projection.test_live(|context, root| {
            let function = first_function(context, root);
            let region = function.deref(context).get_region(0);
            let blocks = region.deref(context).iter(context).collect::<Vec<_>>();
            let branch = blocks[0].deref(context).get_terminator(context).unwrap();
            let boolean = branch.deref(context).get_operand(0);
            let returned = blocks[1].deref(context).get_terminator(context).unwrap();
            LiveOperation::replace_operand(returned, context, 0, boolean);
        });
        projection.test_rebase_epoch();
        let mut analysis = AnalysisState::new(Limits::production_hard_ceiling());
        let result = projection
            .with_function(0, budget, |context, function| {
                analysis.invoke(0, context, function)
            })
            .unwrap();
        assert!(matches!(result, Err(Failure::Analysis { function: 0, .. })));
        assert_eq!(analysis.last.unwrap().function(), 0);
        assert!(projection.check(budget).is_err());
    });
}

#[test]
fn fixed_pass_and_between_function_mutate_restore_are_detected() {
    for mode in 0..3 {
        with_checked(&two_roots(), |checked, budget| {
            match mode {
                0 => super::super::pliron_pipeline::transiently_mutate_next_production_analysis_for_test_v1(),
                1 => super::super::pliron_pipeline::structurally_mutate_next_production_analysis_for_test_v1(),
                _ => MUTATE_BETWEEN.with(|flag| flag.set(true)),
            }
            let error = with_canonical_ranked_policy_checks_v1(
                checked,
                budget,
                |_, _| -> Result<(), Failure> {
                    panic!("mutation must precede callback");
                },
            )
            .unwrap_err();
            assert!(matches!(
                error.failure(),
                Failure::Analysis { .. } | Failure::Mutation
            ));
            assert!(error.observation().work_upper_bound() > 0);
        });
    }
}

#[test]
fn unused_pointer_slice_vector_signatures_are_not_absent_requirements() {
    let vector = fe2o3_kernel_ir::FixedVectorTypeV12::new(
        ScalarType::U32,
        2,
        fe2o3_kernel_ir::VectorLayoutV12::Contiguous,
    );
    for ty in [
        Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
        Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
        Type::Vector(vector),
    ] {
        let mut module = Module::new("unused-carrier");
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::internal_helper(
            "helper",
            Signature::new(vec![ty], vec![]),
            vec![ValueId(0)],
            vec![block],
        ));
        with_checked(&module, |checked, budget| {
            let error =
                with_canonical_ranked_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
            assert!(matches!(
                error.failure(),
                Failure::UnsupportedGraph {
                    function: 0,
                    block: None,
                    ..
                }
            ));
            assert_eq!(error.observation().work_upper_bound(), 0);
        });
    }
}

#[test]
fn ignored_invalid_and_foreign_budget_queries_poison_the_callback() {
    for foreign in [false, true] {
        with_checked(&noop(), |checked, budget| {
            let mut other_work = Work::new(WORK);
            let mut other = Budget::new(&mut other_work, STORAGE);
            let error =
                with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
                    if foreign {
                        assert!(matches!(
                            policies.function_count(&mut other),
                            Err(Failure::Resource(Resource::Accounting))
                        ));
                    } else {
                        assert!(matches!(
                            policies.report(1, budget),
                            Err(Failure::InvalidQuery { function: 1 })
                        ));
                    }
                    Ok(())
                })
                .unwrap_err();
            assert!(matches!(
                error.failure(),
                Failure::Resource(Resource::Accounting) | Failure::InvalidQuery { function: 1 }
            ));
            assert_eq!(other.work(), 0);
        });
    }
}

mod native_loop_tests {
    include!("canonical_ranked_native_loops_v1_tests.rs");
}
