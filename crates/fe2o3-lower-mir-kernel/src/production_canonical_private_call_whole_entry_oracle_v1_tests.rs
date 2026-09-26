// Independent logical-debit transcripts, not successful-run calibration.
// The original source owner is an explicitly prepaid input boundary.
use std::mem::{align_of, size_of};

mod transcript {
    include!("production_canonical_scalar_assertion_qualification_oracle_v1_tests.rs");

    impl Trace {
        // Aggregate components preserve enclosing scope floors and postflights.
        // They are admitted in full or the query is refused, never converted
        // into an invented atomic work debit or simultaneous reservation.
        pub(super) fn composed(
            &self,
            work_limit: usize,
            storage_limit: usize,
            component: impl Fn(&'static str) -> Option<(usize, usize)>,
        ) -> Result<Predicted, &'static str> {
            assert!(storage_limit >= self.floor);
            let mut state = State {
                predicted: Predicted {
                    work: 0,
                    storage: self.floor,
                    peak: self.floor,
                    first_work: None,
                    first_storage: None,
                    denied_at: None,
                    marks: Vec::new(),
                },
                work_limit,
                storage_limit,
                paid_scopes: Vec::new(),
                poisoned: BTreeMap::new(),
            };
            for event in &self.events {
                let result = match event.debit {
                    Debit::Work { amount, guard } => state.work(amount, guard),
                    Debit::Reserve(bytes) => state.reserve(bytes),
                    Debit::Release(bytes) => {
                        state.release(bytes);
                        Ok(())
                    }
                    Debit::Enter(scope) => {
                        state.paid_scopes.push((scope, state.predicted.storage));
                        Ok(())
                    }
                    Debit::Leave(scope) => state.leave(scope, &self.scopes[scope]),
                    Debit::Mark(mark) => {
                        if let Some((work, peak)) = component(mark) {
                            let end = state.predicted.work.checked_add(work).unwrap();
                            let peak = state.predicted.storage.checked_add(peak).unwrap();
                            if end > work_limit || peak > storage_limit {
                                return Err(mark);
                            }
                            state.predicted.work = end;
                            state.predicted.peak = state.predicted.peak.max(peak);
                        }
                        state.predicted.marks.push(mark);
                        Ok(())
                    }
                };
                if let Err(denial) = result {
                    state.predicted.denied_at = Some((event.point, denial));
                    while let Some(&(scope, _)) = state.paid_scopes.last() {
                        let _ = state.leave(scope, &self.scopes[scope]);
                    }
                    return Ok(state.predicted);
                }
            }
            assert!(state.paid_scopes.is_empty());
            Ok(state.predicted)
        }
    }

    #[test]
    fn whole_composed_trace_refuses_opaque_interiors_and_keeps_cleanup_suffix() {
        let mut trace = Trace::new(43);
        let scope = trace.enter(&[(3, None)]);
        trace.work(2);
        trace.reserve(5);
        trace.mark("opaque component");
        trace.work(7);
        trace.leave(scope);
        let component = |name| (name == "opaque component").then_some((17, 19));
        let full = trace.composed(29, 67, component).unwrap();
        assert_eq!(
            (
                full.work,
                full.storage,
                full.peak,
                full.first_work,
                full.first_storage
            ),
            (29, 43, 67, None, None)
        );
        for work in 2..19 {
            assert_eq!(
                trace.composed(work, 67, component).unwrap_err(),
                "opaque component"
            );
        }
        for storage in 48..67 {
            assert_eq!(
                trace.composed(29, storage, component).unwrap_err(),
                "opaque component"
            );
        }
        let suffix = trace.composed(25, 67, component).unwrap();
        assert_eq!(
            (
                suffix.work,
                suffix.storage,
                suffix.peak,
                suffix.first_work,
                suffix.first_storage
            ),
            (22, 43, 67, Some(26), None)
        );
        assert!(suffix.denied_at.is_some());
    }
}
use transcript::{Point, Trace};

