use dialect_gpu::{
    AddressSpaceAttr,
    optimization_v1::{
        AccessModeAttr, BFloat16Attr, BFloat16Type, BinaryKindAttr, BinaryOp, BranchOp, CallOp,
        CastKindAttr, CastOp, CondBranchOp, ConstantOp, IndexAttr, IndexType, LoadOp, PointerType,
        ReturnOp, SelectOp, SliceDataOp, SliceLengthOp, SliceType, StoreOp,
    },
};
use pliron::{
    attribute::AttrObj,
    basic_block::BasicBlock,
    builtin::{
        attributes::{IntegerAttr, TypeAttr},
        op_interfaces::BranchOpInterface,
        ops::FuncOp,
        types::{FP16Type, FP32Type, FP64Type, FunctionType, IntegerType, Signedness},
    },
    context::Context,
    identifier::Identifier,
    op::{Op, op_cast, verify_op},
    opts::{
        constants::{BranchOpFoldInterface, ConstFoldInterface},
        dce::SideEffects,
    },
    r#type::{TypeHandle, Typed},
    utils::apint::{APInt, bw},
};

fn integer_attr(ctx: &Context, value: u32) -> AttrObj {
    Box::new(IntegerAttr::new(
        IntegerType::get(ctx, 32, Signedness::Unsigned),
        APInt::from_u32(value, bw(32)),
    ))
}

fn bool_attr(ctx: &Context, value: bool) -> AttrObj {
    Box::new(IntegerAttr::new(
        IntegerType::get(ctx, 1, Signedness::Signless),
        APInt::from_u8(u8::from(value), bw(1)),
    ))
}

fn assert_cast_legality(
    ctx: &mut Context,
    name: &str,
    kind: CastKindAttr,
    from: TypeHandle,
    to: TypeHandle,
    expected: bool,
) {
    let source = BasicBlock::new(ctx, None, vec![from]);
    let value = source.deref(ctx).get_argument(0);
    let cast = CastOp::new(ctx, kind, value, to);
    assert_eq!(
        verify_op(&cast, ctx).is_ok(),
        expected,
        "unexpected verifier result for {name}"
    );
}

#[test]
fn constants_and_integer_binary_ops_fold_without_erasing_traps() {
    let ctx = &mut Context::new();
    let lhs_attr = integer_attr(ctx, 40);
    let rhs_attr = integer_attr(ctx, 2);
    let lhs = ConstantOp::new(ctx, lhs_attr.clone());
    let rhs = ConstantOp::new(ctx, rhs_attr.clone());

    verify_op(&lhs, ctx).expect("typed gpu constant");
    let constant_fold = op_cast::<dyn ConstFoldInterface>(&lhs).expect("constant fold interface");
    assert_eq!(constant_fold.check_fold(ctx, &[]).len(), 1);

    let add = BinaryOp::new(ctx, BinaryKindAttr::Add, lhs.result(ctx), rhs.result(ctx));
    verify_op(&add, ctx).expect("typed binary operation");
    let add_effects = op_cast::<dyn SideEffects>(&add).expect("binary side effects interface");
    assert!(add_effects.has_side_effects(ctx));
    let add_fold = op_cast::<dyn ConstFoldInterface>(&add).expect("binary fold interface");
    let folded = add_fold.check_fold(ctx, &[Some(lhs_attr), Some(rhs_attr)]);
    let value = folded[0]
        .as_ref()
        .and_then(|attr| attr.downcast_ref::<IntegerAttr>())
        .expect("folded integer")
        .value();
    assert_eq!(value.to_u64(), 42);

    let bitand = BinaryOp::new(
        ctx,
        BinaryKindAttr::BitAnd,
        lhs.result(ctx),
        rhs.result(ctx),
    );
    let bitand_effects =
        op_cast::<dyn SideEffects>(&bitand).expect("binary side effects interface");
    assert!(!bitand_effects.has_side_effects(ctx));

    let divide = BinaryOp::new(
        ctx,
        BinaryKindAttr::Divide,
        lhs.result(ctx),
        rhs.result(ctx),
    );
    let divide_effects =
        op_cast::<dyn SideEffects>(&divide).expect("binary side effects interface");
    assert!(divide_effects.has_side_effects(ctx));
    let divide_fold = op_cast::<dyn ConstFoldInterface>(&divide).expect("binary fold interface");
    assert!(
        divide_fold.check_fold(
            ctx,
            &[Some(integer_attr(ctx, 40)), Some(integer_attr(ctx, 0))]
        )[0]
        .is_none()
    );
    assert!(
        add_fold.check_fold(
            ctx,
            &[
                Some(integer_attr(ctx, u32::MAX)),
                Some(integer_attr(ctx, 1))
            ],
        )[0]
        .is_none(),
        "folding must not erase an unsigned-overflow trap"
    );
}

