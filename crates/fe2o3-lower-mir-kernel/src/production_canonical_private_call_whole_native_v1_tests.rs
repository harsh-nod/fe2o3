// Native analysis units are not canonical Budget byte reservations. No value
// below is a sampled receipt or an implementation envelope-helper invocation.
use fe2o3_kernel_analysis::KernelCheckPassKindV1 as Pass;
use fe2o3_pliron::ProductionAnalysisResourcePhaseV1 as Phase;

const PASS_ORDER: [Pass; 9] = [
    Pass::TensorLayout,
    Pass::MemoryBounds,
    Pass::AtomicLegality,
    Pass::RaceFreedom,
    Pass::HierarchicalOwnership,
    Pass::BarrierConvergence,
    Pass::PipelineProtocol,
    Pass::WorkgroupMemory,
    Pass::SemanticRefinement,
];

// These field-type surrogates have required same-host companion tests in the
// private facade test host. They are not portable repr(Rust) layout claims.
type GuardLayout = (
    usize,
    fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    usize,
    std::cell::Cell<Option<Result<ArgumentResourceV1, usize>>>,
);
type TerminalLayout = (
    &'static fe2o3_kernel_analysis::CanonicalKirInventoryV1<'static>,
    Vec<fe2o3_pliron::CanonicalTrapPairV1>,
    Vec<fe2o3_pliron::CanonicalTrapIncomingEdgeV1<'static, 'static>>,
    Vec<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>,
    Option<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>,
);

fn rows<T>(count: usize, trace: &mut Trace) {
    trace.work(1);
    trace.reserve(sum(&[size_of::<Vec<T>>(), exact_capacity::<T>(count)]));
    trace.reserve(0);
}

fn empty_terminal(module: &fe2o3_kernel_ir::Module, trace: &mut Trace) {
    let graph = Graph::module(module);
    for function in &module.functions {
        for block in &function.body.as_ref().unwrap().blocks {
            assert!(matches!(
                block.terminator,
                Some(
                    fe2o3_kernel_ir::Terminator::Return { .. }
                        | fe2o3_kernel_ir::Terminator::Branch { .. }
                )
            ));
        }
    }
    trace.reserve(size_of::<TerminalLayout>());
    rows::<fe2o3_pliron::CanonicalTrapPairV1>(graph.blocks.len(), trace);
    rows::<fe2o3_pliron::CanonicalTrapIncomingEdgeV1<'_, '_>>(graph.successor_blocks.len(), trace);
    rows::<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>(graph.functions.len(), trace);
    units(trace, graph.functions.len() + graph.blocks.len());
    for callee in &graph.callees {
        assert!(!callee.starts_with("__fe2o3_ir_"));
        trace.work(1);
        trace.work(product(8, callee.len() + 2));
    }
}

pub(super) fn empty_trap_shape<T>(
    module: &fe2o3_kernel_ir::Module,
    trace: &mut Trace,
    callback: impl FnOnce(&mut Trace),
) {
    use fe2o3_pliron::{CanonicalRankedPolicyFailureV1 as Error, CheckedCanonicalTrapShapeV1};
    trace.work(2);
    let scope = trace.enter(&[]);
    trace.work(1); // Foundation inventory query before any scratch reservation.
    trace.reserve(sum(&[
        size_of::<CheckedCanonicalTrapShapeV1<'_, '_>>(),
        size_of::<GuardLayout>(),
        product(2, size_of::<std::thread::Result<Result<T, Error>>>()),
    ]));
    empty_terminal(module, trace);
    callback(trace);
    trace.leave(scope);
}

pub(super) fn physical(module: &fe2o3_kernel_ir::Module, trace: &mut Trace) -> usize {
    use fe2o3_kernel_analysis::{
        CanonicalKirPrivateMemoryErrorV1, CanonicalKirPrivateMemoryStorageV1,
        CheckedCanonicalKirPrivateMemoryV1 as Physical,
    };
    type Cleanup = (
        &'static mut ArgumentBudgetV1<'static>,
        usize,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        usize,
        bool,
    );
    type Result = std::result::Result<
        (
            Physical<'static, 'static>,
            CanonicalKirPrivateMemoryStorageV1,
        ),
        CanonicalKirPrivateMemoryErrorV1,
    >;
    trace.work(4);
    let scope = trace.enter(&[]);
    trace.reserve(size_of::<Cleanup>());
    trace.reserve(size_of::<std::thread::Result<Result>>());
    trace.reserve(size_of::<std::thread::Result<()>>());
    let proof = source::physical(module, trace);
    trace.work(4);
    trace.leave(scope);
    proof
}

