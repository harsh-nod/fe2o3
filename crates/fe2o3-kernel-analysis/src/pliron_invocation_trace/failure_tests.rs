use super::*;
use dialect_kernel::{IndexConstantOp, IndexType, IndexUnknownOp};
use pliron::builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType};

#[derive(Clone, Copy)]
enum Branch {
    Less,
    Equal,
    LessArgs,
    EqualArgs,
    Edge,
}

enum OperandSource {
    Argument,
    Unknown,
    Quotient,
}

fn failure(branch: Branch, unknown_left: bool, source: OperandSource) -> String {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    let index = IndexType::get(&context).into();
    let ty = FunctionType::get(&context, vec![index], vec![]);
    let function = FuncOp::new(&mut context, "unresolved_branch".try_into().unwrap(), ty);
    let entry = function.get_entry_block(&context);
    let mut unknown = entry.deref(&context).get_argument(0);
    match source {
        OperandSource::Argument => {}
        OperandSource::Unknown => {
            let operation = IndexUnknownOp::new(&mut context);
            operation.get_operation().insert_at_back(entry, &context);
            unknown = operation.result(&context);
        }
        OperandSource::Quotient => {
            let divisor = IndexConstantOp::new(&mut context, 64);
            divisor.get_operation().insert_at_back(entry, &context);
            let divisor = divisor.result(&context);
            let operation =
                IndexBinaryOp::new(&mut context, IndexBinaryKindAttr::Divide, unknown, divisor);
            operation.get_operation().insert_at_back(entry, &context);
            unknown = operation.result(&context);
        }
    }
    let one = IndexConstantOp::new(&mut context, 1);
    one.get_operation().insert_at_back(entry, &context);
    let one = one.result(&context);
    let arguments = if matches!(branch, Branch::Edge) {
        vec![index]
    } else {
        vec![]
    };
    let exit = BasicBlock::new(&mut context, None, arguments);
    exit.insert_at_back(function.get_region(&context), &context);
    ReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(exit, &context);
    let (lhs, rhs) = if unknown_left {
        (unknown, one)
    } else {
        (one, unknown)
    };
    let terminator = match branch {
        Branch::Less => {
            IndexLessThanBranchOp::new(&mut context, lhs, rhs, exit, exit).get_operation()
        }
        Branch::Equal => {
            IndexEqualBranchOp::new(&mut context, lhs, rhs, exit, exit).get_operation()
        }
        Branch::LessArgs => {
            IndexLessThanBranchArgsOp::new(&mut context, lhs, rhs, vec![], vec![], exit, exit)
                .get_operation()
        }
        Branch::EqualArgs => {
            IndexEqualBranchArgsOp::new(&mut context, lhs, rhs, vec![], vec![], exit, exit)
                .get_operation()
        }
        Branch::Edge => BranchArgsOp::new(&mut context, vec![unknown], exit).get_operation(),
    };
    terminator.insert_at_back(entry, &context);
    pliron::operation::verify_operation(function.get_operation(), &context).unwrap();
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    let sparse = crate::analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
    let failure =
        trace_pliron_invocations_with_inputs_v1(&context, &inventory, &sparse, None).unwrap_err();
    crate::pliron_barrier::trace_failure_detail(failure)
}

#[test]
fn unresolved_comparisons_identify_the_operand_and_live_block_argument() {
    for branch in [
        Branch::Less,
        Branch::Equal,
        Branch::LessArgs,
        Branch::EqualArgs,
    ] {
        for (unknown_left, role) in [(true, "left"), (false, "right")] {
            assert_eq!(
                failure(branch, unknown_left, OperandSource::Argument),
                format!(
                    "branch in block 0 cannot be traced: {role} comparison operand is unresolved: block 0 argument 0"
                )
            );
        }
    }
}

#[test]
fn unresolved_computed_operand_identifies_the_live_definition() {
    assert_eq!(
        failure(Branch::Equal, true, OperandSource::Unknown),
        "branch in block 0 cannot be traced: left comparison operand is unresolved: block 0 op 0 result 0 (kernel.index_unknown)"
    );
}

#[test]
fn unresolved_edge_value_is_not_reported_as_a_comparison() {
    assert_eq!(
        failure(Branch::Edge, true, OperandSource::Argument),
        "branch in block 0 cannot be traced: successor argument 0 is unresolved: block 0 argument 0"
    );
}

#[test]
fn unresolved_quotient_reports_its_kind_and_direct_operand_definitions() {
    assert_eq!(
        failure(Branch::Equal, true, OperandSource::Quotient),
        "branch in block 0 cannot be traced: left comparison operand is unresolved: block 0 op 1 result 0 (kernel.index_binary Some(Divide)); operand 0: block 0 argument 0; operand 1: block 0 op 0 result 0 (kernel.index_constant Some(64))"
    );
}
