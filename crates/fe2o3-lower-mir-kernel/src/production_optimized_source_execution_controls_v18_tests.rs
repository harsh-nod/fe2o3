use super::*;

fn binding<T>(result: SourceOwnedResultV18<T>, expected: &'static str) {
    assert!(
        matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(actual)) if actual == expected)
    );
}

// Copies below are independent hostile oracle inputs, not replacement owners.
// Every positive and mutation queries the same actual admitted inventories and
// original sidecars, after the complete public recipe scope already succeeded.
pub(super) fn actual_controls(
    view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    let rows = build(view, budget)?;
    assert!(!rows.is_empty());
    let mut coordinates: Vec<_> = rows.iter().map(|row| row.input).collect();
    check_census(view.checked.input(), coordinates.iter().copied(), budget)?;
    let removed = coordinates.remove(0);
    binding(
        check_census(view.checked.input(), coordinates.iter().copied(), budget),
        "execution recipe complete operation census",
    );
    coordinates.insert(0, removed);
    coordinates.insert(0, removed);
    binding(
        check_census(view.checked.input(), coordinates.iter().copied(), budget),
        "execution recipe complete operation census",
    );
    coordinates.remove(0);
    let mut output: Vec<_> = rows.iter().filter_map(|row| row.output).collect();
    output.sort_unstable();
    check_census(view.checked.output(), output.iter().copied(), budget)?;
    assert!(!output.is_empty());
    output.remove(0);
    binding(
        check_census(view.checked.output(), output.iter().copied(), budget),
        "execution recipe complete operation census",
    );
    assert!(ordered_disposition(removed, view.operation(removed, budget)?)?.is_some());
    binding(
        ordered_disposition(
            removed,
            ProductionOptimizedSourceOperationV18::Rewritten { input: removed },
        ),
        "execution recipe ordered operation rewritten",
    );

    let mut events_checked = 0;
    for root in &view.original.source.owner.inner.pending.roots {
        let mut sites = Vec::new();
        for (sidecar_index, sidecar) in root.sidecars.rows.iter().enumerate() {
            let events = sidecar.lifecycle_events.as_ref().unwrap();
            for (event_index, event) in events.rows.iter().enumerate() {
                sites.push((
                    events.instance.index(),
                    event.block.index(),
                    sidecar_index,
                    event_index,
                ));
            }
        }
        census::sort_unique(&mut sites, |row| [row.0, row.1 as usize], budget)?;
        if sites.is_empty() {
            continue;
        }
        let site = sites[0];
        assert_eq!(
            census::require_site(&sites, (site.0, site.1), budget)?,
            site
        );
        sites.remove(0);
        binding(
            census::require_site(&sites, (site.0, site.1), budget),
            "execution recipe missing original event",
        );
        sites.push(site);
        sites.push(site);
        binding(
            census::sort_unique(&mut sites, |row| [row.0, row.1 as usize], budget),
            "execution recipe duplicate original site or insertion",
        );
        let events = root.sidecars.rows[site.2]
            .lifecycle_events
            .as_ref()
            .unwrap();
        let event = events.rows[site.3];
        census::check_event(event.source, event.block, event)?;
        let mut foreign = event;
        foreign.block = SemanticBlockIdV1::from_index(u32::MAX);
        binding(
            census::check_event(event.source, event.block, foreign),
            "execution recipe foreign original event",
        );
        foreign = event;
        foreign.source = match event.source {
            DeferredLifecycleSourceV29::Issuance { .. } => {
                DeferredLifecycleSourceV29::Issuance { root: usize::MAX }
            }
            _ => DeferredLifecycleSourceV29::Return { event: usize::MAX },
        };
        binding(
            census::check_event(event.source, event.block, foreign),
            "execution recipe foreign original event",
        );
        let mut insertions: Vec<_> = root
            .insertions
            .iter()
            .enumerate()
            .map(|(index, row)| (row.instance.index(), row.event, index))
            .collect();
        census::sort_unique(&mut insertions, |row| [row.0, row.1], budget)?;
        let insertion_index = census::require_insertion(&insertions, (site.0, site.3), budget)?;
        let insertion = root.insertions[insertion_index];
        let coordinate = census::check_insertion(view, root, events, event, &insertion, budget)?;
        insertions.retain(|row| (row.0, row.1) != (site.0, site.3));
        binding(
            census::require_insertion(&insertions, (site.0, site.3), budget),
            "execution recipe missing original insertion",
        );
        insertions.extend([(site.0, site.3, insertion_index); 2]);
        binding(
            census::sort_unique(&mut insertions, |row| [row.0, row.1], budget),
            "execution recipe duplicate original site or insertion",
        );
        let mut foreign_insertion = insertion;
        foreign_insertion.source_span = usize::MAX;
        binding(
            census::check_insertion(view, root, events, event, &foreign_insertion, budget),
            "execution recipe original insertion span",
        );
        foreign_insertion = insertion;
        foreign_insertion.before.first = u32::MAX;
        binding(
            census::check_insertion(view, root, events, event, &foreign_insertion, budget),
            "execution recipe original insertion gap",
        );
        let input = view.checked.input();
        let operation =
            input.operations()[resources::operation_index(input, coordinate, budget)?].operation;
        check_payload(event.kind, operation, budget)?;
        let mut rewritten = operation.clone();
        rewritten.kind = OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(0));
        binding(
            check_payload(event.kind, &rewritten, budget),
            "execution recipe original operation payload",
        );
        rewritten = operation.clone();
        if rewritten.results.is_empty() {
            rewritten.results.push(ValueDef::new(
                ValueId(u32::MAX),
                Type::Execution(Role::Context),
            ));
        } else {
            rewritten.results[0].id = ValueId(u32::MAX);
        }
        binding(
            check_payload(event.kind, &rewritten, budget),
            "execution recipe original operation results",
        );
        events_checked += 1;
    }
    assert!(events_checked > 0);
    drop((rows, coordinates, output));
    budget.release_storage(budget.storage() - floor)?;
    Ok(())
}

#[test]
fn execution_recipe_index_lookup_has_exact_independent_work_cuts() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
    let sites = [(3usize, 7u32, 0usize, 0usize)];
    for limit in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = census::require_site(&sites, (3, 7), &mut budget);
        if limit == 0 {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Work(_)
                ))
            ));
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.failed_work(), Some(1));
        } else {
            assert_eq!(result.unwrap(), sites[0]);
            assert_eq!(budget.work(), 1);
        }
        assert_eq!(budget.storage(), 0);
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    binding(
        census::require_site(&[], (3, 7), &mut budget),
        "execution recipe missing original event",
    );
    assert_eq!(budget.work(), 1);
}
