use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirBlockControlV1, CanonicalKirEdgeControlV1, CanonicalKirOutputUseV1,
};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionDescendantV1, CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument,
    CanonicalKirEdgeCoordinateV1 as Edge, CanonicalKirTransitionRangeV1,
};
use index::ProductionOptimizedSourceGapIntervalV18 as GapInterval;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Physical disposition of an authenticated original operation.
pub enum ProductionOptimizedSourceOperationV18 {
    /// Retained as this exact checked output occurrence.
    Retained {
        /// Original operation coordinate.
        input: OpCoordinate,
        /// Actual successor operation coordinate.
        output: OpCoordinate,
    },
    /// Removed by an admitted pure rewrite; output values require use joins.
    Rewritten {
        /// Original operation coordinate.
        input: OpCoordinate,
    },
    /// Removed from a block independently checked unreachable.
    RemovedUnreachable {
        /// Original operation coordinate.
        input: OpCoordinate,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Checked control status and interval of an original source gap.
pub enum ProductionOptimizedSourceGapV18 {
    /// Reachable source gap, bounded by its retained actual anchors.
    Reachable(GapInterval),
    /// No checked execution reaches the original gap.
    Unreachable {
        /// Physical interval if unreachable output remains.
        placement: Option<GapInterval>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Actual placement of an original source terminator.
pub enum ProductionOptimizedSourceTerminatorV18 {
    /// Terminator of the resulting block.
    Retained {
        /// Actual output block.
        output: Block,
        /// Whether checked execution reaches the source block.
        reachable: bool,
    },
    /// Consumed internal edge in an independently checked block merge.
    InternalConnector {
        /// Actual merged output block.
        output: Block,
        /// Original block's segment within the merge chain.
        segment: u32,
        /// Whether checked execution reaches the source block.
        reachable: bool,
    },
    /// Unreachable source block with no physical successor placement.
    RemovedUnreachable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// One complete source-span attachment after checked optimization.
pub enum ProductionOptimizedSourceSpanV18 {
    /// Operation retained, rewritten, or removed by unreachable control.
    Operation(ProductionOptimizedSourceOperationV18),
    /// Explicit zero-operation span at an interval of output gaps.
    Gap(ProductionOptimizedSourceGapV18),
    /// Source call already removed by original checked instance expansion.
    OriginalRemovedCall,
    /// Original authenticated source span with no physical operations.
    OriginalNoOperations,
}

#[derive(Clone, Copy)]
pub(super) enum AttachmentTarget {
    Operation(ProductionOptimizedSourceOperationV18),
    Definition {
        input: Definition,
        outputs: CanonicalKirTransitionRangeV1,
    },
    Block {
        input: Block,
        control: CanonicalKirBlockControlV1,
    },
    Terminator {
        input: Block,
        output: ProductionOptimizedSourceTerminatorV18,
    },
    Edge {
        input: Edge,
        control: CanonicalKirEdgeControlV1,
    },
    Gap(ProductionOptimizedSourceGapV18),
    Use {
        input: UseCoordinate,
        output: Option<CanonicalKirOutputUseV1>,
    },
    EdgeArgument {
        input: EdgeArgument,
        output: Option<EdgeArgument>,
    },
    OriginalRemovedCall,
    OriginalNoOutput,
}

fn ordinal(value: usize) -> SourceOwnedResultV18<u32> {
    u32::try_from(value).map_err(|_| ArgumentResourceV1::Arithmetic.into())
}

#[derive(Default)]
struct SiteControlCensus {
    reachable: bool,
    unreachable: bool,
    original_no_output: bool,
}

fn control_query_headers<T>() -> SourceOwnedResultV18<usize> {
    argument_sum_v1(&[
        std::mem::size_of::<SiteControlCensus>(),
        std::mem::size_of::<Option<CanonicalKirBlockControlV1>>(),
        std::mem::size_of::<SourceOwnedResultV18<T>>(),
    ])
    .map_err(Into::into)
}

impl SiteControlCensus {
    fn record(&mut self, reachable: bool) {
        self.reachable |= reachable;
        self.unreachable |= !reachable;
    }

    fn disposition(&self) -> ProductionOptimizedSourceSiteControlV18 {
        use ProductionOptimizedSourceSiteControlV18 as D;
        match (self.reachable, self.unreachable) {
            (true, false) => D::Retained,
            (false, true) => D::RemovedUnreachable,
            (true, true) => D::Mixed,
            (false, false) => D::OriginalUnmaterialized,
        }
    }
}

fn span_control(
    view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    row: &AttachmentTarget,
    census: &mut SiteControlCensus,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(1)?;
    match row {
        AttachmentTarget::Operation(operation) => {
            let input = match operation {
                ProductionOptimizedSourceOperationV18::Retained { input, .. }
                | ProductionOptimizedSourceOperationV18::Rewritten { input }
                | ProductionOptimizedSourceOperationV18::RemovedUnreachable { input } => *input,
            };
            let control = view
                .control
                .block(input.block, budget)
                .map_err(transition_error)?;
            match operation {
                ProductionOptimizedSourceOperationV18::Retained { output, .. } => {
                    let placement =
                        control
                            .placement
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "retained optimized source operation has no segment",
                            ))?;
                    budget.charge_work(2)?;
                    if output.block != placement.output
                        || view.index.operation(view.checked, input, budget)? != Some(*output)
                    {
                        return resources::binding("optimized source operation changed segment");
                    }
                    let ordinal =
                        resources::block_index(view.checked.output(), output.block, budget)?;
                    let block = view.checked.rows().blocks.get(ordinal).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding("optimized source output block"),
                    )?;
                    budget.charge_work(3)?;
                    let segment_ordinal = (block.segments.start as usize)
                        .checked_add(placement.segment as usize)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    if block.output != output.block
                        || placement.segment >= block.segments.len
                        || view
                            .checked
                            .rows()
                            .segments
                            .get(segment_ordinal)
                            .is_none_or(|segment| segment.input != input.block)
                    {
                        return resources::binding("optimized source operation segment owner");
                    }
                }
                ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                    if !control.reachable {
                        return resources::binding("optimized rewrite has unreachable control");
                    }
                }
                ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                    if control.reachable {
                        return resources::binding("optimized removal has executable control");
                    }
                }
            }
            census.record(control.reachable);
        }
        AttachmentTarget::Gap(ProductionOptimizedSourceGapV18::Reachable(_)) => census.record(true),
        AttachmentTarget::Gap(ProductionOptimizedSourceGapV18::Unreachable { .. }) => {
            census.record(false)
        }
        AttachmentTarget::OriginalRemovedCall | AttachmentTarget::OriginalNoOutput => {
            census.original_no_output = true;
        }
        AttachmentTarget::Definition { .. }
        | AttachmentTarget::Block { .. }
        | AttachmentTarget::Terminator { .. }
        | AttachmentTarget::Edge { .. }
        | AttachmentTarget::Use { .. }
        | AttachmentTarget::EdgeArgument { .. } => {
            return resources::binding("optimized source control has a non-span attachment");
        }
    }
    Ok(())
}

