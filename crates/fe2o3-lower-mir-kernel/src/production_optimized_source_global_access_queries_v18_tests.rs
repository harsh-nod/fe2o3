// White-box checks stay beside the private rows. No copied row authorizes a
// native operation; both source factories below reach the real optimizer.
fn global_header_oracle_v18() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    h::<PendingGlobalSourceAccessesV18<'_>>()
        + h::<&PendingGlobalSourceAccessesV18<'_>>()
        + h::<&CheckedDescriptorSourceRolesV18<'_>>()
        + h::<&mut ArgumentBudgetV1<'_>>()
        + h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            SliceOperation,
            &mut ArgumentBudgetV1<'_>,
        )>()
        + h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            &SliceOperation,
            &mut ArgumentBudgetV1<'_>,
        )>()
        + h::<(&&DescriptorSourceRoleRowV18, &SliceOperation)>()
        + h::<(&&mut DescriptorSourceRoleRowV18, &SliceOperation)>()
        + h::<(
            &ProductionSourceCorrespondenceV18<'_>,
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            &mut [DescriptorSourceRoleRowV18],
            &mut ArgumentBudgetV1<'_>,
        )>()
        + h::<(
            &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
            GlobalSourceLogicalEndpointV18,
            &mut ArgumentBudgetV1<'_>,
        )>()
        + h::<(
            &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
            fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
            &mut ArgumentBudgetV1<'_>,
        )>()
        + h::<Option<&GlobalSourceAccessPairV18>>()
        + h::<&GlobalSourceAccessPairV18>()
        + h::<&DescriptorSourceRoleRowV18>()
        + h::<Option<&DescriptorSourceRoleRowV18>>()
        + h::<&mut DescriptorSourceRoleRowV18>()
        + h::<Option<&mut DescriptorSourceRoleRowV18>>()
        + h::<Option<GlobalSourceAccessPairV18>>()
        + 2 * h::<GlobalSourceAccessPairV18>()
        + 3 * h::<GlobalSourceLogicalEndpointV18>()
        + 3 * h::<GlobalSourceAccessEndpointV18>()
        + 2 * h::<Option<GlobalSourceAccessEndpointV18>>()
        + h::<GlobalSourceAccessOriginV18>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>>()
        + h::<&Option<Terminator>>()
        + h::<Option<&Terminator>>()
        + h::<&Terminator>()
        + h::<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>()
        + h::<fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1>()
        + h::<SliceDefinition>()
        + h::<Result<u32, std::num::TryFromIntError>>()
        + h::<&CanonicalKirOperationRefV1<'_>>()
        + h::<&CanonicalKirOperationRefV1<'_>>()
        + h::<&OperationKind>()
        + h::<&ValueId>()
        + h::<SliceDefinition>()
        + h::<Option<&CanonicalKirDefinitionRefV1<'_>>>()
        + h::<&CanonicalKirDefinitionRefV1<'_>>()
        + size_of::<
            Result<
                Option<&CanonicalKirDefinitionRefV1<'_>>,
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
            >,
        >()
        + h::<&CanonicalKirDefinitionRefV1<'_>>()
        + h::<Option<&CanonicalKirDefinitionRefV1<'_>>>()
        + size_of::<
            Result<
                Option<&CanonicalKirDefinitionRefV1<'_>>,
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
            >,
        >()
        + h::<&fe2o3_kernel_ir::Operation>()
        + h::<&OperationKind>()
        + h::<&[fe2o3_kernel_ir::ValueDef]>()
        + h::<&fe2o3_kernel_ir::ValueDef>()
        + h::<&Type>()
        + 2 * h::<&ValueId>()
        + h::<&MemoryAccess>()
        + h::<Option<ScalarType>>()
        + h::<(ValueId, ValueId, MemoryAccess, ScalarType, bool)>()
        + 3 * h::<ValueId>()
        + h::<MemoryAccess>()
        + h::<ScalarType>()
        + h::<bool>()
        + 2 * h::<usize>()
        + h::<u32>()
        + h::<()>()
}

#[test]
fn pending_global_access_headers_have_an_independent_exact_and_one_short_equation() {
    let expected = global_header_oracle_v18();
    assert_eq!(global_source_headers_v18().unwrap(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected - usize::from(short));
        let result = budget.reserve_storage(global_source_headers_v18().unwrap());
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == expected && error.limit() == expected - 1));
            assert_eq!(budget.storage(), 0);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), expected);
            budget.release_storage(expected).unwrap();
        }
        assert_eq!(budget.work(), 0);
    }
}

