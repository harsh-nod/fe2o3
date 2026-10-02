pub(in super::super) fn native_helper_argument_owner(
    expanded_rust_call: Option<bool>,
) -> ProductionPreRankedKirOwnerV1 {
    let ordinary = expanded_rust_call.is_none();
    let original = argument_call_owner(
        expanded_rust_call.unwrap_or(false),
        ArgumentTupleShape::Mixed,
        false,
        false,
        true,
        ArgumentCallResult::Scalar,
        false,
    );
    let semantic = original.source_semantic();
    let mut functions = Vec::new();
    for (index, function) in semantic.functions().iter().enumerate() {
        let mut abi = function.abi().clone();
        let mut locals = function.locals().to_vec();
        let mut blocks = function.blocks().to_vec();
        if index == 0 {
            let first = &blocks[0];
            let statements = if ordinary {
                abi = SemanticFunctionAbiV1::from_rustc(
                    abi.identity(), semantic.target().identity(),
                    SemanticCanonAbiV1::Rust, SemanticExternAbiV1::Rust,
                    false, false, 1,
                    vec![semantic.functions()[1].abi().arguments()[1].clone()],
                    abi.return_value().clone(),
                ).unwrap().with_source_argument_ownership(vec![
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ]).unwrap();
                locals = vec![
                    locals[0].clone(),
                    SemanticLocalDeclV1::new(
                        locals[1].identity(), PAIR,
                        SemanticLocalRoleV1::Argument(0), function.source(),
                    ),
                ];
                let from = SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(1),
                    vec![SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Field(2), U32,
                    ).unwrap()],
                    U32,
                ).unwrap();
                vec![SemanticStatementV1::new(
                    first.source(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(0), vec![], U32,
                        ).unwrap(),
                        SemanticRvalueV1::new(
                            U32, SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(from)),
                        ),
                    )),
                )]
            } else {
                first.statements().to_vec()
            };
            blocks = vec![SemanticBasicBlockV1::new(
                first.identity(), first.source(), statements,
                SemanticTerminatorV1::new(first.source(), SemanticTerminatorKindV1::Return),
            ).unwrap()];
        } else if ordinary {
            for block in &mut blocks {
                let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                    continue;
                };
                let call = SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(2), vec![], PAIR,
                        ).unwrap(),
                    )],
                    call.destination().cloned(), call.unwind(),
                ).unwrap();
                *block = SemanticBasicBlockV1::new(
                    block.identity(), block.source(), block.statements().to_vec(),
                    SemanticTerminatorV1::new(block.source(), SemanticTerminatorKindV1::Call(call)),
                ).unwrap();
            }
        }
        let mut replacement = SemanticFunctionDeclV1::new(
            function.identity(), function.role(), function.item_definition_identity(),
            function.monomorphization_identity(), function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(), function.source(), abi, locals,
            function.entry(), blocks,
        ).unwrap();
        if let Some(entry) = function.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        functions.push(replacement);
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(), semantic.types().to_vec(), vec![], vec![], vec![],
        functions, semantic.callables().to_vec(), semantic.roots().to_vec(),
    ).unwrap().admit_current_production(SemanticMirLimitsV1::default()).unwrap();
    let source = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap();
    let roster = argument_launch_roster(&source);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 512 * 1024 * 1024);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        source, roster, ProductionSemanticKirLimitsV1::default(), &mut budget,
    ).unwrap()
}
