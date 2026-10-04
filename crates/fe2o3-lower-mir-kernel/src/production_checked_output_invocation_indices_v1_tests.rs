use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, LaunchDomain,
    LaunchExtent, Module, Operation, Signature, Terminator, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticLayoutIdentityV1, SemanticTargetDataLayoutV1};

fn module(kind: IndexKind, axis: Axis, roots: usize) -> Module {
    let mut module = Module::new("launch-scalar-census");
    for ordinal in 0..roots {
        let name = format!("root_{ordinal}");
        let mut block = BasicBlock::new(BlockId(97));
        block.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(91), Type::INDEX)],
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex { kind, axis },
                Type::INDEX,
            )),
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            name.clone(),
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            name.clone(),
            name,
            LaunchDomain::D3 {
                x: LaunchExtent::Static(128),
                y: LaunchExtent::Static(2),
                z: LaunchExtent::Static(3),
            },
        ));
    }
    module
}

fn with_inventory(
    module: &Module,
    use_inventory: impl FnOnce(&CanonicalKirInventoryV1<'_>, &mut AssertOriginBudgetV1<'_>),
) {
    let mut work = Work::new(usize::MAX);
    let mut budget = AssertOriginBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    let (owner, owner_storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    use_inventory(&inventory, &mut budget);
    budget.release_storage(budget.storage() - floor).unwrap();
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn invocation_index_census_exact_local_and_workgroup_x_keep_all_root_sites() {
    for kind in [IndexKind::Local, IndexKind::Workgroup] {
        for roots in [1, 2, 8] {
            with_inventory(&module(kind, Axis::X, roots), |inventory, budget| {
                let floor = budget.storage();
                let work = budget.work();
                for (ordinal, row) in inventory.operations().iter().enumerate() {
                    assert!(native(inventory, ordinal, row, budget).unwrap());
                    assert_eq!(budget.storage(), floor);
                }
                assert_eq!(budget.work() - work, roots * (64 + 2 * roots));
                let private = private_memory::check(inventory, 1024, budget).unwrap();
                let division = unsigned_division::check(
                    inventory,
                    SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(
                        [250; 32],
                    )),
                    budget,
                )
                .unwrap();
                let helpers = scalar_helpers::check(inventory, budget).unwrap();
                census::native(
                    inventory,
                    &private,
                    &division,
                    &helpers,
                    "launch scalar test",
                    |_, _| Ok(false),
                    budget,
                )
                .unwrap();
            });
        }
    }
}

#[test]
fn invocation_index_census_foreign_copied_or_wrong_site_is_rejected_without_debit() {
    let graph = module(IndexKind::Local, Axis::X, 2);
    with_inventory(&graph, |inventory, budget| {
        with_inventory(&graph, |foreign, _| {
            let row = &inventory.operations()[0];
            let copied = CanonicalKirOperationRefV1 {
                coordinate: row.coordinate,
                operation: row.operation,
                results: row.results.clone(),
                operands: row.operands.clone(),
                effects: row.effects.clone(),
            };
            let floor = (budget.work(), budget.storage());
            for (ordinal, row) in [
                (0, &foreign.operations()[0]),
                (0, &copied),
                (0, &inventory.operations()[1]),
                (usize::MAX, row),
            ] {
                assert!(matches!(
                    native(inventory, ordinal, row, budget),
                    Err(E::Unsupported {
                        phase: "launch scalar",
                        detail: "same borrowed operation site",
                    })
                ));
                assert_eq!((budget.work(), budget.storage()), floor);
            }
            assert!(native(inventory, 0, row, budget).unwrap());
        });
    });
}

#[test]
fn invocation_index_census_axes_and_other_hierarchy_kinds_do_not_gain_permission() {
    for (kind, axis) in [
        (IndexKind::Local, Axis::Y),
        (IndexKind::Local, Axis::Z),
        (IndexKind::Workgroup, Axis::Y),
        (IndexKind::Workgroup, Axis::Z),
        (IndexKind::WorkgroupSize, Axis::X),
        (IndexKind::WorkgroupCount, Axis::X),
        (IndexKind::Global, Axis::Y),
        (IndexKind::Global, Axis::Z),
    ] {
        with_inventory(&module(kind, axis, 1), |inventory, budget| {
            let before = (budget.work(), budget.storage());
            assert!(!native(inventory, 0, &inventory.operations()[0], budget).unwrap());
            assert_eq!((budget.work(), budget.storage()), before);
            let private = private_memory::check(inventory, 1024, budget).unwrap();
            let division = unsigned_division::check(
                inventory,
                SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(
                    [250; 32],
                )),
                budget,
            )
            .unwrap();
            let helpers = scalar_helpers::check(inventory, budget).unwrap();
            assert!(matches!(
                census::native(
                    inventory,
                    &private,
                    &division,
                    &helpers,
                    "launch scalar test",
                    |_, _| Ok(false),
                    budget
                ),
                Err(E::UnsupportedOperation {
                    phase: "launch scalar test",
                    detail: "closed opcode census",
                    ..
                })
            ));
        });
    }
}

