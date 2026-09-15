use super::*;
use dialect_kernel::{
    IndexConstantOp, IndexType, OwnershipContractOp, OwnershipCoverageAttr, OwnershipPartitionAttr,
    RankedViewType,
};
use pliron::builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType};

fn fixture(
    context: &mut Context,
    bound: Option<u64>,
    header_write: bool,
    pure_header: bool,
) -> FuncOp {
    dialect_kernel::register_dialect(
        context,
        &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(context).unwrap();
    let index = IndexType::get(context).into();
    let ty = FunctionType::get(context, vec![index], vec![]);
    let function = FuncOp::new(context, "silent_loop".try_into().unwrap(), ty);
    let entry = function.get_entry_block(context);
    let blocks = [1, 1, 0].map(|args| {
        let block = BasicBlock::new(context, None, vec![index; args]);
        block.insert_at_back(function.get_region(context), context);
        block
    });
    let [header, body, exit] = blocks;
    let layout = ExecutionLayoutOp::new_with_domain(
        context,
        41,
        [1, 1, 1],
        [1, 1, 1],
        1,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    let ty = RankedViewType::new(context, 32, true, vec![1]).unwrap();
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        ty,
        vec![],
        MemorySpaceAttr::Global,
        17,
        17,
    )
    .unwrap();
    let ownership = OwnershipContractOp::new(
        context,
        view.result(context),
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    for operation in [
        layout.get_operation(),
        view.get_operation(),
        ownership.get_operation(),
        zero.get_operation(),
        one.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let bound = match bound {
        Some(bound) => {
            let value = IndexConstantOp::new(context, bound);
            value.get_operation().insert_at_back(entry, context);
            value.result(context)
        }
        None => entry.deref(context).get_argument(0),
    };
    BranchArgsOp::new(context, vec![zero.result(context)], header)
        .get_operation()
        .insert_at_back(entry, context);
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        view.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    write
        .get_operation()
        .insert_at_back(if header_write { header } else { exit }, context);
    if pure_header {
        IndexConstantOp::new(context, 9)
            .get_operation()
            .insert_at_back(header, context);
    }
    let induction = header.deref(context).get_argument(0);
    IndexLessThanBranchArgsOp::new(
        context,
        induction,
        bound,
        vec![induction],
        vec![],
        body,
        exit,
    )
    .get_operation()
    .insert_at_back(header, context);
    let body_induction = body.deref(context).get_argument(0);
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        body_induction,
        one.result(context),
    );
    next.get_operation().insert_at_back(body, context);
    BranchArgsOp::new(context, vec![next.result(context)], header)
        .get_operation()
        .insert_at_back(body, context);
    ReturnOp::new(context)
        .get_operation()
        .insert_at_back(exit, context);
    pliron::operation::verify_operation(function.get_operation(), context).unwrap();
    assert_eq!(
        crate::run_pliron_progress_check_v1(context, &function).status(),
        crate::KernelCheckStatusV1::Clean
    );
    function
}

fn traces(
    context: &Context,
    function: &FuncOp,
) -> Result<Vec<PlironInvocationTraceV1>, PlironTraceFailureV1> {
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, function).unwrap();
    let sparse = crate::analyze_pliron_sparse_indices_v1(context, function).unwrap();
    let layout = pliron_execution_layout_with_inventory_v1(context, &inventory)?;
    trace_pliron_invocations_with_inputs_v1(context, &inventory, &sparse, layout)
}

#[test]
fn unresolved_silent_loop_preserves_the_single_exit_write() {
    let mut context = Context::new();
    let function = fixture(&mut context, None, false, false);
    let traces = traces(&context, &function).unwrap();
    assert_eq!(traces.len(), 1);
    assert_eq!(traces[0].events.len(), 1);
    assert!(
        traces[0]
            .blocks
            .iter()
            .any(|visit| visit.block == 1 && visit.summarized)
    );
    assert!(
        crate::run_pliron_hierarchical_ownership_check_v1(&context, &function)
            .all_total_view_contracts_are_proved()
    );
}

#[test]
fn unresolved_header_work_is_never_summarized_as_silent() {
    for (header_write, pure_header) in [(true, false), (false, true)] {
        let mut context = Context::new();
        let function = fixture(&mut context, None, header_write, pure_header);
        assert!(matches!(
            traces(&context, &function).unwrap_err(),
            PlironTraceFailureV1::UnresolvedBranch { block: 1, .. }
        ));
        let report = crate::run_pliron_hierarchical_ownership_check_v1(&context, &function);
        assert!(!report.is_clean());
        assert!(!report.all_total_view_contracts_are_proved());
    }
}

#[test]
fn resolved_loop_retains_every_header_write_and_rejects_overwrite() {
    let mut context = Context::new();
    let function = fixture(&mut context, Some(2), true, false);
    let traces = traces(&context, &function).unwrap();
    assert_eq!(traces.len(), 1);
    assert_eq!(traces[0].events.len(), 3);
    assert!(traces[0].blocks.iter().all(|visit| !visit.summarized));
    let report = crate::run_pliron_hierarchical_ownership_check_v1(&context, &function);
    assert!(!report.is_clean());
    assert!(!report.all_total_view_contracts_are_proved());
}

#[test]
fn escaping_induction_is_not_replaced_with_its_initial_value_after_summary() {
    for bound in [None, Some(0), Some(2)] {
        let mut context = Context::new();
        let function = fixture(&mut context, bound, false, false);
        let blocks = function
            .get_region(&context)
            .deref(&context)
            .iter(&context)
            .collect::<Vec<_>>();
        let exit = blocks[3];
        let induction = blocks[1].deref(&context).get_argument(0);
        let one = blocks[0]
            .deref(&context)
            .iter(&context)
            .find_map(|op| {
                if !Operation::is_op::<IndexConstantOp>(op, &context) {
                    return None;
                }
                let constant = IndexConstantOp::from_operation(op);
                (constant.value(&context) == Some(1)).then(|| constant.result(&context))
            })
            .unwrap();
        let write_block = BasicBlock::new(&mut context, None, vec![]);
        write_block.insert_at_back(function.get_region(&context), &context);
        let done = BasicBlock::new(&mut context, None, vec![]);
        done.insert_at_back(function.get_region(&context), &context);
        let operations = exit.deref(&context).iter(&context).collect::<Vec<_>>();
        for operation in operations {
            operation.unlink(&context);
            operation.insert_at_back(write_block, &context);
        }
        IndexLessThanBranchOp::new(&mut context, induction, one, write_block, done)
            .get_operation()
            .insert_at_back(exit, &context);
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(done, &context);
        pliron::operation::verify_operation(function.get_operation(), &context).unwrap();
        assert!(crate::run_pliron_progress_check_v1(&context, &function).is_clean());
        let result = traces(&context, &function);
        if let Some(bound) = bound {
            let traces = result.unwrap();
            assert_eq!(traces[0].events.len(), usize::from(bound == 0));
            assert!(traces[0].blocks.iter().all(|visit| !visit.summarized));
        } else {
            assert!(matches!(
                result.unwrap_err(),
                PlironTraceFailureV1::UnresolvedBranch { block: 1, .. }
            ));
        }
        assert_eq!(
            crate::run_pliron_hierarchical_ownership_check_v1(&context, &function)
                .all_total_view_contracts_are_proved(),
            bound == Some(0)
        );
    }
}
