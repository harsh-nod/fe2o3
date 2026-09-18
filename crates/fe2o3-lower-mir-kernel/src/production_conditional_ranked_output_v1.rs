//! Borrowed, descriptive correspondence for one conditional canonical output.
//! Ranked candidates and their correspondence rows remain untrusted inputs.

use super::*;
use crate::NativeRankedSourceCandidateV1;
use dialect_kernel::{AccessKindAttr, DYNAMIC_EXTENT, MemorySpaceAttr};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
    ConditionalTotalViewAddressDomainV1,
};
use fe2o3_pliron::{ProductionEffectRefinementContractV2, ProductionGpuWriteSiteV2};

/// A ranked extent operand with no established canonical-length interpretation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionConditionalRankedExtentV1 {
    /// Even `Argument(0)` remains an uninterpreted ranked operand here.
    Unbound(ProductionRankedValueV1),
}

/// Failure of the narrow descriptive join, not a failed runtime condition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionConditionalRankedOutputErrorV1 {
    /// The shared caller ledger cannot admit the traversal.
    Resource(ResourceError),
    /// Candidate root or launch rank differs from the canonical binding.
    CandidateRoot,
    /// The canonical location does not identify the supported global store.
    CanonicalStore,
    /// The store has no valid owner-qualified source span.
    SourceSpan,
    /// Several spans claim the store or its source construct.
    AmbiguousSourceSpan,
    /// No exact source-access occurrence maps to a ranked operation.
    SourceAccess,
    /// Source or ranked access coordinates have multiple correspondence rows.
    AmbiguousSourceAccess,
    /// The mapped operation is not a rank-one scalar write.
    RankedWrite,
    /// No effect contract agrees with the exact ranked write.
    Contract,
    /// More than one contract claims the ranked write.
    AmbiguousContract,
    /// The output view has a missing or inconsistent definition.
    View,
    /// Multiple view definitions claim the output operand.
    AmbiguousView,
}

impl From<ResourceError> for ProductionConditionalRankedOutputErrorV1 {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for ProductionConditionalRankedOutputErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(out),
            _ => write!(out, "conditional ranked output correspondence: {self:?}"),
        }
    }
}

impl std::error::Error for ProductionConditionalRankedOutputErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

type JoinResult<T> = Result<T, ProductionConditionalRankedOutputErrorV1>;
use ProductionConditionalRankedOutputErrorV1 as JoinError;

/// Exact occurrence checks against an inert ranked candidate, not authentication.
///
/// This borrow preserves the canonical binding and the candidate; it does not
/// establish source/ranked translation, index or extent equivalence, reference
/// argument identity, reference-value equivalence, or any runtime premise. Both
/// RequestEffectRefinement and RequireEffectRefinement are inspected as data;
/// neither supplies authority to this query. Production admission remains gated
/// on existing source replay, evidence and final-graph checks.
/// Unique matching view rows do not establish whole-candidate SSA validity.
pub struct ProductionConditionalRankedOutputV1<'a> {
    binding: &'a ProductionConditionalOutputBindingV1<'a>,
    candidate: NativeRankedSourceCandidateV1<'a>,
    source: &'a ProductionRankedAccessSourceV1,
    contract: &'a ProductionEffectRefinementContractV2,
    dynamic_extent: ProductionConditionalRankedExtentV1,
}

impl fmt::Debug for ProductionConditionalRankedOutputV1<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("ProductionConditionalRankedOutputV1")
            .field("source", self.source)
            .field("gpu_write_site", &self.contract.gpu_write_site())
            .field(
                "reference_output_site",
                &self.contract.reference_output_site(),
            )
            .field("view", &self.contract.view())
            .field("indices", &self.contract.indices())
            .field("dynamic_extent", &self.dynamic_extent)
            .field("address_domain", &self.address_domain())
            .finish()
    }
}

