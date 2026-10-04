use dialect_gpu::{
    AddressSpaceAttr,
    optimization_v1::{AccessModeAttr, CastKindAttr, CastOp, SliceType},
    register_dialect,
};
use pliron::{
    attribute::AttrObj,
    basic_block::BasicBlock,
    builtin::types::{IntegerType, Signedness},
    combine::{Parser, eof},
    context::Context,
    op::{op_cast, verify_op},
    opts::{constants::ConstFoldInterface, dce::SideEffects},
    parsable::{Parsable, parse_from_str},
    printable::Printable,
    r#type::TypeHandle,
};

fn cast(ctx: &mut Context, from: TypeHandle, to: TypeHandle) -> CastOp {
    let source = BasicBlock::new(ctx, None, vec![from]);
    let value = source.deref(ctx).get_argument(0);
    CastOp::new(ctx, CastKindAttr::SliceToGeneric, value, to)
}

#[test]
fn registered_slice_exposure_attribute_roundtrips() {
    let mut ctx = Context::new();
    register_dialect(&mut ctx).unwrap();
    let attr: AttrObj = Box::new(CastKindAttr::SliceToGeneric);
    let text = attr.disp(&ctx).to_string();
    let parsed = parse_from_str(AttrObj::parser(()).skip(eof()), &mut ctx, &text).unwrap();
    assert_eq!(parsed.disp(&ctx).to_string(), text);
    assert_eq!(
        parsed.downcast_ref::<CastKindAttr>(),
        Some(&CastKindAttr::SliceToGeneric)
    );
}

#[test]
fn exact_slice_exposure_is_pure_but_never_numeric_folded() {
    let mut ctx = Context::new();
    let scalar: TypeHandle = IntegerType::get(&ctx, 32, Signedness::Unsigned).into();
    for space in [
        AddressSpaceAttr::Global,
        AddressSpaceAttr::Constant,
        AddressSpaceAttr::Private,
        AddressSpaceAttr::Workgroup,
    ] {
        for rights in [
            AccessModeAttr::ReadOnly,
            AccessModeAttr::ReadWrite,
            AccessModeAttr::WriteOnly,
        ] {
            let from = SliceType::get(&ctx, scalar, space, rights).into();
            let to = SliceType::get(&ctx, scalar, AddressSpaceAttr::Generic, rights).into();
            let operation = cast(&mut ctx, from, to);
            assert_eq!(
                verify_op(&operation, &ctx).is_ok(),
                space != AddressSpaceAttr::Constant || rights == AccessModeAttr::ReadOnly
            );
            assert!(
                !op_cast::<dyn SideEffects>(&operation)
                    .unwrap()
                    .has_side_effects(&ctx)
            );
            let folded = op_cast::<dyn ConstFoldInterface>(&operation)
                .unwrap()
                .check_fold(&ctx, &[None]);
            assert_eq!(folded.len(), 1);
            assert!(folded[0].is_none());
        }
    }
}

#[test]
fn slice_exposure_rejects_direction_permission_and_same_width_type_mutations() {
    let mut ctx = Context::new();
    let word: TypeHandle = IntegerType::get(&ctx, 32, Signedness::Unsigned).into();
    let signed: TypeHandle = IntegerType::get(&ctx, 32, Signedness::Signed).into();
    let private: TypeHandle = SliceType::get(
        &ctx,
        word,
        AddressSpaceAttr::Private,
        AccessModeAttr::ReadWrite,
    )
    .into();
    let generic: TypeHandle = SliceType::get(
        &ctx,
        word,
        AddressSpaceAttr::Generic,
        AccessModeAttr::ReadWrite,
    )
    .into();
    let read_only = SliceType::get(
        &ctx,
        word,
        AddressSpaceAttr::Generic,
        AccessModeAttr::ReadOnly,
    )
    .into();
    let wrong_word = SliceType::get(
        &ctx,
        signed,
        AddressSpaceAttr::Generic,
        AccessModeAttr::ReadWrite,
    )
    .into();
    let private_read_only = SliceType::get(
        &ctx,
        word,
        AddressSpaceAttr::Private,
        AccessModeAttr::ReadOnly,
    )
    .into();
    let private_write_only = SliceType::get(
        &ctx,
        word,
        AddressSpaceAttr::Private,
        AccessModeAttr::WriteOnly,
    )
    .into();
    let write_only = SliceType::get(
        &ctx,
        word,
        AddressSpaceAttr::Generic,
        AccessModeAttr::WriteOnly,
    )
    .into();
    assert!(verify_op(&cast(&mut ctx, private, generic), &ctx).is_ok());
    for (from, to) in [
        (generic, private),
        (generic, generic),
        (private, private),
        (private, read_only),
        (private, wrong_word),
        (private_write_only, generic),
        (private_write_only, read_only),
        (private_read_only, generic),
        (private_read_only, write_only),
        (word, generic),
        (private, word),
    ] {
        let operation = cast(&mut ctx, from, to);
        assert!(verify_op(&operation, &ctx).is_err());
    }
}
