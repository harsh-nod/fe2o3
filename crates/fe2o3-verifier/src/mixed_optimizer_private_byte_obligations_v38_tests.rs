#[test]
fn byte_function_physical_analysis_rejects_equal_bytes_from_a_foreign_inventory() {
    let module = memory_module(1);
    with_inventory(&module, |inventory, _, floor| {
        with_inventory(&module, |foreign, physical, _| {
            assert_eq!(
                inventory.owner().canonical_bytes(),
                foreign.owner().canonical_bytes()
            );
            let allocations = NoAllocations(inventory.owner());
            let result = run(floor, LIMIT, LIMIT, |out| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    FormalIndexWidth::Bits64,
                    &allocations,
                    out,
                )
                .map(|_| ())
            });
            assert!(matches!(
                result.0,
                Err(Error::Statement(
                    "actual byte function owner or occurrence differs"
                ))
            ));
        });
    });
}

#[test]
fn byte_function_unknown_external_initialization_keeps_complete_dynamic_guards() {
    use fe2o3_kernel_analysis::{
        CanonicalKirPrivateByteObligationV38 as Obligation,
        CanonicalKirPrivateByteOperationKindV38 as Kind,
    };
    with_inventory(&memory_module(1), |inventory, physical, floor| {
        let reads: Vec<_> = inventory
            .operations()
            .iter()
            .enumerate()
            .filter(|(_, row)| matches!(row.operation.kind, OperationKind::Load { .. }))
            .map(|(ordinal, _)| ordinal)
            .collect();
        assert_eq!(reads.len(), 1);
        let fact = physical.operation(reads[0]).unwrap();
        assert_eq!(fact.kind(), Kind::Read);
        assert!(!fact.is_proven());
        assert_eq!(fact.payload_initialized(), None);
        for obligation in [
            Obligation::ExternalMemory,
            Obligation::Currentness,
            Obligation::Bounds,
            Obligation::Alignment,
            Obligation::Initialization,
        ] {
            assert!(fact.requires(obligation));
        }
        let allocations = NoAllocations(inventory.owner());
        let text = run(floor, LIMIT, LIMIT, |out| {
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )?
            .emit(38, out)
        })
        .0
        .unwrap();
        assert!(text.contains("byte_pointer_type_v30(p, 1, 8)"));
        assert!(text.contains("byte_range_aligned_v30(s.memory, p, 4, 4)"));
        assert!(text.contains("byte_scalar_range_initialized_v37(s.memory, p, 4)"));
        assert!(text.contains("s.pc != 0"));
        assert!(text.contains("MemoryValueV30::Undefined"));
    });
}

#[test]
fn byte_function_incomplete_physical_copy_closure_is_not_silently_admitted() {
    use fe2o3_kernel_analysis::CanonicalKirPrivateByteObligationV38 as Obligation;
    let mut module = memory_module(1);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(KirOperation::new(
        vec![ValueDef::new(
            ValueId(0),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        )],
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(KirFunction::internal_helper(
        "other-allocation",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    with_inventory_byte_limit(&module, 0, |inventory, physical, floor| {
        assert!(
            physical
                .operation(0)
                .unwrap()
                .requires(Obligation::CopyBoundaryClosure)
        );
        // The selected function has no Alloca, so its resolver is still exact.
        // The analysis is constructed once for the whole inventory and refuses
        // to present a partial closure as a complete static result.
        let allocations = NoAllocations(inventory.owner());
        let result = run(floor, LIMIT, LIMIT, |out| {
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )
            .map(|_| ())
        });
        assert!(matches!(
            result.0,
            Err(Error::Statement("unresolved physical byte obligation"))
        ));
    });
}