#[test]
fn exact_index_and_bfloat_constants_retain_their_distinct_types() {
    let ctx = &mut Context::new();
    let index = ConstantOp::new(ctx, Box::new(IndexAttr(u64::MAX)));
    let bf16 = ConstantOp::new(ctx, Box::new(BFloat16Attr(0x7fc1)));

    verify_op(&index, ctx).expect("index constant");
    verify_op(&bf16, ctx).expect("bf16 constant");
    assert_ne!(
        index.result(ctx).get_type(ctx),
        bf16.result(ctx).get_type(ctx)
    );
    assert_eq!(
        index
            .value(ctx)
            .downcast_ref::<IndexAttr>()
            .expect("index attribute")
            .0,
        u64::MAX
    );
    assert_eq!(
        bf16.value(ctx)
            .downcast_ref::<BFloat16Attr>()
            .expect("bf16 attribute")
            .0,
        0x7fc1
    );
}

#[test]
fn cast_verifier_matches_exact_kernel_ir_scalar_legality() {
    let ctx = &mut Context::new();
    let bool_type = IntegerType::get(ctx, 1, Signedness::Signless).into();
    let i8_type = IntegerType::get(ctx, 8, Signedness::Signed).into();
    let i16_type = IntegerType::get(ctx, 16, Signedness::Signed).into();
    let i32_type = IntegerType::get(ctx, 32, Signedness::Signed).into();
    let u8_type = IntegerType::get(ctx, 8, Signedness::Unsigned).into();
    let u16_type = IntegerType::get(ctx, 16, Signedness::Unsigned).into();
    let u32_type = IntegerType::get(ctx, 32, Signedness::Unsigned).into();
    let u64_type = IntegerType::get(ctx, 64, Signedness::Unsigned).into();
    let u128_type = IntegerType::get(ctx, 128, Signedness::Unsigned).into();
    let signless_i32_type = IntegerType::get(ctx, 32, Signedness::Signless).into();
    let index_type = IndexType::get(ctx).into();
    let f16_type = FP16Type::get(ctx).into();
    let bf16_type = BFloat16Type::get(ctx).into();
    let f32_type = FP32Type::get(ctx).into();
    let f64_type = FP64Type::get(ctx).into();

    for (name, kind, from, to) in [
        (
            "integer truncate",
            CastKindAttr::Truncate,
            i16_type,
            u8_type,
        ),
        (
            "bool zero extend",
            CastKindAttr::ZeroExtend,
            bool_type,
            i8_type,
        ),
        (
            "unsigned zero extend",
            CastKindAttr::ZeroExtend,
            u8_type,
            i16_type,
        ),
        (
            "signed sign extend",
            CastKindAttr::SignExtend,
            i8_type,
            u16_type,
        ),
        (
            "float extend",
            CastKindAttr::FloatExtend,
            bf16_type,
            f32_type,
        ),
        (
            "float truncate",
            CastKindAttr::FloatTruncate,
            f64_type,
            f16_type,
        ),
        (
            "integer to float",
            CastKindAttr::IntegerToFloat,
            u128_type,
            f16_type,
        ),
        (
            "float to integer",
            CastKindAttr::FloatToInteger,
            f32_type,
            i32_type,
        ),
        ("integer bitcast", CastKindAttr::Bitcast, i32_type, u32_type),
        ("float bitcast", CastKindAttr::Bitcast, f16_type, bf16_type),
        (
            "cross-category bitcast",
            CastKindAttr::Bitcast,
            f32_type,
            u32_type,
        ),
        (
            "u32 to index",
            CastKindAttr::ZeroExtend,
            u32_type,
            index_type,
        ),
        ("u64 to index", CastKindAttr::Bitcast, u64_type, index_type),
        ("index to u64", CastKindAttr::Bitcast, index_type, u64_type),
    ] {
        assert_cast_legality(ctx, name, kind, from, to, true);
    }

    for (name, kind, from, to) in [
        ("truncate widens", CastKindAttr::Truncate, i8_type, i16_type),
        (
            "truncate to bool",
            CastKindAttr::Truncate,
            u8_type,
            bool_type,
        ),
        (
            "zero extend signed",
            CastKindAttr::ZeroExtend,
            i8_type,
            u16_type,
        ),
        (
            "zero extend narrows",
            CastKindAttr::ZeroExtend,
            u16_type,
            u8_type,
        ),
        (
            "sign extend unsigned",
            CastKindAttr::SignExtend,
            u8_type,
            i16_type,
        ),
        (
            "sign extend narrows",
            CastKindAttr::SignExtend,
            i16_type,
            i8_type,
        ),
        (
            "float extend narrows",
            CastKindAttr::FloatExtend,
            f32_type,
            f16_type,
        ),
        (
            "float extend equal width",
            CastKindAttr::FloatExtend,
            f16_type,
            bf16_type,
        ),
        (
            "float truncate widens",
            CastKindAttr::FloatTruncate,
            f16_type,
            f64_type,
        ),
        (
            "float truncate equal width",
            CastKindAttr::FloatTruncate,
            bf16_type,
            f16_type,
        ),
        (
            "bool to float",
            CastKindAttr::IntegerToFloat,
            bool_type,
            f32_type,
        ),
        (
            "index to float",
            CastKindAttr::IntegerToFloat,
            index_type,
            f64_type,
        ),
        (
            "float to bool",
            CastKindAttr::FloatToInteger,
            f32_type,
            bool_type,
        ),
        (
            "float to index",
            CastKindAttr::FloatToInteger,
            f64_type,
            index_type,
        ),
        (
            "same type bitcast",
            CastKindAttr::Bitcast,
            i32_type,
            i32_type,
        ),
        (
            "unequal width bitcast",
            CastKindAttr::Bitcast,
            i32_type,
            f64_type,
        ),
        ("bool bitcast", CastKindAttr::Bitcast, bool_type, u8_type),
        (
            "u16 to index direct",
            CastKindAttr::ZeroExtend,
            u16_type,
            index_type,
        ),
        (
            "index to u32 direct",
            CastKindAttr::Truncate,
            index_type,
            u32_type,
        ),
        (
            "u32 index wrong kind",
            CastKindAttr::Bitcast,
            u32_type,
            index_type,
        ),
        (
            "signless integer",
            CastKindAttr::Bitcast,
            signless_i32_type,
            u32_type,
        ),
    ] {
        assert_cast_legality(ctx, name, kind, from, to, false);
    }
}

