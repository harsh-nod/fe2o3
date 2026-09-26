// Final-consume custody arithmetic. The native final proof and private observer
// are separate obligations; this leaf does not equate lineage with consumption.
use fe2o3_kernel_ir::{Module, OperationKind as Kind, Terminator};

pub(super) fn vector<T>(count: usize, trace: &mut Trace) -> usize {
    let bytes = exact_capacity::<T>(count);
    trace.reserve(bytes);
    trace.reserve(0);
    bytes
}

fn function(trace: &mut Trace) {
    trace.work(2);
}
fn block(trace: &mut Trace) {
    trace.work(1);
    function(trace);
    trace.work(1);
}
pub(super) fn operation(trace: &mut Trace) {
    trace.work(1);
    block(trace);
    trace.work(1);
}
fn edge(trace: &mut Trace) {
    trace.work(1);
    block(trace);
    trace.work(1);
}
fn definition(parameter: bool, trace: &mut Trace) {
    trace.work(1);
    if parameter {
        function(trace);
    } else {
        operation(trace);
    }
    trace.work(1);
}

fn empty(actual: &Module, original: &Module, trace: &mut Trace) -> usize {
    let a = Graph::module(actual);
    let o = Graph::module(original);
    trace.work(1);
    trace.reserve(size_of::<CsLineageV1>());
    sum(&[
        size_of::<CsLineageV1>(),
        vector::<CsFunctionV1>(a.functions.len(), trace),
        vector::<ProductionCanonicalScalarOperationOriginV1>(a.operations, trace),
        vector::<CsDefinitionLinkV1>(a.definitions.len(), trace),
        vector::<std::ops::Range<usize>>(a.definitions.len(), trace),
        vector::<std::ops::Range<usize>>(a.blocks.len(), trace),
        vector::<ProductionCanonicalScalarBlockSegmentV1>(o.blocks.len(), trace),
        vector::<ProductionCanonicalScalarBlockControlV1>(o.blocks.len(), trace),
        vector::<ProductionCanonicalScalarEdgeControlV1>(o.successor_blocks.len(), trace),
        vector::<CsUseV1>(a.used_definitions.len(), trace),
        vector::<CsEdgeV1>(a.successor_blocks.len(), trace),
        vector::<CsEdgeArgumentV1>(a.edge_arguments, trace),
    ])
}

fn identity(module: &Module, trace: &mut Trace) -> usize {
    let graph = Graph::module(module);
    let retained = empty(module, module, trace);
    units(
        trace,
        sum(&[
            graph.functions.len(),
            graph.operations,
            product(2, graph.definitions.len()),
            product(3, graph.blocks.len()),
            graph.used_definitions.len(),
            product(2, graph.successor_blocks.len()),
            graph.edge_arguments,
        ]),
    );
    trace.work(12);
    retained
}

fn pair(original: &Module, input: &Module, output: &Module, trace: &mut Trace) -> usize {
    replay::closed_pair(input, output);
    let before = Graph::module(input).inventory(trace);
    trace.reserve(before);
    let after = Graph::module(output).inventory(trace);
    trace.reserve(after);
    trace.work(8);
    let checked = replay::transition(input, output, trace);
    trace.reserve(checked);
    let control = replay::control(input, output, trace);
    trace.reserve(control);
    trace.work(sum(&[
        admission::inverse(input).4,
        admission::inverse(output).4,
    ]));
    let definitions = replay::definitions(output);
    for _ in &definitions {
        trace.work(3);
        trace.work(1);
    }
    trace.work(definitions.len());
    let retained = empty(output, original, trace);
    for _ in &input.functions {
        function(trace);
        trace.work(1);
    }
    for _ in 0..Graph::module(input).operations {
        operation(trace);
        trace.work(1);
    }
    for &(lookup, _) in &definitions {
        trace.work(1);
        definition(lookup == 2, trace);
        trace.work(1);
    }
    let mut links: Vec<_> = (0..definitions.len()).map(|n| (n, n, false)).collect();
    heap_schedule(&mut links, trace, false, false, |_, _| 1);
    for _ in &definitions {
        trace.work(2);
    }
    trace.work(0);
    units(trace, product(2, definitions.len()));
    units(trace, Graph::module(original).blocks.len());
    for ((original, input), output) in original
        .functions
        .iter()
        .zip(&input.functions)
        .zip(&output.functions)
    {
        let original = &original.body.as_ref().unwrap().blocks;
        let input = &input.body.as_ref().unwrap().blocks;
        let output = &output.body.as_ref().unwrap().blocks;
        assert_eq!(original.len() % input.len(), 0);
        assert_eq!(input.len() % output.len(), 0);
        for _ in output {
            trace.work(1);
            for input_index in 0..input.len() / output.len() {
                block(trace);
                units(trace, 3); // Analysis control.block: block + function + query.
                for _ in 0..original.len() / input.len() {
                    trace.work(5);
                    block(trace);
                    if input.len() != output.len() && input_index == 0 {
                        edge(trace);
                    }
                    trace.work(1);
                }
            }
            trace.work(1);
        }
    }
    let o = Graph::module(original);
    let a = Graph::module(input);
    let b = Graph::module(output);
    for _ in &o.successor_blocks {
        trace.work(3);
        block(trace);
        if !a.successor_blocks.is_empty() {
            units(trace, 4);
        }
        trace.work(1);
    }
    for function in &output.functions {
        for row in &function.body.as_ref().unwrap().blocks {
            for op in &row.operations {
                let operands = match &op.kind {
                    Kind::Load { .. } => 1,
                    Kind::Store { .. } => 2,
                    Kind::Call { arguments, .. } => arguments.len(),
                    _ => 0,
                };
                for _ in 0..operands {
                    trace.work(1);
                    operation(trace);
                    units(trace, 2);
                }
            }
            if let Some(Terminator::Return { values }) = &row.terminator {
                for _ in values {
                    trace.work(1);
                    block(trace);
                    units(trace, 2);
                }
            }
        }
    }
    for _ in &b.successor_blocks {
        edge(trace);
        trace.work(1);
    }
    assert_eq!(b.edge_arguments, 0);
    trace.work(8);
    trace.work(12);
    retained
}

