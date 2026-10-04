use crate::{
    KernelCheckPassKindV1, KernelCheckStatusV1, RankedRaceFindingV1, RankedRaceReportV1,
    require_pliron_ranked_race_freedom_before_lowering_v1, run_pliron_ranked_race_check_v1,
};
use dialect_gpu::{AddressSpaceAttr, ExecutionLayoutOp, FenceOp, MemoryOrderAttr, MemoryScopeAttr};
use dialect_kernel::{
    AccessKindAttr, AllocationEffectOp, AtomicOrderingAttr, AtomicScopeAttr, BranchOp,
    CheckedRowStripedIndex2DOp, CheckedTiledIndex2DOp, DIALECT_NAME,
    GFX950_TRANSPOSE_FP8_WORKGROUP_ALLOCATION_ORIGIN_V1,
    GFX950_TRANSPOSE_FP8_WORKGROUP_NOALIAS_CLASS_V1, IndexBinaryKindAttr, IndexBinaryOp,
    IndexConstantOp, IndexEqualBranchOp, IndexLessThanBranchOp, IndexType, IndexUnknownOp,
    InvocationIndexOp, MemorySpaceAttr, RankedAccessOp, RankedViewOp, RankedViewType, ReturnOp,
    register_dialect,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    r#type::TypeHandle,
    value::Value,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(
        &mut context,
        &DialectName::try_new(DIALECT_NAME).expect("valid dialect"),
    )
    .expect("register kernel dialect");
    dialect_gpu::register_dialect(&mut context).expect("register gpu dialect");
    context
}

fn function(context: &mut Context, name: &str) -> FuncOp {
    let function_type = FunctionType::get(context, vec![], vec![]);
    FuncOp::new(
        context,
        name.try_into().expect("valid function"),
        function_type,
    )
}

fn function_with_index_arguments(
    context: &mut Context,
    name: &str,
    arguments: usize,
) -> (FuncOp, Vec<Value>) {
    let index: TypeHandle = IndexType::get(context).into();
    let function = FuncOp::new(
        context,
        name.try_into().expect("valid function"),
        FunctionType::get(context, vec![index; arguments], vec![]),
    );
    let entry = function.get_entry_block(context);
    let arguments = (0..arguments)
        .map(|ordinal| entry.deref(context).get_argument(ordinal))
        .collect();
    (function, arguments)
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn block(context: &mut Context, function: &FuncOp, name: &str) -> Ptr<BasicBlock> {
    let block = BasicBlock::new(
        context,
        Some(name.try_into().expect("valid block name")),
        vec![],
    );
    block.insert_at_back(function.get_region(context), context);
    block
}

fn view(context: &mut Context, shape: Vec<u64>, memory_space: MemorySpaceAttr) -> RankedViewOp {
    let view_type = RankedViewType::new(context, 32, true, shape).expect("ranked view type");
    RankedViewOp::new_in_space(context, view_type, vec![], memory_space).expect("ranked view")
}

fn view_with_contract(
    context: &mut Context,
    shape: Vec<u64>,
    memory_space: MemorySpaceAttr,
    allocation_origin: u64,
    noalias_class: u64,
) -> RankedViewOp {
    let view_type = RankedViewType::new(context, 32, true, shape).expect("ranked view type");
    RankedViewOp::new_in_space_with_allocation_contract(
        context,
        view_type,
        vec![],
        memory_space,
        allocation_origin,
        noalias_class,
    )
    .expect("ranked view")
}

fn access(
    context: &mut Context,
    kind: AccessKindAttr,
    view: Value,
    index: Value,
) -> RankedAccessOp {
    let atomic_ordering = match kind {
        AccessKindAttr::AtomicRead => Some(AtomicOrderingAttr::Acquire),
        AccessKindAttr::AtomicWrite => Some(AtomicOrderingAttr::Release),
        AccessKindAttr::AtomicReadModifyWrite => Some(AtomicOrderingAttr::AcquireRelease),
        AccessKindAttr::Read | AccessKindAttr::Write => None,
    };
    match atomic_ordering {
        Some(ordering) => RankedAccessOp::new_atomic(
            context,
            kind,
            ordering,
            AtomicScopeAttr::Device,
            view,
            vec![index],
        ),
        None => RankedAccessOp::new(context, kind, view, vec![index]),
    }
    .unwrap()
}

fn large_affine_store_family(formulas: &[(u64, u64)]) -> RankedRaceReportV1 {
    const LAUNCH: u64 = 65_537;

    let context = &mut setup();
    let function = function(context, "large_affine_store_family");
    let entry = function.get_entry_block(context);
    let maximum = formulas
        .iter()
        .map(|(stride, offset)| stride * (LAUNCH - 1) + offset)
        .max()
        .unwrap_or(0);
    let memory = view(context, vec![maximum + 1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, LAUNCH);
    append(context, entry, &memory);
    append(context, entry, &invocation);
    for &(stride, offset) in formulas {
        let stride = IndexConstantOp::new(context, stride);
        let base = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Multiply,
            invocation.result(context),
            stride.result(context),
        );
        let offset = IndexConstantOp::new(context, offset);
        let index = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            base.result(context),
            offset.result(context),
        );
        let write = access(
            context,
            AccessKindAttr::Write,
            memory.result(context),
            index.result(context),
        );
        append(context, entry, &stride);
        append(context, entry, &base);
        append(context, entry, &offset);
        append(context, entry, &index);
        append(context, entry, &write);
    }
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);
    run_pliron_ranked_race_check_v1(context, &function)
}

