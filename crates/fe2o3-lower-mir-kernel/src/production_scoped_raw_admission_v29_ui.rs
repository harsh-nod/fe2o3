use super::*;

// These probes use the actual production signatures and private owner module.
// UI-BEGIN positive
#[allow(dead_code)]
fn positive_source<'root, 'source, 'view, 'arena, 'work>(
    instances: &'root ExecutionInstancesV29<'source>,
    plan: &'root SourceReferencePlanV29<'root, 'source>,
    root: &source_storage_v29::SourceStorageRootV29<'view, 'arena, 'source>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: OwnedScopedSourceSlotsV29,
    payload: PrivateArrayPayloadV1,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'work>,
) -> Result<(), source_storage_v29::SourceStorageRootCallbackErrorV29<'view>> {
    let output = scoped_raw_admission_v29::with_original_source_root_v29(
        instances,
        Some(plan),
        root,
        limits,
        budget,
        |preparation, budget| preparation.assemble(emitted, slots, None, payload, budget),
    )?;
    drop(output);
    Ok(())
}

#[allow(dead_code)]
fn positive_physical(
    correspondence: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    scoped_raw_admission_v29::with_checked_source_memory_v29(
        correspondence,
        root,
        None,
        budget,
        |_, _| Ok(()),
    )
}
// UI-END positive

// UI-BEGIN raw_graph
#[cfg(fe2o3_raw_admission_ui_case = "raw_graph")]
fn raw_graph(
    value: scoped_raw_admission_v29::PendingRawAssemblyV29,
) -> PendingScopedRootEmissionV29 {
    value.pending
}
// UI-END raw_graph

// UI-BEGIN unfinished_graph
#[cfg(fe2o3_raw_admission_ui_case = "unfinished_graph")]
fn unfinished_graph(
    value: scoped_raw_admission_v29::UnfinishedSourceRootV29,
) -> PendingScopedRootEmissionV29 {
    value.pending
}
// UI-END unfinished_graph

// UI-BEGIN checked_path
#[cfg(fe2o3_raw_admission_ui_case = "checked_path")]
fn checked_path(
    value: scoped_raw_admission_v29::PendingRawAssemblyV29,
) -> PendingScopedRootEmissionV29 {
    value
}
// UI-END checked_path

// UI-BEGIN physical_escape
#[cfg(fe2o3_raw_admission_ui_case = "physical_escape")]
fn physical_escape<'a>(
    correspondence: &'a ProductionSourceCorrespondenceV18<'a>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a scoped_raw_admission_v29::CheckedSourceMemoryV29<'a>> {
    scoped_raw_admission_v29::with_checked_source_memory_v29(
        correspondence,
        root,
        None,
        budget,
        |view, _| Ok(view),
    )
}
// UI-END physical_escape

// UI-BEGIN physical_capture
#[cfg(fe2o3_raw_admission_ui_case = "physical_capture")]
fn physical_capture(
    correspondence: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let mut saved = None;
    scoped_raw_admission_v29::with_checked_source_memory_v29(
        correspondence,
        root,
        None,
        budget,
        |view, _| -> SourceOwnedResultV18<()> {
            saved = Some(view);
            Ok(())
        },
    )?;
    std::mem::forget(saved);
    Ok(())
}
// UI-END physical_capture

// UI-BEGIN forged_completion
#[cfg(fe2o3_raw_admission_ui_case = "forged_completion")]
fn forged_completion<'a>(
    correspondence: &'a ProductionSourceCorrespondenceV18<'a>,
    pending: &'a scoped_raw_admission_v29::PendingSourceMemoryV29,
) -> scoped_raw_admission_v29::CheckedSourceMemoryV29<'a> {
    scoped_raw_admission_v29::CheckedSourceMemoryV29 {
        correspondence,
        root: 0,
        pending,
        required: 0,
    }
}
// UI-END forged_completion
