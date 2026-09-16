// Shared endpoint custody for actual optimizer output and independently checked
// receipt input. The receipt path never creates an optimizer owner.

#[derive(Clone, Copy)]
enum SourceOutputCheckedEndpointV1<'output> {
    Optimizer(&'output fe2o3_pliron::CheckedNeutralKernelIrOwnerV1),
    OptimizerPolicy3(&'output fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1),
    Receipt {
        output: &'output fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        receipt: &'output fe2o3_kernel_ir::InertCanonicalKirTransitionReceiptV1,
        retained: usize,
    },
}

#[derive(Clone, Copy)]
struct SourceOutputEndpointStorageV1(usize);

impl SourceOutputEndpointStorageV1 {
    const fn retained_storage(self) -> usize {
        self.0
    }
}

impl<'output> SourceOutputCheckedEndpointV1<'output> {
    fn owner(self) -> &'output fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        match self {
            Self::Optimizer(checked) => checked.owner(),
            Self::OptimizerPolicy3(checked) => checked.owner(),
            Self::Receipt { output, .. } => output,
        }
    }

    fn storage(self) -> SourceOutputEndpointStorageV1 {
        SourceOutputEndpointStorageV1(match self {
            Self::Optimizer(checked) => checked.storage().retained_storage(),
            Self::OptimizerPolicy3(checked) => checked.storage().retained_storage(),
            Self::Receipt { retained, .. } => retained,
        })
    }

    fn candidate(self) -> fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'output> {
        match self {
            Self::Optimizer(checked) => checked.occurrences().candidate(),
            Self::OptimizerPolicy3(checked) => checked.occurrences().candidate(),
            Self::Receipt { receipt, .. } => receipt.candidate(),
        }
    }

    fn native_input_audit_bytes(self) -> Option<&'output [u8]> {
        match self {
            Self::Optimizer(checked) => Some(checked.native_input_audit_bytes()),
            Self::OptimizerPolicy3(checked) => Some(checked.native_input_audit_bytes()),
            Self::Receipt { .. } => None,
        }
    }

    fn policy3_execution(self) -> Option<&'output fe2o3_pliron::Policy3ExecutionWitnessV1> {
        match self {
            Self::OptimizerPolicy3(checked) => Some(checked.execution()),
            Self::Optimizer(_) | Self::Receipt { .. } => None,
        }
    }
}

