//! Structural physical-layout checks, not source validity or active-variant proof.
//! The public checker grants no source, module, target or runtime authority.

use crate::storage_layout_v1::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError, ScalarType,
    verification_bounded_sort_by_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageLayoutLimitsV1 {
    pub rows: usize,
    pub edges: usize,
    pub containment_depth: usize,
    pub object_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageLayoutProblemV1 {
    Rows,
    Edges,
    Depth,
    InvalidId,
    Alignment,
    Size,
    Overlap,
    Scalar,
    Vector,
    Pointer,
    Slice,
    Variant,
    ContainmentCycle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageLayoutErrorV1 {
    Resource(ResourceError),
    Invalid {
        row: usize,
        problem: StorageLayoutProblemV1,
    },
}

impl From<ResourceError> for StorageLayoutErrorV1 {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for StorageLayoutErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => std::fmt::Display::fmt(error, formatter),
            Self::Invalid { row, problem } => {
                write!(formatter, "storage layout row {row}: {problem:?}")
            }
        }
    }
}

impl std::error::Error for StorageLayoutErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Invalid { .. } => None,
        }
    }
}

/// Borrows the exact structurally checked table, without authority or Clone.
/// Callers must separately join source/module ownership and target/runtime facts.
#[derive(Debug)]
pub struct StructurallyCheckedStorageLayoutsV1<'a> {
    rows: &'a [StorageLayoutV1],
}

impl<'a> StructurallyCheckedStorageLayoutsV1<'a> {
    /// Inert layout data only. Initialization, value validity, variant state,
    /// target ABI agreement and source provenance are deliberately unproved.
    pub fn rows(&self) -> &[StorageLayoutV1] {
        self.rows
    }

    /// Resolution always uses this exact borrowed table. Bare IDs must never be
    /// used for cross-module type compatibility or context-wide type uniquing.
    pub fn row(&self, id: StorageLayoutIdV1) -> Option<&StorageLayoutV1> {
        self.rows.get(id.0 as usize)
    }
}

#[derive(Clone, Copy)]
struct Frame {
    row: usize,
    next: usize,
    height: usize,
}

struct Scratch<'a, 'work> {
    budget: &'a mut Budget<'work>,
    colors: Option<Vec<u8>>,
    frames: Option<Vec<Frame>>,
    heights: Option<Vec<usize>>,
    ranges: Option<Vec<(u64, u64)>>,
    keys: Option<Vec<u128>>,
    charged: usize,
}

// Logical fixed headers include the owned vectors, allocation/return temporaries,
// DFS locals and simultaneously live outcome slots. Caller-owned input is borrowed.
fn headers() -> Result<usize, ResourceError> {
    let allocation_result = std::mem::size_of::<Result<Vec<u8>, StorageLayoutErrorV1>>()
        .max(std::mem::size_of::<Result<Vec<Frame>, StorageLayoutErrorV1>>())
        .max(std::mem::size_of::<Result<Vec<usize>, StorageLayoutErrorV1>>());
    let allocation_result = allocation_result
        .max(std::mem::size_of::<
            Result<Vec<(u64, u64)>, StorageLayoutErrorV1>,
        >())
        .max(std::mem::size_of::<Result<Vec<u128>, StorageLayoutErrorV1>>());
    [
        (1, std::mem::size_of::<Scratch<'_, '_>>()),
        (
            2,
            std::mem::size_of::<Result<Scratch<'_, '_>, StorageLayoutErrorV1>>(),
        ),
        (2, std::mem::size_of::<Vec<u8>>()),
        (2, std::mem::size_of::<Vec<Frame>>()),
        (2, std::mem::size_of::<Vec<usize>>()),
        (2, std::mem::size_of::<Vec<(u64, u64)>>()),
        (2, std::mem::size_of::<Vec<u128>>()),
        (2, allocation_result),
        (2, std::mem::size_of::<Result<(), StorageLayoutErrorV1>>()),
        (2, std::mem::size_of::<Result<(), ResourceError>>()),
        (
            2,
            std::mem::size_of::<
                Result<StructurallyCheckedStorageLayoutsV1<'_>, StorageLayoutErrorV1>,
            >(),
        ),
        (2, std::mem::size_of::<Frame>()),
    ]
    .into_iter()
    .try_fold(0_usize, |total, (count, bytes)| {
        bytes
            .checked_mul(count)
            .and_then(|bytes| total.checked_add(bytes))
            .ok_or(ResourceError::Arithmetic)
    })
}

