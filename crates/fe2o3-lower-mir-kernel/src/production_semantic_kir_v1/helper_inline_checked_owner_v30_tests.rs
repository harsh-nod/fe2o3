// Inert MIR34 fixtures through the existing checked-owner/ranked pipeline.
// These are exact owner/replay regressions, not live rustc source-custody evidence.
use super::*;
mod helper_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/helper_inline_singleton_semantic_fixture_v1.rs"
    ));
}
const TUPLE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn helper_semantic(kind: SemanticGfx942InlineInstructionV30) -> ProductionSemanticMirOwnerV1 {
    let direct = semantic(kind);
    let source = direct.semantic();
    let original_root = &source.functions()[0];
    // Admit the shared donor before deriving this distinct checked-owner fixture.
    let helper_owner = ProductionSemanticMirOwnerV1::try_new(
        helper_fixture::Fixture::new(kind).admit(),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let helper_donor = helper_owner.semantic();
    let original_helper = &helper_donor.functions()[1];
    let mut types = source.types().to_vec();
    // Keep the direct-root IDs (u32 at1, pointer at2) and append the tuple at3.
    // Its donor identity [3;32] sorts before the retained u32/pointer [68;32]
    // and [70;32], so this distinct inert fixture type needs a fresh identity.
    // Every layout, shape, ABI-property and nominal-kind fact is preserved.
    assert_eq!(types.len(), TUPLE.index() as usize);
    let tuple_donor = &helper_donor.types()[helper_fixture::SINGLETON.index() as usize];
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([72; 32]),
            SemanticLayoutIdentityV1::from_sha256([73; 32]),
            tuple_donor.layout().clone(),
            tuple_donor.shape().clone(),
        )
        .with_rustc_abi_properties(tuple_donor.abi_properties())
        .with_rust_type_kind(tuple_donor.rust_type_kind()),
    );
    assert!(
        types
            .windows(2)
            .all(|pair| pair[0].identity() < pair[1].identity())
    );
    let tuple_place =
        |local| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], TUPLE).unwrap();
    let component = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U32).unwrap()],
        U32,
    )
    .unwrap();
    let original_call = match original_root.blocks()[0].terminator().kind() {
        SemanticTerminatorKindV1::Call(call) => call,
        _ => panic!("original marker call"),
    };
    let root_call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        original_call.arguments().to_vec(),
        Some(SemanticCallDestinationV1::new(
            tuple_place(2),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let output = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap();
    let store = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            output,
            SemanticOperandV1::Copy(component),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    );
    let mut root_locals = original_root.locals().to_vec();
    root_locals[2] = SemanticLocalDeclV1::new(
        root_locals[2].identity(),
        TUPLE,
        SemanticLocalRoleV1::Temporary,
        root_locals[2].source(),
    );
    let root = helper_fixture::replace_body(
        original_root,
        root_locals,
        vec![
            helper_fixture::block(60, vec![], SemanticTerminatorKindV1::Call(root_call)),
            helper_fixture::block(61, vec![store], SemanticTerminatorKindV1::Return),
        ],
    );
    let abi = SemanticFunctionAbiV1::new(
        original_helper.abi().identity(),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![abi_value(U32); kind.input_count()],
        abi_value(TUPLE),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue;
        kind.input_count()
    ])
    .unwrap();
    let mut helper_locals = original_helper.locals().to_vec();
    helper_locals[0] = SemanticLocalDeclV1::new(
        helper_locals[0].identity(),
        TUPLE,
        SemanticLocalRoleV1::Return,
        helper_locals[0].source(),
    );
    // MOV has only one source argument; its unused second donor argument is not
    // part of this exact callable ABI. Keep marker result local3 stable.
    if kind.input_count() == 1 {
        helper_locals[2] = SemanticLocalDeclV1::new(
            helper_locals[2].identity(),
            U32,
            SemanticLocalRoleV1::Temporary,
            helper_locals[2].source(),
        );
    }
    let tuple_return = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            tuple_place(0),
            SemanticRvalueV1::new(
                TUPLE,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![SemanticOperandV1::Copy(helper_fixture::word(3))],
                    )
                    .unwrap(),
                ),
            ),
        )),
    );
    let helper = SemanticFunctionDeclV1::new(
        original_helper.identity(),
        SemanticFunctionRoleV1::InternalHelper,
        original_helper.item_definition_identity(),
        original_helper.monomorphization_identity(),
        original_helper.generic_type_arguments_identity(),
        original_helper.const_generic_arguments_identity(),
        original_helper.source(),
        abi,
        helper_locals,
        original_helper.entry(),
        vec![
            original_helper.blocks()[0].clone(),
            helper_fixture::block(62, vec![tuple_return], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            helper_donor.callables()[2].clone(),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v34(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}
fn checked_helper(
    kind: SemanticGfx942InlineInstructionV30,
) -> Result<ProductionSemanticKirOwnerV1, ProductionSemanticKirErrorV1> {
    let receipt =
        ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate(
            helper_semantic(kind),
            ranked(expression(kind, ProductionOverflowContractV2::Wrapping)),
            "inert helper checked-owner test projection".into(),
            vec![ProductionRankedAccessSourceV1::new(1, Some(0), 0, 0, 4)],
        )?;
    ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
        1,
    )
}

#[test]
fn singleton_helper_all_six_original_values_reach_checked_owner_and_explicit_replay() {
    for kind in helper_fixture::KINDS {
        let owner = checked_helper(kind).unwrap();
        let validation = owner.mir_pliron_translation_validation().unwrap();
        assert_eq!(validation.value_expressions(), 1);
        assert_eq!(validation.memory_effects(), 1);
        let functions = &owner.module().functions;
        assert_eq!(
            functions
                .iter()
                .filter(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
                .count(),
            1
        );
        let instructions = functions
            .iter()
            .flat_map(|function| function.body.iter())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::InlineAssembly(assembly) => Some(assembly),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].mnemonic, kind.mnemonic());
        owner.verify_equivalence().unwrap();
    }
}

#[test]
fn helper_explicit_replay_rejects_distinct_equal_valued_ssa_operand_after_byte_identity_repair() {
    let kind = SemanticGfx942InlineInstructionV30::VAddU32;
    let mut owner = checked_helper(kind).unwrap();
    let RetainedProductionKirModuleV1::Legacy(module) = &mut owner.module else {
        panic!("test requires mutable legacy checked-owner fixture");
    };
    let helper = module
        .functions
        .iter_mut()
        .find(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
        .unwrap();
    let parameters = helper.body.as_ref().unwrap().parameters.clone();
    assert_eq!(parameters.len(), 2);
    assert_ne!(parameters[0], parameters[1]);
    let assembly = helper
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::InlineAssembly(assembly) => Some(assembly),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        assembly.operands[1].kind,
        AssemblyOperandKind::Input(parameters[0])
    );
    assert_eq!(
        assembly.operands[2].kind,
        AssemblyOperandKind::Input(parameters[1])
    );
    assembly.operands[1].kind = AssemblyOperandKind::Input(parameters[1]);
    verify_module(module).unwrap();
    // Both helper actual arguments are u32::MAX. The scalar value relation alone
    // cannot distinguish this source-operand substitution.
    owner.canonical_kernel_ir = ProductionCanonicalKernelIrV1::from_module(module.clone()).unwrap();
    let checks = &owner.generic_checks[0];
    let values = validate_mir_pliron_translation_with_semantic_v1(
        Some(owner.semantic_ssa.source_semantic()),
        owner.module(),
        &owner.correspondence,
        &checks.function_name,
        &checks.lowering,
        &checks.access_sources,
        &checks.executable_effect_sources,
        owner.limits.max_operations,
    )
    .unwrap();
    assert_eq!(values, checks.translation_validation);
    // This explicit audit, unlike mere value equality or a repaired canonical
    // digest, reconstructs the original source operands and must reject.
    assert!(matches!(
        owner.verify_equivalence(),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}
