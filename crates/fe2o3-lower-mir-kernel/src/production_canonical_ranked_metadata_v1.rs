use fe2o3_kernel_analysis::{
    CanonicalRankedMetadataFactV1 as CrFactV1, CanonicalRankedMetadataKindV1 as CrKindV1,
    CanonicalRankedMetadataRowV1 as CrInertRowV1, CanonicalRankedMetadataV1 as CrInertV1,
    CanonicalRankedSubjectV1 as CrSubjectV1,
};
type CrResultV1<T> = Result<T, ProductionCanonicalRankedSourceErrorV1>;

/// Source authentication and resource errors; structural coverage is not safety.
#[derive(Debug)]
pub enum ProductionCanonicalRankedSourceErrorV1 {
    /// Existing complete source replay or source/ABI correspondence refused.
    Source(ProductionSemanticKirErrorV1),
    /// Shared canonical resource ledger refused a request.
    Resource(ArgumentResourceV1),
    /// The single graph inventory or lazy analysis scope refused.
    Analysis(fe2o3_pliron::CanonicalAnalysisScopeErrorV1),
    /// Independent complete structural coverage refused.
    Coverage(fe2o3_kernel_analysis::CanonicalRankedViewErrorV1),
    /// Actual source contract derivation or graph binding refused.
    Catalog(ProductionSourceOutputCatalogErrorV1),
    /// A typed source/graph coordinate or complete roster is inconsistent.
    Invalid(&'static str),
    /// A callback or construction panicked; no completion is returned.
    Panicked,
}
impl fmt::Display for ProductionCanonicalRankedSourceErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
            Self::Analysis(e) => e.fmt(f),
            Self::Coverage(e) => e.fmt(f),
            Self::Catalog(e) => e.fmt(f),
            Self::Invalid(reason) => write!(f, "canonical ranked source: {reason}"),
            Self::Panicked => f.write_str("canonical ranked source callback panicked"),
        }
    }
}
impl std::error::Error for ProductionCanonicalRankedSourceErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(e) => Some(e),
            Self::Resource(e) => Some(e),
            Self::Analysis(e) => Some(e),
            Self::Coverage(e) => Some(e),
            Self::Catalog(e) => Some(e),
            Self::Invalid(_) | Self::Panicked => None,
        }
    }
}
impl From<ProductionSemanticKirErrorV1> for ProductionCanonicalRankedSourceErrorV1 {
    fn from(e: ProductionSemanticKirErrorV1) -> Self {
        Self::Source(e)
    }
}
impl From<ArgumentResourceV1> for ProductionCanonicalRankedSourceErrorV1 {
    fn from(e: ArgumentResourceV1) -> Self {
        Self::Resource(e)
    }
}
impl From<fe2o3_pliron::CanonicalAnalysisScopeErrorV1> for ProductionCanonicalRankedSourceErrorV1 {
    fn from(e: fe2o3_pliron::CanonicalAnalysisScopeErrorV1) -> Self {
        Self::Analysis(e)
    }
}
impl From<fe2o3_kernel_analysis::CanonicalRankedViewErrorV1>
    for ProductionCanonicalRankedSourceErrorV1
{
    fn from(e: fe2o3_kernel_analysis::CanonicalRankedViewErrorV1) -> Self {
        Self::Coverage(e)
    }
}
impl From<ProductionSourceOutputCatalogErrorV1> for ProductionCanonicalRankedSourceErrorV1 {
    fn from(e: ProductionSourceOutputCatalogErrorV1) -> Self {
        Self::Catalog(e)
    }
}
fn cr_invalid_v1(reason: &'static str) -> ProductionCanonicalRankedSourceErrorV1 {
    ProductionCanonicalRankedSourceErrorV1::Invalid(reason)
}
fn cr_vec_v1<T>(count: usize, budget: &mut ArgumentBudgetV1<'_>) -> CrResultV1<Vec<T>> {
    let requested = argument_product_v1(count, std::mem::size_of::<T>())?;
    budget.reserve_storage(requested)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    budget.reserve_storage(argument_product_v1(
        rows.capacity() - count,
        std::mem::size_of::<T>(),
    )?)?;
    Ok(rows)
}
fn cr_push_v1<T>(rows: &mut Vec<T>, row: T, budget: &mut ArgumentBudgetV1<'_>) -> CrResultV1<()> {
    budget.charge_work(1)?;
    if rows.len() == rows.capacity() {
        return Err(cr_invalid_v1("prepaid row capacity"));
    }
    rows.push(row);
    Ok(())
}

