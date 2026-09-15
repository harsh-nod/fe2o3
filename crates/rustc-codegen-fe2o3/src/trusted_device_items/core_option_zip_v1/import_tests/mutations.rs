//! Typed counterexamples derived from the actual imported core body. These
//! changed bodies are negative planner inputs, not authenticated source claims.
use super::*;

fn replace(
    mir: &AdmittedInertSemanticMirV1,
    function_id: SemanticFunctionIdV1,
    block_index: usize,
    statements: Vec<SemanticStatementV1>,
) -> AdmittedInertSemanticMirV1 {
    let mut functions = mir.functions().to_vec();
    let function = &functions[function_id.index() as usize];
    assert!(function.kernel_entry().is_none());
    assert!(function.defined_capability_contract().is_none());
    let mut blocks = function.blocks().to_vec();
    let block = &blocks[block_index];
    blocks[block_index] = SemanticBasicBlockV1::new(
        block.identity(),
        block.source(),
        statements,
        block.terminator().clone(),
    )
    .unwrap();
    functions[function_id.index() as usize] = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap();
    InertSemanticMirRequestV1::new_with_callables(
        mir.target(),
        mir.types().to_vec(),
        mir.allocations().to_vec(),
        mir.statics().to_vec(),
        mir.vtables().to_vec(),
        functions,
        mir.callables().to_vec(),
        mir.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .expect("negative remains type/layout/ABI canonical; failure must be initializedness")
}

pub(super) fn check(
    mir: &AdmittedInertSemanticMirV1,
    function_id: SemanticFunctionIdV1,
    block: usize,
    statement: usize,
    place: &SemanticPlaceV1,
) {
    let function = &mir.functions()[function_id.index() as usize];
    let source = function.blocks()[block].statements()[statement].source();
    let enum_destination = function
        .locals()
        .iter()
        .enumerate()
        .find(|(index, local)| local.ty() == place.ty() && *index != place.local().index() as usize)
        .map(|(index, _)| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(index as u32),
                vec![],
                place.ty(),
            )
            .unwrap()
        })
        .expect("actual zip argument local has exactly this enum type");
    let original_bytes = mir.canonical_encoding().to_vec();
    for mutation in 0..3 {
        let kind = match mutation {
            0 => SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                enum_destination.clone(),
                SemanticRvalueV1::new(
                    place.ty(),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place.clone())),
                ),
            )),
            1 => SemanticStatementKindV1::Deinitialize(place.clone()),
            2 => SemanticStatementKindV1::SetDiscriminant {
                place: place.clone(),
                variant_index: 1,
            },
            _ => unreachable!(),
        };
        let mut statements = function.blocks()[block].statements().to_vec();
        // Preserve the original discriminant definition and all branch/drop flags.
        statements.insert(statement, SemanticStatementV1::new(source, kind));
        let changed = replace(mir, function_id, block, statements);
        let error = plan_semantic_function_ssa_with_module_v1(
            function_id,
            &changed.functions()[function_id.index() as usize],
            changed.types(),
            changed.callables(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap_err();
        // Deinitialize kills the enum; its following tag read is the rejected use.
        let expected_statement = statement + usize::from(mutation == 1);
        assert!(
            matches!(error, ProductionSemanticSsaErrorV1::PartialMove {
            function, block: found_block, statement: Some(found_statement), local,
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
        } if function == function_id && found_block as usize == block
            && found_statement as usize == expected_statement && local == place.local().index()),
            "counterexample {mutation}: {error:?}"
        );
        assert_eq!(changed.types(), mir.types());
        assert_eq!(changed.callables(), mir.callables());
        assert_eq!(
            changed.functions()[function_id.index() as usize].entry(),
            function.entry()
        );
    }
    assert_eq!(mir.canonical_encoding(), original_bytes);
}
