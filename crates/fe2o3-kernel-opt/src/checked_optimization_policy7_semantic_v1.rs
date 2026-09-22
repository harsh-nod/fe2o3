//! Borrowed Policy7 claims and exact I/J semantics, not execution authority.
use crate::{
    CanonicalPolicy6SemanticErrorV1, CanonicalPolicy6SemanticInputsV1,
    ReplayedPolicy6SemanticRelationV1, check_published_policy6_semantic_relation_v1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirRedundantStoreErrorV1, CanonicalKirRedundantStoreRetainedOperationV1 as Retained,
    CanonicalKirRedundantStoreRowV1 as Row, CheckedCanonicalKirRedundantStoreV1 as Relation,
    check_canonical_kir_redundant_store_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirOperationCoordinateV1 as Coordinate, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Existing F2P7EX1 header size; not a new portable framing allocation.
pub const POLICY7_EXECUTION_HEADER_BYTES_V1: usize = 384;
/// Two exact function/block/operation coordinates per existing transcript row.
pub const POLICY7_EXECUTION_ROW_BYTES_V1: usize = 24;

#[path = "checked_optimization_policy7_rows_v1.rs"]
mod rows;
pub use rows::{
    DecodedCanonicalPolicy7RowsV1, MAX_POLICY7_EXECUTION_RECORD_BYTES_V1,
    decode_canonical_policy7_rows_v1,
};

#[derive(Debug)]
pub enum CanonicalPolicy7SemanticErrorV1 {
    Record,
    Rows,
    /// Boxed diagnostic only, outside the returned semantic receipt domain.
    Policy6(Box<CanonicalPolicy6SemanticErrorV1>),
    Continuation(CanonicalKirRedundantStoreErrorV1),
    Resource(Resource),
    Panicked,
}
type Error = CanonicalPolicy7SemanticErrorV1;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Policy7 semantic composition: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Policy6(error) => Some(error.as_ref()),
            Self::Continuation(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Record | Self::Rows | Self::Panicked => None,
        }
    }
}

/// Existing record and typed rows stay caller-owned. This is not a row decoder.
#[derive(Clone, Copy)]
pub struct CanonicalPolicy7ContinuationClaimsV1<'a> {
    pub execution_record: &'a [u8],
    pub deletion_rows: &'a [Row],
    pub retained_operations: &'a [Retained],
}
/// Semantic subjects only, never an optimizer pass or production-mode selector.
#[derive(Clone, Copy)]
pub struct CanonicalPolicy7SemanticInputsV1<'a> {
    pub prefix: CanonicalPolicy6SemanticInputsV1<'a>,
    pub output: &'a Owner,
    pub continuation: CanonicalPolicy7ContinuationClaimsV1<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPolicy7SemanticStorageV1(usize);
impl CanonicalPolicy7SemanticStorageV1 {
    /// New logical retained bytes, returned unreserved. Borrowed backing is excluded.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Independently checked I/J relation joined to the exact supplied transcript.
/// The nested P6 bytes are joined, but their semantic claims are NOT checked by
/// this continuation-only receipt. No executed-policy/source/proof authority.
pub struct CheckedCanonicalPolicy7ContinuationRelationV1<'a> {
    relation: Relation<'a>,
    record: &'a [u8],
    storage: CanonicalPolicy7SemanticStorageV1,
}
impl<'a> CheckedCanonicalPolicy7ContinuationRelationV1<'a> {
    pub const fn relation(&self) -> &Relation<'a> {
        &self.relation
    }
    pub const fn unauthenticated_execution_record(&self) -> &'a [u8] {
        self.record
    }
    pub const fn storage(&self) -> CanonicalPolicy7SemanticStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// One retained P6 semantic receipt and one I/J relation, with all subjects and
