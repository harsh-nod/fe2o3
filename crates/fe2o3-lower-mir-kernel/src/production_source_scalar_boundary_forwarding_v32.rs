// Checked source boundaries are opaque terminals. Every other block parameter
// must have the same terminal on all syntactic incoming edges. Incoming edges
// of a terminal are checked separately by the source boundary equations.
fn source_boundary_forwarding_headers_v32() -> Result<usize, ArgumentResourceV1> {
    type State = origin_worklist_v1::OriginStateV1<usize>;
    type Work = origin_worklist_v1::OriginWorkV1<usize>;
    type Frame<'a, 'w> = (
        Work,
        Result<Work, origin_worklist_v1::OriginWorkErrorV1>,
        Vec<State>,
        Result<Vec<State>, origin_worklist_v1::OriginWorkErrorV1>,
        Vec<Option<usize>>,
        Vec<(usize, usize)>,
        Vec<OptimizedSourceScalarBoundaryV31>,
        Result<Vec<Option<usize>>, ProductionSemanticKirErrorV1>,
        Result<Vec<(usize, usize)>, ProductionSemanticKirErrorV1>,
        Result<Vec<OptimizedSourceScalarBoundaryV31>, ProductionSemanticKirErrorV1>,
        SourceOwnedResultV18<Vec<OptimizedSourceScalarBoundaryV31>>,
        [Result<(), origin_worklist_v1::OriginWorkErrorV1>; 3],
        [State; 3],
        [usize; 20],
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1,
        &'a [(usize, usize)],
        &'a mut ArgumentBudgetV1<'w>,
        std::ops::Range<usize>,
        SourceOwnedResultV18<()>,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, '_>>(),
        std::mem::align_of::<Frame<'_, '_>>(),
    ])
}

fn source_boundary_origin_error_v32(
    error: origin_worklist_v1::OriginWorkErrorV1,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        origin_worklist_v1::OriginWorkErrorV1::Resource(error) => error.into(),
        origin_worklist_v1::OriginWorkErrorV1::Shape => {
            ProductionSourceOwnedViewErrorV18::Binding("source scalar forwarding graph differs")
        }
    }
}

fn source_boundary_forwarding_v32(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    terminals: &[(usize, usize)],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<OptimizedSourceScalarBoundaryV31>> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    use origin_worklist_v1::{OriginStateV1 as State, OriginWorkV1 as Work};
    budget.charge_work(3)?;
    let row = inventory
        .functions()
        .get(function.0 as usize)
        .filter(|row| row.coordinate == function && row.function.body.is_some())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source scalar forwarding function differs",
        ))?;
    let range = row.definitions.clone();
    let definitions = inventory.definitions().get(range.clone()).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding(
            "source scalar forwarding definition range differs",
        ),
    )?;
    let edges = inventory
        .edge_arguments()
        .get(row.edge_arguments.clone())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source scalar forwarding edge range differs",
        ))?;
    let mut seeds =
        emission_vec_v1(definitions.len(), budget).map_err(source_emission_error_v18)?;
    budget.charge_work(definitions.len())?;
    seeds.resize(definitions.len(), None);
    for &(definition, original) in terminals {
        budget.charge_work(3)?;
        let local = definition
            .checked_sub(range.start)
            .filter(|at| *at < range.len())
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source scalar forwarding terminal belongs to another function",
            ))?;
        if !matches!(definitions[local].coordinate, Definition::BlockArgument { block, .. } if block.function == function)
            || seeds[local].replace(original).is_some()
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source scalar forwarding terminal repeats or is not a block argument",
            ));
        }
    }
    let mut count = 0;
    for edge in edges {
        budget.charge_work(4)?;
        if !range.contains(&edge.incoming_definition)
            || !range.contains(&edge.target_definition)
            || edge.coordinate.edge.source.function != function
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source scalar forwarding edge belongs to another function",
            ));
        }
        let source = edge.incoming_definition - range.start;
        let target = edge.target_definition - range.start;
        if !matches!(definitions[target].coordinate, Definition::BlockArgument { block, .. } if block.function == function)
            || definitions[source].ty != definitions[target].ty
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source scalar forwarding edge type or target differs",
            ));
        }
        if seeds[target].is_none() {
            count = argument_sum_v1(&[count, 1])?;
        }
    }
    let mut work =
        Work::new(definitions.len(), count, budget).map_err(source_boundary_origin_error_v32)?;
    for (at, definition) in definitions.iter().enumerate() {
        budget.charge_work(2)?;
        let (actual, state) = match definition.coordinate {
            Definition::BlockArgument { block, .. } => (
                block.function,
                match seeds[at] {
                    Some(original) => State::Exact(original),
                    None if block.block == 0 => State::Unknown,
                    None => State::Pending,
                },
            ),
            Definition::FunctionArgument { function, .. } => (function, State::Unknown),
            Definition::Result { operation, .. } => (operation.block.function, State::Unknown),
        };
        if actual != function {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source scalar forwarding definition belongs to another function",
            ));
        }
        work.seed_next(state, budget)
            .map_err(source_boundary_origin_error_v32)?;
    }
    for edge in edges {
        budget.charge_work(2)?;
        let target = edge.target_definition - range.start;
        if seeds[target].is_none() {
            work.add_link(edge.incoming_definition - range.start, target, budget)
                .map_err(source_boundary_origin_error_v32)?;
        }
    }
    let solved = work
        .solve(budget)
        .map_err(source_boundary_origin_error_v32)?;
    let mut values =
        emission_vec_v1(definitions.len(), budget).map_err(source_emission_error_v18)?;
    for (at, state) in solved.iter().enumerate() {
        budget.charge_work(2)?;
        if seeds[at].is_some() {
            continue;
        }
        if let State::Exact(original) = state {
            let value = definitions[at]
                .value
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source scalar forwarded parameter has no value",
                ))?;
            values.push(OptimizedSourceScalarBoundaryV31 {
                value,
                original: *original,
                definition: range.start + at,
            });
        }
    }
    Ok(values)
}
