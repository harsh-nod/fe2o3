// Exact debit schedule for retained operations/definitions and straight-chain
// merges. Expected rows come from source structure, never optimizer receipts.
use fe2o3_kernel_ir::{Module, OperationKind as Kind, Terminator, Type};

pub(super) fn scratch() -> usize {
    use fe2o3_pliron::{PlironOptimizationPassV1, UnauthenticatedPolicy3ExecutionClaimV1};
    sum(&[
        416,
        128,
        size_of::<UnauthenticatedPolicy3ExecutionClaimV1<'_>>(),
        size_of::<[PlironOptimizationPassV1; 8]>(),
        size_of::<[PlironOptimizationPassV1; 2]>(),
        128,
    ])
}

fn type_work(ty: &Type, trace: &mut Trace) {
    trace.work(1);
    match ty {
        Type::Unit | Type::Scalar(_) => {}
        Type::Pointer(pointer) => type_work(&pointer.pointee, trace),
        _ => panic!("closed transition type profile"),
    }
}

pub(super) fn definitions(module: &Module) -> Vec<(usize, &Type)> {
    let mut rows = Vec::new();
    for function in &module.functions {
        let body = function.body.as_ref().unwrap();
        assert_eq!(body.parameters.len(), function.signature.parameters.len());
        rows.extend(function.signature.parameters.iter().map(|ty| (2, ty)));
        for block in &body.blocks {
            assert!(block.parameters.is_empty());
            for operation in &block.operations {
                rows.extend(operation.results.iter().map(|result| (4, &result.ty)));
            }
        }
    }
    rows
}

pub(super) fn closed_pair(input: &Module, output: &Module) -> usize {
    assert_eq!(input.id, output.id);
    assert_eq!(input.kernels, output.kernels);
    assert!(input.required_capabilities.is_empty());
    assert_eq!(input.required_capabilities, output.required_capabilities);
    assert_eq!(input.functions.len(), output.functions.len());
    let mut merged = 0;
    for (a, b) in input.functions.iter().zip(&output.functions) {
        assert_eq!((&a.id, &a.signature, a.role), (&b.id, &b.signature, b.role));
        assert!(a.required_capabilities.is_empty());
        assert_eq!(a.required_capabilities, b.required_capabilities);
        let a = a.body.as_ref().unwrap();
        let b = b.body.as_ref().unwrap();
        assert_eq!(a.parameters, b.parameters);
        assert!(a.blocks.iter().all(|block| block.parameters.is_empty()));
        assert!(b.blocks.iter().all(|block| block.parameters.is_empty()));
        if a.blocks.len() == b.blocks.len() {
            assert_eq!(a, b);
        } else {
            assert_eq!((a.blocks.len(), b.blocks.len()), (2, 1));
            assert_eq!(a.blocks[0].operations, b.blocks[0].operations);
            assert_eq!(a.blocks[1].terminator, b.blocks[0].terminator);
            assert!(a.blocks[1].operations.is_empty());
            merged += 1;
        }
        for (index, block) in a.blocks.iter().enumerate() {
            match block.terminator.as_ref().unwrap() {
                Terminator::Branch { target, arguments } => {
                    assert!(arguments.is_empty());
                    assert_eq!(*target, a.blocks[index + 1].id);
                }
                Terminator::Return { .. } => assert_eq!(index + 1, a.blocks.len()),
                _ => panic!("closed transition chain profile"),
            }
            for operation in &block.operations {
                assert!(matches!(
                    operation.kind,
                    Kind::Constant(_)
                        | Kind::Alloca { count: None, .. }
                        | Kind::Load { .. }
                        | Kind::Store { .. }
                        | Kind::Call { .. }
                ));
            }
        }
    }
    merged
}

pub(super) fn transition(input: &Module, output: &Module, trace: &mut Trace) -> usize {
    use fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1;
    let scope = trace.enter(&[]);
    let retained = size_of::<CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>>();
    trace.work(1);
    trace.reserve(retained);
    solver(input, output, trace, true);
    trace.leave(scope);
    retained
}

