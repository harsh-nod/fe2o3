use super::*;
use super::super::super::tests::LAYOUTS;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18 as Inventory, CanonicalRankedMetadataV18 as Metadata,
    CanonicalKirPrivateMemoryLimitsV1, CanonicalKirPrivateMemoryErrorV1,
    build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    check_canonical_kir_private_memory_v18,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    ExecutionOperationV15 as E, ExecutionRoleV15 as Role, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1, StorageOperationV1 as Storage,
    Terminator, Type, ValueDef, ValueId,
};
const AMPLE: usize = 1 << 40;
const PRIVATE_LIMITS: CanonicalKirPrivateMemoryLimitsV1 = CanonicalKirPrivateMemoryLimitsV1 { max_cells: 64 };

fn module(scalar: ScalarType, bytes: u64, alignment: u32, lifecycle: bool) -> Module {
    let mut m = Module::new("native-private-typed");
    m.storage_layouts.push(StorageLayoutV1 { size: bytes, alignment, kind: StorageLayoutKindV1::Scalar(scalar) });
    let mut block = BasicBlock::new(BlockId(17));
    block.operations = vec![
        Operation::new(vec![ValueDef::new(ValueId(10),
            Type::pointer(Type::StorageObject(StorageLayoutIdV1(0)), AddressSpace::Private, AccessMode::ReadWrite))],
            OperationKind::Alloca { element: Type::StorageObject(StorageLayoutIdV1(0)), count: None,
                address_space: AddressSpace::Private, alignment }),
        Operation::new(vec![], OperationKind::Storage(Storage::WriteValue {
            address: ValueId(10), value: ValueId(20), access: MemoryAccess::new(AddressSpace::Private, alignment) })),
        Operation::new(vec![ValueDef::new(ValueId(40), Type::Scalar(scalar))],
            OperationKind::Storage(Storage::ReadValue { address: ValueId(10), access: MemoryAccess::new(AddressSpace::Private, alignment) })),
    ];
    if lifecycle {
        block.operations.extend([
            Operation::new(vec![ValueDef::new(ValueId(50), Type::Execution(Role::Context))],
                OperationKind::Execution(E::ContextIssue)),
            Operation::new(vec![ValueDef::new(ValueId(51), Type::Execution(Role::Workgroup))],
                OperationKind::Execution(E::WorkgroupDerive { context: ValueId(50) })),
            Operation::new(vec![], OperationKind::Execution(E::ScopeEnd { workgroup: ValueId(51), discarded: vec![] })),
        ]);
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    m.functions.push(Function::kernel_entry("entry", Signature::new(vec![Type::Scalar(scalar)], vec![]), vec![ValueId(20)], vec![block]));
    m.kernels.push(Kernel::new("entry", "entry", LaunchDomain::D1 { x: LaunchExtent::Static(64) }));
    m
}

fn with_physical<T>(
    m: &Module,
    run: impl FnOnce(&mut CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
        &CheckedCanonicalKirPrivateMemoryV18<'_, '_>, &mut Budget<'_>) -> T,
) -> T {
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (owner, owner_storage) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        m, LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(owner_storage.retained_storage()).unwrap();
    let (inventory, inventory_storage) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(inventory_storage.retained_storage()).unwrap();
    let metadata = Metadata::new(&owner, &[]);
    let metadata_storage = metadata.storage_extent(&mut budget).unwrap();
    budget.reserve_storage(metadata_storage).unwrap();
    let (candidate, candidate_storage) = build_canonical_ranked_candidate_v18(&inventory, &metadata, &mut budget).unwrap();
    budget.reserve_storage(candidate_storage.retained_storage()).unwrap();
    let (physical, physical_storage) = check_canonical_kir_private_memory_v18(&inventory, PRIVATE_LIMITS, &mut budget).unwrap();
    budget.reserve_storage(physical_storage.retained_storage()).unwrap();
    // The checked view's queries require its exact retained floor. Construct
    // the same-owner physical proof before opening that view, as measure does.
    let floor = budget.storage();
    let result = with_checked_canonical_ranked_view_v18(&inventory, &metadata, &candidate, &mut budget,
        |checked, budget| Ok::<_, CanonicalRankedViewErrorV1>(run(checked, &physical, budget))).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(physical);
    budget.release_storage(physical_storage.retained_storage()).unwrap();
    drop(candidate);
    budget.release_storage(candidate_storage.retained_storage()).unwrap();
    drop(metadata);
    budget.release_storage(metadata_storage).unwrap();
    drop(inventory);
    budget.release_storage(inventory_storage.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(owner_storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    result
}

#[test]
fn private_native_v18_physical_preparation_precedes_checked_view_exact_floor() {
    let complete = Cell::new(false);
    with_physical(&module(ScalarType::U32, 4, 4, false), |checked, physical, budget| {
        let floor = budget.storage();
        assert!(std::ptr::eq(checked.inventory(budget).unwrap().owner(), physical.inventory().owner()));
        assert!(physical.operation(0) && physical.operation(1) && physical.operation(2));
        with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
            assert!(std::ptr::eq(pending.owner(budget)?, physical.inventory().owner()));
            assert_eq!(pending.obligations(budget)?.len(), 3);
            complete.set(true);
            Ok(())
        }).unwrap();
        assert_eq!(budget.storage(), floor);
    });
    assert!(complete.get());
}

#[test]
fn private_native_v18_all_scalar_layouts_pair_nine_stages_without_source_authority() {
    for scalar in [ScalarType::Bool, ScalarType::I8, ScalarType::U16, ScalarType::I32,
        ScalarType::U32, ScalarType::I64, ScalarType::U64, ScalarType::I128, ScalarType::U128,
        ScalarType::F16, ScalarType::Bf16, ScalarType::F32, ScalarType::F64]
    {
        let bytes = u64::from(scalar.bit_width().unwrap().div_ceil(8));
        for alignment in [1, bytes as u32] {
            with_physical(&module(scalar, bytes, alignment, true), |checked, physical, budget| {
                let owner = physical.inventory().owner();
                let floor = budget.storage();
                let complete = Cell::new(false);
                with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
                    pending.with_private_memory_observations_v18(physical, budget, |view, budget| {
                        // This crate-root type annotation proves the positive public reexport.
                        let view: &crate::PendingCanonicalPrivateMemoryPoliciesV18<'_, '_> = view;
                        assert!(std::ptr::eq(view.owner(budget)?, owner));
                        assert_eq!(view.function_count(budget)?, 1);
                        assert_eq!(view.paired_stage_count(0, budget)?, Some(9));
                        let report = view.report(0, budget)?.unwrap();
                        assert!(report.is_clean());
                        assert_eq!(report.pass_order(), &crate::production_analysis::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                        assert_eq!(view.obligations(budget)?.len(), 6);
                        assert_eq!(view.obligations(budget)?.iter().filter(|row|
                            row.requirement() == CanonicalRankedSourceRequirementV18::Memory).count(), 3);
                        assert!(!view.source_roles_are_complete());
                        assert!(!view.ranked_verification_is_complete());
                        assert!(!view.grants_artifact_or_launch_authority());
                        assert!(view.history(0, budget)?.is_some());
                        let observed = view.observation(budget)?;
                        assert!(observed.work_upper_bound() > 0 && observed.retained_storage_units() > 0);
                        complete.set(true);
                        Ok(())
                    }).unwrap();
                    Ok(())
                }).unwrap();
                assert!(complete.get());
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn private_native_v18_keeps_ordinary_lifecycle_and_strict_source_gates_closed() {
    with_physical(&module(ScalarType::U32, 4, 4, true), |checked, physical, budget| {
        with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
            pending.with_private_memory_observations_v18(physical, budget, |view, budget| {
                assert_eq!(view.paired_stage_count(0, budget)?, Some(9)); Ok(())
            }).unwrap();
            let old = pending.with_native_observations(budget, |_, _| -> Result<(), Failure> {
                panic!("lifecycle-only path admitted private memory")
            }).unwrap_err();
            assert!(matches!(old.failure(), Failure::Analysis { function: 0, .. }));
            Ok(())
        }).unwrap();
        let strict = with_canonical_ranked_policy_checks_v18(checked, LAYOUTS, budget,
            |_, _| -> Result<(), Failure> { panic!("physical observation completed source roles") }).unwrap_err();
        assert!(matches!(strict.failure(), Failure::SourceRequirementV18 {
            requirement: CanonicalRankedSourceRequirementV18::Memory, coordinate,
        } if coordinate.operation == 0));
    });
}

#[test]
fn private_native_v18_preserves_selected_error_panic_and_exact_callback_floor() {
    for mode in 0..4 {
        with_physical(&module(ScalarType::U32, 4, 4, true), |checked, physical, budget| {
            let complete = Cell::new(false);
            let outer = with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
                let floor = budget.storage();
                let error = pending.with_private_memory_observations_v18(physical, budget, |view, budget| -> Result<(), Failure> {
                    assert_eq!(view.paired_stage_count(0, budget)?, Some(9));
                    if mode >= 2 { budget.release_storage(1)?; }
                    if mode % 2 == 1 { std::panic::panic_any("native private selected panic"); }
                    Err(Failure::Callback("native private selected error"))
                }).unwrap_err();
                match mode {
                    0 => assert!(matches!(error.failure(), Failure::Callback("native private selected error"))),
                    1 => assert!(matches!(error.failure(), Failure::Panicked)),
                    _ => assert!(matches!(error.failure(), Failure::Resource(Resource::Accounting))),
                }
                assert!(error.last_invocation().is_some());
                assert_eq!(budget.storage(), floor);
                complete.set(true);
                Ok(())
            });
            assert!(complete.get());
            if mode < 2 { outer.unwrap(); }
            else { assert!(matches!(outer, Err(Failure::Resource(Resource::Accounting)))); }
        });
    }
}

#[test]
fn private_native_v18_foreign_ledger_and_swallowed_invalid_report_remain_sticky() {
    for foreign in [false, true] {
        with_physical(&module(ScalarType::U32, 4, 4, false), |checked, physical, budget| {
            let complete = Cell::new(false);
            let outer = with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
                let error = pending.with_private_memory_observations_v18(physical, budget, |view, budget| {
                    assert!(view.report(0, budget)?.is_some());
                    if foreign {
                        let mut work = Work::new(AMPLE);
                        let mut other = Budget::new(&mut work, AMPLE);
                        assert!(matches!(view.owner(&mut other), Err(Failure::Resource(Resource::Accounting))));
                        assert_eq!((other.work(), other.storage()), (0, 0));
                    } else { assert!(matches!(view.report(1, budget), Err(Failure::InvalidQuery { function: 1 }))); }
                    let stopped = (budget.work(), budget.storage());
                    let replay = view.report(0, budget).unwrap_err();
                    if foreign { assert!(matches!(replay, Failure::Resource(Resource::Accounting))); }
                    else { assert!(matches!(replay, Failure::InvalidQuery { function: 1 })); }
                    assert_eq!((budget.work(), budget.storage()), stopped);
                    complete.set(true);
                    Ok(())
                }).unwrap_err();
                if foreign { assert!(matches!(error.failure(), Failure::Resource(Resource::Accounting))); }
                else { assert!(matches!(error.failure(), Failure::InvalidQuery { function: 1 })); }
                Ok(())
            });
            assert!(complete.get());
            if foreign { assert!(matches!(outer, Err(Failure::Resource(Resource::Accounting)))); }
            else { outer.unwrap(); }
        });
    }
}

