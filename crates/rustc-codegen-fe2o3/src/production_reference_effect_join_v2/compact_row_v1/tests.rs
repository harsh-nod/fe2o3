// Synthetic component inputs. All source authentication is tested separately.
fn compact_row_join_fixture(
    read: bool,
) -> (
    ProductionRankedKernelV1,
    AuthenticatedReferenceEffectBindingsV1,
    RankedGpuWriteV2,
    Vec<ProductionRankedValueIdV1>,
) {
    let id = ProductionRankedValueIdV1::new;
    let local = |n| ProductionRankedValueV1::Local(id(n));
    let mut ir = crate::reference_effect_v1::compact_row_source_tests::fixture();
    if !read {
        ir.blocks[4].terminator = ReferenceTerminatorV1::Goto { target: 5 };
        ir.blocks[6].assignments[0].value =
            ReferenceValueV1::Use(ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
                scalar: ReferenceScalarTypeV1::U32,
                bits: 17,
            }));
    }
    let mut binding = input_guard_reference(InputGuardShape::And);
    binding.observable_output_writes =
        crate::reference_effect_v1::compact_row_source_tests::writes(&ir).into_boxed_slice();
    binding.effect_ir_sha256 = ir.canonical_sha256_v1();
    binding.effect_ir = ir;
    let mut entry = vec![ProductionRankedOperationV1::InvocationIndex {
        result: id(0),
        dimension: 0,
        launch_extent: 1024,
    }];
    for n in 1..=2 {
        entry.push(ProductionRankedOperationV1::ViewInSpace {
            result: id(n),
            element_width: 32,
            writable: n == 2,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![ProductionRankedValueV1::Argument(n - 1)],
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: n as u64,
            noalias_class: n as u64,
        });
    }
    for (n, value) in [(3, 64), (4, 16), (5, 256)] {
        entry.push(ProductionRankedOperationV1::IndexConstant {
            result: id(n),
            value,
        });
    }
    for (n, kind, lhs, rhs) in [
        (6, IndexBinaryKindAttr::Divide, 0, 3),
        (7, IndexBinaryKindAttr::Remainder, 0, 3),
        (8, IndexBinaryKindAttr::Multiply, 6, 4),
        (9, IndexBinaryKindAttr::Add, 8, 7),
    ] {
        entry.push(ProductionRankedOperationV1::IndexBinary {
            result: id(n),
            kind,
            lhs: local(lhs),
            rhs: local(rhs),
        });
    }
    for n in 10..13 {
        entry.push(ProductionRankedOperationV1::SemanticConstant {
            result: id(n),
            value: 0,
        });
    }
    entry.push(ProductionRankedOperationV1::SemanticSymbol {
        result: id(13),
        symbol: 0,
    });
    let mut blocks = vec![];
    for block in 0..6 {
        let (lhs, rhs) = match block {
            0 => (ProductionRankedValueV1::Argument(0), local(5)),
            1 => (ProductionRankedValueV1::Argument(1), local(5)),
            2 => (local(6), local(4)),
            3 => (local(7), local(4)),
            4 => (local(9), ProductionRankedValueV1::Argument(0)),
            _ => (local(9), ProductionRankedValueV1::Argument(1)),
        };
        blocks.push(ProductionRankedBlockV1::new(
            if block == 0 {
                std::mem::take(&mut entry)
            } else {
                vec![]
            },
            if block < 2 {
                ProductionRankedTerminatorV1::IndexEqual {
                    lhs,
                    rhs,
                    true_block: block + 1,
                    false_block: 7,
                }
            } else {
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs,
                    rhs,
                    true_block: block + 1,
                    false_block: 7,
                }
            },
        ));
    }
    let mut operations = vec![];
    if read {
        operations.push(ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read,
            view: local(1),
            indices: vec![local(9)],
        });
    }
    operations.push(ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Write,
        view: local(2),
        indices: vec![local(9)],
    });
    blocks.push(ProductionRankedBlockV1::new(
        operations,
        ProductionRankedTerminatorV1::Return,
    ));
    blocks.push(ProductionRankedBlockV1::new(
        vec![],
        ProductionRankedTerminatorV1::Return,
    ));
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    let value = if read {
        ProductionSemanticExpressionV2::Load(fe2o3_pliron::ProductionSemanticLoadV2 {
            block: 6,
            operation: 0,
            scalar,
            read_mode: fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedNonVolatile,
            allocation_origin: 1,
            view: local(1),
            indices: vec![local(9)].into_boxed_slice(),
        })
    } else {
        ProductionSemanticExpressionV2::Constant { scalar, bits: 17 }
    };
    let write = RankedGpuWriteV2 {
        block: 6,
        operation: usize::from(read),
        allocation_origin: 2,
        view: local(2),
        indices: vec![local(9)],
        value: Ok(value),
    };
    (
        ProductionRankedKernelV1::new("compact_row_component", 2, blocks).unwrap(),
        AuthenticatedReferenceEffectBindingsV1::new(vec![binding]),
        write,
        (10..14).map(id).collect(),
    )
}