impl<'a> ProductionConditionalRankedOutputV1<'a> {
    /// Exact canonical coverage and source-argument binding retained by this query.
    pub const fn binding(&self) -> &'a ProductionConditionalOutputBindingV1<'a> {
        self.binding
    }
    /// Still an inert candidate, not a freshly verified or authenticated graph.
    pub const fn candidate(&self) -> NativeRankedSourceCandidateV1<'a> {
        self.candidate
    }
    /// Unique source-access row selected by the canonical store occurrence.
    pub const fn source(&self) -> &'a ProductionRankedAccessSourceV1 {
        self.source
    }
    /// The output argument uses the producer's logical ABI normalization, not
    /// the raw CPU parameter ordinal. This query does not validate that relation.
    pub const fn contract(&self) -> &'a ProductionEffectRefinementContractV2 {
        self.contract
    }
    /// Ranked write coordinates shared by the selected access and contract.
    pub const fn gpu_write_site(&self) -> ProductionGpuWriteSiteV2 {
        self.contract.gpu_write_site()
    }
    /// Output view operand shared by the ranked access and contract.
    pub const fn view(&self) -> ProductionRankedValueV1 {
        self.contract.view()
    }
    /// Exact ranked operand only; canonical index equivalence is not established.
    pub fn ranked_index(&self) -> ProductionRankedValueV1 {
        self.contract.indices()[0]
    }
    /// Ranked extent operand, still unrelated to a canonical slice length.
    pub const fn dynamic_extent(&self) -> ProductionConditionalRankedExtentV1 {
        self.dynamic_extent
    }
    /// Unchanged canonical address-arithmetic premise, not discharged here.
    pub const fn address_domain(&self) -> ConditionalTotalViewAddressDomainV1 {
        self.binding.coverage().address_domain()
    }
}

impl ProductionConditionalOutputBindingV1<'_> {
    /// Joins the canonical store through exact source spans to one ranked contract.
    ///
    /// Requires the owner's retained_analysis_storage_v1() reservation on the
    /// SAME caller ledger used for coverage and binding. Borrowed candidate
    /// storage remains in its existing allocation domain. The query allocates
    /// no heap storage, uses bounded linear scans, charges before visiting rows
    /// and operations, and leaves live/peak storage unchanged on every outcome.
    /// No WorkMeter, executable graph, copied argument map or proof is created.
    ///
    /// Only one scalar global write and one rank-one dynamic view are supported.
    /// No allocation-level correspondence fallback is accepted. The ranked
    /// dynamic extent remains Unbound even when it is Argument(0). Canonical
    /// predicate/length facts are available through binding(), not inferred from
    /// the contract's domain or precondition expressions.
    pub fn inspect_ranked_output_v1<'a>(
        &'a self,
        candidate: NativeRankedSourceCandidateV1<'a>,
        budget: &mut Budget<'_>,
    ) -> JoinResult<ProductionConditionalRankedOutputV1<'a>> {
        budget.charge_work(4)?;
        if budget.storage() < self.owner().retained_analysis_storage_v1() {
            return Err(ResourceError::Accounting.into());
        }
        if candidate.semantic_root() != self.association().correspondence_owner().index()
            || candidate.launch_rank() != self.source_launch().source_rank()
        {
            return Err(JoinError::CandidateRoot);
        }
        let site = canonical_store_source(
            self.coverage().function(),
            &self.owner().correspondence,
            self.association(),
            self.coverage().store_location(),
            budget,
        )?;
        let source = ranked_source(candidate.access_sources(), site, budget)?;
        let write = ranked_write(candidate, source, budget)?;
        let (extent, contract) = ranked_view_and_contract(
            candidate,
            &write,
            self.source_argument(),
            self.coverage().element_bytes(),
            budget,
        )?;
        Ok(ProductionConditionalRankedOutputV1 {
            binding: self,
            candidate,
            source,
            contract: contract.ok_or(JoinError::Contract)?,
            dynamic_extent: extent,
        })
    }
}

#[derive(Clone, Copy)]
struct SourceSpan {
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: u32,
    statement: Option<u32>,
    kir_block: BlockId,
    first: u32,
    count: u32,
}

fn source_spans(correspondence: &SemanticKirCorrespondenceV1) -> impl Iterator<Item = SourceSpan> {
    correspondence
        .statement_operation_spans()
        .iter()
        .map(|row| SourceSpan {
            owner: row.correspondence_owner(),
            function: row.semantic_function(),
            block: row.semantic_block().index(),
            statement: Some(row.statement_ordinal()),
            kir_block: row.kernel_ir_block(),
            first: row.first_operation_ordinal(),
            count: row.operation_count(),
        })
        .chain(
            correspondence
                .terminator_operation_spans()
                .iter()
                .map(|row| SourceSpan {
                    owner: row.correspondence_owner(),
                    function: row.semantic_function(),
                    block: row.semantic_block().index(),
                    statement: None,
                    kir_block: row.kernel_ir_block(),
                    first: row.first_operation_ordinal(),
                    count: row.operation_count(),
                }),
        )
}