#[test]
fn pointer_access_is_typed_and_memory_ops_remain_effectful() {
    let ctx = &mut Context::new();
    let u32_ty = IntegerType::get(ctx, 32, Signedness::Unsigned).into();
    let read_write = PointerType::get(
        ctx,
        u32_ty,
        AddressSpaceAttr::Global,
        AccessModeAttr::ReadWrite,
    )
    .into();
    let pointer_block = BasicBlock::new(ctx, None, vec![read_write]);
    let pointer = pointer_block.deref(ctx).get_argument(0);
    let value = ConstantOp::new(ctx, integer_attr(ctx, 7));

    let load = LoadOp::new(ctx, pointer, 4, true).expect("readable pointer");
    let store = StoreOp::new(ctx, pointer, value.result(ctx), 4, false).expect("writable pointer");
    verify_op(&load, ctx).expect("load types");
    verify_op(&store, ctx).expect("store types");
    assert_eq!(load.address_space(ctx), Some(AddressSpaceAttr::Global));
    assert_eq!(load.alignment(ctx), Some(4));
    assert_eq!(load.is_volatile(ctx), Some(true));
    assert_eq!(store.alignment(ctx), Some(4));
    assert_eq!(store.is_volatile(ctx), Some(false));
    assert!(op_cast::<dyn SideEffects>(&load).is_none());
    assert!(op_cast::<dyn SideEffects>(&store).is_none());

    let write_only = PointerType::get(
        ctx,
        u32_ty,
        AddressSpaceAttr::Global,
        AccessModeAttr::WriteOnly,
    )
    .into();
    let write_only_block = BasicBlock::new(ctx, None, vec![write_only]);
    let write_only_pointer = write_only_block.deref(ctx).get_argument(0);
    assert!(LoadOp::new(ctx, write_only_pointer, 4, false).is_none());
    assert!(LoadOp::new(ctx, pointer, 3, false).is_none());
}

