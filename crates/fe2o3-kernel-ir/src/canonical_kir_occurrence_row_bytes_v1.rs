//! Frame-neutral occurrence grammar, never a checker policy or graph receipt.
use super::*;

#[path = "canonical_kir_occurrence_rows_owned_v1.rs"]
mod owned_rows;
pub use owned_rows::*;

/// Logical new header and owned byte capacity, or the fixed borrowed view header.
/// Returned unreserved; borrowed input storage is not counted again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirOccurrenceRowBytesStorageV1(usize);
impl CanonicalKirOccurrenceRowBytesStorageV1 {
    /// Reserve before further controlled work; release only after the value drops.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Owned inert row-body bytes and counts, without magic, policy or endpoint IDs.
/// Construction checks only the shared row grammar and complete range partitions.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::InertCanonicalKirOccurrenceRowBytesV1;
/// fn duplicate(value: InertCanonicalKirOccurrenceRowBytesV1) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::InertCanonicalKirOccurrenceRowBytesV1;
/// fn escape(value: InertCanonicalKirOccurrenceRowBytesV1) -> &'static [u8] {
///     value.canonical_row_bytes()
/// }
/// ```
pub struct InertCanonicalKirOccurrenceRowBytesV1 {
    bytes: Vec<u8>,
    counts: [u32; 9],
    storage: CanonicalKirOccurrenceRowBytesStorageV1,
}
impl InertCanonicalKirOccurrenceRowBytesV1 {
    /// The existing nine-axis counts in their canonical order.
    pub const fn counts(&self) -> [u32; 9] {
        self.counts
    }
    /// Only row bodies: no enclosing F2NTR header or historical policy claim.
    pub fn canonical_row_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// New logical owned payload; input rows remain caller-owned.
    pub const fn storage(&self) -> CanonicalKirOccurrenceRowBytesStorageV1 {
        self.storage
    }
    /// Row serialization never grants graph, execution or publication authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Allocation-free syntax/partition view. Coordinates remain untrusted claims.
/// No typed slices are manufactured from encoded or unaligned byte storage.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{CanonicalKirOccurrenceRowsRefV1,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
///     read_canonical_kir_occurrence_row_bytes_v1 as read};
/// fn escape(bytes: Vec<u8>, budget: &mut Budget<'_>) -> CanonicalKirOccurrenceRowsRefV1<'static> {
///     read(&bytes, [0; 9], budget).unwrap()
/// }
/// ```
pub struct CanonicalKirOccurrenceRowsRefV1<'bytes> {
    bytes: &'bytes [u8],
    counts: [u32; 9],
    storage: CanonicalKirOccurrenceRowBytesStorageV1,
}
impl<'bytes> CanonicalKirOccurrenceRowsRefV1<'bytes> {
    /// The unchanged borrowed body, not a semantically admitted transition.
    pub const fn canonical_row_bytes(&self) -> &'bytes [u8] {
        self.bytes
    }
    /// Exact declared counts whose extent and partitions were checked.
    pub const fn counts(&self) -> [u32; 9] {
        self.counts
    }
    /// Fixed logical view header, unreserved; no owned row allocation exists.
    pub const fn storage(&self) -> CanonicalKirOccurrenceRowBytesStorageV1 {
        self.storage
    }
    /// Syntax validation establishes no graph or execution authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T>,
) -> Result<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(value) => value,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(CanonicalKirTransitionReceiptErrorV1::Panicked)
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
    if let Err(error) = cleanup {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    drop(payloads);
    result
}

fn layout(counts: [u32; 9], budget: &mut Budget<'_>) -> Result<([usize; 10], usize)> {
    budget.charge_work(9)?;
    let mut offsets = [0usize; 10];
    let mut total_rows = 0usize;
    for (axis, (count, width)) in counts.into_iter().zip(WIDTHS).enumerate() {
        let count =
            usize::try_from(count).map_err(|_| CanonicalKirTransitionReceiptErrorV1::Limit)?;
        offsets[axis + 1] = count
            .checked_mul(width)
            .and_then(|bytes| offsets[axis].checked_add(bytes))
            .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?;
        total_rows = total_rows
            .checked_add(count)
            .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?;
    }
    if offsets[9] > MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1 {
        return Err(CanonicalKirTransitionReceiptErrorV1::Limit);
    }
    Ok((offsets, total_rows))
}

