// A name is usable only inside the complete source/actual boundary relation.
// It is not a memory leaf, a chosen predecessor, or a proof receipt.
use fe2o3_mir_model::{
    SsaBlockIdV1 as BoundaryBlockV31, SsaEdgeIdV1 as BoundaryEdgeV31,
    SsaVariableIdV1 as BoundaryVariableV31,
};

#[derive(Clone, Copy)]
struct SourceScalarBoundaryV31 {
    instance: usize,
    block: BoundaryBlockV31,
    variable: BoundaryVariableV31,
    ty: SemanticTypeIdV1,
    scalar: ProductionSemanticScalarTypeV2,
    definition: usize,
    value: ValueId,
    symbol: u32,
}

#[derive(Clone, Copy)]
struct SourceScalarBoundaryLookupV31 {
    key: [usize; 4],
    row: usize,
}

struct SourceScalarBoundariesV31 {
    rows: Vec<SourceScalarBoundaryV31>,
    lookup: Vec<SourceScalarBoundaryLookupV31>,
}
impl SourceScalarBoundariesV31 {
    fn empty() -> Self {
        Self {
            rows: Vec::new(),
            lookup: Vec::new(),
        }
    }
}

fn source_boundary_error_v31(
    error: fe2o3_kernel_analysis::SourceSsaBoundaryErrorV31,
) -> ProductionSourceOwnedViewErrorV18 {
    use fe2o3_kernel_analysis::SourceSsaBoundaryErrorV31 as Error;
    match error {
        Error::Resource(error) => error.into(),
        Error::Statement(_) | Error::ForeignPlan | Error::Panicked => {
            ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary equations refused")
        }
    }
}

fn source_boundary_headers_v31() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        SourceScalarBoundariesV31,
        SourceScalarBoundaryV31,
        SourceScalarBoundaryLookupV31,
        Vec<SourceScalarBoundaryV31>,
        Vec<SourceScalarBoundaryLookupV31>,
        Result<Vec<SourceScalarBoundaryV31>, ProductionSemanticKirErrorV1>,
        Result<Vec<SourceScalarBoundaryLookupV31>, ProductionSemanticKirErrorV1>,
        SourceOwnedResultV18<SourceScalarBoundariesV31>,
        SourceOwnedResultV18<Option<&'a SourceScalarBoundaryV31>>,
        SourceOwnedResultV18<ProductionSemanticExpressionV2>,
        SourceOwnedResultV18<Option<usize>>,
        [usize; 8],
        [usize; 4],
        &'a SourceScalarLeavesV18<'a, 'a>,
        &'a Function,
        &'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>,
        Type,
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}

