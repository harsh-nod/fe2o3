use dialect_kernel::{
    AccessKindAttr, BranchOp, DIALECT_NAME, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexLessThanBranchOp, InvocationIndexOp, MemorySpaceAttr, RankedAccessOp, RankedViewOp,
    RankedViewType, ReturnOp, register_dialect,
};
use fe2o3_kernel_analysis::{
    KernelCheckPassKindV1, RankedRaceFindingV1, RankedRaceStatusV1,
    require_pliron_ranked_race_freedom_before_lowering_v1, run_pliron_ranked_race_check_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    r#type::TypeHandle,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(
        &mut context,
        &DialectName::try_new(DIALECT_NAME).expect("valid dialect"),
    )
    .expect("register kernel dialect");
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
    assert_eq!(report.status(), RankedRaceStatusV1::Clean);
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
    assert_eq!(report.status(), RankedRaceStatusV1::Rejected);
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
                == RankedRaceStatusV1::Rejected,
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
        let atomic = RankedAccessOp::new(
            context,
            AccessKindAttr::AtomicReadModifyWrite,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        let other = RankedAccessOp::new(
            context,
            other,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        let ret = ReturnOp::new(context);
        append(context, entry, &memory);
        append(context, entry, &invocation);
        append(context, entry, &zero);
        append(context, entry, &atomic);
        append(context, entry, &other);
        append(context, entry, &ret);
        assert_eq!(
            run_pliron_ranked_race_check_v1(context, &function).status()
                == RankedRaceStatusV1::Rejected,
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
        let atomic_read = RankedAccessOp::new(
            context,
            AccessKindAttr::AtomicRead,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        let other = RankedAccessOp::new(
            context,
            other,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        let ret = ReturnOp::new(context);
        append(context, entry, &memory);
        append(context, entry, &invocation);
        append(context, entry, &zero);
        append(context, entry, &atomic_read);
        append(context, entry, &other);
        append(context, entry, &ret);
        assert_eq!(
            run_pliron_ranked_race_check_v1(context, &function).status(),
            RankedRaceStatusV1::Clean,
        );
    }
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
fn dynamic_launch_identity_is_proved_symbolically_after_a_bounds_guard() {
    let context = &mut setup();
    let function = function(context, "dynamic_identity");
    let entry = function.get_entry_block(context);
    let access_block = block(context, &function, "access");
    let exit = block(context, &function, "exit");
    let output = view(context, vec![1024], MemorySpaceAttr::Global);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let extent = IndexConstantOp::new(context, 1024);
    let branch = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        extent.result(context),
        access_block,
        exit,
    );
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        output.result(context),
        vec![invocation.result(context)],
    )
    .unwrap();
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &output);
    append(context, entry, &invocation);
    append(context, entry, &extent);
    append(context, entry, &branch);
    append(context, access_block, &write);
    append(context, access_block, &to_exit);
    append(context, exit, &ret);

    assert!(run_pliron_ranked_race_check_v1(context, &function).is_clean());
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
                RankedRaceStatusV1::Rejected
            } else {
                RankedRaceStatusV1::Clean
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
    assert_eq!(report.status(), RankedRaceStatusV1::Rejected);
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

#[test]
fn dialect_index_type_is_still_the_only_function_index_type() {
    let context = &mut setup();
    let index: TypeHandle = dialect_kernel::IndexType::get(context).into();
    assert!(index.deref(context).is::<dialect_kernel::IndexType>());
}
