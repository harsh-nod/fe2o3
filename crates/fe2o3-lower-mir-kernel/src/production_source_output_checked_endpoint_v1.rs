// Shared endpoint custody for actual optimizer output and independently checked
// receipt input. The receipt path never creates an optimizer owner.

#[derive(Clone, Copy)]
enum SourceOutputCheckedEndpointV1<'output> {
    Optimizer(&'output fe2o3_pliron::CheckedNeutralKernelIrOwnerV1),
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
            Self::Receipt { output, .. } => output,
        }
    }

    fn storage(self) -> SourceOutputEndpointStorageV1 {
        SourceOutputEndpointStorageV1(match self {
            Self::Optimizer(checked) => checked.storage().retained_storage(),
            Self::Receipt { retained, .. } => retained,
        })
    }

    fn candidate(self) -> fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'output> {
        match self {
            Self::Optimizer(checked) => checked.occurrences().candidate(),
            Self::Receipt { receipt, .. } => receipt.candidate(),
        }
    }
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
