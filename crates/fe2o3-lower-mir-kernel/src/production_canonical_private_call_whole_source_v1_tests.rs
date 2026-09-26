use fe2o3_kernel_analysis::{
    CanonicalKirBlockRefV1, CanonicalKirCallRefV1, CanonicalKirDefinitionRefV1,
    CanonicalKirEdgeArgumentRefV1, CanonicalKirEdgeRefV1, CanonicalKirEffectRefV1,
    CanonicalKirFunctionRefV1, CanonicalKirInventoryV1, CanonicalKirKernelRefV1,
    CanonicalKirOperationRefV1, CanonicalKirUseRefV1,
};
use fe2o3_kernel_ir::{
    BlockId, CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1, ValueId,
};

// This closed arithmetic profile retains the actual ordered source spans,
// including zero-emission terminators. Shared typed argument traversal is not
// silently represented by this no-argument profile.
pub(super) struct RootMetadata {
    spans: Vec<std::ops::Range<usize>>,
    terminators: usize,
    operations: usize,
    locals: usize,
    callables: usize,
    launch_names: usize,
}

fn retained_vec<T>(count: usize, trace: &mut Trace) -> usize {
    let bytes = exact_capacity::<T>(count);
    trace.reserve(bytes);
    trace.reserve(0); // Independently asserted exact-capacity premise.
    bytes
}

impl RootMetadata {
    pub(super) fn source(source: &ProductionPreRankedKirOwnerV1) -> Self {
        let module = source.executable().module();
        let [function] = module.functions.as_slice() else {
            panic!("one-root metadata arithmetic profile")
        };
        let [kernel] = module.kernels.as_slice() else {
            panic!("one actual kernel required")
        };
        assert_eq!(function.role, fe2o3_kernel_ir::FunctionRole::KernelEntry);
        assert!(function.signature.parameters.is_empty());
        assert!(function.signature.results.is_empty());
        let body = function.body.as_ref().unwrap();
        assert_eq!(body.blocks.len(), 1);
        let rows = &source.correspondence;
        let [association] = rows.lowered_functions.as_ref() else {
            panic!("one source association required")
        };
        assert!(rows.parameter_bindings.is_empty());
        assert!(rows.parameter_component_bindings.is_empty());
        assert!(rows.ignored_parameter_bindings.is_empty());
        let semantic = source.semantic_ssa().source_semantic();
        assert_eq!(semantic.functions().len(), 1);
        assert_eq!(
            semantic.callables(),
            &[
                fe2o3_mir_model::semantic_mir_v1::SemanticCallableDeclV1::defined(
                    association.semantic_function,
                )
            ]
        );
        let function = &semantic.functions()[association.semantic_function.index() as usize];
        assert!(function.abi().source_input_types().is_empty());
        assert_eq!(source.assert_origins().source_site_count(), 0);
        let raw = rows
            .statement_operation_spans
            .iter()
            .map(|s| {
                (
                    s.kernel_ir_block,
                    s.first_operation_ordinal,
                    s.operation_count,
                )
            })
            .chain(rows.terminator_operation_spans.iter().map(|s| {
                (
                    s.kernel_ir_block,
                    s.first_operation_ordinal,
                    s.operation_count,
                )
            }))
            .chain(rows.synthetic_operation_spans.iter().map(|s| {
                (
                    s.kernel_ir_block,
                    s.first_operation_ordinal,
                    s.operation_count,
                )
            }));
        let spans: Vec<_> = raw
            .map(|(block, first, count)| {
                assert_eq!(block, body.blocks[0].id);
                let first = usize::try_from(first).unwrap();
                first..sum(&[first, usize::try_from(count).unwrap()])
            })
            .collect();
        let operations = body.blocks[0].operations.len();
        let mut occurrences: Vec<_> = spans.iter().flat_map(Clone::clone).collect();
        occurrences.sort_unstable();
        assert_eq!(occurrences, (0..operations).collect::<Vec<_>>());
        Self {
            spans,
            terminators: rows.terminator_operation_spans.len(),
            operations,
            locals: function.locals().len(),
            callables: semantic.callables().len(),
            launch_names: sum(&[
                kernel.id.as_str().len(),
                function
                    .kernel_entry()
                    .unwrap()
                    .export_symbol()
                    .as_bytes()
                    .len(),
            ]),
        }
    }

    pub(super) fn build_rows(&self, trace: &mut Trace) -> usize {
        shared_source::Metadata::root(&self.spans, self.operations).build(trace)
    }

    pub(super) fn check_rows(&self, trace: &mut Trace) {
        shared_source::Metadata::root(&self.spans, self.operations).check(trace)
    }

    fn argument_query(&self, trace: &mut Trace) {
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
        trace.work(product(4, self.locals));
        trace.reserve(product(
            self.locals,
            sum(&[
                size_of::<Option<&SemanticKirIgnoredParameterBindingV1>>(),
                size_of::<bool>(),
            ]),
        ));
        trace.work(self.locals);
        trace.leave(scope);
    }

    pub(super) fn build_arguments(&self, trace: &mut Trace) -> usize {
        self.argument_query(trace);
        let retained = sum(&[
            retained_vec::<ProductionCanonicalRankedArgumentV1>(0, trace),
            retained_vec::<ProductionArgumentProjectionV1>(0, trace),
            retained_vec::<std::ops::Range<usize>>(1, trace),
            retained_vec::<Option<ProductionHelperLocalFrameV1<'_>>>(1, trace),
        ]);
        trace.work(1); // Root has a real None helper-frame row.
        self.argument_query(trace);
        trace.work(1);
        retained
    }