/// row/record backing borrowed. Equal-byte separately admitted owners are valid
/// semantic subjects; they are not authenticated as actual producer custody.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::ReplayedPolicy7SemanticRelationV1;
/// fn clone(value: ReplayedPolicy7SemanticRelationV1<'_>) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::ReplayedPolicy7SemanticRelationV1;
/// fn escape<'a>(value: ReplayedPolicy7SemanticRelationV1<'a>)
///     -> ReplayedPolicy7SemanticRelationV1<'static> { value }
/// ```
pub struct ReplayedPolicy7SemanticRelationV1<'a> {
    prefix: ReplayedPolicy6SemanticRelationV1<'a>,
    continuation: CheckedCanonicalPolicy7ContinuationRelationV1<'a>,
    storage: CanonicalPolicy7SemanticStorageV1,
}
impl<'a> ReplayedPolicy7SemanticRelationV1<'a> {
    pub const fn policy6_relation(&self) -> &ReplayedPolicy6SemanticRelationV1<'a> {
        &self.prefix
    }
    pub const fn continuation(&self) -> &CheckedCanonicalPolicy7ContinuationRelationV1<'a> {
        &self.continuation
    }
    pub const fn storage(&self) -> CanonicalPolicy7SemanticStorageV1 {
        self.storage
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn scoped<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> Result<T, Error>,
) -> Result<T, Error> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut panic_payload = None;
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            panic_payload = Some(payload);
            Err(Error::Panicked)
        }
    };
    let cleanup = if ledger != budget.work_ledger_identity_v1() {
        Err(Resource::Accounting)
    } else {
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|bytes| budget.release_storage(bytes))
    };
    match cleanup {
        Ok(()) => {
            drop(panic_payload);
            result
        }
        Err(error) => {
            let drop_payload = catch_unwind(AssertUnwindSafe(|| drop(result))).err();
            drop(panic_payload);
            drop(drop_payload);
            Err(error.into())
        }
    }
}

fn coordinate(encoded: &[u8], value: Coordinate) -> bool {
    encoded[..4] == value.block.function.0.to_le_bytes()
        && encoded[4..8] == value.block.block.to_le_bytes()
        && encoded[8..12] == value.operation.to_le_bytes()
}

struct RecordLayout {
    deletions: usize,
    retained: usize,
    count: usize,
    extent: usize,
}
impl RecordLayout {
    fn new(deletions: usize, retained: usize) -> Result<Self, Error> {
        let count = deletions
            .checked_add(retained)
            .ok_or(Resource::Arithmetic)?;
        let extent = count
            .checked_mul(POLICY7_EXECUTION_ROW_BYTES_V1)
            .and_then(|n| n.checked_add(POLICY7_EXECUTION_HEADER_BYTES_V1))
            .ok_or(Resource::Arithmetic)?;
        Ok(Self {
            deletions,
            retained,
            count,
            extent,
        })
    }
}

// Both callers prepay the complete fixed header before this bounded check.
// P6 and graph identities are payload claims here, not independently verified.
fn check_record_header(bytes: &[u8], layout: &RecordLayout) -> Result<(), Error> {
    if bytes.len() != layout.extent
        || &bytes[..8] != b"F2P7EX1\0"
        || bytes[8..10] != 1u16.to_le_bytes()
        || bytes[10..12] != 7u16.to_le_bytes()
        || bytes[12..16] != 256u32.to_le_bytes()
        || bytes[368..376] != 1u64.to_le_bytes()
    {
        return Err(Error::Record);
    }
    for (offset, count) in [
        (352, layout.deletions),
        (360, layout.retained),
        (376, layout.extent),
    ] {
        if bytes[offset..offset + 8]
            != u64::try_from(count)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes()
        {
            return Err(Error::Record);
        }
    }
    Ok(())
}

fn deletion_order(previous: Option<Coordinate>, row: &Row) -> bool {
    row.anchor < row.removed && previous.is_none_or(|last| last < row.removed)
}
fn retained_order(previous: Option<(Coordinate, Coordinate)>, row: &Retained) -> bool {
    previous.is_none_or(|(a, b)| a < row.input && b < row.output)
}