pub(super) fn lineage(input: &Module, output: &Module, merged: bool, trace: &mut Trace) -> usize {
    lineage_observer(input, output, merged, None, trace)
}

pub(super) fn lineage_observer(
    input: &Module,
    output: &Module,
    merged: bool,
    observer: Option<&shared_source::Origins>,
    trace: &mut Trace,
) -> usize {
    trace.work(1);
    trace.work(2);
    let scope = trace.enter(&[]);
    let mut previous = identity(input, trace);
    trace.leave(scope);
    trace.reserve(previous);
    for round in 0..if merged { 2 } else { 1 } {
        let source = if round == 0 { input } else { output };
        for (before, after) in [(source, source), (source, output)] {
            trace.work(2);
            let scope = trace.enter(&[]);
            let next = pair(input, before, after, trace);
            let observation = observer.map_or(0, |origins| origins.transport_pair(trace));
            trace.leave(scope);
            trace.reserve(sum(&[next, observation]));
            if observation != 0 {
                trace.release(observation);
            }
            trace.release(previous);
            previous = next;
        }
    }
    trace.work(1);
    previous
}

pub(super) fn lineage_component(input: &Module, output: &Module, merged: bool, trace: &mut Trace) {
    trace.work(2);
    let scope = trace.enter(&[]);
    let inventory = Graph::module(input).inventory(trace);
    trace.reserve(inventory);
    lineage(input, output, merged, trace);
    trace.leave(scope);
}