#[test]
fn invocation_index_census_result_schema_still_requires_verified_canonical_admission() {
    for fault in 0..5 {
        let mut graph = module(IndexKind::Local, Axis::X, 1);
        let operation = &mut graph.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
        match fault {
            0 => operation.results[0].ty = Type::BOOL,
            1 => {
                if let OperationKind::Intrinsic(intrinsic) = &mut operation.kind {
                    intrinsic.result_type = Type::BOOL;
                }
            }
            2 => operation
                .results
                .push(ValueDef::new(ValueId(92), Type::INDEX)),
            3 => operation.results[0].ty = Type::Scalar(fe2o3_kernel_ir::ScalarType::I64),
            4 => operation.results.clear(),
            _ => unreachable!(),
        }
        let mut work = Work::new(usize::MAX);
        let mut budget = AssertOriginBudgetV1::new(&mut work, usize::MAX);
        assert!(Owner::from_module_ref_with_verification_budget_v12(&graph, &mut budget).is_err());
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn invocation_index_census_helper_role_and_copied_effect_ranges_gain_no_permission() {
    let mut graph = module(IndexKind::Local, Axis::X, 1);
    let mut helper = graph.functions[0].clone();
    helper.id = "helper".into();
    helper.role = FunctionRole::InternalHelper;
    graph.functions.push(helper);
    with_inventory(&graph, |inventory, budget| {
        let row = &inventory.operations()[1];
        let floor = budget.storage();
        assert!(!native(inventory, 1, row, budget).unwrap());
        assert_eq!(budget.storage(), floor);
        let original = &inventory.operations()[0];
        assert!(original.effects.is_empty());
        assert!(original.compiler_ordering().is_empty());
        // Effects and ordering derive from the verified operation, not a caller
        // annotation. A copied row cannot attach invented ranges to that site.
        for (results, operands, effects) in [
            (original.results.clone(), original.operands.clone(), 0..1),
            (original.results.clone(), 0..1, original.effects.clone()),
            (0..0, original.operands.clone(), original.effects.clone()),
        ] {
            let changed = CanonicalKirOperationRefV1 {
                coordinate: original.coordinate,
                operation: original.operation,
                results,
                operands,
                effects,
            };
            let before = (budget.work(), budget.storage());
            assert!(matches!(
                native(inventory, 0, &changed, budget),
                Err(E::Unsupported {
                    phase: "launch scalar",
                    detail: "same borrowed operation site",
                })
            ));
            assert_eq!((budget.work(), budget.storage()), before);
        }
        let physical = Operation::new(
            vec![],
            OperationKind::Barrier(fe2o3_kernel_ir::Barrier {
                execution_scope: fe2o3_kernel_ir::SynchronizationScope::Workgroup,
                memory_scope: fe2o3_kernel_ir::SynchronizationScope::Workgroup,
                semantics: fe2o3_kernel_ir::BarrierSemantics::new(
                    fe2o3_kernel_ir::MemoryOrdering::AcquireRelease,
                    [fe2o3_kernel_ir::AddressSpace::Workgroup],
                ),
            }),
        );
        let ordered = Operation::new(
            vec![ValueDef::new(
                ValueId(91),
                Type::Execution(fe2o3_kernel_ir::ExecutionRoleV15::Context),
            )],
            OperationKind::Execution(fe2o3_kernel_ir::ExecutionOperationV15::ContextIssue),
        );
        assert!(!physical.memory_effects().is_empty());
        assert!(!ordered.compiler_ordering_effects_v12().is_empty());
        for operation in [&physical, &ordered] {
            let changed = CanonicalKirOperationRefV1 {
                coordinate: original.coordinate,
                operation,
                results: original.results.clone(),
                operands: original.operands.clone(),
                effects: original.effects.clone(),
            };
            let before = (budget.work(), budget.storage());
            assert!(matches!(
                native(inventory, 0, &changed, budget),
                Err(E::Unsupported {
                    phase: "launch scalar",
                    detail: "same borrowed operation site",
                })
            ));
            assert_eq!((budget.work(), budget.storage()), before);
        }
        assert!(native(inventory, 0, original, budget).unwrap());
    });
}

#[test]
fn invocation_index_census_legacy_global_and_extent_keep_exact_existing_debit() {
    for extent in [false, true] {
        let mut graph = module(IndexKind::Global, Axis::X, 1);
        if extent {
            let operation = &mut graph.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
            operation.kind = OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::LaunchExtent { axis: Axis::X },
                Type::INDEX,
            ));
        }
        with_inventory(&graph, |inventory, budget| {
            let before = (budget.work(), budget.storage());
            assert!(!native(inventory, 0, &inventory.operations()[0], budget).unwrap());
            assert_eq!((budget.work(), budget.storage()), before);
            let private = private_memory::check(inventory, 1024, budget).unwrap();
            let division = unsigned_division::check(
                inventory,
                SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(
                    [250; 32],
                )),
                budget,
            )
            .unwrap();
            let helpers = scalar_helpers::check(inventory, budget).unwrap();
            let before = (budget.work(), budget.storage());
            census::native(
                inventory,
                &private,
                &division,
                &helpers,
                "legacy launch",
                |_, _| Ok(false),
                budget,
            )
            .unwrap();
            // Entry(2), root(3)+existing helper query(3), one definition(1),
            // one block(1), one opcode(5). The new helper is never called.
            assert_eq!(budget.work() - before.0, 2 + 3 + 3 + 1 + 1 + 5);
            assert_eq!(budget.storage(), before.1);
        });
    }
}