impl<'g> ProductionOptimizedSourceCorrespondenceV18<'g> {
    /// Authenticates the original block once, then checks all indexed physical
    /// segments. The borrowed header must remain paid with its consuming facts.
    pub fn source_block_control(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceBlockControlV18<'_, 'g>> {
        self.retain((|| {
            self.query(budget)?;
            let headers =
                control_query_headers::<ProductionOptimizedSourceBlockControlV18<'_, '_>>()?;
            budget.reserve_storage(headers)?;
            let result = (|| {
                let entry = self
                    .original
                    .source_block_entry(root, instance, block, budget)?;
                let function = self.original.source.instance(root, instance, budget)?.0;
                budget.charge_work(2)?;
                let semantic = self
                    .original
                    .source
                    .owner
                    .inner
                    .source
                    .owner
                    .source_semantic();
                let source_block = semantic
                    .functions()
                    .get(function.index() as usize)
                    .and_then(|function| function.blocks().get(block.index() as usize))
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "optimized source block declaration",
                    ))?;
                let statements = source_block.statements().len();
                let mut census = SiteControlCensus::default();
                if let Some(entry) = entry {
                    census.record(
                        self.control
                            .block(entry, budget)
                            .map_err(transition_error)?
                            .reachable,
                    );
                }
                let mut visit =
                    |statement: Option<u32>,
                     rows: &[AttachmentTarget],
                     budget: &mut ArgumentBudgetV1<'_>| {
                        if statement.is_some_and(|statement| statement as usize >= statements)
                            || rows.is_empty()
                        {
                            return resources::binding(
                                "optimized source block span declaration or coverage",
                            );
                        }
                        for row in rows {
                            span_control(self, row, &mut census, budget)?;
                        }
                        Ok(())
                    };
                let visitor_headers = argument_sum_v1(&[
                    std::mem::size_of_val(&visit),
                    std::mem::size_of::<SourceOwnedResultV18<usize>>(),
                ])?;
                budget.reserve_storage(visitor_headers)?;
                let visited = self
                    .index
                    .visit_block_sites(root, instance, block, budget, &mut visit);
                drop(visit);
                let cleanup = budget.release_storage(visitor_headers);
                let sites = match visited {
                    Err(error) => return Err(error),
                    Ok(count) => {
                        cleanup?;
                        count
                    }
                };
                if entry.is_none() && (sites != 0 || census.original_no_output) {
                    return resources::binding("optimized source spans lack original block entry");
                }
                if entry.is_some()
                    && sites
                        != statements
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?
                {
                    return resources::binding(
                        "optimized source block has incomplete semantic site census",
                    );
                }
                let disposition = census.disposition();
                self.check(budget)?;
                Ok(ProductionOptimizedSourceBlockControlV18 {
                    optimized: self,
                    root,
                    instance,
                    block,
                    statements,
                    disposition,
                })
            })();
            // This closed query allocates no backing and calls no consumer.
            // Drop its scratch before returning either the original refusal or
            // a borrowed header already covered by the consuming facts.
            let cleanup = budget.release_storage(headers);
            match result {
                Err(error) => Err(error),
                Ok(value) => {
                    cleanup?;
                    Ok(value)
                }
            }
        })())
    }
}

