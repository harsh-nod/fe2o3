use crate::{
    AddressSpaceAttr as Space,
    optimization_v1::{
        AccessModeAttr as Access, IndexType, PointerType, PreservedOperationKindAttr,
        PreservedOperationOp,
    },
    storage_operations_v18::*,
    storage_types_v18::*,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::types::{IntegerType, Signedness},
    common_traits::Verify,
    context::Context,
    op::Op,
    opts::dce::SideEffects,
    r#type::TypeHandle,
    value::Value,
};

fn context() -> Context {
    let mut context = Context::new();
    crate::register_dialect(&mut context).unwrap();
    context
}
fn key(byte: u8) -> StorageTableKeyAttrV18 {
    StorageTableKeyAttrV18::new([byte; 32], 18)
}
fn pointer(
    context: &Context,
    key: StorageTableKeyAttrV18,
    row: u32,
    space: Space,
    access: Access,
) -> TypeHandle {
    PointerType::get(
        context,
        StorageObjectTypeV18::get(context, key, StorageOrdinalAttrV18(row)).into(),
        space,
        access,
    )
    .into()
}
fn arguments(context: &mut Context, types: Vec<TypeHandle>) -> Vec<Value> {
    let block = BasicBlock::new(context, None, types);
    (0..block.deref(context).get_num_arguments())
        .map(|i| block.deref(context).get_argument(i))
        .collect()
}
fn descriptor(kind: StorageKindAttrV18) -> StorageDescriptorV18 {
    StorageDescriptorV18 {
        kind,
        selector: 0,
        overlap: StorageOverlapAttrV18::None,
        read: StorageAccessAttrV18::ABSENT,
        write: StorageAccessAttrV18::ABSENT,
    }
}
fn memory(space: Space) -> StorageAccessAttrV18 {
    StorageAccessAttrV18::new(space, 8, false)
}

#[test]
fn storage_v18_type_keys_join_content_and_row_not_bare_ordinal() {
    let context = context();
    let a = StorageObjectTypeV18::get(&context, key(1), StorageOrdinalAttrV18(0));
    let same = StorageObjectTypeV18::get(&context, key(1), StorageOrdinalAttrV18(0));
    let other_table = StorageObjectTypeV18::get(&context, key(2), StorageOrdinalAttrV18(0));
    let other_row = StorageObjectTypeV18::get(&context, key(1), StorageOrdinalAttrV18(1));
    assert_eq!(a, same);
    assert_ne!(a, other_table);
    assert_ne!(a, other_row);
    assert_eq!(key(7).digest(), [7; 32]);
    assert_eq!(key(7).encoded_length(), 18);
    a.deref(&context).verify(&context).unwrap();
    let malformed = StorageObjectTypeV18::get(
        &context,
        StorageTableKeyAttrV18::new([0; 32], 3),
        StorageOrdinalAttrV18(0),
    );
    assert!(malformed.deref(&context).verify(&context).is_err());
}

#[test]
fn storage_v18_all_typed_carriers_verify_and_are_conservatively_effectful() {
    use StorageKindAttrV18::*;
    let mut context = context();
    let rw = pointer(&context, key(1), 0, Space::Private, Access::ReadWrite);
    let wo = pointer(&context, key(1), 1, Space::Private, Access::WriteOnly);
    let index = IndexType::get(&context).into();
    let value = IntegerType::get(&context, 64, Signedness::Unsigned).into();
    let discriminant = IntegerType::get(&context, 128, Signedness::Unsigned).into();
    let values = arguments(&mut context, vec![rw, rw, index, value]);
    for kind in [
        ProjectField,
        ProjectArray,
        ProjectVariant,
        VariantForWrite,
        ReadValue,
        WriteValue,
        CopyObject,
        SetDiscriminant,
        ReadDiscriminant,
    ] {
        let mut d = descriptor(kind);
        let (operands, results) = match kind {
            ProjectField => (vec![values[0]], vec![rw]),
            ProjectArray => (vec![values[0], values[2]], vec![rw]),
            ProjectVariant => {
                d.read = memory(Space::Private);
                (vec![values[0]], vec![rw])
            }
            VariantForWrite => (vec![values[0]], vec![wo]),
            ReadValue => {
                d.read = memory(Space::Private);
                (vec![values[0]], vec![value])
            }
            ReadDiscriminant => {
                d.read = memory(Space::Private);
                (vec![values[0]], vec![discriminant])
            }
            WriteValue => {
                d.write = memory(Space::Private);
                (vec![values[0], values[3]], vec![])
            }
            CopyObject => {
                d.read = memory(Space::Private);
                d.write = memory(Space::Private);
                d.overlap = StorageOverlapAttrV18::MayOverlap;
                (vec![values[0], values[1]], vec![])
            }
            SetDiscriminant => {
                d.write = memory(Space::Private);
                (vec![values[0]], vec![])
            }
        };
        let operation = StorageOpV18::new(&mut context, d, operands, results);
        operation.verify(&context).unwrap();
        operation.verify_interfaces(&context).unwrap();
        assert_eq!(operation.descriptor(&context), Some(d));
        assert!(operation.has_side_effects(&context));
    }
}