pub(super) fn test_pending_global_accesses_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    issued: bool,
    expected: [usize; 2],
    expected_volatile: usize,
    disposition: usize,
    completed: &std::cell::Cell<bool>,
    retained_floor: &std::cell::Cell<Option<usize>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    let result = original.with_pending_global_accesses_v18(optimized, 0, budget, |view, budget| {
        assert!(std::ptr::eq(view.original(), original));
        assert!(std::ptr::eq(view.optimized(), optimized));
        assert_eq!(view.root(), 0);
        let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
        let output = optimized.output_inventory(budget)?;
        assert_eq!(view.operation_count(budget)?, function.operations.len());
        let mut counts = [0; 2];
        let mut volatile = 0;
        for operation in &output.operations()[function.operations.clone()] {
            let queried = view.access(operation.coordinate, budget)?;
            if matches!(&operation.operation.kind, OperationKind::Load { access, .. }
                | OperationKind::Store { access, .. } if access.address_space == AddressSpace::Global && access.volatile) {
                assert!(queried.is_none(), "ordered access is not ordinary global coverage");
                volatile += 1;
            }
            let Some(pair) = queried else { continue; };
            assert_eq!(pair.output.logical.access.operation, operation.coordinate);
            assert!(matches!((issued, pair.origin),
                (true, GlobalSourceAccessOriginV18::Issued { .. })
                | (false, GlobalSourceAccessOriginV18::Assertion(_))));
            assert_eq!(pair.input.scalar, pair.output.scalar);
            assert_eq!(pair.input.memory, pair.output.memory);
            assert_eq!(pair.input.writing, pair.output.writing);
            for (inventory, endpoint) in [(original.inventory, &pair.input), (output, &pair.output)] {
                let actual = source_operation_row_v18(inventory, endpoint.logical.access.operation, budget)?;
                assert_eq!(endpoint.logical.access.effect, 0);
                assert_eq!(actual.effects.len(), 1);
                assert_eq!(endpoint.memory.address_space, AddressSpace::Global);
                assert!(!endpoint.memory.volatile);
                match (&actual.operation.kind, endpoint.writing) {
                    (OperationKind::Load { pointer, access }, false) => {
                        assert_eq!((*pointer, *access), (endpoint.pointer, endpoint.memory));
                        assert_eq!(actual.operation.results.len(), 1);
                        assert_eq!(actual.operation.results[0].id, endpoint.value);
                        assert_eq!(actual.operation.results[0].ty, Type::Scalar(endpoint.scalar));
                    }
                    (OperationKind::Store { pointer, value, access }, true) => {
                        assert_eq!((*pointer, *value, *access), (endpoint.pointer, endpoint.value, endpoint.memory));
                        assert!(actual.operation.results.is_empty());
                    }
                    _ => panic!("pending row changed its actual raw Global operation"),
                }
                let pointer = inventory.definition_for_value(endpoint.logical.access.operation.block.function,
                    endpoint.pointer, budget).map_err(source_pointer_inventory_error_v18)?.unwrap();
                assert_eq!(pointer.coordinate, SliceDefinition::Result { operation: endpoint.logical.address, result: 0 });
                let address = source_operation_row_v18(inventory, endpoint.logical.address, budget)?;
                let exact_index = optimized_source_definition_row_v18(
                    inventory, endpoint.address_index, budget)?;
                assert!(matches!(address.operation.kind, OperationKind::GetElementPointer { offset, .. }
                    if Some(offset) == exact_index.value));
                assert!(matches!(source_operation_row_v18(inventory, endpoint.logical.length, budget)?.operation.kind,
                    OperationKind::SliceLength { .. }));
                assert!(matches!(source_operation_row_v18(inventory, endpoint.logical.data, budget)?.operation.kind,
                    OperationKind::SliceData { .. }));
                optimized_source_definition_row_v18(inventory, endpoint.logical.root, budget)?;
                optimized_source_definition_row_v18(inventory, endpoint.logical.index, budget)?;
                assert_eq!(global_source_guard_definition_v18(inventory, endpoint.logical.guard_edge.source, budget)?,
                    endpoint.logical.guard_condition);
            }
            counts[usize::from(pair.output.writing)] += 1;
        }
        assert_eq!(counts, expected);
        assert_eq!(volatile, expected_volatile);
        completed.set(true);
        match disposition {
            0 => Ok(()),
            1 => Err(ProductionSourceOwnedViewErrorV18::Binding("selected pending global callback")),
            2 => std::panic::panic_any("selected pending global panic"),
            3 => {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
                let mut foreign = ArgumentBudgetV1::new(&mut work, 1000);
                assert!(matches!(view.operation_count(&mut foreign),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
                assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                let stopped = (budget.work(), budget.storage());
                assert!(matches!(view.operation_count(budget),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting))));
                assert_eq!((budget.work(), budget.storage()), stopped);
                assert!(original.source.cleanup.is_denied());
                retained_floor.set(Some(budget.storage()));
                Ok(())
            }
            4 => {
                let mut foreign = output.operations()[function.operations.start].coordinate;
                foreign.block.function.0 += 1;
                assert!(matches!(view.access(foreign, budget),
                    Err(ProductionSourceOwnedViewErrorV18::Binding("pending global access changed actual occurrence"))));
                let stopped = (budget.work(), budget.storage());
                assert!(matches!(view.operation_count(budget),
                    Err(ProductionSourceOwnedViewErrorV18::Binding("pending global access changed actual occurrence"))));
                assert_eq!((budget.work(), budget.storage()), stopped);
                Ok(())
            }
            _ => panic!("closed pending global disposition"),
        }
    });
    assert_eq!(budget.storage(), retained_floor.get().unwrap_or(floor));
    result
}