impl<'a, 'work> Scratch<'a, 'work> {
    fn new(budget: &'a mut Budget<'work>) -> Result<Self, StorageLayoutErrorV1> {
        let charged = headers()?;
        budget.reserve_storage(charged)?;
        Ok(Self {
            budget,
            colors: None,
            frames: None,
            heights: None,
            ranges: None,
            keys: None,
            charged,
        })
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), StorageLayoutErrorV1> {
        let total = self
            .charged
            .checked_add(bytes)
            .ok_or(ResourceError::Arithmetic)?;
        self.budget.reserve_storage(bytes)?;
        self.charged = total;
        Ok(())
    }

    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, StorageLayoutErrorV1> {
        self.budget.charge_work(1)?;
        let requested = count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(ResourceError::Arithmetic)?;
        self.reserve(requested)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| ResourceError::Allocation)?;
        let actual = values
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(ResourceError::Arithmetic)?;
        self.reserve(
            actual
                .checked_sub(requested)
                .ok_or(ResourceError::Accounting)?,
        )?;
        Ok(values)
    }

    fn finish(&mut self) -> Result<(), ResourceError> {
        drop(self.colors.take());
        drop(self.frames.take());
        drop(self.heights.take());
        drop(self.ranges.take());
        drop(self.keys.take());
        let charged = std::mem::replace(&mut self.charged, 0);
        self.budget.release_storage(charged)
    }
}

impl Drop for Scratch<'_, '_> {
    fn drop(&mut self) {
        // No callback or foreign meter can alter this exclusive scope's floor.
        // Normal completion propagates cleanup failure; unwind still drops first.
        let _ = self.finish();
    }
}

fn invalid(row: usize, problem: StorageLayoutProblemV1) -> StorageLayoutErrorV1 {
    StorageLayoutErrorV1::Invalid { row, problem }
}

fn lookup<'a>(
    rows: &'a [StorageLayoutV1],
    id: StorageLayoutIdV1,
    row: usize,
    budget: &mut Budget<'_>,
) -> Result<&'a StorageLayoutV1, StorageLayoutErrorV1> {
    budget.charge_work(1)?;
    rows.get(id.0 as usize)
        .ok_or_else(|| invalid(row, StorageLayoutProblemV1::InvalidId))
}

fn field_range(
    rows: &[StorageLayoutV1],
    parent: usize,
    field: StorageFieldV1,
    budget: &mut Budget<'_>,
) -> Result<(u64, u64), StorageLayoutErrorV1> {
    let child = lookup(rows, field.layout, parent, budget)?;
    budget.charge_work(3)?;
    let end = field
        .offset
        .checked_add(child.size)
        .ok_or_else(|| invalid(parent, StorageLayoutProblemV1::Size))?;
    if end > rows[parent].size {
        return Err(invalid(parent, StorageLayoutProblemV1::Size));
    }
    // Packed field placement is allowed. Projection/access validation must use
    // placement_alignment, not silently reuse the child type's ABI alignment.
    Ok((field.offset, end))
}

fn disjoint(a: (u64, u64), b: (u64, u64)) -> bool {
    a.0 == a.1 || b.0 == b.1 || a.1 <= b.0 || b.1 <= a.0
}

fn scalar_width(scalar: ScalarType, row: &StorageLayoutV1) -> bool {
    match scalar.bit_width() {
        Some(bits) => row.size == u64::from(bits.div_ceil(8)),
        None => scalar == ScalarType::Index && matches!(row.size, 1 | 2 | 4 | 8 | 16),
    }
}

fn unique_keys(
    keys: &mut [u128],
    budget: &mut Budget<'_>,
    row: usize,
) -> Result<(), StorageLayoutErrorV1> {
    verification_bounded_sort_by_v1(keys, 1, budget, std::cmp::Ord::cmp)?;
    for pair in keys.windows(2) {
        budget.charge_work(1)?;
        if pair[0] == pair[1] {
            return Err(invalid(row, StorageLayoutProblemV1::Variant));
        }
    }
    Ok(())
}

