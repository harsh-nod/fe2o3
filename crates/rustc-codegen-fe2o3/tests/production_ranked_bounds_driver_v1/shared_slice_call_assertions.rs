fn assert_shared_slice_call_chain_v1(module: &fe2o3_kernel_ir::Module) {
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, FunctionRole, OperationKind, ScalarType, Type,
    };
    let slice = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let mut reachable = std::collections::BTreeSet::new();
    let mut pending = module
        .functions
        .iter()
        .filter(|function| function.role == FunctionRole::KernelEntry)
        .map(|function| function.id.clone())
        .collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if !reachable.insert(id.clone()) {
            continue;
        }
        let function = module.function(&id).unwrap();
        for operation in function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
        {
            if let OperationKind::Call { callee, .. } = &operation.kind {
                pending.push(callee.clone());
            }
        }
    }
    let metadata_helper = module.functions.iter().find(|function| {
        reachable.contains(&function.id)
            && function.role == FunctionRole::InternalHelper
            && function.signature.parameters == [slice.clone()]
            && function.body.as_ref().is_some_and(|body| {
                body.blocks.iter().flat_map(|block| &block.operations).any(|operation| {
                    matches!(operation.kind, OperationKind::SliceLength { slice } if body.parameters == [slice])
                })
            })
    }).expect("retained metadata helper has one slice parameter and no ZST parameter");
    let closure = module.functions.iter().find(|function| {
        reachable.contains(&function.id)
            && function.role == FunctionRole::InternalHelper
            && function.signature.parameters == [slice.clone()]
            && function.body.as_ref().is_some_and(|body| {
                body.blocks.iter().flat_map(|block| &block.operations).any(|operation| {
                    matches!(&operation.kind, OperationKind::Call { callee, arguments } if callee == &metadata_helper.id && arguments.len() == 1)
                })
            })
    });
    assert!(
        closure.is_some(),
        "kernel-reachable owned closure retains its slice helper call"
    );
}

fn assert_shared_slice_source_v1(architecture: &str, target: &ScratchTarget) -> std::path::PathBuf {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let path = target
        .path()
        .join(format!("slice-source-{architecture}.fe2sim"));
    let exported = output(
        simulation_export_command_for_feature(
            architecture,
            &path,
            &target.path().join(architecture),
            Some(5),
            "rust_call",
        ),
        "export source slice capture correspondence",
    );
    assert!(exported.status.success(), "{}", exported.stderr);
    let bundle = fe2o3_kernel_ir::VerifiedSimulationBundleV5::from_canonical_bytes(
        std::fs::read(&path).unwrap(),
    )
    .unwrap();
    let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        bundle.semantic_mir(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let mut found = false;
    for function in semantic.functions() {
        if function.abi().extern_abi() != SemanticExternAbiV1::RustCall {
            continue;
        }
        let mut pending = vec![function.abi().source_input_types()[0]];
        let (mut slices, mut zeros, mut visits) = (0, 0, 0);
        while let Some(ty) = pending.pop() {
            visits += 1;
            assert!(visits < 1024, "bounded source capture shape");
            let declaration = &semantic.types()[ty.index() as usize];
            zeros += usize::from(declaration.layout().size_bytes() == Some(0));
            match declaration.shape() {
                SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields) => {
                    pending.extend(fields.fields())
                }
                SemanticTypeShapeV1::Pointer(pointer)
                    if pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                        && pointer.metadata() == SemanticPointerMetadataV1::SliceLength =>
                {
                    slices += 1;
                }
                _ => {}
            }
        }
        if slices == 1 && zeros >= 3 {
            assert_eq!(
                function.abi().source_argument_ownership()[0],
                SemanticSourceArgumentOwnershipV1::ByValue
            );
            found = true;
        }
    }
    assert!(
        found,
        "actual owned RustCall receiver retains nested slice and ZST capture shape"
    );

    let rejected = target
        .path()
        .join(format!("slice-read-{architecture}.fe2sim"));
    let exported = output(
        simulation_export_command_for_feature(
            architecture,
            &rejected,
            &target.path().join(architecture),
            None,
            "rust_call_slice_read",
        ),
        "refuse retained slice helper memory effects",
    );
    assert!(
        !exported.status.success(),
        "slice-reading helper must remain refused"
    );
    assert!(
        exported.stderr.contains(
            "reachable deterministic scalar helper is not interprocedurally complete and pure"
        ),
        "{}",
        exported.stderr
    );
    assert!(
        exported
            .stderr
            .contains("helper declaration at Rust source"),
        "{}",
        exported.stderr
    );
    assert!(!rejected.exists());
    path
}