pub(super) fn control(input: &Module, output: &Module, trace: &mut Trace) -> usize {
    use fe2o3_kernel_analysis::{
        CanonicalKirBlockControlV1, CanonicalKirEdgeControlV1, CanonicalKirOutputUseV1,
        CheckedCanonicalKirControlIndexV1,
    };
    use fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1;
    let graph = Graph::module(input);
    let scope = trace.enter(&[]);
    trace.work(1);
    let header = size_of::<CheckedCanonicalKirControlIndexV1<'_, '_, '_>>();
    trace.reserve(header);
    let arrays = [
        (
            exact_capacity::<CanonicalKirBlockControlV1>(graph.blocks.len()),
            graph.blocks.len(),
        ),
        (
            exact_capacity::<CanonicalKirEdgeControlV1>(graph.successor_blocks.len()),
            graph.successor_blocks.len(),
        ),
        (
            exact_capacity::<Option<CanonicalKirOutputUseV1>>(graph.used_definitions.len()),
            graph.used_definitions.len(),
        ),
        (
            exact_capacity::<Option<CanonicalKirEdgeArgumentCoordinateV1>>(graph.edge_arguments),
            graph.edge_arguments,
        ),
    ];
    for (bytes, count) in arrays {
        trace.reserve(bytes);
        trace.work(count);
    }
    solver(input, output, trace, false);
    trace.leave(scope);
    sum(&[header, arrays.iter().map(|row| row.0).sum()])
}

