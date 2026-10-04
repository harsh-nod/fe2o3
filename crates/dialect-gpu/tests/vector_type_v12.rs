use dialect_gpu::{
    AddressSpaceAttr,
    optimization_v1::{AccessModeAttr, BFloat16Type, PointerType, SliceType},
    register_dialect,
    vector_v12::{FixedVectorTypeV12, VectorLaneCountAttrV12, VectorLayoutAttrV12},
};
use pliron::{
    attribute::AttrObj,
    builtin::types::{FP16Type, FP32Type, FP64Type, IntegerType, Signedness},
    combine::{Parser, eof},
    common_traits::Verify,
    context::Context,
    parsable::{Parsable, parse_from_str},
    printable::Printable,
    r#type::TypeHandle,
};

fn context() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context).expect("gpu dialect registers");
    context
}

fn vector(
    context: &Context,
    element: TypeHandle,
    lanes: u16,
    layout: VectorLayoutAttrV12,
) -> TypeHandle {
    FixedVectorTypeV12::try_get(context, element, lanes, layout)
        .expect("legal vector descriptor")
        .into()
}

#[test]
fn every_numeric_element_and_legal_vector_layout_is_admitted() {
    let context = context();
    let mut elements = Vec::<TypeHandle>::new();
    for width in [8, 16, 32, 64, 128] {
        for signedness in [Signedness::Signed, Signedness::Unsigned] {
            elements.push(IntegerType::get(&context, width, signedness).into());
        }
    }
    elements.extend::<[TypeHandle; 4]>([
        FP16Type::get(&context).into(),
        BFloat16Type::get(&context).into(),
        FP32Type::get(&context).into(),
        FP64Type::get(&context).into(),
    ]);
    for element in elements {
        for (lanes, layout) in [
            (2, VectorLayoutAttrV12::CONTIGUOUS),
            (4, VectorLayoutAttrV12::interleaved(2)),
            (1024, VectorLayoutAttrV12::interleaved(32)),
        ] {
            let ty = vector(&context, element, lanes, layout);
            ty.deref(&context).verify(&context).unwrap();
        }
    }
}

#[test]
fn vector_descriptor_rejects_invalid_lanes_elements_and_interleaves() {
    let context = context();
    let f32_ty = FP32Type::get(&context).into();
    let bool_ty = IntegerType::get(&context, 1, Signedness::Signless).into();
    for lanes in [0, 1, 1025] {
        assert!(
            FixedVectorTypeV12::try_get(&context, f32_ty, lanes, VectorLayoutAttrV12::CONTIGUOUS)
                .is_none()
        );
    }
    for factor in [1, 3, 4, 8] {
        assert!(
            FixedVectorTypeV12::try_get(
                &context,
                f32_ty,
                4,
                VectorLayoutAttrV12::interleaved(factor),
            )
            .is_none()
        );
    }
    assert!(
        FixedVectorTypeV12::try_get(&context, bool_ty, 4, VectorLayoutAttrV12::CONTIGUOUS)
            .is_none()
    );
    assert!(VectorLaneCountAttrV12(0).verify(&context).is_err());
    assert!(VectorLayoutAttrV12(1).verify(&context).is_err());
    let malformed = FixedVectorTypeV12::get(
        &context,
        f32_ty,
        VectorLaneCountAttrV12(4),
        VectorLayoutAttrV12::interleaved(3),
    );
    assert!(malformed.verify(&context).is_err());
}

#[test]
fn nested_pointer_and_slice_vector_types_remain_first_class() {
    let context = context();
    let f32_ty = FP32Type::get(&context).into();
    let vector = vector(&context, f32_ty, 8, VectorLayoutAttrV12::interleaved(2));
    let pointer = PointerType::get(
        &context,
        vector,
        AddressSpaceAttr::Private,
        AccessModeAttr::ReadWrite,
    );
    pointer.verify(&context).unwrap();
    let slice = SliceType::get(
        &context,
        vector,
        AddressSpaceAttr::Global,
        AccessModeAttr::ReadOnly,
    );
    slice.verify(&context).unwrap();
}

#[test]
fn registered_vector_attributes_and_type_round_trip_exactly() {
    let mut context = context();
    let attributes: [AttrObj; 2] = [
        Box::new(VectorLaneCountAttrV12(8)),
        Box::new(VectorLayoutAttrV12::interleaved(2)),
    ];
    for attribute in attributes {
        let text = attribute.disp(&context).to_string();
        let parsed = parse_from_str(AttrObj::parser(()).skip(eof()), &mut context, &text)
            .expect("registered vector attribute parses");
        assert_eq!(parsed.disp(&context).to_string(), text);
        parsed.verify(&context).unwrap();
    }
    let element = FP32Type::get(&context).into();
    let vector = vector(&context, element, 8, VectorLayoutAttrV12::interleaved(2));
    let text = vector.disp(&context).to_string();
    let parsed = parse_from_str(TypeHandle::parser(()).skip(eof()), &mut context, &text)
        .expect("registered vector type parses");
    assert_eq!(parsed, vector);
    parsed.deref(&context).verify(&context).unwrap();
    let raw = parsed.deref(&context);
    let descriptor = raw.downcast_ref::<FixedVectorTypeV12>().unwrap();
    assert_eq!(descriptor.element(), element);
    assert_eq!(descriptor.lanes(), 8);
    assert_eq!(descriptor.layout(), VectorLayoutAttrV12::interleaved(2));
}