#[test]
fn branch_interfaces_preserve_successor_arguments_and_fold_conditions() {
    let ctx = &mut Context::new();
    let u32_ty = IntegerType::get(ctx, 32, Signedness::Unsigned).into();
    let source = BasicBlock::new(ctx, None, vec![u32_ty]);
    let then_block = BasicBlock::new(ctx, None, vec![u32_ty]);
    let else_block = BasicBlock::new(ctx, None, vec![u32_ty]);
    let argument = source.deref(ctx).get_argument(0);

    let branch = BranchOp::new(ctx, then_block, vec![argument]);
    let branch_iface =
        op_cast::<dyn BranchOpInterface>(&branch).expect("branch operation interface");
    assert_eq!(branch_iface.successor_operands(ctx, 0), vec![argument]);

    let condition = ConstantOp::new(ctx, bool_attr(ctx, false));
    let conditional = CondBranchOp::new(
        ctx,
        condition.result(ctx),
        then_block,
        vec![argument],
        else_block,
        vec![argument],
    );
    verify_op(&conditional, ctx).expect("conditional branch types");
    let fold = op_cast::<dyn BranchOpFoldInterface>(&conditional)
        .expect("conditional branch fold interface");
    assert_eq!(
        fold.check_fold(ctx, &[Some(bool_attr(ctx, false)), None, None]),
        vec![else_block]
    );
}

#[test]
fn select_folds_only_with_a_known_condition() {
    let ctx = &mut Context::new();
    let condition = ConstantOp::new(ctx, bool_attr(ctx, true));
    let true_value = ConstantOp::new(ctx, integer_attr(ctx, 11));
    let false_value = ConstantOp::new(ctx, integer_attr(ctx, 22));
    let select = SelectOp::new(
        ctx,
        condition.result(ctx),
        true_value.result(ctx),
        false_value.result(ctx),
    );
    verify_op(&select, ctx).expect("select types");
    let fold = op_cast::<dyn ConstFoldInterface>(&select).expect("select fold interface");
    assert!(fold.check_fold(ctx, &[None, None, None])[0].is_none());
    let attrs = [
        Some(bool_attr(ctx, true)),
        Some(integer_attr(ctx, 11)),
        Some(integer_attr(ctx, 22)),
    ];
    let result = fold.check_fold(ctx, &attrs);
    assert_eq!(
        result[0]
            .as_ref()
            .and_then(|attr| attr.downcast_ref::<IntegerAttr>())
            .expect("selected constant")
            .value()
            .to_u64(),
        11
    );
}

