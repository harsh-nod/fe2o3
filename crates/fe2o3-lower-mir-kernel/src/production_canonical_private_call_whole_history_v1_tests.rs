use fe2o3_pliron::{PlironOptimizationPassReportV1, PlironOptimizationReportV1};

#[derive(Clone, Copy, Debug)]
struct BridgeCensus {
    bytes: usize,
    tree: usize,
    slots: usize,
    functions: usize,
    signature_nodes: usize,
    value_type_nodes: usize,
    edges: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Envelope {
    work: usize,
    storage: usize,
}

impl BridgeCensus {
    fn source(module: &fe2o3_kernel_ir::Module, bytes: usize) -> Self {
        use fe2o3_kernel_ir::Type;
        fn nodes(ty: &Type) -> usize {
            match ty {
                Type::Unit | Type::Scalar(_) => 1,
                Type::Pointer(pointer) => sum(&[1, nodes(&pointer.pointee)]),
                _ => panic!("closed scalar/private bridge type profile"),
            }
        }
        let graph = Graph::module(module);
        let mut signatures = 0;
        let mut values = 0;
        for function in &module.functions {
            for ty in function
                .signature
                .parameters
                .iter()
                .chain(&function.signature.results)
            {
                signatures = sum(&[signatures, nodes(ty)]);
            }
            for block in &function.body.as_ref().unwrap().blocks {
                for parameter in &block.parameters {
                    values = sum(&[values, nodes(&parameter.ty)]);
                }
                for operation in &block.operations {
                    for result in &operation.results {
                        values = sum(&[values, nodes(&result.ty)]);
                    }
                }
            }
        }
        Self {
            bytes,
            tree: sum(&[
                3,
                product(3, graph.functions.len()),
                product(3, graph.blocks.len()),
                product(2, graph.operations),
            ]),
            slots: sum(&[graph.definitions.len(), graph.used_definitions.len()]),
            functions: graph.functions.len(),
            signature_nodes: signatures,
            value_type_nodes: values,
            edges: graph.successor_blocks.len(),
        }
    }

