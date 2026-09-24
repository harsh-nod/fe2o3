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

/// A descriptive ranked extent interpretation, never production authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionConditionalRankedExtentV1 {
    /// Even `Argument(0)` remains an uninterpreted ranked operand here.
    Unbound(ProductionRankedValueV1),
    /// A retained proposal rederived against this borrow's exact canonical facts.
    /// Candidate verification/source replay and runtime premises remain external.
    CanonicalOutputLength {
        /// Exact ranked dimension operand; its ordinal carries no meaning.
        operand: ProductionRankedValueV1,
        /// Exact `SliceLength(output_value)` result in the retained canonical owner.
        length: ValueId,
    },
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
    /// No checked-view extent provenance was retained for this access.
    MissingExtentSource,
    /// Extent provenance disagrees with the canonical source or exact ranked access.
    ExtentSource,
    /// Another access proposes a competing meaning for the selected extent/view.
    ConflictingExtentSource,
    /// The selected ranked index is not uniquely the dynamic global-X invocation.
    ExtentIndex,
    /// The candidate uses the extent outside the supported view/bounds roles.
    ExtentUse,
    /// No exact index-less-than-extent success edge leads to the selected write.
    ExtentGuard,
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
/// establish source/ranked translation, reference
/// argument identity, reference-value equivalence, or any runtime premise. Both
/// RequestEffectRefinement and RequireEffectRefinement are inspected as data;
/// neither supplies authority to this query. Production admission remains gated
/// on existing source replay, evidence and final-graph checks.
/// Unique matching view rows do not establish whole-candidate SSA validity.
/// The initial extent is unbound. Explicit rederivation can interpret its narrow
/// checked-view role using canonical facts, but is still not source replay.
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
    /// Exact ranked operand; rederivation additionally checks dynamic global X.
    pub fn ranked_index(&self) -> ProductionRankedValueV1 {
        self.contract.indices()[0]
    }
    /// Ranked extent interpretation; initially unbound until explicit rederivation.
    pub const fn dynamic_extent(&self) -> ProductionConditionalRankedExtentV1 {
        self.dynamic_extent
    }
    /// Unchanged canonical address-arithmetic premise, not discharged here.
    pub const fn address_domain(&self) -> ConditionalTotalViewAddressDomainV1 {
        self.binding.coverage().address_domain()
    }

    /// Checks the borrowed ranked CFG's conditional, single-write structure.
    ///
    /// Rederives the existing extent relation on the SAME ledger and requires
    /// the canonical owner's retained-storage floor. Constructor validation of
    /// definitions, scope and edges is reused; no graph is copied or created.
    /// Each bounds case takes at most the recipe's block count. Literal branch
    /// operands are found by prepaid bounded scans of that same recipe.
    ///
    /// Read joins reuse metered canonical/argument scratch and retain no map.
    /// Unsupported conditions, block arguments, non-total expressions and
    /// unmatched effects refuse, including in unreachable blocks. Traps and cycles
    /// refuse when feasible; only exact supported edges establish infeasibility.
    /// Source translation, value, ownership, reference and runtime obligations
    /// remain external, including the unchanged address-domain premise and N <= G.
    pub fn check_ranked_coverage_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<ProductionConditionalRankedCoverageV1<'a>, ProductionConditionalRankedCoverageErrorV1>
    {
        super::production_conditional_ranked_coverage_v1::check_ranked_coverage_v1(self, budget)
    }

    /// Rederives the retained rank-one extent proposal from exact canonical facts.
    ///
    /// This consumes no graph and retains no new map. Input reads reuse the
    /// canonical/checked-argument scratch scopes on the caller's ledger.
    /// Work is prepaid on the SAME caller ledger; the canonical retained-storage
    /// floor must still be live. Missing provenance is a refusal, including for
    /// `Argument(0)`. Only an argument extent, one dynamic global-X index and the
    /// closed checked-view use fragment are supported. Unsupported operations
    /// fail closed rather than hiding an unrelated extent use.
    ///
    /// This is an internal provenance prerequisite, not authenticated translation
    /// or total-coverage credit. Existing candidate verification/source replay
    /// must precede any authoritative consumer. Reference argument/value relations
    /// and all runtime conditions remain unproved. In particular GlobalLaunch
    /// address representability remains required even when the output length is
    /// zero; GuardedOutput retains its existing output-span/zero-offset premise.
    /// The matching guard is not a dominance or complete CFG proof. Coverage
    /// separately checks any intervening input guards before the selected write.
    pub fn rederive_output_extent_v1(mut self, budget: &mut Budget<'_>) -> JoinResult<Self> {
        budget.charge_work(4)?;
        if budget.storage() < self.binding.owner().retained_analysis_storage_v1() {
            return Err(ResourceError::Accounting.into());
        }
        let write = ranked_write(self.candidate, self.source, budget)?;
        let operand = match self.dynamic_extent {
            ProductionConditionalRankedExtentV1::Unbound(operand)
            | ProductionConditionalRankedExtentV1::CanonicalOutputLength { operand, .. } => operand,
        };
        rederive_extent_source(
            self.candidate,
            self.source,
            &write,
            operand,
            self.binding.source_argument(),
            Some(self.binding),
            budget,
        )?;
        // Coverage is sealed to this exact verified owner: its length is the
        // output's SliceLength, and its predicate is global-X < that length.
        self.dynamic_extent = ProductionConditionalRankedExtentV1::CanonicalOutputLength {
            operand,
            length: self.binding.coverage().length(),
        };
        Ok(self)
    }
}

