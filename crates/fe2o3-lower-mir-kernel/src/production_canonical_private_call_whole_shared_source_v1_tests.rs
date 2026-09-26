// Root-qualified source census used by final custody. This is not a substitute
// for the still-separate typed argument/source continuation debit schedule.
use super::consume::vector;
use fe2o3_kernel_ir::OperationKind as Kind;

pub(super) fn call_effects(module: &fe2o3_kernel_ir::Module, trace: &mut Trace) -> usize {
    use fe2o3_kernel_analysis::{
        CanonicalKirCallEffectDecisionV1 as Decision, CanonicalKirCallEffectsV1 as Report,
    };
    type Frame = (
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        usize,
        usize,
        Decision,
    );
    let scope = trace.enter(&[]);
    let count = module.functions.len();
    trace.reserve(size_of::<Report<'_, '_>>());
    trace.work(count);
    let states = exact_capacity::<u8>(count);
    trace.reserve(states);
    trace.work(count);
    trace.reserve(exact_capacity::<Frame>(count));
    let operations: Vec<Vec<_>> = module
        .functions
        .iter()
        .map(|f| {
            f.body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|b| &b.operations)
                .collect()
        })
        .collect();
    let mut seen = vec![false; count];
    for root in 0..count {
        trace.work(1);
        if seen[root] {
            continue;
        }
        seen[root] = true;
        let mut pending = vec![(root, 0)];
        while let Some((function, next)) = pending.last_mut() {
            trace.work(1);
            let Some(operation) = operations[*function].get(*next) else {
                pending.pop();
                continue;
            };
            *next += 1;
            match &operation.kind {
                Kind::Call { callee, .. } => {
                    assert!(!callee.as_str().starts_with("__fe2o3_ir_"));
                    trace.work(1);
                    trace.work(product(2, callee.as_str().len() + 1));
                    let target = module
                        .functions
                        .iter()
                        .position(|f| f.id == *callee)
                        .unwrap();
                    if !seen[target] {
                        seen[target] = true;
                        pending.push((target, 0));
                    }
                }
                Kind::Constant(_)
                | Kind::Alloca { count: None, .. }
                | Kind::Load { .. }
                | Kind::Store { .. } => {}
                _ => panic!("closed call-effect source profile"),
            }
        }
    }
    trace.leave(scope);
    sum(&[size_of::<Report<'_, '_>>(), states])
}