#[test]
fn identity_write_is_injective_for_every_static_invocation() {
    let context = &mut setup();
    let function = function(context, "identity_write");
    let entry = function.get_entry_block(context);
    let output = view(context, vec![64], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        output.result(context),
        vec![invocation.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &write);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.pass(), KernelCheckPassKindV1::RaceFreedom);
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
    assert!(report.findings().is_empty());
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
}

#[test]
fn constant_output_coordinate_reports_two_exact_invocations() {
    let context = &mut setup();
    let function = function(context, "duplicate_output");
    let entry = function.get_entry_block(context);
    let output = view(context, vec![64], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let zero = IndexConstantOp::new(context, 0);
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        output.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &zero);
    append(context, entry, &write);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    let [
        RankedRaceFindingV1::ConflictingEffects {
            indices,
            first,
            second,
            ..
        },
    ] = report.findings()
    else {
        panic!("unexpected findings: {:?}", report.findings());
    };
    assert_eq!(indices, &[0]);
    assert_eq!(first.invocation(), &[0]);
    assert_eq!(second.invocation(), &[1]);
    assert_eq!(first.access(), AccessKindAttr::Write);
    assert_eq!(second.access(), AccessKindAttr::Write);
    let error = require_pliron_ranked_race_freedom_before_lowering_v1(context, &function)
        .expect_err("duplicate write must stop lowering")
        .to_string();
    assert!(error.contains("error[FE2O3-RACE-001]"));
    assert!(error.contains("distinct concurrent invocations"));
    assert!(error.contains("invocation [0]"));
    assert!(error.contains("invocation [1]"));
}

