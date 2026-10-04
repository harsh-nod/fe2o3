//! Versioned Policy4 composition wire. Published semantic replay is deliberately
//! separate from authentication against a sealed actual execution owner.

use crate::checked_optimization_policy4_v1::policy4_record_v1;
use crate::{
    CanonicalPolicy3ExecutionReceiptErrorV1,
    CheckedCanonicalKernelIrOwnerPolicy4V1 as CheckedOwner, POLICY4_EXECUTION_RECORD_BYTES_V1,
    ReplayedPolicy3SemanticRelationV1, decode_and_check_canonical_policy3_execution_receipt_v1,
    decode_and_check_published_policy3_semantic_relation_v1,
    encode_checked_canonical_policy3_execution_receipt_v1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirStoreForwardingErrorV1, CanonicalKirStoreForwardingRowV1 as Row,
    check_canonical_kir_store_forwarding_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKirBlockCoordinateV1,
    CanonicalKirFunctionCoordinateV1, CanonicalKirOperationCoordinateV1 as Coordinate,
    MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

pub const CANONICAL_POLICY4_EXECUTION_RECEIPT_MAGIC_V1: [u8; 8] = *b"F2OP4R1\0";
pub const CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1: usize =
    40 + POLICY4_EXECUTION_RECORD_BYTES_V1;
pub const MAX_CANONICAL_POLICY4_EXECUTION_RECEIPT_BYTES_V1: usize =
    MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1;
const ROW_BYTES: usize = 24;

#[derive(Debug)]
pub enum CanonicalPolicy4ExecutionReceiptErrorV1 {
    Header,
    Limit,
    ExecutionClaim,
    ExecutionWitness,
    Policy3(CanonicalPolicy3ExecutionReceiptErrorV1),
    Forwarding(CanonicalKirStoreForwardingErrorV1),
    Resource(Resource),
    Panicked,
}
impl From<Resource> for CanonicalPolicy4ExecutionReceiptErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for CanonicalPolicy4ExecutionReceiptErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Policy4 receipt: {self:?}")
    }
}
impl std::error::Error for CanonicalPolicy4ExecutionReceiptErrorV1 {}
type Error = CanonicalPolicy4ExecutionReceiptErrorV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPolicy4ExecutionReceiptStorageV1(usize);
impl CanonicalPolicy4ExecutionReceiptStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Inert framed bytes. Possession does not authenticate any optimizer execution.
pub struct InertCanonicalPolicy4ExecutionReceiptV1 {
    bytes: Vec<u8>,
    storage: CanonicalPolicy4ExecutionReceiptStorageV1,
}
impl InertCanonicalPolicy4ExecutionReceiptV1 {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub const fn storage(&self) -> CanonicalPolicy4ExecutionReceiptStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Both semantic stages rechecked against actual B/C/O. Execution claims remain
/// unauthenticated, including on a no-op graph. No conversion to a sealed owner
/// or execution witness exists. Borrowed owners and wire stay caller-reserved.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_kernel_opt::{ReplayedPolicy4SemanticRelationV1 as Receipt,
///     decode_and_check_published_policy4_semantic_relation_v1 as decode};
/// fn escape<'a>(b: &'a Owner, c: &'a Owner, o: Owner, wire: &'a [u8], budget: &mut Budget<'_>)
///     -> Receipt<'a, 'a, 'static, 'a> { decode(b, c, &o, wire, budget).unwrap() }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_opt::ReplayedPolicy4SemanticRelationV1;
/// fn duplicate(receipt: ReplayedPolicy4SemanticRelationV1<'_, '_, '_, '_>) { let _ = receipt.clone(); }
/// ```
pub struct ReplayedPolicy4SemanticRelationV1<'input, 'intermediate, 'output, 'wire> {
    policy3: ReplayedPolicy3SemanticRelationV1<'input, 'intermediate, 'wire>,
    output: &'output Owner,
    wire: &'wire [u8],
    rows: Vec<Row>,
    storage: CanonicalPolicy4ExecutionReceiptStorageV1,
}
impl<'input, 'intermediate, 'output, 'wire>
    ReplayedPolicy4SemanticRelationV1<'input, 'intermediate, 'output, 'wire>
{
    /// Borrow the independently checked B/C relation, not execution authority.
    pub const fn policy3_relation(
        &self,
    ) -> &ReplayedPolicy3SemanticRelationV1<'input, 'intermediate, 'wire> {
        &self.policy3
    }

