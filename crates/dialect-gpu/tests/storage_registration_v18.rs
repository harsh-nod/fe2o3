use dialect_gpu::{
    AddressSpaceAttr, DIALECT_NAME, TargetNeutralGpuOpInterface,
    optimization_v1::{
        AccessModeAttr, BFloat16Type, CastKindAttr, IndexType, PointerType,
        PreservedOperationKindAttr, PreservedOperationOp, ReturnOp, SliceType,
    },
    storage_operations_v18::{
        StorageAccessAttrV18, StorageDescriptorV18, StorageKindAttrV18, StorageOpV18,
        StorageOverlapAttrV18,
    },
    storage_types_v18::{
        ExecutionRoleTypeV18, StorageObjectTypeV18, StorageOrdinalAttrV18, StorageTableKeyAttrV18,
    },
};
use pliron::{
    attribute::AttrObj,
    builtin::{ops::FuncOp, types::FunctionType},
    combine::{Parser, eof},
    context::Context,
    dialect::DialectName,
    linked_list::ContainsLinkedList,
    op::{Op, op_cast, verify_op},
    operation::{Operation, verify_operation},
    opts::dce::SideEffects,
    parsable::{Parsable, parse_from_str},
    printable::Printable,
    r#type::{TypeHandle, Typed},
};

fn context(owner_hook: bool) -> Context {
    let mut context = Context::new();
    if owner_hook {
        dialect_gpu::dialect_registration()
            .unwrap()
            .register_into(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap())
            .unwrap();
    } else {
        dialect_gpu::register_dialect(&mut context).unwrap();
    }
    context
}

#[test]
fn storage_v18_and_existing_gpu_entities_roundtrip_on_both_registration_routes() {
    for owner_hook in [false, true] {
        let mut context = context(owner_hook);
        let key = StorageTableKeyAttrV18::new([0x18; 32], 48);
        let attributes: Vec<AttrObj> = vec![
            Box::new(key),
            Box::new(StorageOrdinalAttrV18(7)),
            Box::new(StorageKindAttrV18::ProjectVariant),
            Box::new(StorageOverlapAttrV18::MayOverlap),
            Box::new(StorageAccessAttrV18::new(
                AddressSpaceAttr::Private,
                8,
                false,
            )),
            Box::new(StorageAccessAttrV18::ABSENT),
            Box::new(CastKindAttr::PointerToGeneric),
            Box::new(CastKindAttr::SliceToGeneric),
            Box::new(CastKindAttr::RestrictPointerAccess),
            Box::new(CastKindAttr::Truncate),
            Box::new(PreservedOperationKindAttr::ExecutionV18),
            Box::new(PreservedOperationKindAttr::OrderedRegionV18),
            Box::new(PreservedOperationKindAttr::OrderedProgramV18),
            Box::new(PreservedOperationKindAttr::VectorLoadV12),
            Box::new(PreservedOperationKindAttr::VerificationContractV12),
        ];
        for attribute in attributes {
            let printed = attribute.disp(&context).to_string();
            let parsed =
                parse_from_str(AttrObj::parser(()).skip(eof()), &mut context, &printed).unwrap();
            assert_eq!(parsed.disp(&context).to_string(), printed);
        }
        let storage: TypeHandle =
            StorageObjectTypeV18::get(&context, key, StorageOrdinalAttrV18(7)).into();
        let role: TypeHandle = ExecutionRoleTypeV18::get(
            &context,
            StorageOrdinalAttrV18(3),
            StorageOrdinalAttrV18(64),
            StorageOrdinalAttrV18(8),
        )
        .into();
        let bf16: TypeHandle = BFloat16Type::get(&context).into();
        let index: TypeHandle = IndexType::get(&context).into();
        let pointer: TypeHandle = PointerType::get(
            &context,
            storage,
            AddressSpaceAttr::Private,
            AccessModeAttr::ReadWrite,
        )
        .into();
        let slice: TypeHandle = SliceType::get(
            &context,
            bf16,
            AddressSpaceAttr::Global,
            AccessModeAttr::ReadOnly,
        )
        .into();
        for ty in [storage, role, bf16, index, pointer, slice] {
            let printed = ty.disp(&context).to_string();
            let parsed =
                parse_from_str(TypeHandle::parser(()).skip(eof()), &mut context, &printed).unwrap();
            assert_eq!(parsed, ty);
        }
    }
}

