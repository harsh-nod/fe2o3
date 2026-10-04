use super::*;

#[test]
fn scalar_boundary_seen_census_is_linear_in_exact_function_ranges() {
    with_policy11(two_phi_roots, |original, optimized, budget| {
        for inventory in [original.inventory, optimized.output_inventory(budget)?] {
            let floor = budget.storage();
            let headers = source_boundary_control_headers_v31()?;
            budget.reserve_storage(headers)?;
            let mut end = 0;
            let mut total_work = 0;
            let mut total_len = 0;
            let mut nonempty = 0;
            for function in inventory.functions() {
                let before = (budget.work(), budget.storage());
                let (bindings, seen) =
                    source_boundary_function_seen_v31(inventory, function.coordinate, budget)?;
                assert_eq!(bindings, function.edge_arguments);
                assert_eq!(bindings.start, end);
                assert_eq!(seen.len(), bindings.len());
                assert!(seen.iter().all(|value| !value));
                assert_eq!(budget.work() - before.0, 6 + bindings.len());
                assert_eq!(
                    budget.storage() - before.1,
                    seen.capacity() * size_of::<bool>()
                );
                for (local, binding) in inventory.edge_arguments()[bindings.clone()]
                    .iter()
                    .enumerate()
                {
                    assert_eq!(binding.coordinate.edge.source.function, function.coordinate);
                    assert_eq!(
                        source_boundary_seen_index_v31(&bindings, bindings.start + local)?,
                        local
                    );
                }
                assert!(matches!(
                    source_boundary_seen_index_v31(&bindings, bindings.end),
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "source SSA physical argument belongs to another function"
                    ))
                ));
                if bindings.start != 0 {
                    assert!(matches!(
                        source_boundary_seen_index_v31(&bindings, bindings.start - 1),
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "source SSA physical argument belongs to another function"
                        ))
                    ));
                }
                total_work += budget.work() - before.0;
                total_len += bindings.len();
                nonempty += usize::from(!bindings.is_empty());
                end = bindings.end;
                let credit = seen.capacity() * size_of::<bool>();
                drop(seen);
                budget.release_storage(credit)?;
                assert_eq!(budget.storage(), before.1);
            }
            assert!(nonempty >= 2);
            assert_eq!(end, inventory.edge_arguments().len());
            assert_eq!(total_len, inventory.edge_arguments().len());
            assert_eq!(total_work, 6 * inventory.functions().len() + total_len);
            budget.release_storage(headers)?;
            assert_eq!(budget.storage(), floor);
        }
        Ok(())
    })
    .unwrap();
}

fn function_seen_probe_v31(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result = (|| {
        budget.reserve_storage(source_boundary_control_headers_v31()?)?;
        let (_, seen) = source_boundary_function_seen_v31(inventory, function, &mut budget)?;
        assert!(!seen.is_empty());
        drop(seen);
        Ok(())
    })();
    let measured = (budget.work(), budget.peak_storage());
    // This isolated probe owns every credit; the immutable inventory is lent by
    // the genuine source fixture and is never admitted on this probe's ledger.
    budget.release_storage(budget.storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    (result, measured.0, measured.1)
}

#[test]
fn scalar_boundary_function_seen_has_exact_and_one_short_resources() {
    with_policy11(two_phi_roots, |_, optimized, budget| {
        let inventory = optimized.output_inventory(budget)?;
        let function = inventory
            .functions()
            .iter()
            .find(|row| !row.edge_arguments.is_empty())
            .unwrap();
        assert!(function.edge_arguments.len() < inventory.edge_arguments().len());
        let (result, work, storage) =
            function_seen_probe_v31(inventory, function.coordinate, usize::MAX, usize::MAX);
        result?;
        assert_eq!(work, 6 + function.edge_arguments.len());
        assert!(
            function_seen_probe_v31(inventory, function.coordinate, work, storage)
                .0
                .is_ok()
        );
        assert!(matches!(
            function_seen_probe_v31(inventory, function.coordinate, work - 1, storage).0,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Work { .. }
            ))
        ));
        assert!(matches!(
            function_seen_probe_v31(inventory, function.coordinate, work, storage - 1).0,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage { .. }
            ))
        ));
        Ok(())
    })
    .unwrap();
}