fn physical_error(error: CanonicalKirPrivateMemoryErrorV1) -> Failure {
    match error { CanonicalKirPrivateMemoryErrorV1::Resource(error) => Failure::Resource(error), _ => Failure::ExactGraph }
}

struct Measured {
    result: Result<(), Failure>, work: usize, peak: usize,
    failed_work: Option<usize>, failed_storage: Option<usize>, completed: bool,
}
fn measure(m: &Module, work_limit: usize, storage_limit: usize, native_limits: Limits) -> Measured {
    let mut prepare_work = Work::new(AMPLE);
    let mut prepare = Budget::new(&mut prepare_work, AMPLE);
    let (owner, owner_storage) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(m, LAYOUTS, &mut prepare).unwrap();
    let floor = owner_storage.retained_storage() + 37;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let mut entered = false;
    let result = (|| -> Result<(), Failure> {
        let (inventory, stored) = Inventory::derive_v18(&owner, &mut budget).map_err(|e| match e {
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(e) => Failure::Resource(e), _ => Failure::ExactGraph })?;
        budget.reserve_storage(stored.retained_storage())?;
        let metadata = Metadata::new(&owner, &[]);
        let metadata_storage = metadata.storage_extent(&mut budget)?;
        budget.reserve_storage(metadata_storage)?;
        let (candidate, stored) = build_canonical_ranked_candidate_v18(&inventory, &metadata, &mut budget)?;
        budget.reserve_storage(stored.retained_storage())?;
        let (physical, stored) = check_canonical_kir_private_memory_v18(&inventory, PRIVATE_LIMITS, &mut budget).map_err(physical_error)?;
        budget.reserve_storage(stored.retained_storage())?;
        let prepared_floor = budget.storage();
        let result = with_checked_canonical_ranked_view_v18(&inventory, &metadata, &candidate, &mut budget, |checked, budget| {
            with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
                pending.with_private_memory_observations_with_limits_v18(&physical, native_limits, budget, |view, budget| {
                    assert_eq!(view.paired_stage_count(0, budget)?, Some(9));
                    assert!(view.report(0, budget)?.unwrap().is_clean());
                    entered = true;
                    Ok(())
                }).map_err(|error| error.failure)
            })
        });
        assert_eq!(budget.storage(), prepared_floor);
        drop(physical); drop(candidate); drop(metadata); drop(inventory);
        result
    })();
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
    let accepted = budget.work();
    let peak = budget.peak_storage();
    let failed_storage = budget.failed_storage();
    drop(budget);
    let completed = result.is_ok() && entered;
    Measured { result, work: accepted, peak, failed_work: work.failed_work(), failed_storage, completed }
}