#[test]
fn storage_v18_registered_operations_roundtrip_without_granting_runtime_authority() {
    for owner_hook in [false, true] {
        let mut context = context(owner_hook);
        let storage: TypeHandle = StorageObjectTypeV18::get(
            &context,
            StorageTableKeyAttrV18::new([0x18; 32], 48),
            StorageOrdinalAttrV18(0),
        )
        .into();
        let pointer: TypeHandle = PointerType::get(
            &context,
            storage,
            AddressSpaceAttr::Private,
            AccessModeAttr::ReadWrite,
        )
        .into();
        let signature = FunctionType::get(&context, vec![pointer], vec![]);
        let function = FuncOp::new(
            &mut context,
            "storage_registration_v18".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let operand = entry.deref(&context).get_argument(0);
        let descriptor = StorageDescriptorV18 {
            kind: StorageKindAttrV18::SetDiscriminant,
            selector: 0,
            overlap: StorageOverlapAttrV18::None,
            read: StorageAccessAttrV18::ABSENT,
            write: StorageAccessAttrV18::new(AddressSpaceAttr::Private, 8, false),
        };
        let operation = StorageOpV18::new(&mut context, descriptor, vec![operand], vec![]);
        verify_op(&operation, &context).unwrap();
        assert_eq!(operation.descriptor(&context), Some(descriptor));
        assert!(
            op_cast::<dyn SideEffects>(&operation)
                .unwrap()
                .has_side_effects(&context)
        );
        let neutral = op_cast::<dyn TargetNeutralGpuOpInterface>(&operation).unwrap();
        assert!(neutral.is_target_neutral());
        assert!(!neutral.grants_runtime_authority());
        operation.get_operation().insert_at_back(entry, &context);
        for kind in [
            PreservedOperationKindAttr::ExecutionV18,
            PreservedOperationKindAttr::OrderedRegionV18,
            PreservedOperationKindAttr::OrderedProgramV18,
        ] {
            let preserved = PreservedOperationOp::new(&mut context, kind, vec![], vec![]);
            verify_op(&preserved, &context).unwrap();
            assert!(
                op_cast::<dyn SideEffects>(&preserved)
                    .unwrap()
                    .has_side_effects(&context)
            );
            assert!(
                !op_cast::<dyn TargetNeutralGpuOpInterface>(&preserved)
                    .unwrap()
                    .grants_runtime_authority()
            );
            preserved.get_operation().insert_at_back(entry, &context);
        }
        let returned = ReturnOp::new(&mut context, vec![]);
        returned.get_operation().insert_at_back(entry, &context);
        verify_operation(function.get_operation(), &context).unwrap();
        let printed = function.get_operation().disp(&context).to_string();
        let parsed = parse_from_str(Operation::top_level_parser(), &mut context, &printed).unwrap();
        verify_operation(parsed, &context).unwrap();
        // Parsing adds locations and fresh arena names; compare the typed graph.
        let parsed_function = Operation::get_op::<FuncOp>(parsed, &context).unwrap();
        assert_eq!(parsed_function.get_type(&context), signature.into());
        assert_eq!(parsed.deref(&context).num_regions(), 1);
        assert_eq!(
            parsed
                .deref(&context)
                .get_region(0)
                .deref(&context)
                .iter(&context)
                .count(),
            1
        );
        let parsed_entry = parsed_function.get_entry_block(&context);
        assert_eq!(parsed_entry.deref(&context).get_num_arguments(), 1);
        let parsed_operand = parsed_entry.deref(&context).get_argument(0);
        assert_eq!(parsed_operand.get_type(&context), pointer);
        let operations: Vec<_> = parsed_entry.deref(&context).iter(&context).collect();
        assert_eq!(operations.len(), 5);
        let storage = Operation::get_op::<StorageOpV18>(operations[0], &context).unwrap();
        assert_eq!(storage.descriptor(&context), Some(descriptor));
        assert_eq!(operations[0].deref(&context).get_operand(0), parsed_operand);
        for (operation, kind) in operations[1..4].iter().zip([
            PreservedOperationKindAttr::ExecutionV18,
            PreservedOperationKindAttr::OrderedRegionV18,
            PreservedOperationKindAttr::OrderedProgramV18,
        ]) {
            let preserved =
                Operation::get_op::<PreservedOperationOp>(*operation, &context).unwrap();
            assert_eq!(preserved.kind(&context), Some(kind));
            assert_eq!(operation.deref(&context).get_num_operands(), 0);
            assert_eq!(operation.deref(&context).get_num_results(), 0);
        }
        for operation in &operations[..4] {
            let operation = Operation::get_op_dyn(*operation, &context);
            assert!(
                op_cast::<dyn SideEffects>(&*operation)
                    .unwrap()
                    .has_side_effects(&context)
            );
            let neutral = op_cast::<dyn TargetNeutralGpuOpInterface>(&*operation).unwrap();
            assert!(neutral.is_target_neutral());
            assert!(!neutral.grants_runtime_authority());
        }
        assert!(Operation::is_op::<ReturnOp>(operations[4], &context));
    }
}
