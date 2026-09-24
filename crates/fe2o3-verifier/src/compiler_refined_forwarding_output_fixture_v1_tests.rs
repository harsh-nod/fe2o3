use crate as fe2o3_verifier;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/support/compiler_proof_inputs_v3.rs"
));
pub(super) fn plain_source(seed: u8) -> ProductionSemanticMirOwnerV1 {
    semantic_owner(seed)
}
pub(super) fn two_roots() -> ProductionSemanticMirOwnerV1 {
    let first = semantic_owner(32);
    let second = semantic_owner(34);
    let helper = semantic_owner(33);
    let first_function = &first.semantic().functions()[0];
    let unit = SemanticTypeIdV1::from_index(0);
    let mut locals = first_function.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(100, 32)),
        unit,
        SemanticLocalRoleV1::Temporary,
        first_function.source(),
    ));
    let mut blocks = first_function.blocks().to_vec();
    blocks.push(block(
        12,
        32,
        vec![],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], unit).unwrap(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        first_function.entry(),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    ));
    let first_function = SemanticFunctionDeclV1::new(
        first_function.identity(),
        first_function.role(),
        first_function.item_definition_identity(),
        first_function.monomorphization_identity(),
        first_function.generic_type_arguments_identity(),
        first_function.const_generic_arguments_identity(),
        first_function.source(),
        first_function.abi().clone(),
        locals,
        SemanticBlockIdV1::from_index(2),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(first_function.kernel_entry().unwrap().clone());
    // Function identity order and descriptor binding order are independent.
    let second_function = &second.semantic().functions()[0];
    let second_entry = second_function.kernel_entry().unwrap();
    let second_function = second_function
        .clone()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            second_entry.export_symbol().clone(),
            SemanticKernelBindingIdentityV1::from_sha256(bytes(5, 31)),
            second_entry.source_contract(),
        ));
    let old = &helper.semantic().functions()[0];
    let abi = SemanticFunctionAbiV1::from_rustc(
        old.abi().identity(),
        old.abi().layout_identity(),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(
            old.abi().source_output_type(),
            SemanticAbiPassModeV1::Ignore,
        ),
    )
    .unwrap();
    let helper = SemanticFunctionDeclV1::new(
        old.identity(),
        SemanticFunctionRoleV1::InternalHelper,
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        abi,
        old.locals().to_vec(),
        old.entry(),
        old.blocks().to_vec(),
    )
    .unwrap();
    let request = InertSemanticMirRequestV1::new(
        first.semantic().target(),
        first.semantic().types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![first_function, helper, second_function],
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(2),
        ],
    )
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(
        request
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap()
}