    pub const fn input(&self) -> &'input Owner {
        self.policy3.semantic_receipt().input()
    }
    pub const fn intermediate(&self) -> &'intermediate Owner {
        self.policy3.semantic_receipt().output()
    }
    pub const fn output(&self) -> &'output Owner {
        self.output
    }
    pub fn forwarding_rows(&self) -> &[Row] {
        &self.rows
    }
    pub fn unauthenticated_execution_record(&self) -> &[u8] {
        &self.wire[40..CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1]
    }
    pub const fn storage(&self) -> CanonicalPolicy4ExecutionReceiptStorageV1 {
        self.storage
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Borrow-bound authenticated composition. This can only be obtained by matching
/// the actual sealed execution owner, not by re-admitting its output bytes.
/// Authentication remains distinct from source/formal/target authority.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_kernel_opt::decode_and_check_canonical_policy4_execution_receipt_v1 as decode;
/// fn output_is_not_execution(b: &Owner, o: &Owner, bytes: &[u8], budget: &mut Budget<'_>) {
///     let _ = decode(b, o, bytes, budget);
/// }
/// ```
pub struct CheckedCanonicalPolicy4ExecutionReceiptV1<'input, 'checked, 'wire> {
    checked: &'checked CheckedOwner,
    semantic: ReplayedPolicy4SemanticRelationV1<'input, 'checked, 'checked, 'wire>,
    storage: CanonicalPolicy4ExecutionReceiptStorageV1,
}
impl<'input, 'checked, 'wire> CheckedCanonicalPolicy4ExecutionReceiptV1<'input, 'checked, 'wire> {
    pub const fn execution_owner(&self) -> &'checked CheckedOwner {
        self.checked
    }
    pub const fn semantic_relation(
        &self,
    ) -> &ReplayedPolicy4SemanticRelationV1<'input, 'checked, 'checked, 'wire> {
        &self.semantic
    }
    pub const fn storage(&self) -> CanonicalPolicy4ExecutionReceiptStorageV1 {
        self.storage
    }
    pub const fn authenticates_execution(&self) -> bool {
        true
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn scoped<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    let floor = budget.storage();
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Panicked)
        }
    };
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(release)?;
    result
}

fn length(policy3: usize, rows: usize) -> Result<usize, Error> {
    let length = rows
        .checked_mul(ROW_BYTES)
        .and_then(|n| n.checked_add(policy3))
        .and_then(|n| n.checked_add(CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1))
        .ok_or(Error::Limit)?;
    if length > MAX_CANONICAL_POLICY4_EXECUTION_RECEIPT_BYTES_V1 {
        return Err(Error::Limit);
    }
    Ok(length)
}

fn push_coordinate(bytes: &mut Vec<u8>, coordinate: Coordinate) {
    bytes.extend_from_slice(&coordinate.block.function.0.to_le_bytes());
    bytes.extend_from_slice(&coordinate.block.block.to_le_bytes());
    bytes.extend_from_slice(&coordinate.operation.to_le_bytes());
}

