use super::*;
use dialect_kernel::{
    BranchArgsOp, BranchOp, DIALECT_NAME, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    ReturnOp, register_dialect,
};
use pliron::dialect::DialectName;

mod native;
mod order;
mod ownership;

fn check(
    context: &Context,
    function: &FuncOp,
    scan: &PrescanV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Checked<ProductionAnalysisResourceUpperBoundV1> {
    super::check(context, function, scan, limits).map(|order| order.resource_upper_bound())
}

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    context
}

fn function(context: &mut Context, inputs: Vec<TypeHandle>) -> FuncOp {
    let signature = FunctionType::get(context, inputs, vec![]);
    FuncOp::new(context, "closure".try_into().unwrap(), signature)
}

fn append(context: &Context, block: Ptr<BasicBlock>, operation: impl Op) {
    operation.get_operation().insert_at_back(block, context);
}

fn fanout(context: &mut Context, count: usize) -> FuncOp {
    let function = function(context, vec![]);
    let entry = function.get_entry_block(context);
    let index = IndexType::get(context).into();
    let join = BasicBlock::new(context, None, vec![index; count]);
    join.insert_at_back(function.get_region(context), context);
    let constant = IndexConstantOp::new(context, 7);
    let arguments = vec![constant.result(context); count];
    append(context, entry, constant);
    let branch = BranchArgsOp::new(context, arguments, join);
    append(context, entry, branch);
    let ret = ReturnOp::new(context);
    append(context, join, ret);
    function
}

fn hard() -> ProductionAnalysisResourceLimitsV1 {
    ProductionAnalysisResourceLimitsV1::production_hard_ceiling()
}

fn reset_trace() {
    TRACE.set(Trace::default());
}

fn observed() -> Trace {
    TRACE.get()
}

fn assert_resource<T>(result: Checked<T>, expected: &'static str) {
    assert!(matches!(result, Err(Failure::Resource(error))
        if error.phase == PHASE && error.resource == expected));
}

