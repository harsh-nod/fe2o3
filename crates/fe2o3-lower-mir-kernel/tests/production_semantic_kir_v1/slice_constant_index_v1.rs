use super::*;

fn read_source(offset: u64) -> ProductionSemanticMirOwnerV1 {
    let source = source_owner(
        IndexSource::Argument,
        Effect::Assign,
        SemanticMutabilityV1::Immutable,
    );
    let semantic = source.semantic();
    let original = &semantic.functions()[0];
    let scalar = SemanticTypeIdV1::from_index(1);
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Dereference,
                SemanticTypeIdV1::from_index(2),
            )
            .unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length: offset + 1,
                    from_end: false,
                },
                scalar,
            )
            .unwrap(),
        ],
        scalar,
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![block(
            190,
            vec![assignment(
                local_place(3, scalar),
                SemanticOperandV1::Copy(place),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

#[test]
fn constant_slice_load_preserves_exact_literal_carrier_element_and_access() {
    // KIR operation-shape coverage only. Ranked bounds-authority tests and real
    // source extraction separately cover the required controlling success edge.
    for offset in [0, 24, 109] {
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            read_source(offset),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        verify_module(lowered.module()).unwrap();
        let body = lowered.module().functions[0].body.as_ref().unwrap();
        let operations = &body.blocks[0].operations;
        assert_eq!(operations.len(), 4);
        assert_eq!(
            operations[0].kind,
            OperationKind::Constant(Constant::Index(offset))
        );
        assert_eq!(operations[0].results[0].ty, Type::INDEX);
        assert_eq!(
            operations[1].kind,
            OperationKind::SliceData {
                slice: body.parameters[0]
            }
        );
        assert_eq!(
            operations[2].kind,
            OperationKind::GetElementPointer {
                base: operations[1].results[0].id,
                offset: operations[0].results[0].id,
            }
        );
        assert_eq!(
            operations[2].results[0].ty,
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly
            )
        );
        assert_eq!(
            operations[3].kind,
            OperationKind::Load {
                pointer: operations[2].results[0].id,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            }
        );
        assert_eq!(operations[3].results[0].ty, Type::Scalar(ScalarType::U32));
        check_span(&lowered, 0, 0, 0, 4);
    }
}

#[test]
fn constant_slice_store_requires_and_preserves_mutable_nonvolatile_access() {
    for effect in [Effect::Assign, Effect::Store] {
        let semantic = request(
            IndexSource::Argument,
            effect,
            SemanticMutabilityV1::Mutable,
            true,
        )
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let source = ProductionSemanticMirOwnerV1::try_new(
            semantic,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            source,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        verify_module(lowered.module()).unwrap();
        let body = lowered.module().functions[0].body.as_ref().unwrap();
        let operations = &body.blocks[0].operations;
        assert_eq!(operations.len(), 5);
        assert_eq!(
            operations[0].kind,
            OperationKind::Constant(Constant::U32(42))
        );
        assert_eq!(
            operations[1].kind,
            OperationKind::Constant(Constant::Index(0))
        );
        assert_eq!(
            operations[2].kind,
            OperationKind::SliceData {
                slice: body.parameters[0]
            }
        );
        assert_eq!(
            operations[3].kind,
            OperationKind::GetElementPointer {
                base: operations[2].results[0].id,
                offset: operations[1].results[0].id,
            }
        );
        assert_eq!(
            operations[3].results[0].ty,
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite
            )
        );
        assert_eq!(
            operations[4].kind,
            OperationKind::Store {
                pointer: operations[3].results[0].id,
                value: operations[0].results[0].id,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            }
        );
        check_span(&lowered, 0, 0, 0, 5);
    }
}