pub(super) fn schema_work(module: &fe2o3_kernel_ir::Module) -> usize {
    use fe2o3_kernel_ir::OperationKind as Kind;
    fn attributes(keys: &[&str]) -> usize {
        sum(&[1, keys.iter().map(|key| key.len() + keys.len()).sum()])
    }
    let mut work = attributes(&["sym_name"]);
    for (ordinal, function) in module.functions.iter().enumerate() {
        let name = format!("kir_fn_{ordinal}");
        work = sum(&[work, 1, attributes(&["sym_name", "func_type"]), name.len()]);
        for block in &function.body.as_ref().unwrap().blocks {
            work += 1;
            for operation in &block.operations {
                let keys: &[&str] = match operation.kind {
                    Kind::Constant(_) => &["gpu_constant_value"],
                    Kind::Alloca { .. } => &["gpu_preserved_operation_kind"],
                    Kind::Load { .. } => &[
                        "gpu_load_address_space",
                        "gpu_load_alignment",
                        "gpu_load_volatile",
                    ],
                    Kind::Store { .. } => &[
                        "gpu_store_address_space",
                        "gpu_store_alignment",
                        "gpu_store_volatile",
                    ],
                    Kind::Call { .. } => &["gpu_call_callee", "gpu_call_signature"],
                    _ => panic!("closed root schema profile"),
                };
                work = sum(&[work, 1, attributes(keys)]);
            }
            work = sum(&[work, 1, attributes(&[])]); // Return.
        }
    }
    for kernel in &module.kernels {
        work += 1 + kernel.entry.as_str().len();
        for (ordinal, function) in module.functions.iter().enumerate() {
            work += 1 + function.id.as_str().len();
            if function.id == kernel.entry {
                work = sum(&[
                    work,
                    module.functions.len(),
                    ordinal + 1,
                    format!("kir_fn_{ordinal}").len(),
                ]);
                break;
            }
        }
    }
    work
}

struct Call<'a> {
    function: usize,
    block: usize,
    operation: usize,
    target: usize,
    name: &'a str,
    slots: usize,
}

fn calls(module: &fe2o3_kernel_ir::Module) -> Vec<Call<'_>> {
    let mut result = Vec::new();
    for (function, f) in module.functions.iter().enumerate() {
        for (block, b) in f.body.as_ref().unwrap().blocks.iter().enumerate() {
            for (operation, op) in b.operations.iter().enumerate() {
                if let fe2o3_kernel_ir::OperationKind::Call { callee, arguments } = &op.kind {
                    let target = module
                        .functions
                        .iter()
                        .position(|f| f.id == *callee)
                        .unwrap();
                    result.push(Call {
                        function,
                        block,
                        operation,
                        target,
                        name: callee.as_str(),
                        slots: arguments.len() + op.results.len(),
                    });
                }
            }
        }
    }
    result
}

fn function_lookup(count: usize, ordinal: usize, trace: &mut Trace) {
    trace.work(count);
    trace.work(ordinal + 1);
}