// The typed fixture has three scalar ABIs, two ordinary callers and one shared
// deterministic scalar leaf. No production summary, decision or observation is
// read while deriving this schedule.
pub(super) fn typed_callable(owner: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) {
    use fe2o3_mir_model::{SemanticDefinedCallableSummariesV1, semantic_mir_v1::*};
    type Direct = (bool, bool, Vec<usize>);
    let semantic = owner.semantic_ssa().source_semantic();
    let functions = semantic.functions();
    assert_eq!(functions.len(), 3);
    let mut callees = vec![Vec::new(); functions.len()];
    trace.reserve(size_of::<SemanticDefinedCallableSummariesV1<'_>>());
    trace.work(0);
    vector::<Direct>(functions.len(), trace);
    for (ordinal, function) in functions.iter().enumerate() {
        let abi = function.abi();
        assert_eq!(
            (abi.arguments().len(), abi.source_input_types().len()),
            (1, 1)
        );
        assert!(abi.hidden_arguments().is_empty() && !abi.c_variadic());
        assert!(matches!(
            semantic.types()[abi.arguments()[0].value().source_ty().index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(_)
        ));
        trace.work(1 + function.locals().len());
        trace.work(2);
        trace.work(1); // The scalar input is not a transparent aggregate carrier.
        for local in function.locals() {
            assert!(matches!(
                semantic.types()[local.ty().index() as usize].shape(),
                SemanticTypeShapeV1::Unit | SemanticTypeShapeV1::Scalar(_)
            ));
            trace.work(1);
        }
        units(trace, 2); // Zero-sized ABI rejects its first nonzero argument.
        trace.work(semantic.types().len());
        vector::<u8>(semantic.types().len(), trace);
        trace.work(1);
        trace.work(function.blocks().len());
        for block in function.blocks() {
            trace.work(1);
            for statement in block.statements() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    panic!("typed helper copy assignment")
                };
                assert!(assignment.destination().projections().is_empty());
                let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) =
                    assignment.value().kind()
                else {
                    panic!("typed helper direct scalar copy")
                };
                assert!(place.projections().is_empty());
                units(trace, 5); // Statement, destination, rvalue, operand, place.
            }
            trace.work(1);
            match block.terminator().kind() {
                SemanticTerminatorKindV1::Return => {}
                SemanticTerminatorKindV1::Call(call) => {
                    assert_eq!(call.arguments().len(), 1);
                    assert!(call.variadic_argument_abis().is_empty());
                    let SemanticOperandV1::Copy(place) = &call.arguments()[0] else {
                        panic!("typed call direct scalar copy")
                    };
                    assert!(place.projections().is_empty());
                    assert!(call.destination().unwrap().place().projections().is_empty());
                    let SemanticCallableDeclV1::Defined { function: callee } =
                        &semantic.callables()[call.callee().index() as usize]
                    else {
                        panic!("typed defined call")
                    };
                    trace.work(2);
                    units(trace, 3); // Operand, place, destination.
                    trace.work(callees[ordinal].len());
                    vector::<usize>(1, trace);
                    callees[ordinal].push(callee.index() as usize);
                }
                _ => panic!("typed callable terminator"),
            }
        }
        assert!(callees[ordinal].len() <= 1);
    }
    trace.work(0);
    vector::<Vec<usize>>(functions.len(), trace);
    trace.work(functions.len());
    let mut callers = vec![Vec::new(); functions.len()];
    for (caller, targets) in callees.iter().enumerate() {
        trace.work(1 + targets.len());
        for &callee in targets {
            trace.work(callers[callee].len());
            vector::<usize>(1, trace);
            callers[callee].push(caller);
        }
    }
    trace.work(functions.len());
    vector::<u8>(functions.len(), trace);
    vector::<usize>(functions.len(), trace);
    trace.work(functions.len());
    trace.work(functions.len());
    vector::<bool>(functions.len(), trace);
    let mut queue = std::collections::VecDeque::<usize>::new();
    queue.try_reserve_exact(functions.len()).unwrap();
    assert_eq!(
        queue.capacity(),
        functions.len(),
        "pinned exact queue-capacity premise"
    );
    vector::<usize>(queue.capacity(), trace);
    let mut remaining: Vec<_> = callees.iter().map(Vec::len).collect();
    for (function, count) in remaining.iter().enumerate() {
        trace.work(1);
        if *count == 0 {
            queue.push_back(function);
        }
    }
    let mut complete = 0;
    while let Some(callee) = queue.pop_front() {
        complete += 1;
        trace.work(1 + callers[callee].len());
        for &caller in &callers[callee] {
            remaining[caller] -= 1;
            if remaining[caller] == 0 {
                queue.push_back(caller);
            }
        }
    }
    assert_eq!(complete, functions.len(), "closed acyclic typed source");
    trace.work(functions.len());
}

