use super::*;

fn replace_blocks(
    function: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
    identity: SemanticFunctionIdentityV1,
) -> SemanticFunctionDeclV1 {
    let replacement = SemanticFunctionDeclV1::new(
        identity,
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
    match function.kernel_entry() {
        Some(entry) => replacement.with_kernel_entry(entry.clone()),
        None => replacement,
    }
}

fn rebuild(
    original: &ProductionSemanticSsaOwnerV1,
    functions: Vec<SemanticFunctionDeclV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let semantic = original.source_semantic();
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

pub(super) fn shared_outgoing() -> ProductionSemanticSsaOwnerV1 {
    let original = call_owner(false, ArgumentCallResult::Zero);
    let mut functions = original.source_semantic().functions().to_vec();
    let helper = &functions[0];
    let leaf = replace_blocks(
        helper,
        helper.blocks().to_vec(),
        SemanticFunctionIdentityV1::from_sha256([219; 32]),
    );
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let source = helper.source();
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(3),
        vec![
            SemanticOperandV1::Copy(place(1, ZERO)),
            SemanticOperandV1::Copy(place(2, TUPLE)),
        ],
        Some(SemanticCallDestinationV1::new(
            place(0, UNIT),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    functions[0] = replace_blocks(
        helper,
        vec![
            SemanticBasicBlockV1::new(
                helper.blocks()[0].identity(),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
            )
            .unwrap(),
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([220; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
        helper.identity(),
    );
    functions.push(leaf);
    rebuild(&original, functions)
}

pub(super) fn unrelated_statements(extra: usize) -> ProductionSemanticSsaOwnerV1 {
    let original = call_owner(false, ArgumentCallResult::Zero);
    let mut functions = original.source_semantic().functions().to_vec();
    let function = &functions[2];
    let mut blocks = function.blocks().to_vec();
    let block = &blocks[0];
    let mut statements = block.statements().to_vec();
    statements.extend(std::iter::repeat_n(statements[0].clone(), extra));
    blocks[0] = SemanticBasicBlockV1::new(
        block.identity(),
        block.source(),
        statements,
        block.terminator().clone(),
    )
    .unwrap();
    functions[2] = replace_blocks(function, blocks, function.identity());
    rebuild(&original, functions)
}
