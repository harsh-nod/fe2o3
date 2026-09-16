//! Canonical composition of exact fixed-policy-3 execution evidence and the
//! unchanged F2NTR1 semantic relation. Neither part can stand in for the other.

use crate::{
    CheckedCanonicalOptimizationReceiptV1,
    KernelIrCheckedOptimizationReceiptErrorV1 as SemanticError,
    decode_and_check_canonical_optimization_receipt_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKirTransitionReceiptErrorV1,
    InertCanonicalKirTransitionReceiptV1, MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::{
    CheckedNeutralKernelIrOwnerPolicy3V1 as CheckedOwner, POLICY3_EXECUTION_RECORD_BYTES_V1,
    policy3_execution_receipt_digest_v1,
};
use std::{
    error::Error,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

pub const CANONICAL_POLICY3_EXECUTION_RECEIPT_MAGIC_V1: [u8; 8] = *b"F2OP3R1\0";
/// 32 fixed framing bytes plus one exact 776-byte execution record.
pub const CANONICAL_POLICY3_EXECUTION_RECEIPT_HEADER_V1: usize =
    32 + POLICY3_EXECUTION_RECORD_BYTES_V1;
pub const MAX_CANONICAL_POLICY3_EXECUTION_RECEIPT_BYTES_V1: usize =
    MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1;

#[derive(Debug)]
pub enum CanonicalPolicy3ExecutionReceiptErrorV1 {
    Header,
    Limit,
    InputHistory,
    ExecutionWitness,
    Codec(CanonicalKirTransitionReceiptErrorV1),
    Semantic(SemanticError),
    Resource(Resource),
    Panicked,
}
impl fmt::Display for CanonicalPolicy3ExecutionReceiptErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "policy-3 execution receipt: {self:?}")
    }
}
impl Error for CanonicalPolicy3ExecutionReceiptErrorV1 {}
impl From<Resource> for CanonicalPolicy3ExecutionReceiptErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type E = CanonicalPolicy3ExecutionReceiptErrorV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPolicy3ExecutionReceiptStorageV1(usize);
impl CanonicalPolicy3ExecutionReceiptStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Owned canonical bytes only. Encoding or possessing these bytes does not
/// certify execution; decoding requires the independently sealed actual owner.
pub struct InertCanonicalPolicy3ExecutionReceiptV1 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    storage: CanonicalPolicy3ExecutionReceiptStorageV1,
}
impl InertCanonicalPolicy3ExecutionReceiptV1 {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn storage(&self) -> CanonicalPolicy3ExecutionReceiptStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Move-only, borrow-bound composition. The existing semantic receipt is
/// independently rechecked against actual B/O, while the execution record is
/// compared byte-for-byte against this sealed policy-3 owner's execution.
pub struct CheckedCanonicalPolicy3ExecutionReceiptV1<'input, 'checked> {
    checked: &'checked CheckedOwner,
    semantic: CheckedCanonicalOptimizationReceiptV1<'input, 'checked>,
    wire: InertCanonicalPolicy3ExecutionReceiptV1,
    storage: CanonicalPolicy3ExecutionReceiptStorageV1,
}
impl<'input, 'checked> CheckedCanonicalPolicy3ExecutionReceiptV1<'input, 'checked> {
    pub const fn input(&self) -> &'input Owner {
        self.semantic.input()
    }
    pub const fn output(&self) -> &'checked Owner {
        self.checked.owner()
    }
    pub const fn execution_owner(&self) -> &'checked CheckedOwner {
        self.checked
    }
    pub const fn receipt(&self) -> &InertCanonicalPolicy3ExecutionReceiptV1 {
        &self.wire
    }
    pub const fn semantic_receipt(
        &self,
    ) -> &CheckedCanonicalOptimizationReceiptV1<'input, 'checked> {
        &self.semantic
    }
    pub const fn storage(&self) -> CanonicalPolicy3ExecutionReceiptStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn exact_history(input: &Owner, checked: &CheckedOwner, budget: &mut Budget<'_>) -> Result<(), E> {
    budget.charge_work(2)?;
    let bytes = input.canonical().canonical_bytes();
    let historical = checked.native_input_audit_bytes();
    budget.charge_work(
        bytes
            .len()
            .checked_add(historical.len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    if bytes != historical {
        return Err(E::InputHistory);
    }
    Ok(())
}

fn checked_length(body: usize) -> Result<usize, E> {
    let length = body
        .checked_add(CANONICAL_POLICY3_EXECUTION_RECEIPT_HEADER_V1)
        .ok_or(E::Limit)?;
    if length > MAX_CANONICAL_POLICY3_EXECUTION_RECEIPT_BYTES_V1 {
        return Err(E::Limit);
    }
    Ok(length)
}

fn allocate(length: usize, budget: &mut Budget<'_>) -> Result<Vec<u8>, E> {
    // Allocation plus immediate visible-capacity reconciliation are prepaid.
    budget.charge_work(2)?;
    budget.reserve_storage(
        size_of::<InertCanonicalPolicy3ExecutionReceiptV1>()
            .checked_add(length)
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| Resource::Allocation)?;
    if bytes.capacity() != length {
        let excess = bytes
            .capacity()
            .checked_sub(length)
            .ok_or(Resource::Accounting)?;
        budget.reserve_storage(excess)?;
        return Err(Resource::Accounting.into());
    }
    Ok(bytes)
}

fn own_wire(
    bytes: Vec<u8>,
    budget: &mut Budget<'_>,
) -> Result<InertCanonicalPolicy3ExecutionReceiptV1, E> {
    let digest = policy3_execution_receipt_digest_v1(&bytes, budget)?;
    let retained = size_of::<InertCanonicalPolicy3ExecutionReceiptV1>()
        .checked_add(bytes.capacity())
        .ok_or(Resource::Arithmetic)?;
    Ok(InertCanonicalPolicy3ExecutionReceiptV1 {
        bytes,
        digest,
        storage: CanonicalPolicy3ExecutionReceiptStorageV1(retained),
    })
}

fn scoped<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, E>,
) -> Result<T, E> {
    let floor = budget.storage();
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(E::Panicked)
        }
    };
    let restored = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|release| budget.release_storage(release));
    if let Err(error) = restored {
        drop(result);
        return Err(error.into());
    }
    result
}