    pub(super) fn check_arguments(&self, trace: &mut Trace) {
        trace.work(6);
        self.argument_query(trace);
    }

    pub(super) fn build_contracts(&self, trace: &mut Trace) -> usize {
        self.contracts().build(trace)
    }

    pub(super) fn check_contracts(&self, trace: &mut Trace) {
        self.contracts().check(trace)
    }

    fn contracts(&self) -> shared_source::Contracts {
        shared_source::Contracts {
            spans: self.spans.len(),
            terminators: self.terminators,
            operations: self.operations,
            callables: self.callables,
            associations: vec![(0, 0)],
            launch_names: vec![self.launch_names],
        }
    }

    pub(super) fn all_components(&self, trace: &mut Trace) {
        let scope = trace.enter(&[]);
        trace.reserve(sum(&[
            size_of::<CrSourceRowsV1<'_>>(),
            size_of::<CrArgumentRowsV1<'_>>(),
            size_of::<CrContractsV1<'_>>()
                - size_of::<fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1>(),
        ]));
        self.build_rows(trace);
        self.build_arguments(trace);
        self.build_contracts(trace);
        self.check_rows(trace);
        self.check_arguments(trace);
        self.check_contracts(trace);
        trace.mark("root metadata component checks complete");
        trace.leave(scope);
    }
}

#[test]
fn whole_source_metadata_zero_emission_spans_and_argument_frames_are_paid() {
    let mut row = RootMetadata {
        spans: vec![0..0],
        terminators: 1,
        operations: 0,
        locals: 1,
        callables: 0,
        launch_names: 10,
    };
    let mut rows = Trace::new(43);
    let scope = rows.enter(&[]);
    let retained = row.build_rows(&mut rows);
    row.check_rows(&mut rows);
    rows.leave(scope);
    assert_eq!(rows.success().work, 31);
    assert_eq!(rows.success().peak, 43 + retained);
    let mut arguments = Trace::new(43);
    let scope = arguments.enter(&[]);
    let retained = row.build_arguments(&mut arguments);
    row.check_arguments(&mut arguments);
    arguments.leave(scope);
    assert_eq!(arguments.success().work, 32);
    assert_eq!(arguments.success().peak, sum(&[
        43, retained, 9,
        size_of::<ScopedSourceCleanupBoundaryV29>(),
        size_of::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>(),
        size_of::<ArgumentEntryV18<'_>>(),
        size_of::<ArgumentQueryCustodyV18>(),
    ]));
    assert_eq!(arguments.run(31, usize::MAX).first_work, Some(32));
    let mut contracts = Trace::new(43);
    let scope = contracts.enter(&[]);
    row.build_contracts(&mut contracts);
    row.check_contracts(&mut contracts);
    contracts.leave(scope);
    // The source codec domain has 44 bytes, including its terminal NUL.
    assert_eq!(contracts.success().work, 346);
    // Four source-catalog scans and one checked-catalog scan also visit
    // Defined callables, although none creates a pipeline catalog row.
    row.callables = 1;
    let mut with_callable = Trace::new(43);
    let scope = with_callable.enter(&[]);
    row.build_contracts(&mut with_callable);
    row.check_contracts(&mut with_callable);
    with_callable.leave(scope);
    assert_eq!(with_callable.success().work, 346 + 4 + 1);
    assert_eq!(with_callable.success().peak, contracts.success().peak);
}

pub(super) struct Projection {
    kernels: usize,
    functions: usize,
    associations: Vec<(usize, usize)>,
}

impl Projection {
    pub(super) fn source(owner: &ProductionPreRankedKirOwnerV1) -> Self {
        let module = owner.executable().module();
        Self {
            kernels: module.kernels.len(),
            functions: module.functions.len(),
            associations: owner
                .correspondence
                .lowered_functions
                .iter()
                .enumerate()
                .map(|(ordinal, row)| {
                    let physical = module
                        .functions
                        .iter()
                        .position(|f| f.id == row.kernel_ir_function)
                        .unwrap();
                    (physical, ordinal)
                })
                .collect(),
        }
    }

    pub(super) fn facts(&self) -> Vec<usize> {
        let mut facts = vec![7];
        facts.extend(std::iter::repeat_n(11, self.kernels));
        facts.extend((0..self.functions).map(|f| {
            sum(&[
                1,
                product(3, self.associations.iter().filter(|r| r.0 == f).count()),
            ])
        }));
        facts
    }

    pub(super) fn build(&self, trace: &mut Trace) -> usize {
        let count = sum(&[1, self.kernels, self.functions]);
        let facts = sum(&self.facts());
        let headers = sum(&[
            size_of::<CrProjectionV1>(),
            size_of::<Vec<(usize, usize)>>(),
        ]);
        trace.reserve(headers);
        let retained = sum(&[
            headers,
            retained_vec::<(CrSubjectV1, CrKindV1, std::ops::Range<usize>)>(count, trace),
            retained_vec::<CrFactV1>(facts, trace),
        ]);
        units(trace, sum(&[8, product(12, self.kernels)]));
        let scope = trace.enter(&[]);
        retained_vec::<(usize, usize)>(self.associations.len(), trace);
        units(trace, self.associations.len());
        let mut ordered = self.associations.clone();
        heap_schedule(&mut ordered, trace, false, false, |_, _| 1);
        for function in 0..self.functions {
            let count = ordered.iter().filter(|r| r.0 == function).count();
            units(trace, count);
            trace.work(1);
            units(trace, product(3, count));
            trace.work(1);
        }
        trace.leave(scope);
        retained
    }