fn sum(values: &[usize]) -> usize {
    values
        .iter()
        .try_fold(0usize, |a, b| a.checked_add(*b))
        .unwrap()
}

fn product(a: usize, b: usize) -> usize {
    a.checked_mul(b).unwrap()
}

fn units(trace: &mut Trace, count: usize) {
    for _ in 0..count {
        trace.work(1);
    }
}

fn exact_capacity<T>(count: usize) -> usize {
    assert_ne!(size_of::<T>(), 0);
    let mut independent = Vec::<T>::new();
    independent.try_reserve_exact(count).unwrap();
    assert_eq!(
        independent.capacity(),
        count,
        "pinned exact-capacity premise changed; do not retune successful totals"
    );
    product(independent.capacity(), size_of::<T>())
}

// This reproduces only the comparison schedule of the existing fallible heap
// helper over independent keys. It neither reads nor validates a real graph.
fn heap<T: Ord>(
    keys: &mut [T],
    trace: &mut Trace,
    dispatch: bool,
    width: impl Fn(&T, &T) -> usize,
) {
    heap_schedule(keys, trace, true, dispatch, width);
}

fn heap_schedule<T: Ord>(
    keys: &mut [T],
    trace: &mut Trace,
    loop_work: bool,
    dispatch: bool,
    width: impl Fn(&T, &T) -> usize,
) {
    fn sift<T: Ord>(
        keys: &mut [T],
        mut root: usize,
        end: usize,
        trace: &mut Trace,
        loop_work: bool,
        dispatch: bool,
        width: &impl Fn(&T, &T) -> usize,
    ) {
        loop {
            if loop_work {
                trace.work(1);
            }
            let left = root * 2 + 1;
            if left >= end {
                break;
            }
            let mut child = left;
            if left + 1 < end {
                if dispatch {
                    trace.work(1);
                }
                trace.work(width(&keys[left], &keys[left + 1]));
                if keys[left] < keys[left + 1] {
                    child += 1;
                }
            }
            if dispatch {
                trace.work(1);
            }
            trace.work(width(&keys[root], &keys[child]));
            if keys[root] >= keys[child] {
                break;
            }
            trace.work(1);
            keys.swap(root, child);
            root = child;
        }
    }
    for root in (0..keys.len() / 2).rev() {
        sift(keys, root, keys.len(), trace, loop_work, dispatch, &width);
    }
    for end in (1..keys.len()).rev() {
        trace.work(1);
        keys.swap(0, end);
        sift(keys, 0, end, trace, loop_work, dispatch, &width);
    }
}

fn find<T: Ord>(keys: &[T], key: &T, trace: &mut Trace, width: impl Fn(&T, &T) -> usize) {
    let (mut lo, mut hi) = (0, keys.len());
    while lo < hi {
        trace.work(1);
        let mid = lo + (hi - lo) / 2;
        trace.work(width(&keys[mid], key));
        match keys[mid].cmp(key) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => return,
        }
    }
    panic!("oracle fixture references a missing independent key");
}

#[derive(Clone, Debug)]
struct Graph<'a> {
    functions: Vec<&'a str>,
    blocks: Vec<(u32, u32)>,
    definitions: Vec<(u32, u32)>,
    operations: usize,
    used_definitions: Vec<(u32, u32)>,
    successor_blocks: Vec<(u32, u32)>,
    edge_arguments: usize,
    effects: usize,
    callees: Vec<&'a str>,
    entries: Vec<&'a str>,
}

impl Graph<'_> {
    fn visits(&self) -> usize {
        sum(&[
            1,
            self.functions.len(),
            product(2, self.blocks.len()),
            self.definitions.len(),
            self.operations,
            self.used_definitions.len(),
            self.successor_blocks.len(),
            self.edge_arguments,
            self.effects,
            self.callees.len(),
            self.entries.len(),
        ])
    }
}