pub(super) fn typed_component(
    owner: &ProductionPreRankedKirOwnerV1,
    component: u8,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (usize, usize, usize, Option<usize>, Option<usize>) {
    let mut trace = Trace::new(floor);
    let scope = trace.enter(&[]);
    trace.reserve(size_of::<Cleanup<'_, '_>>());
    trace.reserve(size_of::<std::thread::Result<R<()>>>());
    if component == 1 {
        typed_callable(owner, &mut trace);
    } else if component == 6 {
        original_source::<()>(owner, &mut trace, |_| {});
    } else if component == 7 {
        root_private_source(owner, &mut trace);
    } else {
        let module = owner.executable().module();
        let inventory = Graph::module(module).inventory(&mut trace);
        trace.reserve(inventory);
        if component == 0 {
            let effects = call_effects(module, &mut trace);
            trace.reserve(effects);
            units(&mut trace, module.functions.len());
        } else if component == 2 {
            call_index(owner, &mut trace);
        } else if component == 3 {
            let physical = native::physical(module, &mut trace);
            trace.reserve(physical);
        } else if component == 4 {
            let sparse = sparse(module, &mut trace);
            trace.reserve(sparse);
        } else {
            assert_eq!(component, 5);
            call_index(owner, &mut trace);
            trace.reserve(size_of::<CrArgumentRowsV1<'_>>());
            typed_arguments(owner, &mut trace);
        }
    }
    trace.leave(scope);
    let p = trace.run(work_limit, storage_limit);
    (p.work, p.storage, p.peak, p.first_work, p.first_storage)
}

pub(super) fn original_source<T>(
    owner: &ProductionPreRankedKirOwnerV1,
    trace: &mut Trace,
    callback: impl FnOnce(&mut Trace),
) {
    let metadata = Metadata::source(owner);
    let arguments = Arguments::source(owner);
    let contracts = Contracts::source(owner);
    source::raw_source::<T>(
        owner,
        trace,
        |trace| {
            call_index(owner, trace);
            trace.work(2);
            trace.work(7);
            metadata.build(trace);
            arguments.build(trace);
            contracts.build(trace);
            metadata.check(trace);
            arguments.check(trace);
            contracts.check(trace);
        },
        callback,
    );
}

pub(super) fn sparse(module: &fe2o3_kernel_ir::Module, trace: &mut Trace) -> usize {
    use fe2o3_kernel_ir::{Terminator, Type};
    let graph = Graph::module(module);
    assert_eq!((graph.functions.len(), graph.operations), (3, 2));
    let mut results = Vec::new();
    let mut operation_uses = 0;
    for (ordinal, function) in module.functions.iter().enumerate() {
        let body = function.body.as_ref().unwrap();
        assert_eq!(
            function.signature.parameters,
            [Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)]
        );
        assert_eq!(body.parameters.len(), 1);
        assert!((1..=2).contains(&body.blocks.len()));
        for (block_index, block) in body.blocks.iter().enumerate() {
            assert!(block.parameters.is_empty());
            for operation in &block.operations {
                assert_eq!(block_index, 0);
                let Kind::Call { arguments, .. } = &operation.kind else {
                    panic!("typed sparse ordinary call")
                };
                assert_eq!(arguments, &body.parameters);
                assert_eq!(operation.results.len(), 1);
                assert_eq!(
                    operation.results[0].ty,
                    Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)
                );
                operation_uses += arguments.len();
                results.push((ordinal as u32, operation.results[0].id.0));
            }
            match block.terminator.as_ref().unwrap() {
                Terminator::Branch { target, arguments } => {
                    assert_eq!(block_index, 0);
                    assert_eq!(*target, body.blocks[1].id);
                    assert!(arguments.is_empty());
                    assert!(body.blocks[1].operations.is_empty());
                }
                Terminator::Return { values } => {
                    assert!(values.is_empty() || values == &body.parameters);
                }
                _ => panic!("typed sparse straight-line control"),
            }
        }
    }
    assert!(
        graph
            .used_definitions
            .iter()
            .all(|used| !results.contains(used))
    );
    let scope = trace.enter(&[]);
    let retained = sparse_allocations(
        graph.definitions.len(),
        graph.used_definitions.len(),
        graph.blocks.len(),
        graph.operations,
        graph.successor_blocks.len(),
        trace,
    );
    // Every execution debit is one. All entry parameters start Dynamic; every
    // call result publishes Dynamic once and has no user to re-enqueue. Each
    // actual block and edge is activated exactly once, before the unresolved
    // cursor visits the two already resolved results.
    units(
        trace,
        sum(&[
            graph.used_definitions.len(),
            graph.definitions.len(),
            graph.functions.len(),
            product(4, graph.blocks.len()),
            product(5, graph.operations),
            product(3, results.len()),
            operation_uses,
            graph.successor_blocks.len(),
        ]),
    );
    trace.leave(scope);
    retained
}

pub(super) fn typed_arguments(owner: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) {
    let arguments = Arguments::source(owner);
    arguments.build(trace);
    arguments.check(trace);
}

pub(super) fn private_calls(owner: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) {
    let semantic = owner.semantic_ssa().source_semantic();
    let module = owner.executable().module();
    let associations = &owner.correspondence.lowered_functions;
    let mut calls = Vec::new();
    for association in associations {
        let mut ordinal = 0;
        for function in &module.functions {
            for operation in function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|b| &b.operations)
            {
                if let Kind::Call { callee, .. } = &operation.kind {
                    if function.id == association.kernel_ir_function {
                        let target = associations
                            .iter()
                            .find(|row| {
                                row.correspondence_owner == association.correspondence_owner
                                    && row.kernel_ir_function == *callee
                            })
                            .unwrap();
                        let source =
                            &semantic.functions()[target.semantic_function.index() as usize];
                        calls.push((
                            (association.correspondence_owner.index(), ordinal),
                            source.locals().len(),
                            callee.as_str().len(),
                        ));
                    }
                    ordinal += 1;
                }
            }
        }
    }
    calls.sort_unstable();
    let keys: Vec<_> = calls.iter().map(|row| row.0).collect();
    trace.work(calls.len());
    for &(key, locals, name) in &calls {
        trace.work(5);
        trace.work(2);
        let helper = trace.enter(&[]);
        trace.work(7);
        find(&keys, &key, trace, |_, _| 1);
        trace.work(5);
        trace.work(9);
        trace.work(product(2, name));
        trace.work(2);
        let selected = trace.enter(&[]);
        trace.work(3);
        trace.work(locals);
        Arguments::shape(0, trace);
        Arguments::entry(locals, semantic.callables().len(), trace, |trace| {
            trace.work(5);
            Arguments::walk(0, trace);
            let result = trace.enter(&[]);
            Arguments::shape(0, trace);
            trace.leave(result);
        });
        trace.leave(selected);
        trace.leave(helper);
    }
    units(trace, associations.len()); // Every RawEmpty frame is an actual None.
}