pub(super) fn final_view(
    owner: &ProductionPreRankedKirOwnerV1,
    output: &Module,
    trace: &mut Trace,
    callback: impl FnOnce(&mut Trace),
) {
    let projection = source::Projection::source(owner);
    projection.build(trace);
    trace.work(4);
    let analysis = trace.enter(&[]);
    let inventory = Graph::module(output).inventory(trace);
    trace.reserve(inventory);
    trace.work(6);
    trace.work(2);
    let scope = trace.enter(&[]);
    projection.check(trace);
    trace.work(8);
    trace.work(3);
    trace.reserve(size_of::<Vec<CrInertRowV1<'_>>>());
    let facts = projection.facts();
    vector::<CrInertRowV1<'_>>(facts.len(), trace);
    units(trace, 1 + output.kernels.len());
    for _ in &output.functions {
        function(trace);
        trace.work(2);
        trace.work(1);
    }
    trace.reserve(size_of::<CrInertV1<'_, '_>>());
    let ranked = admission::Ranked::new(output, facts);
    let candidate = ranked.candidate(trace);
    trace.reserve(candidate);
    ranked.checked(trace, |trace| {
        trace.work(1);
        callback(trace);
    });
    trace.leave(scope);
    trace.leave(analysis);
}

fn root_final_calls(
    owner: &ProductionPreRankedKirOwnerV1,
    output: &Module,
    origins: &shared_source::Origins,
    trace: &mut Trace,
) {
    let original = owner.executable().module();
    replay::closed_pair(original, output);
    let graph = Graph::module(output);
    origins.check(trace);
    let effects = shared_source::call_effects(output, trace);
    trace.reserve(effects);
    trace.work(1);
    trace.work(1);
    trace.reserve(size_of::<CpcFinalCallIndexV1>());
    vector::<Option<usize>>(graph.functions.len(), trace);
    trace.work(graph.functions.len());
    for _ in &graph.functions {
        function(trace);
        function(trace);
        trace.work(1);
    }
    vector::<Option<usize>>(graph.operations, trace);
    trace.work(graph.operations);
    for _ in &graph.callees {
        operation(trace);
        trace.work(1);
    }
    for _ in &owner.correspondence.lowered_functions {
        function(trace);
        trace.work(1);
        trace.work(1);
        trace.work(2);
    }
    let actual: Vec<_> = output
        .functions
        .iter()
        .flat_map(|f| &f.body.as_ref().unwrap().blocks)
        .flat_map(|b| &b.operations)
        .collect();
    for (ordinal, row) in origins
        .operations
        .iter()
        .enumerate()
        .filter(|(_, row)| row.private)
    {
        trace.work(2);
        if !row.call {
            continue;
        }
        let Kind::Call { callee, .. } = &actual[ordinal].kind else {
            panic!("retained final call");
        };
        operation(trace);
        trace.work(1);
        for _ in &row.aliases {
            trace.work(1);
            function(trace);
            function(trace);
            trace.work(4);
            trace.work(product(2, callee.as_str().len()));
        }
        for alias in &row.aliases {
            find(&row.aliases, alias, trace, |_, _| 1);
        }
    }
}

fn root_final_join(
    owner: &ProductionPreRankedKirOwnerV1,
    module: &Module,
    origins: &shared_source::Origins,
    trace: &mut Trace,
) {
    let graph = Graph::module(module);
    trace.work(1); // Native trap-policies query.
    trace.work(3);
    trace.work(1);
    let sparse = if module.functions.len() == 1 {
        SparseRoot::source(module).derive(trace)
    } else {
        shared_source::sparse(module, trace)
    };
    trace.reserve(sparse);
    trace.work(1); // Actual zero pair count.
    units(
        trace,
        sum(&[
            owner.correspondence.statement_operation_spans.len(),
            owner.correspondence.terminator_operation_spans.len(),
            owner.correspondence.synthetic_operation_spans.len(),
        ]),
    );
    root_final_calls(owner, module, origins, trace);
    origins.check(trace);
    origins.check_current(trace);
    trace.reserve(size_of::<CpcSiteViewV1<'_, '_, '_>>());
    vector::<Option<usize>>(graph.operations, trace);
    for _ in 0..graph.operations {
        trace.work(2);
        operation(trace);
        trace.work(1);
    }
    trace.work(1); // Physical-proof borrow.
    trace.work(1); // Original source guard before the transported reader.
    root_private_reader_with(owner, trace, false);
    trace.work(1); // Exact final native owner.
    trace.work(3);
    trace.reserve(sum(&[
        size_of::<ProductionCanonicalPrivateCallPoliciesV1<'_, '_, '_>>(),
        size_of::<CrGuardV1>(),
        size_of::<std::thread::Result<CsResultV1<()>>>(),
        size_of::<std::thread::Result<()>>(),
    ]));
    let callback = trace.enter(&[(1, Some(10)), (1, Some(11)), (1, Some(12))]);
    trace.mark("fresh final private-call callback");
    trace.leave(callback);
}

pub(super) fn root_consume(
    owner: &ProductionPreRankedKirOwnerV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Result<(usize, usize, usize, Option<usize>, Option<usize>, bool), &'static str> {
    let module = owner.executable().module();
    let merged = module.functions.len() != 1;
    let output = if merged {
        history::shared_history_retained(module).0
    } else {
        module.clone()
    };
    let origins = shared_source::Origins::source(owner);
    let mut trace = Trace::new(floor);
    trace.work(1);
    cpc_scope::<()>(&mut trace, |trace| {
        root_private_source_with(owner, trace, |trace| {
            replay::history(module, &output, merged, trace);
            origins.build(trace);
            origins.transport_original(trace);
            lineage_observer(module, &output, merged, Some(&origins), trace);
            final_view(owner, &output, trace, |trace| {
                native::root_memory_facade::<CsResultV1<()>>(&output, trace, |trace| {
                    cpc_scope::<()>(trace, |trace| {
                        root_final_join(owner, &output, &origins, trace)
                    });
                });
            });
        });
    });
    let inverse = admission::inverse(&output);
    let predicted = trace.composed(work_limit, storage_limit, |mark| match mark {
        "final canonical inverse interior cutoff is not derived" => Some((inverse.0, inverse.2)),
        "native schema attribute-order interior cutoff is not derived" => {
            Some((native::schema_work(&output), 0))
        }
        _ => None,
    })?;
    let called = predicted
        .marks
        .contains(&"fresh final private-call callback");
    Ok((
        predicted.work,
        predicted.storage,
        predicted.peak,
        predicted.first_work,
        predicted.first_storage,
        called,
    ))
}

#[test]
fn whole_consume_composition_keeps_postflight_and_refuses_opaque_interiors() {
    let mut trace = Trace::new(43);
    let scope = trace.enter(&[(1, None)]);
    trace.work(3);
    trace.reserve(7);
    trace.mark("opaque");
    trace.work(2);
    trace.leave(scope);
    let run = |work, storage| trace.composed(work, storage, |_| Some((9, 11)));
    let full = run(15, 61).unwrap();
    assert_eq!((full.work, full.storage, full.peak), (15, 43, 61));
    let terminal = run(14, 61).unwrap();
    assert_eq!(
        (terminal.work, terminal.storage, terminal.first_work),
        (14, 43, Some(15))
    );
    assert_eq!(run(11, 61).unwrap_err(), "opaque");
    assert_eq!(run(15, 60).unwrap_err(), "opaque");
    let early = run(2, 61).unwrap();
    assert_eq!(
        (early.work, early.storage, early.first_work),
        (1, 43, Some(3))
    );
}