fn record(
    prefix: &[u8; 256],
    input: &Owner,
    output: &Owner,
    claims: CanonicalPolicy7ContinuationClaimsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(3)?;
    let layout = RecordLayout::new(claims.deletion_rows.len(), claims.retained_operations.len())?;
    let extent = layout.extent;
    let bytes = claims.execution_record;
    if bytes.len() != extent {
        return Err(Error::Record);
    }
    budget.charge_work(
        extent
            .checked_add(layout.count)
            .ok_or(Resource::Arithmetic)?,
    )?;
    check_record_header(bytes, &layout)?;
    if &bytes[16..272] != prefix {
        return Err(Error::Record);
    }
    for (offset, owner) in [(272, input), (312, output)] {
        let id = owner.canonical().identity();
        if &bytes[offset..offset + 32] != id.digest()
            || bytes[offset + 32..offset + 40] != id.canonical_length().to_le_bytes()
        {
            return Err(Error::Record);
        }
    }
    let mut encoded =
        bytes[POLICY7_EXECUTION_HEADER_BYTES_V1..].chunks_exact(POLICY7_EXECUTION_ROW_BYTES_V1);
    let mut previous = None;
    for row in claims.deletion_rows {
        let bytes = encoded.next().ok_or(Error::Record)?;
        if !deletion_order(previous, row)
            || !coordinate(&bytes[..12], row.anchor)
            || !coordinate(&bytes[12..], row.removed)
        {
            return Err(Error::Rows);
        }
        previous = Some(row.removed);
    }
    let mut previous = None;
    for row in claims.retained_operations {
        let bytes = encoded.next().ok_or(Error::Record)?;
        if !retained_order(previous, row)
            || !coordinate(&bytes[..12], row.input)
            || !coordinate(&bytes[12..], row.output)
        {
            return Err(Error::Rows);
        }
        previous = Some((row.input, row.output));
    }
    if encoded.next().is_some() || !encoded.remainder().is_empty() {
        return Err(Error::Record);
    }
    Ok(())
}

/// Checks exact borrowed record/typed-row joins and the existing independent I/J
/// algorithm. The nested prefix record is not semantically checked here. Caller
/// graph/row/wire backing remains reserved once or explicitly externally owned.
pub fn check_canonical_policy7_continuation_relation_v1<'a>(
    prefix_record: &'a [u8; 256],
    input: &'a Owner,
    output: &'a Owner,
    claims: CanonicalPolicy7ContinuationClaimsV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<CheckedCanonicalPolicy7ContinuationRelationV1<'a>, Error> {
    scoped(budget, |budget| {
        let wrapper = size_of::<CheckedCanonicalPolicy7ContinuationRelationV1<'_>>()
            .checked_sub(size_of::<Relation<'_>>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        record(prefix_record, input, output, claims, budget)?;
        let (relation, storage) = check_canonical_kir_redundant_store_v1(
            input,
            output,
            claims.deletion_rows,
            claims.retained_operations,
            budget,
        )
        .map_err(Error::Continuation)?;
        budget.reserve_storage(storage.retained_storage())?;
        let retained = wrapper
            .checked_add(storage.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        Ok(CheckedCanonicalPolicy7ContinuationRelationV1 {
            relation,
            record: claims.execution_record,
            storage: CanonicalPolicy7SemanticStorageV1(retained),
        })
    })
}

/// Independently replays P6 once, then exact fixed redundant-store semantics.
/// No optimizer is run and no sealed execution witness is decoded or constructed.
pub fn check_published_policy7_semantic_relation_v1<'a>(
    inputs: CanonicalPolicy7SemanticInputsV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<ReplayedPolicy7SemanticRelationV1<'a>, Error> {
    scoped(budget, |budget| {
        let wrapper = size_of::<ReplayedPolicy7SemanticRelationV1<'_>>()
            .checked_sub(size_of::<ReplayedPolicy6SemanticRelationV1<'_>>())
            .and_then(|n| {
                n.checked_sub(size_of::<CheckedCanonicalPolicy7ContinuationRelationV1<'_>>())
            })
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let prefix = check_published_policy6_semantic_relation_v1(inputs.prefix, budget)
            .map_err(|e| Error::Policy6(Box::new(e)))?;
        let prefix_storage = prefix.storage().retained_storage();
        budget.reserve_storage(prefix_storage)?;
        let continuation = check_canonical_policy7_continuation_relation_v1(
            prefix.unauthenticated_composition_record(),
            inputs.prefix.output,
            inputs.output,
            inputs.continuation,
            budget,
        )?;
        let continuation_storage = continuation.storage().retained_storage();
        budget.reserve_storage(continuation_storage)?;
        let retained = wrapper
            .checked_add(prefix_storage)
            .and_then(|n| n.checked_add(continuation_storage))
            .ok_or(Resource::Arithmetic)?;
        Ok(ReplayedPolicy7SemanticRelationV1 {
            prefix,
            continuation,
            storage: CanonicalPolicy7SemanticStorageV1(retained),
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy7_semantic_resource_v1_tests.rs"]
mod resource_tests;
