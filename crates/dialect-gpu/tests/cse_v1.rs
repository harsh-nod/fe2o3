use dialect_gpu::{
    cse_v1::LocalPureCsePassV1,
    optimization_v1::{
        BinaryKindAttr, BinaryOp, CastKindAttr, CastOp, ConstantOp, SelectOp,
        SelectSameValuePattern,
    },
};
use pliron::{
    attribute::AttrObj,
    builtin::{
        attributes::{FPSingleAttr, IntegerAttr},
        op_interfaces::{OneRegionInterface, SingleBlockRegionInterface},
        ops::ModuleOp,
        types::{IntegerType, Signedness},
    },
    context::Context,
    identifier::Identifier,
    irbuild::{IRStatus, match_rewrite::PassWrapper},
    linked_list::ContainsLinkedList,
    op::{Op, verify_op},
    pass::{AnalysisManager, Pass},
    r#type::Typed,
    utils::apint::{APInt, bw},
};

fn integer_attr(context: &Context, value: u32) -> AttrObj {
    Box::new(IntegerAttr::new(
        IntegerType::get(context, 32, Signedness::Unsigned),
        APInt::from_u32(value, bw(32)),
    ))
}

fn bool_attr(context: &Context, value: bool) -> AttrObj {
    Box::new(IntegerAttr::new(
        IntegerType::get(context, 1, Signedness::Signless),
        APInt::from_u8(u8::from(value), bw(1)),
    ))
}

fn module(context: &mut Context, name: &str) -> ModuleOp {
    ModuleOp::new(
        context,
        Identifier::try_from(name).expect("valid module name"),
    )
}

fn operation_count(module: &ModuleOp, context: &Context) -> usize {
    let block = module
        .get_region(context)
        .deref(context)
        .get_head()
        .expect("module entry block");
    block.deref(context).iter(context).count()
}

#[test]
fn exact_total_expression_is_reused_from_earlier_in_the_block() {
    let context = &mut Context::new();
    let module = module(context, "exact_cse");
    let condition = ConstantOp::new(context, bool_attr(context, true));
    let lhs = ConstantOp::new(context, integer_attr(context, 6));
    let rhs = ConstantOp::new(context, integer_attr(context, 3));
    let earlier = BinaryOp::new(
        context,
        BinaryKindAttr::BitAnd,
        lhs.result(context),
        rhs.result(context),
    );
    let duplicate = BinaryOp::new(
        context,
        BinaryKindAttr::BitAnd,
        lhs.result(context),
        rhs.result(context),
    );
    let duplicate_pointer = duplicate.get_operation();
    let consumer = SelectOp::new(
        context,
        condition.result(context),
        duplicate.result(context),
        lhs.result(context),
    );

    for operation in [
        condition.get_operation(),
        lhs.get_operation(),
        rhs.get_operation(),
        earlier.get_operation(),
        duplicate_pointer,
        consumer.get_operation(),
    ] {
        module.append_operation(context, operation, 0);
    }
    assert_eq!(operation_count(&module, context), 6);

    let report = LocalPureCsePassV1
        .run(
            module.get_operation(),
            context,
            &mut AnalysisManager::default(),
        )
        .expect("local CSE succeeds");

    assert_eq!(report.ir_changed, IRStatus::Changed);
    assert_eq!(operation_count(&module, context), 5);
    assert_eq!(
        consumer.get_operand_true_value(context),
        earlier.result(context)
    );
    assert!(duplicate_pointer.try_deref(context).is_err());
}

