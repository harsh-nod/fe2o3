// Genuine nominal DisjointSlice ABI and original issued-pointer source. The
// unsupported raw UniqueBorrow factory remains a separate boundary negative.
fn issued_descriptor_role_owner_v18(mode: DescriptorRoleSourceV18) -> ProductionSemanticSsaOwnerV1 {
    issued_descriptor_role_owner_with_access_v18(mode, true)
}

fn issued_descriptor_role_owner_with_access_v18(mode: DescriptorRoleSourceV18, used: bool) -> ProductionSemanticSsaOwnerV1 {
    assert!(used || matches!(mode, DescriptorRoleSourceV18::Constant));
    let base = source_issued_pointer_source_tests_v29::owner_with_access(false);
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let scalar = SemanticTypeIdV1::from_index(1);
    assert_eq!(original.locals().len(), 8);
    let pointer = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(7), vec![
        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar).unwrap(),
    ], scalar).unwrap();
    let temporary = |index| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], scalar).unwrap();
    let constant = |bits| SemanticOperandV1::Constant(SemanticConstantV1::new(scalar,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap())));
    let mut locals = original.locals().to_vec();
    let mut statements = original.blocks()[3].statements().to_vec();
    assert_eq!(statements.len(), 1, "original Some payload extraction remains exact");
    if used && !matches!(mode, DescriptorRoleSourceV18::VolatileOnly) {
        locals.push(SemanticLocalDeclV1::new(SemanticLocalIdentityV1::from_sha256([80; 32]), scalar,
            SemanticLocalRoleV1::Temporary, SemanticSourceProvenanceV1::unavailable()));
        statements.push(SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(temporary(8), SemanticRvalueV1::new(scalar,
                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(pointer.clone(),
                    if matches!(mode, DescriptorRoleSourceV18::VolatileRead) { SemanticVolatilityV1::Volatile }
                    else { SemanticVolatilityV1::NonVolatile }, None)))))));
    }
    let operand = match mode {
        DescriptorRoleSourceV18::ReadValue => SemanticOperandV1::Copy(temporary(8)),
        DescriptorRoleSourceV18::Arithmetic => {
            locals.push(SemanticLocalDeclV1::new(SemanticLocalIdentityV1::from_sha256([81; 32]), scalar,
                SemanticLocalRoleV1::Temporary, SemanticSourceProvenanceV1::unavailable()));
            statements.push(SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(temporary(9), SemanticRvalueV1::new(scalar,
                    SemanticRvalueKindV1::Binary { operation: SemanticBinaryOpV1::Add,
                        left: SemanticOperandV1::Copy(temporary(8)), right: constant(1) })))));
            SemanticOperandV1::Copy(temporary(9))
        }
        _ => constant(17),
    };
    if used { statements.push(SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(pointer, operand,
            if matches!(mode, DescriptorRoleSourceV18::Volatile | DescriptorRoleSourceV18::VolatileOnly) {
                SemanticVolatilityV1::Volatile
            } else { SemanticVolatilityV1::NonVolatile }, None)))); }
    let mut blocks = original.blocks().to_vec();
    blocks[3] = SemanticBasicBlockV1::new(blocks[3].identity(), blocks[3].source(), statements,
        blocks[3].terminator().clone()).unwrap();
    let root = SemanticFunctionDeclV1::new(original.identity(), original.role(), original.item_definition_identity(),
        original.monomorphization_identity(), original.generic_type_arguments_identity(), original.const_generic_arguments_identity(),
        original.source(), original.abi().clone(), locals, original.entry(), blocks).unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(source.target(), source.types().to_vec(),
        source.allocations().to_vec(), source.statics().to_vec(), source.vtables().to_vec(), vec![root],
        source.callables().to_vec(), source.roots().to_vec()).unwrap()
        .admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(ProductionSemanticMirOwnerV1::try_new(admitted,
        ProductionSemanticMirLimitsV1::default()).unwrap(), ProductionSemanticSsaLimitsV1::default()).unwrap()
}

