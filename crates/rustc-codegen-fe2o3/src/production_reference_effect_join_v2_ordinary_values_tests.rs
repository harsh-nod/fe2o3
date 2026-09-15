// Request-construction components, not authenticated source/optimizer or local
// Verus proof-runtime fixtures. Both independently recorded writes are required.
fn mixed_value_reference_fixture_v2() -> (
    ProductionRankedKernelV1,
    AuthenticatedReferenceEffectBindingsV1,
    Vec<RankedGpuWriteV2>,
    Vec<ProductionRankedValueIdV1>,
) {
    let (kernel, mut first) = dynamic_point_kernel(false);
    let mut blocks = kernel.blocks().to_vec();
    let second_view = ProductionRankedValueIdV1::new(2);
    let source_value = ProductionRankedValueIdV1::new(11);
    let mut entry = blocks[0].operations().to_vec();
    entry.push(ProductionRankedOperationV1::ViewInSpace {
        result: second_view,
        element_width: 32,
        writable: true,
        shape: vec![DYNAMIC_EXTENT],
        dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
        memory_space: MemorySpaceAttr::Global,
        allocation_origin: 2,
        noalias_class: 2,
    });
    let reserved = (3..11)
        .map(ProductionRankedValueIdV1::new)
        .collect::<Vec<_>>();
    for ids in reserved.chunks_exact(4) {
        for id in &ids[..3] {
            entry.push(ProductionRankedOperationV1::SemanticConstant {
                result: *id,
                value: 0,
            });
        }
        entry.push(ProductionRankedOperationV1::SemanticSymbol {
            result: ids[3],
            symbol: 0,
        });
    }
    blocks[0] = ProductionRankedBlockV1::new(entry, blocks[0].terminator().clone());
    let expression = first.value.clone().unwrap();
    blocks[2] = ProductionRankedBlockV1::new(
        vec![
            ProductionRankedOperationV1::SemanticExpression {
                result: source_value,
                numerical_contract: ProductionNumericalContractV2::exact_for_expression(
                    &expression,
                ),
                expression,
            },
            ProductionRankedOperationV1::ValueAccess {
                kind: AccessKindAttr::Write,
                view: first.view,
                indices: first.indices.clone(),
                value: ProductionRankedValueV1::Local(source_value),
            },
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                view: ProductionRankedValueV1::Local(second_view),
                indices: first.indices.clone(),
            },
        ],
        ProductionRankedTerminatorV1::Return,
    );
    first.operation = 1;
    let mut second = first.clone();
    second.operation = 2;
    second.allocation_origin = 2;
    second.view = ProductionRankedValueV1::Local(second_view);
    second.value = Ok(ProductionSemanticExpressionV2::Constant {
        scalar: ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        bits: 19,
    });
    let outputs = [17, 19]
        .into_iter()
        .enumerate()
        .map(|(argument, bits)| {
            let constant = ReferenceConstantV1::Scalar {
                scalar: ReferenceScalarTypeV1::U32,
                bits,
            };
            ReferenceOutputWriteV1 {
                argument: argument as u32,
                block: 0,
                statement: argument as u32,
                coordinate: ReferenceOutputCoordinateV1::LogicalPoint(
                    vec![ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }]
                        .into_boxed_slice(),
                ),
                guard: ReferencePathPredicateV1::unconditional_v1(),
                rhs: ReferenceEffectExpressionV1::Constant(constant.clone()),
                value: ReferenceValueV1::Use(ReferenceOperandV1::Constant(constant)),
            }
        })
        .collect::<Vec<_>>();
    let effect_ir = ReferenceEffectIrV1 {
        argument_count: 2,
        local_count: 3,
        relations: (0..2)
            .map(
                |argument| ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                    argument,
                    element: ReferenceScalarTypeV1::U32,
                },
            )
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        blocks: vec![ReferenceBlockV1 {
            block: 0,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Return,
        }]
        .into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: outputs.clone().into_boxed_slice(),
    };
    let identity = ReferenceFunctionIdentityV1 {
        def_path_hash: [1; 16],
        function_sha256: [2; 32],
        item_definition_sha256: [3; 32],
        monomorphization_sha256: [4; 32],
        generic_type_arguments_sha256: [5; 32],
        const_generic_arguments_sha256: [6; 32],
        rustc_mir_body_sha256: [7; 32],
    };
    let bindings =
        AuthenticatedReferenceEffectBindingsV1::new(vec![AuthenticatedReferenceEffectBindingV1 {
            registration_path: "component".into(),
            logical_kernel_name: "component".into(),
            kernel: identity,
            reference: identity,
            effect_ir_sha256: [8; 32],
            effect_ir,
            observable_output_writes: outputs.into_boxed_slice(),
        }]);
    (
        ProductionRankedKernelV1::new("mixed_value_reference", 2, blocks).unwrap(),
        bindings,
        vec![first, second],
        reserved,
    )
}

#[test]
fn mixed_preattached_and_legacy_reference_requests_keep_both_exact_values() {
    let (kernel, bindings, writes, reserved) = mixed_value_reference_fixture_v2();
    let original = kernel.blocks()[2].operations()[..2].to_vec();
    let legacy_value = reserved[5];
    let request =
        prepare_reference_effect_request_v2(kernel, &bindings, &writes, reserved).unwrap();
    assert_eq!(request.requests.len(), 2);
    assert_eq!(request.kernel.blocks()[2].operations()[..2], original);
    assert!(
        matches!(request.kernel.blocks()[2].operations()[2], ProductionRankedOperationV1::ValueAccess {
        value: ProductionRankedValueV1::Local(value), ..
    } if value == legacy_value)
    );
    assert_eq!(
        request
            .kernel
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
            .filter(|operation| {
                matches!(
                    operation,
                    ProductionRankedOperationV1::ValueAccess {
                        kind: AccessKindAttr::Write,
                        ..
                    }
                )
            })
            .count(),
        2
    );
    assert_eq!(
        request
            .requests
            .iter()
            .map(|request| (request.block, request.operation))
            .collect::<Vec<_>>(),
        vec![(2, 3), (2, 4)]
    );
}

#[test]
fn preattached_reference_value_requires_exact_tree_contract_and_complete_bijection() {
    for hostile in 0..3 {
        let (kernel, bindings, mut writes, reserved) = mixed_value_reference_fixture_v2();
        match hostile {
            0 => {
                writes[0].value = Ok(ProductionSemanticExpressionV2::Constant {
                    scalar: ProductionSemanticScalarTypeV2::Integer {
                        signed: false,
                        bits: 32,
                    },
                    bits: 18,
                })
            }
            1 => {
                writes.pop();
            }
            2 => writes.push(writes[0].clone()),
            _ => unreachable!(),
        }
        let result = prepare_reference_effect_request_v2(kernel, &bindings, &writes, reserved);
        match hostile {
            0 => assert!(matches!(
                result,
                Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                    detail: "pre-attached GPU write value differs from its exact source expression or numerical contract",
                    ..
                })
            )),
            _ => assert!(matches!(
                result,
                Err(ProductionReferenceEffectJoinErrorV2::EffectBijection(_))
            )),
        }
    }
    let (kernel, _, writes, _) = mixed_value_reference_fixture_v2();
    let expression = writes[0].value.as_ref().unwrap();
    assert!(
        preattached_gpu_write_value_v2(
            &kernel,
            &writes[0],
            expression,
            ProductionNumericalContractV2::Relaxed
        )
        .is_err()
    );
}
