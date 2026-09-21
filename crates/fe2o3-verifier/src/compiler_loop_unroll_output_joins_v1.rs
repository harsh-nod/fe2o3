//! U framing adapters; all source, physical ABI and symbol predicates stay shared.
use super::super::joins as shared;
use super::*;
use fe2o3_compiler_ffi::InertLoopUnrollRootRefV1 as Root;
use fe2o3_compiler_lineage::MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3 as MAX_ROOTS;

pub(super) fn association(frame: &Frame<'_>, budget: &mut Budget<'_>) -> R<()> {
    shared::association_fields(
        frame.field(Field::OriginalInputV4),
        frame.field(Field::OriginalVerus),
        frame.field(Field::SemanticMir),
        frame.field(Field::OriginalMiddleEnd),
        frame.field(Field::OriginalNative),
        frame.field(Field::OriginalCorrespondence),
        frame.field(Field::OriginalFormalMemory),
        budget,
    )
    .map_err(E::Join)
}

pub(super) fn root_working() -> R<usize> {
    READ_STORAGE
        .checked_add(size_of::<Root<'_>>())
        .and_then(|n| n.checked_add(size_of::<[[bool; MAX_ROOTS]; 3]>()))
        .ok_or(Resource::Arithmetic.into())
}

pub(super) fn roots(
    frame: &Frame<'_>,
    inputs: &Inputs<'_>,
    output: &Graph,
    native: &Native,
    descriptor: &Descriptor,
    budget: &mut Budget<'_>,
) -> R<()> {
    let count = shared::root_header(frame.root_count(), inputs, output, native, descriptor)
        .map_err(E::Join)?;
    let scan = shared::root_scan(inputs, output, native, descriptor).map_err(E::Join)?;
    // Returned row and caller seen tables remain live separately from READ.
    budget.reserve_storage(root_working()?)?;
    let mut seen = [[false; MAX_ROOTS]; 3];
    for ordinal in 0..count {
        budget.charge_work(scan.checked_add(512).ok_or(Resource::Arithmetic)?)?;
        let limit = budget.storage_limit();
        let row = frame
            .root(ordinal as u32, limit, |work| budget.charge_work(work))
            .map_err(E::Framing)?;
        shared::root_row(
            ordinal,
            &row,
            inputs,
            output,
            descriptor.table(),
            &mut seen,
            budget,
        )
        .map_err(E::Join)?;
    }
    shared::symbols(output, native, descriptor.table(), budget).map_err(E::Join)
}