pub(super) fn test_pending_global_growth_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    expected_stores: usize,
    observed: &std::cell::Cell<Option<[usize; 2]>>,
    stage: &std::cell::Cell<&'static str>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    stage.set("pending scope construction");
    let result = original.with_pending_global_accesses_v18(optimized, 0, budget, |view, budget| {
        stage.set("pending operation count");
        let count = view.operation_count(budget)?;
        assert!(count > expected_stores);
        stage.set("original root and output census");
        let (_, original_ordinal) = original.source.root(view.root(), budget)?;
        let root_name = original.inventory.functions()[original_ordinal]
            .function
            .id
            .as_str();
        let functions = optimized.output_inventory(budget)?.functions().len();
        assert!(functions > 0);
        // The full owner check has 16 fixed work and one paid binary name
        // lookup. It does not reconstruct source or walk the operation rows.
        let owner_bound = 16 + (root_name.len() + 2) * (functions.ilog2() as usize + 1);
        let fixed_start = budget.work();
        stage.set("complete owner query");
        view.roles.check(budget)?;
        let fixed_work = budget.work() - fixed_start;
        assert!(fixed_work >= 16 + root_name.len() + 2);
        assert!(fixed_work <= owner_bound);
        let mut stores = 0;
        let mut largest_lookup = 0;
        for row in view.roles.rows {
            stage.set("repeated exact access query");
            let target = [
                row.output.block.function.0 as usize,
                row.output.block.block as usize,
                row.output.operation as usize,
            ];
            // Independently count the ordered-key comparisons and midpoint
            // updates, without invoking either production lookup helper.
            let mut interval = 0..count;
            let mut lookup_work = 1;
            while !interval.is_empty() {
                let middle = interval.start + (interval.end - interval.start) / 2;
                let coordinate = view.roles.rows[middle].output;
                let key = [
                    coordinate.block.function.0 as usize,
                    coordinate.block.block as usize,
                    coordinate.operation as usize,
                ];
                let compared = key
                    .iter()
                    .zip(target)
                    .position(|(a, b)| *a != b)
                    .map_or(3, |index| index + 1);
                lookup_work += 6 + compared;
                if key < target {
                    lookup_work += 1;
                    interval.start = middle + 1;
                } else {
                    interval.end = middle;
                }
            }
            assert_eq!(view.roles.rows[interval.start].output, row.output);
            assert!(lookup_work <= 1 + 10 * (count.ilog2() as usize + 1));
            largest_lookup = largest_lookup.max(lookup_work);
            let retained = budget.storage();
            let mut published = false;
            for repeat in 0..2 {
                let start = budget.work();
                let pair = view.access(row.output, budget)?;
                assert_eq!(budget.work() - start, fixed_work + lookup_work + 48);
                assert!(budget.work() - start <= owner_bound + lookup_work + 48);
                assert_eq!(budget.storage(), retained);
                if repeat == 0 {
                    published = pair.is_some();
                } else {
                    assert_eq!(pair.is_some(), published);
                }
                if let Some(pair) = pair {
                    assert!(pair.output.writing);
                    assert!(!row.write_recipe_pending);
                    assert!(matches!(
                        pair.origin,
                        GlobalSourceAccessOriginV18::Issued { .. }
                    ));
                    assert_eq!(pair.output.logical.access.operation, row.output);
                }
            }
            stores += usize::from(published);
        }
        if stores != expected_stores {
            // Preserve the independent physical census on the failing path.
            let _ = (|| -> SourceOwnedResultV18<()> {
                let output = optimized.output_inventory(budget)?;
                let function =
                    optimized_source_root_function_v18(original, optimized, view.root(), budget)?;
                let output_operations = &output.operations()[function.operations.clone()];
                let input_function = &original.inventory.functions()[original_ordinal];
                let input_operations =
                    &original.inventory.operations()[input_function.operations.clone()];
                let input_stores = input_operations
                    .iter()
                    .filter(|row| matches!(row.operation.kind, OperationKind::Store { .. }))
                    .count();
                assert_eq!(
                    input_stores, expected_stores,
                    "independent original Store census"
                );
                assert_eq!(output_operations.len(), view.roles.rows.len());
                for (actual, row) in output_operations
                    .iter()
                    .zip(view.roles.rows)
                    .filter(|(actual, _)| {
                        matches!(actual.operation.kind, OperationKind::Store { .. })
                    })
                    .take(32)
                {
                    assert_eq!(actual.coordinate, row.output);
                }
                Ok(())
            })();
        }
        assert_eq!(stores, expected_stores);
        observed.set(Some([count, largest_lookup]));
        stage.set("pending scope postflight");
        Ok(())
    });
    assert_eq!(budget.storage(), floor);
    if result.is_ok() {
        stage.set("optimized consumer postflight");
    }
    result
}