#[test]
fn pinned_pointer_table_and_use_vector_capacities() {
    assert_eq!(size_of::<usize>(), 8);
    assert_eq!(size_of::<Ptr<Operation>>(), 16);
    assert_eq!(size_of::<Ptr<BasicBlock>>(), 16);
    assert_eq!(size_of::<Use<Value>>(), 24);
    assert_eq!(size_of::<Use<Ptr<BasicBlock>>>(), 24);
    assert_eq!(size_of::<HashSet<Ptr<Operation>>>(), 48);
    assert_eq!(size_of::<HashMap<Ptr<Operation>, usize>>(), 48);
    assert_eq!(size_of::<(Ptr<Operation>, usize)>(), 24);
    assert_eq!(cells::<CheckedOrder<'_>>(), 13);
    for (entries, buckets, usable, heap_cells) in [
        (0, 0, 0, 0),
        (1, 4, 3, 11),
        (3, 4, 3, 11),
        (4, 8, 7, 19),
        (7, 8, 7, 19),
        (8, 16, 14, 36),
        (14, 16, 14, 36),
        (15, 32, 28, 70),
    ] {
        let profile = Table::pointers::<Ptr<Operation>>(entries).unwrap();
        assert_eq!(profile.buckets, buckets);
        assert_eq!(profile.usable(), usable);
        assert_eq!(profile.storage, 6 + heap_cells);
        let mut actual = HashSet::<Ptr<Operation>>::new();
        actual.try_reserve(entries).unwrap();
        assert_eq!(actual.capacity(), usable);
        let order = Table::entries::<Ptr<Operation>, (Ptr<Operation>, usize)>(entries, 6).unwrap();
        assert_eq!(order.buckets, buckets);
        assert_eq!(order.storage, 6 + heap_cells + buckets);
        assert_eq!(order.lookup, profile.lookup);
        let mut actual = HashMap::<Ptr<Operation>, usize>::new();
        actual.try_reserve(entries).unwrap();
        assert_eq!(actual.capacity(), usable);
    }
    for (count, capacity) in [(0, 0), (1, 4), (3, 4), (4, 4), (5, 5)] {
        let context = &mut setup();
        let function = fanout(context, count);
        let scan = prescan(context, &function).unwrap();
        let result = scan.operations[0][0].deref(context).get_result(0);
        assert_eq!(result.uses(context).capacity(), capacity);
        assert_eq!(use_storage::<Use<Value>>(count).unwrap(), 3 + capacity * 3);
    }
    assert_resource(
        Table::pointers::<Ptr<Operation>>(usize::MAX),
        "def-use closure resource arithmetic",
    );
    assert_resource(
        use_storage::<Use<Value>>(usize::MAX),
        "def-use closure resource arithmetic",
    );
}

#[test]
fn fanout_literal_work_peak_and_exact_boundaries() {
    // B=2/O=3 reserve four buckets each. Every pointer lookup is 408
    // logical visits. The two independent definition/use scans contribute
    // 2*triangular(n), not a linear estimate for a wide successor signature.
    // Physical-order admission adds 39 visits; owner transfer adds 8. Each
    // operand adds 4 classification visits and prepays one 408+4 order query.
    for (count, work, peak) in [
        (0, 3079, 167),
        (1, 4331, 167),
        (3, 6839, 167),
        (4, 8096, 167),
        (5, 9355, 170),
    ] {
        let context = &mut setup();
        let function = fanout(context, count);
        let scan = prescan(context, &function).unwrap();
        reset_trace();
        let bound = check(context, &function, &scan, hard()).unwrap();
        assert_eq!(bound.work_upper_bound(), work, "fanout {count}");
        assert_eq!(bound.peak_storage_upper_bound(), peak, "fanout {count}");
        assert_eq!(bound.retained_storage_upper_bound(), 28);
        assert_eq!(observed().order_indexes, 1);
        assert_eq!(observed().live_order_indexes, 0);
        assert_eq!(observed().retired_order_indexes, 1);
        assert_eq!(observed().definition_rosters, count);
        assert_eq!(observed().user_rosters, count + 1);
        assert_eq!(observed().value_use_vectors, usize::from(count != 0));
        assert_eq!(observed().successor_use_vectors, 1);
        assert_eq!(observed().full_verifications, 0);
        assert_eq!(
            check(
                context,
                &function,
                &scan,
                ProductionAnalysisResourceLimitsV1::new(work, peak)
            )
            .unwrap(),
            bound
        );
        assert_resource(
            check(
                context,
                &function,
                &scan,
                ProductionAnalysisResourceLimitsV1::new(work - 1, peak),
            ),
            "work upper bound",
        );
        assert_resource(
            check(
                context,
                &function,
                &scan,
                ProductionAnalysisResourceLimitsV1::new(work, peak - 1),
            ),
            "peak storage upper bound",
        );
    }
}

#[test]
fn owner_reservations_and_use_vectors_follow_admission() {
    let context = &mut setup();
    let function = fanout(context, 5);
    let scan = prescan(context, &function).unwrap();
    reset_trace();
    assert_resource(
        check(
            context,
            &function,
            &scan,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, 151),
        ),
        "peak storage upper bound",
    );
    assert_eq!(observed().owner_reservations, 0);
    reset_trace();
    assert_resource(
        check(
            context,
            &function,
            &scan,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, 169),
        ),
        "peak storage upper bound",
    );
    assert_eq!(observed().owner_reservations, 1);
    assert_eq!(observed().value_use_vectors, 0);
    assert_eq!(observed().successor_use_vectors, 0);
    // A rejecting invocation must not contaminate the next fresh ledger.
    assert!(check(context, &function, &scan, hard()).is_ok());
}

#[test]
fn actual_prescan_capacity_is_live_during_closure() {
    let context = &mut setup();
    let function = fanout(context, 3);
    let mut scan = prescan(context, &function).unwrap();
    let before = check(context, &function, &scan, hard()).unwrap();
    let old = scan.operations[0].capacity();
    scan.operations[0].reserve_exact(64);
    let delta = (scan.operations[0].capacity() - old) * 2;
    let after = check(context, &function, &scan, hard()).unwrap();
    assert_eq!(before.work_upper_bound(), after.work_upper_bound());
    assert_eq!(
        before.peak_storage_upper_bound() + delta,
        after.peak_storage_upper_bound()
    );
}