fn validate_row(
    rows: &[StorageLayoutV1],
    index: usize,
    scratch: &mut Scratch<'_, '_>,
) -> Result<(), StorageLayoutErrorV1> {
    let row = &rows[index];
    let budget = &mut *scratch.budget;
    match &row.kind {
        StorageLayoutKindV1::Scalar(scalar) => {
            budget.charge_work(1)?;
            if !scalar_width(*scalar, row) || u64::from(row.alignment) > row.size {
                return Err(invalid(index, StorageLayoutProblemV1::Scalar));
            }
        }
        StorageLayoutKindV1::Vector(vector) => {
            budget.charge_work(4)?;
            let width = vector
                .byte_width()
                .map(u64::from)
                .ok_or_else(|| invalid(index, StorageLayoutProblemV1::Vector))?;
            let mask = u64::from(row.alignment) - 1;
            if width.checked_add(mask).map(|size| size & !mask) != Some(row.size) {
                return Err(invalid(index, StorageLayoutProblemV1::Vector));
            }
        }
        StorageLayoutKindV1::Pointer(pointer) => {
            lookup(rows, pointer.pointee, index, budget)?;
            budget.charge_work(1)?;
            if !matches!(pointer.stored_bits, 8 | 16 | 32 | 64 | 128)
                || row.size != u64::from(pointer.stored_bits / 8)
                || u64::from(row.alignment) > row.size
            {
                return Err(invalid(index, StorageLayoutProblemV1::Pointer));
            }
        }
        StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => {
            let union = matches!(row.kind, StorageLayoutKindV1::Union(_));
            let ranges = scratch.ranges.as_mut().ok_or(ResourceError::Accounting)?;
            ranges.clear();
            for &field in fields.iter() {
                let range = field_range(rows, index, field, budget)?;
                budget.charge_work(1)?;
                if union {
                    if field.offset != 0 {
                        return Err(invalid(index, StorageLayoutProblemV1::Size));
                    }
                } else if range.0 != range.1 {
                    ranges.push(range);
                }
            }
            verification_bounded_sort_by_v1(ranges, 2, budget, std::cmp::Ord::cmp)?;
            for pair in ranges.windows(2) {
                budget.charge_work(1)?;
                if pair[0].1 > pair[1].0 {
                    return Err(invalid(index, StorageLayoutProblemV1::Overlap));
                }
            }
        }
        StorageLayoutKindV1::Array {
            element,
            length,
            stride,
        } => {
            let child = lookup(rows, *element, index, budget)?;
            budget.charge_work(3)?;
            if *stride != child.size
                || length.checked_mul(*stride) != Some(row.size)
                || row.alignment != child.alignment
            {
                return Err(invalid(index, StorageLayoutProblemV1::Size));
            }
        }
        StorageLayoutKindV1::Slice {
            element,
            value_space,
            access,
            data,
            length,
        } => {
            lookup(rows, *element, index, budget)?;
            let data_range = field_range(rows, index, *data, budget)?;
            let length_range = field_range(rows, index, *length, budget)?;
            let pointer_row = lookup(rows, data.layout, index, budget)?;
            let length_row = lookup(rows, length.layout, index, budget)?;
            budget.charge_work(4)?;
            if !matches!(&pointer_row.kind, StorageLayoutKindV1::Pointer(pointer)
                if pointer.pointee == *element && pointer.value_space == *value_space
                    && pointer.access == *access)
                || !matches!(
                    length_row.kind,
                    StorageLayoutKindV1::Scalar(ScalarType::Index)
                )
                || !disjoint(data_range, length_range)
            {
                return Err(invalid(index, StorageLayoutProblemV1::Slice));
            }
        }
        StorageLayoutKindV1::Variants { encoding, variants } => {
            budget.charge_work(1)?;
            if variants.is_empty() {
                return Err(invalid(index, StorageLayoutProblemV1::Variant));
            }
            field_range(rows, index, encoding.tag(), budget)?;
            let tag = lookup(rows, encoding.tag().layout, index, budget)?;
            let bits = match &tag.kind {
                StorageLayoutKindV1::Scalar(scalar)
                    if scalar.is_integer()
                        || (*scalar == ScalarType::Bool
                            && matches!(encoding, StorageVariantEncodingV1::Niche { .. })) =>
                {
                    u16::try_from(tag.size.checked_mul(8).ok_or(ResourceError::Arithmetic)?).ok()
                }
                StorageLayoutKindV1::Pointer(pointer)
                    if matches!(encoding, StorageVariantEncodingV1::Niche { .. }) =>
                {
                    Some(pointer.stored_bits)
                }
                _ => None,
            }
            .ok_or_else(|| invalid(index, StorageLayoutProblemV1::Variant))?;
            if !(1..=128).contains(&bits) {
                return Err(invalid(index, StorageLayoutProblemV1::Variant));
            }
            let keys = scratch.keys.as_mut().ok_or(ResourceError::Accounting)?;
            keys.clear();
            for variant in variants.iter() {
                field_range(
                    rows,
                    index,
                    StorageFieldV1 {
                        offset: 0,
                        layout: variant.layout,
                    },
                    budget,
                )?;
                budget.charge_work(1)?;
                match (encoding, variant.direct_tag_bits) {
                    (StorageVariantEncodingV1::Direct { .. }, Some(value))
                        if bits == 128 || value < (1_u128 << bits) => {}
                    (StorageVariantEncodingV1::Niche { .. }, None) => {}
                    _ => return Err(invalid(index, StorageLayoutProblemV1::Variant)),
                }
                keys.push(variant.discriminant);
            }
            unique_keys(keys, budget, index)?;
            if matches!(encoding, StorageVariantEncodingV1::Direct { .. }) {
                keys.clear();
                for variant in variants.iter() {
                    budget.charge_work(1)?;
                    keys.push(variant.direct_tag_bits.ok_or(ResourceError::Accounting)?);
                }
                unique_keys(keys, budget, index)?;
            }
            if let StorageVariantEncodingV1::Niche {
                untagged_variant,
                first_niche_variant,
                last_niche_variant,
                niche_start,
                ..
            } = encoding
            {
                budget.charge_work(5)?;
                let first = *first_niche_variant as usize;
                let last = *last_niche_variant as usize;
                let untagged = *untagged_variant as usize;
                if first > last
                    || last >= variants.len()
                    || untagged >= variants.len()
                    || (first..=last).contains(&untagged)
                    || (bits < 128
                        && (*niche_start >= (1_u128 << bits)
                            || (last - first + 1) as u128 > (1_u128 << bits)))
                {
                    return Err(invalid(index, StorageLayoutProblemV1::Variant));
                }
                for (position, variant) in variants.iter().enumerate() {
                    budget.charge_work(1)?;
                    if variant.discriminant != position as u128
                        || (position != untagged
                            && !(first..=last).contains(&position)
                            && !variant.uninhabited)
                    {
                        return Err(invalid(index, StorageLayoutProblemV1::Variant));
                    }
                }
            }
        }
    }
    Ok(())
}