fn canonical_store_source(
    function: &Function,
    correspondence: &SemanticKirCorrespondenceV1,
    association: &SemanticKirFunctionCorrespondenceV1,
    location: FunctionOperationLocation,
    budget: &mut Budget<'_>,
) -> JoinResult<SemanticAccessSiteV1> {
    let body = function.body.as_ref().ok_or(JoinError::CanonicalStore)?;
    let mut block = None;
    for row in &body.blocks {
        budget.charge_work(2)?;
        if row.id == location.block && block.replace(row).is_some() {
            return Err(JoinError::CanonicalStore);
        }
    }
    let block = block.ok_or(JoinError::CanonicalStore)?;
    let mut selected = None;
    for span in source_spans(correspondence) {
        budget.charge_work(8)?;
        if span.owner != association.correspondence_owner()
            || span.function != association.semantic_function()
            || span.kir_block != location.block
        {
            continue;
        }
        let end = span
            .first
            .checked_add(span.count)
            .ok_or(ResourceError::Arithmetic)?;
        if end as usize > block.operations.len() {
            return Err(JoinError::SourceSpan);
        }
        if (span.first as usize..end as usize).contains(&location.operation_index)
            && selected.replace(span).is_some()
        {
            return Err(JoinError::AmbiguousSourceSpan);
        }
    }
    let span = selected.ok_or(JoinError::SourceSpan)?;
    // One source construct must not have several independently numbered spans.
    let mut spans = 0_u32;
    for other in source_spans(correspondence) {
        budget.charge_work(5)?;
        if other.owner == span.owner
            && other.function == span.function
            && other.block == span.block
            && other.statement == span.statement
        {
            spans = spans.checked_add(1).ok_or(ResourceError::Arithmetic)?;
        }
    }
    if spans != 1 {
        return Err(JoinError::AmbiguousSourceSpan);
    }
    let mut access_ordinal = 0_u32;
    let mut selected_access = None;
    for operation_index in span.first as usize..=location.operation_index {
        budget.charge_work(2)?;
        let operation = &block.operations[operation_index];
        let mut operation_access_ordinal = 0_u32;
        try_visit_kir_memory_accesses_v1(operation, |(_, kind, space, atomic)| {
            budget.charge_work(5)?;
            // Current canonical coverage excludes all private/local accesses.
            // Refuse them rather than invent a different private-access census.
            if space == MemorySpaceAttr::Private {
                return Err(JoinError::CanonicalStore);
            }
            if operation_index == location.operation_index {
                if operation_access_ordinal != 0
                    || kind != AccessKindAttr::Write
                    || space != MemorySpaceAttr::Global
                    || atomic.is_some()
                    || !matches!(
                        operation.kind,
                        OperationKind::Store { .. } | OperationKind::GuardedStore { .. }
                    )
                {
                    return Err(JoinError::CanonicalStore);
                }
                selected_access = Some(access_ordinal);
            }
            access_ordinal = access_ordinal
                .checked_add(1)
                .ok_or(ResourceError::Arithmetic)?;
            operation_access_ordinal = operation_access_ordinal
                .checked_add(1)
                .ok_or(ResourceError::Arithmetic)?;
            Ok(())
        })?;
    }
    Ok(SemanticAccessSiteV1 {
        block: span.block,
        statement: span.statement,
        ordinal: selected_access.ok_or(JoinError::CanonicalStore)?,
    })
}

fn ranked_source<'a>(
    sources: &'a [ProductionRankedAccessSourceV1],
    site: SemanticAccessSiteV1,
    budget: &mut Budget<'_>,
) -> JoinResult<&'a ProductionRankedAccessSourceV1> {
    let mut selected = None;
    for source in sources {
        budget.charge_work(4)?;
        if source.semantic_block() == site.block
            && source.semantic_statement() == site.statement
            && source.semantic_access_ordinal() == site.ordinal
            && selected.replace(source).is_some()
        {
            return Err(JoinError::AmbiguousSourceAccess);
        }
    }
    let selected = selected.ok_or(JoinError::SourceAccess)?;
    let mut occurrences = 0_u32;
    for source in sources {
        budget.charge_work(3)?;
        if source.ranked_block() == selected.ranked_block()
            && source.ranked_operation() == selected.ranked_operation()
        {
            occurrences = occurrences
                .checked_add(1)
                .ok_or(ResourceError::Arithmetic)?;
        }
    }
    if occurrences != 1 {
        return Err(JoinError::AmbiguousSourceAccess);
    }
    Ok(selected)
}