impl ProductionOptimizedSourceBlockControlV18<'_, '_> {
    /// Complete block disposition, including every original physical segment.
    pub fn disposition(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceSiteControlV18> {
        self.optimized.retain((|| {
            self.optimized.query(budget)?;
            Ok(self.disposition)
        })())
    }

    /// Checks one original site through the same immutable block census/index.
    pub fn site(
        &self,
        statement: Option<u32>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceSiteControlV18> {
        self.optimized.retain((|| {
            self.optimized.query(budget)?;
            let headers = control_query_headers::<ProductionOptimizedSourceSiteControlV18>()?;
            budget.reserve_storage(headers)?;
            let result = (|| {
                budget.charge_work(1)?;
                if statement.is_some_and(|statement| statement as usize >= self.statements) {
                    return resources::binding("optimized source control statement locator");
                }
                let rows = self.optimized.index.optional_site(
                    self.root,
                    self.instance,
                    self.block,
                    statement,
                    budget,
                )?;
                let Some(rows) = rows else {
                    if self.disposition
                        == ProductionOptimizedSourceSiteControlV18::OriginalUnmaterialized
                    {
                        return Ok(self.disposition);
                    }
                    return resources::binding(
                        "materialized source block lacks requested site span",
                    );
                };
                let mut census = SiteControlCensus::default();
                for row in rows {
                    span_control(self.optimized, row, &mut census, budget)?;
                }
                if census.original_no_output {
                    match self.disposition {
                        ProductionOptimizedSourceSiteControlV18::Retained => census.record(true),
                        ProductionOptimizedSourceSiteControlV18::RemovedUnreachable => {
                            census.record(false)
                        }
                        ProductionOptimizedSourceSiteControlV18::Mixed => {
                            census.record(true);
                            census.record(false);
                        }
                        ProductionOptimizedSourceSiteControlV18::OriginalUnmaterialized => {
                            return resources::binding(
                                "source no-output span lacks checked control",
                            );
                        }
                    }
                }
                let disposition = census.disposition();
                if disposition == ProductionOptimizedSourceSiteControlV18::OriginalUnmaterialized {
                    return resources::binding("optimized source site has no control evidence");
                }
                self.optimized.check(budget)?;
                Ok(disposition)
            })();
            let cleanup = budget.release_storage(headers);
            match result {
                Err(error) => Err(error),
                Ok(value) => {
                    cleanup?;
                    Ok(value)
                }
            }
        })())
    }
}

#[cfg(test)]
mod control_header_tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn control_query_scratch_accounts_actual_census_and_result_headers() {
        let common =
            size_of::<SiteControlCensus>() + size_of::<Option<CanonicalKirBlockControlV1>>();
        let block = common
            + size_of::<SourceOwnedResultV18<ProductionOptimizedSourceBlockControlV18<'_, '_>>>();
        let site =
            common + size_of::<SourceOwnedResultV18<ProductionOptimizedSourceSiteControlV18>>();
        assert_eq!(
            control_query_headers::<ProductionOptimizedSourceBlockControlV18<'_, '_>>().unwrap(),
            block
        );
        assert_eq!(
            control_query_headers::<ProductionOptimizedSourceSiteControlV18>().unwrap(),
            site
        );
        for bytes in [block, site] {
            for short in [false, true] {
                let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(0);
                let mut budget = ArgumentBudgetV1::new(&mut work, 7 + bytes - usize::from(short));
                budget.reserve_storage(7).unwrap();
                let result = budget.reserve_storage(bytes);
                if short {
                    assert!(
                        matches!(result, Err(ArgumentResourceV1::Storage(error)) if error.actual() == 7 + bytes)
                    );
                    assert_eq!((budget.storage(), budget.peak_storage()), (7, 7));
                    assert_eq!(budget.failed_storage(), Some(7 + bytes));
                } else {
                    result.unwrap();
                    assert_eq!(
                        (budget.storage(), budget.peak_storage()),
                        (7 + bytes, 7 + bytes)
                    );
                }
                assert_eq!(budget.work(), 0);
            }
        }
    }
}
pub(super) fn block_coordinate(function: usize, block: usize) -> SourceOwnedResultV18<Block> {
    Ok(Block {
        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(ordinal(function)?),
        block: ordinal(block)?,
    })
}
pub(super) fn operation_coordinate(
    point: TileScalarPointV29,
) -> SourceOwnedResultV18<OpCoordinate> {
    Ok(OpCoordinate {
        block: block_coordinate(point.function, point.block)?,
        operation: ordinal(point.operation)?,
    })
}

pub(super) fn definition_range(
    checked: &Transition<'_, '_, '_, '_>,
    input: Definition,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<CanonicalKirTransitionRangeV1> {
    let index = resources::definition_index(checked.input(), input, budget)?;
    budget.charge_work(2)?;
    let row =
        checked
            .rows()
            .definitions
            .get(index)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized source definition row",
            ))?;
    if row.input != input {
        return resources::binding("optimized source definition row order");
    }
    let end = (row.outputs.start as usize)
        .checked_add(row.outputs.len as usize)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    if end > checked.rows().definition_outputs.len() {
        return resources::binding("optimized source descendant range");
    }
    Ok(row.outputs)
}

