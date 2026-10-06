// Closed native memory contract. Only authentic graph import adapters can
// implement it; the historical private entrypoints reject mixed profiles.
// Callers cannot supply stage producers, classifications, or expected counts.
mod native_private_seal {
    pub(super) trait Sealed {}
}

/// Analysis domain of an authenticated, complete native function. No caller
/// can supply this contract independently of its sealed graph/epoch adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeCfgDomainV26 {
    AllBlocksReachableV1,
    /// Retain disconnected blocks without granting facts for them. Every native
    /// schema remains checked, and entry reachability is recomputed from every
    /// actual successor before proving bounds on executable accesses.
    EntryReachableSubgraphV26,
}

pub(crate) trait NativePrivateInputV1: native_private_seal::Sealed {
    fn context(&self) -> &Context;
    fn function(&self) -> &FuncOp;
    fn ordinal(&self) -> usize;
    fn epoch(&self) -> u64;
    fn operation_count(&self) -> usize;
    fn cfg_domain_v26(&self) -> NativeCfgDomainV26 {
        NativeCfgDomainV26::AllBlocksReachableV1
    }
    fn supports_conditional_globals_v26(&self) -> bool {
        false
    }
    fn authenticate(&self, context: &Context, function: &FuncOp) -> bool;
    fn conditional_global_counts_v26(&self) -> Option<[usize; 2]> {
        None
    }
    fn operation(
        &self,
        context: &Context,
        pointer: Ptr<Operation>,
    ) -> Option<
        crate::production_analysis::canonical_ranked_checks_v1::private::PrivateOperationKindV1,
    >;
    fn attribute(
        &self,
        context: &Context,
        pointer: Ptr<Operation>,
        key: &str,
        dialect: &str,
        name: &str,
    ) -> bool;
    fn identity_lookup_work(&self) -> Option<usize>;
    fn stage_lookup_work(&self) -> Option<usize>;
    fn run_fixed(
        &self,
        limits: crate::ProductionAnalysisResourceLimitsV1,
        receipt: Option<&mut crate::invocation_receipt_v1::InvocationReceiptV1>,
    ) -> Result<
        crate::canonical_private_v1::CanonicalPrivatePipelineOutcomeV1,
        crate::PipelineErrorV1,
    >;
}
