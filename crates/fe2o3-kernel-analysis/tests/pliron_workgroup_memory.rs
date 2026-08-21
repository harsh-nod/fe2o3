use dialect_gpu::{AddressSpaceAttr, BarrierOp, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
use dialect_kernel::{
    AccessKindAttr, DIALECT_NAME, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    InvocationIndexOp, MemorySpaceAttr, RankedAccessOp, RankedViewOp, RankedViewType, ReturnOp,
    register_dialect,
};
use fe2o3_kernel_analysis::{
    KernelCheckPassKindV1, PlironWorkgroupMemoryFindingV1,
    require_pliron_workgroup_memory_safety_before_lowering_v1,
    run_pliron_workgroup_memory_check_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    value::Value,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    context
}

fn function(context: &mut Context, name: &str) -> FuncOp {
    FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    )
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn view(context: &mut Context) -> RankedViewOp {
    let ty = RankedViewType::new(context, 32, true, vec![64]).unwrap();
    RankedViewOp::new_in_space(context, ty, vec![], MemorySpaceAttr::Workgroup).unwrap()
}

fn barrier(context: &mut Context, address_space: AddressSpaceAttr) -> BarrierOp {
    BarrierOp::new(
        context,
        HierarchyAttr::Workgroup,
        MemoryScopeAttr::Workgroup,
        address_space,
        MemoryOrderAttr::AcquireRelease,
    )
}

fn access(
    context: &mut Context,
    kind: AccessKindAttr,
    view: &RankedViewOp,
    index: Value,
) -> RankedAccessOp {
    RankedAccessOp::new(context, kind, view.result(context), vec![index]).unwrap()
}

#[test]
fn read_before_any_write_is_rejected_with_exact_invocation_and_address() {
    let context = &mut setup();
    let function = function(context, "read_before_write");
    let entry = function.get_entry_block(context);
    let shared = view(context);
    let invocation = InvocationIndexOp::new(context, 0, 8);
    let read = access(
        context,
        AccessKindAttr::Read,
        &shared,
        invocation.result(context),
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &shared);
    append(context, entry, &invocation);
    append(context, entry, &read);
    append(context, entry, &ret);

    let error =
        require_pliron_workgroup_memory_safety_before_lowering_v1(context, &function).unwrap_err();
    assert!(matches!(
        error.report().findings().first(),
        Some(PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization { .. })
    ));
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("error[FE2O3-WORKGROUP-001]"));
    assert!(diagnostic.contains("invocation [0]"));
    assert!(diagnostic.contains("address [0]"));
}

#[test]
fn atomic_reads_and_read_modify_write_require_initialization_but_atomic_store_does_not() {
    for (kind, rejected) in [
        (AccessKindAttr::AtomicRead, true),
        (AccessKindAttr::AtomicReadModifyWrite, true),
        (AccessKindAttr::AtomicWrite, false),
    ] {
        let context = &mut setup();
        let function = function(context, "atomic_initialization");
        let entry = function.get_entry_block(context);
        let shared = view(context);
        let invocation = InvocationIndexOp::new(context, 0, 8);
        let effect = access(context, kind, &shared, invocation.result(context));
        let ret = ReturnOp::new(context);
        append(context, entry, &shared);
        append(context, entry, &invocation);
        append(context, entry, &effect);
        append(context, entry, &ret);
        let report = run_pliron_workgroup_memory_check_v1(context, &function);
        assert_eq!(
            report.findings().iter().any(|finding| matches!(
                finding,
                PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization { .. }
            )),
            rejected,
            "unexpected initialization result for {kind:?}",
        );
    }
}

#[test]
fn same_invocation_write_then_read_needs_no_barrier() {
    let context = &mut setup();
    let function = function(context, "private_order_in_shared_memory");
    let entry = function.get_entry_block(context);
    let shared = view(context);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let write = access(
        context,
        AccessKindAttr::Write,
        &shared,
        invocation.result(context),
    );
    let read = access(
        context,
        AccessKindAttr::Read,
        &shared,
        invocation.result(context),
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &shared);
    append(context, entry, &invocation);
    append(context, entry, &write);
    append(context, entry, &read);
    append(context, entry, &ret);
    assert!(run_pliron_workgroup_memory_check_v1(context, &function).is_clean());
}