fn check_graph(
    rows: &[StorageLayoutV1],
    limits: StorageLayoutLimitsV1,
    scratch: &mut Scratch<'_, '_>,
) -> Result<(), StorageLayoutErrorV1> {
    scratch.budget.charge_work(1)?;
    if rows.len() > limits.rows || rows.len() > u32::MAX as usize {
        return Err(invalid(0, StorageLayoutProblemV1::Rows));
    }
    let mut edges = 0_usize;
    let mut max_ranges = 0;
    let mut max_keys = 0;
    for (index, row) in rows.iter().enumerate() {
        scratch.budget.charge_work(3)?;
        if row.alignment == 0
            || !row.alignment.is_power_of_two()
            || row.size % u64::from(row.alignment) != 0
        {
            return Err(invalid(index, StorageLayoutProblemV1::Alignment));
        }
        if row.size > limits.object_bytes {
            return Err(invalid(index, StorageLayoutProblemV1::Size));
        }
        let contained = row
            .kind
            .containment_count()
            .ok_or(ResourceError::Arithmetic)?;
        let references = usize::from(matches!(
            row.kind,
            StorageLayoutKindV1::Pointer(_) | StorageLayoutKindV1::Slice { .. }
        ));
        edges = edges
            .checked_add(contained)
            .and_then(|n| n.checked_add(references))
            .ok_or(ResourceError::Arithmetic)?;
        if edges > limits.edges {
            return Err(invalid(index, StorageLayoutProblemV1::Edges));
        }
        match &row.kind {
            StorageLayoutKindV1::Record(fields) => max_ranges = max_ranges.max(fields.len()),
            StorageLayoutKindV1::Variants { variants, .. } => {
                max_keys = max_keys.max(variants.len())
            }
            _ => {}
        }
    }
    scratch.ranges = Some(scratch.vector::<(u64, u64)>(max_ranges)?);
    scratch.keys = Some(scratch.vector::<u128>(max_keys)?);
    for index in 0..rows.len() {
        validate_row(rows, index, scratch)?;
    }
    let mut colors = scratch.vector::<u8>(rows.len())?;
    scratch.budget.charge_work(rows.len())?;
    colors.resize(rows.len(), 0);
    scratch.colors = Some(colors);
    scratch.frames = Some(scratch.vector::<Frame>(rows.len())?);
    let mut heights = scratch.vector::<usize>(rows.len())?;
    scratch.budget.charge_work(rows.len())?;
    heights.resize(rows.len(), 0);
    scratch.heights = Some(heights);
    let colors = scratch.colors.as_mut().ok_or(ResourceError::Accounting)?;
    let frames = scratch.frames.as_mut().ok_or(ResourceError::Accounting)?;
    let heights = scratch.heights.as_mut().ok_or(ResourceError::Accounting)?;
    for root in 0..rows.len() {
        scratch.budget.charge_work(1)?;
        if colors[root] != 0 {
            continue;
        }
        if limits.containment_depth == 0 {
            return Err(invalid(root, StorageLayoutProblemV1::Depth));
        }
        colors[root] = 1;
        frames.push(Frame {
            row: root,
            next: 0,
            height: 1,
        });
        while let Some(frame) = frames.last().copied() {
            scratch.budget.charge_work(1)?;
            let top = frames.len() - 1;
            let parent = frame.row;
            let child = rows[parent].kind.containment_child(frame.next);
            frames[top].next = frame.next.checked_add(1).ok_or(ResourceError::Arithmetic)?;
            let Some(child) = child else {
                let height = frame.height;
                if height > limits.containment_depth {
                    return Err(invalid(parent, StorageLayoutProblemV1::Depth));
                }
                heights[parent] = height;
                colors[parent] = 2;
                frames.pop();
                if let Some(outer) = frames.last_mut() {
                    outer.height = outer
                        .height
                        .max(height.checked_add(1).ok_or(ResourceError::Arithmetic)?);
                }
                continue;
            };
            let child = child.0 as usize;
            let color = colors
                .get(child)
                .copied()
                .ok_or_else(|| invalid(parent, StorageLayoutProblemV1::InvalidId))?;
            if color == 1 {
                return Err(invalid(parent, StorageLayoutProblemV1::ContainmentCycle));
            }
            if color == 0 {
                if frames.len() >= limits.containment_depth {
                    return Err(invalid(parent, StorageLayoutProblemV1::Depth));
                }
                colors[child] = 1;
                frames.push(Frame {
                    row: child,
                    next: 0,
                    height: 1,
                });
            } else {
                frames[top].height = frame.height.max(
                    heights[child]
                        .checked_add(1)
                        .ok_or(ResourceError::Arithmetic)?,
                );
            }
        }
    }
    Ok(())
}