    pub(super) fn check(&self, trace: &mut Trace) {
        trace.work(4);
        units(trace, sum(&[7, product(11, self.kernels)]));
        let scope = trace.enter(&[]);
        trace.reserve(size_of::<Vec<bool>>());
        retained_vec::<bool>(self.associations.len(), trace);
        trace.work(self.associations.len());
        for function in 0..self.functions {
            trace.work(4);
            for _ in self.associations.iter().filter(|r| r.0 == function) {
                trace.work(7);
            }
        }
        trace.work(self.associations.len());
        trace.leave(scope);
    }

    pub(super) fn with_checked<T>(
        &self,
        owner: &ProductionPreRankedKirOwnerV1,
        trace: &mut Trace,
        callback: impl FnOnce(&mut Trace) -> T,
    ) -> T {
        trace.work(2);
        let scope = trace.enter(&[]);
        self.build(trace);
        self.check(trace);
        trace.reserve(sum(&[
            size_of::<Vec<CrInertRowV1<'_>>>(),
            size_of::<CrInertV1<'_, '_>>(),
            size_of::<ProductionCanonicalRankedMetadataV1<'_>>(),
            size_of::<CrGuardV1>(),
            size_of::<ProductionCanonicalRankedSourceViewV1<'_, '_, '_, '_, '_>>(),
        ]));
        let facts = self.facts();
        retained_vec::<CrInertRowV1<'_>>(facts.len(), trace);
        units(trace, facts.len());
        let ranked = admission::Ranked::new(owner.executable().module(), facts);
        let candidate = ranked.candidate(trace);
        trace.reserve(candidate);
        let mut result = None;
        ranked.checked(trace, |trace| result = Some(callback(trace)));
        trace.leave(scope);
        result.unwrap()
    }
}

#[test]
fn whole_source_projection_preserves_function_alias_multiplicity() {
    let single = Projection {
        kernels: 1,
        functions: 1,
        associations: vec![(0, 0)],
    };
    assert_eq!(single.facts(), [7, 11, 4]);
    let mut trace = Trace::new(43);
    let retained = single.build(&mut trace);
    single.check(&mut trace);
    assert_eq!(trace.success().work, 27 + 35);
    assert_eq!(trace.success().storage, 43 + retained);
    assert_eq!(
        trace.success().peak,
        43 + retained + size_of::<Vec<bool>>() + 1
    );
    let shared = Projection {
        kernels: 2,
        functions: 3,
        associations: vec![(0, 0), (2, 1), (1, 2), (2, 3)],
    };
    assert_eq!(shared.facts(), [7, 11, 11, 4, 4, 7]);
}

pub(super) fn root_source<T>(
    owner: &ProductionPreRankedKirOwnerV1,
    trace: &mut Trace,
    callback: impl FnOnce(&mut Trace),
) {
    let metadata = RootMetadata::source(owner);
    raw_source::<T>(
        owner,
        trace,
        |trace| {
            root_call_build(owner, trace);
            trace.work(2);
            trace.work(7);
            metadata.build_rows(trace);
            metadata.build_arguments(trace);
            metadata.build_contracts(trace);
            metadata.check_rows(trace);
            metadata.check_arguments(trace);
            metadata.check_contracts(trace);
        },
        callback,
    );
}

// Independent envelope for the analysis owner's rejected callback value. The
// fixed four retries are the bounded-panic cleanup contract, not sampled work.
fn analysis_callback<T>(custody_work: usize, trace: &mut Trace) {
    type PanicPayload = Box<dyn std::any::Any + Send>;
    let (disposal_work, headers) = if std::mem::needs_drop::<T>() {
        (1 + 4, sum(&[
            size_of::<std::thread::Result<CrResultV1<T>>>(),
            size_of::<std::panic::AssertUnwindSafe<T>>(),
            product(4, size_of::<PanicPayload>()),
            size_of::<std::panic::AssertUnwindSafe<PanicPayload>>(),
            product(2, size_of::<std::thread::Result<()>>()),
            size_of::<std::ops::Range<usize>>(),
            size_of::<usize>(),
        ]))
    } else {
        (0, 0)
    };
    trace.work(sum(&[custody_work, disposal_work]));
    trace.reserve(headers);
}

