// Source-only construction shared with KIR; the caller supplies an already
// checked entry plan or performs the original KIR entry validation.
fn checked_root_context_source_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    input: &ProductionKernelContextLoweringInputV1,
    entry_plan: impl FnOnce(
        SemanticTypeIdV1,
    )
        -> Result<Option<KernelContextEntryPlanV1>, ProductionSemanticKirErrorV1>,
) -> Result<RootKernelContextLoweringV1, ProductionSemanticKirErrorV1> {
    let selected_root = input.selected_root;
    let semantic = owner.source_semantic();
    let view = owner
        .execution_view_for_root(selected_root)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let root = semantic
        .functions()
        .get(selected_root.index() as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if !input.is_complete()
        || !semantic.roots().contains(&selected_root)
        || root.role() != SemanticFunctionRoleV1::KernelRoot
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let entry = root
        .kernel_entry()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let symbol = std::str::from_utf8(entry.export_symbol().as_bytes())
        .map_err(|_| unsupported(0, None, None, "kernel export symbol is not UTF-8"))?;
    let issuances = semantic_kernel_context_issuances_v1(semantic)?;
    let Some([semantic_type]) = issuances.get(&selected_root).map(Vec::as_slice) else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    Ok(RootKernelContextLoweringV1 {
        selected_root,
        semantic_type: *semantic_type,
        entry_transfer: entry_plan(*semantic_type)?,
        context_type: KernelContextTypeV1::new(
            symbol,
            input.kernel_marker_identity,
            input.target_brand_identity,
            input.launch_brand_identity,
        ),
        source: KernelContextSourceIdentityV1::new(
            input.frontend_unit_identity,
            *semantic.functions()[view.source_body().index() as usize]
                .identity()
                .as_bytes(),
            *entry.kernel_binding_identity().as_bytes(),
            input.issuance_identity,
        ),
    })
}