#[test]
fn compact_row_join_prepares_both_placements_with_original_indices_and_rosters() {
    for read in [false, true] {
        let (kernel, bindings, write, reserved) = compact_row_join_fixture(read);
        let original = kernel.clone();
        assert_eq!(reserved_reference_value_count_v2(&bindings).unwrap(), 4);
        let mut request =
            prepare_reference_effect_request_v2(kernel, &bindings, &[write.clone()], reserved)
                .unwrap();
        assert_eq!(request.source_site_remap.is_some(), read);
        assert!(
            request.kernel.blocks()[0]
                .operations()
                .iter()
                .any(|op| matches!(
                    op,
                    ProductionRankedOperationV1::SemanticExpression {
                        expression: ProductionSemanticExpressionV2::Binary {
                            operation: ProductionSemanticBinaryOpV2::Add,
                            ..
                        },
                        ..
                    }
                ))
        );
        for (old, new) in original.blocks().iter().zip(request.kernel.blocks()) {
            assert_eq!(old.terminator(), new.terminator());
        }
        let accesses = request.kernel.blocks()[6]
            .operations()
            .iter()
            .filter_map(|op| match op {
                ProductionRankedOperationV1::Access { kind, indices, .. }
                | ProductionRankedOperationV1::ValueAccess { kind, indices, .. } => {
                    Some((kind, indices))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(accesses.len(), if read { 2 } else { 1 });
        for (_, indices) in accesses {
            assert_eq!(*indices, write.indices);
        }
        assert_eq!(
            request.kernel.blocks()[0]
                .operations()
                .iter()
                .filter(|op| matches!(
                    op,
                    ProductionRankedOperationV1::OwnershipContract {
                        view,
                        coverage: OwnershipCoverageAttr::TotalView,
                        partition: OwnershipPartitionAttr::ExactSets,
                    } if *view == write.view
                ))
                .count(),
            1
        );
        assert!(
            request.kernel.blocks()[1..]
                .iter()
                .flat_map(|b| b.operations())
                .all(|op| !matches!(op, ProductionRankedOperationV1::OwnershipContract { .. }))
        );
        let site = &request.requests[0];
        assert_eq!((site.block, site.operation), (6, if read { 4 } else { 1 }));
        let ProductionRankedOperationV1::RequestEffectRefinement { contract, .. } =
            &request.kernel.blocks()[site.block].operations()[site.operation]
        else {
            panic!("final request position")
        };
        let store = contract.gpu_write_site();
        assert_eq!(
            store,
            ProductionGpuWriteSiteV2::new(6, if read { 3 } else { 0 })
        );
        assert!(
            matches!(request.kernel.blocks()[6].operations()[store.operation() as usize],
            ProductionRankedOperationV1::ValueAccess { value, .. } if value == contract.gpu_value())
        );
        if read {
            use fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1 as Source;
            let mut sources = [
                Source::new(4, Some(3), 0, 6, 0),
                Source::new(6, Some(0), 0, 6, 1),
            ];
            request
                .remap_source_sites_v2(&mut sources, &mut [])
                .unwrap();
            assert_eq!(sources[0], Source::new(4, Some(3), 0, 6, 0));
            assert_eq!(sources[1], Source::new(6, Some(0), 0, 6, 3));
        }
    }
}

#[test]
fn compact_row_join_never_uses_output_extent_as_physical_point_domain() {
    let (kernel, bindings, write, _) = compact_row_join_fixture(true);
    let binding = &bindings.as_slice()[0];
    let domains = [
        crate::production_reference_bounds_v2::CompilerOwnedOutputDomainV2 {
            reference: &binding.observable_output_writes[0],
            ranked_view: write.view,
        },
    ];
    crate::production_reference_bounds_v2::discharge_reference_bounds_with_budget_v2(
        &kernel,
        &binding.effect_ir,
        &domains,
        &mut ReferenceSymbolicWorkBudgetV2::default(),
    )
    .unwrap();
    let mut ir = binding.effect_ir.clone();
    ir.blocks[2].terminator = ReferenceTerminatorV1::Goto { target: 3 };
    let error = crate::production_reference_bounds_v2::discharge_reference_bounds_with_budget_v2(
        &kernel,
        &ir,
        &domains,
        &mut ReferenceSymbolicWorkBudgetV2::default(),
    )
    .unwrap_err();
    assert!(
        error.detail().contains("preceding CPU predicate"),
        "{error:?}"
    );
}

#[test]
fn compact_row_join_missing_reversed_and_wrong_length_gpu_guards_do_not_pair() {
    for mutation in 0..3 {
        let (kernel, bindings, write, reserved) = compact_row_join_fixture(false);
        let mut blocks = kernel.blocks().to_vec();
        let site = if mutation == 2 { 1 } else { 3 };
        let term = match mutation {
            0 => ProductionRankedTerminatorV1::Branch { target: 4 },
            1 => {
                let ProductionRankedTerminatorV1::IndexLessThan { lhs, rhs, .. } =
                    *blocks[3].terminator()
                else {
                    panic!()
                };
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs,
                    rhs,
                    true_block: 7,
                    false_block: 4,
                }
            }
            _ => ProductionRankedTerminatorV1::Branch { target: 2 },
        };
        blocks[site] = ProductionRankedBlockV1::new(blocks[site].operations().to_vec(), term);
        let kernel = ProductionRankedKernelV1::new("compact_row_mutated", 2, blocks).unwrap();
        assert!(
            matches!(
                prepare_reference_effect_request_v2(kernel, &bindings, &[write], reserved),
                Err(ProductionReferenceEffectJoinErrorV2::EffectBijection(_))
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn compact_row_join_wrong_input_coordinate_preserves_read_identity_check() {
    let (kernel, bindings, mut write, reserved) = compact_row_join_fixture(true);
    let Ok(ProductionSemanticExpressionV2::Load(load)) = &mut write.value else {
        panic!()
    };
    load.indices[0] = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
    assert!(matches!(
        prepare_reference_effect_request_v2(kernel, &bindings, &[write], reserved),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "safe reference load has no exact ranked GPU read with matching input, type, and index"
        ))
    ));
}

#[test]
fn compact_row_join_swapped_gpu_map_operands_do_not_reuse_cpu_descriptor() {
    let (kernel, bindings, write, reserved) = compact_row_join_fixture(false);
    let mut blocks = kernel.blocks().to_vec();
    let mut entry = blocks[0].operations().to_vec();
    let ProductionRankedOperationV1::IndexBinary { lhs, rhs, .. } = &mut entry[9] else {
        panic!()
    };
    std::mem::swap(lhs, rhs);
    blocks[0] = ProductionRankedBlockV1::new(entry, blocks[0].terminator().clone());
    let kernel = ProductionRankedKernelV1::new("compact_row_swapped", 2, blocks).unwrap();
    assert!(matches!(
        prepare_reference_effect_request_v2(kernel, &bindings, &[write], reserved),
        Err(ProductionReferenceEffectJoinErrorV2::EffectBijection(
            ReferenceEffectBijectionErrorV1::CoordinateMismatch { .. }
        ))
    ));
}

#[test]
fn compact_row_join_source_and_write_guard_owner_cannot_be_substituted() {
    let (kernel, bindings, write, _) = compact_row_join_fixture(false);
    let ir = &bindings.as_slice()[0].effect_ir;
    let mut scope = gpu_write_path_scope_v2(&kernel, ir, &write).unwrap();
    let other = write.clone();
    let other_ir = (*ir).clone();
    assert!(matches!(
        compact_row_v1::gpu_coordinate(&kernel, ir, 1, &other, &mut scope),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
            detail: "compact row guard belongs to another write",
            ..
        })
    ));
    assert!(matches!(
        compact_row_v1::gpu_coordinate(&kernel, &other_ir, 1, &write, &mut scope),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
            detail: "compact row guard belongs to another write",
            ..
        })
    ));
}