fn solver(input: &Module, output: &Module, trace: &mut Trace, complete: bool) {
    use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
    use fe2o3_kernel_ir::{CanonicalKirTransitionCandidateV1, ScalarType};
    type State = (
        &'static Inventory<'static>,
        &'static Inventory<'static>,
        CanonicalKirTransitionCandidateV1<'static>,
        [Vec<usize>; 16],
        [Vec<u8>; 2],
        Vec<Option<(ScalarType, u128)>>,
    );
    let merged = closed_pair(input, output);
    let a = Graph::module(input);
    let b = Graph::module(output);
    let definitions = definitions(input);
    assert_eq!(definitions.len(), b.definitions.len());
    let (f, bi, bo, oi, oo, d, ei, eo) = (
        a.functions.len(),
        a.blocks.len(),
        b.blocks.len(),
        a.operations,
        b.operations,
        definitions.len(),
        a.successor_blocks.len(),
        b.successor_blocks.len(),
    );
    assert_eq!(oi, oo);
    assert_eq!(a.edge_arguments, 0);
    assert_eq!(b.edge_arguments, 0);
    let scope = trace.enter(&[]);
    trace.work(1);
    trace.reserve(size_of::<State>());
    macro_rules! array {
        ($ty:ty, $count:expr) => {{
            trace.reserve(exact_capacity::<$ty>($count));
            trace.work($count);
        }};
    }
    for count in [f, f, bi, bi, bo, bo, oi, oo, d] {
        array!(usize, count);
    }
    array!(u8, d);
    array!(usize, d);
    array!(Option<(ScalarType, u128)>, d);
    for count in [bi, ei, ei, ei] {
        array!(usize, count);
    }
    array!(usize, bi); // Cached selected successor for every input block.
    array!(u8, bi);
    array!(usize, bi);
    units(trace, d + 3 * ei); // Parents and edge targets (row + block lookup).
    trace.work(1);
    trace.work(product(2, input.id.as_str().len()));
    trace.work(1);
    for kernel in &input.kernels {
        assert!(kernel.required_capabilities.is_empty());
        trace.work(1);
        trace.work(product(2, kernel.id.as_str().len()));
        trace.work(product(2, kernel.entry.as_str().len()));
        trace.work(1);
    }
    for function in &input.functions {
        units(trace, 2);
        trace.work(product(2, function.id.as_str().len()));
        for types in [&function.signature.parameters, &function.signature.results] {
            trace.work(1);
            for ty in types {
                type_work(ty, trace);
            }
        }
        trace.work(1);
        units(trace, function.signature.parameters.len());
    }
    // Per output block: row/range/function; per retained segment:
    // row/input block/input function. Every original block belongs to one chain.
    for (a, b) in input.functions.iter().zip(&output.functions) {
        let a = &a.body.as_ref().unwrap().blocks;
        let b = &b.body.as_ref().unwrap().blocks;
        for _ in b {
            units(trace, 3 + 4 * (a.len() / b.len()));
        }
    }
    units(trace, f);
    for function in &input.functions {
        for block in &function.body.as_ref().unwrap().blocks {
            for operation in &block.operations {
                units(trace, 9);
                match &operation.kind {
                    Kind::Alloca { element, .. } => type_work(element, trace),
                    Kind::Call { callee, .. } => trace.work(product(2, callee.as_str().len())),
                    _ => {}
                }
                for result in &operation.results {
                    type_work(&result.ty, trace);
                }
            }
        }
    }
    for &(lookup, ty) in &definitions {
        units(trace, 5 + lookup);
        type_work(ty, trace);
    }
    units(trace, 3 * d + bo);
    for function in &input.functions {
        for block in &function.body.as_ref().unwrap().blocks {
            for operation in &block.operations {
                trace.work(1);
                if matches!(operation.kind, Kind::Constant(_)) {
                    units(trace, 2);
                }
            }
        }
    }
    // No definition is replaced. Seeded literals precede the single iteration;
    // both parameters and result obligations unite their own unchanged roots.
    trace.work(1);
    units(trace, bi); // Refresh selected successors before reachability.
    trace.work(bi);
    units(trace, f);
    // Reachability processes entry blocks as a LIFO queue. Edge visits are
    // four single debits and occur before that target's later pop.
    for function in input.functions.iter().rev() {
        for block in &function.body.as_ref().unwrap().blocks {
            trace.work(1);
            if matches!(block.terminator, Some(Terminator::Branch { .. })) {
                units(trace, 4);
            }
        }
    }
    units(trace, bi + 3 * oi);
    for &(lookup, _) in &definitions {
        units(trace, 7 + lookup);
    }
    for (a, b) in input.functions.iter().zip(&output.functions) {
        let a = &a.body.as_ref().unwrap().blocks;
        let b = &b.body.as_ref().unwrap().blocks;
        for _ in b {
            units(trace, 2);
            if a.len() != b.len() {
                units(trace, 16);
            }
        }
    }
    assert_eq!(merged, bi - bo);
    units(trace, 8 * eo);
    for function in &input.functions {
        for block in &function.body.as_ref().unwrap().blocks {
            trace.work(1);
            if matches!(block.terminator, Some(Terminator::Branch { .. })) {
                units(trace, 4);
            }
        }
    }
    for function in &output.functions {
        for block in &function.body.as_ref().unwrap().blocks {
            trace.work(1);
            match block.terminator.as_ref().unwrap() {
                Terminator::Branch { .. } => units(trace, 4),
                Terminator::Return { values } => units(trace, values.len()),
                _ => unreachable!(),
            }
        }
    }
    if !complete {
        units(trace, bi);
        units(trace, 6 * ei); // Row, source lookup, executable source lookup.
        for function in &output.functions {
            for block in &function.body.as_ref().unwrap().blocks {
                for operation in &block.operations {
                    let operands = match &operation.kind {
                        Kind::Load { .. } => 1,
                        Kind::Store { .. } => 2,
                        Kind::Call { arguments, .. } => arguments.len(),
                        _ => 0,
                    };
                    units(trace, 9 * operands); // Row plus two four-debit lookups.
                }
                if let Some(Terminator::Return { values }) = &block.terminator {
                    units(trace, 7 * values.len());
                }
            }
        }
        trace.leave(scope);
        return;
    }
    for &(lookup, _) in &definitions {
        units(trace, 5 + lookup);
    }
    units(trace, oo);
    for function in &output.functions {
        for block in &function.body.as_ref().unwrap().blocks {
            for operation in &block.operations {
                let operands = match &operation.kind {
                    Kind::Load { .. } => 1,
                    Kind::Store { .. } => 2,
                    Kind::Call { arguments, .. } => arguments.len(),
                    _ => 0,
                };
                units(trace, 10 * operands);
            }
            if let Some(Terminator::Return { values }) = &block.terminator {
                units(trace, 6 * values.len());
            }
        }
    }
    units(trace, 3 * oi);
    for function in &output.functions {
        for block in &function.body.as_ref().unwrap().blocks {
            trace.work(1);
            for operation in &block.operations {
                trace.work(1);
                if !matches!(operation.kind, Kind::Constant(_)) {
                    units(trace, 2);
                }
            }
        }
    }
    trace.leave(scope);
}

