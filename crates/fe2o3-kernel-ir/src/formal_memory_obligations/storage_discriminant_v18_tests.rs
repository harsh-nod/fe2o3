use super::*;
use crate::{
    AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    Function, Module, Signature, StorageLayoutLimitsV1, Terminator,
};

fn fixture() -> VerifiedCanonicalKernelIrModuleV18 {
    let mut module = Module::new("tag-obligation");
    module.storage_layouts = vec![
        StorageLayoutV1 { size: 1, alignment: 1, kind: StorageLayoutKindV1::Scalar(ScalarType::U8) },
        StorageLayoutV1 { size: 8, alignment: 8, kind: StorageLayoutKindV1::Scalar(ScalarType::U64) },
        StorageLayoutV1 { size: 24, alignment: 8, kind: StorageLayoutKindV1::Record(
            vec![StorageFieldV1 { offset: 8, layout: StorageLayoutIdV1(1) }].into_boxed_slice()) },
        StorageLayoutV1 { size: 24, alignment: 8, kind: StorageLayoutKindV1::Variants {
            encoding: StorageVariantEncodingV1::Direct { tag: StorageFieldV1 { offset: 4, layout: StorageLayoutIdV1(0) } },
            variants: [3, 17, 250].into_iter().zip([255, 1_u128 << 100, u128::MAX])
                .map(|(bits, discriminant)| StorageVariantV1 { discriminant, direct_tag_bits: Some(bits),
                    uninhabited: false, layout: StorageLayoutIdV1(2) }).collect::<Vec<_>>().into_boxed_slice(),
        } },
    ];
    let pointer = Type::pointer(Type::StorageObject(StorageLayoutIdV1(3)), AddressSpace::Private, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(12));
    block.operations.push(Operation::new(vec![ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U128))],
        OperationKind::Storage(StorageOperationV1::ReadDiscriminant { address: ValueId(0), access: MemoryAccess::new(AddressSpace::Private, 1) })));
    block.terminator = Some(Terminator::Return { values: vec![ValueId(2)] });
    module.functions.push(Function::definition("f", Signature::new(vec![pointer.clone(), pointer],
        vec![Type::Scalar(ScalarType::U128)]), vec![ValueId(0), ValueId(1)], vec![block]));
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(&module,
        StorageLayoutLimitsV1 { rows: 8, edges: 16, containment_depth: 8, object_bytes: 32 }, &mut budget).unwrap().0
}

fn coordinate() -> Coordinate {
    Coordinate { block: Block { function: FunctionCoordinate(0), block: 0 }, operation: 0 }
}

fn definition(argument: u32) -> Definition {
    Definition::FunctionArgument { function: FunctionCoordinate(0), argument }
}

#[test]
fn obligation_retains_exact_owner_operand_and_tag_only_geometry_with_all_proofs_pending() {
    let owner = fixture();
    let mut work = Work::new(1_000);
    let mut budget = Budget::new(&mut work, 10_000);
    let obligation = derive_canonical_storage_discriminant_read_v18(&owner, coordinate(), definition(0), &mut budget).unwrap();
    assert!(std::ptr::eq(obligation.owner(), &owner));
    assert!(std::ptr::eq(obligation.operation(), &owner.module().functions[0].body.as_ref().unwrap().blocks[0].operations[0]));
    assert_eq!(obligation.address(), ValueId(0));
    assert_eq!(obligation.address_definition(), definition(0));
    assert_eq!(obligation.coordinate(), coordinate());
    assert_eq!((obligation.tag().offset, obligation.tag_layout().size), (4, 1));
    assert_eq!(obligation.variants().iter().map(|row| row.discriminant).collect::<Vec<_>>(), [255, 1 << 100, u128::MAX]);
    assert_eq!(obligation.requirements().len(), 7);
    assert!(obligation.requirements().contains(&StorageDiscriminantReadRequirementV18::TagInitialization));
    assert!(obligation.requirements().contains(&StorageDiscriminantReadRequirementV18::CurrentEnclosingGuards));
    assert_eq!(budget.work(), 24);
    assert_eq!(budget.storage(), std::mem::size_of::<CanonicalStorageDiscriminantReadObligationV18<'_>>());
}

#[test]
fn same_type_foreign_operand_and_nonexistent_roster_coordinates_do_not_supply_subjects() {
    let owner = fixture();
    for (coordinate, definition, expected) in [
        (coordinate(), definition(1), CanonicalStorageDiscriminantReadErrorV18::AddressDefinition),
        (Coordinate { operation: 1, ..coordinate() }, definition(0), CanonicalStorageDiscriminantReadErrorV18::Coordinate),
        (Coordinate { block: Block { block: 12, ..coordinate().block }, ..coordinate() }, definition(0), CanonicalStorageDiscriminantReadErrorV18::Coordinate),
    ] {
        let mut work = Work::new(1_000);
        let mut budget = Budget::new(&mut work, 10_000);
        assert!(matches!(derive_canonical_storage_discriminant_read_v18(&owner, coordinate, definition, &mut budget), Err(error) if error == expected));
    }
}

#[test]
fn tag_subject_has_independent_exact_and_one_short_work_and_retained_header_limits() {
    let owner = fixture();
    let bytes = std::mem::size_of::<CanonicalStorageDiscriminantReadObligationV18<'_>>();
    for (work_limit, storage_limit, success) in [(24, bytes, true), (23, bytes, false), (24, bytes - 1, false)] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, 17 + storage_limit);
        budget.reserve_storage(17).unwrap();
        let result = derive_canonical_storage_discriminant_read_v18(&owner, coordinate(), definition(0), &mut budget);
        assert_eq!(result.is_ok(), success);
        if success {
            assert_eq!(budget.work(), 24);
            assert_eq!(budget.storage(), 17 + bytes);
            assert_eq!(budget.peak_storage(), 17 + bytes);
            drop(result);
            budget.release_storage(bytes).unwrap();
        } else {
            assert!(matches!(result, Err(CanonicalStorageDiscriminantReadErrorV18::Resource(_))));
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.storage(), 17);
            if work_limit == 23 {
                assert_eq!(budget.work_budget_v1().failed_work(), Some(24));
                assert_eq!(budget.peak_storage(), 17 + bytes);
            } else {
                assert_eq!(budget.failed_storage(), Some(17 + bytes));
                assert_eq!(budget.peak_storage(), 17);
            }
        }
        assert_eq!(budget.storage(), 17);
    }
}
