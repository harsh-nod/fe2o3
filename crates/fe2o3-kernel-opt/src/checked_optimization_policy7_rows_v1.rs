//! Inert typed rows from the existing P7 transcript; no graph or execution seal.
use super::*;
use fe2o3_kernel_ir::{CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1};

/// Local decoder input ceiling, not an increase to any enclosing evidence cap.
pub const MAX_POLICY7_EXECUTION_RECORD_BYTES_V1: usize = 4 * 1024 * 1024;

/// Owns only decoded rows and borrows the unchanged transcript. Fixed P7 syntax
/// and row order are checked; nested P6 and graph identities remain inert claims.
/// Semantic and sealed-execution checks must still use their existing adapters.
/// Claims cannot outlive this owner, and this owner cannot outlive its wire.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::{DecodedCanonicalPolicy7RowsV1, CanonicalPolicy7ContinuationClaimsV1};
/// fn escape<'a>(rows: DecodedCanonicalPolicy7RowsV1<'a>)
///     -> CanonicalPolicy7ContinuationClaimsV1<'a> { rows.claims() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedCanonicalPolicy7RowsV1;
/// fn escape<'a>(rows: DecodedCanonicalPolicy7RowsV1<'a>)
///     -> DecodedCanonicalPolicy7RowsV1<'static> { rows }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedCanonicalPolicy7RowsV1;
/// fn clone(rows: DecodedCanonicalPolicy7RowsV1<'_>) { let _ = rows.clone(); }
/// ```
pub struct DecodedCanonicalPolicy7RowsV1<'wire> {
    wire: &'wire [u8],
    deletions: Vec<Row>,
    retained: Vec<Retained>,
    storage: CanonicalPolicy7SemanticStorageV1,
}
impl DecodedCanonicalPolicy7RowsV1<'_> {
    /// This borrow, not the input wire lifetime, bounds all returned row slices.
    pub fn claims(&self) -> CanonicalPolicy7ContinuationClaimsV1<'_> {
        CanonicalPolicy7ContinuationClaimsV1 {
            execution_record: self.wire,
            deletion_rows: &self.deletions,
            retained_operations: &self.retained,
        }
    }
    /// Wrapper and actual typed Vec capacities; borrowed wire backing excluded.
    pub const fn storage(&self) -> CanonicalPolicy7SemanticStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
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

fn allocate<T>(count: usize, budget: &mut Budget<'_>) -> Result<(Vec<T>, usize), Error> {
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let capacity = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        capacity
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok((values, capacity))
}

/// Decodes the existing record only. Does not admit graphs, rerun optimization,
/// validate P6 semantics, authenticate execution, or construct a sealed witness.
///
/// One work debit covers the O(1) length gate, including tiny/oversized refusal.
/// A fixed header prepayment precedes both encoded count reads and all header
/// checks. Only then is complete body/row work prepaid before allocation/scan.
/// Actual Vec capacities are reconciled before initialization. Success returns
/// owned storage unreserved; caller must reserve it while composing checks.
/// Input backing is borrowed/external, not copied or charged a second time.
/// Work accumulates on the same ledger; every exit restores the incoming floor.
pub fn decode_canonical_policy7_rows_v1<'wire>(
    bytes: &'wire [u8],
    budget: &mut Budget<'_>,
) -> Result<DecodedCanonicalPolicy7RowsV1<'wire>, Error> {
    scoped(budget, |budget| {
        budget.charge_work(1)?;
        if !(POLICY7_EXECUTION_HEADER_BYTES_V1..=MAX_POLICY7_EXECUTION_RECORD_BYTES_V1)
            .contains(&bytes.len())
        {
            return Err(Error::Record);
        }
        budget.charge_work(POLICY7_EXECUTION_HEADER_BYTES_V1 + 3)?;
        let count = |offset| -> Result<usize, Error> {
            usize::try_from(u64::from_le_bytes(
                bytes[offset..offset + 8]
                    .try_into()
                    .map_err(|_| Error::Record)?,
            ))
            .map_err(|_| Resource::Arithmetic.into())
        };
        let layout = RecordLayout::new(count(352)?, count(360)?)?;
        check_record_header(bytes, &layout)?;
        budget.charge_work(
            (bytes.len() - POLICY7_EXECUTION_HEADER_BYTES_V1)
                .checked_add(layout.count)
                .and_then(|n| n.checked_add(4))
                .ok_or(Resource::Arithmetic)?,
        )?;
        budget.reserve_storage(size_of::<DecodedCanonicalPolicy7RowsV1<'_>>())?;
        let (mut deletions, deletion_capacity) = allocate::<Row>(layout.deletions, budget)?;
        let (mut retained, retained_capacity) = allocate::<Retained>(layout.retained, budget)?;
        let mut encoded =
            bytes[POLICY7_EXECUTION_HEADER_BYTES_V1..].chunks_exact(POLICY7_EXECUTION_ROW_BYTES_V1);
        let mut previous = None;
        for _ in 0..layout.deletions {
            let bytes = encoded.next().ok_or(Error::Record)?;
            let row = Row {
                anchor: coordinate(&bytes[..12]),
                removed: coordinate(&bytes[12..]),
            };
            if !deletion_order(previous, &row) {
                return Err(Error::Rows);
            }
            previous = Some(row.removed);
            deletions.push(row);
        }
        let mut previous = None;
        for _ in 0..layout.retained {
            let bytes = encoded.next().ok_or(Error::Record)?;
            let row = Retained {
                input: coordinate(&bytes[..12]),
                output: coordinate(&bytes[12..]),
            };
            if !retained_order(previous, &row) {
                return Err(Error::Rows);
            }
            previous = Some((row.input, row.output));
            retained.push(row);
        }
        if encoded.next().is_some() || !encoded.remainder().is_empty() {
            return Err(Error::Record);
        }
        let storage = size_of::<DecodedCanonicalPolicy7RowsV1<'_>>()
            .checked_add(deletion_capacity)
            .and_then(|n| n.checked_add(retained_capacity))
            .ok_or(Resource::Arithmetic)?;
        Ok(DecodedCanonicalPolicy7RowsV1 {
            wire: bytes,
            deletions,
            retained,
            storage: CanonicalPolicy7SemanticStorageV1(storage),
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy7_rows_v1_tests.rs"]
mod tests;