#[test]
fn whole_source_owned_callback_envelopes_keep_conditional_exact_and_short_boundaries() {
    type Owned = Result<(), Box<dyn std::any::Any + Send>>;
    assert!(!std::mem::needs_drop::<()>());
    assert!(std::mem::needs_drop::<Owned>());
    assert!(std::mem::needs_drop::<CsResultV1<()>>());
    let mut unit = Trace::new(43);
    let scope = unit.enter(&[]);
    analysis_callback::<()>(4, &mut unit);
    analysis_callback::<()>(6, &mut unit);
    unit.leave(scope);
    assert_eq!((unit.success().work, unit.success().storage, unit.success().peak), (10, 43, 43));

    let header = sum(&[
        size_of::<std::thread::Result<CrResultV1<Owned>>>(),
        size_of::<std::panic::AssertUnwindSafe<Owned>>(),
        product(4, size_of::<Box<dyn std::any::Any + Send>>()),
        size_of::<std::panic::AssertUnwindSafe<Box<dyn std::any::Any + Send>>>(),
        product(2, size_of::<std::thread::Result<()>>()),
        size_of::<std::ops::Range<usize>>(),
        size_of::<usize>(),
    ]);
    let mut owned = Trace::new(43);
    let outer = owned.enter(&[]);
    analysis_callback::<Owned>(4, &mut owned);
    let inner = owned.enter(&[]);
    analysis_callback::<Owned>(6, &mut owned);
    owned.leave(inner);
    owned.leave(outer);
    let peak = sum(&[43, product(2, header)]);
    let exact = owned.run(20, peak);
    assert_eq!((exact.work, exact.storage, exact.peak, exact.first_work, exact.first_storage),
        (20, 43, peak, None, None));
    assert_eq!(owned.run(19, peak).first_work, Some(20));
    assert_eq!(owned.run(20, peak - 1).first_storage, Some(peak));
    let paired = owned.run(19, peak - 1);
    assert_eq!((paired.first_work, paired.first_storage), (Some(20), None));
}

pub(super) fn raw_source<T>(
    owner: &ProductionPreRankedKirOwnerV1,
    trace: &mut Trace,
    metadata: impl FnOnce(&mut Trace),
    callback: impl FnOnce(&mut Trace),
) {
    assert_eq!(
        owner.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::RawEmpty
    );
    let projection = Projection::source(owner);
    trace.work(2);
    let outer = trace.enter(&[]);
    trace.work(2);
    // The actual RawEmpty source replay has no caller-ledger parameter. It is
    // still run by the SUT; this is logical debit accounting, not CPU or RSS.
    let analysis = trace.enter(&[]);
    analysis_callback::<T>(4, trace);
    let inventory = Graph::module(owner.executable().module()).inventory(trace);
    trace.reserve(inventory);
    analysis_callback::<T>(6, trace);
    trace.work(2);
    let backing = trace.enter(&[]);
    trace.reserve(sum(&[
        size_of::<CrSourceRowsV1<'_>>(),
        size_of::<CrArgumentRowsV1<'_>>(),
        size_of::<CrContractsV1<'_>>()
            - size_of::<fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1>(),
        size_of::<ProductionCanonicalRankedMetadataV1<'_>>(),
        size_of::<CrGuardV1>(),
        size_of::<std::thread::Result<CrResultV1<T>>>(),
    ]));
    metadata(trace);
    projection.with_checked(owner, trace, callback);
    trace.leave(backing);
    trace.leave(analysis);
    trace.leave(outer);
}