#[test]
fn private_native_v18_whole_measured_work_storage_exact_and_one_short() {
    let m = module(ScalarType::U32, 4, 4, true);
    let full = measure(&m, AMPLE, AMPLE, Limits::production_hard_ceiling());
    full.result.unwrap();
    assert!(full.completed);
    let exact = measure(&m, full.work, full.peak, Limits::production_hard_ceiling());
    exact.result.unwrap();
    assert!(exact.completed);
    assert_eq!((exact.work, exact.peak), (full.work, full.peak));
    for (work, storage, is_work) in [(full.work - 1, full.peak, true), (full.work, full.peak - 1, false)] {
        let short = measure(&m, work, storage, Limits::production_hard_ceiling());
        assert!(!short.completed);
        match short.result {
            Err(Failure::Resource(Resource::Work(e)))
            | Err(Failure::View(CanonicalRankedViewErrorV1::Resource(Resource::Work(e)))) if is_work => {
                assert_eq!(e.limit(), work); assert!(e.actual() > work);
                assert_eq!(short.failed_work, Some(e.actual()));
            }
            Err(Failure::StorageBridge(crate::KirBridgeErrorV18::Canonical(
                fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Layout(
                    fe2o3_kernel_ir::StorageLayoutErrorV1::Resource(Resource::Storage(e)))))) if !is_work => {
                // The exact measured peak is now owned by canonical layout
                // admission inside native import, not the outer view wrapper.
                assert_eq!(e.limit(), storage);
                assert_eq!(e.actual(), full.peak);
                assert_eq!(e.actual(), storage + 1);
                assert_eq!(short.failed_storage, Some(full.peak));
                assert_eq!(short.failed_work, None);
            }
            other => panic!("wrong resource boundary: {other:?}"),
        }
    }
}

