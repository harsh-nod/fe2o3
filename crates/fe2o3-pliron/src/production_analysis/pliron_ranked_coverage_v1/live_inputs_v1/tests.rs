use super::*;
use crate::production_analysis::{
    LivePlironStructuralIdentityProviderV1, PlironStructuralIdentityProviderV1,
    ProductionAnalysisResourceLimitsV1 as Limits,
};
use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
use dialect_kernel::{
    BranchOp, DYNAMIC_EXTENT, IndexLessThanBranchOp, IndexType, InvocationIndexOp, MemorySpaceAttr,
    OwnershipContractOp, OwnershipCoverageAttr, OwnershipPartitionAttr, RankedAccessOp,
    RankedViewOp, RankedViewType, ReturnOp, TrapOp,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
    dialect::DialectName,
    op::Op as _,
    operation::verify_operation,
};

struct Fixture {
    function: FuncOp,
    ownership: Ptr<Operation>,
    output: LiveValue,
    reads: [LiveReadBoundV1; 2],
    census: Census,
    epoch: u64,
    context: Context,
}

// The raw arena is a descriptive checker fixture, not source admission or a
// conditional production receipt. Both inputs have distinct dynamic extents.
fn fixture(global: bool, input_space: MemorySpaceAttr, writable: bool) -> Fixture {
    fixture_with_guards(global, input_space, writable, true)
}

