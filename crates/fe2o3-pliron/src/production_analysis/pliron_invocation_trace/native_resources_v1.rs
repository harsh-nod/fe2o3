//! Byte-domain reservations and bounded native fold scratch.
use crate::production_analysis::CanonicalRankedPolicyFailureV1 as Failure;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{collections::HashMap, hash::Hash, mem::size_of};

pub(crate) fn reserve_rows<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, Failure> {
    budget.charge_work(1)?;
    budget.reserve_storage(
        size_of::<Vec<T>>()
            .checked_add(
                count
                    .checked_mul(size_of::<T>())
                    .ok_or(Resource::Arithmetic)?,
            )
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        values
            .capacity()
            .checked_sub(count)
            .and_then(|v| v.checked_mul(size_of::<T>()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(values)
}

pub(crate) fn reserve_map<K: Eq + Hash, V>(
    count: usize,
    budget: &mut Budget<'_>,
) -> Result<HashMap<K, V>, Failure> {
    // A paid capacity envelope, not allocator size-class accounting. The bridge
    // already uses this domain for arena/interner capacity envelopes.
    let stride = size_of::<(K, V)>()
        .checked_add(16)
        .ok_or(Resource::Arithmetic)?;
    let rows = count
        .checked_mul(2)
        .and_then(|v| v.checked_add(1))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(1)?;
    budget.reserve_storage(
        size_of::<HashMap<K, V>>()
            .checked_add(rows.checked_mul(stride).ok_or(Resource::Arithmetic)?)
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut map = HashMap::new();
    map.try_reserve(count).map_err(|_| Resource::Allocation)?;
    if map.capacity() > rows {
        budget.reserve_storage(
            (map.capacity() - rows)
                .checked_mul(stride)
                .ok_or(Resource::Arithmetic)?,
        )?;
    }
    Ok(map)
}

/// Pinned Awi uses one inline digit through 64 bits and digit-rounded backing
/// above it. The admitted scalar roster ends at 128 bits; multiply's 2W
/// temporaries therefore end at 256 bits. No dependency or quota is changed.
pub(crate) fn awi_backing(width: usize) -> Result<usize, Resource> {
    if usize::BITS != 64 || width == 0 || width > 256 {
        return Err(Resource::Arithmetic);
    }
    if width <= 64 {
        return Ok(0);
    }
    width
        .checked_add(63)
        .map(|v| (v / 64) * 8)
        .ok_or(Resource::Arithmetic)
}

pub(crate) fn fold_scratch(
    width: usize,
    operands: usize,
    results: usize,
) -> Result<usize, Resource> {
    use pliron::{attribute::AttrObj, builtin::attributes::IntegerAttr, utils::apint::APInt};
    let scalar = size_of::<IntegerAttr>()
        .checked_add(awi_backing(width)?)
        .ok_or(Resource::Arithmetic)?;
    let slots = operands.checked_add(results).ok_or(Resource::Arithmetic)?;
    let vectors = 2_usize
        .checked_mul(size_of::<Vec<Option<AttrObj>>>())
        .and_then(|v| v.checked_add(slots.checked_mul(size_of::<Option<AttrObj>>())?))
        .ok_or(Resource::Arithmetic)?;
    // integer_operands clones two full IntegerAttrs, including type handles.
    // The remaining APInts are two value() clones, two W truncations and the
    // final W product, plus six 2W multiply temporaries.
    let wide = 6_usize
        .checked_mul(
            size_of::<APInt>()
                .checked_add(awi_backing(
                    width.checked_mul(2).ok_or(Resource::Arithmetic)?,
                )?)
                .ok_or(Resource::Arithmetic)?,
        )
        .ok_or(Resource::Arithmetic)?;
    vectors
        .checked_add(slots.checked_mul(scalar).ok_or(Resource::Arithmetic)?)
        .and_then(|v| v.checked_add(wide))
        .and_then(|v| v.checked_add(2_usize.checked_mul(scalar)?))
        .and_then(|v| {
            v.checked_add(5_usize.checked_mul(size_of::<APInt>() + awi_backing(width).ok()?)?)
        })
        .ok_or(Resource::Arithmetic)
}