/// A complete source occurrence, including statements with no emitted operation.
/// These references remain source facts, never replacement executable expressions.
#[derive(Clone, Copy)]
pub enum ProductionCanonicalRankedSourceSiteV1<'a> {
    /// Actual statement and its root-qualified original emission span.
    Statement {
        /// Retained source correspondence, not caller annotations.
        span: &'a SemanticKirStatementOperationSpanV1,
        /// Actual typed semantic statement.
        source: &'a fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
    },
    /// Actual terminator and its possibly empty operation span.
    Terminator {
        /// Retained source correspondence.
        span: &'a SemanticKirTerminatorOperationSpanV1,
        /// Actual typed semantic terminator, including intrinsic contracts.
        source: &'a fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorV1,
    },
    /// Closed lowering rule, with no invented source permission.
    Synthetic(&'a SemanticKirSyntheticOperationSpanV1),
}

/// One root-qualified source span joined to an exact dense graph operation range.
pub struct ProductionCanonicalRankedSpanV1<'a> {
    association: usize,
    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    operations: std::ops::Range<usize>,
    site: ProductionCanonicalRankedSourceSiteV1<'a>,
}
impl ProductionCanonicalRankedSpanV1<'_> {
    /// Index in the complete root-qualified function association roster.
    pub const fn association(&self) -> usize {
        self.association
    }
    /// Exact actual graph block, including synthetic trap blocks.
    pub const fn block(&self) -> fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
        self.block
    }
    /// Dense positions in the one shared inventory; zero ranges are retained.
    pub fn operations(&self) -> std::ops::Range<usize> {
        self.operations.clone()
    }
    /// Typed immutable source occurrence or closed synthetic rule.
    pub const fn site(&self) -> ProductionCanonicalRankedSourceSiteV1<'_> {
        self.site
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CrOriginV1 {
    operation: usize,
    association: usize,
    span: usize,
}

struct CrSourceRowsV1<'a> {
    associations: Vec<((u32, u32), usize)>,
    spans: Vec<ProductionCanonicalRankedSpanV1<'a>>,
    origins: Vec<CrOriginV1>,
    operation_origins: Vec<std::ops::Range<usize>>,
}

