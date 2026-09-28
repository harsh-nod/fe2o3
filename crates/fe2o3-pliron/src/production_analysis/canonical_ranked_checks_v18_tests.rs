use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18 as Inventory, CanonicalRankedMetadataV18 as Metadata,
    build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Constant,
    Function, IntegerSwitchCase, Module, Operation, OperationKind, ScalarType, Signature,
    StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1, Terminator, Type, ValueDef, ValueId,
};

const AMPLE: usize = 1 << 40;
pub(super) const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

std::thread_local! {
    static MUTATE_AND_RESTORE: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn after_function(graph: &crate::KirPlironGraphV18<'_>, _: usize) {
    if MUTATE_AND_RESTORE.with(|flag| flag.replace(false)) {
        graph.test_ranked_mutate_and_restore_v18();
    }
}

pub(super) fn with_checked<T>(
    module: &Module,
    run: impl FnOnce(&mut CheckedCanonicalRankedViewV18<'_, '_, '_, '_>, &mut Budget<'_>) -> T,
) -> T {
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (owner, owned) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LAYOUTS,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(owned.retained_storage()).unwrap();
    let (inventory, inventory_storage) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let metadata = Metadata::new(&owner, &[]);
    let metadata_storage = metadata.storage_extent(&mut budget).unwrap();
    budget.reserve_storage(metadata_storage).unwrap();
    let (candidate, candidate_storage) =
        build_canonical_ranked_candidate_v18(&inventory, &metadata, &mut budget).unwrap();
    budget
        .reserve_storage(candidate_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let result = with_checked_canonical_ranked_view_v18(
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
    budget.release_storage(owned.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    result
}

pub(super) fn pointer_flow(integer_switch: bool) -> Module {
    let pointer = Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(0)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let mut module = Module::new("actual-v18-pointer-control");
    module.storage_layouts.push(StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U64),
    });
    let mut entry = BasicBlock::new(BlockId(17));
    entry.terminator = Some(if integer_switch {
        Terminator::IntegerSwitch {
            selector: ValueId(0),
            cases: vec![
                IntegerSwitchCase {
                    value: Constant::U32(1),
                    target: BlockId(22),
                    arguments: vec![ValueId(1)],
                },
                IntegerSwitchCase {
                    value: Constant::U32(2),
                    target: BlockId(22),
                    arguments: vec![ValueId(2)],
                },
            ],
            default_target: BlockId(22),
            default_arguments: vec![ValueId(1)],
        }
    } else {
        Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(22),
            then_arguments: vec![ValueId(1)],
            else_target: BlockId(22),
            else_arguments: vec![ValueId(2)],
        }
    });
    let mut end = BasicBlock::new(BlockId(22));
    end.parameters
        .push(ValueDef::new(ValueId(3), pointer.clone()));
    end.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let selector = if integer_switch {
        Type::Scalar(ScalarType::U32)
    } else {
        Type::BOOL
    };
    module.functions.push(Function::definition(
        "typed",
        Signature::new(
            vec![selector, pointer.clone(), pointer.clone()],
            vec![pointer],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, end],
    ));
    module
}

#[test]
fn actual_typed_pointer_blocks_duplicate_edges_switch_and_return_reach_fixed_policy() {
    for module in [pointer_flow(false), pointer_flow(true)] {
        with_checked(&module, |checked, budget| {
            let exact = checked.inventory(budget).unwrap().owner();
            let floor = budget.storage();
            with_canonical_ranked_policy_checks_v18(checked, LAYOUTS, budget, |policies, budget| {
                assert!(std::ptr::eq(policies.owner(budget)?, exact));
                assert_eq!(policies.owner(budget)?.module(), &module);
                assert_eq!(policies.report(0, budget)?.unwrap().pass_order(),
                    &super::super::super::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                assert!(policies.report(0, budget)?.unwrap().is_clean());
                assert_eq!(policies.pending_obligations().iter().count(), 19);
                assert!(!policies.ranked_verification_is_complete());
                assert!(!policies.grants_artifact_or_launch_authority());
                Ok(())
            }).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn declarations_are_explicit_and_multiple_functions_share_cumulative_history() {
    let mut module = pointer_flow(false);
    module.functions.insert(
        0,
        Function::declaration("external", Signature::new(vec![], vec![])),
    );
    let mut end = BasicBlock::new(BlockId(71));
    end.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::definition(
        "second",
        Signature::new(vec![], vec![]),
        vec![],
        vec![end],
    ));
    with_checked(&module, |checked, budget| {
        with_canonical_ranked_policy_checks_v18(checked, LAYOUTS, budget, |policies, budget| {
            assert_eq!(policies.function_count(budget)?, 3);
            assert!(policies.report(0, budget)?.is_none());
            assert!(policies.history(0, budget)?.is_none());
            let first = policies.history(1, budget)?.unwrap();
            let second = policies.history(2, budget)?.unwrap();
            assert_eq!(first.function(), 1);
            assert_eq!(first.floor().work_upper_bound(), 0);
            assert_eq!(second.function(), 2);
            assert_eq!(
                second.floor().work_upper_bound(),
                first.invocation().work_upper_bound()
            );
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn declaration_only_module_keeps_absent_reports_and_zero_invocation_accounting() {
    let mut module = pointer_flow(false);
    module.functions[0] = Function::declaration("typed", module.functions[0].signature.clone());
    module.functions.push(Function::declaration(
        "external",
        Signature::new(vec![], vec![]),
    ));
    with_checked(&module, |checked, budget| {
        let floor = budget.storage();
        assert!(floor > 0);
        with_canonical_ranked_policy_checks_v18(checked, LAYOUTS, budget, |policies, budget| {
            assert_eq!(policies.function_count(budget)?, 2);
            for ordinal in 0..2 {
                assert!(policies.report(ordinal, budget)?.is_none());
                assert!(policies.history(ordinal, budget)?.is_none());
            }
            let observation = policies.observation(budget)?;
            assert_eq!(observation.work_upper_bound(), 0);
            assert_eq!(observation.retained_storage_units(), 0);
            assert_eq!(observation.peak_storage_units(), 0);
            assert_eq!(policies.pending_obligations().iter().count(), 19);
            assert!(!policies.ranked_verification_is_complete());
            assert!(!policies.grants_artifact_or_launch_authority());
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn callback_panic_keeps_real_invocation_history_and_restores_the_owner_floor() {
    with_checked(&pointer_flow(false), |checked, budget| {
        let floor = budget.storage();
        let error = with_canonical_ranked_policy_checks_v18(
            checked,
            LAYOUTS,
            budget,
            |policies, budget| -> Result<(), Failure> {
                assert!(policies.report(0, budget)?.unwrap().is_clean());
                panic!("intentional V18 policy callback panic");
            },
        )
        .unwrap_err();
        assert!(matches!(error.failure(), Failure::Panicked));
        assert!(error.observation().work_upper_bound() > 0);
        assert!(error.last_invocation().is_some());
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn rejected_callback_result_is_drained_before_refund_even_when_drop_panics() {
    #[derive(Debug)]
    struct Rejected<'a>(&'a Cell<usize>);
    impl Drop for Rejected<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            resources::cleanup_trace_record("V18 rejected result drop", 0);
            panic!("intentional rejected V18 result destructor panic");
        }
    }
    with_checked(&pointer_flow(false), |checked, budget| {
        let floor = budget.storage();
        let drops = Cell::new(0);
        resources::cleanup_trace_start();
        let error = with_canonical_ranked_policy_checks_v18(
            checked,
            LAYOUTS,
            budget,
            |policies, budget| {
                assert!(matches!(
                    policies.report(usize::MAX, budget),
                    Err(Failure::InvalidQuery {
                        function: usize::MAX
                    })
                ));
                Ok(Rejected(&drops))
            },
        )
        .unwrap_err();
        let trace = resources::cleanup_trace_take();
        assert_eq!(drops.get(), 1);
        assert!(matches!(
            error.failure(),
            Failure::InvalidQuery {
                function: usize::MAX
            }
        ));
        let event = |name| trace.iter().position(|(event, _)| *event == name).unwrap();
        assert!(event("callback discard") < event("V18 rejected result drop"));
        assert!(event("V18 rejected result drop") < event("before refund"));
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn missing_memory_recipe_refuses_before_any_policy_or_callback() {
    let mut module = pointer_flow(false);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![ValueDef::new(
                ValueId(4),
                Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U64),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ));
    with_checked(&module, |checked, budget| {
        let mut called = false;
        let error = with_canonical_ranked_policy_checks_v18(checked, LAYOUTS, budget, |_, _| {
            called = true;
            Ok(())
        })
        .unwrap_err();
        assert!(!called);
        assert_eq!(error.observation().work_upper_bound(), 0);
        assert!(matches!(
            error.failure(),
            Failure::SourceRequirementV18 {
                requirement: CanonicalRankedSourceRequirementV18::Memory,
                ..
            }
        ));
    });
}

#[test]
fn swallowed_invalid_query_stays_first_and_prevents_publication() {
    with_checked(&pointer_flow(false), |checked, budget| {
        let floor = budget.storage();
        let error = with_canonical_ranked_policy_checks_v18(
            checked,
            LAYOUTS,
            budget,
            |policies, budget| {
                assert!(matches!(
                    policies.report(usize::MAX, budget),
                    Err(Failure::InvalidQuery { .. })
                ));
                assert!(matches!(
                    policies.report(0, budget),
                    Err(Failure::InvalidQuery { .. })
                ));
                Ok(())
            },
        )
        .unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::InvalidQuery {
                function: usize::MAX
            }
        ));
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn foreign_ledger_query_cannot_be_swallowed() {
    with_checked(&pointer_flow(false), |checked, budget| {
        let error =
            with_canonical_ranked_policy_checks_v18(checked, LAYOUTS, budget, |policies, _| {
                let mut work = Work::new(AMPLE);
                let mut foreign = Budget::new(&mut work, AMPLE);
                assert!(matches!(
                    policies.owner(&mut foreign),
                    Err(Failure::Resource(Resource::Accounting))
                ));
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::Resource(Resource::Accounting)
        ));
    });
}

fn one_cast(kind: fe2o3_kernel_ir::CastKind, from: Type, to: Type) -> Module {
    let mut block = BasicBlock::new(BlockId(3));
    block.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(1), to.clone())],
        OperationKind::Cast {
            kind,
            value: ValueId(0),
            to: to.clone(),
        },
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    let mut module = Module::new("actual-v18-cast-profile");
    module.functions.push(Function::definition(
        "cast",
        Signature::new(vec![from], vec![to]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

#[test]
fn pointer_and_slice_casts_require_memory_recipes_before_policy_or_callback() {
    use fe2o3_kernel_ir::CastKind;
    let pointer = |space, access| Type::pointer(Type::Scalar(ScalarType::U32), space, access);
    let slice = |space| Type::slice(Type::Scalar(ScalarType::U32), space, AccessMode::ReadWrite);
    for module in [
        one_cast(
            CastKind::RestrictPointerAccess,
            pointer(AddressSpace::Private, AccessMode::ReadWrite),
            pointer(AddressSpace::Private, AccessMode::ReadOnly),
        ),
        one_cast(
            CastKind::PointerToGeneric,
            pointer(AddressSpace::Private, AccessMode::ReadWrite),
            pointer(AddressSpace::Generic, AccessMode::ReadWrite),
        ),
        one_cast(
            CastKind::SliceToGeneric,
            slice(AddressSpace::Global),
            slice(AddressSpace::Generic),
        ),
    ] {
        with_checked(&module, |checked, budget| {
            let mut called = false;
            let error =
                with_canonical_ranked_policy_checks_v18(checked, LAYOUTS, budget, |_, _| {
                    called = true;
                    Ok(())
                })
                .unwrap_err();
            assert!(!called);
            assert_eq!(error.observation().work_upper_bound(), 0);
            assert!(matches!(
                error.failure(),
                Failure::SourceRequirementV18 {
                    requirement: CanonicalRankedSourceRequirementV18::Memory,
                    ..
                }
            ));
        });
    }
}

#[test]
fn pointer_select_is_not_a_scalar_recipe() {
    let mut module = pointer_flow(false);
    let function = &mut module.functions[0];
    let pointer = function.signature.results[0].clone();
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![ValueDef::new(ValueId(4), pointer)],
            OperationKind::Select {
                condition: ValueId(0),
                true_value: ValueId(1),
                false_value: ValueId(2),
            },
        ));
    with_checked(&module, |checked, budget| {
        let error = with_canonical_ranked_policy_checks_v18(
            checked,
            LAYOUTS,
            budget,
            |_, _| -> Result<(), Failure> {
                panic!("pointer select admitted without source recipe")
            },
        )
        .unwrap_err();
        assert_eq!(error.observation().work_upper_bound(), 0);
        assert!(matches!(
            error.failure(),
            Failure::SourceRequirementV18 {
                requirement: CanonicalRankedSourceRequirementV18::Memory,
                ..
            }
        ));
    });
}

#[test]
fn scalar_bitcast_reaches_policy_but_pointer_bitcast_cannot_form_verified_owner() {
    use fe2o3_kernel_ir::CastKind;
    with_checked(
        &one_cast(CastKind::Bitcast, Type::Scalar(ScalarType::U32), Type::F32),
        |checked, budget| {
            with_canonical_ranked_policy_checks_v18(
                checked,
                LAYOUTS,
                budget,
                |policies, budget| {
                    assert!(policies.report(0, budget)?.unwrap().is_clean());
                    Ok(())
                },
            )
            .unwrap();
        },
    );
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    for (from, to) in [
        (pointer.clone(), Type::Scalar(ScalarType::U64)),
        (Type::Scalar(ScalarType::U64), pointer.clone()),
        (pointer.clone(), pointer),
    ] {
        let module = one_cast(CastKind::Bitcast, from, to);
        let mut work = Work::new(AMPLE);
        let mut budget = Budget::new(&mut work, AMPLE);
        assert!(
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module,
                LAYOUTS,
                &mut budget
            )
            .is_err()
        );
    }
}

#[test]
fn actual_v18_native_mutation_and_restore_refuses_after_policy_before_callback() {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            MUTATE_AND_RESTORE.with(|flag| flag.set(false));
        }
    }
    let _reset = Reset;
    with_checked(&pointer_flow(false), |checked, budget| {
        MUTATE_AND_RESTORE.with(|flag| flag.set(true));
        let mut called = false;
        let floor = budget.storage();
        let error = with_canonical_ranked_policy_checks_v18(checked, LAYOUTS, budget, |_, _| {
            called = true;
            Ok(())
        })
        .unwrap_err();
        assert!(!called);
        assert!(!MUTATE_AND_RESTORE.with(Cell::get));
        assert!(matches!(error.failure(), Failure::Mutation));
        assert!(error.observation().work_upper_bound() > 0);
        assert!(error.last_invocation().is_some());
        assert_eq!(budget.storage(), floor);
    });
}

struct BoundaryObservation {
    result: Result<(), Failure>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn run_policy_boundary(
    module: &Module,
    work_limit: usize,
    storage_limit: usize,
) -> BoundaryObservation {
    // Fixture ownership is prepared outside the measured consumer scope. The
    // actual V18 inventory, checked view and native policy use the fresh ledger.
    let mut prepare_work = Work::new(AMPLE);
    let mut prepare = Budget::new(&mut prepare_work, AMPLE);
    let (owner, owner_storage) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LAYOUTS,
            &mut prepare,
        )
        .unwrap();
    let floor = owner_storage.retained_storage() + 37;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = (|| -> Result<(), Failure> {
        let (inventory, storage) =
            Inventory::derive_v18(&owner, &mut budget).map_err(|error| match error {
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => {
                    Failure::Resource(error)
                }
                _ => Failure::ExactGraph,
            })?;
        budget.reserve_storage(storage.retained_storage())?;
        let metadata = Metadata::new(&owner, &[]);
        let metadata_storage = metadata.storage_extent(&mut budget)?;
        budget.reserve_storage(metadata_storage)?;
        let (candidate, candidate_storage) =
            build_canonical_ranked_candidate_v18(&inventory, &metadata, &mut budget)?;
        budget.reserve_storage(candidate_storage.retained_storage())?;
        let prepared_floor = budget.storage();
        let result = with_checked_canonical_ranked_view_v18(
            &inventory,
            &metadata,
            &candidate,
            &mut budget,
            |checked, budget| {
                with_canonical_ranked_policy_checks_v18(
                    checked,
                    LAYOUTS,
                    budget,
                    |policies, budget| {
                        assert_eq!(policies.function_count(budget)?, 1);
                        assert!(policies.report(0, budget)?.is_some());
                        Ok(())
                    },
                )
                .map_err(|error| error.failure)
            },
        );
        assert_eq!(budget.storage(), prepared_floor);
        drop(candidate);
        drop(metadata);
        drop(inventory);
        result
    })();
    // All temporary owners have already been destroyed on success and refusal.
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
    let accepted = budget.work();
    let peak = budget.peak_storage();
    let failed_storage = budget.failed_storage();
    drop(budget);
    BoundaryObservation {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

#[test]
fn actual_v18_policy_exact_and_one_short_work_storage_boundaries() {
    let module = pointer_flow(true);
    let complete = run_policy_boundary(&module, AMPLE, AMPLE);
    complete.result.unwrap();
    assert!(complete.work > 0 && complete.peak > 0);
    let exact = run_policy_boundary(&module, complete.work, complete.peak);
    exact.result.unwrap();
    assert_eq!((exact.work, exact.peak), (complete.work, complete.peak));
    assert_eq!((exact.failed_work, exact.failed_storage), (None, None));
    // Measured replay boundaries, not an independent algebraic prefix oracle.
    let work_short = run_policy_boundary(&module, complete.work - 1, complete.peak);
    assert!(work_short.result.is_err());
    assert!(work_short.failed_work.is_some());
    assert_eq!(work_short.failed_storage, None);
    let storage_short = run_policy_boundary(&module, complete.work, complete.peak - 1);
    assert!(storage_short.result.is_err());
    assert!(storage_short.failed_storage.is_some());
    assert_eq!(storage_short.failed_work, None);
}
