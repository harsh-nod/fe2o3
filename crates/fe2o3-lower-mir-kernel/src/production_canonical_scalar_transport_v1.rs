/// Inert operation attribution; synthesized constants inherit no operation grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionCanonicalScalarOperationOriginV1 {
    /// The exact surviving original-N operation, not a union of eliminated sites.
    Original(CsOperationV1),
    /// Actual first synthesis site in the retained adjacent history.
    SynthesizedConstant {
        /// Zero-based real fixed-point round.
        round: u16,
        /// True for the fixed integer substage, false for its scalar successor.
        integer: bool,
        /// Exact input definition of that synthesis substage, not an N coordinate.
        definition: CsDefinitionV1,
    },
}

/// One exact original block and its connector in a final ordered merge chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCanonicalScalarBlockSegmentV1 {
    /// Original-N block coordinate.
    pub original: CsBlockV1,
    /// Original-N successor occurrence; None only at the end of this chain.
    pub connector: Option<CsEdgeV1>,
}
/// Complete original block placement; omission is not a source-obligation grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCanonicalScalarBlockControlV1 {
    /// Actual original-N block.
    pub original: CsBlockV1,
    /// Actual final block and position in its composed original-block chain.
    pub placement: Option<fe2o3_kernel_analysis::CanonicalKirBlockPlacementV1>,
    /// Reachability allowed by every checked adjacent control relation.
    pub reachable: bool,
}
/// Complete original successor placement, preserving duplicate destinations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCanonicalScalarEdgeControlV1 {
    /// Actual original-N successor occurrence.
    pub original: CsEdgeV1,
    /// Final retained edge, exact internal connector, or omitted occurrence.
    pub placement: fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1,
    /// Execution allowed by every checked adjacent control relation.
    pub executable: bool,
}
/// One value descendant, separate from surviving operation-source attribution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCanonicalScalarDefinitionDescendantV1 {
    /// Original-N definition, including definitions with other descendants.
    pub original: CsDefinitionV1,
    /// Actual final definition.
    pub output: CsDefinitionV1,
    /// Retained only if every link in this exact chain retained the identity.
    pub kind: fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CsDefinitionLinkV1 {
    original: usize,
    output: usize,
    retained: bool,
}
struct CsLineageV1 {
    functions: Vec<CsFunctionV1>,
    operations: Vec<ProductionCanonicalScalarOperationOriginV1>,
    definitions: Vec<CsDefinitionLinkV1>,
    ancestors: Vec<std::ops::Range<usize>>,
    blocks: Vec<std::ops::Range<usize>>,
    segments: Vec<ProductionCanonicalScalarBlockSegmentV1>,
    block_controls: Vec<ProductionCanonicalScalarBlockControlV1>,
    edge_controls: Vec<ProductionCanonicalScalarEdgeControlV1>,
    uses: Vec<CsUseV1>,
    edges: Vec<CsEdgeV1>,
    arguments: Vec<CsEdgeArgumentV1>,
    storage: usize,
}
impl CsLineageV1 {
    fn empty(
        actual: &CanonicalKirInventoryV1<'_>,
        original: &CanonicalKirInventoryV1<'_>,
        segments: usize,
        definitions: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self> {
        budget.charge_work(1)?;
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        Ok(Self {
            functions: cs_vec_v1(actual.functions().len(), budget)?,
            operations: cs_vec_v1(actual.operations().len(), budget)?,
            definitions: cs_vec_v1(definitions, budget)?,
            ancestors: cs_vec_v1(actual.definitions().len(), budget)?,
            blocks: cs_vec_v1(actual.blocks().len(), budget)?,
            segments: cs_vec_v1(segments, budget)?,
            block_controls: cs_vec_v1(original.blocks().len(), budget)?,
            edge_controls: cs_vec_v1(original.edges().len(), budget)?,
            uses: cs_vec_v1(actual.uses().len(), budget)?,
            edges: cs_vec_v1(actual.edges().len(), budget)?,
            arguments: cs_vec_v1(actual.edge_arguments().len(), budget)?,
            storage: 0,
        })
    }
    fn finish(&mut self, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<()> {
        budget.charge_work(12)?;
        self.storage = argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            cs_extent_v1(&self.functions)?,
            cs_extent_v1(&self.operations)?,
            cs_extent_v1(&self.definitions)?,
            cs_extent_v1(&self.ancestors)?,
            cs_extent_v1(&self.blocks)?,
            cs_extent_v1(&self.segments)?,
            cs_extent_v1(&self.block_controls)?,
            cs_extent_v1(&self.edge_controls)?,
            cs_extent_v1(&self.uses)?,
            cs_extent_v1(&self.edges)?,
            cs_extent_v1(&self.arguments)?,
        ])?;
        Ok(())
    }
    fn shape(
        &self,
        actual: &CanonicalKirInventoryV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<()> {
        budget.charge_work(8)?;
        if self.functions.len() != actual.functions().len()
            || self.operations.len() != actual.operations().len()
            || self.ancestors.len() != actual.definitions().len()
            || self.blocks.len() != actual.blocks().len()
            || self.uses.len() != actual.uses().len()
            || self.edges.len() != actual.edges().len()
            || self.arguments.len() != actual.edge_arguments().len()
        {
            return Err(cs_invalid_v1("complete current occurrence rosters"));
        }
        Ok(())
    }
}

fn cs_at_v1(
    range: &std::ops::Range<usize>,
    ordinal: u32,
    bound: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<usize> {
    budget.charge_work(1)?;
    let offset = usize::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    let index = range
        .start
        .checked_add(offset)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    if range.start > range.end || range.end > bound || index >= range.end {
        return Err(cs_invalid_v1("coordinate extent"));
    }
    Ok(index)
}
fn cs_function_v1(
    i: &CanonicalKirInventoryV1<'_>,
    c: CsFunctionV1,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<usize> {
    b.charge_work(2)?;
    let n = usize::try_from(c.0).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    if i.functions().get(n).is_none_or(|r| r.coordinate != c) {
        return Err(cs_invalid_v1("function coordinate"));
    }
    Ok(n)
}
fn cs_block_v1(
    i: &CanonicalKirInventoryV1<'_>,
    c: CsBlockV1,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<usize> {
    b.charge_work(1)?;
    let f = cs_function_v1(i, c.function, b)?;
    let n = cs_at_v1(&i.functions()[f].blocks, c.block, i.blocks().len(), b)?;
    if i.blocks()[n].coordinate != c {
        return Err(cs_invalid_v1("block coordinate"));
    }
    Ok(n)
}
fn cs_operation_v1(
    i: &CanonicalKirInventoryV1<'_>,
    c: CsOperationV1,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<usize> {
    b.charge_work(1)?;
    let p = cs_block_v1(i, c.block, b)?;
    let n = cs_at_v1(
        &i.blocks()[p].operations,
        c.operation,
        i.operations().len(),
        b,
    )?;
    if i.operations()[n].coordinate != c {
        return Err(cs_invalid_v1("operation coordinate"));
    }
    Ok(n)
}
fn cs_definition_v1(
    i: &CanonicalKirInventoryV1<'_>,
    c: CsDefinitionV1,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<usize> {
    b.charge_work(1)?;
    let n = match c {
        CsDefinitionV1::FunctionArgument { function, argument } => {
            let f = cs_function_v1(i, function, b)?;
            if usize::try_from(argument).map_err(|_| ArgumentResourceV1::Arithmetic)?
                >= i.functions()[f].function.signature.parameters.len()
            {
                return Err(cs_invalid_v1("function argument"));
            }
            cs_at_v1(
                &i.functions()[f].definitions,
                argument,
                i.definitions().len(),
                b,
            )?
        }
        CsDefinitionV1::BlockArgument { block, argument } => {
            let p = cs_block_v1(i, block, b)?;
            cs_at_v1(
                &i.blocks()[p].parameters,
                argument,
                i.definitions().len(),
                b,
            )?
        }
        CsDefinitionV1::Result { operation, result } => {
            let p = cs_operation_v1(i, operation, b)?;
            cs_at_v1(&i.operations()[p].results, result, i.definitions().len(), b)?
        }
    };
    if i.definitions()[n].coordinate != c {
        return Err(cs_invalid_v1("definition coordinate"));
    }
    Ok(n)
}
fn cs_use_v1(
    i: &CanonicalKirInventoryV1<'_>,
    c: CsUseV1,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<usize> {
    b.charge_work(1)?;
    let n = match c {
        CsUseV1::OperationOperand { operation, operand } => {
            let p = cs_operation_v1(i, operation, b)?;
            cs_at_v1(&i.operations()[p].operands, operand, i.uses().len(), b)?
        }
        CsUseV1::TerminatorOperand { block, operand } => {
            let p = cs_block_v1(i, block, b)?;
            cs_at_v1(&i.blocks()[p].terminator_uses, operand, i.uses().len(), b)?
        }
    };
    if i.uses()[n].coordinate != c {
        return Err(cs_invalid_v1("use coordinate"));
    }
    Ok(n)
}
fn cs_edge_v1(
    i: &CanonicalKirInventoryV1<'_>,
    c: CsEdgeV1,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<usize> {
    b.charge_work(1)?;
    let p = cs_block_v1(i, c.source, b)?;
    let n = cs_at_v1(&i.blocks()[p].edges, c.successor, i.edges().len(), b)?;
    if i.edges()[n].coordinate != c {
        return Err(cs_invalid_v1("edge coordinate"));
    }
    Ok(n)
}
fn cs_argument_v1(
    i: &CanonicalKirInventoryV1<'_>,
    c: CsEdgeArgumentV1,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<usize> {
    b.charge_work(1)?;
    let p = cs_edge_v1(i, c.edge, b)?;
    let n = cs_at_v1(
        &i.edges()[p].bindings,
        c.argument,
        i.edge_arguments().len(),
        b,
    )?;
    if i.edge_arguments()[n].coordinate != c {
        return Err(cs_invalid_v1("edge argument coordinate"));
    }
    Ok(n)
}
fn cs_range_v1(
    r: fe2o3_kernel_ir::CanonicalKirTransitionRangeV1,
    bound: usize,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<std::ops::Range<usize>> {
    b.charge_work(1)?;
    let start = usize::try_from(r.start).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    let end = start
        .checked_add(usize::try_from(r.len).map_err(|_| ArgumentResourceV1::Arithmetic)?)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    if end > bound {
        return Err(cs_invalid_v1("transition range"));
    }
    Ok(start..end)
}

fn cs_identity_v1(
    i: &CanonicalKirInventoryV1<'_>,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<CsLineageV1> {
    let mut out = CsLineageV1::empty(i, i, i.blocks().len(), i.definitions().len(), b)?;
    for r in i.functions() {
        cs_push_v1(&mut out.functions, r.coordinate, b)?;
    }
    for r in i.operations() {
        cs_push_v1(
            &mut out.operations,
            ProductionCanonicalScalarOperationOriginV1::Original(r.coordinate),
            b,
        )?;
    }
    for n in 0..i.definitions().len() {
        cs_push_v1(
            &mut out.definitions,
            CsDefinitionLinkV1 {
                original: n,
                output: n,
                retained: true,
            },
            b,
        )?;
        cs_push_v1(&mut out.ancestors, n..n + 1, b)?;
    }
    for (n, r) in i.blocks().iter().enumerate() {
        cs_push_v1(&mut out.blocks, n..n + 1, b)?;
        cs_push_v1(
            &mut out.segments,
            ProductionCanonicalScalarBlockSegmentV1 {
                original: r.coordinate,
                connector: None,
            },
            b,
        )?;
        cs_push_v1(
            &mut out.block_controls,
            ProductionCanonicalScalarBlockControlV1 {
                original: r.coordinate,
                placement: Some(fe2o3_kernel_analysis::CanonicalKirBlockPlacementV1 {
                    output: r.coordinate,
                    segment: 0,
                }),
                reachable: true,
            },
            b,
        )?;
    }
    for r in i.uses() {
        cs_push_v1(&mut out.uses, r.coordinate, b)?;
    }
    for r in i.edges() {
        cs_push_v1(&mut out.edges, r.coordinate, b)?;
        cs_push_v1(
            &mut out.edge_controls,
            ProductionCanonicalScalarEdgeControlV1 {
                original: r.coordinate,
                placement: fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Retained(
                    r.coordinate,
                ),
                executable: true,
            },
            b,
        )?;
    }
    for r in i.edge_arguments() {
        cs_push_v1(&mut out.arguments, r.coordinate, b)?;
    }
    out.finish(b)?;
    Ok(out)
}

fn cs_definition_fanout_v1(
    before: &CanonicalKirInventoryV1<'_>,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    old: &CsLineageV1,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<usize> {
    let mut count = 0usize;
    for (n, r) in rows.definitions.iter().enumerate() {
        b.charge_work(3)?;
        if before.definitions()[n].coordinate != r.input {
            return Err(cs_invalid_v1("definition input order"));
        }
        let range = cs_range_v1(r.outputs, rows.definition_outputs.len(), b)?;
        count = argument_sum_v1(&[
            count,
            argument_product_v1(old.ancestors[n].len(), range.len())?,
        ])?;
    }
    Ok(count)
}
fn cs_definitions_v1(
    after: &CanonicalKirInventoryV1<'_>,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    old: &CsLineageV1,
    next: &mut CsLineageV1,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1 as Kind;
    for (n, r) in rows.definitions.iter().enumerate() {
        let range = cs_range_v1(r.outputs, rows.definition_outputs.len(), b)?;
        for descendant in &rows.definition_outputs[range] {
            let output = cs_definition_v1(after, descendant.output, b)?;
            for ancestor in &old.definitions[old.ancestors[n].clone()] {
                cs_push_v1(
                    &mut next.definitions,
                    CsDefinitionLinkV1 {
                        original: ancestor.original,
                        output,
                        retained: ancestor.retained && descendant.kind == Kind::Retained,
                    },
                    b,
                )?;
            }
        }
    }
    source_catalog_sort_v1(
        &mut next.definitions,
        |r| (r.output, r.original, !r.retained),
        b,
    )
    .map_err(ProductionCanonicalRankedSourceErrorV1::from)?;
    let mut written = 0;
    for read in 0..next.definitions.len() {
        b.charge_work(2)?;
        let row = next.definitions[read];
        if written != 0
            && (
                next.definitions[written - 1].original,
                next.definitions[written - 1].output,
            ) == (row.original, row.output)
        {
            continue;
        }
        next.definitions[written] = row;
        written += 1;
    }
    b.charge_work(next.definitions.len() - written)?;
    next.definitions.truncate(written);
    let mut position = 0;
    for output in 0..after.definitions().len() {
        let start = position;
        while position < written && next.definitions[position].output == output {
            b.charge_work(1)?;
            position += 1;
        }
        if start == position {
            return Err(cs_invalid_v1("final definition has no source descendant"));
        }
        cs_push_v1(&mut next.ancestors, start..position, b)?;
    }
    if position != written {
        return Err(cs_invalid_v1("definition ancestor tail"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cs_blocks_v1(
    original: &CanonicalKirInventoryV1<'_>,
    before: &CanonicalKirInventoryV1<'_>,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    old: &CsLineageV1,
    next: &mut CsLineageV1,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    for r in original.blocks() {
        cs_push_v1(
            &mut next.block_controls,
            ProductionCanonicalScalarBlockControlV1 {
                original: r.coordinate,
                placement: None,
                reachable: false,
            },
            b,
        )?;
    }
    for row in rows.blocks {
        let start = next.segments.len();
        for segment in &rows.segments[cs_range_v1(row.segments, rows.segments.len(), b)?] {
            let input = cs_block_v1(before, segment.input, b)?;
            let current = control.block(segment.input, b)?;
            let old_range = old.blocks[input].clone();
            for (offset, old_segment) in old.segments[old_range.clone()].iter().enumerate() {
                b.charge_work(5)?;
                let ordinal = cs_block_v1(original, old_segment.original, b)?;
                let previous = old.block_controls[ordinal];
                if previous.placement
                    != Some(fe2o3_kernel_analysis::CanonicalKirBlockPlacementV1 {
                        output: segment.input,
                        segment: u32::try_from(offset)
                            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    })
                    || next.block_controls[ordinal].placement.is_some()
                {
                    return Err(cs_invalid_v1("original block chain custody"));
                }
                let mut copied = *old_segment;
                if offset + 1 == old_range.len() {
                    if copied.connector.is_some() {
                        return Err(cs_invalid_v1("old chain terminator"));
                    }
                    copied.connector = segment
                        .connector
                        .map(|edge| cs_edge_v1(before, edge, b).map(|n| old.edges[n]))
                        .transpose()?;
                }
                let position = u32::try_from(next.segments.len() - start)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?;
                next.block_controls[ordinal] = ProductionCanonicalScalarBlockControlV1 {
                    original: copied.original,
                    placement: Some(fe2o3_kernel_analysis::CanonicalKirBlockPlacementV1 {
                        output: row.output,
                        segment: position,
                    }),
                    reachable: previous.reachable && current.reachable,
                };
                cs_push_v1(&mut next.segments, copied, b)?;
            }
        }
        cs_push_v1(&mut next.blocks, start..next.segments.len(), b)?;
    }
    use fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1 as Place;
    for old_edge in &old.edge_controls {
        b.charge_work(3)?;
        let source = next.block_controls[cs_block_v1(original, old_edge.original.source, b)?];
        let (placement, executable) = match old_edge.placement {
            Place::Retained(edge) => {
                let checked = control.edge(edge, b)?;
                let placement = match checked.placement {
                    Place::InternalConnector(_) => Place::InternalConnector(
                        source
                            .placement
                            .ok_or_else(|| cs_invalid_v1("connector source placement"))?,
                    ),
                    other => other,
                };
                (placement, old_edge.executable && checked.executable)
            }
            Place::InternalConnector(_) => match source.placement {
                Some(p) => (
                    Place::InternalConnector(p),
                    old_edge.executable && source.reachable,
                ),
                None => (Place::Omitted, false),
            },
            Place::Omitted => (Place::Omitted, false),
        };
        cs_push_v1(
            &mut next.edge_controls,
            ProductionCanonicalScalarEdgeControlV1 {
                original: old_edge.original,
                placement,
                executable,
            },
            b,
        )?;
    }
    Ok(())
}

trait CsPairObserverV1 {
    type Next;
    #[allow(clippy::too_many_arguments)]
    fn pair(
        &self,
        input: &CanonicalKirInventoryV1<'_>,
        output: &CanonicalKirInventoryV1<'_>,
        control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
        rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
        previous: &CsLineageV1,
        next: &CsLineageV1,
        round: u16,
        integer: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self::Next>;
    fn retained(next: &Self::Next) -> usize;
    // New backing is already paid. Destroy old backing before returning its debit.
    fn replace(&mut self, next: Self::Next) -> usize;
}
struct CsNoPairObserverV1;
impl CsPairObserverV1 for CsNoPairObserverV1 {
    type Next = ();
    #[allow(clippy::too_many_arguments)]
    fn pair(
        &self,
        _: &CanonicalKirInventoryV1<'_>,
        _: &CanonicalKirInventoryV1<'_>,
        _: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
        _: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
        _: &CsLineageV1,
        _: &CsLineageV1,
        _: u16,
        _: bool,
        _: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<()> {
        Ok(())
    }
    fn retained(_: &()) -> usize {
        0
    }
    fn replace(&mut self, _: ()) -> usize {
        0
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn cs_pair_v1(
    original: &CanonicalKirInventoryV1<'_>,
    input: &CsGraphV1,
    output: &CsGraphV1,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    old: &CsLineageV1,
    round: u16,
    integer: bool,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<CsLineageV1> {
    cs_pair_with_observer_v1(
        original,
        input,
        output,
        rows,
        old,
        round,
        integer,
        &CsNoPairObserverV1,
        b,
    )
    .map(|(lineage, ())| lineage)
}

#[allow(clippy::too_many_arguments)]
fn cs_pair_with_observer_v1<O: CsPairObserverV1>(
    original: &CanonicalKirInventoryV1<'_>,
    input: &CsGraphV1,
    output: &CsGraphV1,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    old: &CsLineageV1,
    round: u16,
    integer: bool,
    observer: &O,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<(CsLineageV1, O::Next)> {
    let (before, receipt) = CanonicalKirInventoryV1::derive(input, b)?;
    b.reserve_storage(receipt.retained_storage())?;
    let (after, receipt) = CanonicalKirInventoryV1::derive(output, b)?;
    b.reserve_storage(receipt.retained_storage())?;
    old.shape(&before, b)?;
    let (checked, receipt) =
        fe2o3_kernel_analysis::check_canonical_kir_transition_v1(&before, &after, rows, b)?;
    b.reserve_storage(receipt.retained_storage())?;
    let (control, receipt) =
        fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1::derive(&checked, b)?;
    b.reserve_storage(receipt.retained_storage())?;
    b.charge_work(argument_sum_v1(&[
        input.canonical().canonical_bytes().len(),
        output.canonical().canonical_bytes().len(),
    ])?)?;
    if input.module().kernels != output.module().kernels {
        return Err(cs_invalid_v1(
            "complete neutral kernel and launch declarations",
        ));
    }
    let fanout = cs_definition_fanout_v1(&before, rows, old, b)?;
    // Admit the actual sparse join product before allocating its output. The
    // later insertion, ordering and deduplication also pay their real visits.
    b.charge_work(fanout)?;
    let mut next = CsLineageV1::empty(&after, original, old.segments.len(), fanout, b)?;
    for row in rows.functions {
        let n = cs_function_v1(&before, row.input, b)?;
        cs_push_v1(&mut next.functions, old.functions[n], b)?;
    }
    for row in rows.operations {
        let origin = match row.origin {
            fe2o3_kernel_ir::CanonicalKirOperationOriginV1::Retained(input) => {
                old.operations[cs_operation_v1(&before, input, b)?]
            }
            fe2o3_kernel_ir::CanonicalKirOperationOriginV1::ConstantFrom(definition) => {
                cs_definition_v1(&before, definition, b)?;
                ProductionCanonicalScalarOperationOriginV1::SynthesizedConstant {
                    round,
                    integer,
                    definition,
                }
            }
        };
        cs_push_v1(&mut next.operations, origin, b)?;
    }
    cs_definitions_v1(&after, rows, old, &mut next, b)?;
    cs_blocks_v1(original, &before, rows, &control, old, &mut next, b)?;
    for row in rows.uses {
        cs_push_v1(
            &mut next.uses,
            old.uses[cs_use_v1(&before, row.input, b)?],
            b,
        )?;
    }
    for row in rows.edges {
        cs_push_v1(
            &mut next.edges,
            old.edges[cs_edge_v1(&before, row.input, b)?],
            b,
        )?;
    }
    for row in rows.edge_arguments {
        cs_push_v1(
            &mut next.arguments,
            old.arguments[cs_argument_v1(&before, row.input, b)?],
            b,
        )?;
    }
    next.shape(&after, b)?;
    next.finish(b)?;
    let observed = observer.pair(
        &before, &after, &control, rows, old, &next, round, integer, b,
    )?;
    Ok((next, observed))
}

fn cs_lineage_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    original: &CanonicalKirInventoryV1<'_>,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<CsLineageV1> {
    cs_lineage_with_observer_v1(owner, original, &mut CsNoPairObserverV1, b)
}

fn cs_lineage_with_observer_v1<O: CsPairObserverV1>(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    original: &CanonicalKirInventoryV1<'_>,
    observer: &mut O,
    b: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<CsLineageV1> {
    b.charge_work(1)?;
    if !std::ptr::eq(original.owner(), owner.original.executable()) {
        return Err(ProductionCanonicalScalarSourceErrorV1::InputCustody);
    }
    let mut previous = cs_scope_v1(b, |b| cs_identity_v1(original, b))?;
    b.reserve_storage(previous.storage)?;
    let mut input = owner.original.executable();
    for round in owner.history.rounds() {
        for (output, rows, integer) in [
            (
                round.integer().owner(),
                round.integer().occurrences().candidate(),
                true,
            ),
            (
                round.scalar().owner(),
                round.scalar().occurrences().candidate(),
                false,
            ),
        ] {
            let (next, observed) = cs_scope_v1(b, |b| {
                cs_pair_with_observer_v1(
                    original,
                    input,
                    output,
                    rows,
                    &previous,
                    round.ordinal(),
                    integer,
                    observer,
                    b,
                )
            })?;
            b.reserve_storage(argument_sum_v1(&[next.storage, O::retained(&observed)])?)?;
            let retired_observation = observer.replace(observed);
            if retired_observation != 0 {
                b.release_storage(retired_observation)?;
            }
            let retired = previous.storage;
            drop(previous);
            b.release_storage(retired)?;
            previous = next;
            input = output;
        }
    }
    b.charge_work(1)?;
    if !std::ptr::eq(input, owner.output()) {
        return Err(ProductionCanonicalScalarSourceErrorV1::InputCustody);
    }
    Ok(previous)
}
