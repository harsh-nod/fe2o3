//! Owned neutral rows, without a graph identity, checker policy or wire frame.
use super::{
    Budget, Candidate, CanonicalKirOccurrenceRowsRefV1,
    CanonicalKirTransitionReceiptErrorV1 as Error, Reader, Resource, Result, Rows, SIZES, layout,
    scoped, validate_ranges,
};
use std::mem::size_of;

/// Exact new logical owner header and all nine observed typed-vector capacities.
/// Returned unreserved; input view and raw byte backing remain caller-owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirOccurrenceRowsStorageV1(usize);
impl CanonicalKirOccurrenceRowsStorageV1 {
    /// Reserve before subsequent controlled work while the returned owner lives.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only materialized row claims. No raw byte copy or source borrow is kept.
/// Owning syntactically valid rows does not prove any graph relation or execution.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::InertOwnedCanonicalKirOccurrenceRowsV1;
/// fn duplicate(rows: InertOwnedCanonicalKirOccurrenceRowsV1) { let _ = rows.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::{InertOwnedCanonicalKirOccurrenceRowsV1 as Rows,
///     CanonicalKirTransitionCandidateV1 as Candidate};
/// fn escape(rows: Rows) -> Candidate<'static> { rows.candidate() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::InertOwnedCanonicalKirOccurrenceRowsV1;
/// fn still_borrowed(rows: InertOwnedCanonicalKirOccurrenceRowsV1) {
///     let candidate = rows.candidate();
///     drop(rows);
///     let _ = candidate.functions.len();
/// }
/// ```
pub struct InertOwnedCanonicalKirOccurrenceRowsV1 {
    rows: Rows,
    storage: CanonicalKirOccurrenceRowsStorageV1,
}
impl InertOwnedCanonicalKirOccurrenceRowsV1 {
    /// Borrow all nine inert typed slices from this owner, not from raw bytes.
    pub fn candidate(&self) -> Candidate<'_> {
        self.rows.candidate()
    }
    /// Exact added logical retained storage, unreserved by the constructor.
    pub const fn storage(&self) -> CanonicalKirOccurrenceRowsStorageV1 {
        self.storage
    }
    /// No graph equivalence, source, artifact or publication authority is issued.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn account_capacity<T>(
    requested: usize,
    observed: usize,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    let actual = observed
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    let requested = requested
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok(actual)
}

fn allocate<T>(count: usize, budget: &mut Budget<'_>) -> Result<(Vec<T>, usize)> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let bytes = account_capacity::<T>(count, values.capacity(), budget)?;
    Ok((values, bytes))
}

/// Materializes the existing neutral view through the unchanged nine row readers.
/// No F2NTR frame is manufactured, no graph is admitted, and no semantic checker
/// is selected. The independent output has no artificial borrow of input bytes.
///
/// All requested typed backing and the new header are prepaid together. Every
/// observed capacity excess is reconciled before any grow-free fill. Conservative
/// logical work covers body reads, row visits, typed-byte writes and the separate
/// existing range check; this is not an allocator/RSS or instruction-count bound.
/// The input frame/view remains reserved once or external. Same-ledger scope
/// cleanup preserves the full inherited floor, cumulative work and first denials.
/// Success returns exact new storage UNRESERVED; reserve it before later work,
/// and drop this owner before releasing that reservation.
pub fn materialize_canonical_kir_occurrence_rows_v1(
    view: &CanonicalKirOccurrenceRowsRefV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(
    InertOwnedCanonicalKirOccurrenceRowsV1,
    CanonicalKirOccurrenceRowsStorageV1,
)> {
    scoped(budget, |budget| {
        budget.charge_work(19)?;
        let (offsets, row_count) = layout(view.counts(), budget)?;
        let bytes = view.canonical_row_bytes();
        if offsets[9] != bytes.len() {
            return Err(Error::Malformed("materialized row extent"));
        }
        let mut counts = [0usize; 9];
        let mut requested = 0usize;
        for (axis, count) in view.counts().into_iter().enumerate() {
            counts[axis] = usize::try_from(count).map_err(|_| Error::Limit)?;
            requested = counts[axis]
                .checked_mul(SIZES[axis])
                .and_then(|n| requested.checked_add(n))
                .ok_or(Resource::Arithmetic)?;
        }
        budget.charge_work(
            bytes
                .len()
                .checked_add(row_count)
                .and_then(|n| n.checked_add(requested))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let header = size_of::<InertOwnedCanonicalKirOccurrenceRowsV1>();
        budget.reserve_storage(header.checked_add(requested).ok_or(Resource::Arithmetic)?)?;
        let mut rows = Rows {
            functions: Vec::new(),
            blocks: Vec::new(),
            segments: Vec::new(),
            operations: Vec::new(),
            definitions: Vec::new(),
            definition_outputs: Vec::new(),
            uses: Vec::new(),
            edges: Vec::new(),
            edge_arguments: Vec::new(),
        };
        let mut retained = header;
        macro_rules! reserve_axis {
            ($axis:expr, $field:ident) => {{
                let (values, capacity_bytes) = allocate(counts[$axis], budget)?;
                rows.$field = values;
                retained = retained
                    .checked_add(capacity_bytes)
                    .ok_or(Resource::Arithmetic)?;
                #[cfg(test)]
                tests::after_allocation($axis)?;
            }};
        }
        reserve_axis!(0, functions);
        reserve_axis!(1, blocks);
        reserve_axis!(2, segments);
        reserve_axis!(3, operations);
        reserve_axis!(4, definitions);
        reserve_axis!(5, definition_outputs);
        reserve_axis!(6, uses);
        reserve_axis!(7, edges);
        reserve_axis!(8, edge_arguments);
        let mut reader = Reader { bytes, offset: 0 };
        macro_rules! fill_axis {
            ($axis:expr, $field:ident, $method:ident) => {
                for _ in 0..counts[$axis] {
                    rows.$field.push(reader.$method()?);
                }
            };
        }
        fill_axis!(0, functions, function_row);
        fill_axis!(1, blocks, block_row);
        fill_axis!(2, segments, segment);
        fill_axis!(3, operations, operation_row);
        fill_axis!(4, definitions, definition_row);
        fill_axis!(5, definition_outputs, descendant);
        fill_axis!(6, uses, use_row);
        fill_axis!(7, edges, edge_row);
        fill_axis!(8, edge_arguments, edge_argument_row);
        if reader.offset != bytes.len() {
            return Err(Error::Malformed("materialized row trailing data"));
        }
        validate_ranges(rows.candidate(), budget)?;
        let storage = CanonicalKirOccurrenceRowsStorageV1(retained);
        Ok((
            InertOwnedCanonicalKirOccurrenceRowsV1 { rows, storage },
            storage,
        ))
    })
}

#[cfg(test)]
#[path = "canonical_kir_occurrence_rows_owned_v1_tests.rs"]
mod tests;