mod source {
    use super::*;
    include!("production_canonical_private_call_whole_source_v1_tests.rs");
}

mod history {
    use super::*;
    include!("production_canonical_private_call_whole_history_v1_tests.rs");

    pub(super) mod execution {
        use super::*;
        include!("production_canonical_private_call_whole_execution_v1_tests.rs");
    }
}

mod replay {
    use super::*;
    include!("production_canonical_private_call_whole_replay_v1_tests.rs");
}

mod consume {
    use super::*;
    include!("production_canonical_private_call_whole_consume_v1_tests.rs");
}

mod shared_source {
    use super::*;
    include!("production_canonical_private_call_whole_shared_source_v1_tests.rs");
}

mod admission {
    use super::*;
    include!("production_canonical_private_call_whole_admission_v1_tests.rs");
}

mod native {
    use super::*;
    include!("production_canonical_private_call_whole_native_v1_tests.rs");
}

pub(crate) fn whole_native_units_v1(
    module: &fe2o3_kernel_ir::Module,
) -> Vec<(usize, usize, usize)> {
    let final_module = if module.functions.len() == 1 {
        module.clone()
    } else {
        history::shared_history_retained(module).0
    };
    native::complete_units(&final_module)
}

pub(crate) fn assert_whole_native_units_v1(
    expected: &[(usize, usize, usize)],
    view: &fe2o3_pliron::CheckedCanonicalTrapPoliciesV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), fe2o3_pliron::CanonicalRankedPolicyFailureV1> {
    fn check(
        actual: fe2o3_pliron::CanonicalRankedPolicyResourceObservationV1,
        expected: (usize, usize, usize),
    ) {
        assert_eq!(
            (
                actual.work_upper_bound(),
                actual.retained_storage_units(),
                actual.peak_storage_units()
            ),
            expected
        );
        assert_eq!(actual.first_denial(), None);
        assert!(!actual.caught_panic());
    }
    let mut floor = (0usize, 0usize, 0usize);
    for (ordinal, &(work, retained, peak)) in expected.iter().enumerate() {
        let history = view.history(ordinal, budget)?;
        assert_eq!(history.function(), ordinal);
        check(history.floor(), floor);
        check(history.invocation(), (work, retained, peak));
        floor = (
            sum(&[floor.0, work]),
            sum(&[floor.1, retained]),
            floor.2.max(sum(&[floor.1, peak])),
        );
    }
    check(view.observation(budget)?, floor);
    Ok(())
}

pub(crate) fn whole_borrowed_verifier_component_v1(
    module: &fe2o3_kernel_ir::Module,
) -> (usize, usize, usize) {
    admission::borrowed_verifier(module)
}

pub(crate) fn whole_inverse_component_v1(
    module: &fe2o3_kernel_ir::Module,
) -> (usize, usize, usize, usize, usize) {
    admission::inverse(module)
}

pub(crate) fn whole_dynamic_native_work_v1(module: &fe2o3_kernel_ir::Module) -> (usize, usize) {
    history::closed_dynamic_work(module)
}

// Complete returned no-op history storage only, not whole prepare work/peak.
pub(crate) fn whole_root_history_retained_component_v1(
    module: &fe2o3_kernel_ir::Module,
) -> [usize; 4] {
    history::root_history_retained(module)
}

pub(crate) fn whole_shared_history_retained_component_v1(
    module: &fe2o3_kernel_ir::Module,
) -> (fe2o3_kernel_ir::Module, [usize; 6]) {
    history::shared_history_retained(module)
}