    fn envelope(self) -> Envelope {
        let volume = sum(&[
            1,
            self.tree,
            self.slots,
            self.functions,
            self.signature_nodes,
            self.value_type_nodes,
            self.edges,
        ]);
        Envelope {
            work: sum(&[
                product(4, product(volume, volume)),
                product(8, product(self.bytes, volume)),
                product(8, sum(&[self.bytes, volume])),
            ]),
            storage: sum(&[
                product(64, sum(&[self.bytes, self.tree, self.slots, 1])),
                4096,
            ]),
        }
    }
}

pub(super) fn meter_header() -> usize {
    // The generic parameter is only PhantomData<fn()->E>. Existing optimizer
    // tests bind actual Meter size/alignment to the pinned PriorMeterLayout.
    type Layout = (
        &'static fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'static>,
        usize,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        usize,
        usize,
        bool,
        bool,
        Option<Box<dyn std::any::Any + Send>>,
    );
    assert_eq!((size_of::<Layout>(), align_of::<Layout>()), (64, 8));
    size_of::<Layout>()
}

// Begins immediately after the fixed-point Meter reservation. The last atomic
// debit is the REAL native execution envelope; callers restrict work so that it
// is denied, before any unmodeled pass/extraction/transition work. This is an
// independently computed prefix, never an optimizer cost-helper invocation.
pub(super) fn prepare_import_prefix(
    module: &fe2o3_kernel_ir::Module,
    bytes: usize,
    trace: &mut Trace,
) -> usize {
    use fe2o3_kernel_opt::{CheckedScalarFixedPointOwnerV1, CheckedScalarFixedPointRoundV1};
    use fe2o3_pliron::{
        OperationHandle, PlironOptimizationPassV1, UnauthenticatedPolicy3ExecutionClaimV1,
    };
    use pliron::{context::Ptr, operation::Operation, r#type::TypeHandle};
    type Signature = (Ptr<Operation>, TypeHandle);
    type Witness = (OperationHandle, [usize; 7], Vec<Signature>);
    let graph = Graph::module(module);
    let census = BridgeCensus::source(module, bytes);
    trace.work(1);
    trace.reserve(size_of::<CheckedScalarFixedPointOwnerV1>());
    trace.reserve(sum(&[
        416,
        128,
        size_of::<UnauthenticatedPolicy3ExecutionClaimV1<'_>>(),
        size_of::<[PlironOptimizationPassV1; 8]>(),
        size_of::<[PlironOptimizationPassV1; 2]>(),
        128,
    ]));
    trace.work(4);
    trace.reserve(exact_capacity::<CheckedScalarFixedPointRoundV1>(16));
    trace.reserve(0);
    trace.work(1); // Mandatory first actual round.
    let observation = trace.enter(&[]);
    trace.work(bytes);
    let import = census.envelope();
    trace.work(import.work);
    trace.reserve(import.storage);
    trace.work(sum(&[
        bytes,
        12,
        b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V12/V1\0".len(),
    ]));
    trace.work(census.tree + 1);
    trace.reserve(sum(&[
        size_of::<Witness>(),
        exact_capacity::<Signature>(census.functions),
    ]));
    trace.work(4);
    trace.work(census.functions);
    for function in &module.functions {
        trace.work(1);
        for block in &function.body.as_ref().unwrap().blocks {
            trace.work(1);
            units(trace, block.operations.len() + 1); // Includes actual Return.
        }
    }
    let results = module
        .functions
        .iter()
        .flat_map(|function| &function.body.as_ref().unwrap().blocks)
        .flat_map(|block| &block.operations)
        .map(|operation| operation.results.len())
        .try_fold(0usize, |total, n| total.checked_add(n))
        .unwrap();
    let occurrence = OccurrenceCensus {
        functions: graph.functions.len(),
        blocks: graph.blocks.len(),
        operations: sum(&[graph.operations, graph.blocks.len()]),
        values: graph.definitions.len(),
        results,
        operands: graph.used_definitions.len(),
        successors: graph.successor_blocks.len(),
        declaration_parameters: 0,
        conditional_branches: 0,
        conditional_operands: 0,
    };
    assert!(graph.successor_blocks.is_empty());
    let capture = occurrence.envelope();
    trace.work(capture.work);
    trace.reserve(capture.storage);
    // The real initial occurrence capture uses its prepaid local meter. It does
    // not add a second debit to this caller ledger before entering execution.
    let refused = execution(bytes, occurrence.nodes(), true).work;
    trace.work(refused);
    trace.leave(observation);
    refused
}

#[derive(Clone, Copy, Debug)]
struct OccurrenceCensus {
    functions: usize,
    blocks: usize,
    operations: usize,
    values: usize,
    results: usize,
    operands: usize,
    successors: usize,
    declaration_parameters: usize,
    conditional_branches: usize,
    conditional_operands: usize,
}

// Exact identity-adjacent checker transcript for the no-argument, one-Return
// empty/private roots. This is not a model of SharedTyped's real block merge.
pub(super) fn root_identity_transition(
    module: &fe2o3_kernel_ir::Module,
    trace: &mut Trace,
) -> usize {
    use fe2o3_kernel_analysis::{
        CanonicalKirInventoryV1 as Inventory, CheckedCanonicalKirTransitionV1,
    };
    use fe2o3_kernel_ir::{
        CanonicalKirTransitionCandidateV1, OperationKind as K, ScalarType, Terminator, Type,
    };
    type Literal = (ScalarType, u128);
    type State = (
        &'static Inventory<'static>,
        &'static Inventory<'static>,
        CanonicalKirTransitionCandidateV1<'static>,
        [Vec<usize>; 16],
        [Vec<u8>; 2],
        Vec<Option<Literal>>,
    );
    fn visit_type(ty: &Type, trace: &mut Trace) {
        trace.work(1);
        match ty {
            Type::Unit | Type::Scalar(_) => {}
            Type::Pointer(pointer) => visit_type(&pointer.pointee, trace),
            _ => panic!("closed identity-transition type profile"),
        }
    }
    let g = Graph::module(module);
    assert_eq!(module.functions.len(), 1);
    assert!(module.required_capabilities.is_empty());
    let function = &module.functions[0];
    assert!(function.required_capabilities.is_empty());
    assert!(function.signature.parameters.is_empty());
    assert!(function.signature.results.is_empty());
    let body = function.body.as_ref().unwrap();
    assert_eq!(body.blocks.len(), 1);
    let block = &body.blocks[0];
    assert!(block.parameters.is_empty());
    assert!(matches!(&block.terminator, Some(Terminator::Return { values }) if values.is_empty()));
    for operation in &block.operations {
        assert!(matches!(
            &operation.kind,
            K::Constant(_) | K::Alloca { count: None, .. } | K::Load { .. } | K::Store { .. }
        ));
    }
    let definitions = block
        .operations
        .iter()
        .flat_map(|op| &op.results)
        .collect::<Vec<_>>();
    let (d, o) = (definitions.len(), block.operations.len());
    assert_eq!(g.definitions.len(), d);
    assert!(g.successor_blocks.is_empty());
    let retained = size_of::<CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>>();
    let scope = trace.enter(&[]);
    trace.work(1);
    trace.reserve(retained);
    trace.work(1);
    trace.reserve(size_of::<State>());
    macro_rules! array {
        ($ty:ty, $count:expr) => {{
            let count = $count;
            trace.reserve(exact_capacity::<$ty>(count));
            trace.work(count);
        }};
    }
    // Actual allocation order: two functions, four blocks, two operations,
    // output anchors/flags, parents/literals, incoming block/three edge maps,
    // selected successors, reachability and pending queue. Zero allocations still debit/reserve zero.
    array!(usize, 1);
    array!(usize, 1);
    for _ in 0..4 {
        array!(usize, 1);
    }
    array!(usize, o);
    array!(usize, o);
    array!(usize, d);
    array!(u8, d);
    array!(usize, d);
    array!(Option<Literal>, d);
    array!(usize, 1);
    for _ in 0..3 {
        array!(usize, 0);
    }
    array!(usize, 1); // The single input block's selected successor cache.
    array!(u8, 1);
    array!(usize, 1);
    units(trace, d); // Parent forest initialization; no edges to initialize.

    trace.work(1);
    trace.work(product(2, module.id.as_str().len()));
    trace.work(1);
    for kernel in &module.kernels {
        assert!(kernel.required_capabilities.is_empty());
        trace.work(1);
        trace.work(product(2, kernel.id.as_str().len()));
        trace.work(product(2, kernel.entry.as_str().len()));
        trace.work(1);
    }
    units(trace, 2); // Function row and input-function coordinate.
    trace.work(product(2, function.id.as_str().len()));
    units(trace, 3); // Empty parameter/result types and empty capabilities.
    units(trace, 8); // Identity block segment plus function-entry framing.
    for operation in &block.operations {
        units(trace, 9); // Row/block/origin/block and exact operation payload.
        if let K::Alloca { element, .. } = &operation.kind {
            visit_type(element, trace);
        }
        for result in &operation.results {
            visit_type(&result.ty, trace);
        }
    }
    for definition in &definitions {
        units(trace, 9); // Row/range/descendant, result lookup and two functions.
        visit_type(&definition.ty, trace);
    }
    units(trace, product(3, d)); // Final anchors with one exact descendant.
    trace.work(1); // Output block has no parameter roster.

    for operation in &block.operations {
        trace.work(1);
        if matches!(&operation.kind, K::Constant(_)) {
            assert_eq!(operation.results.len(), 1);
            units(trace, 2); // Root lookup then initial literal publication.
        }
    }
    // All identities match their own initial roots; no union changes a parent.
    // Constants were seeded before the loop, so exactly one iteration executes.
    units(trace, 6); // Iteration, selected edge, reachable.fill(1), function, pop, phi block.
    units(trace, product(3, o)); // Operation row and block coordinate only.
    units(trace, product(11, d)); // Descendant lookup, equal roots, same-root union.
    units(trace, 4); // Connector row/range, input coverage, actual Return.
    units(trace, product(9, d)); // Value obligations with one identity descendant.
    units(trace, o); // No synthesized constant origins.
    units(trace, product(10, g.used_definitions.len())); // Full operation uses.
    units(trace, product(3, o)); // Input ordered-operation roster and block lookup.
    trace.work(1);
    for operation in &block.operations {
        trace.work(1);
        if !matches!(&operation.kind, K::Constant(_)) {
            units(trace, 2);
        }
    }
    trace.leave(scope);
    retained
}

impl OccurrenceCensus {
    fn nodes(self) -> usize {
        sum(&[
            self.functions,
            product(2, self.blocks),
            self.operations,
            product(5, self.values),
            self.results,
            product(2, self.operands),
            product(2, self.successors),
            self.declaration_parameters,
            product(3, self.conditional_branches),
            product(2, self.conditional_operands),
        ])
        .max(1)
    }

    fn envelope(self) -> Envelope {
        let n = self.nodes();
        assert!(n <= 262_144);
        Envelope {
            work: sum(&[product(320, product(n, n)), product(256, n)]),
            storage: sum(&[product(3648, n), 8192]),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Execution {
    work: usize,
    persistent: usize,
    temporary: usize,
    report: usize,
}

fn capture_nodes(bytes: usize, registered: usize) -> usize {
    assert_ne!(registered, 0);
    sum(&[product(2, bytes), 64]).min(131_072).min(registered)
}

fn capture_envelope(nodes: usize) -> Envelope {
    // Separate map-capture profile, not the occurrence-capture profile above.
    Envelope {
        work: sum(&[product(80, product(nodes, nodes)), product(64, nodes)]),
        storage: sum(&[product(1792, nodes), 4096]),
    }
}

fn execution(bytes: usize, registered: usize, integer: bool) -> Execution {
    let q = sum(&[bytes, 32_769]);
    let capture = capture_envelope(capture_nodes(bytes, registered));
    Execution {
        work: sum(&[product(192, q), 25_268_224, capture.work]),
        persistent: sum(&[product(8, q), 4096, capture.storage]),
        temporary: sum(&[product(80, q), 4096]),
        report: sum(&[
            size_of::<PlironOptimizationReportV1>(),
            product(
                if integer { 2 } else { 8 },
                size_of::<PlironOptimizationPassReportV1>(),
            ),
        ]),
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MapCensus {
    nodes: usize,
    events: usize,
    sources: usize,
    input_endpoints: usize,
    output_endpoints: usize,
    functions: usize,
    blocks: usize,
}

// Fresh independent vectors exercise allocation-capacity premises only. They
// contain no production rows, reports or successful observed capacities.
fn pushed_capacity<T>(count: usize) -> usize {
    let mut values = Vec::<std::mem::MaybeUninit<T>>::new();
    for _ in 0..count {
        values.push(std::mem::MaybeUninit::uninit());
    }
    product(values.capacity(), size_of::<T>())
}

fn cloned_capacity<T: Copy>(count: usize) -> usize {
    let source = vec![std::mem::MaybeUninit::<T>::uninit(); count];
    let cloned = source.clone();
    assert_eq!(
        cloned.capacity(),
        count,
        "pinned Vec::clone capacity premise changed"
    );
    product(count, size_of::<T>())
}

fn map_retained(
    nodes: usize,
    events: usize,
    passes: usize,
    sources: usize,
    targets: usize,
) -> usize {
    use fe2o3_pliron::{
        KirOptimizationEndpointV12 as Endpoint, KirOptimizationMapV12, KirOptimizationRelationV12,
        PlironOptimizationPassV1,
    };
    type Node = (Option<Option<u32>>, Option<Endpoint>, Option<u32>);
    type Event = (u8, (u32, [u32; 2]));
    type Pass = (PlironOptimizationPassV1, u64, u64, usize, usize);
    // All three selected fixture profiles synthesize no operations. The actual
    // shared merge's event count must be derived before calling this function;
    // it must NOT reuse the two no-op roots' events=0 premise.
    sum(&[
        size_of::<KirOptimizationMapV12>(),
        cloned_capacity::<Node>(nodes),
        cloned_capacity::<Event>(events),
        exact_capacity::<Option<Endpoint>>(nodes),
        cloned_capacity::<Pass>(passes),
        pushed_capacity::<KirOptimizationRelationV12>(sources),
        pushed_capacity::<Endpoint>(targets),
    ])
}

fn capture_occurrence_retained(module: &fe2o3_kernel_ir::Module) -> usize {
    use fe2o3_kernel_ir::{
        CanonicalKirBlockSegmentV1, CanonicalKirBlockTransitionV1,
        CanonicalKirDefinitionDescendantV1, CanonicalKirDefinitionTransitionV1,
        CanonicalKirEdgeArgumentTransitionV1, CanonicalKirEdgeTransitionV1,
        CanonicalKirFunctionTransitionV1, CanonicalKirOperationTransitionV1,
        CanonicalKirUseTransitionV1,
    };
    let g = Graph::module(module);
    // assemble_occurrence_rows allocates from the complete capture rosters, not
    // final row lengths: operation capacity includes every native terminator,
    // and edge-argument capacity equals ALL captured uses, even with no edges.
    // The selected profiles create no new operations/blocks/definitions.
    sum(&[
        size_of::<fe2o3_pliron::KirNeutralOccurrenceRowsV1>(),
        exact_capacity::<CanonicalKirFunctionTransitionV1>(g.functions.len()),
        exact_capacity::<CanonicalKirBlockTransitionV1>(g.blocks.len()),
        exact_capacity::<CanonicalKirBlockSegmentV1>(g.blocks.len()),
        exact_capacity::<CanonicalKirOperationTransitionV1>(sum(&[g.operations, g.blocks.len()])),
        exact_capacity::<CanonicalKirDefinitionTransitionV1>(g.definitions.len()),
        pushed_capacity::<CanonicalKirDefinitionDescendantV1>(g.definitions.len()),
        exact_capacity::<CanonicalKirUseTransitionV1>(g.used_definitions.len()),
        exact_capacity::<CanonicalKirEdgeTransitionV1>(g.successor_blocks.len()),
        exact_capacity::<CanonicalKirEdgeArgumentTransitionV1>(g.used_definitions.len()),
    ])
}

fn checked_retained(
    input: &fe2o3_kernel_ir::Module,
    output: &fe2o3_kernel_ir::Module,
    integer: bool,
    events: usize,
) -> usize {
    use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
    use fe2o3_pliron::{
        CheckedNeutralKernelIrOwnerIntegerContinuationV1, CheckedNeutralKernelIrOwnerPolicy3V1,
        KirBridgeCorrespondenceV1, KirNeutralOccurrenceRowsV1, KirOptimizationMapV12,
    };
    let source = Graph::module(input);
    let graph = Graph::module(output);
    let (_, _, _, _, input_bytes) = admission::inverse(input);
    let (_, canonical, _, _, _) = admission::inverse(output);
    let correspondence = pushed_capacity::<KirBridgeCorrespondenceV1>(sum(&[
        graph.functions.len(),
        product(2, graph.blocks.len()),
        graph.operations,
    ]));
    let occurrence = capture_occurrence_retained(input)
        .checked_sub(size_of::<KirNeutralOccurrenceRowsV1>())
        .unwrap();
    let common = sum(&[
        canonical
            .checked_sub(size_of::<VerifiedCanonicalKernelIrModuleV12>())
            .unwrap(),
        correspondence,
        occurrence,
        exact_capacity::<u8>(input_bytes), // Exact adjacent input audit copy.
    ]);
    let (header, passes) = if integer {
        (
            size_of::<CheckedNeutralKernelIrOwnerIntegerContinuationV1>(),
            2,
        )
    } else {
        (size_of::<CheckedNeutralKernelIrOwnerPolicy3V1>(), 8)
    };
    let sources = sum(&[source.operations, source.blocks.len()]);
    let nodes = sum(&[sources, source.definitions.len()]);
    let targets = sum(&[graph.operations, graph.blocks.len()]);
    let map = map_retained(nodes, events, passes, sources, targets)
        .checked_sub(size_of::<KirOptimizationMapV12>())
        .unwrap();
    sum(&[
        header,
        common,
        map,
        exact_capacity::<PlironOptimizationPassReportV1>(passes),
    ])
}

fn history_retained(checked: &[usize]) -> (usize, usize) {
    use fe2o3_kernel_opt::{CheckedScalarFixedPointOwnerV1, CheckedScalarFixedPointRoundV1};
    // The actual factory retains full round receipts alongside inline round
    // headers. Do not subtract those inline overlaps or the 16-slot spare space.
    let history = sum(&[
        size_of::<CheckedScalarFixedPointOwnerV1>(),
        exact_capacity::<CheckedScalarFixedPointRoundV1>(16),
        sum(checked),
    ]);
    let additional = sum(&[
        size_of::<ProductionCanonicalScalarFixedPointOwnerV1>(),
        history,
    ]);
    (history, additional)
}

pub(super) fn root_history_retained(module: &fe2o3_kernel_ir::Module) -> [usize; 4] {
    let mut check_profile = Trace::new(0);
    root_identity_transition(module, &mut check_profile);
    let integer = checked_retained(module, module, true, 0);
    let scalar = checked_retained(module, module, false, 0);
    let (history, additional) = history_retained(&[integer, scalar]);
    [integer, scalar, history, additional]
}

pub(super) fn shared_history_retained(
    input: &fe2o3_kernel_ir::Module,
) -> (fe2o3_kernel_ir::Module, [usize; 6]) {
    use fe2o3_kernel_ir::{FunctionRole, OperationKind, Terminator, Type};
    let g = Graph::module(input);
    assert_eq!(
        (
            g.functions.len(),
            g.blocks.len(),
            g.operations,
            g.definitions.len(),
            g.used_definitions.len(),
            g.successor_blocks.len(),
            g.edge_arguments
        ),
        (3, 5, 2, 5, 3, 2, 0)
    );
    assert_eq!(input.kernels.len(), 2);
    let mut output = input.clone();
    let mut merged = 0;
    for function in &mut output.functions {
        assert_eq!(
            function.signature.parameters,
            [Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)]
        );
        let body = function.body.as_mut().unwrap();
        assert_eq!(body.parameters.len(), 1);
        assert!(body.blocks.iter().all(|block| block.parameters.is_empty()));
        if function.role == FunctionRole::InternalHelper {
            assert_eq!(
                function.signature.results,
                [Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)]
            );
            assert_eq!(body.blocks.len(), 1);
            assert!(body.blocks[0].operations.is_empty());
            assert!(
                matches!(&body.blocks[0].terminator, Some(Terminator::Return { values })
                if values == &body.parameters)
            );
            continue;
        }
        assert!(function.signature.results.is_empty());
        assert_eq!(body.blocks.len(), 2);
        assert_eq!(body.blocks[0].operations.len(), 1);
        assert!(
            matches!(&body.blocks[0].operations[0].kind, OperationKind::Call { arguments, .. }
            if arguments == &body.parameters)
        );
        assert!(body.blocks[1].operations.is_empty());
        assert!(
            matches!(&body.blocks[0].terminator, Some(Terminator::Branch { target, arguments })
            if *target == body.blocks[1].id && arguments.is_empty())
        );
        assert!(
            matches!(&body.blocks[1].terminator, Some(Terminator::Return { values }) if values.is_empty())
        );
        body.blocks[0].terminator = body.blocks[1].terminator.clone();
        body.blocks.truncate(1);
        merged += 1;
    }
    assert_eq!(merged, 2);
    // Pinned simplify_cfg try_merge_succ erases each zero-result Branch, moves
    // its existing Return, and erases an already-empty argument-free block.
    // Map capture records Erase+Move only: reinsertion registers a live old key.
    let i0 = checked_retained(input, input, true, 0);
    let s0 = checked_retained(input, &output, false, product(2, merged));
    let i1 = checked_retained(&output, &output, true, 0);
    let s1 = checked_retained(&output, &output, false, 0);
    let (history, additional) = history_retained(&[i0, s0, i1, s1]);
    (output, [i0, s0, i1, s1, history, additional])
}

impl MapCensus {
    pub(super) fn source(
        input: &fe2o3_kernel_ir::Module,
        output: &fe2o3_kernel_ir::Module,
        events: usize,
    ) -> Self {
        let a = Graph::module(input);
        let b = Graph::module(output);
        let source = sum(&[a.operations, a.blocks.len()]);
        Self {
            nodes: sum(&[source, a.definitions.len()]),
            events,
            sources: source,
            input_endpoints: sum(&[source, a.definitions.len()]),
            output_endpoints: sum(&[b.operations, b.blocks.len(), b.definitions.len()]),
            functions: sum(&[a.functions.len(), b.functions.len()]),
            blocks: sum(&[a.blocks.len(), b.blocks.len()]),
        }
    }

    pub(super) fn check_work(self, target_cap: usize) -> usize {
        let order = self
            .nodes
            .max(self.input_endpoints)
            .max(self.output_endpoints);
        let log = (usize::BITS - order.leading_zeros()) as usize;
        let targets = target_cap.min(product(self.sources, self.nodes));
        let visits = product(self.sources + 1, sum(&[self.nodes, self.events, 1]));
        let sorting = product(
            sum(&[
                product(4, self.nodes),
                self.input_endpoints,
                self.output_endpoints,
                targets,
            ]),
            log + 1,
        );
        product(
            128,
            sum(&[
                1,
                self.functions,
                self.blocks,
                self.nodes,
                self.events,
                visits,
                sorting,
            ]),
        )
    }

    pub(super) fn finish_work(self, target_cap: usize, roster: usize) -> usize {
        sum(&[
            product(2, self.check_work(target_cap)),
            product(128, product(roster, self.nodes + 1)),
        ])
    }
}

// Only closed one-block definitions with at most one pure expression. The
// source-to-native schema supplies widths; no native pass result is an input.
#[derive(Clone, Copy)]
struct NativeRow {
    width: usize,
    pure: bool,
}

fn dynamic_storage(functions: &[Vec<NativeRow>]) -> (usize, usize) {
    use pliron::{basic_block::BasicBlock, context::Ptr, operation::Operation, value::Value};
    use std::collections::HashMap;
    assert_eq!(size_of::<usize>(), 8);
    assert_eq!(align_of::<usize>(), 8);
    // Explicitly conditional on dialect-gpu's companion test
    // private_call_whole_entry_pinned_cse_layout_equivalence_premises. These
    // accessible field-shape types are NOT portable private-layout surrogates.
    type KeyPremise = (Ptr<Operation>, u8);
    type AvailablePremise = (KeyPremise, u64, Option<usize>);
    type VisitPremise = Result<Ptr<BasicBlock>, usize>;
    let pending_count = functions.len().max(4).checked_next_power_of_two().unwrap();
    let pending = sum(&[
        size_of::<Vec<Ptr<Operation>>>(),
        exact_capacity::<Ptr<Operation>>(pending_count),
    ]);
    let integer = sum(&[pending, size_of::<Vec<Value>>()]);
    let mut region_peak = 0;
    for operations in functions {
        let pure = operations.iter().filter(|row| row.pure).count();
        assert!(pure <= 1);
        let mut region = sum(&[
            size_of::<HashMap<u64, usize>>(),
            size_of::<Vec<AvailablePremise>>(),
            size_of::<Vec<VisitPremise>>(),
            size_of::<Vec<Value>>(),
            exact_capacity::<VisitPremise>(4),
        ]);
        if pure != 0 {
            // Independently allocate only to assert the actual-capacity premise;
            // this is not an invocation or observation of the CSE transform.
            let mut table = HashMap::<u64, usize>::new();
            table.try_reserve(4).unwrap();
            let actual_slots = (table.capacity() + 1).checked_next_power_of_two().unwrap();
            assert_eq!(
                actual_slots, 8,
                "pinned hash-table envelope premise changed"
            );
            region = sum(&[
                region,
                exact_capacity::<AvailablePremise>(4),
                product(8, sum(&[size_of::<(u64, usize)>(), size_of::<usize>(), 1])),
            ]);
        }
        region_peak = region_peak.max(region);
    }
    (integer, pending + region_peak)
}

fn dynamic_work(functions: &[Vec<NativeRow>]) -> (usize, usize) {
    let mut integer = Trace::new(0);
    integer.work(1);
    integer.work(2); // Initial pending capacity four.
    integer.work(1); // Module operation.
    integer.work(1); // Module Graph region, not an SSA region.
    integer.work(1); // Module symbol block.
    let mut pending_capacity = 4usize;
    for ordinal in 0..functions.len() {
        integer.work(1);
        if ordinal == pending_capacity {
            integer.work(pending_capacity + 2);
            pending_capacity = product(pending_capacity, 2);
        }
    }
    integer.work(functions.len());
    for operations in functions {
        integer.work(1); // Function container.
        integer.work(1); // Its SSA region.
        integer.work(1); // Its single block.
        units(&mut integer, operations.len());
        integer.work(0); // No nested operation regions.
    }
    // CSE uses the identical outer traversal, then one SSA-region dispatch.
    let mut scalar = Trace::new(0);
    scalar.work(integer.success().work);
    for operations in functions {
        assert!(operations.iter().filter(|row| row.pure).count() <= 1);
        scalar.work(1); // Request the existing dominator tree.
        scalar.work(1); // Enter eliminate_region.
        scalar.work(2); // Initial visits capacity four.
        scalar.work(1); // Pop Enter(block).
        for row in operations {
            scalar.work(1);
            scalar.work(row.width);
            if row.pure {
                scalar.work(row.width + 1); // Fingerprint this first key.
                scalar.work(1); // Retain its unique available definition.
                scalar.work(2); // Initial available capacity four.
                scalar.work(2); // Initial hash-table request four.
            }
        }
        scalar.work(0); // No dominator children.
        scalar.work(1); // Pop Exit(block).
        units(
            &mut scalar,
            operations.iter().filter(|row| row.pure).count(),
        );
    }
    (integer.success().work, scalar.success().work)
}

fn closed_dynamic_rows(module: &fe2o3_kernel_ir::Module) -> Vec<Vec<NativeRow>> {
    use fe2o3_kernel_ir::{OperationKind, Terminator};
    let functions = module
        .functions
        .iter()
        .map(|function| {
            let body = function.body.as_ref().expect("closed definition profile");
            body.blocks
                .iter()
                .flat_map(|block| {
                    let mut rows: Vec<_> = block
                        .operations
                        .iter()
                        .map(|operation| {
                            let (attributes, operands, pure) = match &operation.kind {
                                OperationKind::Constant(_) => (1, 0, true),
                                OperationKind::Alloca { count, .. } => {
                                    assert!(count.is_none());
                                    (1, 0, false)
                                }
                                OperationKind::Load { .. } => (3, 1, false),
                                OperationKind::Store { .. } => (3, 2, false),
                                // CallOp imports both gpu_call_callee and gpu_call_signature.
                                // CSE charges structured width before rejecting an effectful call.
                                OperationKind::Call { arguments, .. } => {
                                    (2, arguments.len(), false)
                                }
                                other => panic!("underived dynamic native operation: {other:?}"),
                            };
                            NativeRow {
                                width: sum(&[operation.results.len(), operands, attributes]),
                                pure,
                            }
                        })
                        .collect();
                    let width = match block.terminator.as_ref().unwrap() {
                        Terminator::Return { values } => values.len(),
                        Terminator::Branch { arguments, .. } => arguments.len(),
                        _ => panic!("closed dynamic native oracle requires Return/Branch"),
                    };
                    rows.push(NativeRow { width, pure: false });
                    rows
                })
                .collect()
        })
        .collect::<Vec<_>>();
    functions
}

pub(super) fn closed_dynamic_work(module: &fe2o3_kernel_ir::Module) -> (usize, usize) {
    assert!(
        module
            .functions
            .iter()
            .all(|f| f.body.as_ref().unwrap().blocks.len() == 1)
    );
    dynamic_work(&closed_dynamic_rows(module))
}

#[test]
fn whole_history_visible_integer_and_cse_work_is_separate_from_opaque_envelopes() {
    let row = |width, pure| NativeRow { width, pure };
    assert_eq!(dynamic_work(&[vec![row(0, false)]]), (12, 19));
    assert_eq!(
        dynamic_work(&[vec![
            row(2, false), // Preserved Alloca, one result and one kind attribute.
            row(2, true),  // U64 constant, one result and one value attribute.
            row(5, false), // Store, two operands and three attributes.
            row(5, false), // Load, one result/operand and three attributes.
            row(0, false), // Return.
        ]]),
        (16, 50)
    );
    assert_eq!(
        dynamic_work(&[
            vec![row(3, false), row(0, false)],
            vec![row(1, false)],
            vec![row(3, false), row(0, false)],
        ]),
        (26, 56)
    );
    // Actual SharedTyped calls each have one result, one argument and both
    // imported attributes. Each scalar round pays two more width units than
    // the generic width-three control above; the two-round factory pays four.
    assert_eq!(
        dynamic_work(&[
            vec![row(1 + 1 + 2, false), row(0, false)],
            vec![row(1, false)],
            vec![row(1 + 1 + 2, false), row(0, false)],
        ]),
        (26, 56 + 2)
    );
    // Crossing the actual pending Vec's four-slot capacity adds its six-work
    // allocation request; each extra empty definition adds six traversal work.
    let four = (0..4).map(|_| vec![row(0, false)]).collect::<Vec<_>>();
    let five = (0..5).map(|_| vec![row(0, false)]).collect::<Vec<_>>();
    assert_eq!(dynamic_work(&four).0, 30);
    assert_eq!(dynamic_work(&five).0, 42);
}

#[test]
fn whole_history_visible_storage_uses_explicit_external_layout_premises() {
    use pliron::{context::Ptr, operation::Operation, value::Value};
    let empty = vec![NativeRow {
        width: 0,
        pure: false,
    }];
    let constant = vec![NativeRow {
        width: 2,
        pure: true,
    }];
    let (integer, cse) = dynamic_storage(&[empty]);
    assert_eq!(
        integer,
        size_of::<Vec<Ptr<Operation>>>()
            + size_of::<Vec<Value>>()
            + 4 * size_of::<Ptr<Operation>>()
    );
    let (integer_constant, cse_constant) = dynamic_storage(&[constant]);
    assert_eq!(integer_constant, integer);
    type AvailablePremise = ((Ptr<Operation>, u8), u64, Option<usize>);
    assert_eq!(
        cse_constant - cse,
        4 * size_of::<AvailablePremise>() + 8 * (size_of::<(u64, usize)>() + 8 + 1)
    );
}

#[test]
fn whole_history_empty_kernel_native_profiles_are_source_expansions() {
    // b is a symbolic byte input here, not a successful serialized observation.
    let b = 300;
    let bridge = BridgeCensus {
        bytes: b,
        tree: 3 + 3 + 1 + 2,
        slots: 0,
        functions: 1,
        signature_nodes: 0,
        value_type_nodes: 0,
        edges: 0,
    };
    assert_eq!(
        bridge.envelope(),
        Envelope {
            work: 572 + 96 * b,
            storage: 4736 + 64 * b,
        }
    );
    let occurrence = OccurrenceCensus {
        functions: 1,
        blocks: 1,
        operations: 1,
        values: 0,
        results: 0,
        operands: 0,
        successors: 0,
        declaration_parameters: 0,
        conditional_branches: 0,
        conditional_operands: 0,
    };
    assert_eq!(occurrence.nodes(), 4);
    assert_eq!(
        occurrence.envelope(),
        Envelope {
            work: 6144,
            storage: 22784
        }
    );
    assert_eq!(
        capture_envelope(4),
        Envelope {
            work: 1536,
            storage: 11264
        }
    );
    let integer = execution(b, 4, true);
    let scalar = execution(b, 4, false);
    assert_eq!(integer.work, 192 * (b + 32769) + 25268224 + 1536);
    assert_eq!(scalar.work, integer.work);
    assert_eq!(scalar.persistent, 8 * (b + 32769) + 4096 + 11264);
    assert_eq!(integer.persistent, scalar.persistent);
    assert_eq!(scalar.temporary, 80 * (b + 32769) + 4096);
    assert_eq!(integer.temporary, scalar.temporary);
    assert_eq!(
        scalar.report - integer.report,
        6 * size_of::<PlironOptimizationPassReportV1>()
    );
    let map = MapCensus {
        nodes: 1,
        events: 0,
        sources: 1,
        input_endpoints: 1,
        output_endpoints: 1,
        functions: 2,
        blocks: 2,
    };
    // rows=1+2+2+1 + (1+1)*(1+0+1) + (4+1+1+1)*(1+1).
    assert_eq!(map.check_work(40), 128 * 24);
    assert_eq!(map.finish_work(40, 1), 2 * (128 * 24) + 128 * 2);
}
