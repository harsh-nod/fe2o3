//! Checked optimizer occurrence receipts over the existing F2NTR1 frame.
//!
//! This adapter preserves checker policy 1 and every historical V2/V3/V4
//! report meaning. A receipt checks the named V12 B -> O local relation; it
//! does not prove target binding, historical pass execution, general semantic
//! equivalence, ranked/formal facts, or final backend admission.

use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1, CanonicalKirTransitionErrorV1,
    check_canonical_kir_transition_receipt_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKirTransitionReceiptErrorV1,
    CanonicalKirTransitionReceiptStorageV1, InertCanonicalKirTransitionReceiptV1,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::CheckedNeutralKernelIrOwnerV1;
use std::{
    error::Error,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// No failure returns unchecked custody or substitutes another endpoint.
#[derive(Debug)]
pub enum KernelIrCheckedOptimizationReceiptErrorV1 {
    /// The supplied B is not the checked owner's complete historical input.
    InputHistory,
    Codec(CanonicalKirTransitionReceiptErrorV1),
    Inventory(CanonicalKirInventoryErrorV1),
    Transition(CanonicalKirTransitionErrorV1),
    Resource(Resource),
    Panicked,
}
impl fmt::Display for KernelIrCheckedOptimizationReceiptErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputHistory => f.write_str("checked optimizer input history differs"),
            Self::Codec(error) => error.fmt(f),
            Self::Inventory(error) => error.fmt(f),
            Self::Transition(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::Panicked => f.write_str("checked optimizer receipt admission panicked"),
        }
    }
}
impl Error for KernelIrCheckedOptimizationReceiptErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::Transition(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::InputHistory | Self::Panicked => None,
        }
    }
}
impl From<Resource> for KernelIrCheckedOptimizationReceiptErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

/// Inert transfer of the owned receipt's header, typed rows, bytes, and adapter
/// header. Borrowed endpoint owners must remain separately reserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedCanonicalOptimizationReceiptStorageV1(usize);
impl CheckedCanonicalOptimizationReceiptStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only receipt checked against these exact borrowed, admitted V12 owners.
/// It retains no graph clone, inventory, mutable row access, or executable
/// authority. The untrusted frame cannot choose either actual endpoint.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedCanonicalOptimizationReceiptV1;
/// fn duplicate(receipt: CheckedCanonicalOptimizationReceiptV1<'_, '_>) {
///     let _ = receipt.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{
///     CanonicalKernelIrVerificationResourceBudgetV1, VerifiedCanonicalKernelIrModuleV12,
/// };
/// use fe2o3_kernel_opt::{
///     CheckedCanonicalOptimizationReceiptV1, decode_and_check_canonical_optimization_receipt_v1,
/// };
/// fn escape<'a>(
///     input: VerifiedCanonicalKernelIrModuleV12,
///     output: &'a VerifiedCanonicalKernelIrModuleV12,
///     bytes: &[u8],
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
/// ) -> CheckedCanonicalOptimizationReceiptV1<'a, 'a> {
///     decode_and_check_canonical_optimization_receipt_v1(&input, output, bytes, budget).unwrap()
/// }
/// ```
pub struct CheckedCanonicalOptimizationReceiptV1<'input, 'output> {
    input: &'input Owner,
    output: &'output Owner,
    receipt: InertCanonicalKirTransitionReceiptV1,
    storage: CheckedCanonicalOptimizationReceiptStorageV1,
}
impl<'input, 'output> CheckedCanonicalOptimizationReceiptV1<'input, 'output> {
    pub const fn input(&self) -> &'input Owner {
        self.input
    }
    pub const fn output(&self) -> &'output Owner {
        self.output
    }
    /// Immutable wire data remains inert outside this borrow-bound checked view.
    pub const fn receipt(&self) -> &InertCanonicalKirTransitionReceiptV1 {
        &self.receipt
    }
    pub const fn storage(&self) -> CheckedCanonicalOptimizationReceiptStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Encode the actual checked optimizer rows, after a complete canonical-byte
/// comparison between B and the immutable historical input retained at adoption.
/// A fresh admitted B with identical bytes is accepted; this is exact content
/// binding, not historical pointer identity. Digests alone are never sufficient.
///
/// Both input and checked owner must already remain reserved on this ledger.
/// The existing codec preserves its standalone cap, restores the incoming floor
/// after dropping scratch, and transfers its exact inert receipt storage.
/// Reserve that transfer before any later controlled allocation. Encoding does
/// not itself return a checked receipt; decoding must independently check B/O.
pub fn encode_checked_canonical_optimization_receipt_v1(
    input: &Owner,
    checked: &CheckedNeutralKernelIrOwnerV1,
    budget: &mut Budget<'_>,
) -> Result<
    (
        InertCanonicalKirTransitionReceiptV1,
        CanonicalKirTransitionReceiptStorageV1,
    ),
    KernelIrCheckedOptimizationReceiptErrorV1,
