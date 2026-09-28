#[derive(Clone, Copy, Debug)]
pub(super) enum SharedEntryTestV18 {
    Positive,
    Scaling(usize),
    IssuedRefusal,
    NodeFault(u8),
    RootJoinFault(u8),
    ForeignFacts,
    ForeignQuery(usize),
    SelectedError,
    Panic,
    WorkThenDropPanic,
    RootHeaderDropPanic,
    SwallowedQueryThenError,
    Custody { change: u8, disposition: u8 },
}

#[test]
fn shared_entry_join_fixed_frame_equation_is_independent() {
    use kernel_argument_abi_v18::{SourceDescriptorRootAbiV29, SourceSharedEntryAbiV18};
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T, ProductionSourceOwnedViewErrorV18>>()
    }
    type Join<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a PendingGlobalReadConditionsV18<'a, 'a>,
        &'a mut ArgumentBudgetV1<'a>,
        &'a mut (),
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a CanonicalKirDefinitionRefV1<'a>,
        SourceDescriptorRootAbiV29<'a>,
        Option<SourceDescriptorRootAbiV29<'a>>,
        SourceSharedEntryAbiV18<'a>,
        Option<SourceSharedEntryAbiV18<'a>>,
        &'a SourceSharedEntryAbiV18<'a>,
        ArgumentViewDataV18<'a>,
        &'a ArgumentViewDataV18<'a>,
        ProductionArgumentNodeV1<'a>,
        &'a ProductionArgumentNodeV1<'a>,
        ProductionArgumentCoverageV1<'a>,
        ProductionPhysicalArgumentV1<'a>,
        Option<(SemanticLocalIdV1, &'a [ProductionArgumentProjectionV1])>,
        &'a [ProductionArgumentProjectionV1],
        &'a Type,
        Type,
        [SliceDefinition; 2],
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        SemanticFunctionIdV1,
        SemanticTypeIdV1,
        (SemanticFunctionIdV1, usize),
        Option<(u32, SemanticTypeIdV1)>,
        &'a mut Option<(u32, SemanticTypeIdV1)>,
        (u32, SemanticTypeIdV1),
        [usize; 4],
        Option<usize>,
        u32,
        ValueId,
        Option<ValueId>,
        bool,
        Result<usize, std::num::TryFromIntError>,
        ScalarType,
        fe2o3_kernel_ir::FormalAllocationIdentity,
        fe2o3_kernel_ir::FormalRuntimeSliceReadDomainV1,
        &'a [u8; 32],
        fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1,
    );
    type Nodes<'a> = (
        &'a PendingGlobalReadConditionsV18<'a, 'a>,
        &'a CanonicalKirDefinitionRefV1<'a>,
        &'a SourceSharedEntryAbiV18<'a>,
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a mut (),
        &'a mut usize,
        &'a mut Option<(u32, SemanticTypeIdV1)>,
        usize,
        (u32, SemanticTypeIdV1),
    );
    type Borrowed<'a> = (&'a PendingSharedEntryRegionV18<'a, 'a>, &'a mut ());
    type Consumer<'a> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        &'a PendingSharedEntryRegionV18<'a, 'a>,
        &'a mut (),
        Borrowed<'a>,
        (
            &'a ProductionSourceCorrespondenceV18<'a>,
            &'a mut ArgumentBudgetV1<'a>,
            Borrowed<'a>,
        ),
        std::panic::AssertUnwindSafe<(
            &'a ProductionSourceCorrespondenceV18<'a>,
            &'a mut ArgumentBudgetV1<'a>,
            Borrowed<'a>,
        )>,
        [usize; 2],
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        Option<SourceOwnedQueryFailureV18>,
        SourceOwnedQueryFailureV18,
        bool,
        SourceOwnedResultV18<()>,
        std::thread::Result<SourceOwnedResultV18<()>>,
        SourceOwnedResultV18<()>,
    );
    type RootBuild<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a mut (),
        &'a mut ArgumentBudgetV1<'a>,
        &'a SemanticFunctionIdV1,
        &'a usize,
        &'a SourceDescriptorRootAbiV29<'a>,
        (&'a PendingSharedEntryRegionsV18<'a, 'a>, &'a mut ()),
        [usize; 3],
    );
    type Walk<'a> = (
        &'a PendingSharedEntryRegionsV18<'a, 'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a PendingGlobalReadConditionsV18<'a, 'a>,
        &'a SourceSharedEntryAbiV18<'a>,
        &'a mut (),
        &'a mut usize,
        &'a mut Option<(u32, SemanticTypeIdV1)>,
        &'a usize,
        &'a ValueId,
        &'a Type,
        &'a u32,
        &'a SemanticTypeIdV1,
        ProductionArgumentNodeV1<'a>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    type Finish<'a> = (
        std::thread::Result<SourceOwnedResultV18<()>>,
        Option<SourceOwnedQueryFailureV18>,
        SourceOwnedResultV18<()>,
        &'a ScopedSourceCleanupV29,
        &'a mut ArgumentBudgetV1<'a>,
        usize,
    );
    type RootCatch<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    let expected = kernel_argument_abi_v18::shared_entry_abi_headers_v18().unwrap()
        + [
            h::<Join<'_>>(),
            2 * h::<Nodes<'_>>(),
            h::<Consumer<'_>>(),
            2 * h::<RootBuild<'_>>(),
            2 * h::<Walk<'_>>(),
            h::<Finish<'_>>(),
            h::<RootCatch<'_>>(),
            h::<std::panic::AssertUnwindSafe<RootCatch<'_>>>(),
            h::<(
                usize,
                usize,
                fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
                bool,
            )>(),
            h::<PendingSharedEntryRegionsV18<'_, '_>>(),
            h::<&PendingSharedEntryRegionsV18<'_, '_>>(),
            h::<(
                &PendingSharedEntryRegionsV18<'_, '_>,
                &mut ArgumentBudgetV1<'_>,
            )>(),
            h::<PendingSharedEntryRegionV18<'_, '_>>(),
            h::<(
                &PendingSharedEntryRegionV18<'_, '_>,
                &mut ArgumentBudgetV1<'_>,
            )>(),
            h::<(&ProductionArgumentNodeV1<'_>, usize, ValueId, &Type)>(),
            h::<Result<(), ProductionSemanticKirErrorV1>>(),
            h::<Result<(), ArgumentResourceV1>>(),
            h::<ProductionSemanticKirErrorV1>(),
            h::<ProductionSourceOwnedViewErrorV18>(),
            h::<Result<&CanonicalKirDefinitionRefV1<'_>, ProductionSourceOwnedViewErrorV18>>(),
            h::<Result<Option<SourceSharedEntryAbiV18<'_>>, ProductionSemanticKirErrorV1>>(),
            h::<Result<Option<SourceDescriptorRootAbiV29<'_>>, ProductionSourceOwnedViewErrorV18>>(
            ),
            h::<Result<(SemanticFunctionIdV1, usize), ProductionSourceOwnedViewErrorV18>>(),
            h::<Option<ValueId>>(),
            h::<fe2o3_mir_model::SemanticSourceArgumentV1<'_>>(),
            h::<(SemanticLocalIdV1, &[ProductionArgumentProjectionV1])>(),
            h::<(
                &PendingGlobalSourceAccessesV18<'_>,
                &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
                &GlobalReadFactsV18<'_, '_>,
                SliceOperation,
                &mut ArgumentBudgetV1<'_>,
            )>(),
            h::<(&ScopedSourceCleanupV29, &mut ArgumentBudgetV1<'_>, usize)>(),
            h::<[usize; 3]>(),
            h::<Result<(), PendingGlobalReadConditionErrorV18>>(),
            2 * h::<std::thread::Result<Result<(), PendingGlobalReadConditionErrorV18>>>(),
        ]
        .into_iter()
        .sum::<usize>();
    assert_eq!(shared_entry_headers_v18().unwrap(), expected);
}

