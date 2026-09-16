use super::*;
use fe2o3_kernel_ir::{AddressSpace, ValueId};

#[derive(Clone, Copy)]
enum Shape {
    Length,
    LengthPair,
    LengthMixed,
    RootRead,
    BranchAndTwoCalls,
    Read,
    Mutable,
    WrongOwnership,
}

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(tag)),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn assign(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    let result = destination.ty();
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(result, value),
        )),
    )
}

fn call(slice: u32, result: u32, target: u32) -> SemanticTerminatorKindV1 {
    call_typed(slice, result, target, ty(4))
}

fn call_typed(
    slice: u32,
    result: u32,
    target: u32,
    result_type: SemanticTypeIdV1,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![SemanticOperandV1::Copy(local_place(slice, ty(3)))],
            Some(SemanticCallDestinationV1::new(
                local_place(result, result_type),
                edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn owner(shape: Shape, f32_element: bool) -> ProductionSemanticMirOwnerV1 {
    let original = indexed_slice_borrow_owner();
    let semantic = original.semantic();
    let root = &semantic.functions()[0];
    // Reuse qualified source layouts, remove the unused thin-reference type,
    // and remap the U64 type from5 to4 before freshly admitting this AST.
    let mut types = semantic.types()[..4].to_vec();
    types.push(semantic.types()[5].clone());
    let pair_result = matches!(shape, Shape::LengthPair | Shape::LengthMixed);
    let result_type = if pair_result { ty(5) } else { ty(4) };
    if pair_result {
        let SemanticBackendReprV1::Scalar(scalar) = *types[4].layout().backend_repr() else {
            panic!("U64 layout")
        };
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(210)),
            SemanticLayoutIdentityV1::from_sha256(bytes(210)),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(scalar, scalar),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![ty(4), ty(0), ty(4)]).unwrap(),
            ),
        ));
    }
    if f32_element {
        types[1] = scalar_type(64, SemanticScalarTypeV1::Float { bits: 32 });
    }
    let mut slice_argument = root.abi().arguments()[0].clone();
    let mut ownership = SemanticSourceArgumentOwnershipV1::SharedBorrow;
    if matches!(shape, Shape::Mutable) {
        let reference = &types[3];
        types[3] = SemanticTypeDeclV1::new(
            reference.identity(),
            reference.layout_identity(),
            reference.layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(2),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                        0,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        );
        let SemanticAbiPassModeV1::Pair { second, .. } = slice_argument.mode() else {
            panic!("qualified slice ABI must be Pair")
        };
        slice_argument = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            ty(3),
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    Some(4),
                )
                .unwrap(),
                second: *second,
            },
        ));
        ownership = SemanticSourceArgumentOwnershipV1::UniqueBorrow;
    }
    let scalar_abi = SemanticAbiValueV1::new(ty(4), root.abi().arguments()[1].mode().clone());
    let mut root_arguments = vec![
        slice_argument.clone(),
        SemanticAbiArgumentV1::source(scalar_abi.clone()),
    ];
    let mut root_ownership = vec![ownership, SemanticSourceArgumentOwnershipV1::ByValue];
    if matches!(shape, Shape::BranchAndTwoCalls) {
        root_arguments.push(slice_argument.clone());
        root_ownership.push(ownership);
    }
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        root.abi().identity(),
        root.abi().layout_identity(),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        u32::try_from(root_arguments.len()).unwrap(),
        root_arguments,
        SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(root_ownership)
    .unwrap();
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(160)),
        root.abi().layout_identity(),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![slice_argument],
        if pair_result {
            let SemanticAbiPassModeV1::Direct(attributes) = scalar_abi.mode() else {
                panic!("U64 ABI")
            };
            SemanticAbiValueV1::new(
                result_type,
                SemanticAbiPassModeV1::Pair {
                    first: *attributes,
                    second: *attributes,
                },
            )
        } else {
            scalar_abi
        },
    )
    .unwrap()
    .with_source_argument_ownership(vec![if matches!(shape, Shape::WrongOwnership) {
        SemanticSourceArgumentOwnershipV1::ByValue
    } else {
        ownership
    }])
    .unwrap();
    let mut root_locals = vec![
        local(170, ty(0), SemanticLocalRoleV1::Return),
        local(171, ty(3), SemanticLocalRoleV1::Argument(0)),
        local(172, ty(4), SemanticLocalRoleV1::Argument(1)),
    ];
    let root_blocks = if matches!(shape, Shape::BranchAndTwoCalls) {
        root_locals.push(local(173, ty(3), SemanticLocalRoleV1::Argument(2)));
        root_locals.push(local(174, ty(4), SemanticLocalRoleV1::Temporary));
        root_locals.push(local(175, ty(3), SemanticLocalRoleV1::Temporary));
        let alias = |source| {
            assign(
                local_place(5, ty(3)),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(local_place(source, ty(3)))),
            )
        };
        vec![
            block(
                180,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(local_place(2, ty(4))),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(
                181,
                vec![alias(1)],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                182,
                vec![alias(3)],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(183, vec![], call(5, 4, 4)),
            block(184, vec![], call(5, 4, 5)),
            block(185, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        root_locals.push(local(173, result_type, SemanticLocalRoleV1::Temporary));
        let mut statements = Vec::new();
        if matches!(shape, Shape::RootRead) {
            root_locals.push(local(174, ty(1), SemanticLocalRoleV1::Temporary));
            statements.push(assign(
                local_place(4, ty(1)),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(1),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(2))
                                .unwrap(),
                            SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                                ty(1),
                            )
                            .unwrap(),
                        ],
                        ty(1),
                    )
                    .unwrap(),
                )),
            ));
        }
        vec![
            block(180, vec![], call_typed(1, 3, 1, result_type)),
            block(181, statements, SemanticTerminatorKindV1::Return),
        ]
    };
    let entry = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root_abi,
        root_locals,
        SemanticBlockIdV1::from_index(0),
        root_blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let mut helper_locals = vec![
        local(190, result_type, SemanticLocalRoleV1::Return),
        local(191, ty(3), SemanticLocalRoleV1::Argument(0)),
    ];
    let mut statements = vec![assign(
        local_place(if pair_result { 2 } else { 0 }, ty(4)),
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::PointerMetadata,
            operand: SemanticOperandV1::Copy(local_place(1, ty(3))),
        },
    )];
    if pair_result {
        helper_locals.push(local(192, ty(4), SemanticLocalRoleV1::Temporary));
        let length = SemanticOperandV1::Copy(local_place(2, ty(4)));
        statements.push(assign(
            local_place(0, result_type),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        length.clone(),
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            ty(0),
                            SemanticConstantValueV1::ZeroSized,
                        )),
                        if matches!(shape, Shape::LengthMixed) {
                            scalar_constant(ty(4), 7, 8)
                        } else {
                            length
                        },
                    ],
                )
                .unwrap(),
            ),
        ));
    }
    if matches!(shape, Shape::Read) {
        helper_locals.push(local(192, ty(4), SemanticLocalRoleV1::Temporary));
        helper_locals.push(local(193, ty(1), SemanticLocalRoleV1::Temporary));
        statements.push(assign(
            local_place(2, ty(4)),
            SemanticRvalueKindV1::Use(scalar_constant(ty(4), 0, 8)),
        ));
        statements.push(assign(
            local_place(3, ty(1)),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(1),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(2))
                            .unwrap(),
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                            ty(1),
                        )
                        .unwrap(),
                    ],
                    ty(1),
                )
                .unwrap(),
            )),
        ));
    }
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(161)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(162)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(163)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(164)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(165)),
        SemanticSourceProvenanceV1::unavailable(),
        helper_abi,
        helper_locals,
        SemanticBlockIdV1::from_index(0),
        vec![block(194, statements, SemanticTerminatorKindV1::Return)],
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(root.abi().layout_identity()),
        types,
        vec![],
        vec![],
        vec![],
        vec![entry, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

#[test]
fn aggregate_slice_metadata_results_keep_separate_per_component_return_conversions() {
    for shape in [Shape::LengthPair, Shape::LengthMixed] {
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            owner(shape, false),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        verify_module(lowered.module()).unwrap();
        let root = SemanticFunctionIdV1::from_index(0);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            usize::MAX,
        );
        lowered.with_checked_call_v1(root, root, SemanticBlockIdV1::from_index(0), &mut budget, |view| {
            assert_eq!(view.result_count(), 2);
            let mut returns = 0;
            view.visit_returns(|site| {
                returns += 1;
                assert_eq!(site.component_count(), 2);
                let first = site.conversion(0).expect("metadata uses INDEX to U64");
                assert!(matches!(first.kind, OperationKind::Cast { kind: fe2o3_kernel_ir::CastKind::Bitcast, value, .. } if Some(value) == site.input(0)));
                if matches!(shape, Shape::LengthPair) {
                    let second = site.conversion(1).expect("each metadata occurrence converts separately");
                    assert!(!std::ptr::eq(first, second));
                    assert_eq!(site.input(0), site.input(1));
                    assert!(matches!(second.kind, OperationKind::Cast { kind: fe2o3_kernel_ir::CastKind::Bitcast, value, .. } if Some(value) == site.input(1)));
                } else {
                    assert!(site.conversion(1).is_none());
                    assert_ne!(site.input(0), site.input(1));
                }
                Ok(())
            })?;
            assert_eq!(returns, 1);
            Ok(())
        }).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn shared_slice_helper_length_preserves_carrier_and_exact_return_cast() {
    for f32_element in [false, true] {
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            owner(Shape::Length, f32_element),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        verify_module(lowered.module()).unwrap();
        let entry = &lowered.module().functions[0];
        let helper = &lowered.module().functions[1];
        let slice = Type::slice(
            Type::Scalar(if f32_element {
                ScalarType::F32
            } else {
                ScalarType::U32
            }),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        assert_eq!(
            helper.signature.parameters.as_slice(),
            std::slice::from_ref(&slice)
        );
        assert_eq!(helper.signature.results, [Type::Scalar(ScalarType::U64)]);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget =
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000);
        let mut nodes = 0;
        lowered
            .with_checked_arguments_v1(
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(1),
                &mut budget,
                |view| {
                    assert_eq!(view.adjusted_arguments()?.len(), 1);
                    assert!(matches!(
                        view.adjusted_arguments()?.next().unwrap().abi().mode(),
                        SemanticAbiPassModeV1::Pair { .. }
                    ));
                    assert!(view.physical(1)?.is_none());
                    view.visit_nodes(|node| {
                        nodes += 1;
                        assert_eq!(node.semantic_type(), ty(3));
                        assert!(node.source_path().is_empty());
                        let fe2o3_lower_mir_kernel::ProductionArgumentCoverageV1::Parameter(
                            parameter,
                        ) = node.coverage()
                        else {
                            panic!("a shared slice is one whole KIR parameter");
                        };
                        assert_eq!(parameter.slot(), 0);
                        assert_eq!(parameter.ty(), &slice);
                        Ok(())
                    })
                },
            )
            .unwrap();
        assert_eq!(nodes, 1);
        assert_eq!(budget.storage(), 0);
        let entry_body = entry.body.as_ref().unwrap();
        let helper_body = helper.body.as_ref().unwrap();
        let [call] = entry_body.blocks[0].operations.as_slice() else {
            panic!("one actual call")
        };
        assert!(
            matches!(&call.kind, OperationKind::Call { callee, arguments } if callee == &helper.id && arguments == &[entry_body.parameters[0]])
        );
        let [length, cast] = helper_body.blocks[0].operations.as_slice() else {
            panic!("length and ABI return conversion")
        };
        assert!(
            matches!(length.kind, OperationKind::SliceLength { slice } if slice == helper_body.parameters[0])
        );
        assert_eq!(length.results[0].ty, Type::INDEX);
        assert!(
            matches!(&cast.kind, OperationKind::Cast { kind: CastKind::Bitcast, value, to } if *value == length.results[0].id && *to == Type::Scalar(ScalarType::U64))
        );
        assert!(
            matches!(&helper_body.blocks[0].terminator, Some(Terminator::Return { values }) if values == &[cast.results[0].id])
        );
        let helper_id = SemanticFunctionIdV1::from_index(1);
        let statement_spans = lowered
            .correspondence()
            .statement_operation_spans()
            .iter()
            .filter(|span| span.semantic_function() == helper_id)
            .collect::<Vec<_>>();
        let terminator_spans = lowered
            .correspondence()
            .terminator_operation_spans()
            .iter()
            .filter(|span| span.semantic_function() == helper_id)
            .collect::<Vec<_>>();
        assert_eq!(statement_spans.len(), 1);
        assert_eq!(
            (
                statement_spans[0].first_operation_ordinal(),
                statement_spans[0].operation_count()
            ),
            (0, 1)
        );
        assert_eq!(terminator_spans.len(), 1);
        assert_eq!(
            (
                terminator_spans[0].first_operation_ordinal(),
                terminator_spans[0].operation_count()
            ),
            (1, 1)
        );
        let effects = analyze_interprocedural_effects_v1(lowered.module()).unwrap();
        assert!(effects.function(&helper.id).unwrap().is_complete_and_pure());
        assert!(effects.function(&entry.id).unwrap().is_complete_and_pure());
    }
}

#[test]
fn shared_slice_helper_calls_retain_actual_phi_carriers_and_multiplicity() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        owner(Shape::BranchAndTwoCalls, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let entry = &lowered.module().functions[0];
    let helper = &lowered.module().functions[1];
    let body = entry.body.as_ref().unwrap();
    let expected = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    assert_eq!(
        entry.signature.parameters,
        [
            expected.clone(),
            Type::Scalar(ScalarType::U64),
            expected.clone()
        ]
    );
    assert_ne!(body.parameters[0], body.parameters[2]);
    let mut calls = Vec::new();
    for block in &body.blocks {
        for operation in &block.operations {
            if let OperationKind::Call { callee, arguments } = &operation.kind {
                assert_eq!(callee, &helper.id);
                assert_eq!(arguments.len(), 1);
                calls.push((block, arguments[0]));
            }
        }
    }
    assert_eq!(calls.len(), 2);
    let (join, phi) = calls[0];
    let (second, second_argument) = calls[1];
    assert_ne!(join.id, second.id);
    let phi_ordinal = join
        .parameters
        .iter()
        .position(|parameter| parameter.id == phi && parameter.ty == expected)
        .expect("distinct source slices require an actual typed merge");
    let mut incoming = Vec::<ValueId>::new();
    for block in &body.blocks {
        if let Some(Terminator::Branch { target, arguments }) = &block.terminator
            && *target == join.id
        {
            incoming.push(arguments[phi_ordinal]);
        }
    }
    incoming.sort();
    let mut source_arguments = vec![body.parameters[0], body.parameters[2]];
    source_arguments.sort();
    assert_eq!(incoming, source_arguments);
    // A single-predecessor call block may reuse the dominating merge directly.
    // If it retains a parameter, its actual incoming edge must forward that merge.
    if second_argument != phi {
        let ordinal = second
            .parameters
            .iter()
            .position(|parameter| parameter.id == second_argument && parameter.ty == expected)
            .expect("the second call must reuse or forward the typed merge");
        let Some(Terminator::Branch { target, arguments }) = &join.terminator else {
            panic!("the first call returns directly to the second call block")
        };
        assert_eq!(*target, second.id);
        assert_eq!(arguments[ordinal], phi);
    }
    let root = SemanticFunctionIdV1::from_index(0);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget =
        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000);
    for (block, argument) in calls {
        lowered.with_checked_call_v1(root, root, SemanticBlockIdV1::from_index(block.id.0), &mut budget, |view| {
            let physical = view.physical(0)?.unwrap();
            assert_eq!(physical.caller_value(), argument);
            assert_eq!(physical.parameter().ty(), &expected);
            let mut saved = None;
            view.visit_returns(|site| { saved = Some(site); Ok(()) })?;
            view.visit_arguments(|node| {
                assert_eq!(node.parameter().source_argument(), 0);
                Ok(())
            })?;
            let site = saved.unwrap();
            let cast = site.conversion(0).expect("slice length return uses INDEX -> U64");
            assert!(matches!(cast.kind, OperationKind::Cast { kind: fe2o3_kernel_ir::CastKind::Bitcast, value, .. } if Some(value) == site.input(0)));
            assert_eq!(physical.parameter().ty(), &expected);
            Ok(())
        }).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn shared_slice_helper_load_is_not_mislabeled_pure() {
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            owner(Shape::Read, false),
            ProductionSemanticKirLimitsV1::default()
        ),
        Err(ProductionSemanticKirErrorV1::HelperEffectsUnavailable { function: 1, .. })
    ));
}

#[test]
fn shared_slice_helper_subset_does_not_admit_mutable_or_unowned_pairs() {
    for shape in [Shape::Mutable, Shape::WrongOwnership] {
        assert!(matches!(
            ProductionSemanticKirOwnerV1::try_lower(
                owner(shape, false),
                ProductionSemanticKirLimitsV1::default()
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 1,
                detail: "helper parameter is not an exact by-value scalar aggregate or shared slice",
                ..
            })
        ));
    }
}

#[path = "shared_slice_helper_llvm_v1.rs"]
mod llvm_abi;

#[path = "shared_slice_capture_v1.rs"]
mod captures;