> {
    let bytes = input.canonical().canonical_bytes();
    let historical = checked.native_input_audit_bytes();
    // Checked length addition and comparison dispatch, then prepaid byte work
    // for both complete byte strings. This is not a digest-only comparison.
    budget.charge_work(2)?;
    let comparison = bytes
        .len()
        .checked_add(historical.len())
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(comparison)?;
    if bytes != historical {
        return Err(KernelIrCheckedOptimizationReceiptErrorV1::InputHistory);
    }
    InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
        input.canonical().identity(),
        checked.owner().canonical().identity(),
        checked.occurrences().candidate(),
        budget,
    )
    .map_err(KernelIrCheckedOptimizationReceiptErrorV1::Codec)
}

/// Decode untrusted F2NTR1 bytes and independently check their complete local
/// transition relation against the two actual V12 owners supplied by the caller.
/// There is no optimizer invocation, replay oracle, alternate-output callback,
/// graph admission by digest, or legacy canonical-format reinterpretation.
///
/// Input, output, and input wire bytes must already remain reserved. Decode,
/// both inventories, checker scratch, and wrapper transfer share this ledger.
/// Every returned exit drops rejected owners/scratch before restoring the entry
/// storage floor; accepted work, peak, and first-failure history are preserved.
/// On success reserve `storage().retained_storage()` before another controlled
/// allocation and release it only after dropping the checked receipt. Endpoint
/// reservations outlive the returned borrows. Logical storage excludes allocator
/// overhead/RSS and does not duplicate already-reserved endpoint graphs.
///
/// This checks only the existing fixed scalar/CFG rules for B -> O. Even a
/// successfully rechecked receipt does not prove the seven-pass roster occurred,
/// establish N -> B target binding, or authorize final codegen/launch/proof.
pub fn decode_and_check_canonical_optimization_receipt_v1<'input, 'output>(
    input: &'input Owner,
    output: &'output Owner,
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<
    CheckedCanonicalOptimizationReceiptV1<'input, 'output>,
    KernelIrCheckedOptimizationReceiptErrorV1,
> {
    let floor = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(1)?;
        let (receipt, receipt_storage) =
            InertCanonicalKirTransitionReceiptV1::decode_with_budget(bytes, budget)
                .map_err(KernelIrCheckedOptimizationReceiptErrorV1::Codec)?;
        budget.reserve_storage(receipt_storage.retained_storage())?;
        let scratch_floor = budget.storage();
        {
            let (input_inventory, input_storage) =
                CanonicalKirInventoryV1::derive(input, budget)
                    .map_err(KernelIrCheckedOptimizationReceiptErrorV1::Inventory)?;
            budget.reserve_storage(input_storage.retained_storage())?;
            let (output_inventory, output_storage) =
                CanonicalKirInventoryV1::derive(output, budget)
                    .map_err(KernelIrCheckedOptimizationReceiptErrorV1::Inventory)?;
            budget.reserve_storage(output_storage.retained_storage())?;
            let (_checked, checked_storage) = check_canonical_kir_transition_receipt_v1(
                &input_inventory,
                &output_inventory,
                &receipt,
                budget,
            )
            .map_err(KernelIrCheckedOptimizationReceiptErrorV1::Transition)?;
            budget.reserve_storage(checked_storage.retained_storage())?;
        }
        // Both inventories and the checked borrow have dropped before release.
        budget.release_storage(
            budget
                .storage()
                .checked_sub(scratch_floor)
                .ok_or(Resource::Accounting)?,
        )?;
        // Two checked header arithmetic operations and one wrapper construction.
        budget.charge_work(3)?;
        let extra = size_of::<CheckedCanonicalOptimizationReceiptV1<'_, '_>>()
            .checked_sub(size_of::<InertCanonicalKirTransitionReceiptV1>())
            .ok_or(Resource::Arithmetic)?;
        let retained = receipt_storage
            .retained_storage()
            .checked_add(extra)
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(extra)?;
        Ok(CheckedCanonicalOptimizationReceiptV1 {
            input,
            output,
            receipt,
            storage: CheckedCanonicalOptimizationReceiptStorageV1(retained),
        })
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(KernelIrCheckedOptimizationReceiptErrorV1::Panicked)
        }
    };
    let restored = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|retained| budget.release_storage(retained));
    if let Err(error) = restored {
        drop(result);
        return Err(error.into());
    }
    result
}

#[cfg(test)]
#[path = "checked_optimization_receipt_v1_tests.rs"]
mod tests;