#[test]
fn compact_row_join_placement_keeps_symbol_check_and_shared_exhaustion() {
    let (kernel, bindings, write, reserved) = compact_row_join_fixture(true);
    let binding = &bindings.as_slice()[0];
    let coordinate = &binding.observable_output_writes[0].coordinate;
    let mut operations = kernel.blocks()[0].operations().to_vec();
    let original = operations.clone();
    let mut exhausted = ReferenceSymbolicWorkBudgetV2::default();
    exhausted
        .charge_v2(crate::reference_effect_v1::MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - 9)
        .unwrap();
    assert!(matches!(
        compact_row_v1::place_coordinate(
            &mut operations,
            reserved[3],
            0,
            coordinate,
            &binding.effect_ir,
            &mut exhausted
        ),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "compact row coordinate materialization exceeds the shared bounds work budget"
        ))
    ));
    assert_eq!(operations, original);
    assert!(exhausted.charge_v2(1).is_err());
    operations[13] = ProductionRankedOperationV1::SemanticSymbol {
        result: reserved[3],
        symbol: 1,
    };
    assert!(matches!(
        compact_row_v1::place_coordinate(
            &mut operations,
            reserved[3],
            0,
            coordinate,
            &binding.effect_ir,
            &mut ReferenceSymbolicWorkBudgetV2::default()
        ),
        Err(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
            13
        ))
    ));
    let mut outputs = [PreparedReferenceOutputV2 {
        write: write.clone(),
        reference_write: binding.observable_output_writes[0].clone(),
        output_argument: 1,
        gpu_expression: write.value.clone().unwrap(),
        reference_expression: write.value.unwrap(),
        numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        reserved_values: reserved,
    }];
    assert!(matches!(
        read_root_placement_v1::place(&kernel, &mut outputs, &binding.effect_ir, &mut exhausted),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "compact row coordinate materialization exceeds the shared bounds work budget"
        ))
    ));
}