#[test]
fn trapping_or_semantically_different_operations_are_not_combined() {
    let context = &mut Context::new();
    let module = module(context, "conservative_cse");
    let lhs = ConstantOp::new(context, integer_attr(context, 6));
    let rhs = ConstantOp::new(context, integer_attr(context, 3));
    let add0 = BinaryOp::new(
        context,
        BinaryKindAttr::Add,
        lhs.result(context),
        rhs.result(context),
    );
    let add1 = BinaryOp::new(
        context,
        BinaryKindAttr::Add,
        lhs.result(context),
        rhs.result(context),
    );
    let bit_and = BinaryOp::new(
        context,
        BinaryKindAttr::BitAnd,
        lhs.result(context),
        rhs.result(context),
    );
    let bit_or = BinaryOp::new(
        context,
        BinaryKindAttr::BitOr,
        lhs.result(context),
        rhs.result(context),
    );
    let float_source = ConstantOp::new(context, Box::new(FPSingleAttr::from(6.0)));
    let float_to_integer0 = CastOp::new(
        context,
        CastKindAttr::FloatToInteger,
        float_source.result(context),
        lhs.result(context).get_type(context),
    );
    let float_to_integer1 = CastOp::new(
        context,
        CastKindAttr::FloatToInteger,
        float_source.result(context),
        lhs.result(context).get_type(context),
    );
    for operation in [
        lhs.get_operation(),
        rhs.get_operation(),
        add0.get_operation(),
        add1.get_operation(),
        bit_and.get_operation(),
        bit_or.get_operation(),
        float_source.get_operation(),
        float_to_integer0.get_operation(),
        float_to_integer1.get_operation(),
    ] {
        module.append_operation(context, operation, 0);
    }
    verify_op(&float_source, context).expect("valid float constant");
    verify_op(&float_to_integer0, context).expect("valid float-to-integer cast");
    verify_op(&float_to_integer1, context).expect("valid float-to-integer cast");

    let before = operation_count(&module, context);
    let report = LocalPureCsePassV1
        .run(
            module.get_operation(),
            context,
            &mut AnalysisManager::default(),
        )
        .expect("local CSE succeeds");

    assert_eq!(report.ir_changed, IRStatus::Unchanged);
    assert_eq!(operation_count(&module, context), before);
}

#[test]
fn candidates_are_never_reused_across_blocks() {
    let context = &mut Context::new();
    let root = module(context, "root");
    let first = module(context, "first");
    let second = module(context, "second");
    let first_constant = ConstantOp::new(context, integer_attr(context, 7));
    let second_constant = ConstantOp::new(context, integer_attr(context, 7));
    first.append_operation(context, first_constant.get_operation(), 0);
    second.append_operation(context, second_constant.get_operation(), 0);
    root.append_operation(context, first.get_operation(), 0);
    root.append_operation(context, second.get_operation(), 0);

    let report = LocalPureCsePassV1
        .run(
            root.get_operation(),
            context,
            &mut AnalysisManager::default(),
        )
        .expect("local CSE succeeds");

    assert_eq!(report.ir_changed, IRStatus::Unchanged);
    assert_eq!(operation_count(&first, context), 1);
    assert_eq!(operation_count(&second, context), 1);
}

#[test]
fn select_same_value_pattern_is_an_executable_canonicalization_pass() {
    let context = &mut Context::new();
    let module = module(context, "select_same");
    let condition = ConstantOp::new(context, bool_attr(context, true));
    let value = ConstantOp::new(context, integer_attr(context, 9));
    let select = SelectOp::new(
        context,
        condition.result(context),
        value.result(context),
        value.result(context),
    );
    let select_pointer = select.get_operation();
    for operation in [
        condition.get_operation(),
        value.get_operation(),
        select_pointer,
    ] {
        module.append_operation(context, operation, 0);
    }

    let report = PassWrapper::new("gpu-select-same-value-v1", SelectSameValuePattern)
        .run(
            module.get_operation(),
            context,
            &mut AnalysisManager::default(),
        )
        .expect("select canonicalization succeeds");

    assert_eq!(report.ir_changed, IRStatus::Changed);
    assert_eq!(operation_count(&module, context), 2);
    assert!(select_pointer.try_deref(context).is_err());
}