pub(super) fn map_census(input: &Module, output: &Module, trace: &mut Trace) {
    let g = Graph::module(input);
    trace.work(sum(&[1, g.operations, g.blocks.len(), g.definitions.len()]));
    for module in [input, output] {
        trace.work(1 + module.functions.len());
        for function in &module.functions {
            let blocks = &function.body.as_ref().unwrap().blocks;
            trace.work(1 + blocks.len());
            for block in blocks {
                trace.work(1 + block.operations.len());
            }
        }
    }
}

pub(super) fn map(input: &Module, output: &Module, events: usize, trace: &mut Trace) {
    let bytes = admission::inverse(input).4;
    let bound = sum(&[product(2, bytes), 64]).min(131_072);
    map_census(input, output, trace);
    let census = history::MapCensus::source(input, output, events);
    trace.work(census.check_work(product(10, bound)));
    let scope = trace.enter(&[]);
    trace.reserve(sum(&[product(1792, bound), 4096]));
    trace.leave(scope);
}

pub(super) fn headers(input_bytes: usize, integer_bytes: usize, trace: &mut Trace) {
    trace.work(sum(&[product(2, input_bytes), 1]));
    trace.work(sum(&[product(2, integer_bytes), 1]));
    for debit in [5, 17, 184, 8, 416, 416, 2, 776] {
        trace.work(debit);
    }
}

pub(super) fn pair(input: &Module, output: &Module, trace: &mut Trace) {
    let a = Graph::module(input).inventory(trace);
    trace.reserve(a);
    let b = Graph::module(output).inventory(trace);
    trace.reserve(b);
    let checked = transition(input, output, trace);
    trace.reserve(checked);
    trace.release(checked);
    trace.release(b);
    trace.release(a);
}

pub(super) fn history(input: &Module, output: &Module, merged: bool, trace: &mut Trace) {
    let rounds = if merged { 2 } else { 1 };
    let scope = trace.enter(&[]);
    trace.reserve(history::meter_header());
    trace.work(4);
    trace.work(3 * rounds);
    trace.reserve(scratch());
    trace.work(256);
    trace.work(257);
    for index in 0..rounds {
        let source = if index == 0 { input } else { output };
        let bytes = admission::inverse(source).4;
        trace.work(1);
        headers(bytes, bytes, trace);
        trace.point(Point::Pair {
            round: index as u16,
            integer: true,
        });
        map(source, source, 0, trace);
        pair(source, source, trace);
        trace.point(Point::Pair {
            round: index as u16,
            integer: false,
        });
        map(
            source,
            output,
            if merged && index == 0 { 4 } else { 0 },
            trace,
        );
        pair(source, output, trace);
        trace.work(sum(&[bytes, admission::inverse(output).4, 1]));
    }
    trace.point(Point::Postflight);
    trace.work(1);
    trace.leave(scope);
}