#[test]
fn read_read_sharing_is_clean_but_read_write_and_write_write_are_rejected() {
    for (first_kind, second_kind, rejected) in [
        (AccessKindAttr::Read, AccessKindAttr::Read, false),
        (AccessKindAttr::Read, AccessKindAttr::Write, true),
        (AccessKindAttr::Write, AccessKindAttr::Read, true),
        (AccessKindAttr::Write, AccessKindAttr::Write, true),
    ] {
        let context = &mut setup();
        let function = function(context, "effect_pair");
        let entry = function.get_entry_block(context);
        let memory = view(context, vec![1], MemorySpaceAttr::Global);
        let invocation = InvocationIndexOp::new(context, 0, 2);
        let zero = IndexConstantOp::new(context, 0);
        let first = RankedAccessOp::new(
            context,
            first_kind,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        let second = RankedAccessOp::new(
            context,
            second_kind,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        let ret = ReturnOp::new(context);
        append(context, entry, &memory);
        append(context, entry, &invocation);
        append(context, entry, &zero);
        append(context, entry, &first);
        append(context, entry, &second);
        append(context, entry, &ret);
        assert_eq!(
            run_pliron_ranked_race_check_v1(context, &function).status()
                == KernelCheckStatusV1::Rejected,
            rejected,
            "unexpected result for {first_kind:?}/{second_kind:?}",
        );
    }
}

#[test]
fn atomics_order_with_atomics_but_not_with_plain_reads_or_writes() {
    for (other, rejected) in [
        (AccessKindAttr::AtomicRead, false),
        (AccessKindAttr::Read, true),
        (AccessKindAttr::Write, true),
    ] {
        let context = &mut setup();
        let function = function(context, "atomic_pair");
        let entry = function.get_entry_block(context);
        let memory = view(context, vec![1], MemorySpaceAttr::Global);
        let invocation = InvocationIndexOp::new(context, 0, 4);
        let zero = IndexConstantOp::new(context, 0);
        let atomic = RankedAccessOp::new_atomic(
            context,
            AccessKindAttr::AtomicReadModifyWrite,
            AtomicOrderingAttr::AcquireRelease,
            AtomicScopeAttr::Device,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        let other = access(context, other, memory.result(context), zero.result(context));
        let ret = ReturnOp::new(context);
        append(context, entry, &memory);
        append(context, entry, &invocation);
        append(context, entry, &zero);
        append(context, entry, &atomic);
        append(context, entry, &other);
        append(context, entry, &ret);
        assert_eq!(
            run_pliron_ranked_race_check_v1(context, &function).status()
                == KernelCheckStatusV1::Rejected,
            rejected,
        );
    }
}

#[test]
fn atomic_reads_share_with_plain_reads_and_all_atomic_effects() {
    for other in [
        AccessKindAttr::Read,
        AccessKindAttr::AtomicRead,
        AccessKindAttr::AtomicWrite,
        AccessKindAttr::AtomicReadModifyWrite,
    ] {
        let context = &mut setup();
        let function = function(context, "atomic_read_pair");
        let entry = function.get_entry_block(context);
        let memory = view(context, vec![1], MemorySpaceAttr::Global);
        let invocation = InvocationIndexOp::new(context, 0, 4);
        let zero = IndexConstantOp::new(context, 0);
        let atomic_read = RankedAccessOp::new_atomic(
            context,
            AccessKindAttr::AtomicRead,
            AtomicOrderingAttr::Acquire,
            AtomicScopeAttr::Device,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        let other = access(context, other, memory.result(context), zero.result(context));
        let ret = ReturnOp::new(context);
        append(context, entry, &memory);
        append(context, entry, &invocation);
        append(context, entry, &zero);
        append(context, entry, &atomic_read);
        append(context, entry, &other);
        append(context, entry, &ret);
        assert_eq!(
            run_pliron_ranked_race_check_v1(context, &function).status(),
            KernelCheckStatusV1::Clean,
        );
    }
}

#[test]
fn cross_workgroup_atomic_overlap_requires_agent_or_wider_scope() {
    for (scope, expected) in [
        (AtomicScopeAttr::Workgroup, KernelCheckStatusV1::Rejected),
        (AtomicScopeAttr::Agent, KernelCheckStatusV1::Clean),
        (AtomicScopeAttr::Device, KernelCheckStatusV1::Clean),
    ] {
        let context = &mut setup();
        let function = function(context, "cross_workgroup_atomic");
        let entry = function.get_entry_block(context);
        let layout = ExecutionLayoutOp::new(context, 41, [128, 1, 1], [64, 1, 1], 64);
        let memory = view(context, vec![1], MemorySpaceAttr::Global);
        let invocation = InvocationIndexOp::new(context, 0, 128);
        let zero = IndexConstantOp::new(context, 0);
        let atomic = RankedAccessOp::new_atomic(
            context,
            AccessKindAttr::AtomicReadModifyWrite,
            AtomicOrderingAttr::AcquireRelease,
            scope,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        let ret = ReturnOp::new(context);
        append(context, entry, &layout);
        append(context, entry, &memory);
        append(context, entry, &invocation);
        append(context, entry, &zero);
        append(context, entry, &atomic);
        append(context, entry, &ret);
        let report = run_pliron_ranked_race_check_v1(context, &function);
        assert_eq!(report.status(), expected, "unexpected status for {scope:?}");
        if scope == AtomicScopeAttr::Workgroup {
            assert!(matches!(
                report.findings(),
                [RankedRaceFindingV1::InsufficientAtomicScope { first, second, .. }]
                    if first.workgroup() == Some(0) && second.workgroup() == Some(1)
            ));
        }
    }
}

#[test]
fn narrow_atomic_overlap_without_layout_is_incomplete() {
    let context = &mut setup();
    let function = function(context, "unresolved_atomic_scope");
    let entry = function.get_entry_block(context);
    let memory = view(context, vec![1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 2);
    let zero = IndexConstantOp::new(context, 0);
    let atomic = RankedAccessOp::new_atomic(
        context,
        AccessKindAttr::AtomicReadModifyWrite,
        AtomicOrderingAttr::AcquireRelease,
        AtomicScopeAttr::Workgroup,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &memory);
    append(context, entry, &invocation);
    append(context, entry, &zero);
    append(context, entry, &atomic);
    append(context, entry, &ret);
    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::ExecutionLayoutUnavailable { .. }]
    ));
}

#[test]
fn fence_only_publication_is_incomplete_without_synchronizes_with() {
    let context = &mut setup();
    let function = function(context, "plain_grid_fence");
    let entry = function.get_entry_block(context);
    let layout = ExecutionLayoutOp::new(context, 42, [128, 1, 1], [64, 1, 1], 64);
    let memory = view(context, vec![1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 128);
    let zero = IndexConstantOp::new(context, 0);
    let first = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let fence = FenceOp::new(
        context,
        MemoryScopeAttr::Device,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::AcquireRelease,
    );
    let second = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &memory);
    append(context, entry, &invocation);
    append(context, entry, &zero);
    append(context, entry, &first);
    append(context, entry, &fence);
    append(context, entry, &second);
    append(context, entry, &ret);
    assert_eq!(
        run_pliron_ranked_race_check_v1(context, &function).status(),
        KernelCheckStatusV1::Incomplete
    );
    assert!(
        run_pliron_ranked_race_check_v1(context, &function)
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                RankedRaceFindingV1::HappensBeforeIncomplete { detail, .. }
                    if detail.contains("fence alone")
            ))
    );
}

#[test]
fn release_store_acquire_load_signaling_needs_a_read_from_proof() {
    let context = &mut setup();
    let function = function(context, "atomic_signal_publication");
    let entry = function.get_entry_block(context);
    let layout = ExecutionLayoutOp::new(context, 43, [128, 1, 1], [64, 1, 1], 64);
    let data = view_with_contract(context, vec![1], MemorySpaceAttr::Global, 431, 431);
    let signal = view_with_contract(context, vec![1], MemorySpaceAttr::Global, 432, 432);
    let invocation = InvocationIndexOp::new(context, 0, 128);
    let zero = IndexConstantOp::new(context, 0);
    let data_write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        data.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let signal_release = RankedAccessOp::new_atomic(
        context,
        AccessKindAttr::AtomicWrite,
        AtomicOrderingAttr::Release,
        AtomicScopeAttr::Agent,
        signal.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let signal_acquire = RankedAccessOp::new_atomic(
        context,
        AccessKindAttr::AtomicRead,
        AtomicOrderingAttr::Acquire,
        AtomicScopeAttr::Agent,
        signal.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let data_read = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        data.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &data);
    append(context, entry, &signal);
    append(context, entry, &invocation);
    append(context, entry, &zero);
    append(context, entry, &data_write);
    append(context, entry, &signal_release);
    append(context, entry, &signal_acquire);
    append(context, entry, &data_read);
    append(context, entry, &ret);
    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        RankedRaceFindingV1::HappensBeforeIncomplete { detail, .. }
            if detail.contains("authenticated read-from relation")
    )));
}