struct Arguments {
    inputs: Vec<(usize, Option<usize>)>,
    callables: usize,
}

impl Arguments {
    fn query(locals: usize, callables: usize, callback: usize, trace: &mut Trace) {
        Self::entry(locals, callables, trace, |trace| {
            Self::walk(callback, trace)
        });
    }
    fn shape(callables: usize, trace: &mut Trace) {
        trace.work(1);
        trace.work(72 + product(4, callables));
        trace.reserve(sum(&[
            size_of::<SemanticKirParameterProjectionV1>(),
            size_of::<ProductionArgumentProjectionV1>(),
            512,
            2048,
        ]));
    }
    fn walk(callback: usize, trace: &mut Trace) {
        trace.work(1);
        let walk = trace.enter(&[]);
        trace.work(1);
        trace.work(0);
        trace.reserve(exact_capacity::<AtomicArgumentFrameV1>(2));
        trace.work(1);
        trace.work(1);
        trace.work(40);
        if callback != 0 {
            trace.work(callback);
        }
        trace.leave(walk);
    }
    fn entry(
        locals: usize,
        callables: usize,
        trace: &mut Trace,
        callback: impl FnOnce(&mut Trace),
    ) {
        use fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1;
        let scope = trace.enter(&[]);
        trace.reserve(sum(&[
            size_of::<ScopedSourceCleanupBoundaryV29>(),
            size_of::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>(),
        ]));
        trace.work(2);
        trace.reserve(sum(&[
            size_of::<ArgumentEntryV18<'_>>(),
            size_of::<ArgumentQueryCustodyV18>(),
        ]));
        trace.work(1);
        trace.work(sum(&[product(4, locals), 101]));
        trace.reserve(sum(&[
            size_of::<Option<SemanticLocalIdV1>>(),
            product(
                locals,
                size_of::<Option<&SemanticKirIgnoredParameterBindingV1>>() + size_of::<bool>(),
            ),
            size_of::<IndexedArgumentTraceV1<'_>>(),
            size_of::<AdjustedArgumentShapeV1>(),
        ]));
        trace.work(1);
        let shape = trace.enter(&[]);
        Self::shape(callables, trace);
        trace.work(40);
        trace.leave(shape);
        trace.work(locals + 1);
        Self::walk(0, trace);
        callback(trace);
        trace.leave(scope);
    }
    fn source(owner: &ProductionPreRankedKirOwnerV1) -> Self {
        use fe2o3_mir_model::semantic_mir_v1::*;
        assert_eq!(
            owner.helper_source_policy_v1(),
            ProductionHelperSourcePolicyV1::RawEmpty
        );
        let semantic = owner.semantic_ssa().source_semantic();
        let rows = &owner.correspondence;
        assert!(
            rows.parameter_component_bindings.is_empty()
                && rows.ignored_parameter_bindings.is_empty()
        );
        let mut inputs = Vec::new();
        for association in &rows.lowered_functions {
            let function = &semantic.functions()[association.semantic_function.index() as usize];
            assert_eq!(
                (
                    function.abi().source_input_types().len(),
                    function.abi().adjusted_arguments().len()
                ),
                (1, 1)
            );
            assert!(matches!(
                semantic.types()[function.abi().source_input_types()[0].index() as usize].shape(),
                SemanticTypeShapeV1::Scalar(_)
            ));
            assert_eq!(
                rows.parameter_bindings
                    .iter()
                    .filter(|binding| binding.correspondence_owner
                        == association.correspondence_owner
                        && binding.semantic_function == association.semantic_function)
                    .count(),
                1
            );
            let helper_name = (function.role() == SemanticFunctionRoleV1::InternalHelper)
                .then_some(association.kernel_ir_function.as_str().len());
            inputs.push((function.locals().len(), helper_name));
        }
        Self {
            inputs,
            callables: semantic.callables().len(),
        }
    }

