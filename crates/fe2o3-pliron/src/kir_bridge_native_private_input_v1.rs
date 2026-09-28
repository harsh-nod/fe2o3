// Closed owner-neutral native contract. Only the two authentic graph import
// adapters can implement it; callers cannot supply stage producers or rows.
mod native_private_seal {
    pub(super) trait Sealed {}
}

pub(crate) trait NativePrivateInputV1: native_private_seal::Sealed {
    fn context(&self) -> &Context;
    fn function(&self) -> &FuncOp;
    fn ordinal(&self) -> usize;
    fn epoch(&self) -> u64;
    fn operation_count(&self) -> usize;
    fn authenticate(&self, context: &Context, function: &FuncOp) -> bool;
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