#[test]
fn affine_stride_and_offset_remain_injective() {
    let context = &mut setup();
    let function = function(context, "strided_output");
    let entry = function.get_entry_block(context);
    let output = view(context, vec![128], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let two = IndexConstantOp::new(context, 2);
    let one = IndexConstantOp::new(context, 1);
    let scaled = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        invocation.result(context),
        two.result(context),
    );
    let offset = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        scaled.result(context),
        one.result(context),
    );
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        output.result(context),
        vec![offset.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &two);
    append(context, entry, &one);
    append(context, entry, &scaled);
    append(context, entry, &offset);
    append(context, entry, &write);
    append(context, entry, &ret);
    assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
}

#[test]
fn guarded_overflowing_affine_multiply_is_not_proved_injective() {
    let context = &mut setup();
    let function = function(context, "guarded_overflowing_multiply");
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 3);
    let factor = IndexConstantOp::new(context, 1_u64 << 63);
    let index = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        invocation.result(context),
        factor.result(context),
    );
    let extent = IndexConstantOp::new(context, 1);
    let guard = IndexLessThanBranchOp::new(
        context,
        index.result(context),
        extent.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        index.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &factor);
    append(context, entry, &index);
    append(context, entry, &extent);
    append(context, entry, &guard);
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert_eq!(
        report.findings(),
        &[RankedRaceFindingV1::BoundsPrerequisiteRejected]
    );
}