fn issued_descriptor_role_abi_v18(owner: &ProductionSemanticSsaOwnerV1)
    -> kernel_argument_abi_v18::tests::FixtureKernelAbiV18
{
    use fe2o3_kernel_descriptor::{DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, LogicalArgumentV1,
        ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeDescriptorV3, SourceTypeRecordV1, ValidName};
    let mut abi = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(owner);
    let semantic = owner.source_semantic();
    let arguments = abi.arguments_mut(0);
    assert_eq!(arguments.len(), 2);
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    for (ordinal, argument) in arguments.iter_mut().enumerate() {
        assert_eq!(argument.semantic_type_identity, semantic.types()[4].identity());
        argument.kind = ProductionKernelArgumentAbiKindV18::Descriptor {
            source: SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32),
            argument: LogicalArgumentV1::disjoint_slice(ordinal as u16,
                ValidName::new(format!("issued{ordinal}")).unwrap(), &source, &layout,
                fe2o3_kernel_descriptor::AccessMode::ReadWrite, (ordinal * 16) as u32).unwrap(),
        };
    }
    abi
}

#[test]
fn original_issued_descriptor_fixture_correction_records_distinct_source_identities() {
    for mode in [DescriptorRoleSourceV18::Constant, DescriptorRoleSourceV18::ReadValue,
        DescriptorRoleSourceV18::Arithmetic, DescriptorRoleSourceV18::Volatile,
        DescriptorRoleSourceV18::VolatileRead, DescriptorRoleSourceV18::VolatileOnly] {
        let old = descriptor_role_owner_v18(mode);
        let corrected = issued_descriptor_role_owner_v18(mode);
        let old_source = old.source_semantic();
        let corrected_source = corrected.source_semantic();
        let old_argument = old_source.functions()[0].locals()[1].ty();
        let corrected_argument = corrected_source.functions()[0].locals()[1].ty();
        assert!(matches!(old_source.types()[old_argument.index() as usize].shape(),
            SemanticTypeShapeV1::Pointer(pointer) if pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.metadata() == SemanticPointerMetadataV1::SliceLength
                && pointer.mutability() == SemanticMutabilityV1::Mutable));
        assert!(matches!(corrected_source.types()[corrected_argument.index() as usize].shape(),
            SemanticTypeShapeV1::Aggregate(_)));
        assert_ne!(old.source_semantic_sha256(), corrected.source_semantic_sha256());
        eprintln!("descriptor fixture correction {mode:?}: original UniqueBorrow source {:02x?}; admitted nominal DisjointSlice source {:02x?}",
            old.source_semantic_sha256(), corrected.source_semantic_sha256());
    }
}

#[test]
fn original_issued_descriptor_roles_check_unused_address_dependencies() {
    let owner = issued_descriptor_role_owner_with_access_v18(DescriptorRoleSourceV18::Constant, false);
    let abi = issued_descriptor_role_abi_v18(&owner);
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_role_owner_with_abi_v18(owner, abi, OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT, |original, optimized, budget| {
            let input_root = original.source.root(0, budget)?.1;
            let recipe = scalar_leaf_collision_recipe_v18(original.inventory.functions()[input_root].function);
            original.with_descriptor_source_roles_v18(optimized, 0, &recipe, budget, |roles, budget| {
                let output = optimized.output_inventory(budget)?;
                let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
                let mut counts = [0; 3];
                for operation in &output.operations()[function.operations.clone()] {
                    let role = roles.role(operation.coordinate, budget)?;
                    match operation.operation.kind {
                        OperationKind::SliceLength { .. } => {
                            assert_eq!(role, Some(DescriptorSourceRoleV18::Length)); counts[0] += 1;
                        }
                        OperationKind::SliceData { .. } => {
                            assert_eq!(role, Some(DescriptorSourceRoleV18::Data)); counts[1] += 1;
                        }
                        OperationKind::GetElementPointer { .. } => {
                            assert_eq!(role, Some(DescriptorSourceRoleV18::Address)); counts[2] += 1;
                        }
                        OperationKind::Load { .. } | OperationKind::Store { .. } => panic!("unused original issuer has no access"),
                        _ => assert!(role.is_none()),
                    }
                }
                assert_eq!(counts, [1, 1, 1]);
                completed.set(true);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            })
        }).0;
    result.unwrap();
    assert!(completed.get());
}