pub(crate) fn whole_history_replay_component_v1(
    input: &fe2o3_kernel_ir::Module,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (usize, usize, usize, Option<usize>, Option<usize>) {
    let mut trace = Trace::new(floor);
    if input.functions.len() == 1 {
        replay::history(input, input, false, &mut trace);
    } else {
        let (output, _) = history::shared_history_retained(input);
        replay::history(input, &output, true, &mut trace);
    }
    let p = trace.run(work_limit, storage_limit);
    (p.work, p.storage, p.peak, p.first_work, p.first_storage)
}

pub(crate) fn whole_root_prepare_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Result<(usize, usize, usize, Option<usize>, Option<usize>), &'static str> {
    let mut plan = history::execution::Plan::new(floor);
    plan.actual(|trace| {
        trace.work(1);
        trace.work(2);
        trace.work(3);
        trace.reserve(size_of::<ProductionCanonicalScalarFixedPointOwnerV1>());
        root_private_source(owner, trace);
    });
    let module = owner.executable().module();
    let merged = module.functions.len() != 1;
    let output = if merged {
        history::shared_history_retained(module).0
    } else {
        module.clone()
    };
    let history = history::execution::factory(module, &output, merged, &mut plan);
    plan.actual(|trace| {
        trace.reserve(history);
        replay::history(module, &output, merged, trace);
        trace.work(3);
    });
    plan.restore(floor);
    plan.run(work_limit, storage_limit)
}

pub(crate) fn whole_history_factory_component_v1(
    input: &fe2o3_kernel_ir::Module,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Result<(usize, usize, usize, Option<usize>, Option<usize>), &'static str> {
    let mut plan = history::execution::Plan::new(floor);
    if input.functions.len() == 1 {
        history::execution::root_factory(input, &mut plan);
    } else {
        let (output, _) = history::shared_history_retained(input);
        history::execution::factory(input, &output, true, &mut plan);
    }
    plan.run(work_limit, storage_limit)
}

pub(crate) fn whole_lineage_component_v1(
    input: &fe2o3_kernel_ir::Module,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (usize, usize, usize, Option<usize>, Option<usize>) {
    let mut trace = Trace::new(floor);
    if input.functions.len() == 1 {
        consume::lineage_component(input, input, false, &mut trace);
    } else {
        let (output, _) = history::shared_history_retained(input);
        consume::lineage_component(input, &output, true, &mut trace);
    }
    let p = trace.run(work_limit, storage_limit);
    (p.work, p.storage, p.peak, p.first_work, p.first_storage)
}

pub(crate) fn whole_root_consume_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Result<(usize, usize, usize, Option<usize>, Option<usize>, bool), &'static str> {
    consume::root_consume(owner, floor, work_limit, storage_limit)
}