#[test]
fn guarded_overflowing_affine_add_is_not_proved_injective() {
    let context = &mut setup();
    let function = function(context, "guarded_overflowing_add");
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 2);
    let maximum = IndexConstantOp::new(context, u64::MAX);
    let index = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        invocation.result(context),
        maximum.result(context),
    );
    let extent = IndexConstantOp::new(context, 1);
    let guard = IndexLessThanBranchOp::new(
        context,
        index.result(context),
        extent.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        index.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &maximum);
    append(context, entry, &index);
    append(context, entry, &extent);
    append(context, entry, &guard);
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert_eq!(
        report.findings(),
        &[RankedRaceFindingV1::BoundsPrerequisiteRejected]
    );
}

include!("pliron_race/checked_access_tests.rs");
include!("pliron_race/checked_tiled_layout_values_v1_tests.rs");
include!("pliron_race/dynamic_domain_tests.rs");
#[test]
fn exact_fallback_retains_both_orders_of_static_effect_pairs() {
    let context = &mut setup();
    let function = function(context, "ordered_conflict_classes");
    let entry = function.get_entry_block(context);
    let output = view(context, vec![2], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 4);
    let one = IndexConstantOp::new(context, 1);
    let two = IndexConstantOp::new(context, 2);
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        invocation.result(context),
        one.result(context),
    );
    let first_index = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        invocation.result(context),
        two.result(context),
    );
    let second_index = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        next.result(context),
        two.result(context),
    );
    let first = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        first_index.result(context),
    );
    let second = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        second_index.result(context),
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &one);
    append(context, entry, &two);
    append(context, entry, &next);
    append(context, entry, &first_index);
    append(context, entry, &second_index);
    append(context, entry, &first);
    append(context, entry, &second);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    let pairs = report
        .findings()
        .iter()
        .map(|finding| match finding {
            RankedRaceFindingV1::ConflictingEffects { first, second, .. } => {
                (first.location().operation(), second.location().operation())
            }
            finding => panic!("unexpected finding: {finding:?}"),
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        pairs,
        [(7, 7), (7, 8), (8, 7), (8, 8)].into_iter().collect()
    );
    assert_eq!(report.findings().len(), 4);
}

