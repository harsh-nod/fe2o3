fn execution_expansion_identity_v1(owner: &ProductionSemanticSsaOwnerV1) -> Option<[u8; 32]> {
    let expansion = owner.execution_expansion();
    expansion
        .roots()
        .iter()
        .any(SemanticExpandedRootV1::has_expanded_calls)
        .then(|| *expansion.identity())
}

fn execution_function_for_root_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
) -> Result<&SemanticFunctionDeclV1, ProductionSemanticKirErrorV1> {
    let source = owner.source_semantic();
    let view = owner
        .execution_view_for_root(root)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let function = checked_execution_function_v1(source, view)?;
    let plan = owner
        .execution_plan_for_root(root)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if plan.function() != view.source_body() || plan.function_identity() != function.identity() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(function)
}

fn checked_execution_function_v1<'a>(
    source: &AdmittedInertSemanticMirV1,
    view: &'a SemanticExpandedRootV1,
) -> Result<&'a SemanticFunctionDeclV1, ProductionSemanticKirErrorV1> {
    let root = view.root();
    let selection = source
        .select_kernel_body_for_root_v1(root)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let original = source
        .functions()
        .get(selection.body().index() as usize)
        .filter(|_| source.roots().binary_search(&root).is_ok())
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let function = view.body();
    // Only the existing exact transparent-wrapper selection may choose a non-root body.
    // Capability entries select the physical root and retain its binds and context issuance.
    if view.source_body() != selection.body()
        || view.instances().first().is_none_or(|instance| {
            instance.function() != selection.body()
                || instance.function_identity() != original.identity()
                || instance.parent().is_some()
        })
        || function.abi() != original.abi()
        || function.kernel_entry() != original.kernel_entry()
        || function.role() != original.role()
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    for (block_index, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        if matches!(
            source.callables().get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::Defined { .. })
        ) {
            return Err(unsupported(
                root.index(),
                Some(block_index as u32),
                None,
                "checked execution view retains an unexpanded defined call",
            ));
        }
    }
    Ok(function)
}

#[derive(Clone, Copy)]
struct SemanticKirExecutionInputV1<'a> {
    source: &'a AdmittedInertSemanticMirV1,
    function: &'a SemanticFunctionDeclV1,
    root: SemanticFunctionIdV1,
    source_body: SemanticFunctionIdV1,
    expansion_identity: Option<[u8; 32]>,
}

impl<'a> SemanticKirExecutionInputV1<'a> {
    fn for_root(
        owner: &'a ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Ok(Self {
            source: owner.source_semantic(),
            function: execution_function_for_root_v1(owner, root)?,
            root,
            source_body: owner
                .execution_view_for_root(root)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                .source_body(),
            expansion_identity: execution_expansion_identity_v1(owner),
        })
    }
}