/// The input and sealed checked owner remain reserved throughout. Success
/// transfers the complete inert wire receipt; reserve its storage immediately.
/// No pass roster, profile, endpoint, or graph is supplied by decoded bytes.
pub fn encode_checked_canonical_policy3_execution_receipt_v1(
    input: &Owner,
    checked: &CheckedOwner,
    budget: &mut Budget<'_>,
) -> Result<InertCanonicalPolicy3ExecutionReceiptV1, E> {
    scoped(budget, |budget| {
        exact_history(input, checked, budget)?;
        let (semantic, storage) = InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
            input.canonical().identity(),
            checked.owner().canonical().identity(),
            checked.occurrences().candidate(),
            budget,
        )
        .map_err(E::Codec)?;
        budget.reserve_storage(storage.retained_storage())?;
        budget.charge_work(2)?;
        let body = semantic.canonical_bytes();
        let length = checked_length(body.len())?;
        budget.charge_work(length)?;
        let mut bytes = allocate(length, budget)?;
        bytes.extend_from_slice(&CANONICAL_POLICY3_EXECUTION_RECEIPT_MAGIC_V1);
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(
            &(CANONICAL_POLICY3_EXECUTION_RECEIPT_HEADER_V1 as u32).to_le_bytes(),
        );
        bytes.extend_from_slice(&u64::try_from(length).map_err(|_| E::Limit)?.to_le_bytes());
        bytes.extend_from_slice(
            &u64::try_from(body.len())
                .map_err(|_| E::Limit)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(checked.execution().canonical_bytes());
        bytes.extend_from_slice(body);
        own_wire(bytes, budget)
    })
}