/// Encode the fixed two-stage execution and exact semantic claims. The caller
/// retains B and the sealed owner; success transfers the actual-capacity wire.
pub fn encode_checked_canonical_policy4_execution_receipt_v1(
    input: &Owner,
    checked: &CheckedOwner,
    budget: &mut Budget<'_>,
) -> Result<InertCanonicalPolicy4ExecutionReceiptV1, Error> {
    scoped(budget, |budget| {
        let policy3 = encode_checked_canonical_policy3_execution_receipt_v1(
            input,
            checked.intermediate_policy3(),
            budget,
        )
        .map_err(Error::Policy3)?;
        budget.reserve_storage(policy3.storage().retained_storage())?;
        budget.charge_work(3)?;
        let total = length(
            policy3.canonical_bytes().len(),
            checked.forwarding_rows().len(),
        )?;
        budget.charge_work(total)?;
        budget.charge_work(2)?;
        let header = size_of::<InertCanonicalPolicy4ExecutionReceiptV1>();
        budget.reserve_storage(header.checked_add(total).ok_or(Resource::Arithmetic)?)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| Resource::Allocation)?;
        budget.reserve_storage(
            bytes
                .capacity()
                .checked_sub(total)
                .ok_or(Resource::Accounting)?,
        )?;
        bytes.extend_from_slice(&CANONICAL_POLICY4_EXECUTION_RECEIPT_MAGIC_V1);
        bytes.extend_from_slice(&[1, 0, 4, 0]);
        bytes.extend_from_slice(
            &(CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1 as u32).to_le_bytes(),
        );
        for value in [
            total,
            policy3.canonical_bytes().len(),
            checked.forwarding_rows().len(),
        ] {
            bytes.extend_from_slice(
                &u64::try_from(value)
                    .map_err(|_| Error::Limit)?
                    .to_le_bytes(),
            );
        }
        bytes.extend_from_slice(checked.execution().canonical_bytes());
        bytes.extend_from_slice(policy3.canonical_bytes());
        for row in checked.forwarding_rows() {
            push_coordinate(&mut bytes, row.store);
            push_coordinate(&mut bytes, row.load);
        }
        if bytes.len() != total {
            return Err(Resource::Accounting.into());
        }
        let retained = header
            .checked_add(bytes.capacity())
            .ok_or(Resource::Arithmetic)?;
        Ok(InertCanonicalPolicy4ExecutionReceiptV1 {
            bytes,
            storage: CanonicalPolicy4ExecutionReceiptStorageV1(retained),
        })
    })
}

struct Frame<'a> {
    policy3: &'a [u8],
    rows: &'a [u8],
    count: usize,
}
fn frame<'a>(
    input: &Owner,
    intermediate: &Owner,
    output: &Owner,
    bytes: &'a [u8],
    budget: &mut Budget<'_>,
) -> Result<Frame<'a>, Error> {
    const HEADER: usize = CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1;
    // Full header scan and generation of the expected fixed-width identity record.
    budget.charge_work(HEADER + POLICY4_EXECUTION_RECORD_BYTES_V1)?;
    if bytes.len() < HEADER {
        return Err(Error::Header);
    }
    if bytes.len() > MAX_CANONICAL_POLICY4_EXECUTION_RECEIPT_BYTES_V1 {
        return Err(Error::Limit);
    }
    if bytes[..8] != CANONICAL_POLICY4_EXECUTION_RECEIPT_MAGIC_V1
        || bytes[8..12] != [1, 0, 4, 0]
        || u32::from_le_bytes(bytes[12..16].try_into().map_err(|_| Error::Header)?) != HEADER as u32
    {
        return Err(Error::Header);
    }
    let word = |offset| -> Result<usize, Error> {
        usize::try_from(u64::from_le_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .map_err(|_| Error::Header)?,
        ))
        .map_err(|_| Error::Limit)
    };
    let total = word(16)?;
    let policy3 = word(24)?;
    let count = word(32)?;
    if total != bytes.len() || length(policy3, count)? != total {
        return Err(Error::Header);
    }
    if bytes[40..HEADER] != policy4_record_v1(input, intermediate, output, count)? {
        return Err(Error::ExecutionClaim);
    }
    let split = HEADER.checked_add(policy3).ok_or(Error::Limit)?;
    Ok(Frame {
        policy3: &bytes[HEADER..split],
        rows: &bytes[split..],
        count,
    })
}

fn coordinate(bytes: &[u8]) -> Coordinate {
    let word = |at| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Coordinate {
        block: CanonicalKirBlockCoordinateV1 {
            function: CanonicalKirFunctionCoordinateV1(word(0)),
            block: word(4),
        },
        operation: word(8),
    }
}