#[test]
fn remainder_mapping_reports_wraparound_collision() {
    let context = &mut setup();
    let function = function(context, "wrapped_output");
    let entry = function.get_entry_block(context);
    let output = view(context, vec![32], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let modulus = IndexConstantOp::new(context, 32);
    let wrapped = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        invocation.result(context),
        modulus.result(context),
    );
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        output.result(context),
        vec![wrapped.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &modulus);
    append(context, entry, &wrapped);
    append(context, entry, &write);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    let finding = report
        .findings()
        .iter()
        .find_map(|finding| match finding {
            RankedRaceFindingV1::ConflictingEffects { first, second, .. } => Some((first, second)),
            _ => None,
        })
        .expect("wraparound conflict");
    assert_eq!(finding.0.invocation(), &[0]);
    assert_eq!(finding.1.invocation(), &[32]);
}

#[test]
fn guarded_quotient_mapping_uses_exact_fallback_and_reports_a_collision() {
    let context = &mut setup();
    let function = function(context, "quotient_output");
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![4], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let divisor = IndexConstantOp::new(context, 16);
    let quotient = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Divide,
        invocation.result(context),
        divisor.result(context),
    );
    let extent = IndexConstantOp::new(context, 4);
    let guard = IndexLessThanBranchOp::new(
        context,
        quotient.result(context),
        extent.result(context),
        access_block,
        exit,
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        output.result(context),
        quotient.result(context),
    );
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &divisor);
    append(context, entry, &quotient);
    append(context, entry, &extent);
    append(context, entry, &guard);
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    let (first, second) = report
        .findings()
        .iter()
        .find_map(|finding| match finding {
            RankedRaceFindingV1::ConflictingEffects { first, second, .. } => Some((first, second)),
            _ => None,
        })
        .expect("quotient collision");
    assert_eq!(first.invocation(), &[0]);
    assert_eq!(second.invocation(), &[1]);
}

#[test]
fn multidimensional_identity_is_clean_and_dropped_dimension_collides() {
    for drop_y in [false, true] {
        let context = &mut setup();
        let function = function(context, "image_output");
        let entry = function.get_entry_block(context);
        let output = view(context, vec![3, 4], MemorySpaceAttr::Global);
        let x = InvocationIndexOp::new(context, 0, 4);
        let y = InvocationIndexOp::new(context, 1, 3);
        let zero = IndexConstantOp::new(context, 0);
        let write = RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            output.result(context),
            vec![
                if drop_y {
                    zero.result(context)
                } else {
                    y.result(context)
                },
                x.result(context),
            ],
        )
        .unwrap();
        let ret = ReturnOp::new(context);
        append(context, entry, &output);
        append(context, entry, &x);
        append(context, entry, &y);
        append(context, entry, &zero);
        append(context, entry, &write);
        append(context, entry, &ret);
        assert_eq!(
            run_pliron_ranked_race_check_v1(context, &function).status(),
            if drop_y {
                KernelCheckStatusV1::Rejected
            } else {
                KernelCheckStatusV1::Clean
            },
        );
    }
}

#[test]
fn private_memory_and_single_invocation_do_not_create_inter_invocation_races() {
    for (space, extent) in [(MemorySpaceAttr::Private, 64), (MemorySpaceAttr::Global, 1)] {
        let context = &mut setup();
        let function = function(context, "nonconcurrent_constant");
        let entry = function.get_entry_block(context);
        let memory = view(context, vec![1], space);
        let invocation = InvocationIndexOp::new(context, 0, extent);
        let zero = IndexConstantOp::new(context, 0);
        let write = RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        let ret = ReturnOp::new(context);
        append(context, entry, &memory);
        append(context, entry, &invocation);
        append(context, entry, &zero);
        append(context, entry, &write);
        append(context, entry, &ret);
        assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
    }
}

