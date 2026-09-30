// Static Project creates a derived pointer, not a memory access or a fresh
// activation. The ordinary currentness equations must still propagate the
// base's lifetime into the result; this check never adds a birth seed.
fn optimized_source_project_headers_v33() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<Vec<bool>>(),
        size_of::<OptimizedSourceObjectCensusV18>(),
        size_of::<SourcePhysicalObjectV18<'_>>(),
        size_of::<ScopedObjectPayloadV29>(),
        size_of::<TileAttachmentKeyV29>(),
        argument_product_v1(
            2,
            size_of::<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>(),
        )?,
        argument_product_v1(2, size_of::<ValueId>())?,
        argument_product_v1(4, size_of::<usize>())?,
        argument_product_v1(2, size_of::<SourceStaticObjectLocationV29>())?,
        size_of::<&SourceAddressMemoryV29<'_>>(),
        size_of::<&mut [bool]>(),
        size_of::<SourceOwnedResultV18<()>>(),
    ])
}

fn check_optimized_source_project_equation_v33(
    source: SourceStaticObjectLocationV29,
    projected: SourceStaticObjectLocationV29,
    graph: &SourceAddressMemoryV29<'_>,
    base: ValueId,
    result: ValueId,
    seen: &mut [bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(3)?;
    if seen.len() != graph.projections.len()
        || graph
            .object_location(base, budget)
            .map_err(immutable_memory_error_v29)?
            != source
        || graph
            .object_location(result, budget)
            .map_err(immutable_memory_error_v29)?
            != projected
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized Project changed source subobject geometry",
        ));
    }
    let node = graph
        .value(result, budget)
        .map_err(immutable_memory_error_v29)?;
    budget.charge_work(call_splice_search_work_v1(graph.projections.len()))?;
    let index = graph
        .projections
        .binary_search_by_key(&node, |row| row.0)
        .map_err(|_| {
            ProductionSourceOwnedViewErrorV18::Binding(
                "optimized Project lacks actual pointer equation",
            )
        })?;
    budget.charge_work(1)?;
    if std::mem::replace(&mut seen[index], true) {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized Project repeats actual pointer equation",
        ));
    }
    Ok(())
}

fn finish_optimized_source_project_equations_v33(
    seen: &[bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(seen.len())?;
    if seen.iter().any(|seen| !seen) {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized Project lacks checked original source role",
        ));
    }
    Ok(())
}

fn check_optimized_source_projects_v33(
    original: &CheckedSourceMemoryV29<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    graph: &SourceAddressMemoryV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<OptimizedSourceObjectCensusV18> {
    original.check(budget)?;
    let relation = original.correspondence;
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    let header = optimized_source_project_headers_v33()?;
    budget.reserve_storage(header)?;
    let mut seen =
        emission_vec_v1(graph.projections.len(), budget).map_err(immutable_memory_error_v29)?;
    budget.charge_work(graph.projections.len())?;
    seen.resize(graph.projections.len(), false);
    let mut census = OptimizedSourceObjectCensusV18::default();
    for row in &original.pending.projects {
        budget.charge_work(1)?;
        let key = TileAttachmentKeyV29 {
            root: original.root,
            family: TileAttachmentFamilyV29::MemoryAnchor,
            instance: row.instance.index(),
            row: row.anchor,
            field: TileAttachmentFieldV29::MemoryPosition,
            component: 0,
            part: 0,
        };
        let [position] = relation.attachment_range(key, budget)? else {
            return relation
                .source
                .missing("optimized Project exact original position");
        };
        let ProductionSourceOperationV18::Operation(input) =
            relation.mapped_source_operation(position.location, budget)?
        else {
            return relation
                .source
                .missing("optimized Project original operation");
        };
        let source = relation.retained_object_payload_at_v29(
            original.root,
            row.instance.index(),
            row.anchor,
            input,
            budget,
        )?;
        if !matches!(
            source.actual.operation,
            ScopedObjectOperationV29::Project {
                step: ScopedObjectProjectionV29::Field(_)
                    | ScopedObjectProjectionV29::ArrayIndex(_),
                ..
            }
        ) {
            return relation
                .source
                .missing("optimized Project changed original zero-footprint role");
        }
        let output = match optimized.operation(input, budget)? {
            ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
            ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                census.unreachable = argument_sum_v1(&[census.unreachable, 1])?;
                continue;
            }
            ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                return relation
                    .source
                    .missing("optimized Project lacks exact retained equation");
            }
        };
        let actual = optimized_source_object_payload_v18(
            relation,
            optimized,
            input,
            output,
            &source.actual,
            budget,
        )?;
        let ScopedObjectOperationV29::Project { base, .. } = actual.operation else {
            return relation
                .source
                .missing("optimized Project changed actual operation");
        };
        let result = actual
            .result
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized Project actual result",
            ))?;
        check_optimized_source_project_equation_v33(
            row.source,
            row.projected,
            graph,
            base,
            result,
            &mut seen,
            budget,
        )?;
        census.retained = argument_sum_v1(&[census.retained, 1])?;
    }
    finish_optimized_source_project_equations_v33(&seen, budget)?;
    let bytes = argument_sum_v1(&[
        header,
        argument_product_v1(seen.capacity(), size_of::<bool>())?,
    ])?;
    drop(seen);
    budget.release_storage(bytes)?;
    Ok(census)
}

#[cfg(test)]
pub(super) fn test_optimized_project_headers_v33() -> Result<usize, ArgumentResourceV1> {
    optimized_source_project_headers_v33()
}

#[cfg(test)]
pub(super) fn test_optimized_project_equation_v33(
    source: SourceStaticObjectLocationV29,
    projected: SourceStaticObjectLocationV29,
    graph: &SourceAddressMemoryV29<'_>,
    base: ValueId,
    result: ValueId,
    seen: &mut [bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    check_optimized_source_project_equation_v33(
        source, projected, graph, base, result, seen, budget,
    )
}

#[cfg(test)]
pub(super) fn test_finish_optimized_project_equations_v33(
    seen: &[bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    finish_optimized_source_project_equations_v33(seen, budget)
}

#[cfg(test)]
mod project_header_tests_v33 {
    use super::*;

    #[test]
    fn optimized_project_header_is_independent_exact_and_one_short() {
        let expected = size_of::<Vec<bool>>()
            + size_of::<OptimizedSourceObjectCensusV18>()
            + size_of::<SourcePhysicalObjectV18<'_>>()
            + size_of::<ScopedObjectPayloadV29>()
            + size_of::<TileAttachmentKeyV29>()
            + 2 * size_of::<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>()
            + 2 * size_of::<ValueId>()
            + 4 * size_of::<usize>()
            + 2 * size_of::<SourceStaticObjectLocationV29>()
            + size_of::<&SourceAddressMemoryV29<'_>>()
            + size_of::<&mut [bool]>()
            + size_of::<SourceOwnedResultV18<()>>();
        assert_eq!(optimized_source_project_headers_v33().unwrap(), expected);
        for limit in [expected, expected - 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = ArgumentBudgetV1::new(&mut work, limit);
            let result = budget.reserve_storage(optimized_source_project_headers_v33().unwrap());
            assert_eq!(result.is_ok(), limit == expected);
            assert_eq!(budget.work(), 0);
            if result.is_ok() {
                budget.release_storage(expected).unwrap();
            }
            assert_eq!(budget.storage(), 0);
        }
    }
}