/// Independently replay both exact endpoint relations without claiming execution
/// provenance. Valid record syntax/profile/identities do not create a witness.
/// Returned owned rows and semantic storage transfer; B/C/O and wire are borrowed.
pub fn decode_and_check_published_policy4_semantic_relation_v1<'b, 'c, 'o, 'w>(
    input: &'b Owner,
    intermediate: &'c Owner,
    output: &'o Owner,
    bytes: &'w [u8],
    budget: &mut Budget<'_>,
) -> Result<ReplayedPolicy4SemanticRelationV1<'b, 'c, 'o, 'w>, Error> {
    scoped(budget, |budget| {
        let frame = frame(input, intermediate, output, bytes, budget)?;
        let policy3 = decode_and_check_published_policy3_semantic_relation_v1(
            input,
            intermediate,
            frame.policy3,
            budget,
        )
        .map_err(Error::Policy3)?;
        budget.reserve_storage(policy3.storage().retained_storage())?;
        budget.charge_work(3)?;
        let wrapper = size_of::<ReplayedPolicy4SemanticRelationV1<'_, '_, '_, '_>>()
            .checked_sub(size_of::<ReplayedPolicy3SemanticRelationV1<'_, '_, '_>>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        budget.charge_work(frame.rows.len())?;
        budget.charge_work(2)?;
        let requested = frame
            .count
            .checked_mul(size_of::<Row>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(requested)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(frame.count)
            .map_err(|_| Resource::Allocation)?;
        let capacity = rows
            .capacity()
            .checked_mul(size_of::<Row>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(
            capacity
                .checked_sub(requested)
                .ok_or(Resource::Accounting)?,
        )?;
        for bytes in frame.rows.chunks_exact(ROW_BYTES) {
            rows.push(Row {
                store: coordinate(&bytes[..12]),
                load: coordinate(&bytes[12..]),
            });
        }
        let storage = {
            let (_checked, storage) =
                check_canonical_kir_store_forwarding_v1(intermediate, output, &rows, budget)
                    .map_err(Error::Forwarding)?;
            budget.reserve_storage(storage.retained_storage())?;
            storage
        };
        budget.release_storage(storage.retained_storage())?;
        let retained = policy3
            .storage()
            .retained_storage()
            .checked_add(wrapper)
            .and_then(|n| n.checked_add(capacity))
            .ok_or(Resource::Arithmetic)?;
        Ok(ReplayedPolicy4SemanticRelationV1 {
            policy3,
            output,
            wire: bytes,
            rows,
            storage: CanonicalPolicy4ExecutionReceiptStorageV1(retained),
        })
    })
}

/// Semantic replay plus exact execution authentication against the actual
/// privately constructed Policy4 owner. No raw-output or receipt-only overload.
pub fn decode_and_check_canonical_policy4_execution_receipt_v1<'b, 'c, 'w>(
    input: &'b Owner,
    checked: &'c CheckedOwner,
    bytes: &'w [u8],
    budget: &mut Budget<'_>,
) -> Result<CheckedCanonicalPolicy4ExecutionReceiptV1<'b, 'c, 'w>, Error> {
    scoped(budget, |budget| {
        let semantic = decode_and_check_published_policy4_semantic_relation_v1(
            input,
            checked.intermediate_policy3().owner(),
            checked.owner(),
            bytes,
            budget,
        )?;
        budget.reserve_storage(semantic.storage().retained_storage())?;
        budget.charge_work(
            POLICY4_EXECUTION_RECORD_BYTES_V1
                .checked_add(
                    semantic
                        .rows
                        .len()
                        .checked_mul(6)
                        .ok_or(Resource::Arithmetic)?,
                )
                .ok_or(Resource::Arithmetic)?,
        )?;
        if semantic.unauthenticated_execution_record() != checked.execution().canonical_bytes()
            || semantic.rows != checked.forwarding_rows()
        {
            return Err(Error::ExecutionWitness);
        }
        let policy3_length = usize::try_from(u64::from_le_bytes(
            bytes[24..32].try_into().map_err(|_| Error::Header)?,
        ))
        .map_err(|_| Error::Limit)?;
        let start = CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1;
        let policy3 = decode_and_check_canonical_policy3_execution_receipt_v1(
            input,
            checked.intermediate_policy3(),
            &bytes[start..start + policy3_length],
            budget,
        )
        .map_err(Error::Policy3)?;
        budget.reserve_storage(policy3.storage().retained_storage())?;
        let temporary = policy3.storage().retained_storage();
        drop(policy3);
        budget.release_storage(temporary)?;
        budget.charge_work(2)?;
        let wrapper = size_of::<CheckedCanonicalPolicy4ExecutionReceiptV1<'_, '_, '_>>()
            .checked_sub(size_of::<ReplayedPolicy4SemanticRelationV1<'_, '_, '_, '_>>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let retained = semantic
            .storage()
            .retained_storage()
            .checked_add(wrapper)
            .ok_or(Resource::Arithmetic)?;
        Ok(CheckedCanonicalPolicy4ExecutionReceiptV1 {
            checked,
            semantic,
            storage: CanonicalPolicy4ExecutionReceiptStorageV1(retained),
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy4_receipt_v1_tests.rs"]
mod tests;