#[test]
fn private_native_v18_rejects_equal_content_foreign_physical_owner() {
    let m = module(ScalarType::U32, 4, 4, false);
    with_physical(&m, |checked, physical, budget| {
        with_physical(&m, |_, foreign, _| {
            assert_eq!(physical.inventory().owner().module(), foreign.inventory().owner().module());
            assert!(!std::ptr::eq(physical.inventory().owner(), foreign.inventory().owner()));
            with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
                let stopped = budget.storage();
                let wrong = pending.with_private_memory_observations_v18(foreign, budget,
                    |_, _| -> Result<(), Failure> { panic!("foreign proof owner") }).unwrap_err();
                assert!(matches!(wrong.failure(), Failure::ExactGraph));
                assert!(wrong.last_invocation().is_none());
                assert_eq!(budget.storage(), stopped);
                pending.with_private_memory_observations_v18(physical, budget, |view, budget| {
                    assert_eq!(view.paired_stage_count(0, budget)?, Some(9)); Ok(())
                }).unwrap();
                Ok(())
            }).unwrap();
        });
    });
}

#[test]
fn private_native_v18_analysis_denial_retains_exact_history_and_no_callback() {
    with_physical(&module(ScalarType::U32, 4, 4, false), |checked, physical, budget| {
        with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
            let observation = pending.with_private_memory_observations_v18(physical, budget,
                |view, budget| view.observation(budget)).unwrap();
            for limits in [
                Limits::new(observation.work_upper_bound() - 1, observation.peak_storage_units()),
                Limits::new(observation.work_upper_bound(), observation.peak_storage_units() - 1),
            ] {
                let error = pending.with_private_memory_observations_with_limits_v18(physical, limits, budget,
                    |_, _| -> Result<(), Failure> { panic!("one-short analysis completed") }).unwrap_err();
                assert!(error.observation().first_denial().is_some());
                let history = error.last_invocation().expect("attempt history must survive denial");
                assert_eq!(history.function(), 0);
                assert!(history.invocation().first_denial().is_some());
            }
            pending.with_private_memory_observations_with_limits_v18(physical,
                Limits::new(observation.work_upper_bound(), observation.peak_storage_units()), budget,
                |view, budget| { assert_eq!(view.paired_stage_count(0, budget)?, Some(9)); Ok(()) }).unwrap();
            Ok(())
        }).unwrap();
    });
}

