/// The frozen V4 relation (also nested in V5) has no aggregate-result component
/// evidence. Preserve its scalar and ignored/ZST envelope until that relation
/// is versioned; this predicate alone authenticates no source or execution.
pub fn legacy_correspondence_result_supported_v4(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionAbiV1,
) -> bool {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticAbiPassModeV1, SemanticTypeShapeV1};
    let value = abi.return_value();
    if value.ty() != abi.source_output_type()
        || value.adjusted().is_some()
        || value.pointee_override().is_some()
    {
        return false;
    }
    let Some(declaration) = types.get(value.ty().index() as usize) else {
        return false;
    };
    match declaration.shape() {
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
            matches!(value.mode(), SemanticAbiPassModeV1::Direct(_))
        }
        _ => {
            declaration.layout().size_bytes() == Some(0)
                && matches!(value.mode(), SemanticAbiPassModeV1::Ignore)
        }
    }
}

/// Checks the whole retained source envelope, independently of a supplied proof
/// roster. Only exact kernel bodies may have special entry results, and only
/// their transparent root wrappers may call them with those results.
pub fn legacy_correspondence_source_results_supported_v4(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
) -> bool {
    legacy_source_function_envelope_v4(semantic, false, |function| {
        legacy_correspondence_result_supported_v4(semantic.types(), function.abi())
    })
}

fn legacy_source_function_envelope_v4(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    allow_roots: bool,
    supported: impl Fn(&fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1) -> bool,
) -> bool {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticCallableDeclV1, SemanticTerminatorKindV1};
    let unsupported = semantic
        .functions()
        .iter()
        .enumerate()
        .filter_map(|(index, function)| (!supported(function)).then_some(index))
        .collect::<BTreeSet<_>>();
    if unsupported.is_empty() {
        return true;
    }
    let mut bodies = BTreeSet::new();
    let mut wrappers = BTreeSet::new();
    for root in semantic.roots() {
        let Some(selected) = semantic.select_kernel_body_for_root_v1(*root) else {
            return false;
        };
        bodies.insert(selected.body().index() as usize);
        if allow_roots {
            bodies.insert(selected.root().index() as usize);
        }
        if selected.has_transparent_result_wrapper() {
            wrappers.insert((
                selected.root().index() as usize,
                selected.body().index() as usize,
            ));
        }
    }
    if !unsupported.is_subset(&bodies) {
        return false;
    }
    semantic
        .functions()
        .iter()
        .enumerate()
        .all(|(caller, function)| {
            function.blocks().iter().all(|block| {
                let callable = match block.terminator().kind() {
                    SemanticTerminatorKindV1::Call(call) => call.callee(),
                    SemanticTerminatorKindV1::TailCall(call) => call.callee(),
                    _ => return true,
                };
                let Some(SemanticCallableDeclV1::Defined { function: callee }) =
                    semantic.callables().get(callable.index() as usize)
                else {
                    return true;
                };
                let callee = callee.index() as usize;
                !unsupported.contains(&callee) || wrappers.contains(&(caller, callee))
            })
        })
}