#[test]
fn storage_v18_discriminant_carrier_requires_exact_unsigned_width_and_nonvolatile_read() {
    let mut context = context();
    let rw = pointer(&context, key(1), 0, Space::Private, Access::ReadWrite);
    let wo = pointer(&context, key(1), 0, Space::Private, Access::WriteOnly);
    let u128 = IntegerType::get(&context, 128, Signedness::Unsigned).into();
    let i128 = IntegerType::get(&context, 128, Signedness::Signed).into();
    let u64 = IntegerType::get(&context, 64, Signedness::Unsigned).into();
    let values = arguments(&mut context, vec![rw, wo]);
    for (operand, result, space, volatile, accepted) in [
        (0, u128, Space::Private, false, true),
        (1, u128, Space::Private, false, false),
        (0, i128, Space::Private, false, false),
        (0, u64, Space::Private, false, false),
        (0, u128, Space::Global, false, false),
        (0, u128, Space::Private, true, false),
    ] {
        let mut d = descriptor(StorageKindAttrV18::ReadDiscriminant);
        d.read = StorageAccessAttrV18::new(space, 8, volatile);
        let operation = StorageOpV18::new(&mut context, d, vec![values[operand]], vec![result]);
        assert_eq!(operation.verify(&context).is_ok(), accepted);
        assert!(operation.has_side_effects(&context));
    }
    for selector in [1, u32::MAX] {
        let mut d = descriptor(StorageKindAttrV18::ReadDiscriminant);
        d.read = memory(Space::Private);
        d.selector = selector;
        assert!(
            StorageOpV18::new(&mut context, d, vec![values[0]], vec![u128])
                .verify(&context)
                .is_err()
        );
    }
}

#[test]
fn storage_v18_refuses_cross_table_rights_space_and_noncanonical_descriptors() {
    let mut context = context();
    let a = pointer(&context, key(1), 0, Space::Private, Access::ReadWrite);
    let b = pointer(&context, key(2), 0, Space::Private, Access::ReadWrite);
    let ro = pointer(&context, key(1), 0, Space::Private, Access::ReadOnly);
    let constant = pointer(&context, key(1), 0, Space::Constant, Access::ReadWrite);
    let values = arguments(&mut context, vec![a, b, ro, constant]);
    for index in [1, 2, 3] {
        let mut d = descriptor(StorageKindAttrV18::CopyObject);
        d.read = memory(Space::Private);
        d.write = memory(if index == 3 {
            Space::Constant
        } else {
            Space::Private
        });
        d.overlap = StorageOverlapAttrV18::NonOverlapping;
        assert!(
            StorageOpV18::new(&mut context, d, vec![values[0], values[index]], vec![])
                .verify(&context)
                .is_err()
        );
    }
    for (input, result) in [(values[0], b), (values[2], a), (values[0], ro)] {
        let d = descriptor(StorageKindAttrV18::VariantForWrite);
        assert!(
            StorageOpV18::new(&mut context, d, vec![input], vec![result])
                .verify(&context)
                .is_err()
        );
    }
    let mut d = descriptor(StorageKindAttrV18::SetDiscriminant);
    d.write = StorageAccessAttrV18::new(Space::Private, 8, true);
    assert!(
        StorageOpV18::new(&mut context, d, vec![values[0]], vec![])
            .verify(&context)
            .is_err()
    );
    let mut d = descriptor(StorageKindAttrV18::ProjectField);
    d.read = memory(Space::Private);
    assert!(
        StorageOpV18::new(&mut context, d, vec![values[0]], vec![a])
            .verify(&context)
            .is_err()
    );
    assert!(
        StorageAccessAttrV18::new(Space::Private, 3, false)
            .verify(&context)
            .is_err()
    );
}

#[test]
fn storage_v18_execution_roles_validate_geometry_without_integer_erasure() {
    let context = context();
    for (role, lanes, elements, accepted) in [
        (1, 0, 0, true),
        (2, 0, 0, true),
        (3, 256, 125, true),
        (4, 1, 1, true),
        (0, 0, 0, false),
        (1, 1, 0, false),
        (3, 0, 1, false),
        (4, 257, 1, false),
    ] {
        let ty = ExecutionRoleTypeV18::get(
            &context,
            StorageOrdinalAttrV18(role),
            StorageOrdinalAttrV18(lanes),
            StorageOrdinalAttrV18(elements),
        );
        assert_eq!(ty.deref(&context).verify(&context).is_ok(), accepted);
    }
}

#[test]
fn storage_v18_preserved_execution_and_ordered_carriers_remain_effectful() {
    let mut context = context();
    for kind in [
        PreservedOperationKindAttr::ExecutionV18,
        PreservedOperationKindAttr::OrderedRegionV18,
        PreservedOperationKindAttr::OrderedProgramV18,
    ] {
        let operation = PreservedOperationOp::new(&mut context, kind, vec![], vec![]);
        assert!(operation.has_side_effects(&context));
        assert_eq!(operation.kind(&context), Some(kind));
    }
}