#[test]
fn private_native_v18_actual_coordinate_origin_schema_and_epoch_faults_refuse() {
    for fault in 0..6 {
        with_physical(&module(ScalarType::U32, 4, 4, false), |checked, physical, budget| {
            let reached = Cell::new(false);
            let result = with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
                // Genuine complete coverage must precede every native mutant.
                pending.with_private_memory_observations_v18(physical, budget, |view, budget| {
                    assert_eq!(view.paired_stage_count(0, budget)?, Some(9)); Ok(())
                }).unwrap();
                let error = pending.graph.test_private_memory_fault_v18(physical, pending.epoch, fault, budget).unwrap_err();
                if fault == 5 { assert!(matches!(error, Failure::Mutation)); }
                else { assert!(matches!(error, Failure::ExactGraph | Failure::NativeSchema)); }
                reached.set(true);
                Ok(())
            });
            assert!(reached.get());
            if fault == 5 { assert!(matches!(result, Err(Failure::Mutation))); }
            else { result.unwrap(); }
        });
    }
}

#[test]
fn private_native_v18_header_one_short_is_first_and_sticky_without_reports() {
    with_physical(&module(ScalarType::U32, 4, 4, false), |checked, physical, budget| {
        let complete = Cell::new(false);
        let outer = with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
            pending.with_private_memory_observations_v18(physical, budget, |view, budget| {
                assert_eq!(view.paired_stage_count(0, budget)?, Some(9)); Ok(())
            }).unwrap();
            let floor = budget.storage();
            let header = pending_private_headers_v18::<()>(0, 1)?;
            assert_eq!(pending_private_headers_v18::<()>(4096, 8)? - header, 4096 + 14);
            let padding = budget.storage_limit() - floor - (header - 1);
            budget.reserve_storage(padding)?;
            let held = budget.storage();
            let error = pending.with_private_memory_observations_v18(physical, budget,
                |_, _| -> Result<(), Failure> { panic!("short headers published reports") }).unwrap_err();
            let Failure::Resource(Resource::Storage(limit)) = error.failure() else { panic!("{error:?}"); };
            assert_eq!((limit.actual(), limit.limit()), (budget.storage_limit() + 1, budget.storage_limit()));
            assert!(error.last_invocation().is_none());
            assert_eq!(budget.storage(), held);
            let stopped = budget.work();
            assert!(matches!(pending.owner(budget), Err(Failure::Resource(Resource::Storage(replayed))) if replayed == *limit));
            assert_eq!((budget.work(), budget.storage()), (stopped, held));
            budget.release_storage(padding)?;
            assert_eq!(budget.storage(), floor);
            complete.set(true);
            Ok(())
        });
        assert!(complete.get());
        assert!(matches!(outer, Err(Failure::Resource(Resource::Storage(_)))));
    });
}