pub(super) fn test_shared_entry_region_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    mode: SharedEntryTestV18,
    work_limit: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    observed: &std::cell::Cell<[usize; 6]>,
) -> SourceOwnedResultV18<()> {
    original.with_pending_global_accesses_v18(optimized, root, budget, |source, budget| {
        original.with_optimized_guarded_reads_v18(
            optimized,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1::default(),
            budget,
            |facts, budget| {
                with_local_read_native_test_view_v18(original, optimized, budget, |native, budget| {
                    if matches!(mode, SharedEntryTestV18::RootHeaderDropPanic) {
                        source.with_shared_entry_regions_v18(budget, |_, _| Ok(()))?;
                        struct DropPanic;
                        impl Drop for DropPanic {
                            fn drop(&mut self) { std::panic::resume_unwind(Box::new(0x169a_u64)); }
                        }
                        let floor = budget.storage();
                        budget.reserve_storage(size_of::<u64>()).unwrap();
                        let fill = budget.storage_limit() - budget.storage();
                        budget.reserve_storage(fill).unwrap();
                        let before = (budget.work(), budget.storage());
                        let dropper = DropPanic;
                        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            source.with_shared_entry_regions_v18(budget, move |_, _| {
                                let _owned = &dropper;
                                panic!("first constructor reservation must deny before root callback");
                            })
                        }));
                        assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x169a);
                        assert_eq!((budget.work(), budget.storage()), before);
                        let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(first)))
                            = original.check(budget) else { panic!("constructor storage failure must precede callback drop"); };
                        assert_eq!(first.limit(), budget.storage_limit());
                        assert!(first.actual() > first.limit());
                        assert_eq!((budget.work(), budget.storage()), before);
                        assert!(matches!(original.check(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Storage(error))) if error == first));
                        budget.release_storage(fill + size_of::<u64>()).unwrap();
                        assert_eq!(budget.storage(), floor);
                        observed.set([1, 0, 0, 0, 0, 0]);
                        return Ok(());
                    }
                    let build_start = budget.work();
                    source.with_shared_entry_regions_v18(budget, |entries, budget| {
                    let pair = source.roles.rows.iter().find_map(|row| row.global.as_ref()
                        .filter(|pair| !pair.output.writing)).expect("genuine source read");
                    let operation = pair.output.logical.access.operation;
                    let floor = budget.storage();
                    match mode {
                        SharedEntryTestV18::RootJoinFault(fault) => {
                            entries.with_shared_entry_region_v18(native, facts, operation, budget, |_, _| {
                                observed.set([1, 0, 0, 0, 0, 0]);
                                Ok(())
                            }).unwrap();
                            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
                                let (other, physical) = original.source.root(1 - root, budget)?;
                                assert_ne!(other, entries.original_function);
                                assert_ne!(physical, entries.physical_function);
                                original.with_root_argument_data_v18(root, budget, |arguments, budget| {
                                    let changed = PendingSharedEntryRegionsV18 {
                                        source: entries.source,
                                        arguments,
                                        profile: entries.profile,
                                        original_function: if fault == 0 { other } else { entries.original_function },
                                        physical_function: if fault == 1 { physical } else { entries.physical_function },
                                    };
                                    let before = budget.storage();
                                    let result = changed.with_shared_entry_region_v18(native, facts, operation, budget,
                                        |_, _| panic!("copied root join must not enter consumer"));
                                    let detail = if fault == 0 { "original source argument correspondence" }
                                        else { "shared entry region changed original root" };
                                    assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                        ProductionSourceOwnedViewErrorV18::Binding(actual))) if actual == detail), "{result:?}");
                                    assert_eq!(budget.storage(), before);
                                    Ok(())
                                })?;
                                Ok(())
                            })?;
                        }
                        SharedEntryTestV18::RootHeaderDropPanic => unreachable!(),
                        SharedEntryTestV18::Scaling(reads) => {
                            let construction = budget.work() - build_start;
                            let mut nodes = 0usize;
                            entries.arguments.visit_nodes_scoped(budget, |_, _| {
                                nodes += 1;
                                Ok(())
                            }).map_err(source_argument_error_v18)?;
                            assert_eq!(budget.storage(), floor);
                            let mut per_read = None;
                            let begin = budget.work();
                            let mut callbacks = 0;
                            for _ in 0..reads {
                                let before = budget.work();
                                entries.with_shared_entry_region_v18(native, facts, operation, budget, |view, _| {
                                    assert_eq!(view.root(), root);
                                    assert_eq!(view.original_argument(), 1);
                                    callbacks += 1;
                                    Ok(())
                                }).map_err(local_read_test_error_v18)?;
                                let increment = budget.work() - before;
                                if let Some(expected) = per_read { assert_eq!(increment, expected); }
                                per_read = Some(increment);
                                assert_eq!(budget.storage(), floor, "no retained per-read rows");
                            }
                            assert_eq!(callbacks, reads);
                            assert_eq!(budget.work() - begin, reads * per_read.unwrap());
                            observed.set([reads, per_read.unwrap(), floor, nodes, root, construction]);
                        }
                        SharedEntryTestV18::Positive => {
                            let mut previous = None;
                            for _ in 0..3 {
                                let before = budget.work();
                                entries.with_shared_entry_region_v18(native, facts, operation, budget, |view, _| {
                                    assert!(std::ptr::eq(view.read.pair, pair));
                                    assert_eq!(view.root(), root);
                                    assert_eq!(view.original_parameter(), pair.input.logical.root);
                                    assert_eq!(view.optimized_parameter(), pair.output.logical.root);
                                    let SliceDefinition::FunctionArgument { argument: input, .. } = view.original_parameter()
                                        else { panic!("actual input parameter"); };
                                    let SliceDefinition::FunctionArgument { argument: output, .. } = view.optimized_parameter()
                                        else { panic!("actual output parameter"); };
                                    assert_eq!(view.allocation().parameter_index(), output);
                                    assert_eq!(view.original_type(), view.source.semantic_type());
                                    let semantic = original.source.owner.inner.source.owner.source_semantic();
                                    assert_eq!(view.original_type_identity(), semantic.types()[view.original_type().index() as usize].identity());
                                    assert_eq!(view.root_binding(), semantic.functions()[view.original_function().index() as usize]
                                        .kernel_entry().unwrap().kernel_binding_identity().as_bytes());
                                    assert_eq!(view.original_argument(), view.source.source_argument());
                                    assert_eq!(view.element(), pair.input.scalar);
                                    assert_eq!(view.element(), pair.output.scalar);
                                    assert_eq!(view.source_sha256(), original.source.owner.inner.source.owner.source_semantic_sha256());
                                    assert!(view.source.source_path().is_empty());
                                    assert!(view.source.local_binding().is_some_and(|(_, path)| path.is_empty()));
                                    assert!(view.requires_runtime_allocation_binding());
                                    assert!(view.requires_runtime_initialized_extent());
                                    assert!(view.requires_runtime_allocation_lifetime());
                                    assert!(view.requires_runtime_base_alignment());
                                    assert!(!view.alias_and_concurrency_are_proved());
                                    assert!(!view.grants_memory_or_launch_authority());
                                    assert!(!view.read.native.memory_safety_is_complete());
                                    let count = observed.get()[0] + 1;
                                    observed.set([count, root, view.original_argument() as usize,
                                        input as usize, output as usize, view.original_function().index() as usize]);
                                    Ok(())
                                }).map_err(local_read_test_error_v18)?;
                                assert_eq!(budget.storage(), floor);
                                let work = budget.work() - before;
                                if let Some(previous) = previous { assert_eq!(work, previous); }
                                previous = Some(work);
                            }
                        }
                        SharedEntryTestV18::IssuedRefusal => {
                            source.with_local_read_conditions_v18(native, facts, operation, budget, |view, _| {
                                assert!(view.is_some(), "issued local read must independently succeed");
                                Ok(())
                            }).unwrap();
                            let result = entries.with_shared_entry_region_v18(native, facts, operation, budget,
                                |_, _| panic!("issued descriptor must not become a SharedSlice entry contract"));
                            assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Binding("shared entry region requires original SharedSlice")))));
                            assert_eq!(budget.storage(), floor);
                            observed.set([1, 0, 0, 0, 0, 0]);
                        }
                        SharedEntryTestV18::NodeFault(fault) => {
                            entries.with_shared_entry_region_v18(native, facts, operation, budget, |view, budget| {
                                let ProductionArgumentCoverageV1::Parameter(parameter) = view.source.coverage()
                                    else { panic!("genuine whole Parameter"); };
                                assert!(shared_entry_node_matches_v18(&view.source, parameter.slot(), parameter.value(), parameter.ty()));
                                let source_root = view.source.source_path().is_empty();
                                let mut local_root = view.source.local_binding().is_some_and(|(_, path)| path.is_empty());
                                let mut coverage = view.source.coverage();
                                let mut slot = parameter.slot();
                                let mut value = parameter.value();
                                let mut ty = parameter.ty();
                                let wrong_type = Type::Scalar(ScalarType::U32);
                                let sibling = parameter.slot() + 1;
                                let sibling_parameter = entries.arguments.physical(sibling, budget)
                                    .map_err(source_argument_error_v18)?
                                    .expect("genuine sibling physical parameter");
                                assert_eq!(sibling_parameter.ty(), parameter.ty());
                                let sibling_value = sibling_parameter.value();
                                assert_ne!(sibling_value, parameter.value());
                                match fault {
                                    0 => slot = sibling,
                                    1 => value = sibling_value,
                                    2 => ty = &wrong_type,
                                    3 => local_root = false,
                                    4 => coverage = ProductionArgumentCoverageV1::WithinAtomicParameter(parameter),
                                    5 => coverage = ProductionArgumentCoverageV1::Zero,
                                    _ => unreachable!(),
                                }
                                // Copied inert projections exercise the predicate, not source admission.
                                assert!(!shared_entry_node_parts_match_v18(source_root, local_root, coverage, slot, value, ty));
                                observed.set([1, 0, 0, 0, 0, 0]);
                                Ok(())
                            }).map_err(local_read_test_error_v18)?;
                            assert_eq!(budget.storage(), floor);
                        }
                        SharedEntryTestV18::ForeignFacts => {
                            let result = fe2o3_kernel_ir::with_canonical_guarded_global_reads_v18(
                                original.inventory.owner(), fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1::default(),
                                budget, |foreign, budget| {
                                    let before = budget.storage();
                                    let result = entries.with_shared_entry_region_v18(native, foreign, operation, budget,
                                        |_, _| panic!("foreign formal owner must not enter"));
                                    assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                        ProductionSourceOwnedViewErrorV18::Binding("pending global read changed formal owner")))));
                                    assert_eq!(budget.storage(), before);
                                    observed.set([1, 0, 0, 0, 0, 0]);
                                    Ok(())
                                });
                            result.unwrap();
                        }
                        SharedEntryTestV18::ForeignQuery(extra) => {
                            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                            let mut foreign = ArgumentBudgetV1::new(&mut work, budget.storage_limit());
                            foreign.reserve_storage(floor + extra).unwrap();
                            let before = (foreign.work(), foreign.storage());
                            let result = entries.with_shared_entry_region_v18(native, facts, operation, &mut foreign,
                                |_, _| panic!("foreign ledger must not enter"));
                            assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)))));
                            assert_eq!((foreign.work(), foreign.storage()), before);
                            assert!(original.source.cleanup.is_denied());
                            observed.set([1, 0, 0, 0, 0, 0]);
                        }
                        SharedEntryTestV18::SelectedError => {
                            let result = entries.with_shared_entry_region_v18(native, facts, operation, budget, |_, _| {
                                observed.set([1, 0, 0, 0, 0, 0]);
                                Err(ProductionSourceOwnedViewErrorV18::Binding("shared entry callback sentinel"))
                            });
                            assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Binding("shared entry callback sentinel")))));
                            assert_eq!(budget.storage(), floor);
                            let before = (budget.work(), budget.storage());
                            assert!(matches!(original.check(budget), Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "shared entry callback sentinel"))));
                            assert_eq!((budget.work(), budget.storage()), before);
                        }
                        SharedEntryTestV18::Panic => {
                            budget.reserve_storage(size_of::<u64>()).unwrap();
                            let before = budget.storage();
                            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                entries.with_shared_entry_region_v18(native, facts, operation, budget, |_, _| {
                                    observed.set([1, 0, 0, 0, 0, 0]);
                                    std::panic::resume_unwind(Box::new(0x1698_u64))
                                })
                            }));
                            assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1698);
                            assert_eq!(budget.storage(), before);
                            budget.release_storage(size_of::<u64>()).unwrap();
                            original.check(budget).unwrap();
                        }
                        SharedEntryTestV18::WorkThenDropPanic => {
                            struct DropPanic;
                            impl Drop for DropPanic {
                                fn drop(&mut self) { std::panic::resume_unwind(Box::new(0x1699_u64)); }
                            }
                            entries.with_shared_entry_region_v18(native, facts, operation, budget, |_, _| Ok(())).unwrap();
                            budget.reserve_storage(size_of::<u64>()).unwrap();
                            let before = budget.storage();
                            budget.charge_work(work_limit - budget.work()).unwrap();
                            let dropper = DropPanic;
                            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                entries.with_shared_entry_region_v18(native, facts, operation, budget, move |_, _| {
                                    let _owned = &dropper;
                                    panic!("authentic owner query work denial must precede callback");
                                })
                            }));
                            assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1699);
                            assert_eq!(budget.storage(), before);
                            budget.release_storage(size_of::<u64>()).unwrap();
                            let before = (budget.work(), budget.storage());
                            let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(first)))
                                = original.check(budget) else { panic!("selected authentic Work refusal"); };
                            assert_eq!((first.actual(), first.limit()), (work_limit + 1, work_limit));
                            assert_eq!((budget.work(), budget.storage()), before);
                            observed.set([1, 0, 0, 0, 0, 0]);
                        }
                        SharedEntryTestV18::SwallowedQueryThenError => {
                            let first = std::cell::Cell::new(None);
                            let result = entries.with_shared_entry_region_v18(native, facts, operation, budget, |_, budget| {
                                budget.charge_work(work_limit - budget.work())?;
                                let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)))
                                    = source.operation_count(budget) else { panic!("authentic source query denial"); };
                                first.set(Some(error));
                                observed.set([1, 0, 0, 0, 0, 0]);
                                Err(ProductionSourceOwnedViewErrorV18::Binding("later shared entry sentinel"))
                            });
                            assert!(matches!(result, Err(PendingGlobalReadConditionErrorV18::Source(
                                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error))))
                                if Some(error) == first.get()));
                            assert_eq!(budget.storage(), floor);
                            let before = (budget.work(), budget.storage());
                            assert!(matches!(original.check(budget), Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Work(error))) if Some(error) == first.get()));
                            assert_eq!((budget.work(), budget.storage()), before);
                        }
                        SharedEntryTestV18::Custody { change, disposition } => {
                            let scaffolding = if change >= 2 { size_of::<CanonicalKernelIrWorkBudgetV1>() } else { 0 }
                                + if disposition == 2 { size_of::<u64>() } else { 0 };
                            budget.reserve_storage(scaffolding).unwrap();
                            let expected = std::cell::Cell::new(0);
                            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                entries.with_shared_entry_region_v18(native, facts, operation, budget, |_, budget| {
                                    match change {
                                        0 => budget.release_storage(1)?,
                                        1 => budget.reserve_storage(1)?,
                                        2 | 3 => {
                                            let work = Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(work_limit)));
                                            let mut foreign = ArgumentBudgetV1::new(work, budget.storage_limit());
                                            foreign.reserve_storage(budget.storage() + usize::from(change == 3))?;
                                            drop(std::mem::replace(budget, foreign));
                                        }
                                        _ => unreachable!(),
                                    }
                                    expected.set(budget.storage());
                                    match disposition {
                                        0 => Ok(()),
                                        1 => Err(ProductionSourceOwnedViewErrorV18::Binding("shared custody sentinel")),
                                        _ => std::panic::resume_unwind(Box::new(0x1698_u64)),
                                    }
                                })
                            }));
                            assert_eq!(budget.storage(), expected.get());
                            assert!(original.source.cleanup.is_denied());
                            if change >= 2 { assert_eq!(budget.work(), 0); }
                            match disposition {
                                0 => assert!(matches!(caught.unwrap(), Err(PendingGlobalReadConditionErrorV18::Source(
                                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))))),
                                1 => assert!(matches!(caught.unwrap(), Err(PendingGlobalReadConditionErrorV18::Source(
                                    ProductionSourceOwnedViewErrorV18::Binding("shared custody sentinel"))))),
                                _ => assert_eq!(*caught.unwrap_err().downcast::<u64>().unwrap(), 0x1698),
                            }
                            observed.set([1, expected.get(), 0, 0, 0, 0]);
                        }
                    }
                    Ok(())
                    })
                })
            },
        ).map_err(|error| match error {
            ProductionOptimizedSourceFormalErrorV18::Source(error)
            | ProductionOptimizedSourceFormalErrorV18::Formal { source_refusal: error, .. }
            | ProductionOptimizedSourceFormalErrorV18::Consumer(error) => error,
        })
    })
}