#[test]
fn convergent_publish_makes_neighbor_values_initialized() {
    let context = &mut setup();
    let function = function(context, "published_neighbor");
    let entry = function.get_entry_block(context);
    let shared = view(context);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let one = IndexConstantOp::new(context, 1);
    let extent = IndexConstantOp::new(context, 64);
    let plus_one = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        invocation.result(context),
        one.result(context),
    );
    let neighbor = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        plus_one.result(context),
        extent.result(context),
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        &shared,
        invocation.result(context),
    );
    let publish = barrier(context, AddressSpaceAttr::Workgroup);
    let read = access(
        context,
        AccessKindAttr::Read,
        &shared,
        neighbor.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        shared.get_operation(),
        invocation.get_operation(),
        one.get_operation(),
        extent.get_operation(),
        plus_one.get_operation(),
        neighbor.get_operation(),
        write.get_operation(),
        publish.get_operation(),
        read.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let report = run_pliron_workgroup_memory_check_v1(context, &function);
    assert_eq!(report.pass(), KernelCheckPassKindV1::WorkgroupMemory);
    assert!(report.is_clean());
}

#[test]
fn missing_or_wrong_space_barrier_does_not_publish_neighbor_values() {
    for address_space in [None, Some(AddressSpaceAttr::Global)] {
        let context = &mut setup();
        let function = function(context, "missing_publish");
        let entry = function.get_entry_block(context);
        let shared = view(context);
        let invocation = InvocationIndexOp::new(context, 0, 2);
        let one = IndexConstantOp::new(context, 1);
        let two = IndexConstantOp::new(context, 2);
        let neighbor = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Remainder,
            one.result(context),
            two.result(context),
        );
        let write = access(
            context,
            AccessKindAttr::Write,
            &shared,
            invocation.result(context),
        );
        let read = access(
            context,
            AccessKindAttr::Read,
            &shared,
            neighbor.result(context),
        );
        append(context, entry, &shared);
        append(context, entry, &invocation);
        append(context, entry, &one);
        append(context, entry, &two);
        append(context, entry, &neighbor);
        append(context, entry, &write);
        if let Some(address_space) = address_space {
            let sync = barrier(context, address_space);
            append(context, entry, &sync);
        }
        append(context, entry, &read);
        let ret = ReturnOp::new(context);
        append(context, entry, &ret);
        let report = run_pliron_workgroup_memory_check_v1(context, &function);
        assert!(report.findings().iter().any(|finding| matches!(
            finding,
            PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization { .. }
        )));
    }
}

#[test]
fn duplicate_plain_writes_race_but_atomic_writes_do_not() {
    for (kind, clean) in [
        (AccessKindAttr::Write, false),
        (AccessKindAttr::AtomicWrite, true),
    ] {
        let context = &mut setup();
        let function = function(context, "duplicate_shared_write");
        let entry = function.get_entry_block(context);
        let shared = view(context);
        let invocation = InvocationIndexOp::new(context, 0, 8);
        let zero = IndexConstantOp::new(context, 0);
        let write = access(context, kind, &shared, zero.result(context));
        let ret = ReturnOp::new(context);
        append(context, entry, &shared);
        append(context, entry, &invocation);
        append(context, entry, &zero);
        append(context, entry, &write);
        append(context, entry, &ret);
        let report = run_pliron_workgroup_memory_check_v1(context, &function);
        assert_eq!(report.is_clean(), clean);
        if !clean {
            assert!(report.findings().iter().any(|finding| matches!(
                finding,
                PlironWorkgroupMemoryFindingV1::ConflictingEffects { .. }
            )));
        }
    }
}

#[test]
fn workgroup_barrier_starts_a_new_race_epoch() {
    let context = &mut setup();
    let function = function(context, "reuse_epoch");
    let entry = function.get_entry_block(context);
    let shared = view(context);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let first = access(
        context,
        AccessKindAttr::Write,
        &shared,
        invocation.result(context),
    );
    let sync = barrier(context, AddressSpaceAttr::Workgroup);
    let second = access(
        context,
        AccessKindAttr::Write,
        &shared,
        invocation.result(context),
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &shared);
    append(context, entry, &invocation);
    append(context, entry, &first);
    append(context, entry, &sync);
    append(context, entry, &second);
    append(context, entry, &ret);
    assert!(run_pliron_workgroup_memory_check_v1(context, &function).is_clean());
}