impl<'source, 'output> ProductionSourceOutputOccurrencesV1<'source, 'output> {
    /// Borrows the original sealed policy-3 execution witness, when present.
    /// Historical optimizer and semantic-receipt endpoints return None: their
    /// semantic relation is never reinterpreted as eight-pass execution proof.
    /// The witness remains separate from source/ranked/formal admission.
    pub fn policy3_execution_v1(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<
        Option<&'output fe2o3_pliron::Policy3ExecutionWitnessV1>,
        ProductionSourceOutputErrorV1,
    > {
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        Ok(self.checked_output.policy3_execution())
    }
}

/// Connects source N to the same actual checked policy-3 output O.
///
/// The shared constructor compares all B canonical bytes with the retained
/// native input audit and independently checks the B/O occurrence relation.
/// It borrows the original move-only policy-3 owner, including its separately
/// sealed execution witness; no optimizer, witness decoder, graph copy, or
/// historical-owner conversion is invoked here. The coordinate witness alone
/// does not authenticate a target: obtain it from the exact production binder.
///
/// N graph/origins, B, checked O/history/execution, and coordinate storage must
/// already be reserved on this caller ledger. The returned view storage is a
/// transfer to reserve before further allocation. Source replay retains its
/// existing separate bound. Success and failure restore the incoming storage
/// floor after scratch drops without resetting work or first-failure history.
/// This endpoint is not final control, memory, reference, or lineage admission.
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
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     ProductionPreRankedKirOwnerV1, derive_source_output_occurrences_v1,
/// };
/// use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;
/// fn no_policy3_downgrade(
///     source: &ProductionPreRankedKirOwnerV1,
///     coordinates: &CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
///     checked: &CheckedNeutralKernelIrOwnerPolicy3V1,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
/// ) {
///     derive_source_output_occurrences_v1(source, coordinates, checked, budget).unwrap();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     ProductionPreRankedKirOwnerV1, derive_source_output_occurrences_policy3_v1,
/// };
/// use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;
/// fn cannot_drop_borrowed_execution(
///     source: &ProductionPreRankedKirOwnerV1,
///     coordinates: &CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
///     checked: CheckedNeutralKernelIrOwnerPolicy3V1,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
/// ) {
///     let (view, _) = derive_source_output_occurrences_policy3_v1(
///         source, coordinates, &checked, budget,
///     ).unwrap();
///     let execution = view.policy3_execution_v1(budget).unwrap().unwrap();
///     drop(checked);
///     let _ = execution.canonical_bytes();
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

/// Independently connects admitted source N and O through the exact supplied
/// N/B coordinate witness and inert B/O receipt. The existing receipt checker
/// compares both actual endpoint identities and then applies the same nine-slice
/// local rules used by the actual optimizer path. This does not prove historical
/// pass execution, general refinement, formal discharge or backend admission.
///
/// N, B, O and the complete decoded receipt must already be reserved on this
/// caller ledger. `output_storage` and `receipt_storage` are the transfers from
/// their actual admission/decoding, not replacements for those owners. The view
/// borrows O and the receipt together and grants no authority. Source replay uses
/// its existing separately bounded engine. New inventories, checks, control and
/// catalog rows use this same canonical ledger; no optimizer or graph cloning is
/// performed. Returned storage is a transfer to reserve before later allocation.
///
/// All Result/unwind exits preserve caller work/peak/failure history and restore
/// incoming storage after dropping scratch. No consumer input can select another
/// output owner by digest or reinterpret an old N-only proof owner as O.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     ProductionPreRankedKirOwnerV1, derive_source_output_occurrences_from_receipt_v1,
/// };
/// use fe2o3_kernel_ir::{
///     CanonicalKernelIrReplayStorageV12, CanonicalKirTransitionReceiptStorageV1,
///     CanonicalKernelIrVerificationResourceBudgetV1, InertCanonicalKirTransitionReceiptV1,
///     VerifiedCanonicalKernelIrModuleV12,
/// };
/// use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1;
/// fn drop_borrowed_receipt(
///     source: &ProductionPreRankedKirOwnerV1,
///     coordinates: &CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
///     output: &VerifiedCanonicalKernelIrModuleV12,
///     output_storage: CanonicalKernelIrReplayStorageV12,
///     receipt: InertCanonicalKirTransitionReceiptV1,
///     receipt_storage: CanonicalKirTransitionReceiptStorageV1,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
/// ) {
///     let (view, _) = derive_source_output_occurrences_from_receipt_v1(
///         source, coordinates, output, output_storage, &receipt, receipt_storage, budget,
///     ).unwrap();
///     drop(receipt);
///     let _ = view.output();
/// }
/// ```
#[allow(
    clippy::too_many_arguments,
    reason = "Source, exact N/B witness, admitted O and receipt each retain separate custody and accounting"
)]
pub fn derive_source_output_occurrences_from_receipt_v1<'source, 'output>(
    source: &'source ProductionPreRankedKirOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<
        'source,
        'source,
    >,
    output: &'output fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    output_storage: fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV12,
    receipt: &'output fe2o3_kernel_ir::InertCanonicalKirTransitionReceiptV1,
    receipt_storage: fe2o3_kernel_ir::CanonicalKirTransitionReceiptStorageV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        ProductionSourceOutputOccurrencesV1<'source, 'output>,
        ProductionSourceOutputStorageV1,
    ),
    ProductionSourceOutputErrorV1,
> {
    budget
        .charge_work(1)
        .map_err(ProductionSourceOutputErrorV1::Resource)?;
    let retained = output_storage
        .retained_storage()
        .checked_add(receipt_storage.retained_storage())
        .ok_or(ProductionSourceOutputErrorV1::Resource(
            AssertOriginResourceV1::Arithmetic,
        ))?;
    derive_source_output_occurrences_with_endpoint_v1(
        source,
        coordinates,
        SourceOutputCheckedEndpointV1::Receipt {
            output,
            receipt,
            retained,
        },
        budget,
    )
}