#[test]
fn slice_projection_results_have_exact_descriptor_types() {
    let ctx = &mut Context::new();
    let u32_type = IntegerType::get(ctx, 32, Signedness::Unsigned).into();
    let slice_type = SliceType::get(
        ctx,
        u32_type,
        AddressSpaceAttr::Global,
        AccessModeAttr::ReadWrite,
    )
    .into();
    let source = BasicBlock::new(ctx, None, vec![slice_type]);
    let slice = source.deref(ctx).get_argument(0);

    let length = SliceLengthOp::new(ctx, slice);
    verify_op(&length, ctx).expect("slice length has index result");
    let length_result = length.get_operation().deref(ctx).get_result(0);
    length_result.set_type(ctx, u32_type);
    assert!(verify_op(&length, ctx).is_err());

    let data = SliceDataOp::new(ctx, slice).expect("valid slice data projection");
    verify_op(&data, ctx).expect("slice data has matching pointer result");
    let wrong_pointer = PointerType::get(
        ctx,
        u32_type,
        AddressSpaceAttr::Global,
        AccessModeAttr::ReadOnly,
    )
    .into();
    let data_result = data.get_operation().deref(ctx).get_result(0);
    data_result.set_type(ctx, wrong_pointer);
    assert!(verify_op(&data, ctx).is_err());
}

#[test]
fn calls_carry_and_enforce_an_explicit_function_type_contract() {
    let ctx = &mut Context::new();
    let u32_type = IntegerType::get(ctx, 32, Signedness::Unsigned).into();
    let u64_type = IntegerType::get(ctx, 64, Signedness::Unsigned).into();
    let source = ConstantOp::new(ctx, integer_attr(ctx, 7));

    let call = CallOp::new(
        ctx,
        "unresolved::callee",
        vec![source.result(ctx)],
        vec![u32_type],
    );
    verify_op(&call, ctx).expect("symbol resolution is deferred but signature is complete");
    assert!(call.signature(ctx).is_some());

    let call_result = call.get_operation().deref(ctx).get_result(0);
    call_result.set_type(ctx, u64_type);
    assert!(verify_op(&call, ctx).is_err());

    let malformed = CallOp::new(ctx, "callee", vec![source.result(ctx)], vec![u32_type]);
    malformed.set_attr_gpu_call_signature(ctx, TypeAttr::new(u32_type));
    assert!(verify_op(&malformed, ctx).is_err());
}

#[test]
fn returns_must_match_the_immediately_enclosing_function() {
    let ctx = &mut Context::new();
    let u32_type = IntegerType::get(ctx, 32, Signedness::Unsigned).into();
    let function_type = FunctionType::get(ctx, vec![], vec![u32_type]);
    let function = FuncOp::new(
        ctx,
        Identifier::try_from("returns_u32").unwrap(),
        function_type,
    );
    let value = ConstantOp::new(ctx, integer_attr(ctx, 9));
    let return_op = ReturnOp::new(ctx, vec![value.result(ctx)]);
    let entry = function.get_entry_block(ctx);
    value.get_operation().insert_at_back(entry, ctx);
    return_op.get_operation().insert_at_back(entry, ctx);
    verify_op(&function, ctx).expect("matching return type");

    let detached = ReturnOp::new(ctx, vec![]);
    assert!(verify_op(&detached, ctx).is_err());

    let wrong_type = FunctionType::get(ctx, vec![], vec![u32_type]);
    let wrong_function = FuncOp::new(
        ctx,
        Identifier::try_from("wrong_return").unwrap(),
        wrong_type,
    );
    let wrong_value = ConstantOp::new(ctx, bool_attr(ctx, true));
    let wrong_return = ReturnOp::new(ctx, vec![wrong_value.result(ctx)]);
    let wrong_entry = wrong_function.get_entry_block(ctx);
    wrong_value.get_operation().insert_at_back(wrong_entry, ctx);
    wrong_return
        .get_operation()
        .insert_at_back(wrong_entry, ctx);
    assert!(verify_op(&wrong_function, ctx).is_err());
}
