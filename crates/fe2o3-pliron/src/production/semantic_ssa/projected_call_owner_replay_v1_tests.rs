use super::*;

#[path = "projected_call_capture_v1_tests.rs"]
mod projected_call_capture_v1_tests;

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBackendPrimitiveV1, SemanticBackendScalarV1, SemanticFieldsShapeV1,
    SemanticRustcVariantsV1, SemanticScalarTypeV1, SemanticScalarValidityRangeV1,
};

fn admitted_projected_call_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = admitted_single_function_semantic();
    let original = &base.functions()[0];
    let unit = SemanticTypeIdV1::from_index(0);
    let integer = SemanticTypeIdV1::from_index(1);
    let array = SemanticTypeIdV1::from_index(2);
    let mut types = base.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(160)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(161)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(162)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(163)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::array(0, 8),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: unit,
            length: 8,
        },
    ));
    let destination = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                unit,
            )
            .unwrap(),
        ],
        unit,
    )
    .unwrap();
    let number = SemanticConstantV1::new(
        integer,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
    );
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        vec![
            test_local(170, unit.index(), SemanticLocalRoleV1::Return),
            test_local(171, array.index(), SemanticLocalRoleV1::Temporary),
            test_local(172, integer.index(), SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            test_block(
                173,
                vec![test_assign_to(
                    test_typed_place(2, integer.index()),
                    SemanticOperandV1::Constant(number),
                )],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            test_block(
                174,
                vec![],
                test_call(
                    1,
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        destination,
                        test_edge(SemanticEdgeRoleV1::CallReturn, 2),
                    )),
                ),
            ),
            test_block(175, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(test_bytes(210)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(211)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(test_bytes(220)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(test_bytes(221)),
        SemanticMonomorphizationIdentityV1::from_sha256(test_bytes(222)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(test_bytes(223)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(test_bytes(224)),
        original.source(),
        helper_abi,
        vec![test_local(225, unit.index(), SemanticLocalRoleV1::Return)],
        SemanticBlockIdV1::from_index(0),
        vec![test_block(226, vec![], SemanticTerminatorKindV1::Return)],
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        base.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default()).unwrap()
}

#[test]
fn admitted_source_owner_replay_rejects_the_old_address_omission_with_source_unchanged() {
    let mut owner = admitted_projected_call_owner();
    owner.verify_replay().unwrap();
    assert_eq!(owner.plans.len(), 2);
    assert_eq!(owner.summary.input_events(), 2);
    assert_eq!(
        owner.plans[0].plan.resolved_events(SsaBlockIdV1::new(1)),
        Some(
            [(
                0,
                SsaResolvedEventV1::Use {
                    variable: variable(2),
                    value: SsaValueV1::Definition(SsaDefinitionIdV1::new(0))
                },
            )]
            .as_slice()
        )
    );
    let identity = owner.identity;
    let source_allocation = owner.source_semantic().functions().as_ptr();
    let module = owner.source_semantic();
    let function = &module.functions()[0];
    let transparent = transparent_borrow_sites_v1(function, module.callables());
    let (actual_input, _, _) = semantic_function_ssa_input_v1(
        function,
        Some(module.types()),
        module.callables(),
        &transparent,
    );
    let mut old_blocks = actual_input.blocks().to_vec();
    old_blocks[1] = SsaBlockInputV1::new(vec![], old_blocks[1].edges().to_vec());
    let old_input = SsaConstructionInputV1::new(
        actual_input.entry(),
        actual_input.variable_count(),
        actual_input.promotable().to_vec(),
        actual_input.entry_definitions().to_vec(),
        old_blocks,
    );
    owner.plans[0].plan =
        plan_ssa_with_limits_v1(&old_input, SsaPlannerLimitsV1::default()).unwrap();
    assert_eq!(owner.identity, identity);
    assert_eq!(
        owner.source_semantic().functions().as_ptr(),
        source_allocation
    );
    assert_eq!(
        owner.verify_replay(),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
}