fn cr_association_v1(
    index: &[((u32, u32), usize)],
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<usize> {
    assert_origin_find_v1(index, budget, |row, budget| {
        budget.charge_work(2)?;
        Ok(row.0.cmp(&(root.index(), function.index())))
    })
    .map_err(call_index_error_v1)?
    .map(|i| index[i].1)
    .ok_or_else(|| cr_invalid_v1("source function association"))
}

fn cr_span_range_v1(
    inventory: &CanonicalKirInventoryV1<'_>,
    calls: &ProductionCanonicalCallsV1<'_>,
    association: usize,
    block: BlockId,
    first: u32,
    count: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<(
    fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    std::ops::Range<usize>,
)> {
    let function = calls
        .groups
        .get(association)
        .ok_or_else(|| cr_invalid_v1("span association"))?
        .function
        .canonical
        .coordinate;
    let actual = inventory
        .block_for_id(function, block, budget)
        .map_err(canonical_call_inventory_error_v1)?
        .ok_or_else(|| cr_invalid_v1("span graph block"))?;
    budget.charge_work(3)?;
    let start = argument_sum_v1(&[actual.operations.start, first as usize])?;
    let end = argument_sum_v1(&[start, count as usize])?;
    if end > actual.operations.end {
        return Err(cr_invalid_v1("span operation range"));
    }
    Ok((actual.coordinate, start..end))
}

fn cr_build_source_rows_v1<'a>(
    owner: &'a ProductionPreRankedKirOwnerV1,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    calls: &'a ProductionCanonicalCallsV1<'a>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<CrSourceRowsV1<'a>> {
    let rows = &owner.correspondence;
    let count = argument_sum_v1(&[
        rows.statement_operation_spans.len(),
        rows.terminator_operation_spans.len(),
        rows.synthetic_operation_spans.len(),
    ])?;
    let mut occurrences = 0usize;
    for count in rows
        .statement_operation_spans
        .iter()
        .map(|r| r.operation_count)
        .chain(
            rows.terminator_operation_spans
                .iter()
                .map(|r| r.operation_count),
        )
        .chain(
            rows.synthetic_operation_spans
                .iter()
                .map(|r| r.operation_count),
        )
    {
        budget.charge_work(1)?;
        occurrences = argument_sum_v1(&[occurrences, count as usize])?;
    }
    let mut out = CrSourceRowsV1 {
        associations: cr_vec_v1(calls.groups.len(), budget)?,
        spans: cr_vec_v1(count, budget)?,
        origins: cr_vec_v1(occurrences, budget)?,
        operation_origins: cr_vec_v1(inventory.operations().len(), budget)?,
    };
    for (i, group) in calls.groups.iter().enumerate() {
        let source = group.function.source;
        cr_push_v1(
            &mut out.associations,
            (
                (
                    source.correspondence_owner.index(),
                    source.semantic_function.index(),
                ),
                i,
            ),
            budget,
        )?;
    }
    source_catalog_sort_v1(&mut out.associations, |row| row.0, budget)?;
    for pair in out.associations.windows(2) {
        budget.charge_work(1)?;
        if pair[0].0 == pair[1].0 {
            return Err(cr_invalid_v1("duplicate source association"));
        }
    }
    for span in &rows.statement_operation_spans {
        let association = cr_association_v1(
            &out.associations,
            span.correspondence_owner,
            span.semantic_function,
            budget,
        )?;
        let function = &owner.semantic_ssa.source_semantic().functions()
            [span.semantic_function.index() as usize];
        let source = function
            .blocks()
            .get(span.semantic_block.index() as usize)
            .and_then(|block| block.statements().get(span.statement_ordinal as usize))
            .ok_or_else(|| cr_invalid_v1("source statement"))?;
        cr_add_span_v1(
            &mut out,
            inventory,
            calls,
            association,
            span.kernel_ir_block,
            span.first_operation_ordinal,
            span.operation_count,
            ProductionCanonicalRankedSourceSiteV1::Statement { span, source },
            budget,
        )?;
    }
    for span in &rows.terminator_operation_spans {
        let association = cr_association_v1(
            &out.associations,
            span.correspondence_owner,
            span.semantic_function,
            budget,
        )?;
        let function = &owner.semantic_ssa.source_semantic().functions()
            [span.semantic_function.index() as usize];
        let source = function
            .blocks()
            .get(span.semantic_block.index() as usize)
            .ok_or_else(|| cr_invalid_v1("source terminator"))?
            .terminator();
        cr_add_span_v1(
            &mut out,
            inventory,
            calls,
            association,
            span.kernel_ir_block,
            span.first_operation_ordinal,
            span.operation_count,
            ProductionCanonicalRankedSourceSiteV1::Terminator { span, source },
            budget,
        )?;
    }
    for span in &rows.synthetic_operation_spans {
        let association = cr_association_v1(
            &out.associations,
            span.correspondence_owner,
            span.semantic_function,
            budget,
        )?;
        cr_add_span_v1(
            &mut out,
            inventory,
            calls,
            association,
            span.kernel_ir_block,
            span.first_operation_ordinal,
            span.operation_count,
            ProductionCanonicalRankedSourceSiteV1::Synthetic(span),
            budget,
        )?;
    }
    // Reuse the existing in-place sort: each comparison and move is charged.
    source_catalog_sort_v1(
        &mut out.origins,
        |row| (row.operation, row.association),
        budget,
    )?;
    let mut next = 0;
    for operation in 0..inventory.operations().len() {
        let start = next;
        while next < out.origins.len() && out.origins[next].operation == operation {
            budget.charge_work(2)?;
            if next != start && out.origins[next - 1].association == out.origins[next].association {
                return Err(cr_invalid_v1("overlapping source operation spans"));
            }
            next += 1;
        }
        if next == start {
            return Err(cr_invalid_v1("unattributed graph operation"));
        }
        cr_push_v1(&mut out.operation_origins, start..next, budget)?;
    }
    if next != out.origins.len() {
        return Err(cr_invalid_v1("extra source operation"));
    }
    Ok(out)
}

fn cr_check_source_rows_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    calls: &ProductionCanonicalCallsV1<'_>,
    rows: &CrSourceRowsV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<()> {
    let raw = &owner.correspondence;
    let total = argument_sum_v1(&[
        raw.statement_operation_spans.len(),
        raw.terminator_operation_spans.len(),
        raw.synthetic_operation_spans.len(),
    ])?;
    budget.charge_work(4)?;
    if rows.spans.len() != total
        || rows.associations.len() != calls.groups.len()
        || rows.operation_origins.len() != inventory.operations().len()
    {
        return Err(cr_invalid_v1("incomplete source metadata"));
    }
    for (i, group) in calls.groups.iter().enumerate() {
        let s = group.function.source;
        let actual = cr_association_v1(
            &rows.associations,
            s.correspondence_owner,
            s.semantic_function,
            budget,
        )?;
        if actual != i
            || !std::ptr::eq(s, &raw.lowered_functions[i])
            || !std::ptr::eq(
                group.function.canonical.function,
                inventory.functions()[group.function.canonical.coordinate.0 as usize].function,
            )
        {
            return Err(cr_invalid_v1("source association identity"));
        }
    }
    for pair in rows.associations.windows(2) {
        budget.charge_work(1)?;
        if pair[0].0 >= pair[1].0 {
            return Err(cr_invalid_v1("source association ordering"));
        }
    }
    let mut next = 0usize;
    let mut expect = |root,
                      function,
                      block,
                      first,
                      count,
                      expected: ProductionCanonicalRankedSourceSiteV1<'_>,
                      budget: &mut ArgumentBudgetV1<'_>|
     -> CrResultV1<()> {
        budget.charge_work(5)?;
        let actual = &rows.spans[next];
        let association = cr_association_v1(&rows.associations, root, function, budget)?;
        let (coordinate, operations) =
            cr_span_range_v1(inventory, calls, association, block, first, count, budget)?;
        let site_matches = match (actual.site, expected) {
            (
                ProductionCanonicalRankedSourceSiteV1::Statement { span: a, source: x },
                ProductionCanonicalRankedSourceSiteV1::Statement { span: b, source: y },
            ) => std::ptr::eq(a, b) && std::ptr::eq(x, y),
            (
                ProductionCanonicalRankedSourceSiteV1::Terminator { span: a, source: x },
                ProductionCanonicalRankedSourceSiteV1::Terminator { span: b, source: y },
            ) => std::ptr::eq(a, b) && std::ptr::eq(x, y),
            (
                ProductionCanonicalRankedSourceSiteV1::Synthetic(a),
                ProductionCanonicalRankedSourceSiteV1::Synthetic(b),
            ) => std::ptr::eq(a, b),
            _ => false,
        };
        if actual.association != association
            || actual.block != coordinate
            || actual.operations != operations
            || !site_matches
        {
            return Err(cr_invalid_v1("source span identity or range"));
        }
        next += 1;
        Ok(())
    };
    for span in &raw.statement_operation_spans {
        let source = &owner.semantic_ssa.source_semantic().functions()
            [span.semantic_function.index() as usize]
            .blocks()[span.semantic_block.index() as usize]
            .statements()[span.statement_ordinal as usize];
        expect(
            span.correspondence_owner,
            span.semantic_function,
            span.kernel_ir_block,
            span.first_operation_ordinal,
            span.operation_count,
            ProductionCanonicalRankedSourceSiteV1::Statement { span, source },
            budget,
        )?;
    }
    for span in &raw.terminator_operation_spans {
        let source = owner.semantic_ssa.source_semantic().functions()
            [span.semantic_function.index() as usize]
            .blocks()[span.semantic_block.index() as usize]
            .terminator();
        expect(
            span.correspondence_owner,
            span.semantic_function,
            span.kernel_ir_block,
            span.first_operation_ordinal,
            span.operation_count,
            ProductionCanonicalRankedSourceSiteV1::Terminator { span, source },
            budget,
        )?;
    }
    for span in &raw.synthetic_operation_spans {
        expect(
            span.correspondence_owner,
            span.semantic_function,
            span.kernel_ir_block,
            span.first_operation_ordinal,
            span.operation_count,
            ProductionCanonicalRankedSourceSiteV1::Synthetic(span),
            budget,
        )?;
    }
    // Complete inverse: each actual operation has exactly one source span for
    // every root association of its physical function, including shared helpers.
    let mut consumed = 0usize;
    for (operation, range) in rows.operation_origins.iter().enumerate() {
        budget.charge_work(3)?;
        if range.start != consumed || range.end < range.start || range.end > rows.origins.len() {
            return Err(cr_invalid_v1("operation source range"));
        }
        let origins = &rows.origins[range.clone()];
        if origins.is_empty() {
            return Err(cr_invalid_v1("missing operation source"));
        }
        let mut previous = None;
        for origin in origins {
            budget.charge_work(6)?;
            let span = rows
                .spans
                .get(origin.span)
                .ok_or_else(|| cr_invalid_v1("source origin span"))?;
            let group = calls
                .groups
                .get(origin.association)
                .ok_or_else(|| cr_invalid_v1("source origin association"))?;
            if origin.operation != operation
                || origin.association != span.association
                || !span.operations.contains(&operation)
                || group.function.canonical.coordinate
                    != inventory.operations()[operation].coordinate.block.function
                || previous.is_some_and(|old| old >= origin.association)
            {
                return Err(cr_invalid_v1("operation source inverse"));
            }
            previous = Some(origin.association);
        }
        consumed = range.end;
    }
    if consumed != rows.origins.len() {
        return Err(cr_invalid_v1("extra operation sources"));
    }
    let mut expected_count = 0usize;
    for (association, group) in calls.groups.iter().enumerate() {
        for operation in group.function.canonical.operations.clone() {
            let range = &rows.operation_origins[operation];
            let origins = &rows.origins[range.clone()];
            let found = assert_origin_find_v1(origins, budget, |row, budget| {
                budget.charge_work(1)?;
                Ok(row.association.cmp(&association))
            })
            .map_err(call_index_error_v1)?;
            if found.is_none() {
                return Err(cr_invalid_v1("missing root-qualified operation source"));
            }
            expected_count = argument_sum_v1(&[expected_count, 1])?;
        }
    }
    if expected_count != consumed {
        return Err(cr_invalid_v1("source operation multiplicity"));
    }
    Ok(())
}

/// Original generated input/output attachment, not a second scalar expression.
pub struct ProductionCanonicalRankedGeneratedValuesV1<'a>(
    &'a SemanticKirGeneratedTerminatorValuesV1,
);
impl ProductionCanonicalRankedGeneratedValuesV1<'_> {
    /// Root-qualified source function/block, not a source-name lookup.
    pub fn source_site(
        &self,
    ) -> (
        SemanticFunctionIdV1,
        SemanticFunctionIdV1,
        SemanticBlockIdV1,
    ) {
        (
            self.0.correspondence_owner,
            self.0.semantic_function,
            self.0.semantic_block,
        )
    }
    /// Original destination local.
    pub const fn destination_local(&self) -> SemanticLocalIdV1 {
        self.0.destination_local
    }
    /// Actual input and generated output values in that function's graph namespace.
    pub const fn values(&self) -> (ValueId, ValueId) {
        (self.0.input, self.0.output)
    }
}
/// Complete existing call-component attachment; these values are inert locators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionCanonicalRankedCallComponentV1 {
    /// Helper return value and optional return conversion.
    Return {
        /// Actual source graph value.
        input: ValueId,
        /// Operation ordinal in the return block.
        conversion: Option<u32>,
    },
    /// Caller continuation slot and optional transport conversion.
    Transport {
        /// Actual edge slot.
        slot: u32,
        /// Operation ordinal in the caller continuation.
        conversion: Option<u32>,
    },
}
/// Exact destination finishing mode, not permission to write the pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionCanonicalRankedCallDestinationV1 {
    /// SSA or ignored local, with no result store.
    Local,
    /// Retained private local result store.
    Retained {
        /// Actual prepared pointer.
        pointer: ValueId,
        /// Exact memory attributes.
        access: MemoryAccess,
    },
    /// Projected destination prepared before evaluating call operands.
    Projected {
        /// Actual prepared pointer.
        pointer: ValueId,
        /// Exact memory attributes.
        access: MemoryAccess,
    },
}
/// Borrow of the complete existing call/return transport row and component pool.
/// Source replay and the existing independent call checker retain their authority.
pub struct ProductionCanonicalRankedCallTransportV1<'a> {
    row: &'a SemanticKirCallReturnV1,
    components: &'a [CallResultComponentV1],
}
impl ProductionCanonicalRankedCallTransportV1<'_> {
    /// Complete source occurrence identity.
    pub fn source_site(
        &self,
    ) -> (
        SemanticFunctionIdV1,
        SemanticFunctionIdV1,
        SemanticBlockIdV1,
    ) {
        (
            self.row.correspondence_owner,
            self.row.semantic_function,
            self.row.semantic_block,
        )
    }
    /// Argument preparation start, actual call, and destination end; None for a return.
    pub fn call_operation_span(&self) -> Option<(u32, u32, u32)> {
        match self.row.kind {
            SemanticKirCallReturnKindV1::Call {
                arguments_first,
                call_operation,
                destination_end,
                ..
            } => Some((arguments_first, call_operation, destination_end)),
            SemanticKirCallReturnKindV1::Return { .. } => None,
        }
    }
    /// Complete destination mode; None for a return.
    pub fn destination(&self) -> Option<ProductionCanonicalRankedCallDestinationV1> {
        let SemanticKirCallReturnKindV1::Call { destination, .. } = self.row.kind else {
            return None;
        };
        Some(match destination {
            SemanticKirCallDestinationV1::Local => {
                ProductionCanonicalRankedCallDestinationV1::Local
            }
            SemanticKirCallDestinationV1::Retained { pointer, access } => {
                ProductionCanonicalRankedCallDestinationV1::Retained { pointer, access }
            }
            SemanticKirCallDestinationV1::Projected { pointer, access } => {
                ProductionCanonicalRankedCallDestinationV1::Projected { pointer, access }
            }
        })
    }
    /// Includes explicit zero-component returns.
    pub fn component_count(&self) -> usize {
        self.row.components().count as usize
    }
    /// Exact component in the validated owner-held pool.
    pub fn component(&self, ordinal: usize) -> Option<ProductionCanonicalRankedCallComponentV1> {
        let range = self.row.components();
        if ordinal >= range.count as usize {
            return None;
        }
        self.components
            .get(range.first as usize + ordinal)
            .map(|row| match *row {
                CallResultComponentV1::Return { input, conversion } => {
                    ProductionCanonicalRankedCallComponentV1::Return { input, conversion }
                }
                CallResultComponentV1::Transport { slot, conversion } => {
                    ProductionCanonicalRankedCallComponentV1::Transport { slot, conversion }
                }
            })
    }
}

fn cr_add_span_v1<'a>(
    rows: &mut CrSourceRowsV1<'a>,
    inventory: &CanonicalKirInventoryV1<'_>,
    calls: &ProductionCanonicalCallsV1<'_>,
    association: usize,
    block: BlockId,
    first: u32,
    count: u32,
    site: ProductionCanonicalRankedSourceSiteV1<'a>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrResultV1<()> {
    let (block, operations) =
        cr_span_range_v1(inventory, calls, association, block, first, count, budget)?;
    let span = rows.spans.len();
    for operation in operations.clone() {
        cr_push_v1(
            &mut rows.origins,
            CrOriginV1 {
                operation,
                association,
                span,
            },
            budget,
        )?;
    }
    cr_push_v1(
        &mut rows.spans,
        ProductionCanonicalRankedSpanV1 {
            association,
            block,
            operations,
            site,
        },
        budget,
    )
}