    fn build(&self, trace: &mut Trace) {
        for &(locals, _) in &self.inputs {
            Self::query(locals, self.callables, 2, trace);
        }
        vector::<ProductionCanonicalRankedArgumentV1>(self.inputs.len(), trace);
        vector::<ProductionArgumentProjectionV1>(0, trace);
        vector::<std::ops::Range<usize>>(self.inputs.len(), trace);
        vector::<Option<ProductionHelperLocalFrameV1<'_>>>(self.inputs.len(), trace);
        for &(locals, helper_name) in &self.inputs {
            if let Some(name) = helper_name {
                trace.work(9);
                trace.work(product(2, name));
            }
            trace.work(1);
            Self::query(locals, self.callables, 2, trace);
            trace.work(1);
        }
    }

    fn check(&self, trace: &mut Trace) {
        for &(locals, helper_name) in &self.inputs {
            if let Some(name) = helper_name {
                trace.work(9);
                trace.work(product(2, name));
            }
            trace.work(6);
            Self::query(locals, self.callables, 13, trace);
        }
    }
}

include!("production_canonical_private_call_whole_shared_metadata_v1_tests.rs");

pub(super) fn call_index(owner: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) -> usize {
    use fe2o3_kernel_ir::{BasicBlock, Function, Terminator, Type, ValueId};
    use fe2o3_mir_model::semantic_mir_v1::*;
    let module = owner.executable().module();
    let semantic = owner.semantic_ssa().source_semantic();
    let rows = &owner.correspondence;
    let groups = rows.lowered_functions.len();
    trace.work(sum(&[
        product(6, groups),
        rows.call_returns.len(),
        rows.terminator_operation_spans.len(),
        rows.parameter_bindings.len(),
        rows.parameter_component_bindings.len(),
        rows.ignored_parameter_bindings.len(),
    ]));
    let retained = sum(&[
        size_of::<ProductionCanonicalCallsV1<'_>>(),
        exact_capacity::<CanonicalCallGroupV1<'_>>(groups),
        exact_capacity::<CanonicalCallBindingV1<'_>>(rows.call_returns.len()),
    ]);
    trace.reserve(retained);
    let targets = trace.enter(&[]);
    trace.work(sum(&[module.functions.len(), product(196, groups)]));
    trace.reserve(sum(&[
        exact_capacity::<&Function>(module.functions.len()),
        exact_capacity::<(usize, &SemanticKirFunctionCorrespondenceV1)>(groups),
    ]));
    let mut names: Vec<_> = module.functions.iter().map(|f| f.id.as_str()).collect();
    heap(&mut names, trace, true, |a, b| a.len().min(b.len()) + 1);
    for pair in names.windows(2) {
        trace.work(pair[0].len().min(pair[1].len()) + 1);
    }
    let lookup = |name: &str, trace: &mut Trace| {
        find(&names, &name, trace, |a, b| a.len().min(b.len()) + 1);
    };
    for source in &rows.lowered_functions {
        let function = module
            .functions
            .iter()
            .find(|f| f.id == source.kernel_ir_function)
            .unwrap();
        let body = function.body.as_ref().unwrap();
        let source_function = &semantic.functions()[source.semantic_function.index() as usize];
        let calls: Vec<_> = rows
            .call_returns
            .iter()
            .filter(|r| {
                r.correspondence_owner == source.correspondence_owner
                    && r.semantic_function == source.semantic_function
            })
            .collect();
        let spans: Vec<_> = rows
            .terminator_operation_spans
            .iter()
            .filter(|r| {
                r.correspondence_owner == source.correspondence_owner
                    && r.semantic_function == source.semantic_function
            })
            .collect();
        lookup(function.id.as_str(), trace); // Shared inventory's independent symbol index.
        trace.work(72);
        lookup(function.id.as_str(), trace);
        let validation = trace.enter(&[]);
        trace.work(calls.len() + spans.len());
        trace.work(source_function.locals().len());
        if function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper {
            assert_eq!(
                function.signature.results,
                [Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)]
            );
            assert!(matches!(
                semantic.types()[source_function.abi().source_output_type().index() as usize]
                    .shape(),
                SemanticTypeShapeV1::Scalar(_)
            ));
            trace.work(2);
            let shape = trace.enter(&[]);
            trace.work(1);
            trace.work(72);
            trace.reserve(sum(&[
                size_of::<SemanticKirParameterProjectionV1>(),
                size_of::<ProductionArgumentProjectionV1>(),
                512,
                2048,
            ]));
            trace.work(1);
            trace.leave(shape);
        }
        let definitions = body.parameters.len()
            + body
                .blocks
                .iter()
                .map(|block| {
                    block.parameters.len()
                        + block
                            .operations
                            .iter()
                            .map(|o| o.results.len())
                            .sum::<usize>()
                })
                .sum::<usize>();
        trace.work(body.blocks.len());
        for block in &body.blocks {
            trace.work(block.operations.len());
        }
        trace.work(product(definitions + body.blocks.len(), 100));
        trace.work(3);
        vector::<(ValueId, &Type)>(definitions, trace);
        trace.work(body.blocks.len());
        for block in &body.blocks {
            trace.work(block.operations.len());
        }
        trace.work(3);
        vector::<&BasicBlock>(body.blocks.len(), trace);
        assert_eq!(spans.len(), body.blocks.len());
        for (span, block) in spans.iter().zip(&body.blocks) {
            trace.work(40);
            trace.work(block.operations.len());
            for operation in &block.operations {
                if let Kind::Call { callee, .. } = &operation.kind {
                    lookup(callee.as_str(), trace);
                }
            }
            match source_function.blocks()[span.semantic_block.index() as usize]
                .terminator()
                .kind()
            {
                SemanticTerminatorKindV1::Return => {
                    let Some(Terminator::Return { values }) = &block.terminator else {
                        panic!("source return anchor")
                    };
                    assert_eq!(span.operation_count, 0);
                    trace.work(values.len());
                    for _ in values {
                        trace.work(40);
                        trace.work(1);
                    }
                }
                SemanticTerminatorKindV1::Call(call) => {
                    assert_eq!(source_function.blocks().len(), 2);
                    assert!(
                        source_function
                            .blocks()
                            .iter()
                            .all(|block| block.statements().is_empty())
                    );
                    assert!(matches!(
                        source_function.blocks()[1].terminator().kind(),
                        SemanticTerminatorKindV1::Return
                    ));
                    assert!(matches!(
                        source_function.locals()
                            [call.destination().unwrap().place().local().index() as usize]
                            .role(),
                        SemanticLocalRoleV1::Temporary
                    ));
                    let [operation] = block.operations.as_slice() else {
                        panic!("single typed call in source block")
                    };
                    let Kind::Call { callee, arguments } = &operation.kind else {
                        panic!("typed ordinary call")
                    };
                    assert_eq!((arguments.len(), operation.results.len()), (1, 1));
                    assert_eq!((span.first_operation_ordinal, span.operation_count), (0, 1));
                    trace.work(72);
                    lookup(callee.as_str(), trace);
                    trace.work(sum(&[
                        product(2, callee.as_str().len()),
                        arguments.len(),
                        call.arguments().len(),
                    ]));
                    for _ in arguments {
                        trace.work(40);
                        trace.work(1);
                    }
                    units(trace, operation.results.len());
                    trace.work(1); // Original one-operation call span.
                    trace.work(40); // Actual continuation block query.
                    let Some(Terminator::Branch { arguments, .. }) = &block.terminator else {
                        panic!("typed call continuation")
                    };
                    assert!(arguments.is_empty());
                    trace.work(0);
                    trace.work(0); // No finishing casts after the call.
                    // The returned temporary is defined on the call edge but
                    // never used: one SSA edge definition and no edge argument.
                    trace.work(1);
                    trace.work(0); // Empty transported result roster.
                }
                _ => panic!("closed call-index source terminator"),
            }
        }
        trace.leave(validation);
    }
    let mut bindings = Vec::new();
    for source in &rows.lowered_functions {
        let function = module
            .functions
            .iter()
            .position(|f| f.id == source.kernel_ir_function)
            .unwrap();
        let mut call_ordinal = module
            .functions
            .iter()
            .take(function)
            .flat_map(|f| &f.body.as_ref().unwrap().blocks)
            .flat_map(|b| &b.operations)
            .filter(|op| matches!(op.kind, Kind::Call { .. }))
            .count();
        for row in &module.functions[function].body.as_ref().unwrap().blocks {
            for op in &row.operations {
                if let Kind::Call { callee, .. } = &op.kind {
                    assert!(!callee.as_str().starts_with("__fe2o3_ir_"));
                    trace.work(1);
                    trace.work(1);
                    trace.work(product(2, callee.as_str().len() + 1));
                    trace.work(41);
                    trace.work(72);
                    bindings.push((source.correspondence_owner.index(), call_ordinal));
                    call_ordinal += 1;
                }
            }
        }
        trace.work(
            rows.call_returns
                .iter()
                .filter(|r| {
                    r.correspondence_owner == source.correspondence_owner
                        && r.semantic_function == source.semantic_function
                })
                .count(),
        );
    }
    heap(&mut bindings, trace, true, |_, _| 1);
    trace.work(bindings.len());
    trace.leave(targets);
    trace.work(3);
    retained
}

