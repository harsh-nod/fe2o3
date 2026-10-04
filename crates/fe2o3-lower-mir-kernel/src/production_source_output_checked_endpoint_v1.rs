// Both routes borrow an actual checked optimizer owner. Policy evidence stays
// with its original owner; this endpoint does not convert owners or receipts.
#[derive(Clone, Copy)]
enum SourceOutputCheckedEndpointV1<'output> {
    Optimizer(&'output fe2o3_pliron::CheckedNeutralKernelIrOwnerV1),
    OptimizerPolicy3(&'output fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1),
}

impl<'output> SourceOutputCheckedEndpointV1<'output> {
    fn owner(self) -> &'output fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        match self {
            Self::Optimizer(checked) => checked.owner(),
            Self::OptimizerPolicy3(checked) => checked.owner(),
        }
    }

    fn occurrences(self) -> &'output fe2o3_pliron::KirNeutralOccurrenceRowsV1 {
        match self {
            Self::Optimizer(checked) => checked.occurrences(),
            Self::OptimizerPolicy3(checked) => checked.occurrences(),
        }
    }

    fn native_input_audit_bytes(self) -> &'output [u8] {
        match self {
            Self::Optimizer(checked) => checked.native_input_audit_bytes(),
            Self::OptimizerPolicy3(checked) => checked.native_input_audit_bytes(),
        }
    }

    fn storage(self) -> fe2o3_pliron::KirCheckedNeutralOptimizationStorageV1 {
        match self {
            Self::Optimizer(checked) => checked.storage(),
            Self::OptimizerPolicy3(checked) => checked.storage(),
        }
    }
}

/// Connects sealed source N to the same actual checked policy-3 output O.
///
/// The shared constructor checks the original N pointer, all B canonical bytes
/// against the retained optimizer input, source replay, and the actual B/O
/// occurrence relation. It borrows the original policy-3 owner with its distinct
/// execution evidence; no historical owner conversion, graph clone, receipt
/// decoder, or optimizer invocation occurs here. The coordinate witness alone
/// does not authenticate a target: obtain it from the exact production binder.
/// Existing helper-policy refusals and final admission gates remain unchanged.
///
/// The live source payload, B, checked O/history/execution and coordinate view
/// must already be reserved on this caller ledger. Reserve the returned view
/// storage before further allocation. Its actual header includes the borrowed
/// endpoint tag. Source replay retains its existing separately bounded engine;
/// this API creates no replacement budget and grants no proof or launch authority.
///
/// Historical V1 owners cannot substitute for the policy-3 owner:
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     ProductionPreRankedKirOwnerV1, derive_source_output_occurrences_policy3_v1,
/// };
/// use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerV1;
/// fn no_historical_substitution(
///     source: &ProductionPreRankedKirOwnerV1,
///     coordinates: &CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
///     checked: &CheckedNeutralKernelIrOwnerV1,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
/// ) {
///     derive_source_output_occurrences_policy3_v1(source, coordinates, checked, budget).unwrap();
/// }
/// ```
///
/// The view retains the borrow of the actual policy-3 owner:
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     ProductionPreRankedKirOwnerV1, derive_source_output_occurrences_policy3_v1,
/// };
/// use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;
/// fn cannot_drop_borrowed_output(
///     source: &ProductionPreRankedKirOwnerV1,
///     coordinates: &CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
///     checked: CheckedNeutralKernelIrOwnerPolicy3V1,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
/// ) {
///     let (view, _) = derive_source_output_occurrences_policy3_v1(
///         source, coordinates, &checked, budget,
///     ).unwrap();
///     drop(checked);
///     let _ = view.output();
/// }
/// ```
pub fn derive_source_output_occurrences_policy3_v1<'source, 'output>(
    source: &'source ProductionPreRankedKirOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<
        'source,
        'source,
    >,
    checked_output: &'output fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        ProductionSourceOutputOccurrencesV1<'source, 'output>,
        ProductionSourceOutputStorageV1,
    ),
    ProductionSourceOutputErrorV1,
> {
    derive_source_output_occurrences_with_endpoint_v1(
        source,
        coordinates,
        SourceOutputCheckedEndpointV1::OptimizerPolicy3(checked_output),
        budget,
    )
}