#[test]
fn original_issued_descriptor_store_recipe_checks_both_unchanged_endpoints() {
    for output in [false, true] {
        let expected = if output {
            "actual optimized scalar expression differs from its original source value"
        } else {
            "actual scalar expression differs from its original source value"
        };
        let completed = std::cell::Cell::new(false);
        let result = run_descriptor_roles_v18(DescriptorRoleEntranceV18::IssuedDisjointSlice, DescriptorRoleSourceV18::Constant,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT, |original, optimized, budget| {
                check_descriptor_roles_v18(original, optimized, true, budget)?;
                let root = original.source.root(0, budget)?.1;
                let recipe = scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
                original.with_optimized_scalar_leaves_v18(optimized, 0, &recipe, budget, |leaves, budget| {
                    let count = leaves.visit_store_inputs(budget, |disposition, budget| -> SourceOwnedResultV18<()> {
                        let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request) = disposition
                            else { panic!("genuine original Store must be retained before recipe mutation"); };
                        let exact = ProductionSemanticExpressionV2::Constant { scalar: request.scalar(budget)?, bits: 17 };
                        request.original.check_expression(&exact, budget)?;
                        request.check_expression(&exact, budget)
                    })?;
                    assert_eq!(count, 1, "complete unchanged source Store census precedes the hostile query");
                    leaves.visit_store_inputs(budget, |disposition, budget| -> SourceOwnedResultV18<()> {
                        let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request) = disposition
                            else { return Ok(()); };
                        let scalar = request.scalar(budget)?;
                        let exact = ProductionSemanticExpressionV2::Constant { scalar, bits: 17 };
                        request.original.check_expression(&exact, budget)?;
                        request.check_expression(&exact, budget)?;
                        // This mutates the requested recipe, not the immutable
                        // graph. Actual wrong-producer coverage is separate.
                        let wrong = ProductionSemanticExpressionV2::Constant { scalar, bits: 18 };
                        let refused = if output { request.check_expression(&wrong, budget) }
                            else { request.original.check_expression(&wrong, budget) };
                        let error = refused.unwrap_err();
                        assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(detail)
                            if detail == expected), "{error:?}");
                        completed.set(true);
                        Err(error)
                    }).map(|count| assert_eq!(count, 1))
                })
            }).0;
        assert!(completed.get(), "the exact same candidate precedes the copied recipe mutation: {result:?}");
        assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(detail))
            if detail == expected));
    }
}

#[test]
fn original_issued_descriptor_checked_output_rejects_foreign_result_use_and_same_typed_input() {
    for fault in 0..3 {
        let completed = std::cell::Cell::new(false);
        let expected = if fault == 0 { "issued output definition lost its original producer" }
            else { "issued output operand changed occurrence or value" };
        let result = run_descriptor_roles_v18(DescriptorRoleEntranceV18::IssuedDisjointSlice, DescriptorRoleSourceV18::Constant,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT, |original, optimized, budget| {
                check_descriptor_roles_v18(original, optimized, true, budget)?;
                slice_view_v1::test_issued_output_substitution_v18(original, optimized, fault, &completed, budget)
            }).0;
        assert!(completed.get(), "fault {fault}: {result:?}");
        assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(detail))
            if detail == expected));
    }
}

#[test]
fn original_issued_descriptor_installer_header_cut_precedes_all_role_publication() {
    let completed = std::cell::Cell::new(false);
    let result = run_descriptor_roles_v18(DescriptorRoleEntranceV18::IssuedDisjointSlice, DescriptorRoleSourceV18::Constant,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT, |original, optimized, budget| {
            check_descriptor_roles_v18(original, optimized, true, budget)?;
            let root = original.source.root(0, budget)?.1;
            let recipe = scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            original.with_optimized_scalar_leaves_v18(optimized, 0, &recipe, budget, |leaves, budget| {
                slice_view_v1::test_issued_installer_header_cut_v18(original, optimized, leaves, &completed, budget)
            })
        }).0;
    assert!(completed.get(), "{result:?}");
    assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(_)))));
}