pub(super) fn empty_assertion_coverage(owner: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) {
    use fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1;
    fn grow_push<T>(len: usize, capacity: &mut usize, trace: &mut Trace) {
        if len == *capacity {
            let next = (len + 1).max(*capacity * 2).max(4);
            trace.work(len + 1);
            trace.reserve(exact_capacity::<T>(next));
            trace.reserve(0);
            trace.release(product(*capacity, size_of::<T>()));
            *capacity = next;
        }
        trace.work(1);
    }
    let module = owner.executable().module();
    let graph = Graph::module(module);
    let spans = sum(&[
        owner.correspondence.statement_operation_spans.len(),
        owner.correspondence.terminator_operation_spans.len(),
        owner.correspondence.synthetic_operation_spans.len(),
    ]);
    assert_eq!(owner.assert_origins().source_site_count(), 0);
    trace.work(1); // Source guard query.
    trace.work(1); // Exact native shape owner query.
    let semantic = owner.semantic_ssa().source_semantic();
    for association in &owner.correspondence.lowered_functions {
        trace.work(1);
        let function = &semantic.functions()[association.semantic_function.index() as usize];
        for block in function.blocks() {
            trace.work(1);
            assert!(!matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Assert { .. }
            ));
        }
    }
    trace.work(2);
    trace.reserve(size_of::<Coverage<'_>>());
    retained_vec::<Option<ProductionCanonicalAssertionV1>>(spans, trace);
    retained_vec::<bool>(spans, trace);
    trace.work(product(2, spans));
    let scope = trace.enter(&[]);
    trace.reserve(size_of::<Cleanup<'_, '_>>());
    trace.reserve(size_of::<std::thread::Result<R<()>>>());
    trace.work(1); // Zero pair count is still a paid shape query.
    trace.reserve(sum(&[
        size_of::<AssertGraphIndexV1<'_>>(),
        size_of::<Vec<usize>>(),
    ]));
    let (mut functions, mut blocks, mut capacity) = (0, 0, 0);
    let mut block_count = 0;
    let mut count = 0;
    for (index, function) in module.functions.iter().enumerate() {
        trace.work(1);
        grow_push::<(&str, AssertFunctionCoordinateV1)>(index, &mut functions, trace);
        let body = function.body.as_ref().unwrap();
        for _ in &body.parameters {
            trace.work(1);
            grow_push::<AssertDefinitionIndexV1>(count, &mut capacity, trace);
            count += 1;
        }
        for (block_index, block) in body.blocks.iter().enumerate() {
            assert_eq!(
                block.id.0 as usize, block_index,
                "closed original assertion-index block keys"
            );
            trace.work(1);
            grow_push::<AssertBlockIndexV1>(block_count, &mut blocks, trace);
            block_count += 1;
            for _ in &block.parameters {
                trace.work(1);
                grow_push::<AssertDefinitionIndexV1>(count, &mut capacity, trace);
                count += 1;
            }
            for op in &block.operations {
                trace.work(1);
                for _ in &op.results {
                    trace.work(1);
                    grow_push::<AssertDefinitionIndexV1>(count, &mut capacity, trace);
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, graph.definitions.len());
    let mut names = graph.functions.clone();
    heap(&mut names, trace, true, |a, b| a.len() + b.len() + 1);
    let mut blocks = graph.blocks.clone();
    heap(&mut blocks, trace, true, |_, _| 0);
    let mut definitions = graph.definitions.clone();
    heap(&mut definitions, trace, true, |_, _| 0);
    for pair in names.windows(2) {
        trace.work(1);
        trace.work(pair[0].len() + pair[1].len() + 1);
    }
    units(trace, blocks.len().saturating_sub(1));
    units(trace, count.saturating_sub(1));
    let sparse = if module.functions.len() == 1 {
        SparseRoot::source(module).derive(trace)
    } else {
        shared_source::sparse(module, trace)
    };
    trace.reserve(sparse);
    retained_vec::<usize>(0, trace);
    trace.work(0);
    // None of the semantic functions is skipped without the charged scan.
    units(trace, semantic.functions().len());
    trace.work(0);
    trace.work(1); // join_synthetic's pair query.
    units(trace, spans);
    trace.leave(scope);
}

impl<'a> Graph<'a> {
    pub(super) fn module(module: &'a fe2o3_kernel_ir::Module) -> Self {
        use fe2o3_kernel_ir::{OperationKind, Terminator};
        let mut graph = Self {
            functions: module.functions.iter().map(|f| f.id.as_str()).collect(),
            blocks: Vec::new(),
            definitions: Vec::new(),
            operations: 0,
            used_definitions: Vec::new(),
            successor_blocks: Vec::new(),
            edge_arguments: 0,
            effects: 0,
            callees: Vec::new(),
            entries: module.kernels.iter().map(|k| k.entry.as_str()).collect(),
        };
        for (function_index, function) in module.functions.iter().enumerate() {
            let f = u32::try_from(function_index).unwrap();
            let body = function.body.as_ref().expect("closed definition profile");
            assert_eq!(body.parameters.len(), function.signature.parameters.len());
            graph
                .definitions
                .extend(body.parameters.iter().map(|id| (f, id.0)));
            for (block_index, block) in body.blocks.iter().enumerate() {
                graph.blocks.push((f, u32::try_from(block_index).unwrap()));
                graph
                    .definitions
                    .extend(block.parameters.iter().map(|value| (f, value.id.0)));
                for operation in &block.operations {
                    graph.operations = sum(&[graph.operations, 1]);
                    graph
                        .definitions
                        .extend(operation.results.iter().map(|value| (f, value.id.0)));
                    let uses = match &operation.kind {
                        OperationKind::Constant(_) | OperationKind::Alloca { count: None, .. } => {
                            vec![]
                        }
                        OperationKind::Load { pointer, .. } => {
                            graph.effects = sum(&[graph.effects, 1]);
                            vec![*pointer]
                        }
                        OperationKind::Store { pointer, value, .. } => {
                            graph.effects = sum(&[graph.effects, 1]);
                            vec![*pointer, *value]
                        }
                        OperationKind::Call { callee, arguments } => {
                            graph.callees.push(callee.as_str());
                            arguments.clone()
                        }
                        other => panic!("underived inventory operation: {other:?}"),
                    };
                    if matches!(operation.kind, OperationKind::Alloca { .. }) {
                        graph.effects = sum(&[graph.effects, 1]);
                    }
                    graph
                        .used_definitions
                        .extend(uses.iter().map(|id| (f, id.0)));
                }
                match block.terminator.as_ref().unwrap() {
                    Terminator::Return { values } => graph
                        .used_definitions
                        .extend(values.iter().map(|id| (f, id.0))),
                    Terminator::Branch { target, arguments } => {
                        let target = body.blocks.iter().position(|b| b.id == *target).unwrap();
                        graph
                            .successor_blocks
                            .push((f, u32::try_from(target).unwrap()));
                        graph.edge_arguments = sum(&[graph.edge_arguments, arguments.len()]);
                        graph
                            .used_definitions
                            .extend(arguments.iter().map(|id| (f, id.0)));
                    }
                    other => panic!("underived inventory terminator: {other:?}"),
                }
            }
        }
        graph
    }

    fn inventory_arrays(&self) -> [(usize, usize); 13] {
        [
            (
                self.functions.len(),
                exact_capacity::<CanonicalKirFunctionRefV1<'_>>(self.functions.len()),
            ),
            (
                self.blocks.len(),
                exact_capacity::<CanonicalKirBlockRefV1<'_>>(self.blocks.len()),
            ),
            (
                self.definitions.len(),
                exact_capacity::<CanonicalKirDefinitionRefV1<'_>>(self.definitions.len()),
            ),
            (
                self.operations,
                exact_capacity::<CanonicalKirOperationRefV1<'_>>(self.operations),
            ),
            (
                self.used_definitions.len(),
                exact_capacity::<CanonicalKirUseRefV1>(self.used_definitions.len()),
            ),
            (
                self.successor_blocks.len(),
                exact_capacity::<CanonicalKirEdgeRefV1<'_>>(self.successor_blocks.len()),
            ),
            (
                self.edge_arguments,
                exact_capacity::<CanonicalKirEdgeArgumentRefV1>(self.edge_arguments),
            ),
            (
                self.effects,
                exact_capacity::<CanonicalKirEffectRefV1<'_>>(self.effects),
            ),
            (
                self.callees.len(),
                exact_capacity::<CanonicalKirCallRefV1<'_>>(self.callees.len()),
            ),
            (
                self.entries.len(),
                exact_capacity::<CanonicalKirKernelRefV1<'_>>(self.entries.len()),
            ),
            (
                self.functions.len(),
                exact_capacity::<(&str, CanonicalKirFunctionCoordinateV1)>(self.functions.len()),
            ),
            (
                self.blocks.len(),
                exact_capacity::<(CanonicalKirBlockCoordinateV1, BlockId, usize)>(
                    self.blocks.len(),
                ),
            ),
            (
                self.definitions.len(),
                exact_capacity::<(CanonicalKirFunctionCoordinateV1, ValueId, usize)>(
                    self.definitions.len(),
                ),
            ),
        ]
    }

    pub(super) fn inventory(&self, trace: &mut Trace) -> usize {
        let scope = trace.enter(&[]);
        units(trace, self.visits());
        let header = size_of::<CanonicalKirInventoryV1<'_>>();
        trace.reserve(header);
        let arrays = self.inventory_arrays();
        for (count, bytes) in arrays {
            if count != 0 {
                trace.work(1);
                trace.reserve(bytes);
            }
        }
        units(
            trace,
            sum(&[
                self.visits(),
                self.functions.len(),
                self.blocks.len(),
                self.definitions.len(),
            ]),
        );
        let mut functions = self.functions.clone();
        heap(&mut functions, trace, false, |a, b| {
            a.len().min(b.len()) + 1
        });
        let mut blocks = self.blocks.clone();
        heap(&mut blocks, trace, false, |_, _| 1);
        let mut definitions = self.definitions.clone();
        heap(&mut definitions, trace, false, |_, _| 1);
        for definition in &self.used_definitions {
            trace.work(1);
            find(&definitions, definition, trace, |_, _| 1);
        }
        for target in &self.successor_blocks {
            trace.work(1);
            find(&blocks, target, trace, |_, _| 1);
        }
        for callee in &self.callees {
            trace.work(1);
            find(&functions, callee, trace, |a, b| a.len().min(b.len()) + 1);
        }
        for entry in &self.entries {
            trace.work(1);
            find(&functions, entry, trace, |a, b| a.len().min(b.len()) + 1);
        }
        let retained = sum(&[header, arrays.iter().map(|(_, n)| *n).sum()]);
        trace.leave(scope);
        retained
    }
}

#[test]
fn whole_source_empty_kernel_inventory_is_not_an_empty_module() {
    let graph = Graph {
        functions: vec!["empty"],
        blocks: vec![(0, 0)],
        definitions: vec![],
        operations: 0,
        used_definitions: vec![],
        successor_blocks: vec![],
        edge_arguments: 0,
        effects: 0,
        callees: vec![],
        entries: vec!["empty"],
    };
    let mut trace = Trace::new(29);
    let retained = graph.inventory(&mut trace);
    // Census 5; five live arrays; fill/index rows 7; entry query 1+1+6.
    assert_eq!(trace.success().work, 25);
    assert_eq!(trace.success().storage, 29);
    assert_eq!(trace.success().peak, 29 + retained);
}

pub(super) fn scratch<T>(count: usize, trace: &mut Trace) -> usize {
    trace.work(6);
    let bytes = sum(&[size_of::<Vec<T>>(), exact_capacity::<T>(count)]);
    trace.reserve(bytes);
    trace.reserve(0);
    bytes
}

pub(super) fn root_call_index(source: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) -> usize {
    root_call_index_with(source, trace, |_| {})
}

pub(super) fn root_call_index_with(
    source: &ProductionPreRankedKirOwnerV1,
    trace: &mut Trace,
    callback: impl FnOnce(&mut Trace),
) -> usize {
    let scope = trace.enter(&[]);
    trace.work(2);
    trace.work(1);
    let retained = root_call_build(source, trace);
    trace.work(1);
    trace.mark("root call-index callback");
    callback(trace);
    trace.leave(scope);
    retained
}

fn root_call_build(source: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) -> usize {
    let module = source.executable().module();
    let [function] = module.functions.as_slice() else {
        panic!("one-root call-index component profile")
    };
    let body = function.body.as_ref().unwrap();
    assert_eq!(function.role, fe2o3_kernel_ir::FunctionRole::KernelEntry);
    assert!(function.signature.parameters.is_empty());
    assert!(function.signature.results.is_empty());
    assert_eq!(body.blocks.len(), 1);
    let graph = Graph::module(module);
    assert!(graph.callees.is_empty());
    assert!(graph.successor_blocks.is_empty());
    let rows = &source.correspondence;
    assert_eq!(rows.lowered_functions.len(), 1);
    assert_eq!(rows.call_returns.len(), 1);
    assert_eq!(rows.terminator_operation_spans.len(), 1);
    assert!(rows.parameter_bindings.is_empty());
    assert!(rows.parameter_component_bindings.is_empty());
    assert!(rows.ignored_parameter_bindings.is_empty());
    shared_source::call_index(source, trace)
}

#[derive(Clone, Debug)]
struct LocalPhysical {
    results: Vec<usize>,
    allocations: Vec<bool>,
    memory_pointers: Vec<Option<(u32, u32)>>,
    operation_uses: Vec<usize>,
    block_operations: Vec<std::ops::Range<usize>>,
    terminator_uses: Vec<usize>,
}

pub(super) fn root_physical(module: &fe2o3_kernel_ir::Module, trace: &mut Trace) -> usize {
    use fe2o3_kernel_ir::Terminator;
    let graph = Graph::module(module);
    assert_eq!(graph.functions.len(), 1);
    assert_eq!(graph.blocks.len(), 1);
    let body = module.functions[0].body.as_ref().unwrap();
    assert!(body.parameters.is_empty());
    let block = &body.blocks[0];
    assert!(block.parameters.is_empty());
    assert!(matches!(&block.terminator, Some(Terminator::Return { values }) if values.is_empty()));
    physical(module, trace)
}

pub(super) fn physical(module: &fe2o3_kernel_ir::Module, trace: &mut Trace) -> usize {
    use fe2o3_kernel_ir::{AddressSpace, OperationKind as O, Terminator, Type};
    let graph = Graph::module(module);
    let mut physical = LocalPhysical {
        results: Vec::new(),
        allocations: Vec::new(),
        memory_pointers: Vec::new(),
        operation_uses: Vec::new(),
        block_operations: Vec::new(),
        terminator_uses: Vec::new(),
    };
    for (ordinal, function) in module.functions.iter().enumerate() {
        assert!(
            function
                .signature
                .parameters
                .iter()
                .all(|ty| matches!(ty, Type::Scalar(_)))
        );
        let body = function.body.as_ref().unwrap();
        for block in &body.blocks {
            assert!(block.parameters.is_empty());
            let start = physical.results.len();
            for operation in &block.operations {
                let (allocation, pointer, uses) = match &operation.kind {
                    O::Constant(_) => (false, None, 0),
                    O::Alloca {
                        element,
                        count,
                        address_space,
                        ..
                    } => {
                        assert_eq!(*address_space, AddressSpace::Private);
                        assert!(matches!(element, Type::Scalar(_)) && count.is_none());
                        (true, None, 0)
                    }
                    O::Load { pointer, .. } => {
                        assert_eq!(
                            body.blocks.len(),
                            1,
                            "cross-block memory remains outside this profile"
                        );
                        (false, Some((ordinal as u32, pointer.0)), 1)
                    }
                    O::Store { pointer, .. } => (false, Some((ordinal as u32, pointer.0)), 2),
                    O::Call { arguments, .. } => {
                        assert!(
                            operation
                                .results
                                .iter()
                                .all(|result| matches!(result.ty, Type::Scalar(_)))
                        );
                        (false, None, arguments.len())
                    }
                    _ => panic!("closed direct scalar private arithmetic profile"),
                };
                physical.results.push(operation.results.len());
                physical.allocations.push(allocation);
                physical.memory_pointers.push(pointer);
                physical.operation_uses.push(uses);
            }
            physical
                .block_operations
                .push(start..physical.results.len());
            let uses = match block.terminator.as_ref().unwrap() {
                Terminator::Return { values } => values.len(),
                Terminator::Branch { arguments, .. } => {
                    assert!(arguments.is_empty());
                    0
                }
                _ => panic!("closed scalar physical terminator"),
            };
            physical.terminator_uses.push(uses);
        }
    }
    physical.build(&graph, trace).1
}

pub(super) fn root_private_profile(owner: &ProductionPreRankedKirOwnerV1, trace: &mut Trace) {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticRvalueKindV1 as Rvalue, SemanticStatementKindV1 as Statement,
        SemanticTerminatorKindV1 as Terminator,
    };
    let semantic = owner.semantic_ssa().source_semantic();
    trace.work(1);
    trace.work(2);
    units(trace, semantic.types().len());
    units(trace, owner.correspondence.parameter_bindings.len());
    units(trace, semantic.callables().len());
    for span in &owner.correspondence.statement_operation_spans {
        trace.work(1);
        let function = &semantic.functions()[span.semantic_function.index() as usize];
        let statement = &function.blocks()[span.semantic_block.index() as usize].statements()
            [span.statement_ordinal as usize];
        match statement.kind() {
            Statement::Store(_) => {
                trace.work(2);
                trace.work(span.operation_count as usize);
            }
            Statement::Assign(value) if matches!(value.value().kind(), Rvalue::Load(_)) => {
                trace.work(2);
                trace.work(span.operation_count as usize);
            }
            Statement::Assign(value) if matches!(value.value().kind(), Rvalue::Use(_)) => {}
            Statement::Nop => {}
            _ => panic!("closed root private source statement"),
        }
    }
    for span in &owner.correspondence.terminator_operation_spans {
        trace.work(1);
        let function = &semantic.functions()[span.semantic_function.index() as usize];
        match function.blocks()[span.semantic_block.index() as usize]
            .terminator()
            .kind()
        {
            Terminator::Return => {}
            Terminator::Call(_) => {
                assert_eq!(span.operation_count, 1);
                trace.work(1);
            }
            _ => panic!("closed private source terminator"),
        }
    }
    for span in &owner.correspondence.synthetic_operation_spans {
        trace.work(1);
        assert_eq!(
            span.rule,
            SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage
        );
        units(trace, span.operation_count as usize);
    }
    trace.work(owner.executable().module().kernels.len());
}

impl LocalPhysical {
    // Each modeled allocation is a single direct scalar cell with None extent;
    // there are no GEPs, unknown private pointers or cross-block loads here.
    fn build(&self, graph: &Graph<'_>, trace: &mut Trace) -> (usize, usize) {
        use fe2o3_kernel_analysis::{
            CanonicalKirPrivateMemoryAddressV1 as Address,
            CheckedCanonicalKirPrivateMemoryV1 as Physical,
        };
        let (d, o, b) = (
            graph.definitions.len(),
            graph.operations,
            graph.blocks.len(),
        );
        for n in [
            self.results.len(),
            self.allocations.len(),
            self.memory_pointers.len(),
            self.operation_uses.len(),
        ] {
            assert_eq!(n, o);
        }
        assert_eq!(self.block_operations.len(), b);
        assert_eq!(self.terminator_uses.len(), b);
        let cells = self.allocations.iter().filter(|x| **x).count();
        trace.work(2);
        let inventory_ref = size_of::<&CanonicalKirInventoryV1<'_>>();
        trace.reserve(inventory_ref);
        let constants = scratch::<Option<u64>>(d, trace);
        let addresses = scratch::<Option<Address>>(d, trace);
        let operations = scratch::<bool>(o, trace);
        let anchors = scratch::<Option<usize>>(o, trace);
        trace.work(sum(&[2 * d, 2 * o]));
        for _ in 0..o {
            trace.work(2);
        }
        for allocation in &self.allocations {
            trace.work(3);
            if *allocation {
                for amount in [3, 5, 1] {
                    trace.work(amount);
                }
            }
        }
        for &results in &self.results {
            trace.work(results + 2);
        }
        for _ in 0..d {
            trace.work(2);
        }
        let latest = scratch::<Option<usize>>(cells, trace);
        trace.work(cells);
        let mut definitions = graph.definitions.clone();
        definitions.sort_unstable();
        for (block, range) in self.block_operations.iter().enumerate() {
            trace.work(cells + 1);
            for ordinal in range.clone() {
                trace.work(3);
                if let Some(pointer) = self.memory_pointers[ordinal] {
                    find(&definitions, &pointer, trace, |_, _| 1);
                    trace.work(6);
                    trace.work(4);
                }
                for _ in 0..self.operation_uses[ordinal] {
                    trace.work(3);
                }
            }
            for _ in 0..self.terminator_uses[block] {
                trace.work(2);
            }
        }
        let legacy_paid = sum(&[
            inventory_ref,
            constants,
            addresses,
            operations,
            anchors,
            latest,
        ]);
        let proof = sum(&[
            size_of::<Physical<'_, '_>>(),
            exact_capacity::<Option<Address>>(d),
            exact_capacity::<bool>(o),
            exact_capacity::<Option<usize>>(o),
        ]);
        (legacy_paid, proof)
    }
}

#[test]
fn whole_source_direct_private_cell_changes_definition_lookup_and_payload_terms() {
    let graph = Graph {
        functions: vec!["private"],
        blocks: vec![(0, 0)],
        definitions: vec![(0, 0), (0, 1), (0, 2)],
        operations: 4,
        used_definitions: vec![(0, 1), (0, 0), (0, 1)],
        successor_blocks: vec![],
        edge_arguments: 0,
        effects: 3,
        callees: vec![],
        entries: vec!["private"],
    };
    let physical = LocalPhysical {
        results: vec![1, 1, 0, 1],
        allocations: vec![false, true, false, false],
        memory_pointers: vec![None, None, Some((0, 1)), Some((0, 1))],
        operation_uses: vec![0, 0, 2, 1],
        block_operations: vec![0..4],
        terminator_uses: vec![0],
    };
    let mut trace = Trace::new(29);
    let scope = trace.enter(&[]);
    let (legacy, proof) = physical.build(&graph, &mut trace);
    trace.leave(scope);
    // setup32 + 4D + 12O + R + 9A + C + (C+1)B + 10M
    // + two middle-key lookups (2 each) + 3U + 2T.
    assert_eq!(
        trace.success().work,
        32 + 12 + 48 + 3 + 9 + 1 + 2 + 20 + 4 + 9
    );
    assert_eq!(trace.success().peak, 29 + legacy);
    assert_eq!(trace.success().storage, 29);
    assert!(legacy > proof);
    // Only accessible public layouts are used in the expected storage sums.
    assert_eq!(legacy, 8 + 5 * 24 + 16 * 3 + 56 * 3 + 4 + 16 * 4 + 16);
    assert_eq!(proof, 80 + 56 * 3 + 4 + 16 * 4);
}
