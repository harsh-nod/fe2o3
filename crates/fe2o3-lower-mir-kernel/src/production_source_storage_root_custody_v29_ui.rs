// These controls compile the real private entry point, never a model signature.
// Each negative block must fail only at its own production-signature boundary.

// UI-BEGIN positive
#[cfg(fe2o3_source_storage_root_custody_ui_case = "positive")]
fn positive<'source>(
    layouts: &mut SourceStorageLayoutsV29<'source>,
    instances: &ExecutionInstancesV29<'source>,
    budget: &mut Budget<'_>,
) -> Result<bool, Error> {
    with_source_storage_root_v29(layouts, instances, budget, |plan, root, budget| {
        let state = root.new_state(instances.root(), SemanticLocalIdV1::from_index(0), budget)?;
        let path = root.root_path(SemanticTypeIdV1::from_index(0), budget)?;
        let copy = root.copy_state(state, budget)?;
        let answer = root.is_initialized(copy, path, budget)?;
        plan.check_owner(instances, budget)?;
        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
        emission.abort_scope(instances, budget)?;
        std::mem::forget(root);
        Ok(answer)
    })
}
// UI-END positive

// UI-BEGIN state
#[cfg(fe2o3_source_storage_root_custody_ui_case = "state")]
fn escape_state<'a, 'source>(
    layouts: &'a mut SourceStorageLayoutsV29<'source>,
    instances: &'a ExecutionInstancesV29<'source>,
    budget: &mut Budget<'_>,
) -> Result<SourceStorageStateHandleV29<'a>, Error> {
    with_source_storage_root_v29(layouts, instances, budget, |_, root, budget| {
        Ok(root.new_state(instances.root(), SemanticLocalIdV1::from_index(0), budget)?)
    })
}
// UI-END state

// UI-BEGIN path
#[cfg(fe2o3_source_storage_root_custody_ui_case = "path")]
fn escape_path<'a, 'source>(
    layouts: &'a mut SourceStorageLayoutsV29<'source>,
    instances: &'a ExecutionInstancesV29<'source>,
    budget: &mut Budget<'_>,
) -> Result<SourceStoragePathHandleV29<'a>, Error> {
    with_source_storage_root_v29(layouts, instances, budget, |_, root, budget| {
        Ok(root.root_path(SemanticTypeIdV1::from_index(0), budget)?)
    })
}
// UI-END path

// UI-BEGIN origins
#[cfg(fe2o3_source_storage_root_custody_ui_case = "origins")]
fn escape_origins<'a, 'source>(
    layouts: &'a mut SourceStorageLayoutsV29<'source>,
    instances: &'a ExecutionInstancesV29<'source>,
    budget: &mut Budget<'_>,
) -> Result<SourceStorageOriginsHandleV29<'a>, Error> {
    with_source_storage_root_v29(layouts, instances, budget, |plan, root, budget| {
        Ok(root.capture_origin(plan, 0, budget)?)
    })
}
// UI-END origins

// UI-BEGIN view
#[cfg(fe2o3_source_storage_root_custody_ui_case = "view")]
fn escape_view<'a, 'source>(
    layouts: &'a mut SourceStorageLayoutsV29<'source>,
    instances: &'a ExecutionInstancesV29<'source>,
    budget: &mut Budget<'_>,
) -> Result<SourceStorageRootV29<'a, 'a, 'source>, Error> {
    with_source_storage_root_v29(layouts, instances, budget, |_, root, _| Ok(root))
}
// UI-END view

// UI-BEGIN owned
#[cfg(fe2o3_source_storage_root_custody_ui_case = "owned")]
fn escape_owned<'source>(
    layouts: &mut SourceStorageLayoutsV29<'source>,
    instances: &ExecutionInstancesV29<'source>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    with_source_storage_root_v29(layouts, instances, budget, |_, root, budget| {
        let handle = root.new_state(instances.root(), SemanticLocalIdV1::from_index(0), budget)?;
        let owned: SourceStorageStateV29<'_, '_> = root.copy_state(handle, budget)?;
        std::mem::forget(owned);
        Ok(())
    })
}
// UI-END owned

// UI-BEGIN capture
#[cfg(fe2o3_source_storage_root_custody_ui_case = "capture")]
fn capture_handle<'source>(
    layouts: &mut SourceStorageLayoutsV29<'source>,
    instances: &ExecutionInstancesV29<'source>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let mut escaped = None;
    with_source_storage_root_v29(layouts, instances, budget, |_, root, budget| {
        escaped =
            Some(root.new_state(instances.root(), SemanticLocalIdV1::from_index(0), budget)?);
        Ok(())
    })?;
    std::mem::forget(escaped);
    Ok(())
}
// UI-END capture

// UI-BEGIN install
#[cfg(fe2o3_source_storage_root_custody_ui_case = "install")]
fn install_while_borrowed<'source>(
    mut layouts: SourceStorageLayoutsV29<'source>,
    instances: &ExecutionInstancesV29<'source>,
    candidate: &mut Module,
    budget: &mut Budget<'_>,
) -> Result<usize, Error> {
    with_source_storage_root_v29(&mut layouts, instances, budget, |_, _, budget| {
        Ok(layouts.install_rows(instances.owner(), candidate, budget)?)
    })
}
// UI-END install

// UI-BEGIN plan
#[cfg(fe2o3_source_storage_root_custody_ui_case = "plan")]
fn escape_plan<'a, 'source>(
    layouts: &'a mut SourceStorageLayoutsV29<'source>,
    instances: &'a ExecutionInstancesV29<'source>,
    budget: &mut Budget<'_>,
) -> Result<&'a SourceReferencePlanV29<'a, 'source>, Error> {
    with_source_storage_root_v29(layouts, instances, budget, |plan, _, _| Ok(plan))
}
// UI-END plan

// UI-BEGIN custody
#[cfg(fe2o3_source_storage_root_custody_ui_case = "custody")]
fn escape_custody<'a, 'source>(
    layouts: &'a mut SourceStorageLayoutsV29<'source>,
    instances: &'a ExecutionInstancesV29<'source>,
    budget: &mut Budget<'_>,
) -> Result<&'a SourceStorageRootCustodyViewV29<'a, 'source>, Error> {
    with_source_storage_root_v29(layouts, instances, budget, |plan, _, _| {
        Ok(plan.storage_root.as_ref().unwrap())
    })
}
// UI-END custody

// UI-BEGIN recorded
#[cfg(fe2o3_source_storage_root_custody_ui_case = "recorded")]
fn escape_recorded<'a, 'source>(
    layouts: &'a mut SourceStorageLayoutsV29<'source>,
    instances: &'a ExecutionInstancesV29<'source>,
    budget: &mut Budget<'_>,
) -> Result<RecordedStorageFailureV29<'a>, Error> {
    with_source_storage_root_v29(layouts, instances, budget, |_, root, budget| {
        Ok(root
            .new_state(
                instances.root(),
                SemanticLocalIdV1::from_index(u32::MAX),
                budget,
            )
            .unwrap_err())
    })
}
// UI-END recorded