// Test-only execution adapter. It returns no borrowed proof or custody object;
// the expectation above is built without calling this adapter or its engines.
pub(crate) fn execute_lineage_component_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    cs_scope_v1(budget, |budget| {
        let (inventory, receipt) =
            CanonicalKirInventoryV1::derive(owner.original.executable(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let lineage = cs_lineage_v1(owner, &inventory, budget)?;
        drop(lineage);
        Ok(())
    })
}

pub(crate) fn whole_inventory_component_v1(
    module: &fe2o3_kernel_ir::Module,
) -> (usize, usize, usize) {
    let graph = Graph::module(module);
    let mut trace = Trace::new(0);
    let retained = graph.inventory(&mut trace);
    let predicted = trace.success();
    assert_eq!(predicted.storage, 0);
    assert_eq!(predicted.peak, retained);
    let terminal = graph.entries.last().expect("genuine kernel entry").len() + 1;
    (predicted.work, retained, terminal)
}

// The five fields are the complete public source-ledger observation, not native
// fixed-nine units. This component has no postflight debit after its callback.
pub(crate) fn whole_root_calls_component_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (usize, usize, usize, Option<usize>, Option<usize>) {
    let mut trace = Trace::new(floor);
    source::root_call_index(owner, &mut trace);
    let predicted = trace.run(work_limit, storage_limit);
    (
        predicted.work,
        predicted.storage,
        predicted.peak,
        predicted.first_work,
        predicted.first_storage,
    )
}

pub(crate) fn whole_root_metadata_component_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (usize, usize, usize, Option<usize>, Option<usize>) {
    let rows = source::RootMetadata::source(owner);
    let mut trace = Trace::new(floor);
    source::root_call_index_with(owner, &mut trace, |trace| rows.all_components(trace));
    let predicted = trace.run(work_limit, storage_limit);
    (
        predicted.work,
        predicted.storage,
        predicted.peak,
        predicted.first_work,
        predicted.first_storage,
    )
}

pub(crate) fn whole_original_source_component_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (usize, usize, usize, Option<usize>, Option<usize>) {
    let mut trace = Trace::new(floor);
    source::root_source::<()>(owner, &mut trace, |_| {});
    let predicted = trace.run(work_limit, storage_limit);
    (
        predicted.work,
        predicted.storage,
        predicted.peak,
        predicted.first_work,
        predicted.first_storage,
    )
}

// The no-assertion source proof still constructs this real sparse report. This
// profile is limited to one no-argument Return block, private cells/constants,
// and at most three operands per operation. Every result resolves on its first
// visit, and all users are already queued when publication visits them.
struct SparseRoot {
    definitions: usize,
    operations: usize,
    uses: usize,
}

impl SparseRoot {
    fn source(module: &fe2o3_kernel_ir::Module) -> Self {
        let [function] = module.functions.as_slice() else {
            panic!("one sparse root")
        };
        let body = function.body.as_ref().unwrap();
        assert!(body.parameters.is_empty());
        let [block] = body.blocks.as_slice() else {
            panic!("one sparse root block")
        };
        assert!(block.parameters.is_empty());
        assert!(matches!(
            block.terminator,
            Some(fe2o3_kernel_ir::Terminator::Return { .. })
        ));
        for operation in &block.operations {
            assert!(matches!(
                operation.kind,
                fe2o3_kernel_ir::OperationKind::Constant(_)
                    | fe2o3_kernel_ir::OperationKind::Alloca { count: None, .. }
                    | fe2o3_kernel_ir::OperationKind::Load { .. }
                    | fe2o3_kernel_ir::OperationKind::Store { .. }
            ));
        }
        let graph = Graph::module(module);
        assert!(graph.successor_blocks.is_empty());
        Self {
            definitions: graph.definitions.len(),
            operations: graph.operations,
            uses: graph.used_definitions.len(),
        }
    }

    fn derive(&self, trace: &mut Trace) -> usize {
        let scope = trace.enter(&[]);
        let retained =
            sparse_allocations(self.definitions, self.uses, 1, self.operations, 0, trace);
        // Every execution debit is one unit here. Initialization visits U+D;
        // root activation, operation/terminator queue and transfer, each
        // publication's use+enqueue pair, then the unresolved-result cursor.
        units(
            trace,
            sum(&[
                5,
                product(5, self.operations),
                product(4, self.definitions),
                product(4, self.uses),
            ]),
        );
        trace.leave(scope);
        retained
    }
}

fn sparse_allocations(
    definitions: usize,
    uses: usize,
    blocks: usize,
    operations: usize,
    edges: usize,
    trace: &mut Trace,
) -> usize {
    use fe2o3_kernel_analysis::{
        CanonicalKirSparseExceptionV1 as Exception, CanonicalKirSparseV1 as Report,
        CanonicalKirSparseValueV1 as Value,
    };
    // Required same-host Engine companion; not a portable repr(Rust) premise.
    type Engine = (
        Report<'static, 'static>,
        [Vec<usize>; 4],
        Vec<u8>,
        [usize; 5],
    );
    fn allocate<T>(count: usize, trace: &mut Trace) -> usize {
        if count == 0 {
            return 0;
        }
        trace.work(1);
        let bytes = exact_capacity::<T>(count);
        trace.reserve(bytes);
        trace.reserve(0);
        units(trace, count);
        bytes
    }
    units(trace, 8);
    trace.reserve(size_of::<Engine>());
    let retained = sum(&[
        size_of::<Report<'_, '_>>(),
        allocate::<Value>(definitions, trace),
        allocate::<u8>(blocks, trace),
        allocate::<u8>(edges, trace),
        allocate::<Exception>(operations, trace),
    ]);
    allocate::<usize>(definitions, trace);
    allocate::<usize>(uses, trace);
    allocate::<u8>(blocks + operations, trace);
    allocate::<usize>(blocks + operations, trace);
    allocate::<usize>(definitions, trace);
    retained
}

pub(crate) fn whole_sparse_root_component_v1(
    module: &fe2o3_kernel_ir::Module,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> ((usize, usize, usize, Option<usize>, Option<usize>), usize) {
    let mut trace = Trace::new(floor);
    let retained = SparseRoot::source(module).derive(&mut trace);
    let p = trace.run(work_limit, storage_limit);
    (
        (p.work, p.storage, p.peak, p.first_work, p.first_storage),
        retained,
    )
}

#[test]
fn whole_oracle_zero_assertion_sparse_work_is_not_zero() {
    for (shape, expected) in [
        (
            SparseRoot {
                definitions: 0,
                operations: 0,
                uses: 0,
            },
            19,
        ),
        (
            SparseRoot {
                definitions: 3,
                operations: 4,
                uses: 3,
            },
            92,
        ),
    ] {
        let mut trace = Trace::new(43);
        shape.derive(&mut trace);
        assert_eq!(trace.success().work, expected);
        assert_eq!(trace.success().storage, 43);
        let denied = trace.run(expected - 1, usize::MAX);
        assert_eq!(denied.work, expected - 1);
        assert_eq!(denied.first_work, Some(expected));
    }
}

include!("production_canonical_private_call_whole_source_adapter_v1_tests.rs");

#[test]
fn whole_source_reader_empty_and_cell_terms_follow_actual_lifetime_scan() {
    // Reader authentication6-work scratch + O initialize + 2O census + 3O
    // alias join + 4O anchors. A real same-block read adds destination9,
    // two17-work kill scans + empty scratch6 + complete1 + empty interval7.
    assert_eq!(1 + 6 + 0 + 0 + 0 + 0, 7);
    assert_eq!(
        1 + 6 + 4 + 2 * 4 + 3 * 4 + 4 * 4 + 9 + 2 * 17 + 6 + 1 + 7,
        104
    );
}

#[test]
fn whole_oracle_shared_heap_and_search_have_literal_atomic_debits() {
    let mut trace = Trace::new(29);
    let mut keys = vec![3, 1, 2];
    heap(&mut keys, &mut trace, true, |_, _| 1);
    assert_eq!(keys, [1, 2, 3]);
    assert_eq!(trace.success().work, 11);
    find(&keys, &2, &mut trace, |_, _| 1);
    assert_eq!(trace.success().work, 13);
    assert_eq!(trace.run(12, usize::MAX).first_work, Some(13));
    let mut inventory = Trace::new(29);
    heap(&mut [3, 1, 2], &mut inventory, false, |_, _| 1);
    assert_eq!(inventory.success().work, 8);
}

#[test]
fn whole_oracle_empty_vectors_and_local_headers_are_explicit() {
    assert_eq!(
        size_of::<usize>(),
        8,
        "oracle profile requires the pinned 64-bit target"
    );
    assert_eq!(size_of::<Option<usize>>(), 16);
    assert_eq!(exact_capacity::<usize>(0), 0);
    assert_eq!(exact_capacity::<usize>(3), 3 * size_of::<usize>());
    assert_eq!(size_of::<Vec<usize>>(), 3 * size_of::<usize>());
    assert_eq!(align_of::<Vec<usize>>(), align_of::<usize>());
    assert_ne!(size_of::<CpcOriginsV1<'_, '_>>(), 0);
    assert_ne!(size_of::<CpcCallIndexV1>(), 0);
    assert_ne!(size_of::<CpcFinalCallIndexV1>(), 0);
}
