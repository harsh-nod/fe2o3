// Composition keeps successful aggregates distinct from atomic transcripts.
// A query that could fail inside an aggregate is refused, never fabricated as
// one work debit or a simultaneous reservation. Only admitted components merge.
enum Segment {
    Atomic(Trace),
    Complete {
        work: usize,
        peak: usize,
        label: &'static str,
    },
}

pub(in super::super) struct Plan {
    floor: usize,
    live: usize,
    segments: Vec<Segment>,
}

impl Plan {
    pub(in super::super) fn new(floor: usize) -> Self {
        Self {
            floor,
            live: floor,
            segments: Vec::new(),
        }
    }

    pub(in super::super) fn actual<T>(&mut self, build: impl FnOnce(&mut Trace) -> T) -> T {
        let mut trace = Trace::new(self.live);
        let value = build(&mut trace);
        self.live = trace.success().storage;
        self.segments.push(Segment::Atomic(trace));
        value
    }

    fn complete(&mut self, work: usize, peak: usize, label: &'static str) {
        self.segments.push(Segment::Complete { work, peak, label });
    }

    pub(in super::super) fn restore(&mut self, floor: usize) {
        let release = self.live.checked_sub(floor).unwrap();
        self.actual(|trace| trace.release(release));
    }

    pub(in super::super) fn run(
        &self,
        work_limit: usize,
        storage_limit: usize,
    ) -> Result<(usize, usize, usize, Option<usize>, Option<usize>), &'static str> {
        assert!(storage_limit >= self.floor);
        let (mut work, mut live, mut peak) = (0usize, self.floor, self.floor);
        for segment in &self.segments {
            match segment {
                Segment::Atomic(trace) => {
                    let p = trace.run(work_limit.checked_sub(work).unwrap(), storage_limit);
                    peak = peak.max(p.peak);
                    if p.denied_at.is_some() {
                        return Ok((
                            sum(&[work, p.work]),
                            self.floor,
                            peak,
                            p.first_work.map(|n| sum(&[work, n])),
                            p.first_storage,
                        ));
                    }
                    work = sum(&[work, p.work]);
                    live = p.storage;
                }
                Segment::Complete {
                    work: amount,
                    peak: temporary,
                    label,
                } => {
                    let end = sum(&[work, *amount]);
                    let component_peak = sum(&[live, *temporary]);
                    if end > work_limit || component_peak > storage_limit {
                        return Err(label);
                    }
                    work = end;
                    peak = peak.max(component_peak);
                }
            }
        }
        assert_eq!(live, self.floor);
        Ok((work, live, peak, None, None))
    }
}

#[test]
fn whole_plan_refuses_both_opaque_interiors_without_manufacturing_first_denials() {
    let mut plan = Plan::new(43);
    plan.actual(|trace| {
        trace.work(2);
        trace.reserve(5);
    });
    plan.complete(17, 19, "opaque component");
    plan.actual(|trace| trace.work(7));
    plan.restore(43);
    assert_eq!(plan.run(26, 67), Ok((26, 43, 67, None, None)));
    for work in 2..19 {
        assert_eq!(plan.run(work, 67), Err("opaque component"));
    }
    for storage in 48..67 {
        assert_eq!(plan.run(26, storage), Err("opaque component"));
    }
    assert_eq!(plan.run(1, 67), Ok((0, 43, 43, Some(2), None)));
    assert_eq!(plan.run(26, 47), Ok((2, 43, 43, None, Some(48))));
    for work in 19..26 {
        assert_eq!(plan.run(work, 67), Ok((19, 43, 67, Some(26), None)));
    }
}

fn occurrence(module: &fe2o3_kernel_ir::Module) -> OccurrenceCensus {
    let g = Graph::module(module);
    OccurrenceCensus {
        functions: g.functions.len(),
        blocks: g.blocks.len(),
        operations: sum(&[g.operations, g.blocks.len()]),
        values: g.definitions.len(),
        results: module
            .functions
            .iter()
            .flat_map(|f| &f.body.as_ref().unwrap().blocks)
            .flat_map(|b| &b.operations)
            .map(|o| o.results.len())
            .sum(),
        operands: g.used_definitions.len(),
        successors: g.successor_blocks.len(),
        declaration_parameters: 0,
        conditional_branches: 0,
        conditional_operands: 0,
    }
}