/// Checks bounded physical structure while preserving the caller's ledger floor.
/// Input ownership, source validity, initialization, active variants, pointer
/// provenance and target representation agreement are not certified here.
pub fn check_storage_layouts_v1<'a>(
    rows: &'a [StorageLayoutV1],
    limits: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<StructurallyCheckedStorageLayoutsV1<'a>, StorageLayoutErrorV1> {
    budget.charge_work(1)?;
    let mut scratch = Scratch::new(budget)?;
    let result = check_graph(rows, limits, &mut scratch);
    let cleanup = scratch.finish();
    result?;
    cleanup?;
    Ok(StructurallyCheckedStorageLayoutsV1 { rows })
}

#[cfg(test)]
#[path = "verification_storage_v1_tests.rs"]
mod tests;

// Inert pinned-host test premises; no resource total or verification authority.
#[cfg(test)]
pub(crate) fn canonical_storage_header_layout_premises_v18() -> [usize; 5] {
    [
        std::mem::size_of::<Scratch<'_, '_>>(),
        std::mem::size_of::<Result<Scratch<'_, '_>, StorageLayoutErrorV1>>(),
        std::mem::size_of::<Vec<Frame>>(),
        std::mem::size_of::<Result<Vec<Frame>, StorageLayoutErrorV1>>(),
        std::mem::size_of::<Frame>(),
    ]
}