#[test]
fn private_native_v18_import_census_has_independent_work_and_fixed_peak() {
    let mut previous = 0;
    let mut fixed = None;
    for extra in [0, 1, 8, 16] {
        let mut m = module(ScalarType::U32, 4, 4, false);
        for i in 0..extra {
            m.functions[0].body.as_mut().unwrap().blocks[0].operations.push(Operation::new(
                vec![ValueDef::new(ValueId(100 + i), Type::Scalar(ScalarType::U32))],
                OperationKind::Storage(Storage::ReadValue {
                    address: ValueId(10), access: MemoryAccess::new(AddressSpace::Private, 4),
                })));
        }
        with_physical(&m, |checked, physical, budget| {
            with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
                let (work, headers) = pending.graph.test_private_memory_census_v18(physical, pending.epoch, budget)?;
                assert!(work > previous);
                previous = work;
                if let Some(before) = fixed { assert_eq!(headers, before); }
                else { fixed = Some(headers); }
                pending.with_private_memory_observations_v18(physical, budget, |view, budget| {
                    assert_eq!(view.paired_stage_count(0, budget)?, Some(9)); Ok(())
                }).unwrap();
                Ok(())
            }).unwrap();
        });
    }
}

#[test]
fn private_native_v18_exact_cfg_and_unreachable_keep_store_intersection() {
    for terminal in [false, true] {
        let mut m = module(ScalarType::U32, 4, 4, false);
        if terminal {
            m.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Unreachable);
        } else {
            m.functions[0].signature.parameters.push(Type::BOOL);
            let body = m.functions[0].body.as_mut().unwrap();
            body.parameters.push(ValueId(900));
            let read = body.blocks[0].operations.pop().unwrap();
            body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(900), then_target: BlockId(18), then_arguments: vec![],
                else_target: BlockId(19), else_arguments: vec![],
            });
            for id in [18, 19] {
                let mut block = BasicBlock::new(BlockId(id));
                block.terminator = Some(Terminator::Branch { target: BlockId(20), arguments: vec![] });
                body.blocks.push(block);
            }
            let mut join = BasicBlock::new(BlockId(20));
            join.operations.push(read);
            join.terminator = Some(Terminator::Return { values: vec![] });
            body.blocks.push(join);
        }
        with_physical(&m, |checked, physical, budget| {
            assert_eq!(physical.latest_stores()[2], Some(1));
            with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
                pending.with_private_memory_observations_v18(physical, budget, |view, budget| {
                    assert_eq!(view.paired_stage_count(0, budget)?, Some(9));
                    assert!(view.report(0, budget)?.unwrap().is_clean());
                    Ok(())
                }).unwrap();
                Ok(())
            }).unwrap();
        });
    }
}

