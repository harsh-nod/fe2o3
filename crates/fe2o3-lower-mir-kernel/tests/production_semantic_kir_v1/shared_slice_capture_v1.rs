use super::*;

fn wrapped_owner(rust_call: Option<bool>) -> ProductionSemanticMirOwnerV1 {
    let original = owner(Shape::Length, false);
    let semantic = original.semantic();
    let root = &semantic.functions()[0];
    let helper = &semantic.functions()[1];
    let mut types = semantic.types().to_vec();
    let wrapper = ty(types.len() as u32);
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(220)),
            SemanticLayoutIdentityV1::from_sha256(bytes(220)),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                *types[3].layout().backend_repr(),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 16], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![ty(3), ty(0)]).unwrap()),
        )
        .with_rustc_abi_properties(types[3].abi_properties()),
    );
    let tuple = ty(types.len() as u32);
    if rust_call.is_some() {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(230)),
            SemanticLayoutIdentityV1::from_sha256(bytes(230)),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(32),
                8,
                SemanticBackendReprV1::memory(true),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 16, 16], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![ty(3), ty(0), ty(3)]).unwrap(),
            ),
        ));
    }
    let rebuild = |source: &SemanticFunctionDeclV1, abi, locals, blocks| {
        SemanticFunctionDeclV1::new(
            source.identity(),
            source.role(),
            source.item_definition_identity(),
            source.monomorphization_identity(),
            source.generic_type_arguments_identity(),
            source.const_generic_arguments_identity(),
            source.source(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    };
    let mut locals = root.locals().to_vec();
    locals.push(local(221, wrapper, SemanticLocalRoleV1::Temporary));
    let mut arguments = vec![SemanticOperandV1::Move(local_place(4, wrapper))];
    let mut tuple_statements = Vec::new();
    if rust_call.is_some() {
        locals.push(local(231, tuple, SemanticLocalRoleV1::Temporary));
        tuple_statements.push(assign(
            local_place(5, tuple),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        SemanticOperandV1::Copy(local_place(1, ty(3))),
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            ty(0),
                            SemanticConstantValueV1::ZeroSized,
                        )),
                        SemanticOperandV1::Copy(local_place(1, ty(3))),
                    ],
                )
                .unwrap(),
            ),
        ));
        arguments.push(SemanticOperandV1::Move(local_place(5, tuple)));
    }
    let root_blocks = vec![
        block(
            222,
            std::iter::once(assign(
                local_place(4, wrapper),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![
                            SemanticOperandV1::Copy(local_place(1, ty(3))),
                            SemanticOperandV1::Constant(SemanticConstantV1::new(
                                ty(0),
                                SemanticConstantValueV1::ZeroSized,
                            )),
                        ],
                    )
                    .unwrap(),
                ),
            ))
            .chain(tuple_statements)
            .collect(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(1),
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        local_place(3, ty(4)),
                        edge(SemanticEdgeRoleV1::CallReturn, 1),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
        block(223, vec![], SemanticTerminatorKindV1::Return),
    ];
    let entry = rebuild(root, root.abi().clone(), locals, root_blocks)
        .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let mut adjusted = vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        wrapper,
        helper.abi().arguments()[0].mode().clone(),
    ))];
    let mut source_inputs = vec![wrapper];
    if rust_call.is_some() {
        source_inputs.push(tuple);
        for (field, field_ty) in [ty(3), ty(0), ty(3)].into_iter().enumerate() {
            adjusted.push(SemanticAbiArgumentV1::rust_call_tuple_field(
                field as u32,
                SemanticAbiValueV1::new(
                    field_ty,
                    if field == 1 {
                        SemanticAbiPassModeV1::Ignore
                    } else {
                        helper.abi().arguments()[0].mode().clone()
                    },
                ),
            ));
        }
    }
    let input_count = source_inputs.len();
    let helper_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        helper.abi().identity(),
        helper.abi().layout_identity(),
        SemanticCanonAbiV1::Rust,
        if rust_call.is_some() {
            SemanticExternAbiV1::RustCall
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        1,
        source_inputs,
        ty(4),
        adjusted,
        helper.abi().return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue;
        input_count
    ])
    .unwrap();
    let field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(3)).unwrap()],
        ty(3),
    )
    .unwrap();
    let mut helper_locals = vec![
        local(224, ty(4), SemanticLocalRoleV1::Return),
        local(225, wrapper, SemanticLocalRoleV1::Argument(0)),
    ];
    match rust_call {
        None => {}
        Some(false) => helper_locals.push(local(232, tuple, SemanticLocalRoleV1::Argument(1))),
        Some(true) => {
            for (field, field_ty) in [ty(3), ty(0), ty(3)].into_iter().enumerate().rev() {
                helper_locals.push(local(
                    234 - field as u8,
                    field_ty,
                    SemanticLocalRoleV1::RustCallTupleField {
                        argument: 1,
                        field: field as u32,
                    },
                ));
            }
        }
    }
    let helper = rebuild(
        helper,
        helper_abi,
        helper_locals,
        vec![block(
            226,
            vec![assign(
                local_place(0, ty(4)),
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: SemanticOperandV1::Copy(field),
                },
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(root.abi().layout_identity()),
        types,
        vec![],
        vec![],
        vec![],
        vec![entry, helper],
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

#[test]
fn by_value_slice_wrapper_has_one_carrier_and_exact_leaf_coverage() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        wrapped_owner(None),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let helper = &lowered.module().functions[1];
    assert_eq!(
        helper.signature.parameters,
        [Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly
        )]
    );
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget =
        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    let mut nodes = 0;
    lowered.with_checked_arguments_v1(
        SemanticFunctionIdV1::from_index(0), SemanticFunctionIdV1::from_index(1), &mut budget,
        |view| view.visit_nodes(|node| {
            use fe2o3_lower_mir_kernel::{ProductionArgumentCoverageV1 as Coverage, ProductionArgumentProjectionV1 as Projection};
            nodes += 1;
            match node.source_path() {
                [Projection::Field(0)] => assert!(matches!(node.coverage(), Coverage::Parameter(parameter) if parameter.slot() == 0)),
                [Projection::Field(1)] => assert!(matches!(node.coverage(), Coverage::Zero)),
                [] => assert!(matches!(node.coverage(), Coverage::Components { first: 0, end: 1 })),
                _ => panic!("unexpected slice wrapper source path"),
            }
            Ok(())
        }),
    ).unwrap();
    assert_eq!(nodes, 3);
    assert_eq!(budget.storage(), 17);
    let required_work = budget.work();
    let required_storage = budget.peak_storage();
    for (work_limit, storage_limit, passes) in [
        (required_work, required_storage, true),
        (required_work - 1, required_storage, false),
        (required_work, required_storage - 1, false),
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            storage_limit,
        );
        budget.reserve_storage(17).unwrap();
        let result = lowered.with_checked_arguments_v1(
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
            &mut budget,
            |view| view.visit_nodes(|_| Ok(())),
        );
        if passes {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
            ));
        }
        assert_eq!(budget.storage(), 17);
    }
    let failure = lowered.with_checked_arguments_v1(
        SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdV1::from_index(1),
        &mut budget,
        |view| view.visit_nodes(|_| Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)),
    );
    assert!(matches!(
        failure,
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(budget.storage(), 17);
}

#[test]
fn packed_and_expanded_slice_tuple_fields_keep_one_value_per_slice() {
    for expanded in [false, true] {
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            wrapped_owner(Some(expanded)),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        verify_module(lowered.module()).unwrap();
        let helper = &lowered.module().functions[1];
        let slice = Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        assert_eq!(helper.signature.parameters, vec![slice; 3]);
        let root = &lowered.module().functions[0];
        let body = root.body.as_ref().unwrap();
        let call = body.blocks[0]
            .operations
            .iter()
            .find(|operation| matches!(operation.kind, OperationKind::Call { .. }))
            .unwrap();
        assert!(
            matches!(&call.kind, OperationKind::Call { arguments, .. } if arguments == &[body.parameters[0]; 3])
        );
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            usize::MAX,
        );
        let mut slots = Vec::new();
        lowered
            .with_checked_arguments_v1(
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(1),
                &mut budget,
                |view| {
                    view.visit_nodes(|node| {
                        if let fe2o3_lower_mir_kernel::ProductionArgumentCoverageV1::Parameter(
                            parameter,
                        ) = node.coverage()
                        {
                            slots.push(parameter.slot());
                        }
                        Ok(())
                    })
                },
            )
            .unwrap();
        assert_eq!(slots, [0, 1, 2]);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn frozen_evidence_refuses_slice_components_but_preserves_whole_slice_helpers() {
    use fe2o3_lower_mir_kernel::{
        InertCanonicalMirToKirCorrespondenceEvidenceV4 as V4,
        InertCanonicalMirToKirCorrespondenceEvidenceV5 as V5,
        ProductionCorrespondenceEvidenceErrorV4 as Error,
        ProductionCorrespondenceEvidenceErrorV5 as ErrorV5,
    };
    for (source, supported) in [
        (owner(Shape::Length, false), true),
        (wrapped_owner(None), false),
        (wrapped_owner(Some(false)), false),
        (wrapped_owner(Some(true)), false),
    ] {
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            source,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        let report = fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(
            lowered.semantic().semantic(),
            SemanticFunctionIdV1::from_index(0),
        )
        .unwrap();
        let v4 = V4::from_live_owner(&lowered, &report);
        let v5 = V5::from_live_owner(&lowered, &report);
        if supported {
            v4.unwrap();
            v5.unwrap();
        } else {
            assert!(matches!(v4, Err(Error::UnsupportedParameterComponents)));
            assert!(matches!(
                v5,
                Err(ErrorV5::NestedV4(Error::UnsupportedParameterComponents))
            ));
        }
    }
}