fn compact_row_threshold_condition(
    operation: ReferenceBinaryOpV1,
    bits: u128,
    scalar: ReferenceScalarTypeV1,
    divisor: u128,
) -> ReferenceEffectExpressionV1 {
    let constant =
        |bits| ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar { scalar, bits });
    ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::LessThan,
        lhs: Box::new(constant(bits)),
        rhs: Box::new(ReferenceEffectExpressionV1::Binary {
            operation,
            lhs: Box::new(ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }),
            rhs: Box::new(constant(divisor)),
            checked: false,
        }),
        checked: false,
    }
}

#[test]
fn compact_row_threshold_exact_unsigned_complement_and_max_boundary() {
    for operation in [ReferenceBinaryOpV1::Divide, ReferenceBinaryOpV1::Remainder] {
        let original =
            compact_row_threshold_condition(operation, 15, ReferenceScalarTypeV1::Usize, 64);
        let (normalized, yes, no) = compact_row_v1::canonical_row_threshold(
            original.clone(),
            3,
            7,
            &mut ReferenceSymbolicWorkBudgetV2::default(),
        )
        .unwrap();
        assert_eq!((yes, no), (7, 3));
        let ReferenceEffectExpressionV1::Binary { lhs, rhs, .. } = normalized else {
            panic!()
        };
        let ReferenceEffectExpressionV1::Binary {
            lhs: old_constant,
            rhs: old_operand,
            ..
        } = original
        else {
            panic!()
        };
        assert_eq!(lhs, old_operand);
        assert_ne!(rhs, old_constant);
        assert!(matches!(
            *rhs,
            ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
                scalar: ReferenceScalarTypeV1::Usize,
                bits: 16
            })
        ));
        for point in (0..1025).chain([u64::MAX]) {
            let value = if operation == ReferenceBinaryOpV1::Divide {
                point / 64
            } else {
                point % 64
            };
            assert_eq!(
                if 15 < value { 3 } else { 7 },
                if value < 16 { yes } else { no }
            );
        }
        let original = compact_row_threshold_condition(
            operation,
            u64::MAX as u128,
            ReferenceScalarTypeV1::Usize,
            64,
        );
        assert_eq!(
            compact_row_v1::canonical_row_threshold(
                original.clone(),
                3,
                7,
                &mut ReferenceSymbolicWorkBudgetV2::default()
            )
            .unwrap(),
            (original, 3, 7)
        );
    }
}

#[test]
fn compact_row_threshold_wrong_type_operation_and_zero_divisor_unchanged() {
    for (operation, scalar, divisor) in [
        (ReferenceBinaryOpV1::Add, ReferenceScalarTypeV1::Usize, 64),
        (ReferenceBinaryOpV1::Divide, ReferenceScalarTypeV1::I64, 64),
        (ReferenceBinaryOpV1::Divide, ReferenceScalarTypeV1::Bool, 1),
        (ReferenceBinaryOpV1::Divide, ReferenceScalarTypeV1::U32, 64),
        (ReferenceBinaryOpV1::Divide, ReferenceScalarTypeV1::Usize, 0),
    ] {
        let original = compact_row_threshold_condition(operation, 0, scalar, divisor);
        assert_eq!(
            compact_row_v1::canonical_row_threshold(
                original.clone(),
                3,
                7,
                &mut ReferenceSymbolicWorkBudgetV2::default()
            )
            .unwrap(),
            (original, 3, 7)
        );
    }
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    work.charge_v2(crate::reference_effect_v1::MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - 5)
        .unwrap();
    assert_eq!(
        compact_row_v1::canonical_row_threshold(
            compact_row_threshold_condition(
                ReferenceBinaryOpV1::Remainder,
                15,
                ReferenceScalarTypeV1::Usize,
                64
            ),
            3,
            7,
            &mut work
        ),
        Err(())
    );
    assert!(work.charge_v2(1).is_err());
}