#[test]
fn private_native_v18_sparse_reordered_block_ids_are_not_coordinate_ordinals() {
    let mut m = module(ScalarType::U32, 4, 4, false);
    m.functions[0].signature.parameters.push(Type::BOOL);
    let body = m.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(900));
    let read = body.blocks[0].operations.pop().unwrap();
    assert_eq!(body.blocks[0].id, BlockId(17));
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(900), then_target: BlockId(5), then_arguments: vec![],
        else_target: BlockId(903), else_arguments: vec![],
    });
    // Declaration order, numerical ID order and branch successor order all differ.
    for id in [903, 5] {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(Terminator::Branch { target: BlockId(61), arguments: vec![] });
        body.blocks.push(block);
    }
    let mut join = BasicBlock::new(BlockId(61));
    join.operations.push(read);
    join.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(join);
    let completed = Cell::new(false);
    with_physical(&m, |checked, physical, budget| {
        let blocks = physical.inventory().blocks();
        assert_eq!(blocks.len(), 4);
        for (ordinal, id) in [17, 903, 5, 61].into_iter().enumerate() {
            assert_eq!(blocks[ordinal].coordinate.block as usize, ordinal);
            assert_eq!(blocks[ordinal].block.id, BlockId(id));
            assert_ne!(blocks[ordinal].block.id, BlockId(ordinal as u32));
        }
        assert_eq!(physical.latest_stores()[2], Some(1));
        with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
            pending.with_private_memory_observations_v18(physical, budget, |view, budget| {
                assert_eq!(view.paired_stage_count(0, budget)?, Some(9));
                assert!(view.report(0, budget)?.unwrap().is_clean());
                assert!(view.history(0, budget)?.is_some());
                assert!(!view.source_roles_are_complete());
                completed.set(true);
                Ok(())
            }).unwrap();
            Ok(())
        }).unwrap();
    });
    assert!(completed.get());
}

#[test]
fn private_native_v18_return_and_condbranch_require_exact_terminator_coordinates() {
    for conditional in [false, true] {
        let mut m = module(ScalarType::U32, 4, 4, false);
        if conditional {
            m.functions[0].signature.parameters.push(Type::BOOL);
            let body = m.functions[0].body.as_mut().unwrap();
            body.parameters.push(ValueId(900));
            body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(900), then_target: BlockId(5), then_arguments: vec![],
                else_target: BlockId(903), else_arguments: vec![],
            });
            for id in [903, 5] {
                let mut block = BasicBlock::new(BlockId(id));
                block.terminator = Some(Terminator::Return { values: vec![] });
                body.blocks.push(block);
            }
        }
        for fault in 0..4 {
            let completed = Cell::new(false);
            with_physical(&m, |checked, physical, budget| {
                with_pending_canonical_ranked_source_roles_v18(checked, LAYOUTS, budget, |pending, budget| {
                    for before in [true, false] {
                        if !before {
                            let error = pending.graph.test_private_terminator_coordinate_fault_v18(
                                physical, pending.epoch, fault, budget).unwrap_err();
                            assert!(matches!(error, Failure::NativeSchema));
                        }
                        pending.with_private_memory_observations_v18(physical, budget, |view, budget| {
                            assert_eq!(view.paired_stage_count(0, budget)?, Some(9));
                            assert!(view.report(0, budget)?.unwrap().is_clean());
                            assert!(view.history(0, budget)?.is_some());
                            assert!(!view.source_roles_are_complete());
                            Ok(())
                        }).unwrap();
                    }
                    completed.set(true);
                    Ok(())
                }).unwrap();
            });
            assert!(completed.get());
        }
    }
}