fn independent_headers() -> usize {
    use std::mem::size_of;
    fn h<T>() -> usize {
        size_of::<T>() + size_of::<Option<T>>() + 2 * size_of::<R<T>>()
    }
    2 * h::<&CanonicalKirInventoryV1<'_>>()
        + 2 * h::<&CanonicalKirOperationRefV1<'_>>()
        + h::<&CanonicalKirFunctionRefV1<'_>>()
        + h::<&CanonicalKirBlockRefV1<'_>>()
        + h::<&CanonicalKirDefinitionRefV1<'_>>()
        + h::<&CanonicalKirKernelRefV1<'_>>()
        + 2 * h::<&IntrinsicOperation>()
        + h::<&ValueDef>()
        + h::<&[ValueDef]>()
        + h::<&[CanonicalKirOperationRefV1<'_>]>()
        + h::<&[CanonicalKirFunctionRefV1<'_>]>()
        + h::<&[CanonicalKirBlockRefV1<'_>]>()
        + h::<&[CanonicalKirDefinitionRefV1<'_>]>()
        + h::<&std::ops::Range<usize>>()
        + h::<&OperationKind>()
        + h::<&Type>()
        + h::<Type>()
        + h::<IntrinsicKind>()
        + h::<ValueId>()
        + h::<fe2o3_kernel_ir::CompilerOrderingEffectSummaryV12>()
        + h::<&[CanonicalKirKernelRefV1<'_>]>()
        + h::<std::slice::Iter<'_, CanonicalKirKernelRefV1<'_>>>()
        + h::<bool>()
        + h::<usize>()
        + size_of::<CanonicalKirDefinitionCoordinateV1>()
        + 2 * size_of::<CanonicalKirOperationCoordinateV1>()
        + 2 * size_of::<&mut AssertOriginBudgetV1<'_>>()
        + 8 * size_of::<usize>()
        + 2 * size_of::<R<()>>()
        + 2 * size_of::<Result<(), AssertOriginResourceV1>>()
}

#[test]
fn invocation_index_census_independent_exact_and_short_query_resources_restore_floor() {
    assert_eq!(headers().unwrap(), independent_headers());
    with_inventory(&module(IndexKind::Workgroup, Axis::X, 1), |inventory, _| {
        const FLOOR: usize = 43;
        let peak = FLOOR + independent_headers();
        for (work_limit, storage_limit, success, accepted) in [
            (66, peak, true, 66),
            (65, peak, false, 64),
            (63, peak, false, 0),
            (66, peak - 1, false, 0),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let result = native(inventory, 0, &inventory.operations()[0], &mut budget);
            if success {
                assert!(result.unwrap());
            } else if storage_limit < peak {
                assert!(matches!(
                    result,
                    Err(E::Resource(AssertOriginResourceV1::Storage(_)))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(E::Resource(AssertOriginResourceV1::Work(_)))
                ));
            }
            assert_eq!(budget.work(), accepted);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(
                budget.peak_storage(),
                if storage_limit < peak { FLOOR } else { peak }
            );
        }
        let mut work = Work::new(132);
        let mut budget = AssertOriginBudgetV1::new(&mut work, peak);
        budget.reserve_storage(FLOOR).unwrap();
        for _ in 0..2 {
            assert!(native(inventory, 0, &inventory.operations()[0], &mut budget).unwrap());
        }
        assert_eq!(budget.work(), 132);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), peak);
    });
}

#[cfg(test)]
#[path = "production_checked_output_invocation_source_v1_tests.rs"]
mod source;
