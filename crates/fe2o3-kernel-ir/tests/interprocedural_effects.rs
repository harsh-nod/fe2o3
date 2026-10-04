use fe2o3_kernel_ir::*;

fn empty_function(name: &str, operations: Vec<Operation>) -> Function {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::definition(name, Signature::new(vec![], vec![]), vec![], vec![block])
}

fn call(callee: &str, arguments: Vec<ValueId>) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new(callee),
            arguments,
        },
    )
}

fn module(functions: Vec<Function>) -> Module {
    let mut module = Module::new("interprocedural_effects");
    module.functions = functions;
    module
}

#[test]
fn acyclic_pure_helper_chain_is_complete() {
    let leaf = empty_function("leaf", vec![]);
    let middle = empty_function("middle", vec![call("leaf", vec![])]);
    let root = empty_function("root", vec![call("middle", vec![])]);
    let analysis = analyze_interprocedural_effects_v1(&module(vec![root, middle, leaf])).unwrap();

    for function in ["root", "middle", "leaf"] {
        let summary = analysis.function(&FunctionId::new(function)).unwrap();
        assert!(summary.is_complete_and_pure());
        assert!(summary.incomplete_reasons().is_empty());
    }
}

#[test]
fn memory_effect_mutation_propagates_to_every_caller() {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let value = Type::Scalar(ScalarType::U32);
    let store = Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![store];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let leaf = Function::definition(
        "leaf",
        Signature::new(vec![pointer.clone(), value.clone()], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let mut caller_block = BasicBlock::new(BlockId(0));
    caller_block.operations = vec![call("leaf", vec![ValueId(0), ValueId(1)])];
    caller_block.terminator = Some(Terminator::Return { values: vec![] });
    let caller = Function::definition(
        "caller",
        Signature::new(vec![pointer, value], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![caller_block],
    );
    let analysis = analyze_interprocedural_effects_v1(&module(vec![caller, leaf])).unwrap();

    for function in ["caller", "leaf"] {
        let summary = analysis.function(&FunctionId::new(function)).unwrap();
        assert!(summary.is_complete());
        assert!(summary.summary().writes(AddressSpace::Global));
        assert!(!summary.is_complete_and_pure());
    }
}

#[test]
fn recursive_call_graph_is_explicitly_incomplete() {
    let recursive = empty_function("recursive", vec![call("recursive", vec![])]);
    let analysis = analyze_interprocedural_effects_v1(&module(vec![recursive])).unwrap();
    let summary = analysis.function(&FunctionId::new("recursive")).unwrap();
    assert!(matches!(
        summary.incomplete_reasons(),
        [InterproceduralEffectIncompleteReasonV1::RecursiveCallCycle { function }]
            if function == &FunctionId::new("recursive")
    ));
}

#[test]
fn external_declaration_never_self_certifies_a_summary() {
    let root = empty_function("root", vec![call("external", vec![])]);
    let external = Function::declaration("external", Signature::new(vec![], vec![]));
    let analysis = analyze_interprocedural_effects_v1(&module(vec![root, external])).unwrap();
    let summary = analysis.function(&FunctionId::new("root")).unwrap();
    assert!(matches!(
        summary.incomplete_reasons(),
        [InterproceduralEffectIncompleteReasonV1::FunctionDeclaration { function }]
            if function == &FunctionId::new("external")
    ));
}
