mod global_metadata_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const ELEMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const PHYSICAL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const VIEW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
    const VIEW_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
    const INDEX: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

    struct MetadataFixture {
        types: Vec<SemanticTypeDeclV1>,
        callables: Vec<SemanticCallableDeclV1>,
        context: RootKernelContextLoweringV1,
        binding: SemanticValueBindingV1,
    }

    fn fixture() -> MetadataFixture {
        let provenance = SemanticKernelCapabilityProvenanceV1::new(
            SemanticFunctionIdV1::from_index(0),
            SemanticKernelBindingIdentityV1::from_sha256([6; 32]),
            SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([4; 32]),
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([2; 32]),
            SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([3; 32]),
            SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([7; 32]),
        )
        .unwrap();
        let context = RootKernelContextLoweringV1 {
            selected_root: SemanticFunctionIdV1::from_index(0),
            semantic_type: UNIT,
            context_type: KernelContextTypeV1::new("metadata_root", [1; 32], [2; 32], [3; 32]),
            source: KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]),
        };
        let pointer = |pointee, metadata| {
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    pointee,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    metadata,
                )
                .unwrap(),
            )
        };
        let ty = |tag, layout, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                layout,
                shape,
            )
        };
        let types = vec![
            unit_type(),
            plain_bit_scalar_type(
                20,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            ty(
                21,
                SemanticTypeLayoutV1::new(None, 4).unwrap(),
                SemanticTypeShapeV1::Slice { element: ELEMENT },
            ),
            ty(
                22,
                SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
                pointer(SLICE, SemanticPointerMetadataV1::SliceLength),
            ),
            ty(
                23,
                SemanticTypeLayoutV1::aggregate(
                    Some(16),
                    8,
                    SemanticAggregateLayoutV1::new(vec![0, 16], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![PHYSICAL, UNIT]).unwrap(),
                ),
            ),
            ty(
                24,
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                pointer(VIEW, SemanticPointerMetadataV1::None),
            ),
            plain_bit_scalar_type(
                25,
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                },
            ),
        ];
        let original = noop_semantic_owner(&["metadata_root"]);
        let callable_identity = SemanticFunctionIdentityV1::from_sha256([77; 32]);
        let callables = vec![SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                callable_identity,
                SemanticItemDefinitionIdentityV1::from_sha256([78; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([79; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([80; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([81; 32]),
                SemanticSourceProvenanceV1::unavailable(),
                original.semantic().functions()[0].abi().clone(),
            ),
            operation: SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                context: UNIT,
                physical: PHYSICAL,
                view: VIEW,
                element: ELEMENT,
                contract: SemanticCapabilityMemoryContractV1::global_read_only(),
                provenance,
                source_identity: callable_identity,
            },
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([82; 32]),
        }];
        let binding = SemanticValueBindingV1::GlobalCapability {
            value: ValueId(40),
            semantic_view: VIEW,
            element: ELEMENT,
            contract: SemanticCapabilityMemoryContractV1::global_read_only(),
            provenance,
            capability: GlobalCapabilityTypeV1::read_only(
                Type::Scalar(ScalarType::U32),
                context.context_type.clone(),
            ),
        };
        MetadataFixture {
            types,
            callables,
            context,
            binding,
        }
    }

    fn physical_field(fixture: &MetadataFixture) -> Option<SemanticValueBindingV1> {
        global_capability_physical_field_v1(
            &fixture.types,
            &fixture.callables,
            Some(&fixture.context),
            VIEW,
            PHYSICAL,
            &fixture.binding,
        )
    }

    #[test]
    fn physical_projection_retains_authority_and_only_exposes_matching_metadata() {
        let fixture = fixture();
        let physical = physical_field(&fixture).unwrap();
        assert!(matches!(
            physical.value().unwrap().1,
            Type::GlobalCapability(_)
        ));
        assert_eq!(
            slice_metadata_carrier_v1(&fixture.types, PHYSICAL, &physical, true),
            Some(ValueId(40))
        );
        assert_eq!(
            slice_metadata_carrier_v1(&fixture.types, SLICE, &physical, false),
            Some(ValueId(40))
        );
        for ty in [UNIT, ELEMENT, VIEW, VIEW_REF, INDEX] {
            assert!(slice_metadata_carrier_v1(&fixture.types, ty, &physical, true).is_none());
        }
        assert!(
            slice_metadata_carrier_v1(&fixture.types, PHYSICAL, &fixture.binding, true).is_none()
        );
        assert!(
            global_capability_physical_field_v1(
                &fixture.types,
                &fixture.callables,
                Some(&fixture.context),
                VIEW,
                PHYSICAL,
                &physical,
            )
            .is_none()
        );
    }

    #[test]
    fn physical_projection_rejects_nominal_field_context_and_bind_substitution() {
        let mut fixture = fixture();
        for (parent, physical) in [(VIEW_REF, PHYSICAL), (VIEW, UNIT), (VIEW, INDEX)] {
            assert!(
                global_capability_physical_field_v1(
                    &fixture.types,
                    &fixture.callables,
                    Some(&fixture.context),
                    parent,
                    physical,
                    &fixture.binding,
                )
                .is_none()
            );
        }
        assert!(
            global_capability_physical_field_v1(
                &fixture.types,
                &fixture.callables,
                None,
                VIEW,
                PHYSICAL,
                &fixture.binding,
            )
            .is_none()
        );
        fixture.context.selected_root = SemanticFunctionIdV1::from_index(1);
        assert!(physical_field(&fixture).is_none());
        fixture.context.selected_root = SemanticFunctionIdV1::from_index(0);
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                    source_identity,
                    ..
                },
            ..
        } = &mut fixture.callables[0]
        else {
            unreachable!()
        };
        *source_identity = SemanticFunctionIdentityV1::from_sha256([99; 32]);
        assert!(physical_field(&fixture).is_none());
        fixture.callables.clear();
        assert!(physical_field(&fixture).is_none());
    }

    #[test]
    fn physical_projection_rejects_offset_role_and_element_substitution() {
        let mut fixture = fixture();
        let original = fixture.types[VIEW.index() as usize].clone();
        fixture.types[VIEW.index() as usize] = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            SemanticTypeLayoutV1::aggregate(
                Some(24),
                8,
                SemanticAggregateLayoutV1::new(vec![8, 24], vec![]).unwrap(),
            )
            .unwrap(),
            original.shape().clone(),
        );
        assert!(physical_field(&fixture).is_none());
        fixture.types[VIEW.index() as usize] = original;
        let SemanticValueBindingV1::GlobalCapability { capability, .. } = &mut fixture.binding
        else {
            unreachable!()
        };
        *capability = GlobalCapabilityTypeV1::new(
            Type::Scalar(ScalarType::U32),
            fixture.context.context_type.clone(),
            GlobalCapabilityRoleV1::ExclusiveReadWrite,
        );
        assert!(physical_field(&fixture).is_none());
        let SemanticValueBindingV1::GlobalCapability { capability, .. } = &mut fixture.binding
        else {
            unreachable!()
        };
        *capability = GlobalCapabilityTypeV1::read_only(
            Type::Scalar(ScalarType::U64),
            fixture.context.context_type.clone(),
        );
        assert!(physical_field(&fixture).is_none());
    }

    #[test]
    fn len_helper_projections_emit_only_capability_length_including_a_physical_temporary() {
        let fixture = fixture();
        let original = noop_semantic_owner(&["metadata_root"]);
        let original = &original.semantic().functions()[0];
        let source = SemanticSourceProvenanceV1::unavailable();
        let function = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            source,
            original.abi().clone(),
            vec![
                original.locals()[0].clone(),
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([240; 32]),
                    VIEW_REF,
                    SemanticLocalRoleV1::Temporary,
                    source,
                ),
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([241; 32]),
                    PHYSICAL,
                    SemanticLocalRoleV1::Temporary,
                    source,
                ),
            ],
            original.entry(),
            original.blocks().to_vec(),
        )
        .unwrap();
        let mut lower = SemanticFunctionLoweringV1::new(
            &fixture.types,
            &fixture.callables,
            &function,
            SemanticParameterBindingsV1 {
                declarations: &[],
                values: &[],
                types: &[],
                local_bindings: None,
            },
            None,
            None,
            BTreeSet::new(),
            1,
            false,
            64,
        )
        .unwrap();
        lower.kernel_context = Some(&fixture.context);
        lower.locals[1] = Some(fixture.binding.clone());
        lower.next_value = 41;
        let projections = vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, VIEW).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), PHYSICAL).unwrap(),
        ];
        let physical = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            projections.clone(),
            PHYSICAL,
        )
        .unwrap();
        let block = SemanticBlockIdV1::from_index(0);
        let mut operations = Vec::new();
        let copied_view = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![projections[0].clone()],
            VIEW,
        )
        .unwrap();
        assert!(matches!(
            lower
                .lower_operand(
                    block,
                    None,
                    &SemanticOperandV1::Copy(copied_view),
                    &mut operations
                )
                .unwrap(),
            SemanticValueBindingV1::GlobalCapability { .. }
        ));
        let carrier = lower
            .lower_operand(
                block,
                None,
                &SemanticOperandV1::Copy(physical.clone()),
                &mut operations,
            )
            .unwrap();
        assert!(operations.is_empty());
        lower.locals[2] = Some(carrier);
        let temporary =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], PHYSICAL).unwrap();
        for place in [physical, temporary] {
            lower
                .lower_rvalue(
                    block,
                    None,
                    INDEX,
                    &SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: SemanticOperandV1::Copy(place),
                    },
                    &mut operations,
                )
                .unwrap();
        }
        let mut projections = projections;
        projections
            .push(SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap());
        let slice =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, SLICE).unwrap();
        lower
            .lower_rvalue(
                block,
                None,
                INDEX,
                &SemanticRvalueKindV1::Length(slice),
                &mut operations,
            )
            .unwrap();
        assert_eq!(operations.len(), 3);
        assert!(operations.iter().all(|operation| matches!(
            operation.kind,
            OperationKind::SliceLength { slice: ValueId(40) }
        )));
        assert!(
            operations
                .iter()
                .all(|operation| kir_memory_accesses_v1(operation).is_empty())
        );
        for projection in [
            SemanticProjectionKindV1::Field(0),
            SemanticProjectionKindV1::OpaqueCast,
            SemanticProjectionKindV1::Subtype,
        ] {
            let substituted = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![SemanticProjectionV1::new(projection, PHYSICAL).unwrap()],
                PHYSICAL,
            )
            .unwrap();
            assert!(
                lower
                    .resolve_place(block, None, &substituted, &mut operations)
                    .is_err()
            );
        }
        assert_eq!(operations.len(), 3);
        let SemanticValueBindingV1::GlobalCapability { capability, .. } = &fixture.binding else {
            unreachable!()
        };
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations = vec![
            Operation::kernel_context_issue(
                ValueId(39),
                fixture.context.context_type.clone(),
                fixture.context.source,
            ),
            Operation::global_capability_bind(
                ValueId(40),
                capability.clone(),
                ValueId(39),
                ValueId(0),
            ),
        ];
        entry.operations.extend(operations);
        entry.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("metadata_test");
        module.functions.push(Function::kernel_entry(
            "metadata_root",
            Signature::new(vec![capability.physical_slice_type()], vec![]),
            vec![ValueId(0)],
            vec![entry],
        ));
        module.kernels.push(Kernel::new(
            "metadata_kernel",
            "metadata_root",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        verify_module(&module).unwrap();
    }

    fn allocation_fixture(conflicting: bool, loop_carried: bool) -> Function {
        let context = KernelContextTypeV1::new("allocation_root", [1; 32], [2; 32], [3; 32]);
        let capability =
            GlobalCapabilityTypeV1::read_only(Type::Scalar(ScalarType::U32), context.clone());
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations = vec![
            Operation::kernel_context_issue(
                ValueId(3),
                context,
                KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]),
            ),
            Operation::global_capability_bind(
                ValueId(4),
                capability.clone(),
                ValueId(3),
                ValueId(0),
            ),
            Operation::global_capability_bind(
                ValueId(5),
                capability.clone(),
                ValueId(3),
                ValueId(1),
            ),
        ];
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(1),
            then_arguments: vec![ValueId(4)],
            else_target: BlockId(1),
            else_arguments: vec![ValueId(if conflicting { 5 } else { 4 })],
        });
        let mut merge = BasicBlock::new(BlockId(1));
        merge.parameters.push(ValueDef::new(
            ValueId(6),
            Type::GlobalCapability(capability.clone()),
        ));
        merge.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(7), capability.physical_pointer_type()),
                OperationKind::SliceData { slice: ValueId(6) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(8), Type::INDEX),
                OperationKind::Constant(Constant::Index(0)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(9), capability.physical_pointer_type()),
                OperationKind::GetElementPointer {
                    base: ValueId(7),
                    offset: ValueId(8),
                },
            ),
        ];
        merge.terminator = Some(if loop_carried {
            Terminator::Branch {
                target: BlockId(1),
                arguments: vec![ValueId(6)],
            }
        } else {
            Terminator::Return { values: vec![] }
        });
        Function::kernel_entry(
            "allocation_root",
            Signature::new(
                vec![
                    capability.physical_slice_type(),
                    capability.physical_slice_type(),
                    Type::BOOL,
                ],
                vec![],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![entry, merge],
        )
    }

    fn allocation_origin(function: &Function, remaining: usize) -> Option<u32> {
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 512 };
        let index =
            build_kir_correlation_index(function.body.as_ref().unwrap(), 32, &mut budget).unwrap();
        external_allocation_parameter_v1(
            function,
            &index,
            ValueId(9),
            &mut BTreeSet::new(),
            &mut UnsupportedIndexCorrelationBudgetV1 { remaining },
        )
    }

    #[test]
    fn global_bind_allocation_origin_survives_parallel_edges_and_loop_backedges() {
        for loop_carried in [false, true] {
            let function = allocation_fixture(false, loop_carried);
            assert_eq!(allocation_origin(&function, 256), Some(0));
            assert_eq!(allocation_origin(&function, 0), None);
        }
    }

    #[test]
    fn global_bind_allocation_origin_rejects_conflicting_or_nonphysical_inputs() {
        assert_eq!(
            allocation_origin(&allocation_fixture(true, false), 256),
            None
        );
        let mut function = allocation_fixture(false, false);
        let OperationKind::GlobalCapabilityBind(bind) =
            &mut function.body.as_mut().unwrap().blocks[0].operations[1].kind
        else {
            unreachable!()
        };
        bind.physical = ValueId(3);
        assert_eq!(allocation_origin(&function, 256), None);
    }

    #[test]
    fn ranked_view_index_preserves_exact_noalias_class_for_both_view_forms() {
        let view = ProductionRankedValueIdV1::new(2);
        for class in [0, 17, 18] {
            let operations = [
                ProductionRankedOperationV1::View {
                    result: view,
                    element_width: 32,
                    writable: false,
                    shape: vec![64],
                    dynamic_extents: vec![],
                    allocation_origin: 5,
                    noalias_class: class,
                },
                ProductionRankedOperationV1::ViewInSpace {
                    result: view,
                    element_width: 32,
                    writable: false,
                    shape: vec![64],
                    dynamic_extents: vec![],
                    memory_space: dialect_kernel::MemorySpaceAttr::Global,
                    allocation_origin: 5,
                    noalias_class: class,
                },
            ];
            for operation in operations {
                let lowering = ranked_correlation_input_for_effects(vec![operation], 1);
                let index = index_ranked_correlation(
                    &lowering,
                    &[],
                    16,
                    &mut UnsupportedIndexCorrelationBudgetV1 { remaining: 256 },
                )
                .unwrap();
                assert_eq!(index.view_definitions[&view].allocation_origin, 5);
                assert_eq!(index.view_definitions[&view].noalias_class, class);
            }
        }
    }
}