fn rederive_extent_source(
    candidate: NativeRankedSourceCandidateV1<'_>,
    source: &ProductionRankedAccessSourceV1,
    write: &RankedWrite,
    extent: ProductionRankedValueV1,
    source_argument: u32,
    binding: Option<&ProductionConditionalOutputBindingV1<'_>>,
    budget: &mut Budget<'_>,
) -> JoinResult<()> {
    budget.charge_work(8)?;
    let proposal = source
        .output_extent()
        .ok_or(JoinError::MissingExtentSource)?;
    if proposal.source_argument() != source_argument
        || proposal.view() != write.view
        || proposal.index() != write.index
        || proposal.extent() != extent
        || write.view == write.index
        || !matches!(extent, ProductionRankedValueV1::Argument(_))
    {
        return Err(JoinError::ExtentSource);
    }
    for row in candidate.access_sources() {
        budget.charge_work(4)?;
        if let Some(other) = row.output_extent()
            && (other.extent() == extent || other.view() == write.view)
            && !std::ptr::eq(row, source)
        {
            return Err(JoinError::ConflictingExtentSource);
        }
    }
    check_extent_uses(candidate, write, extent, binding, budget)
}

fn check_extent_uses(
    candidate: NativeRankedSourceCandidateV1<'_>,
    write: &RankedWrite,
    extent: ProductionRankedValueV1,
    binding: Option<&ProductionConditionalOutputBindingV1<'_>>,
    budget: &mut Budget<'_>,
) -> JoinResult<()> {
    use ProductionRankedOperationV1 as Op;
    use fe2o3_pliron::ProductionRankedTerminatorV1 as Term;
    let mut index_seen = false;
    let mut write_guard = false;
    for (block_index, block) in candidate.kernel().blocks().iter().enumerate() {
        budget.charge_work(4)?;
        if block.index_argument_count() != 0 {
            return Err(JoinError::ExtentUse);
        }
        for (operation_index, operation) in block.operations().iter().enumerate() {
            budget.charge_work(12)?;
            let result = match operation {
                Op::InvocationIndex {
                    result,
                    dimension,
                    launch_extent,
                } => {
                    if ProductionRankedValueV1::Local(*result) != write.index
                        || *dimension != 0
                        || *launch_extent != 0
                        || index_seen
                    {
                        return Err(JoinError::ExtentIndex);
                    }
                    index_seen = true;
                    continue;
                }
                Op::View {
                    result,
                    dynamic_extents,
                    ..
                }
                | Op::ViewInSpace {
                    result,
                    dynamic_extents,
                    ..
                } => {
                    if ProductionRankedValueV1::Local(*result) == write.view {
                        if dynamic_extents.as_slice() != [extent] {
                            return Err(JoinError::ExtentUse);
                        }
                    } else if !matched_read_role_v1(
                        binding,
                        candidate,
                        write,
                        Some(ProductionRankedValueV1::Local(*result)),
                        None,
                        budget,
                    )? {
                        return Err(JoinError::ExtentUse);
                    }
                    continue;
                }
                Op::Access {
                    kind: AccessKindAttr::Read,
                    ..
                } => {
                    if !matched_read_role_v1(
                        binding,
                        candidate,
                        write,
                        None,
                        Some((block_index as u32, operation_index as u32)),
                        budget,
                    )? {
                        return Err(JoinError::ExtentUse);
                    }
                    None
                }
                Op::ValueAccess {
                    kind: AccessKindAttr::Write,
                    view,
                    indices,
                    ..
                } => {
                    if block_index != write.site.block() as usize
                        || operation_index != write.site.operation() as usize
                        || *view != write.view
                        || indices.as_slice() != [write.index]
                        || matches!(operation, Op::ValueAccess { value, .. } if *value == extent)
                    {
                        return Err(JoinError::ExtentUse);
                    }
                    None
                }
                // Semantic expression symbols/loads are in a separate source
                // namespace, not ranked operands. No expression proof is made.
                Op::IndexConstant { result, .. }
                | Op::IndexUnknown { result }
                | Op::SemanticConstant { result, .. }
                | Op::SemanticSymbol { result, .. }
                | Op::SemanticExpression { result, .. } => Some(*result),
                Op::IndexBinary {
                    result, lhs, rhs, ..
                }
                | Op::SemanticBinary {
                    result, lhs, rhs, ..
                } => {
                    if *lhs == extent || *rhs == extent {
                        return Err(JoinError::ExtentUse);
                    }
                    Some(*result)
                }
                Op::OwnershipContract { view, .. } if *view == write.view => None,
                Op::ExecutionLayout { .. } => None,
                Op::RequestEffectRefinement { contract, .. }
                | Op::RequireEffectRefinement { contract, .. } => {
                    if contract.gpu_write_site() != write.site {
                        return Err(JoinError::ExtentUse);
                    }
                    for values in [
                        contract.indices(),
                        contract.gpu_coordinates(),
                        contract.reference_coordinates(),
                    ] {
                        for value in values {
                            budget.charge_work(1)?;
                            if *value == extent {
                                return Err(JoinError::ExtentUse);
                            }
                        }
                    }
                    for value in [
                        contract.view(),
                        contract.gpu_domain(),
                        contract.reference_domain(),
                        contract.gpu_precondition(),
                        contract.reference_precondition(),
                        contract.gpu_value(),
                        contract.reference_value(),
                    ] {
                        budget.charge_work(1)?;
                        if value == extent {
                            return Err(JoinError::ExtentUse);
                        }
                    }
                    None
                }
                _ => return Err(JoinError::ExtentUse),
            };
            if result.is_some_and(|result| {
                let value = ProductionRankedValueV1::Local(result);
                value == write.index || value == write.view
            }) {
                return Err(JoinError::ExtentIndex);
            }
        }
        budget.charge_work(6)?;
        match block.terminator() {
            Term::IndexLessThan {
                lhs,
                rhs,
                true_block,
                false_block,
            } if *rhs == extent && *lhs == write.index && true_block != false_block => {
                if *true_block == write.site.block()
                    || binding.is_some_and(|binding| binding.coverage().read_count() != 0)
                {
                    write_guard = true;
                }
            }
            Term::IndexLessThan { lhs, rhs, .. } | Term::IndexEqual { lhs, rhs, .. }
                if *lhs != extent && *rhs != extent => {}
            Term::Branch { .. } | Term::Return | Term::Trap => {}
            _ => return Err(JoinError::ExtentUse),
        }
    }
    if !index_seen {
        return Err(JoinError::ExtentIndex);
    }
    if !write_guard {
        return Err(JoinError::ExtentGuard);
    }
    Ok(())
}

include!("production_conditional_ranked_reads_v1.rs");

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
    /// Only one value-carrying scalar global write is supported. Input views
    /// must separately join exact canonical reads during extent rederivation.
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
            AccessKindAttr::Write,
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

pub(super) fn canonical_store_source(
    function: &Function,
    correspondence: &SemanticKirCorrespondenceV1,
    association: &SemanticKirFunctionCorrespondenceV1,
    location: FunctionOperationLocation,
    expected_kind: AccessKindAttr,
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
                    || kind != expected_kind
                    || space != MemorySpaceAttr::Global
                    || atomic.is_some()
                    || !matches!(
                        (expected_kind, &operation.kind),
                        (
                            AccessKindAttr::Write,
                            OperationKind::Store { .. } | OperationKind::GuardedStore { .. }
                        ) | (
                            AccessKindAttr::Read,
                            OperationKind::Load { .. } | OperationKind::GuardedLoad { .. }
                        )
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

pub(super) fn ranked_source<'a>(
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
    value: ProductionRankedValueV1,
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
        ProductionRankedOperationV1::ValueAccess {
            kind: AccessKindAttr::Write,
            view,
            indices,
            value,
        } => (view, indices, *value),
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
        || write.value != contract.gpu_value()
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