pub(super) fn operation(
    checked: &Transition<'_, '_, '_, '_>,
    control: &Control<'_, '_, '_>,
    index: &SourceIndex,
    input: OpCoordinate,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionOptimizedSourceOperationV18> {
    if let Some(output) = index.operation(checked, input, budget)? {
        return Ok(ProductionOptimizedSourceOperationV18::Retained { input, output });
    }
    let block = control
        .block(input.block, budget)
        .map_err(transition_error)?;
    Ok(if block.reachable {
        ProductionOptimizedSourceOperationV18::Rewritten { input }
    } else {
        ProductionOptimizedSourceOperationV18::RemovedUnreachable { input }
    })
}

pub(super) fn gap(
    checked: &Transition<'_, '_, '_, '_>,
    control: &Control<'_, '_, '_>,
    index: &SourceIndex,
    block: Block,
    operation: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionOptimizedSourceGapV18> {
    let fact = control.block(block, budget).map_err(transition_error)?;
    let placement = index.gap(checked, block, operation, budget)?;
    if fact.reachable {
        let placement = placement.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "reachable source gap has no checked placement",
        ))?;
        Ok(ProductionOptimizedSourceGapV18::Reachable(placement))
    } else {
        Ok(ProductionOptimizedSourceGapV18::Unreachable { placement })
    }
}

