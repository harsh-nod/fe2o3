//! Preflight half of the exact synthetic-local order, including debug IDs.

use super::*;
use crate::production_safe_core_shift_v1::locals::{self, LocalOrderV1};

fn error(
    error: locals::Error<ProductionSemanticPreflightErrorV1>,
) -> ProductionSemanticPreflightErrorV1 {
    match error {
        locals::Error::Resource(error) => error,
        locals::Error::Allocation | locals::Error::Invalid => {
            ProductionSemanticPreflightErrorV1::IdentityTableMismatch
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn remap_v1(
    function: SemanticFunctionIdV1,
    identity: SemanticFunctionIdentityV1,
    body_sha256: [u8; 32],
    raw_locals: &[RetainedSemanticLocalProducerV1],
    receiver: Option<ReceiverLocalV1>,
    calls: &[NormalizedRustcIntrinsicRecipeV1<'_>],
    raw_to_semantic: &mut [SemanticLocalIdV1],
    debug: &mut [RetainedDebugSourceVariableV2],
    counts: &mut RawMirPreflightCountsV1,
    limits: SemanticMirLimitsV1,
) -> Result<(), ProductionSemanticPreflightErrorV1> {
    // Pass-one order is caller, then raw block. Bound the two binary searches
    // once per function instead of rescanning every call for every function.
    counts.charge(
        SemanticMirResourceV1::ValidationWork,
        if calls.is_empty() {
            0
        } else {
            2 * (usize::BITS - calls.len().leading_zeros()) as usize + 2
        },
        limits,
    )?;
    let begin = calls.partition_point(|call| call.caller < function);
    let end = calls.partition_point(|call| call.caller <= function);
    let calls = &calls[begin..end];
    let count = locals::prepaid_rows_v1(calls, |work| {
        counts.charge(SemanticMirResourceV1::ValidationWork, work, limits)
    })?
    .filter(|call| matches!(call.operation, NormalizedCallV1::SafeCoreShift(_)))
    .count();
    if count == 0 {
        return Ok(());
    }
    let capacity = count
        .checked_mul(2)
        .and_then(|n| n.checked_add(usize::from(receiver.is_some())))
        .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
    counts.charge(SemanticMirResourceV1::ValidationWork, capacity, limits)?;
    let mut inserted = Vec::new();
    inserted
        .try_reserve_exact(capacity)
        .map_err(|_| ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
    if let Some(receiver) = receiver {
        inserted.push(receiver.identity);
    }
    for call in locals::prepaid_rows_v1(calls, |work| {
        counts.charge(SemanticMirResourceV1::ValidationWork, work, limits)
    })? {
        if !matches!(call.operation, NormalizedCallV1::SafeCoreShift(_)) {
            continue;
        }
        inserted.extend_from_slice(&locals::identities_v1(
            identity,
            rustc_block_identity_v1(identity, body_sha256, call.block),
            call.identities.function(),
        ));
    }
    let order = LocalOrderV1::new(
        raw_locals.iter().map(|local| local.identity),
        &inserted,
        |work| counts.charge(SemanticMirResourceV1::ValidationWork, work, limits),
    )
    .map_err(error)?;
    let old_count = raw_locals
        .len()
        .checked_add(usize::from(receiver.is_some()))
        .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
    counts.charge(SemanticMirResourceV1::ValidationWork, old_count, limits)?;
    let mut remap = Vec::new();
    remap
        .try_reserve_exact(old_count)
        .map_err(|_| ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
    remap.resize(old_count, None);
    for local in raw_locals {
        let slot = raw_to_semantic
            .get_mut(local.rustc_local as usize)
            .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
        let new = order
            .index(local.identity, |work| {
                counts.charge(SemanticMirResourceV1::ValidationWork, work, limits)
            })
            .map_err(error)?;
        if remap
            .get_mut(slot.index() as usize)
            .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?
            .replace(new)
            .is_some()
        {
            return Err(ProductionSemanticPreflightErrorV1::IdentityTableMismatch);
        }
        *slot = new;
    }
    counts.charge(SemanticMirResourceV1::ValidationWork, debug.len(), limits)?;
    for variable in debug {
        if let RetainedDebugSourceVariableClassV2::Local(local) = &mut variable.class {
            *local = remap
                .get(local.index() as usize)
                .copied()
                .flatten()
                .ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?;
        }
    }
    Ok(())
}