fn fixture_with_guards(
    global: bool,
    input_space: MemorySpaceAttr,
    writable: bool,
    guard_inputs: bool,
) -> Fixture {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    let index: TypeHandle = IndexType::get(&context).into();
    let signature = FunctionType::get(&context, vec![index; 3], vec![]);
    let function = FuncOp::new(
        &mut context,
        "conditional_inputs".try_into().unwrap(),
        signature,
    );
    let entry = function.get_entry_block(&context);
    let mut blocks = Vec::new();
    for name in ["guard_a", "guard_b", "body", "exit", "trap"] {
        let block = BasicBlock::new(&mut context, Some(name.try_into().unwrap()), vec![]);
        block.insert_at_back(function.get_region(&context), &context);
        blocks.push(block);
    }
    let [guard_a, guard_b, body, exit, trap] = blocks.as_slice() else {
        unreachable!()
    };
    let layout = ExecutionLayoutOp::new_with_domain(
        &mut context,
        17,
        [0, 1, 1],
        [2, 1, 1],
        2,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    layout.get_operation().insert_at_back(entry, &context);
    let invocation = InvocationIndexOp::new(&mut context, 0, 0);
    invocation.get_operation().insert_at_back(entry, &context);
    let point = invocation.result(&context);
    let mut views = Vec::new();
    for argument in 0..3 {
        let extent = entry.deref(&context).get_argument(argument);
        let ty = RankedViewType::new(
            &context,
            32,
            argument == 0 || writable,
            vec![DYNAMIC_EXTENT],
        )
        .unwrap();
        let view = RankedViewOp::new_in_space_with_allocation_contract(
            &mut context,
            ty,
            vec![extent],
            if argument == 0 {
                MemorySpaceAttr::Global
            } else {
                input_space
            },
            argument as u64 + 1,
            argument as u64 + 1,
        )
        .unwrap();
        view.get_operation().insert_at_back(entry, &context);
        views.push(view.result(&context));
    }
    let ownership = OwnershipContractOp::new(
        &mut context,
        views[0],
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    ownership.get_operation().insert_at_back(entry, &context);
    let mut reads = Vec::new();
    for argument in 1..3 {
        let read = RankedAccessOp::new(
            &mut context,
            AccessKindAttr::Read,
            views[argument],
            vec![point],
        )
        .unwrap();
        read.get_operation()
            .insert_at_back(if global { entry } else { *body }, &context);
        reads.push(LiveReadBoundV1 {
            operation: read.get_operation(),
            view: views[argument],
            index: point,
            extent: entry.deref(&context).get_argument(argument),
            domain: if global {
                Domain::GlobalLaunch
            } else {
                Domain::GuardedOutput
            },
        });
    }
    let output_extent = entry.deref(&context).get_argument(0);
    IndexLessThanBranchOp::new(&mut context, point, output_extent, *guard_a, *exit)
        .get_operation()
        .insert_at_back(entry, &context);
    for (block, argument, successor) in [(*guard_a, 1, *guard_b), (*guard_b, 2, *body)] {
        if guard_inputs {
            let extent = entry.deref(&context).get_argument(argument);
            IndexLessThanBranchOp::new(&mut context, point, extent, successor, *trap)
                .get_operation()
                .insert_at_back(block, &context);
        } else {
            BranchOp::new(&mut context, successor)
                .get_operation()
                .insert_at_back(block, &context);
        }
    }
    RankedAccessOp::new(&mut context, AccessKindAttr::Write, views[0], vec![point])
        .unwrap()
        .get_operation()
        .insert_at_back(*body, &context);
    BranchOp::new(&mut context, *exit)
        .get_operation()
        .insert_at_back(*body, &context);
    ReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(*exit, &context);
    TrapOp::new(&mut context)
        .get_operation()
        .insert_at_back(*trap, &context);
    verify_operation(function.get_operation(), &context).unwrap();
    let census = LivePlironStructuralIdentityProviderV1::new(&context, &function)
        .capture_with_resource_limits_v1(Limits::production_hard_ceiling())
        .ok()
        .unwrap()
        .input_census;
    let epoch = context.ir_mutation_attempt_epoch().unwrap().value();
    Fixture {
        function,
        ownership: ownership.get_operation(),
        output: views[0],
        reads: reads.try_into().unwrap(),
        census,
        epoch,
        context,
    }
}

fn query(f: &Fixture, reads: &[LiveReadBoundV1]) -> LiveResultV1 {
    query_mode(f, Some(reads))
}

fn query_mode(f: &Fixture, reads: Option<&[LiveReadBoundV1]>) -> LiveResultV1 {
    // Use the same structural-capture admission as the production manager.
    let capture = LivePlironStructuralIdentityProviderV1::new(&f.context, &f.function)
        .capture_with_resource_limits_v1(Limits::production_hard_ceiling())
        .ok()
        .unwrap();
    let mut am = Manager::new_with_resource_contract(
        &f.function,
        f.census,
        capture.resource_upper_bound,
        0,
        Limits::production_hard_ceiling(),
    )
    .unwrap();
    am.prepare_function_inventory(&f.context, &f.function);
    let inventory = am.function_inventory_handle().unwrap();
    match reads {
        Some(reads) => check_conditional_ownership_live_rule_with_input_bounds_v1(
            (&f.context, &f.function),
            inventory.as_ref(),
            f.census,
            f.epoch,
            (f.ownership, f.output),
            reads,
            &mut am,
            None,
        ),
        None => check_conditional_ownership_live_rule_with_observation_v1(
            (&f.context, &f.function),
            inventory.as_ref(),
            f.census,
            f.epoch,
            (f.ownership, f.output),
            &mut am,
            None,
        ),
    }
}

#[test]
fn live_input_bounds_replay_guarded_and_launch_domains() {
    for global in [false, true] {
        let f = fixture(global, MemorySpaceAttr::Global, false);
        let facts = query(&f, &f.reads).unwrap().unwrap();
        assert_eq!(facts.normal_exits, [4, 4]);
        assert_eq!(query(&f, &f.reads), Ok(Ok(facts)));
        assert_eq!(query(&f, &[]), Ok(Err(RuleRefusalV1::Coordinate)));
        let mut swapped = f.reads;
        swapped.swap(0, 1);
        assert_eq!(query(&f, &swapped), Ok(Ok(facts)));
    }
}

#[test]
fn empty_explicit_roster_never_selects_legacy_coverage() {
    for global in [false, true] {
        let f = fixture_with_guards(global, MemorySpaceAttr::Global, false, false);
        assert!(query_mode(&f, None).unwrap().is_ok());
        assert!(query(&f, &f.reads).unwrap().is_ok());
        assert_eq!(query(&f, &[]), Ok(Err(RuleRefusalV1::Coordinate)));
    }
}

#[test]
fn live_input_bounds_require_complete_unique_exact_read_roster() {
    let f = fixture(false, MemorySpaceAttr::Global, false);
    for fault in 0..9 {
        let mut reads = f.reads.to_vec();
        match fault {
            0 => {
                reads.pop();
            }
            1 => reads.push(reads[0]),
            2 => reads[1].operation = reads[0].operation,
            3 => reads[0].extent = reads[1].extent,
            4 => reads[0].view = reads[1].view,
            5 => reads[0].index = reads[0].extent,
            6 => reads[0].operation = f.ownership,
            7 => reads[0].view = f.output,
            8 => reads[0].domain = Domain::GlobalLaunch,
            _ => unreachable!(),
        }
        assert_eq!(
            query(&f, &reads),
            Ok(Err(RuleRefusalV1::Coordinate)),
            "fault {fault}"
        );
    }
    let f = fixture(true, MemorySpaceAttr::Global, false);
    let mut reads = f.reads;
    reads[0].domain = Domain::GuardedOutput;
    assert_eq!(query(&f, &reads), Ok(Err(RuleRefusalV1::Coordinate)));
}

#[test]
fn live_input_bounds_reject_non_global_and_writable_inputs() {
    for (space, writable) in [
        (MemorySpaceAttr::Workgroup, false),
        (MemorySpaceAttr::Global, true),
    ] {
        let f = fixture(false, space, writable);
        assert_eq!(query(&f, &f.reads), Ok(Err(RuleRefusalV1::Coordinate)));
    }
}

#[test]
fn live_input_bounds_reject_stale_epoch_before_using_coordinates() {
    let f = fixture(false, MemorySpaceAttr::Global, false);
    drop(f.ownership.deref_mut(&f.context));
    assert_eq!(query(&f, &f.reads), Ok(Err(RuleRefusalV1::Coordinate)));
}

#[test]
fn live_input_bounds_admit_work_and_storage_before_replay() {
    let f = fixture(false, MemorySpaceAttr::Global, false);
    let hard = Limits::production_hard_ceiling();
    // Isolate this query's admission boundary from structural capture's larger
    // temporary peak, while retaining a nonzero inherited work/storage prefix.
    let initial = Bound::checked_phase(Phase::StructuralIdentity, 11, 37, 0).unwrap();
    let mut am =
        Manager::new_with_resource_contract(&f.function, f.census, initial, 0, hard).unwrap();
    am.prepare_function_inventory(&f.context, &f.function);
    let inventory = am.function_inventory_handle().unwrap();
    let floor = am.resource_upper_bound();
    let expected = floor
        .checked_then_retain(
            preflight(f.census, f.reads.len()).unwrap(),
            Phase::HierarchicalOwnership,
        )
        .unwrap();
    assert!(
        check_conditional_ownership_live_rule_with_input_bounds_v1(
            (&f.context, &f.function),
            inventory.as_ref(),
            f.census,
            f.epoch,
            (f.ownership, f.output),
            &f.reads,
            &mut am,
            None,
        )
        .unwrap()
        .is_ok()
    );
    assert_eq!(am.resource_upper_bound(), expected);
    drop((inventory, am));
    for (work_short, storage_short, resource) in [
        (0, 0, None),
        (1, 0, Some("work upper bound")),
        (0, 1, Some("peak storage upper bound")),
    ] {
        let mut am = Manager::new_with_resource_contract(
            &f.function,
            f.census,
            initial,
            0,
            Limits::new(
                expected.work_upper_bound() - work_short,
                expected.peak_storage_upper_bound() - storage_short,
            ),
        )
        .unwrap();
        am.prepare_function_inventory(&f.context, &f.function);
        let inventory = am.function_inventory_handle().unwrap();
        let prefix = am.resource_upper_bound();
        let result = check_conditional_ownership_live_rule_with_input_bounds_v1(
            (&f.context, &f.function),
            inventory.as_ref(),
            f.census,
            f.epoch,
            (f.ownership, f.output),
            &f.reads,
            &mut am,
            None,
        );
        if let Some(resource) = resource {
            assert_eq!(result, Err(live_limit(resource)));
            assert_eq!(am.resource_upper_bound(), prefix);
        } else {
            assert!(result.unwrap().is_ok());
            assert_eq!(am.resource_upper_bound(), expected);
        }
    }
    assert!(
        preflight(f.census, 0).unwrap().work_upper_bound()
            > live_preflight(f.census).unwrap().work_upper_bound()
    );
    assert!(preflight(f.census, usize::MAX).is_err());
}
