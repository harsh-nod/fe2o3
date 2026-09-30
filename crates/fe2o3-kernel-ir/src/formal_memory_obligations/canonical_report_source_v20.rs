//! Borrow-only access to the exact retained report source context.
use super::*;

/// Lexical same-owner source queries beside the unchanged complete report.
///
/// Its source references and coordinates are descriptive. Branch truth, legal
/// launch authority, source/output correspondence and memory admission remain
/// separate obligations. No second CFG, definition map or budget is created.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::*;
/// fn escape<'a, 'b>(view: &CanonicalFormalReportViewV19<'a, 'b>, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) -> &'static CanonicalFormalSourceScopeV20<'static, 'static> {
///     view.source_scope_v20(budget).unwrap()
/// }
/// ```
pub struct CanonicalFormalSourceScopeV20<'report, 'owner> {
    inner: &'report ActualOwnerReportViewV19<'report, 'owner>,
    launch: CanonicalFormalLaunchInputV19,
    width: FormalIndexWidth,
}

impl<'report, 'owner> CanonicalFormalSourceScopeV20<'report, 'owner> {
    pub(super) fn new(
        inner: &'report ActualOwnerReportViewV19<'report, 'owner>,
        launch: CanonicalFormalLaunchInputV19,
        width: FormalIndexWidth,
    ) -> Self {
        Self {
            inner,
            launch,
            width,
        }
    }
    /// The exact unchanged original report, including every reason/conflict row.
    pub fn report(&self) -> &FormalMemoryObligationAnalysis {
        self.inner.analysis()
    }
    /// The original actual canonical owner, never a reconstructed token.
    pub fn original_owner(&self) -> &VerifiedCanonicalKernelIrModuleV18 {
        self.inner.original_owner()
    }
    /// The exact original entry function used by retained source indices.
    pub fn original_function(&self) -> &Function {
        self.inner.original_function()
    }
    /// Original kernel ordinal in that owner's kernel roster.
    pub fn root_index(&self) -> usize {
        self.inner.root_index()
    }
    /// Original descriptive launch input; this getter grants no launch authority.
    pub fn launch_input(&self) -> CanonicalFormalLaunchInputV19 {
        self.launch
    }
    /// Original descriptive logical index width, not a pointer-width assertion.
    pub fn index_width(&self) -> FormalIndexWidth {
        self.width
    }
    /// Checks actual owner/root and original account; same-account errors latch.
    /// Foreign account identity refuses without poisoning the original scope.
    pub fn check(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root: usize,
        budget: &mut Budget<'_>,
    ) -> PublicResult<()> {
        self.inner
            .source_queries_v20()
            .check(owner, root, budget)
            .map_err(Into::into)
    }
    /// Number of retained CFG block ordinals, paid without scanning the graph.
    pub fn block_count(&self, budget: &mut Budget<'_>) -> PublicResult<usize> {
        self.inner
            .source_queries_v20()
            .block_count(budget)
            .map_err(Into::into)
    }
    /// Actual source block at the retained CFG ordinal; no block is cloned.
    pub fn block_at(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> PublicResult<Option<&'owner crate::BasicBlock>> {
        self.inner
            .source_queries_v20()
            .block_at(ordinal, budget)
            .map_err(Into::into)
    }
    /// Paid original block-ID lookup; absent IDs remain absent.
    pub fn block_ordinal(
        &self,
        block: BlockId,
        budget: &mut Budget<'_>,
    ) -> PublicResult<Option<usize>> {
        self.inner
            .source_queries_v20()
            .block_ordinal(block, budget)
            .map_err(Into::into)
    }
    /// Paid reachability query from the retained original CFG.
    pub fn reachable(&self, block: BlockId, budget: &mut Budget<'_>) -> PublicResult<bool> {
        self.inner
            .source_queries_v20()
            .reachable(block, budget)
            .map_err(Into::into)
    }
    /// Number of retained dense definition ordinals, without copying the map.
    pub fn definition_count(&self, budget: &mut Budget<'_>) -> PublicResult<usize> {
        self.inner
            .source_queries_v20()
            .definition_count(budget)
            .map_err(Into::into)
    }
    /// Paid dense ordinal/type lookup and optional reachable single-result
    /// defining operation. Parameters, multi-result and dead operations do not
    /// acquire an invented scalar producer; their original ordinal/type remains.
    pub fn definition(
        &self,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> PublicResult<Option<(usize, &'owner Type, Option<&'owner Operation>)>> {
        self.inner
            .source_queries_v20()
            .definition(value, budget)
            .map_err(Into::into)
    }
    /// Proves only CFG structure: a non-entry target has exactly source as its
    /// distinct predecessor, source is reachable, and target dominates access.
    /// The caller must still authenticate distinct conditional targets and the
    /// original Boolean/selector meaning; this is not a predicate truth permit.
    pub fn unique_predecessor_dominates(
        &self,
        source: BlockId,
        target: BlockId,
        access: BlockId,
        budget: &mut Budget<'_>,
    ) -> PublicResult<bool> {
        self.inner
            .source_queries_v20()
            .unique_predecessor_dominates(source, target, access, budget)
            .map_err(Into::into)
    }
}