fn reconcile_capacity(requested: usize, observed: usize, budget: &mut Budget<'_>) -> Result<()> {
    budget.reserve_storage(
        observed
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(())
}

/// Encodes the existing nine typed row arrays without assigning a checker policy.
/// The caller retains input rows/owners. New output/header and observed capacity
/// are prepaid before grow-free fill. No F2NTR frame is built or stripped.
/// Success transfers exact new logical storage unreserved; every valid-ledger
/// exit restores the inherited floor with cumulative work/peak/denial intact.
pub fn encode_canonical_kir_occurrence_row_bytes_v1(
    candidate: Candidate<'_>,
    budget: &mut Budget<'_>,
) -> Result<(
    InertCanonicalKirOccurrenceRowBytesV1,
    CanonicalKirOccurrenceRowBytesStorageV1,
)> {
    scoped(budget, |budget| {
        budget.charge_work(10)?;
        let mut encoded_counts = [0; 9];
        for (out, count) in encoded_counts.iter_mut().zip(counts(candidate)) {
            *out = u32::try_from(count).map_err(|_| CanonicalKirTransitionReceiptErrorV1::Limit)?;
        }
        let (offsets, count) = layout(encoded_counts, budget)?;
        validate_ranges(candidate, budget)?;
        let length = offsets[9];
        budget.charge_work(
            length
                .checked_add(count)
                .ok_or(CanonicalKirTransitionReceiptErrorV1::Limit)?,
        )?;
        let header = size_of::<InertCanonicalKirOccurrenceRowBytesV1>();
        budget.reserve_storage(header.checked_add(length).ok_or(Resource::Arithmetic)?)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Resource::Allocation)?;
        let capacity = bytes.capacity();
        reconcile_capacity(length, capacity, budget)?;
        let mut writer = Writer { bytes, length };
        rows::encode(&mut writer, candidate)?;
        if writer.bytes.len() != length {
            return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                "row body extent",
            ));
        }
        let storage = CanonicalKirOccurrenceRowBytesStorageV1(
            header.checked_add(capacity).ok_or(Resource::Arithmetic)?,
        );
        Ok((
            InertCanonicalKirOccurrenceRowBytesV1 {
                bytes: writer.bytes,
                counts: encoded_counts,
                storage,
            },
            storage,
        ))
    })
}

/// Checks all existing row tags/padding and both complete range partitions.
/// This is allocation-free grammar validation, not an endpoint or rewrite check.
/// A complete prepaid syntax pass precedes the prepaid block/definition revisit;
/// all other nested frames and semantic relation checks remain caller duties.
/// Returned fixed header is unreserved; raw backing remains borrowed/external.
pub fn read_canonical_kir_occurrence_row_bytes_v1<'bytes>(
    bytes: &'bytes [u8],
    counts: [u32; 9],
    budget: &mut Budget<'_>,
) -> Result<CanonicalKirOccurrenceRowsRefV1<'bytes>> {
    scoped(budget, |budget| {
        budget.charge_work(1)?;
        if bytes.len() > MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1 {
            return Err(CanonicalKirTransitionReceiptErrorV1::Limit);
        }
        let (offsets, count) = layout(counts, budget)?;
        if offsets[9] != bytes.len() {
            return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                "row body extent",
            ));
        }
        let revisits = usize::try_from(counts[1])
            .ok()
            .and_then(|n| n.checked_add(usize::try_from(counts[4]).ok()?))
            .ok_or(Resource::Arithmetic)?;
        let visit_bytes = usize::try_from(counts[1])
            .ok()
            .and_then(|n| n.checked_mul(WIDTHS[1]))
            .and_then(|n| n.checked_add(usize::try_from(counts[4]).ok()?.checked_mul(WIDTHS[4])?))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(
            bytes
                .len()
                .checked_add(count)
                .and_then(|n| n.checked_add(revisits))
                .and_then(|n| n.checked_add(visit_bytes))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let storage = CanonicalKirOccurrenceRowBytesStorageV1(size_of::<
            CanonicalKirOccurrenceRowsRefV1<'_>,
        >());
        budget.reserve_storage(storage.0)?;
        let mut reader = Reader { bytes, offset: 0 };
        macro_rules! visit {
            ($axis:expr, $method:ident) => {
                for _ in 0..counts[$axis] {
                    let _ = reader.$method()?;
                }
            };
        }
        visit!(0, function_row);
        visit!(1, block_row);
        visit!(2, segment);
        visit!(3, operation_row);
        visit!(4, definition_row);
        visit!(5, descendant);
        visit!(6, use_row);
        visit!(7, edge_row);
        visit!(8, edge_argument_row);
        if reader.offset != bytes.len() {
            return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                "row body trailing data",
            ));
        }
        reader.offset = offsets[1];
        let mut end = 0;
        for _ in 0..counts[1] {
            let row = reader.block_row()?;
            if row.segments.len == 0 {
                return Err(CanonicalKirTransitionReceiptErrorV1::RangePartition);
            }
            end = range_end(row.segments, end)?;
        }
        if end != usize::try_from(counts[2]).map_err(|_| Resource::Arithmetic)? {
            return Err(CanonicalKirTransitionReceiptErrorV1::RangePartition);
        }
        reader.offset = offsets[4];
        end = 0;
        for _ in 0..counts[4] {
            end = range_end(reader.definition_row()?.outputs, end)?;
        }
        if end != usize::try_from(counts[5]).map_err(|_| Resource::Arithmetic)? {
            return Err(CanonicalKirTransitionReceiptErrorV1::RangePartition);
        }
        Ok(CanonicalKirOccurrenceRowsRefV1 {
            bytes,
            counts,
            storage,
        })
    })
}

#[cfg(test)]
#[path = "canonical_kir_occurrence_row_bytes_v1_tests.rs"]
mod tests;