pub(super) fn terminator(
    checked: &Transition<'_, '_, '_, '_>,
    control: &Control<'_, '_, '_>,
    input: Block,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionOptimizedSourceTerminatorV18> {
    let fact = control.block(input, budget).map_err(transition_error)?;
    let Some(placement) = fact.placement else {
        if fact.reachable {
            return resources::binding("reachable source terminator lacks placement");
        }
        return Ok(ProductionOptimizedSourceTerminatorV18::RemovedUnreachable);
    };
    let index = resources::block_index(checked.output(), placement.output, budget)?;
    budget.charge_work(2)?;
    let row =
        checked
            .rows()
            .blocks
            .get(index)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized source terminal block row",
            ))?;
    if row.output != placement.output || placement.segment >= row.segments.len {
        return resources::binding("optimized source terminal segment row");
    }
    Ok(if placement.segment + 1 == row.segments.len {
        ProductionOptimizedSourceTerminatorV18::Retained {
            output: placement.output,
            reachable: fact.reachable,
        }
    } else {
        ProductionOptimizedSourceTerminatorV18::InternalConnector {
            output: placement.output,
            segment: placement.segment,
            reachable: fact.reachable,
        }
    })
}

pub(super) fn project(
    location: TileAttachmentLocationV29,
    checked: &Transition<'_, '_, '_, '_>,
    control: &Control<'_, '_, '_>,
    index: &SourceIndex,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<AttachmentTarget> {
    use TileAttachmentLocationV29 as L;
    use TileScalarSourceV29 as S;
    budget.charge_work(1)?;
    Ok(match location {
        L::Origin(S::Operation(point)) => AttachmentTarget::Operation(operation(
            checked,
            control,
            index,
            operation_coordinate(point)?,
            budget,
        )?),
        L::Origin(
            source @ (S::FunctionParameter { .. } | S::BlockParameter { .. } | S::Result { .. }),
        ) => {
            let input = match source {
                S::FunctionParameter {
                    function,
                    parameter,
                } => Definition::FunctionArgument {
                    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(ordinal(function)?),
                    argument: ordinal(parameter)?,
                },
                S::BlockParameter {
                    function,
                    block,
                    parameter,
                } => Definition::BlockArgument {
                    block: block_coordinate(function, block)?,
                    argument: ordinal(parameter)?,
                },
                S::Result { operation, result } => Definition::Result {
                    operation: operation_coordinate(operation)?,
                    result: ordinal(result)?,
                },
                S::Block { .. } | S::Operation(_) | S::Terminator { .. } | S::Edge { .. } => {
                    return resources::binding("optimized source definition attachment arm");
                }
            };
            AttachmentTarget::Definition {
                input,
                outputs: definition_range(checked, input, budget)?,
            }
        }
        L::Origin(S::Block { function, block }) => {
            let input = block_coordinate(function, block)?;
            AttachmentTarget::Block {
                input,
                control: control.block(input, budget).map_err(transition_error)?,
            }
        }
        L::Origin(S::Terminator { function, block }) => {
            let input = block_coordinate(function, block)?;
            AttachmentTarget::Terminator {
                input,
                output: terminator(checked, control, input, budget)?,
            }
        }
        L::Origin(S::Edge {
            function,
            block,
            edge,
        }) => {
            let input = Edge {
                source: block_coordinate(function, block)?,
                successor: ordinal(edge)?,
            };
            AttachmentTarget::Edge {
                input,
                control: control.edge(input, budget).map_err(transition_error)?,
            }
        }
        L::Gap(point) => AttachmentTarget::Gap(gap(
            checked,
            control,
            index,
            block_coordinate(point.function, point.block)?,
            point.operation,
            budget,
        )?),
        L::Use(input) => AttachmentTarget::Use {
            input,
            output: control.operand(input, budget).map_err(transition_error)?,
        },
        L::EdgeArgument(input) => AttachmentTarget::EdgeArgument {
            input,
            output: control
                .edge_argument(input, budget)
                .map_err(transition_error)?,
        },
        L::Tombstone => AttachmentTarget::OriginalRemovedCall,
        L::NoOutput => AttachmentTarget::OriginalNoOutput,
    })
}

impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    pub(in super::super) fn source_span_entry_count(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.retain((|| {
            self.query(budget)?;
            self.original.source.instance(root, instance, budget)?;
            let rows = self.index.site(root, instance, block, statement, budget)?;
            budget.charge_work(1)?;
            let count = rows.len();
            self.check(budget)?;
            Ok(count)
        })())
    }

    pub(in super::super) fn source_span_entry(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceSpanV18> {
        self.retain((|| {
            self.query(budget)?;
            self.original.source.instance(root, instance, budget)?;
            let rows = self.index.site(root, instance, block, statement, budget)?;
            budget.charge_work(1)?;
            let row = rows
                .get(ordinal)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized source span entry ordinal",
                ))?;
            let span = match row {
                AttachmentTarget::Operation(row) => {
                    ProductionOptimizedSourceSpanV18::Operation(*row)
                }
                AttachmentTarget::Gap(row) => ProductionOptimizedSourceSpanV18::Gap(*row),
                AttachmentTarget::OriginalRemovedCall => {
                    ProductionOptimizedSourceSpanV18::OriginalRemovedCall
                }
                AttachmentTarget::OriginalNoOutput => {
                    ProductionOptimizedSourceSpanV18::OriginalNoOperations
                }
                AttachmentTarget::Definition { .. }
                | AttachmentTarget::Block { .. }
                | AttachmentTarget::Terminator { .. }
                | AttachmentTarget::Edge { .. }
                | AttachmentTarget::Use { .. }
                | AttachmentTarget::EdgeArgument { .. } => {
                    return resources::binding(
                        "optimized source span has a non-operation attachment",
                    );
                }
            };
            self.check(budget)?;
            Ok(span)
        })())
    }

    // Private currentness transport uses the already-built exact gap index.
    // Choosing a state within a reachable interval still requires a separate
    // proof that every interposed operation preserves that state.
    pub(in super::super) fn physical_gap(
        &self,
        block: Block,
        operation: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceGapV18> {
        self.retain((|| {
            self.query(budget)?;
            gap(
                self.checked,
                self.control,
                self.index,
                block,
                operation,
                budget,
            )
        })())
    }

    /// Returns the checked disposition of an exact original operation locator.
    /// A retained coordinate alone grants no typed-memory or source admission.
    pub fn operation(
        &self,
        input: OpCoordinate,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceOperationV18> {
        self.retain((|| {
            self.query(budget)?;
            operation(self.checked, self.control, self.index, input, budget)
        })())
    }

    /// Complete checked descendants. A consumer must select by an actual output
    /// occurrence; matching type or value text cannot choose one arbitrarily.
    pub fn definition_descendants(
        &self,
        input: Definition,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[CanonicalKirDefinitionDescendantV1]> {
        self.retain((|| {
            self.query(budget)?;
            let range = definition_range(self.checked, input, budget)?;
            let first = range.start as usize;
            let end = first
                .checked_add(range.len as usize)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            self.checked
                .rows()
                .definition_outputs
                .get(first..end)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized source descendant custody",
                ))
        })())
    }

    /// Resolves one exact input use to its actual output occurrence, if retained.
    pub fn operand(
        &self,
        input: UseCoordinate,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<CanonicalKirOutputUseV1>> {
        self.retain((|| {
            self.query(budget)?;
            self.control
                .operand(input, budget)
                .map_err(transition_error)
        })())
    }

    /// Resolves an edge payload occurrence without conflating duplicate targets.
    pub fn edge_argument(
        &self,
        input: EdgeArgument,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<EdgeArgument>> {
        self.retain((|| {
            self.query(budget)?;
            self.control
                .edge_argument(input, budget)
                .map_err(transition_error)
        })())
    }
}