fn source_boundary_derive_headers_v31() -> Result<usize, ArgumentResourceV1> {
    type Owner<'a> = fe2o3_kernel_analysis::SourceSsaBoundariesV31<'a>;
    type Receipt = fe2o3_kernel_analysis::SourceSsaBoundaryStorageV31;
    type Frame<'a, 'w> = (
        Vec<Vec<BoundaryBlockV31>>,
        Vec<BoundaryBlockV31>,
        Vec<fe2o3_kernel_analysis::SourceSsaBlockEventsV299>,
        Result<Vec<fe2o3_kernel_analysis::SourceSsaBlockEventsV299>, ProductionSemanticKirErrorV1>,
        fe2o3_kernel_analysis::SourceSsaBlockEventsV299,
        fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'a>,
        Option<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'a>>,
        Option<&'a [fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1]>,
        Option<fe2o3_pliron::ProductionSemanticSsaOccurrenceViewV1<'a>>,
        std::iter::Enumerate<
            std::slice::Iter<'a, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
        >,
        Option<usize>,
        [usize; 2],
        Owner<'a>,
        Receipt,
        Result<(Owner<'a>, Receipt), fe2o3_kernel_analysis::SourceSsaBoundaryErrorV31>,
        SourceOwnedResultV18<(Owner<'a>, Receipt)>,
        SourceOwnedResultV18<Vec<Vec<BoundaryBlockV31>>>,
        SourceOwnedResultV18<Vec<BoundaryBlockV31>>,
        Result<Vec<Vec<BoundaryBlockV31>>, ProductionSemanticKirErrorV1>,
        Result<Vec<BoundaryBlockV31>, ProductionSemanticKirErrorV1>,
        Result<EntryValueV20, fe2o3_kernel_analysis::SourceSsaBoundaryErrorV31>,
        SourceOwnedResultV18<EntryValueV20>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a SemanticFunctionDeclV1,
        &'a fe2o3_mir_model::SsaConstructionPlanV1,
        &'a mut Vec<SourceScalarBoundaryV31>,
        &'a mut ArgumentBudgetV1<'w>,
        [usize; 8],
        SourceOwnedResultV18<()>,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, '_>>(),
        std::mem::align_of::<Frame<'_, '_>>(),
    ])
}

fn source_boundary_scalar_v31(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    variable: BoundaryVariableV31,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<(SemanticTypeIdV1, ProductionSemanticScalarTypeV2)>> {
    budget.charge_work(3)?;
    let local = function.locals().get(variable.get() as usize).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary local is absent"),
    )?;
    if !matches!(
        types
            .get(local.ty().index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(_))
    ) {
        return Ok(None);
    }
    let lowered = lower_scalar_type(types, local.ty()).map_err(source_emission_error_v18)?;
    let scalar = kir_semantic_scalar_v1(&lowered).filter(|scalar| {
        matches!(
            scalar,
            ProductionSemanticScalarTypeV2::Bool
                | ProductionSemanticScalarTypeV2::Integer {
                    bits: 8 | 16 | 32 | 64,
                    ..
                }
        )
    });
    Ok(scalar.map(|scalar| (local.ty(), scalar)))
}

fn source_scalar_boundaries_v31(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    memory: &[SourceScalarLeafRowV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SourceScalarBoundariesV31> {
    relation.retain_query((|| {
        budget.reserve_storage(source_boundary_headers_v31()?)?;
        let owner = relation.source.source_ssa(budget)?;
        let semantic = owner.source_semantic();
        let root_row = relation.source.root_row(root)?;
        let mut capacity = 0usize;
        for source in &root_row.coordinates.sources.rows {
            budget.charge_work(2)?;
            if !relation.source.instance_active(root, source.instance.index(), budget)? { continue; }
            let plan = owner.plan_for_function(source.function).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary plan is absent"))?.plan();
            for block in plan.reverse_postorder() {
                budget.charge_work(1)?;
                capacity = argument_sum_v1(&[capacity, plan.transport_variables(*block).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary block is absent"))?.len()])?;
            }
        }
        let mut rows = emission_vec_v1(capacity, budget).map_err(source_emission_error_v18)?;
        if capacity == 0 { return Ok(SourceScalarBoundariesV31 { rows, lookup: Vec::new() }); }
        let mut next = match memory.last() {
            Some(row) => row.symbol.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?,
            None => 0,
        };
        for source in &root_row.coordinates.sources.rows {
            budget.charge_work(2)?;
            let instance = source.instance.index();
            if !relation.source.instance_active(root, instance, budget)? { continue; }
            let plan = owner.plan_for_function(source.function).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary plan is absent"))?.plan();
            let function = semantic.functions().get(source.function.index() as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary function is absent"))?;
            let occurrences = owner.occurrences_v1().and_then(|rows| rows.function(source.function)).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary occurrences are absent"))?;
            let capture = (&mut rows, &mut next, relation, root, instance, plan, function, semantic.types(), occurrences);
            let run = move |budget: &mut ArgumentBudgetV1<'_>| {
                let (rows, next, relation, root, instance, plan, function, types, occurrences) = capture;
                relation.retain_query((|| {
                    budget.charge_work(3)?;
                    if occurrences.block_count_v299() != function.blocks().len()
                        || !occurrences.owner().plan_for_function(occurrences.function())
                            .is_some_and(|row| std::ptr::eq(row.plan(), plan)) {
                        return relation.source.missing("source scalar boundary occurrence owner differs");
                    }
                    let mut topology = emission_vec_v1(function.blocks().len(), budget).map_err(source_emission_error_v18)?;
                    let mut failure_roster = emission_vec_v1(function.blocks().len(), budget).map_err(source_emission_error_v18)?;
                    for (index, block) in function.blocks().iter().enumerate() {
                        budget.charge_work(3)?;
                        let id = BoundaryBlockV31::new(u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?);
                        let events = occurrences.block_events_v299(id).ok_or(
                            ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary event block is absent"))?;
                        failure_roster.push(fe2o3_kernel_analysis::SourceSsaBlockEventsV299 {
                            events: events.len(), terminal_failure_start: occurrences.terminal_failure_start(id),
                        });
                        let mut edges = emission_vec_v1(block.terminator().kind().edge_count(), budget).map_err(source_emission_error_v18)?;
                        block.terminator().kind().try_for_each_edge(|edge| {
                            budget.charge_work(1)?;
                            edges.push(BoundaryBlockV31::new(edge.target().index()));
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        })?;
                        topology.push(edges);
                    }
                    let (boundaries, receipt) = fe2o3_kernel_analysis::SourceSsaBoundariesV31::derive_with_terminal_failures_v299(
                        plan, BoundaryBlockV31::new(function.entry().index()), &topology, &failure_roster, budget).map_err(source_boundary_error_v31)?;
                    budget.reserve_storage(receipt.retained_storage())?;
                    for block in plan.reverse_postorder() {
                        for variable in plan.transport_variables(*block).ok_or(
                            ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary block is absent"))? {
                            budget.charge_work(4)?;
                            let Some((ty, scalar)) = source_boundary_scalar_v31(types, function, *variable, budget)? else { continue; };
                            let original = EntryValueV20::BlockArgument { block: *block, variable: *variable };
                            if boundaries.value(plan, *block, *variable, budget).map_err(source_boundary_error_v31)? != original {
                                return relation.source.missing("source scalar boundary is not its original live-in name");
                            }
                            let definition = relation.ssa_scalar_definition_v30(root, instance, original, budget)?.ok_or(
                                ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary has a unit binding"))?;
                            let actual = relation.inventory.definitions().get(definition).ok_or(
                                ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary definition is absent"))?;
                            let entry = relation.source_block_entry(root, instance, SemanticBlockIdV1::from_index(block.get()), budget)?.ok_or(
                                ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary is unreachable"))?;
                            if !matches!(actual.coordinate, fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, .. } if block == entry)
                                || kir_semantic_scalar_v1(actual.ty) != Some(scalar) {
                                return relation.source.missing("source scalar boundary physical parameter differs");
                            }
                            let value = actual.value.ok_or(ProductionSourceOwnedViewErrorV18::Binding("source scalar boundary value is absent"))?;
                            if *next >= PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 || rows.len() == rows.capacity() {
                                return relation.source.missing("source scalar boundary census or name capacity differs");
                            }
                            rows.push(SourceScalarBoundaryV31 { instance, block: *block, variable: *variable, ty, scalar, definition, value, symbol: *next });
                            *next = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                        }
                    }
                    drop(boundaries);
                    drop(failure_roster);
                    drop(topology);
                    Ok(())
                })())
            };
            let headers = argument_sum_v1(&[source_boundary_derive_headers_v31()?, std::mem::size_of_val(&run), std::mem::align_of_val(&run)])?;
            source_scalar_normalization_scratch_v18(relation.source.cleanup, budget, headers, run)?;
        }
        if rows.is_empty() { return Ok(SourceScalarBoundariesV31 { rows, lookup: Vec::new() }); }
        let physical = relation.source.root(root, budget)?.1;
        let function = relation.inventory.functions().get(physical).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source scalar forwarding function is absent"))?;
        let lookup_count = argument_sum_v1(&[argument_product_v1(rows.len(), 4)?, function.definitions.len()])?;
        let mut lookup = emission_vec_v1(lookup_count, budget).map_err(source_emission_error_v18)?;
        for (row, value) in rows.iter().enumerate() {
            budget.charge_work(4)?;
            for key in [
                [0, value.instance, value.block.get() as usize, value.variable.get() as usize],
                [1, value.value.0 as usize, 0, 0], [2, value.symbol as usize, 0, 0],
                [3, value.definition, 0, 0],
            ] { lookup.push(SourceScalarBoundaryLookupV31 { key, row }); }
        }
        source_scalar_normalization_scratch_v18(relation.source.cleanup, budget,
            source_boundary_forwarding_headers_v32()?, |budget| {
                let mut terminals = emission_vec_v1(rows.len(), budget).map_err(source_emission_error_v18)?;
                for (at, row) in rows.iter().enumerate() {
                    budget.charge_work(1)?;
                    terminals.push((row.definition, at));
                }
                let forwarded = source_boundary_forwarding_v32(relation.inventory, function.coordinate, &terminals, budget)?;
                for row in &forwarded {
                    budget.charge_work(2)?;
                    if lookup.len() == lookup.capacity() { return Err(ArgumentResourceV1::Accounting.into()); }
                    lookup.push(SourceScalarBoundaryLookupV31 { key: [4, row.value.0 as usize, 0, 0], row: row.original });
                }
                drop((forwarded, terminals));
                Ok(())
            })?;
        private_array_heapsort_v1(&mut lookup, |row| row.key, &mut SourceCorrespondenceWorkV18(budget), || ArgumentResourceV1::Arithmetic.into())?;
        for pair in lookup.windows(2) {
            budget.charge_work(1)?;
            if pair[0].key == pair[1].key { return relation.source.missing("source scalar boundary names are ambiguous"); }
        }
        Ok(SourceScalarBoundariesV31 { rows, lookup })
    })())
}

impl SourceScalarLeavesV18<'_, '_> {
    fn boundary_find_v31(
        &self,
        key: [usize; 4],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&SourceScalarBoundaryV31>> {
        self.relation.retain_query((|| {
            self.query(budget)?;
            budget.charge_work(
                self.boundaries.lookup.len().checked_ilog2().unwrap_or(0) as usize + 4,
            )?;
            let Ok(at) = self
                .boundaries
                .lookup
                .binary_search_by_key(&key, |row| row.key)
            else {
                return Ok(None);
            };
            let row = self
                .boundaries
                .rows
                .get(self.boundaries.lookup[at].row)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source scalar boundary lookup row differs",
                ))?;
            let expected = match key[0] {
                0 => [
                    0,
                    row.instance,
                    row.block.get() as usize,
                    row.variable.get() as usize,
                ],
                1 => [1, row.value.0 as usize, 0, 0],
                2 => [2, row.symbol as usize, 0, 0],
                3 => [3, row.definition, 0, 0],
                4 => [4, key[1], 0, 0],
                _ => {
                    return self
                        .relation
                        .source
                        .missing("source scalar boundary lookup family differs");
                }
            };
            let definition = self
                .relation
                .inventory
                .definitions()
                .get(row.definition)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source scalar boundary definition differs",
                ))?;
            if key != expected
                || definition.value != Some(row.value)
                || kir_semantic_scalar_v1(definition.ty) != Some(row.scalar)
            {
                return self
                    .relation
                    .source
                    .missing("source scalar boundary lookup changed its exact binding");
            }
            if key[0] == 4 {
                let physical = self.relation.source.root(self.root, budget)?.1;
                let function = self.relation.inventory.functions().get(physical).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding("source scalar forwarding function is absent"))?;
                let value = ValueId(u32::try_from(key[1]).map_err(|_| ArgumentResourceV1::Arithmetic)?);
                let actual = self.relation.inventory.definition_for_value(function.coordinate, value, budget)
                    .map_err(source_pointer_inventory_error_v18)?.ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding("source scalar forwarded definition is absent"))?;
                if kir_semantic_scalar_v1(actual.ty) != Some(row.scalar)
                    || !matches!(actual.coordinate, fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, .. } if block.function == function.coordinate) {
                    return self.relation.source.missing("source scalar forwarded parameter differs");
                }
            }
            Ok(Some(row))
        })())
    }
}

include!("production_source_scalar_boundary_forwarding_v32.rs");

impl ProductionSourceScalarLeavesV18<'_> {
    fn boundary_expression_v31(
        &self,
        instance: usize,
        block: BoundaryBlockV31,
        variable: BoundaryVariableV31,
        ty: SemanticTypeIdV1,
        scalar: ProductionSemanticScalarTypeV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        let row = self
            .leaves
            .boundary_find_v31(
                [0, instance, block.get() as usize, variable.get() as usize],
                budget,
            )?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "private entry requires unsupported SSA block argument",
            ))?;
        if row.ty != ty || row.scalar != scalar {
            return self
                .leaves
                .relation
                .source
                .missing("source scalar boundary original type differs");
        }
        Ok(ProductionSemanticExpressionV2::Symbol {
            symbol: row.symbol,
            scalar,
        })
    }
}

include!("production_source_scalar_boundary_control_v31.rs");