struct RankedWrite {
    site: ProductionGpuWriteSiteV2,
    view: ProductionRankedValueV1,
    index: ProductionRankedValueV1,
    value: Option<ProductionRankedValueV1>,
}

fn ranked_write(
    candidate: NativeRankedSourceCandidateV1<'_>,
    source: &ProductionRankedAccessSourceV1,
    budget: &mut Budget<'_>,
) -> JoinResult<RankedWrite> {
    budget.charge_work(6)?;
    let operation = candidate
        .kernel()
        .blocks()
        .get(source.ranked_block() as usize)
        .and_then(|block| block.operations().get(source.ranked_operation() as usize))
        .ok_or(JoinError::RankedWrite)?;
    let (view, indices, value) = match operation {
        ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Write,
            view,
            indices,
        } => (view, indices, None),
        ProductionRankedOperationV1::ValueAccess {
            kind: AccessKindAttr::Write,
            view,
            indices,
            value,
        } => (view, indices, Some(*value)),
        _ => return Err(JoinError::RankedWrite),
    };
    let [index] = indices.as_slice() else {
        return Err(JoinError::RankedWrite);
    };
    Ok(RankedWrite {
        site: ProductionGpuWriteSiteV2::new(source.ranked_block(), source.ranked_operation()),
        view: *view,
        index: *index,
        value,
    })
}

fn retain_contract<'a>(
    selected: &mut Option<&'a ProductionEffectRefinementContractV2>,
    contract: &'a ProductionEffectRefinementContractV2,
    write: &RankedWrite,
    budget: &mut Budget<'_>,
) -> JoinResult<()> {
    budget.charge_work(6)?;
    if contract.gpu_write_site() != write.site {
        return Ok(());
    }
    if contract.view() != write.view
        || contract.indices() != [write.index]
        || write
            .value
            .is_some_and(|value| value != contract.gpu_value())
    {
        return Err(JoinError::Contract);
    }
    if selected.replace(contract).is_some() {
        return Err(JoinError::AmbiguousContract);
    }
    Ok(())
}

fn ranked_view_and_contract<'a>(
    candidate: NativeRankedSourceCandidateV1<'a>,
    write: &RankedWrite,
    source_argument: u32,
    element_bytes: u64,
    budget: &mut Budget<'_>,
) -> JoinResult<(
    ProductionConditionalRankedExtentV1,
    Option<&'a ProductionEffectRefinementContractV2>,
)> {
    let mut extent = None;
    let mut contract = None;
    let expected_width = element_bytes
        .checked_mul(8)
        .ok_or(ResourceError::Arithmetic)?;
    let expected_origin = u64::from(source_argument) + 1;
    for block in candidate.kernel().blocks() {
        budget.charge_work(1)?;
        for operation in block.operations() {
            budget.charge_work(10)?;
            match operation {
                ProductionRankedOperationV1::RequestEffectRefinement { contract: row, .. }
                | ProductionRankedOperationV1::RequireEffectRefinement { contract: row, .. } => {
                    retain_contract(&mut contract, row, write, budget)?;
                }
                ProductionRankedOperationV1::View {
                    result,
                    element_width,
                    writable,
                    shape,
                    dynamic_extents,
                    allocation_origin,
                    ..
                }
                | ProductionRankedOperationV1::ViewInSpace {
                    result,
                    element_width,
                    writable,
                    shape,
                    dynamic_extents,
                    allocation_origin,
                    ..
                } if ProductionRankedValueV1::Local(*result) == write.view => {
                    if matches!(operation, ProductionRankedOperationV1::ViewInSpace { memory_space, .. } if *memory_space != MemorySpaceAttr::Global)
                        || !writable
                        || u64::from(*element_width) != expected_width
                        || *allocation_origin != expected_origin
                        || shape.as_slice() != [DYNAMIC_EXTENT]
                    {
                        return Err(JoinError::View);
                    }
                    let [operand] = dynamic_extents.as_slice() else {
                        return Err(JoinError::View);
                    };
                    if extent
                        .replace(ProductionConditionalRankedExtentV1::Unbound(*operand))
                        .is_some()
                    {
                        return Err(JoinError::AmbiguousView);
                    }
                }
                _ => {}
            }
        }
    }
    Ok((extent.ok_or(JoinError::View)?, contract))
}

#[cfg(test)]
#[path = "production_conditional_ranked_output_v1_tests.rs"]
mod tests;