pub(super) struct OriginOperation {
    pub(super) private: bool,
    pub(super) operands: usize,
    pub(super) results: usize,
    pub(super) aliases: Vec<usize>,
    pub(super) call: bool,
}

pub(super) struct Origins {
    pub(super) operations: Vec<OriginOperation>,
    associations: Vec<u64>,
    // One row per source call alias: physical-private ordinal, association.
    calls: Vec<(usize, usize)>,
}

impl Origins {
    pub(super) fn source(owner: &ProductionPreRankedKirOwnerV1) -> Self {
        let module = owner.executable().module();
        let correspondence = &owner.correspondence;
        let associations: Vec<_> = correspondence
            .lowered_functions
            .iter()
            .map(|row| {
                (u64::from(row.correspondence_owner.index()) << 32)
                    | u64::from(row.semantic_function.index())
            })
            .collect();
        let mut coordinates = Vec::new();
        let mut operations = Vec::new();
        for (function, f) in module.functions.iter().enumerate() {
            for block in &f.body.as_ref().unwrap().blocks {
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    let (private, call, operands) = match &operation.kind {
                        Kind::Constant(_) => (false, false, 0),
                        Kind::Alloca { count: None, .. } => (true, false, 0),
                        Kind::Load { .. } => (true, false, 1),
                        Kind::Store { .. } => (true, false, 2),
                        Kind::Call { arguments, .. } => (true, true, arguments.len()),
                        _ => panic!("closed private source census"),
                    };
                    coordinates.push((function, block.id, ordinal));
                    operations.push(OriginOperation {
                        private,
                        call,
                        operands,
                        results: operation.results.len(),
                        aliases: Vec::new(),
                    });
                }
            }
        }
        let spans = correspondence
            .statement_operation_spans
            .iter()
            .map(|s| {
                (
                    s.correspondence_owner,
                    s.semantic_function,
                    s.kernel_ir_block,
                    s.first_operation_ordinal,
                    s.operation_count,
                )
            })
            .chain(correspondence.terminator_operation_spans.iter().map(|s| {
                (
                    s.correspondence_owner,
                    s.semantic_function,
                    s.kernel_ir_block,
                    s.first_operation_ordinal,
                    s.operation_count,
                )
            }))
            .chain(correspondence.synthetic_operation_spans.iter().map(|s| {
                (
                    s.correspondence_owner,
                    s.semantic_function,
                    s.kernel_ir_block,
                    s.first_operation_ordinal,
                    s.operation_count,
                )
            }));
        for (root, semantic, block, first, count) in spans {
            let association = correspondence
                .lowered_functions
                .iter()
                .position(|row| {
                    row.correspondence_owner == root && row.semantic_function == semantic
                })
                .unwrap();
            let function = module
                .functions
                .iter()
                .position(|f| {
                    f.id == correspondence.lowered_functions[association].kernel_ir_function
                })
                .unwrap();
            for ordinal in first as usize..sum(&[first as usize, count as usize]) {
                let operation = coordinates
                    .iter()
                    .position(|row| *row == (function, block, ordinal))
                    .unwrap();
                operations[operation].aliases.push(association);
            }
        }
        let mut calls = Vec::new();
        let mut physical = 0;
        for operation in &operations {
            assert!(!operation.aliases.is_empty());
            if !operation.private {
                continue;
            }
            if operation.call {
                calls.extend(
                    operation
                        .aliases
                        .iter()
                        .map(|&association| (physical, association)),
                );
            }
            physical += 1;
        }
        // The selected source call order is the original root order and each
        // root has one ordinary call. Reject other schedules explicitly.
        assert!(calls.windows(2).all(|pair| pair[0] < pair[1]));
        Self {
            operations,
            associations,
            calls,
        }
    }

    pub(super) fn private_count(&self) -> usize {
        self.operations
            .iter()
            .filter(|operation| operation.private)
            .count()
    }

    fn check_associations(&self, trace: &mut Trace) {
        trace.work(1);
        for _ in &self.associations {
            trace.work(3);
        }
    }

    fn call_row(&self, association: usize, trace: &mut Trace) {
        trace.work(3);
        trace.work(1);
        let mut keys = self.associations.clone();
        keys.sort_unstable();
        find(&keys, &self.associations[association], trace, |_, _| 1);
        trace.work(1);
        consume::operation(trace);
        trace.work(2);
    }

    fn check_aliases(&self, trace: &mut Trace) {
        trace.work(1);
        for _ in &self.calls {
            trace.work(3);
        }
        for operation in 0..self.private_count() {
            trace.work(1);
            units(
                trace,
                self.calls.iter().filter(|row| row.0 == operation).count(),
            );
        }
    }

    pub(super) fn check(&self, trace: &mut Trace) {
        units(trace, 2);
        self.check_associations(trace);
        trace.work(1);
        for operation in &self.operations {
            trace.work(3);
            if operation.private {
                for _ in &operation.aliases {
                    trace.work(4);
                }
            }
        }
        trace.work(1);
        self.check_aliases(trace);
        for &(_, association) in &self.calls {
            self.call_row(association, trace);
        }
    }

    pub(super) fn build(&self, trace: &mut Trace) {
        trace.work(1);
        for _ in &self.operations {
            trace.work(2);
        }
        trace.reserve(size_of::<CpcOriginsV1<'_, '_>>());
        vector::<CpcOriginalOperationV1>(self.private_count(), trace);
        vector::<ProductionCanonicalPrivateSourceAliasV1>(
            self.operations
                .iter()
                .filter(|row| row.private)
                .map(|row| row.aliases.len())
                .sum(),
            trace,
        );
        vector::<ProductionCanonicalPrivateCallSiteV1>(self.calls.len(), trace);
        vector::<(u64, usize)>(self.associations.len(), trace);
        units(trace, 2 * self.associations.len());
        let mut keys = self.associations.clone();
        heap(&mut keys, trace, true, |_, _| 1);
        vector::<Option<usize>>(self.operations.len(), trace);
        trace.work(self.operations.len());
        vector::<(usize, usize, usize)>(self.calls.len(), trace);
        vector::<std::ops::Range<usize>>(self.private_count(), trace);
        trace.work(self.private_count());
        self.check_associations(trace);
        for operation in &self.operations {
            trace.work(2);
            if operation.private {
                units(trace, 2 + operation.aliases.len());
            }
        }
        for &(_, association) in &self.calls {
            self.call_row(association, trace);
            trace.work(1);
        }
        units(trace, self.calls.len());
        let mut aliases = self.calls.clone();
        heap(&mut aliases, trace, true, |_, _| 1);
        let mut next = 0;
        for operation in 0..self.private_count() {
            trace.work(1);
            while next < aliases.len() {
                trace.work(1);
                if aliases[next].0 != operation {
                    break;
                }
                next += 1;
            }
        }
        self.check_aliases(trace);
        self.check(trace);
    }

    fn temporary_index(&self, trace: &mut Trace) -> usize {
        trace.reserve(size_of::<Vec<Option<usize>>>());
        let backing = vector::<Option<usize>>(self.operations.len(), trace);
        trace.work(self.operations.len());
        sum(&[size_of::<Vec<Option<usize>>>(), backing])
    }

    pub(super) fn check_current(&self, trace: &mut Trace) {
        self.check(trace);
        trace.work(2);
        let index = self.temporary_index(trace);
        for _ in self.operations.iter().filter(|row| row.private) {
            trace.work(4);
            trace.work(1);
            trace.work(2);
            trace.work(1); // Original block.
            consume::operation(trace);
        }
        for _ in &self.operations {
            trace.work(2);
        }
        trace.release(index);
    }

    fn allocate_transport(&self, trace: &mut Trace) -> usize {
        trace.work(1);
        trace.reserve(size_of::<CsaTransportV1>());
        vector::<ProductionCanonicalScalarAssertionV1>(0, trace);
        trace.reserve(size_of::<CpcTransportV1<'_, '_, '_>>() - size_of::<CsaTransportV1>());
        let operations =
            vector::<ProductionCanonicalPrivateOperationV1>(self.private_count(), trace);
        sum(&[size_of::<CpcTransportV1<'_, '_, '_>>(), operations])
    }

    pub(super) fn transport_original(&self, trace: &mut Trace) -> usize {
        let retained = self.allocate_transport(trace);
        units(trace, self.private_count());
        retained
    }

    pub(super) fn transport_pair(&self, trace: &mut Trace) -> usize {
        self.check_current(trace);
        trace.work(2);
        let retained = self.allocate_transport(trace);
        let inverse = self.temporary_index(trace);
        for operation in &self.operations {
            trace.work(2);
            consume::operation(trace);
            if operation.private {
                consume::operation(trace);
            }
        }
        for operation in self.operations.iter().filter(|row| row.private) {
            trace.work(3);
            consume::operation(trace);
            consume::operation(trace);
            consume::operation(trace);
            trace.work(3);
            for _ in 0..operation.operands {
                trace.work(3);
            }
            for _ in 0..operation.results {
                trace.work(3);
                units(trace, 2);
            }
            trace.work(1);
        }
        self.check_current(trace);
        trace.release(inverse);
        retained
    }
}
