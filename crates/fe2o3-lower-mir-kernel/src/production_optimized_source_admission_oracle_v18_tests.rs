// Independent test-side census. Do not call Pliron's admission/profile helpers.

fn optimizer_fixture_type_nodes_v18(ty: &Type) -> usize {
    match ty {
        Type::Pointer(pointer) => 1 + optimizer_fixture_type_nodes_v18(&pointer.pointee),
        Type::Slice(slice) => 1 + optimizer_fixture_type_nodes_v18(&slice.element),
        Type::Vector(_) => 2,
        Type::Unit | Type::Scalar(_) | Type::StorageObject(_) | Type::Execution(_) => 1,
    }
}

fn optimized_source_fixture_bridge_work_v18(bytes: usize, module: &Module, live: bool) -> usize {
    let mut tree = 3;
    let mut slots = 0;
    let mut types = 0;
    let mut edges = 0;
    for function in &module.functions {
        types += function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
            .map(optimizer_fixture_type_nodes_v18)
            .sum::<usize>();
        let Some(body) = &function.body else { continue };
        tree += 3;
        slots += body.parameters.len();
        if live {
            types += function
                .signature
                .parameters
                .iter()
                .map(optimizer_fixture_type_nodes_v18)
                .sum::<usize>();
        }
        for block in &body.blocks {
            tree += 3 + 2 * block.operations.len();
            slots += block.parameters.len();
            types += block
                .parameters
                .iter()
                .map(|value| optimizer_fixture_type_nodes_v18(&value.ty))
                .sum::<usize>();
            for operation in &block.operations {
                slots += operation.results.len() + operation.kind.operand_count();
                types += operation
                    .results
                    .iter()
                    .map(|value| optimizer_fixture_type_nodes_v18(&value.ty))
                    .sum::<usize>();
            }
            let terminator = block.terminator.as_ref().unwrap();
            slots += terminator.operand_count();
            terminator
                .try_visit_edges_v1(|_, _| {
                    edges += 1;
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
        }
    }
    let structure = tree + slots + module.functions.len() + types + edges + 1;
    4 * structure * structure + 8 * bytes * structure + 8 * (bytes + structure)
}

fn optimized_source_fixture_precharge_v18(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
) -> (usize, usize) {
    let bytes = owner.canonical_bytes().len();
    let module = owner.module();
    let (mut blocks, mut operations, mut values, mut results, mut operands, mut successors) =
        (0, 0, 0, 0, 0, 0);
    let (mut declarations, mut conditionals, mut conditional_operands) = (0, 0, 0);
    let (mut max_definition, mut max_operands, mut max_successors) = (1, 0, 0);
    for function in &module.functions {
        let Some(body) = &function.body else {
            declarations += function.signature.parameters.len();
            continue;
        };
        values += body.parameters.len();
        for (index, block) in body.blocks.iter().enumerate() {
            blocks += 1;
            values += block.parameters.len();
            max_definition = max_definition
                .max(block.parameters.len() + if index == 0 { body.parameters.len() } else { 0 });
            for operation in &block.operations {
                operations += 1;
                values += operation.results.len();
                results += operation.results.len();
                operands += operation.kind.operand_count();
                max_definition = max_definition.max(operation.results.len());
                max_operands = max_operands.max(operation.kind.operand_count());
            }
            let terminator = block.terminator.as_ref().unwrap();
            operations += 1;
            operands += terminator.operand_count();
            max_operands = max_operands.max(terminator.operand_count());
            let mut outgoing = 0;
            terminator
                .try_visit_edges_v1(|_, _| {
                    outgoing += 1;
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
            successors += outgoing;
            max_successors = max_successors.max(outgoing);
            if matches!(
                terminator,
                fe2o3_kernel_ir::Terminator::ConditionalBranch { .. }
                    | fe2o3_kernel_ir::Terminator::Switch { .. }
                    | fe2o3_kernel_ir::Terminator::IntegerSwitch { .. }
            ) {
                conditionals += 1;
                conditional_operands += terminator.operand_count();
            }
        }
    }
    let nodes = (module.functions.len()
        + 2 * blocks
        + operations
        + 5 * values
        + results
        + 2 * operands
        + 2 * successors
        + declarations
        + 3 * conditionals
        + 2 * conditional_operands)
        .max(1);
    assert!(
        bytes <= 16_384,
        "optimizer fixture canonical bytes: {bytes}"
    );
    // N is still checked against the exact execution frame. The old 1024
    // shortcut followed from 320*N^2; the unchanged work bounds below now
    // constrain the actual arity-sensitive production equation instead.
    let live_operations = operations + values + conditionals;
    let live_results = results + values;
    let live_operands = operands + conditional_operands;
    let live_successors = successors + conditionals;
    let validation =
        4 * live_operands + 2 * live_results + (max_successors.max(1) + 4) * live_successors;
    let event = [
        32 + 2 * max_definition + 3 * live_operands + validation,
        32 + 2 * max_definition
            + 3 * live_successors
            + validation
            + 3 * live_successors * max_operands,
        32 + validation + live_operands + live_successors + live_results,
        32 + 8 * max_operands + 8 * max_definition,
        32 + 3 * live_operations
            + validation
            + live_operands
            + live_successors
            + 2 * max_definition,
    ]
    .into_iter()
    .max()
    .unwrap();
    let census = 4 * nodes + blocks + live_operations + 2 * values + validation;
    let occurrence = 64 * nodes + 12 * census + 10 * nodes * event;
    let old_capture = nodes.min(2 * bytes + 64).min(131_072);
    let capture = old_capture.min((operations + 3 * values + conditionals).max(1));
    let logarithm = (usize::BITS - capture.leading_zeros()) as usize;
    let map_work = 16
        * (10 * old_capture * (max_definition + 1)
            + capture * (8 * (max_definition + logarithm + 4) + 4));
    let prepaid = occurrence
        + 25_268_224
        + 192 * (bytes + 32_769)
        + map_work
        + optimized_source_fixture_bridge_work_v18(bytes, module, false);
    assert!(
        prepaid <= OPTIMIZED_SOURCE_PRECHARGE_BOUND_V18,
        "structural observer/bridge precharge: {prepaid}, nodes={nodes}, bytes={bytes}"
    );
    (prepaid, nodes)
}