fn digest(bytes: usize, trace: &mut Trace) {
    trace.work(sum(&[
        bytes,
        12,
        b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V12/V1\0".len(),
    ]));
}

pub(in super::super) fn plain_import(module: &fe2o3_kernel_ir::Module, trace: &mut Trace) {
    use fe2o3_pliron::OperationHandle;
    use pliron::{context::Ptr, operation::Operation, r#type::TypeHandle};
    type Signature = (Ptr<Operation>, TypeHandle);
    type Witness = (OperationHandle, [usize; 7], Vec<Signature>);
    let bytes = admission::inverse(module).4;
    let census = BridgeCensus::source(module, bytes);
    trace.work(bytes);
    trace.work(census.envelope().work);
    trace.reserve(census.envelope().storage);
    digest(bytes, trace);
    trace.work(census.tree + 1);
    trace.reserve(sum(&[
        size_of::<Witness>(),
        exact_capacity::<Signature>(census.functions),
    ]));
}

fn imported(module: &fe2o3_kernel_ir::Module, bytes: usize, plan: &mut Plan) -> OccurrenceCensus {
    assert_eq!(bytes, admission::inverse(module).4);
    let limits = occurrence(module);
    plan.actual(|trace| {
        plain_import(module, trace);
        trace.work(4);
        trace.work(module.functions.len());
        for function in &module.functions {
            trace.work(1);
            for block in &function.body.as_ref().unwrap().blocks {
                trace.work(1);
                units(trace, block.operations.len() + 1);
            }
        }
        trace.work(limits.envelope().work);
        trace.reserve(limits.envelope().storage);
    });
    limits
}

fn live_census(module: &fe2o3_kernel_ir::Module, bytes: usize) -> BridgeCensus {
    let mut census = BridgeCensus::source(module, bytes);
    for function in &module.functions {
        assert!(
            function
                .signature
                .parameters
                .iter()
                .all(|ty| matches!(ty, fe2o3_kernel_ir::Type::Scalar(_)))
        );
        census.value_type_nodes += function.signature.parameters.len();
    }
    census
}

fn materialize_prelude(
    module: &fe2o3_kernel_ir::Module,
    bytes: usize,
    census: &BridgeCensus,
    trace: &mut Trace,
) {
    trace.work(sum(&[bytes, census.tree]));
    for function in &module.functions {
        for (index, block) in function.body.as_ref().unwrap().blocks.iter().enumerate() {
            assert!(block.parameters.is_empty());
            trace.work(product(
                66,
                if index == 0 {
                    function.signature.parameters.len()
                } else {
                    0
                },
            ));
            for operation in &block.operations {
                trace.work(product(66, operation.results.len()));
            }
            trace.work(0);
        }
    }
    trace.work(census.envelope().work);
    trace.reserve(census.envelope().storage);
}

pub(in super::super) fn unchanged_materialization(
    module: &fe2o3_kernel_ir::Module,
    trace: &mut Trace,
) {
    use fe2o3_pliron::{KirBridgeCorrespondenceV1, KirBridgeOptimizedReceiptV1};
    let (_, canonical, _, _, bytes) = admission::inverse(module);
    let census = live_census(module, bytes);
    let g = Graph::module(module);
    let scope = trace.enter(&[]);
    materialize_prelude(module, bytes, &census, trace);
    trace.mark("final canonical inverse interior cutoff is not derived");
    trace.reserve(canonical);
    digest(bytes, trace);
    trace.reserve(sum(&[
        size_of::<KirBridgeOptimizedReceiptV1>(),
        pushed_capacity::<KirBridgeCorrespondenceV1>(sum(&[
            g.functions.len(),
            2 * g.blocks.len(),
            g.operations,
        ])),
    ]));
    trace.release(census.envelope().storage);
    trace.work(product(2, bytes));
    trace.leave(scope);
}

fn extracted(
    input: &fe2o3_kernel_ir::Module,
    output: &fe2o3_kernel_ir::Module,
    bytes: usize,
    registered: usize,
    integer: bool,
    events: usize,
    plan: &mut Plan,
) -> usize {
    use fe2o3_pliron::{KirBridgeCorrespondenceV1, KirBridgeOptimizedReceiptV1};
    let floor = plan.live;
    let a = Graph::module(input);
    let g = Graph::module(output);
    let census = live_census(output, bytes);
    // Function arguments are native entry-block arguments. The source census
    // counts their signature but the live census also visits those value types.
    let (work, canonical, peak, _, output_bytes) = admission::inverse(output);
    let correspondence = sum(&[
        size_of::<KirBridgeOptimizedReceiptV1>(),
        pushed_capacity::<KirBridgeCorrespondenceV1>(sum(&[
            g.functions.len(),
            2 * g.blocks.len(),
            g.operations,
        ])),
    ]);
    plan.actual(|trace| materialize_prelude(output, bytes, &census, trace));
    plan.complete(
        work,
        peak,
        "canonical inverse interior cutoff is not derived",
    );
    plan.actual(|trace| {
        trace.reserve(canonical);
        digest(output_bytes, trace);
        trace.reserve(correspondence);
        trace.release(census.envelope().storage);
    });
    plan.restore(floor);
    let admitted = sum(&[canonical, correspondence]);
    let endpoints = sum(&[g.operations, g.blocks.len(), g.definitions.len()]);
    let broad = capture_envelope(sum(&[product(2, bytes), 64]).min(131_072));
    let narrow = capture_envelope(capture_nodes(bytes, registered));
    let map = map_retained(
        sum(&[a.operations, a.blocks.len(), a.definitions.len()]),
        events,
        if integer { 2 } else { 8 },
        sum(&[a.operations, a.blocks.len()]),
        sum(&[g.operations, g.blocks.len()]),
    );
    plan.actual(|trace| {
        trace.reserve(admitted);
        units(
            trace,
            sum(&[1, g.functions.len(), g.blocks.len(), g.operations]),
        );
        trace.work(product(g.functions.len(), g.functions.len()));
        trace.reserve(broad.storage);
        trace.work(sum(&[product(4, g.functions.len()), 4]));
        units(trace, sum(&[g.functions.len(), g.blocks.len(), endpoints]));
        replay::map_census(input, output, trace);
        trace.work(
            MapCensus::source(input, output, events)
                .finish_work(product(10, capture_nodes(bytes, registered)), endpoints),
        );
        let finish = trace.enter(&[]);
        trace.reserve(narrow.storage);
        trace.leave(finish);
        trace.reserve(map);
        trace.release(broad.storage);
    });
    plan.restore(floor);
    sum(&[admitted, map])
}

fn wrappers(integer: bool) -> (usize, usize) {
    use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
    use fe2o3_pliron::{
        CheckedNeutralKernelIrOwnerIntegerContinuationV1, CheckedNeutralKernelIrOwnerPolicy3V1,
        KirBridgeOptimizedReceiptV1, KirNeutralOccurrenceRowsV1,
        KirNeutralOptimizationOutputIntegerContinuationV1, KirNeutralOptimizationOutputPolicy3V1,
        KirOptimizationMapV12,
    };
    let components = sum(&[
        size_of::<VerifiedCanonicalKernelIrModuleV12>(),
        size_of::<PlironOptimizationReportV1>(),
        size_of::<KirBridgeOptimizedReceiptV1>(),
        size_of::<KirOptimizationMapV12>(),
        size_of::<KirNeutralOccurrenceRowsV1>(),
    ]);
    let (observed, checked) = if integer {
        (
            size_of::<KirNeutralOptimizationOutputIntegerContinuationV1<'_>>(),
            size_of::<CheckedNeutralKernelIrOwnerIntegerContinuationV1>(),
        )
    } else {
        (
            size_of::<KirNeutralOptimizationOutputPolicy3V1<'_>>(),
            size_of::<CheckedNeutralKernelIrOwnerPolicy3V1>(),
        )
    };
    (
        observed.checked_sub(components).unwrap(),
        checked.checked_sub(components).unwrap(),
    )
}

fn dynamic(module: &fe2o3_kernel_ir::Module, integer: bool, trace: &mut Trace) {
    use pliron::{basic_block::BasicBlock, context::Ptr, operation::Operation, value::Value};
    use std::collections::HashMap;
    type Available = ((Ptr<Operation>, u8), u64, Option<usize>);
    type Visit = Result<Ptr<BasicBlock>, usize>;
    let functions = closed_dynamic_rows(module);
    let scope = trace.enter(&[]);
    trace.work(1);
    trace.reserve(sum(&[
        size_of::<Vec<Ptr<Operation>>>(),
        if integer { size_of::<Vec<Value>>() } else { 0 },
    ]));
    trace.work(2);
    trace.reserve(exact_capacity::<Ptr<Operation>>(4));
    units(trace, 3); // Module container, Graph region and symbol block.
    let mut capacity = 4;
    for index in 0..functions.len() {
        trace.work(1);
        if index == capacity {
            trace.work(capacity + 2);
            trace.reserve(exact_capacity::<Ptr<Operation>>(capacity));
            capacity *= 2;
        }
    }
    trace.work(functions.len());
    for (function, operations) in module.functions.iter().zip(&functions) {
        assert!(operations.iter().filter(|row| row.pure).count() <= 1);
        let blocks = function.body.as_ref().unwrap().blocks.len();
        units(trace, 2 + blocks + operations.len());
        if !integer {
            assert_eq!(
                blocks, 1,
                "scalar CSE executes after the independently derived merges"
            );
            trace.work(1); // Dominator request before entering the SSA scope.
            let region = trace.enter(&[]);
            trace.work(1);
            trace.reserve(sum(&[
                size_of::<HashMap<u64, usize>>(),
                size_of::<Vec<Available>>(),
                size_of::<Vec<Visit>>(),
                size_of::<Vec<Value>>(),
            ]));
            trace.work(2);
            trace.reserve(exact_capacity::<Visit>(4));
            trace.work(1);
            for row in operations {
                trace.work(1);
                trace.work(row.width);
                if row.pure {
                    trace.work(row.width + 1);
                    trace.work(1);
                    trace.work(2);
                    trace.reserve(exact_capacity::<Available>(4));
                    trace.work(2);
                    let mut independent = HashMap::<u64, usize>::new();
                    independent.try_reserve(4).unwrap();
                    assert_eq!(
                        (independent.capacity() + 1).checked_next_power_of_two(),
                        Some(8)
                    );
                    trace.reserve(product(
                        8,
                        sum(&[size_of::<(u64, usize)>(), size_of::<usize>(), 1]),
                    ));
                }
            }
            trace.work(0); // No dominator children in the one-block profile.
            trace.work(1);
            units(trace, operations.iter().filter(|row| row.pure).count());
            trace.leave(region);
        }
        trace.work(0); // No nested operation regions.
    }
    trace.leave(scope);
}

fn checked_stage(
    input: &fe2o3_kernel_ir::Module,
    output: &fe2o3_kernel_ir::Module,
    integer: bool,
    events: usize,
    plan: &mut Plan,
) -> usize {
    use fe2o3_pliron::KirNeutralOwnedOriginStorageV1;
    let floor = plan.live;
    let bytes = admission::inverse(input).4;
    let limits = imported(input, bytes, plan);
    let profile = execution(bytes, limits.nodes(), integer);
    plan.actual(|trace| {
        trace.work(profile.work);
        trace.reserve(sum(&[
            profile.persistent,
            profile.temporary,
            profile.report,
        ]));
    });
    plan.actual(|trace| dynamic(if integer { input } else { output }, integer, trace));
    plan.actual(|trace| {
        trace.release(sum(&[profile.temporary, profile.report]));
        trace.reserve(profile.report);
    });
    let extraction = extracted(input, output, bytes, limits.nodes(), integer, events, plan);
    let rows = capture_occurrence_retained(input);
    let (observed_wrapper, checked_wrapper) = wrappers(integer);
    plan.actual(|trace| {
        trace.reserve(extraction);
        trace.reserve(limits.envelope().storage);
        // Occurrence roster traversal and assembly consume the prepaid local
        // capture meter, with no additional caller-ledger work debit.
        trace.release(limits.envelope().storage.checked_sub(rows).unwrap());
        trace.reserve(observed_wrapper);
        trace.work(if integer { 8 } else { 20 });
        trace.work(if integer { 416 } else { 776 });
    });
    let observed = sum(&[extraction, profile.report, rows, observed_wrapper]);
    plan.restore(floor);
    plan.actual(|trace| {
        trace.reserve(observed);
        if !integer {
            trace.work(1);
        }
        trace.work(1);
        let scratch = trace.enter(&[]);
        let a = Graph::module(input).inventory(trace);
        trace.reserve(a);
        let b = Graph::module(output).inventory(trace);
        trace.reserve(b);
        let checked = replay::transition(input, output, trace);
        trace.reserve(checked);
        trace.reserve(0); // Empty origin callback result has no backing.
        trace.leave(scratch);
        trace.reserve(size_of::<KirNeutralOwnedOriginStorageV1>());
        trace.reserve(checked_wrapper);
        trace.work(bytes);
        trace.reserve(exact_capacity::<u8>(bytes));
        trace.work(2);
    });
    let checked = sum(&[
        observed.checked_sub(observed_wrapper).unwrap(),
        checked_wrapper,
        bytes,
    ]);
    assert_eq!(checked, checked_retained(input, output, integer, events));
    plan.restore(floor);
    checked
}

pub(in super::super) fn root_factory(module: &fe2o3_kernel_ir::Module, plan: &mut Plan) -> usize {
    let mut check = Trace::new(0);
    root_identity_transition(module, &mut check);
    factory(module, module, false, plan)
}

pub(in super::super) fn factory(
    input: &fe2o3_kernel_ir::Module,
    output: &fe2o3_kernel_ir::Module,
    merged: bool,
    plan: &mut Plan,
) -> usize {
    use fe2o3_kernel_opt::{CheckedScalarFixedPointOwnerV1, CheckedScalarFixedPointRoundV1};
    let floor = plan.live;
    plan.actual(|trace| {
        trace.reserve(meter_header());
        trace.work(1);
        trace.reserve(size_of::<CheckedScalarFixedPointOwnerV1>());
        trace.reserve(replay::scratch());
        trace.work(4);
        trace.reserve(exact_capacity::<CheckedScalarFixedPointRoundV1>(16));
        trace.reserve(0);
    });
    let rounds = if merged { 2 } else { 1 };
    let mut retained = Vec::new();
    for index in 0..rounds {
        let source = if index == 0 { input } else { output };
        plan.actual(|trace| trace.work(1));
        let integer = checked_stage(source, source, true, 0, plan);
        plan.actual(|trace| trace.reserve(integer));
        let scalar = checked_stage(
            source,
            output,
            false,
            if merged && index == 0 { 4 } else { 0 },
            plan,
        );
        let bytes = admission::inverse(source).4;
        plan.actual(|trace| {
            trace.reserve(scalar);
            replay::headers(bytes, bytes, trace);
            trace.work(sum(&[bytes, admission::inverse(output).4, 1]));
            trace.work(1);
        });
        retained.extend([integer, scalar]);
    }
    plan.actual(|trace| {
        trace.work(256);
        trace.work(product(3, rounds));
        trace.work(1);
    });
    let (history, _) = history_retained(&retained);
    plan.restore(floor);
    history
}

#[test]
fn whole_execution_composition_refuses_unexpanded_component_cutoffs() {
    let mut plan = Plan::new(43);
    plan.actual(|trace| {
        trace.work(3);
        trace.reserve(7);
    });
    plan.complete(9, 11, "unexpanded");
    plan.actual(|trace| trace.work(2));
    plan.restore(43);
    assert_eq!(plan.run(14, 61), Ok((14, 43, 61, None, None)));
    assert_eq!(plan.run(13, 61), Ok((12, 43, 61, Some(14), None)));
    assert_eq!(plan.run(11, 61), Err("unexpanded"));
    assert_eq!(plan.run(14, 60), Err("unexpanded"));
    assert_eq!(plan.run(2, 61), Ok((0, 43, 43, Some(3), None)));
}