fn validate_header<'a>(
    bytes: &'a [u8],
    checked: &CheckedOwner,
    budget: &mut Budget<'_>,
) -> Result<&'a [u8], E> {
    const HEADER: usize = CANONICAL_POLICY3_EXECUTION_RECEIPT_HEADER_V1;
    budget.charge_work(32)?;
    if bytes.len() < HEADER {
        return Err(E::Header);
    }
    if bytes.len() > MAX_CANONICAL_POLICY3_EXECUTION_RECEIPT_BYTES_V1 {
        return Err(E::Limit);
    }
    if bytes[..8] != CANONICAL_POLICY3_EXECUTION_RECEIPT_MAGIC_V1
        || bytes[8..12] != [1, 0, 0, 0]
        || u32::from_le_bytes(bytes[12..16].try_into().map_err(|_| E::Header)?) != HEADER as u32
    {
        return Err(E::Header);
    }
    let total = usize::try_from(u64::from_le_bytes(
        bytes[16..24].try_into().map_err(|_| E::Header)?,
    ))
    .map_err(|_| E::Limit)?;
    let body = usize::try_from(u64::from_le_bytes(
        bytes[24..32].try_into().map_err(|_| E::Header)?,
    ))
    .map_err(|_| E::Limit)?;
    if checked_length(body)? != total || total != bytes.len() {
        return Err(E::Header);
    }
    budget.charge_work(POLICY3_EXECUTION_RECORD_BYTES_V1)?;
    // Full fixed-width equality authenticates all eight closed pass identities,
    // profiles, epochs, B/O identities and the map binding against actual trusted
    // execution. Decoding never derives a policy roster from this untrusted data.
    if bytes[32..HEADER] != checked.execution().canonical_bytes()[..] {
        return Err(E::ExecutionWitness);
    }
    Ok(&bytes[HEADER..])
}

/// Requires both the actual sealed execution owner and independently checked
/// semantic rows. Even unchanged B/O cannot substitute another policy record.
/// All allocations/cleanup use this same ledger; borrowed owners and input wire
/// remain separately reserved. The semantic adapter owns its decoded row bytes,
/// so coexistence with the full framed wire is explicitly charged twice.
pub fn decode_and_check_canonical_policy3_execution_receipt_v1<'input, 'checked>(
    input: &'input Owner,
    checked: &'checked CheckedOwner,
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<CheckedCanonicalPolicy3ExecutionReceiptV1<'input, 'checked>, E> {
    scoped(budget, |budget| {
        let body = validate_header(bytes, checked, budget)?;
        exact_history(input, checked, budget)?;
        let semantic = decode_and_check_canonical_optimization_receipt_v1(
            input,
            checked.owner(),
            body,
            budget,
        )
        .map_err(E::Semantic)?;
        budget.reserve_storage(semantic.storage().retained_storage())?;
        budget.charge_work(bytes.len())?;
        let mut owned = allocate(bytes.len(), budget)?;
        owned.extend_from_slice(bytes);
        let wire = own_wire(owned, budget)?;
        budget.charge_work(3)?;
        let wrapper = size_of::<CheckedCanonicalPolicy3ExecutionReceiptV1<'_, '_>>()
            .checked_sub(size_of::<CheckedCanonicalOptimizationReceiptV1<'_, '_>>())
            .and_then(|n| n.checked_sub(size_of::<InertCanonicalPolicy3ExecutionReceiptV1>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let retained = semantic
            .storage()
            .retained_storage()
            .checked_add(wire.storage().retained_storage())
            .and_then(|n| n.checked_add(wrapper))
            .ok_or(Resource::Arithmetic)?;
        Ok(CheckedCanonicalPolicy3ExecutionReceiptV1 {
            checked,
            semantic,
            wire,
            storage: CanonicalPolicy3ExecutionReceiptStorageV1(retained),
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy3_v1_tests.rs"]
mod tests;