fn call_joins(count: usize, calls: &[Call<'_>], trace: &mut Trace) {
    for call in calls {
        trace.work(call.block + call.operation + 2);
        trace.work(call.name.len() + 1);
        function_lookup(count, call.function, trace);
        trace.work(1);
        function_lookup(count, call.target, trace);
    }
}

pub(super) fn root_memory_facade<T>(
    module: &fe2o3_kernel_ir::Module,
    trace: &mut Trace,
    callback: impl FnOnce(&mut Trace),
) {
    use fe2o3_kernel_analysis::{
        CanonicalKirCallEffectsV1, CanonicalKirPrivateCellCensusV1,
        CheckedCanonicalKirPrivateMemoryV1 as Physical,
    };
    use fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1;
    use fe2o3_pliron::{
        CanonicalPrivatePipelineReportV1, CanonicalRankedPolicyFailureV1 as Error,
        CanonicalRankedPolicyHistoryV1, CheckedCanonicalPrivateMemoryPoliciesV1, KirPlironGraphV12,
        OperationHandle,
    };
    use pliron::{
        builtin::ops::FuncOp,
        context::{Context, Ptr},
        operation::Operation,
        r#type::TypeHandle,
    };
    type Analysis = (
        ([usize; 2], [usize; 3]),
        [usize; 2],
        Option<(Phase, &'static str)>,
        bool,
        Option<CanonicalRankedPolicyHistoryV1>,
    );
    type Report = (
        (CanonicalPrivatePipelineReportV1, [usize; 3]),
        CanonicalRankedPolicyHistoryV1,
    );
    type Facts = (
        &'static fe2o3_kernel_analysis::CanonicalKirInventoryV1<'static>,
        Result<CanonicalKirPrivateCellCensusV1<'static, 'static>, Physical<'static, 'static>>,
        CanonicalKirCallEffectsV1<'static, 'static>,
        Vec<usize>,
        Option<&'static ()>,
    );
    type Witness = (
        OperationHandle,
        [usize; 7],
        Vec<(Ptr<Operation>, TypeHandle)>,
    );
    type Terminal = (Vec<CanonicalKirFunctionCoordinateV1>, Option<TypeHandle>);
    type Projection = (KirPlironGraphV12<'static>, Witness, u64, Option<Terminal>);
    type PrivateProjection = (Projection, &'static ());
    type Row = (Ptr<Operation>, u8);
    type Admission = (
        &'static (),
        &'static Context,
        &'static FuncOp,
        Vec<Row>,
        usize,
        u64,
    );
    let graph = Graph::module(module);
    let count = graph.functions.len();
    let calls = calls(module);
    trace.work(2);
    let scope = trace.enter(&[]);
    trace.work(1);
    trace.reserve(sum(&[
        size_of::<Analysis>(),
        size_of::<GuardLayout>(),
        size_of::<CheckedCanonicalPrivateMemoryPoliciesV1<'_, '_>>(),
        size_of::<std::thread::Result<Result<T, Error>>>(),
        size_of::<std::thread::Result<()>>(),
    ]));
    empty_terminal(module, trace);
    trace.reserve(size_of::<Facts>());
    let proof = physical(module, trace);
    trace.reserve(proof);
    let effects = shared_source::call_effects(module, trace);
    trace.reserve(effects);
    rows::<usize>(count, trace);
    for function in &module.functions {
        trace.work(1);
        units(
            trace,
            function.signature.parameters.len() + function.signature.results.len(),
        );
    }
    for function in &module.functions {
        for block in &function.body.as_ref().unwrap().blocks {
            trace.work(1);
            trace.work(0);
            assert!(block.parameters.is_empty());
        }
    }
    for operation in module
        .functions
        .iter()
        .flat_map(|f| &f.body.as_ref().unwrap().blocks)
        .flat_map(|b| &b.operations)
    {
        trace.work(2);
        match operation.kind {
            fe2o3_kernel_ir::OperationKind::Constant(_) => units(trace, operation.results.len()),
            fe2o3_kernel_ir::OperationKind::Call { .. } => trace.work(calls.len()),
            _ => trace.work(1), // Physical allocation/access proof query.
        }
        if matches!(
            operation.kind,
            fe2o3_kernel_ir::OperationKind::Alloca { .. }
                | fe2o3_kernel_ir::OperationKind::Load { .. }
                | fe2o3_kernel_ir::OperationKind::Store { .. }
        ) {
            trace.work(1);
        }
    }
    rows::<bool>(count, trace);
    trace.work(count);
    for call in &calls {
        trace.work(2);
        trace.work(product(2, call.name.len()));
        trace.work(0);
        trace.work(graph.operations);
        units(trace, call.slots);
    }
    let mut complete = vec![false; count];
    let mut order = Vec::new();
    while order.len() < count {
        let before = order.len();
        for function in 0..count {
            trace.work(1);
            if complete[function] {
                continue;
            }
            let mut ready = true;
            for call in calls.iter().filter(|call| call.function == function) {
                trace.work(1);
                ready &= complete[call.target];
            }
            if ready {
                complete[function] = true;
                order.push(function);
            }
        }
        assert!(order.len() > before, "closed acyclic native call profile");
    }
    units(trace, count); // Fresh call-effect decisions.
    trace.reserve(size_of::<PrivateProjection>());
    trace.reserve(size_of::<Projection>());
    history::execution::plain_import(module, trace);
    trace.reserve(sum(&[
        size_of::<Terminal>(),
        exact_capacity::<CanonicalKirFunctionCoordinateV1>(count),
    ]));
    trace.reserve(0);
    trace.work(count + 1);
    // Attribute map iteration has no guaranteed order. Only its independent
    // successful sum is used; cuts intersecting this census are refused.
    trace.mark("native schema attribute-order interior cutoff is not derived");
    call_joins(count, &calls, trace);
    history::execution::unchanged_materialization(module, trace);
    rows::<Report>(count, trace);
    for (ordinal, function) in module.functions.iter().enumerate() {
        let blocks = &function.body.as_ref().unwrap().blocks;
        let operations = blocks.iter().map(|b| b.operations.len()).sum::<usize>();
        trace.work(1);
        trace.reserve(sum(&[
            size_of::<Admission>(),
            exact_capacity::<Row>(operations + blocks.len()),
        ]));
        trace.reserve(0);
        trace.work(operations + product(2, blocks.len()));
        trace.work(0);
        function_lookup(count, ordinal, trace);
    }
    // The real fixed nine uses its separate native-unit meter. No canonical
    // Budget debit is manufactured from a native-unit outcome or manager size.
    for function in order {
        trace.work(1);
        for _ in calls.iter().filter(|call| call.function == function) {
            trace.work(1);
            trace.work(count);
        }
    }
    trace.mark("native schema attribute-order interior cutoff is not derived");
    call_joins(count, &calls, trace);
    history::execution::unchanged_materialization(module, trace);
    callback(trace);
    trace.leave(scope);
}

// Reuse the already-independent source arithmetic, not a production calculator.
// This module has no tests and invokes no engine. Only the cost-isomorphic
// ordinal-one local-cell profile is used below; its caller profile is not used.
#[path = "../../fe2o3-pliron/src/production_analysis/canonical_private_nine_oracle_v1_numbers.rs"]
mod prior_private;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeUnits {
    work: usize,
    retained: usize,
    peak: usize,
}

fn private_prepare(operations: usize) -> NativeUnits {
    NativeUnits {
        work: sum(&[product(32, product(operations + 1, operations + 1)), 32]),
        retained: 0,
        peak: 6,
    }
}

fn private_overlay(operations: usize) -> NativeUnits {
    // Setup: nine optional11-field rows, pending12, remaining8 =>128.
    // Each stage adds prepare + record32; final publication is9*12+8.
    let stage = private_prepare(operations);
    NativeUnits {
        work: sum(&[128, product(9, stage.work + 32), 9 * 12 + 8]),
        retained: 128,
        peak: 128 + stage.peak.max(4),
    }
}

fn private_root_nine() -> NativeUnits {
    // Entry prologue emits Alloca before Constant ui64 11, Store, Load,
    // Return. Swapping the old helper's first two operations preserves its
    // structural item schedule [3,3,6,6,1], summed identity text lengths,
    // constant->store operand1 and pointer->store/load operand0 use lists.
    // Thus complete work/retained/peak agree, not identity bytes or arbitrary
    // interior failure prefixes. B1/O5/A3/R3/attrs10/types6, max arity2 and
    // resolved rank1 agree too. kir_fn_0 and kir_fn_1 have equal lengths;
    // neither definition contains a callee string.
    // This equivalence does NOT apply to the empty or nonzero typed-call cases.
    let oracle = prior_private::Oracle::derive(1);
    assert_eq!((oracle.shape.o, oracle.shape.a, oracle.shape.r), (5, 3, 3));
    assert_eq!(
        (oracle.shape.attrs, oracle.shape.types, oracle.shape.arity),
        (10, 6, 2)
    );
    assert_eq!(oracle.stages.len(), 9);
    for (stage, pass) in oracle.stages.iter().zip(PASS_ORDER) {
        assert_eq!(stage.pass, pass);
    }
    NativeUnits {
        work: oracle.complete.w,
        retained: oracle.complete.r,
        peak: oracle.complete.p,
    }
}

// The additional closed profiles have one Return block, zero or one U64 entry
// argument, and at most one typed call. This is a source census, not native
// report auto-detection. Private-cell accounting keeps its existing derivation.
struct ScalarNative<'a> {
    ordinal: usize,
    arguments: usize,
    results: usize,
    returned: usize,
    callee: Option<&'a str>,
    operations: usize,
    attributes: usize,
    types: usize,
}

impl<'a> ScalarNative<'a> {
    fn source(module: &'a fe2o3_kernel_ir::Module, ordinal: usize) -> Self {
        use fe2o3_kernel_ir::{OperationKind, Terminator, Type};
        let function = &module.functions[ordinal];
        let body = function.body.as_ref().unwrap();
        assert_eq!(
            body.blocks.len(),
            1,
            "native unit oracle needs independently merged final F"
        );
        assert!(function.signature.parameters.len() <= 1);
        assert!(
            function
                .signature
                .parameters
                .iter()
                .all(|ty| *ty == Type::Scalar(fe2o3_kernel_ir::ScalarType::U64))
        );
        assert!(function.signature.results.len() <= 1);
        assert!(
            function
                .signature
                .results
                .iter()
                .all(|ty| *ty == Type::Scalar(fe2o3_kernel_ir::ScalarType::U64))
        );
        let block = &body.blocks[0];
        assert!(block.parameters.is_empty());
        let Some(Terminator::Return { values }) = &block.terminator else {
            panic!("closed scalar native Return profile");
        };
        assert_eq!(values.len(), function.signature.results.len());
        assert!(block.operations.len() <= 1);
        let callee = block.operations.first().map(|op| {
            let OperationKind::Call { callee, arguments } = &op.kind else {
                panic!("closed scalar native typed-call profile");
            };
            assert_eq!(arguments.len(), 1);
            assert_eq!(op.results.len(), 1);
            assert_eq!(
                op.results[0].ty,
                Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)
            );
            assert_eq!(arguments[0], body.parameters[0]);
            assert!(
                callee
                    .as_str()
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            );
            callee.as_str()
        });
        if !values.is_empty() {
            assert_eq!(values[0], body.parameters[0]);
        }
        let arguments = function.signature.parameters.len();
        let results = usize::from(callee.is_some());
        let returned = values.len();
        // Call roots and argument-returning helpers are mutually exclusive:
        // identity use-list accounting below requires one user per argument.
        assert!(
            results + returned <= 1,
            "closed scalar native single-use profile"
        );
        assert_eq!(arguments, usize::from(callee.is_some() || returned != 0));
        Self {
            ordinal,
            arguments,
            results,
            returned,
            callee,
            operations: 1 + results,
            attributes: 2 + 2 * results,
            types: 2 * (1 + arguments + returned) + arguments + 4 * results,
        }
    }

    fn identity(&self) -> (prior_private::Triple, usize, usize) {
        use prior_private::Triple;
        let (o, a, r, ba) = (
            self.operations,
            self.results + self.returned,
            self.results,
            self.arguments,
        );
        let signature = match (ba, self.returned) {
            (0, 0) => "builtin.type builtin.function <() -> ()>",
            (1, 0) => "builtin.type builtin.function <(builtin.integer ui64) -> ()>",
            (1, 1) => {
                "builtin.type builtin.function <(builtin.integer ui64) -> (builtin.integer ui64)>"
            }
            _ => unreachable!(),
        };
        let symbol = format!("builtin.identifier kir_fn_{}", self.ordinal);
        let mut text = (0usize, 0usize, 0usize, 0usize);
        let mut row = |component: &str, strings: &[&str], cells: usize, location: &str| {
            let bytes = sum(&[component.len(), strings.iter().map(|s| s.len()).sum()]);
            text.0 += bytes;
            text.1 += sum(&[1, 8, bytes, product(8, strings.len() + cells)]);
            text.2 += location.len();
            text.3 += 1;
        };
        row(
            "format",
            &["fe2o3.pliron.ranked.structural-identity.v1"],
            0,
            "",
        );
        row("operation", &["builtin.func"], 1, "");
        row(
            "attributes",
            &[
                "func_type",
                "builtin.type",
                signature,
                "sym_name",
                "builtin.identifier",
                &symbol,
            ],
            1,
            "",
        );
        row("block", &[], 3, "");
        row("attributes", &[], 1, "");
        if ba != 0 {
            row(
                "block argument type",
                &["builtin.integer", "builtin.integer ui64"],
                2,
                "",
            );
        }
        if let Some(callee) = self.callee {
            let callee = format!("builtin.string \"{callee}\"");
            row("operation", &["gpu.call"], 3, "gpu.call");
            row(
                "result types",
                &["builtin.integer", "builtin.integer ui64"],
                2,
                "gpu.call",
            );
            row("operands", &[], 2, "gpu.call");
            row(
                "attributes",
                &[
                    "gpu_call_callee",
                    "builtin.string",
                    &callee,
                    "gpu_call_signature",
                    "builtin.type",
                    "builtin.type builtin.function <(builtin.integer ui64) -> (builtin.integer ui64)>",
                ],
                1,
                "gpu.call",
            );
            row("successors", &[], 1, "gpu.call");
        }
        row("operation", &["gpu.return"], 3, "gpu.return");
        row("result types", &[], 1, "gpu.return");
        row("operands", &[], 1 + self.returned, "gpu.return");
        row("attributes", &[], 1, "gpu.return");
        row("successors", &[], 1, "gpu.return");
        let (i, k, names, records) = text;
        assert_eq!(records, 5 + ba + 5 * o);
        let lookup = 512 * (o + 1) * (o + 1);
        let operation_items = o + a + r + self.attributes - 2;
        let structure_work = lookup + 4 + ba + operation_items;
        let structure_storage = 5 + ba + operation_items + 3 * o;
        let rendered = 23 + 8 * ba + 3 * o + 9 * (self.attributes - 2) + 8 * r;
        let roots = self.attributes + 1 + ba + r;
        let summary = 240 * 4 + 3;
        let textual = Triple::new(
            rendered * 65536 + records * summary * 4 + roots * 65536 * 4 + structure_work,
            0,
            5 * 65536 + summary + structure_storage + 2,
        );
        // Both pointer tables have four buckets. Every nonempty use roster is
        // the sole entry argument's one local operand, never a result use.
        let pointer_lookup = 16 + 2 * 16 + (4 + 16) * (2 + 16);
        let op_heap = (4usize * 24 + 4 + 16).div_ceil(8);
        let closure = Triple {
            w: 32
                + 17
                + 40
                + 4
                + pointer_lookup
                + 8
                + o * (pointer_lookup + 9)
                + 4
                + 4
                + 4 * o
                + a * (4 + pointer_lookup + 1 + 4)
                + 4
                + 4 * o
                + 9 * (ba + r)
                + a * (5 + pointer_lookup + 1)
                + 8
                + 8,
            r: 13 + op_heap,
            p: 64 + (14 + 4 * 2 + 4 * 3 + 4 * 2) + 17 + 6 + op_heap + if a == 0 { 13 } else { 15 },
        };
        let graph = 1 + o + a + r + ba + self.attributes + self.types;
        let height = usize::BITS as usize - self.attributes.leading_zeros() as usize;
        let capture = 7 * graph
            + (4 + height) * i
            + 2 * k
            + 2 * records
            + 4 * records * summary
            + 3 * names
            + 1
            + lookup;
        let retained = k + records + records * summary + names + 9;
        let temporary = 3 + 3 * o + r + ba + i + records * summary + closure.r;
        (
            Triple {
                w: textual.w + closure.w + capture,
                r: retained,
                p: closure
                    .p
                    .max(textual.p + closure.r)
                    .max(retained + temporary),
            },
            i,
            k,
        )
    }
}

// Temporary upper bounds below are intentionally not atomic cutoff oracles.
// They prove that the independently exact trace-attempt peak dominates every
// later component. Work and retained units are exact successful envelope terms.
fn scalar_nine(s: &ScalarNative<'_>) -> NativeUnits {
    use prior_private::Triple as T;
    const N: usize = 1 << 20;
    let (o, a, r, ba, attrs) = (
        s.operations,
        s.results + s.returned,
        s.results,
        s.arguments,
        s.attributes,
    );
    let (identity, i, k) = s.identity();
    let h = identity.r;
    let j = 1 + o + a + r + ba + attrs + s.types + i + k;
    let sparse = T::new(
        8 * (1 + o + a + r + ba) + 2 + ba + 1 + 62 * (r + ba) + 20 * (2 * a + o + ba) + 16,
        28 * (r + ba) + 24,
        59 * (r + ba) + 8 + 4 * a + 2 * o + 6,
    );
    let before_trace = T::new(1, 10, 0)
        .then(identity)
        .then(T::new(o + 2, o + 2, 0))
        .then(sparse)
        .then(T::new(2, 2, 1))
        .then(T::new(o + 12, 10, 0));
    let trace = T::new(
        4 * o + 1 + 2 + 1 + (N + 1) * 36,
        2 + N * 17,
        2 + r + 3 * ba + N * (ba + 2) + 1 + 1836,
    );
    let mut total = before_trace.then(trace);
    let trace_peak = total.p;
    let keep = |current: T, next: T| {
        let next = current.then(next);
        assert!(
            next.p <= trace_peak,
            "non-trace component invalidates peak dominance"
        );
        next
    };
    total = keep(total, T::new(128, 128, 0)).then(T::new(8, 10, 0));
    let findings = 30 * o + 1;
    let height = usize::BITS as usize - o.leading_zeros() as usize;
    let tensor = T::new(
        8 * j + o + o * (height + 8) + 3 * o + o * 3 * 2048 * 28 + 3 * (N + 1),
        findings * 152 + i + o * 24,
        32 * o + 2048 * 8 + 23 + 16 * (r + ba) + 3 * a + 19 + ba * 1024,
    );
    // For ba=1 the forwarding row/index rosters own only fixed scalar/pointer
    // fields and three container headers; 1024 words strictly covers their
    // per-row bound (ROW+1+64+HEADERS), selector5 and phi2. No ABI equality used.
    let bf = 1 + a;
    let bounds = T::new(
        3 * j + (2 * o + a + r + attrs + 9 + a) + a * (N + 1) + bf * 192 * 4 + 4 * i + 256 * 4,
        (bf * 256).max(64 + 2 * i + 256),
        11 + bf + j + 2 * i + 128 + usize::from(a != 0) * 48,
    );
    let lookup = (r + a).max(ba) + 20 * attrs;
    let scan = 32 + 8 + 12 * o + 4 * a + (a + o) * (lookup + 80);
    let race = T::new(
        scan + lookup + 4 * 38 + 64 + 32 * o + 176 + 2 * 1246,
        1246,
        2 * 38 + 16 + 1246 + 160,
    );
    let pipeline = T::new(4 * o + 1, 0, 0);
    let barrier = T::new(
        o + 256 + 32 * o + 512 + 8 + 1024 + 8192 + pipeline.w,
        4 * 256 + i + 4096,
        120 + 6 * o + 1024 + 96 + 4 * 1024 + 2 * 4096,
    );
    let structure = (o + 1) + 1 + o + a + r + attrs + ba;
    let progress = T::new(
        4 * structure + structure + 12 + 2 + 1 + a + o + 8 * (552 + 32 * (r + ba)),
        1041,
        2 * structure + 14 + structure + 1 + a + 3 + 194,
    );
    let producers = [
        tensor,
        bounds,
        T::new(2 * o, 32 * o, 8 * o),
        race,
        T::new(o, 0, 0),
        barrier,
        pipeline,
        T::new(4 * o + pipeline.w, 1024, 8 * o),
        T::new(2 * o + progress.w, progress.r, progress.p - progress.r),
    ];
    let witness = o * 8 * 40 + 1;
    let replay = 2 * o + 19 * (N + 1) + o * 8 * 48 + 96;
    let replay_temp = 6 * o + (2 * o + 1) * 8 + 96;
    let validations = [
        T::new(2129, 2049, 1025),
        T::new(
            49 + 2 * replay + witness + 2048,
            1 + witness.max(2048),
            1 + witness + replay_temp + 2048,
        ),
        T::new(2129, 2049, 1025),
        T::new(2129, 2049, 1025),
        T::new(2112, 2049, 1025),
        T::new(2129, 2049, 1025),
        T::new(2112, 2049, 1025),
        T::new(2129, 2049, 1025),
        T::new(2292, 2049, 1025),
    ];
    for stage in 0..9 {
        if stage == 3 {
            total = keep(total, T::new(8 * o, 3, 0));
        }
        if stage == 5 {
            total = keep(total, T::new(1, 1, 0));
        }
        if stage == 7 {
            total = keep(total, T::new(4, 1024, 8));
        }
        let mut local = T::new(32 * (o + 1) * (o + 1) + 32, 0, 6);
        if stage == 5 {
            local = local.then(T::new(96 + 6 * o + 12, 1, 64));
        }
        local = local.then(producers[stage]);
        if stage == 5 || stage == 8 {
            local = local.then(T::new(53, 0, 44));
        }
        let checkpoint = T::new(identity.w + 2 * h + 1, h + 1, identity.p);
        total = total.replace(h, local.then(checkpoint));
        assert!(total.p <= trace_peak);
        total = keep(total, T::new(32, 0, 4).then(validations[stage]));
    }
    total = total.replace(h, T::new(k + 13, k + 11, h));
    total = keep(total, T::new(116, 0, 4));
    total = total.replace(sparse.r + 2 + 10 + trace.r + 3 + 1 + 1024, T::ZERO);
    assert_eq!(total.p, trace_peak);
    NativeUnits {
        work: total.w,
        retained: total.r,
        peak: total.p,
    }
}

pub(super) fn complete_units(module: &fe2o3_kernel_ir::Module) -> Vec<(usize, usize, usize)> {
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, Constant, MemoryAccess, OperationKind, Terminator, Type,
    };
    module.functions.iter().enumerate().map(|(ordinal, function)| {
        let private = function.body.as_ref().unwrap().blocks.iter().flat_map(|b| &b.operations)
            .any(|op| matches!(op.kind, fe2o3_kernel_ir::OperationKind::Alloca { .. }));
        let complete = if private {
            assert_eq!(module.functions.len(), 1);
            assert_eq!(ordinal, 0);
            assert!(function.signature.parameters.is_empty() && function.signature.results.is_empty());
            let body = function.body.as_ref().unwrap();
            assert!(body.parameters.is_empty());
            assert_eq!(body.blocks.len(), 1);
            let block = &body.blocks[0];
            assert!(block.parameters.is_empty());
            assert!(matches!(&block.terminator, Some(Terminator::Return { values }) if values.is_empty()));
            assert_eq!(block.operations.len(), 4);
            let allocation = &block.operations[0];
            let constant = &block.operations[1];
            let store = &block.operations[2];
            let load = &block.operations[3];
            assert!(matches!(constant.kind, OperationKind::Constant(Constant::U64(11))));
            assert_eq!(constant.results.len(), 1);
            assert_eq!(constant.results[0].ty, Type::Scalar(fe2o3_kernel_ir::ScalarType::U64));
            assert!(matches!(allocation.kind, OperationKind::Alloca {
                element: Type::Scalar(fe2o3_kernel_ir::ScalarType::U64), count: None, address_space: AddressSpace::Private,
                alignment: 8 }));
            assert_eq!(allocation.results.len(), 1);
            assert!(matches!(&allocation.results[0].ty, Type::Pointer(pointer)
                if *pointer.pointee == Type::Scalar(fe2o3_kernel_ir::ScalarType::U64) && pointer.address_space == AddressSpace::Private
                    && pointer.access == AccessMode::ReadWrite));
            let access = MemoryAccess::new(AddressSpace::Private, 8);
            assert!(matches!(&store.kind, OperationKind::Store { pointer, value, access: actual }
                if *pointer == allocation.results[0].id && *value == constant.results[0].id
                    && *actual == access));
            assert!(store.results.is_empty());
            assert!(matches!(&load.kind, OperationKind::Load { pointer, access: actual }
                if *pointer == allocation.results[0].id && *actual == access));
            assert_eq!(load.results.len(), 1);
            assert_eq!(load.results[0].ty, Type::Scalar(fe2o3_kernel_ir::ScalarType::U64));
            private_root_nine()
        } else { scalar_nine(&ScalarNative::source(module, ordinal)) };
        (complete.work, complete.retained, complete.peak)
    }).collect()
}

#[test]
fn whole_native_private_overlay_is_parameterized_but_not_the_ordinary_nine_total() {
    for operations in [1, 2, 5, 33] {
        let extra = private_overlay(operations);
        assert_eq!(extra.work, 820 + 288 * (operations + 1) * (operations + 1));
        assert_eq!((extra.retained, extra.peak), (128, 134));
        // Identity lookup is charged in prescan/text and final capture, for
        // the initial identity plus each of the nine actual checkpoints.
        let identity_lookup_only = 2 * 10 * 512 * (operations + 1) * (operations + 1);
        assert!(identity_lookup_only > extra.work);
    }
}

#[test]
fn whole_native_local_cell_reuses_independent_profile_only_after_shape_equivalence() {
    let complete = private_root_nine();
    let overlay = private_overlay(5);
    assert!(complete.work > overlay.work);
    assert!(complete.retained > overlay.retained);
    assert!(complete.peak >= complete.retained);
    assert_eq!(
        "builtin.identifier kir_fn_0".len(),
        "builtin.identifier kir_fn_1".len()
    );
}