#[test]
fn dynamic_global_launch_needs_symbolic_disjointness_and_workgroup_effects_defer() {
    let context = &mut setup();
    let global_function = function(context, "unresolved_domain");
    let entry = global_function.get_entry_block(context);
    let memory = view(context, vec![1], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let zero = IndexConstantOp::new(context, 0);
    let read = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &memory);
    append(context, entry, &invocation);
    append(context, entry, &zero);
    append(context, entry, &read);
    append(context, entry, &ret);
    assert!(run_pliron_ranked_race_check_v1(context, &global_function).is_clean());

    let constant_write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    constant_write
        .get_operation()
        .insert_before(context, ret.get_operation());
    let report = run_pliron_ranked_race_check_v1(context, &global_function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(
        report.findings()[0]
            .to_string()
            .contains("dynamic launch dimension")
    );

    let context = &mut setup();
    let function = function(context, "workgroup_deferred");
    let entry = function.get_entry_block(context);
    let memory = view(context, vec![2], MemorySpaceAttr::Workgroup);
    let invocation = InvocationIndexOp::new(context, 0, 2);
    let access = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        memory.result(context),
        vec![invocation.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &memory);
    append(context, entry, &invocation);
    append(context, entry, &access);
    append(context, entry, &ret);
    assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
}

#[test]
fn oversized_static_launch_is_rejected_before_effect_enumeration() {
    let context = &mut setup();
    let function = function(context, "oversized_launch");
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 65_537);
    let memory = view(context, vec![1], MemorySpaceAttr::Global);
    let zero = IndexConstantOp::new(context, 0);
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &invocation);
    append(context, entry, &memory);
    append(context, entry, &zero);
    append(context, entry, &write);
    append(context, entry, &ret);
    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::LaunchDomainTooLarge {
            invocations: 65_537,
            ..
        }]
    ));
}

include!("pliron_race/presburger_tests.rs");
#[test]
fn declared_layout_checks_constant_effect_without_invocation_index() {
    let context = &mut setup();
    let function = function(context, "constant_without_index");
    let entry = function.get_entry_block(context);
    let layout = ExecutionLayoutOp::new(context, 50, [64, 1, 1], [64, 1, 1], 64);
    let memory = view_with_contract(context, vec![1], MemorySpaceAttr::Global, 50, 50);
    let zero = IndexConstantOp::new(context, 0);
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &memory);
    append(context, entry, &zero);
    append(context, entry, &write);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::ConflictingEffects { first, second, .. }]
            if first.invocation() == [0, 0, 0] && second.invocation() == [1, 0, 0]
    ));
}

#[test]
fn overlapping_atomics_require_both_scopes_to_cover_the_pair() {
    let context = &mut setup();
    let function = function(context, "mixed_atomic_scopes");
    let entry = function.get_entry_block(context);
    let layout = ExecutionLayoutOp::new(context, 51, [128, 1, 1], [64, 1, 1], 64);
    let memory = view_with_contract(context, vec![1], MemorySpaceAttr::Global, 51, 51);
    let zero = IndexConstantOp::new(context, 0);
    let agent_write = RankedAccessOp::new_atomic(
        context,
        AccessKindAttr::AtomicWrite,
        AtomicOrderingAttr::Release,
        AtomicScopeAttr::Agent,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let workgroup_read = RankedAccessOp::new_atomic(
        context,
        AccessKindAttr::AtomicRead,
        AtomicOrderingAttr::Acquire,
        AtomicScopeAttr::Workgroup,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &layout);
    append(context, entry, &memory);
    append(context, entry, &zero);
    append(context, entry, &agent_write);
    append(context, entry, &workgroup_read);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        RankedRaceFindingV1::InsufficientAtomicScope { first, second, .. }
            if first.workgroup() == Some(0) && second.workgroup() == Some(1)
    )));
}

include!("pliron_race/alias_tests.rs");
include!("pliron_race/stable_root_layouts_v1_tests.rs");