#[test]
fn duplicate_and_detached_owner_rosters_are_rejected() {
    let context = &mut setup();
    let function = fanout(context, 1);
    let mut scan = prescan(context, &function).unwrap();
    scan.blocks[1] = scan.blocks[0];
    assert!(matches!(
        check(context, &function, &scan, hard()),
        Err(Failure::Invalid(
            "identity block roster is duplicate or detached"
        ))
    ));
    let mut scan = prescan(context, &function).unwrap();
    scan.operations[0][1] = scan.operations[0][0];
    assert!(matches!(
        check(context, &function, &scan, hard()),
        Err(Failure::Invalid(
            "identity operation roster is duplicate or detached"
        ))
    ));
    let mut scan = prescan(context, &function).unwrap();
    scan.operations.swap(0, 1);
    assert!(matches!(
        check(context, &function, &scan, hard()),
        Err(Failure::Invalid(
            "identity operation roster is duplicate or detached"
        ))
    ));
}

#[test]
fn inconsistent_rosters_pay_the_actual_live_floor_before_diagnostics() {
    for missing_block in [false, true] {
        let context = &mut setup();
        let function = fanout(context, 1);
        let mut scan = prescan(context, &function).unwrap();
        if missing_block {
            scan.blocks.pop();
        } else {
            scan.operations.pop();
        }
        scan.operations[0].reserve_exact(1024);
        let mut budget = Budget::new(hard()).unwrap();
        let failure = check_inner(context, &function, &scan, &mut budget).unwrap_err();
        assert!(matches!(
            failure,
            Failure::Invalid("identity block and operation rosters disagree")
        ));
        assert!(budget.incoming > 2048);
        budget.admit_failure(&failure).unwrap();
        let work = budget.work;
        let peak = budget.peak;
        reset_trace();
        assert!(matches!(
            check(
                context,
                &function,
                &scan,
                ProductionAnalysisResourceLimitsV1::new(work, peak)
            ),
            Err(Failure::Invalid(
                "identity block and operation rosters disagree"
            ))
        ));
        assert_resource(
            check(
                context,
                &function,
                &scan,
                ProductionAnalysisResourceLimitsV1::new(work - 1, peak),
            ),
            "work upper bound",
        );
        assert_resource(
            check(
                context,
                &function,
                &scan,
                ProductionAnalysisResourceLimitsV1::new(work, peak - 1),
            ),
            "peak storage upper bound",
        );
        assert_eq!(observed().owner_reservations, 0);
    }
}

#[test]
fn mismatched_forward_and_reverse_cardinalities_are_rejected() {
    let context = &mut setup();
    let function = fanout(context, 3);
    for successor in [false, true] {
        let mut scan = prescan(context, &function).unwrap();
        if successor {
            scan.successors += 1;
        } else {
            scan.operands += 1;
        }
        assert!(matches!(
            check(context, &function, &scan, hard()),
            Err(Failure::Invalid(
                "local operands or successors have missing use backlinks"
            ))
        ));
    }
}

#[test]
fn successful_real_capture_includes_closure_and_rejects_one_under_before_verification() {
    let context = &mut setup();
    let function = fanout(context, 5);
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
    reset_trace();
    let capture = provider
        .capture_with_resource_limits_v1(hard())
        .ok()
        .unwrap();
    assert_eq!(observed().full_verifications, 1);
    let bound = capture.resource_upper_bound;
    let work = bound.work_upper_bound();
    let peak = bound.peak_storage_upper_bound();
    let exact = provider
        .capture_with_resource_limits_v1(ProductionAnalysisResourceLimitsV1::new(work, peak))
        .ok()
        .unwrap();
    assert_eq!(exact.resource_upper_bound, bound);
    assert!(
        capture
            .snapshot
            .identity
            .exactly_matches(&exact.snapshot.identity)
    );
    for limits in [
        ProductionAnalysisResourceLimitsV1::new(work - 1, peak),
        ProductionAnalysisResourceLimitsV1::new(work, peak - 1),
    ] {
        reset_trace();
        assert!(matches!(
            provider.capture_with_resource_limits_v1(limits),
            Err(IdentityCaptureFailureV1::ResourceLimit(_))
        ));
        assert_eq!(observed().full_verifications, 0);
    }
}
