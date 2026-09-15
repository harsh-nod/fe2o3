// Component graph/reference inputs only. No fixture is a source-authentication
// receipt, and preparing distinct roots is not a numerical proof result.

type RosterLoad = fe2o3_pliron::ProductionSemanticLoadV2;

#[test]
fn reference_roster_unchanged_add_keeps_equal_independent_roots() {
    let (kernel, bindings, mut write, loads, reserved) = roster_join_fixture();
    let ProductionSemanticExpressionV2::Binary { rhs, .. } = write.value.as_mut().unwrap() else {
        panic!("add")
    };
    *rhs = Box::new(ProductionSemanticExpressionV2::Load(loads[1].clone()));
    let input = roster_join_input(kernel, write, loads);
    let request = prepare_projected_reference_effect_request_v2(input, &bindings, reserved)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let expressions = [2, 3].map(
        |index| match &request.kernel.blocks()[3].operations()[index] {
            ProductionRankedOperationV1::SemanticExpression { expression, .. } => expression,
            _ => panic!("independent root"),
        },
    );
    assert_eq!(expressions[0], expressions[1]);
}

fn roster_join_fixture() -> (
    ProductionRankedKernelV1,
    AuthenticatedReferenceEffectBindingsV1,
    RankedGpuWriteV2,
    Vec<RosterLoad>,
    Vec<ProductionRankedValueIdV1>,
) {
    let (kernel, bindings, mut write, reserved) = read_placement_fixture();
    let mut binding = bindings.as_slice()[0].clone();
    use crate::reference_effect_v1::ReferencePlaceProjectionV1;
    let place = |local| ReferencePlaceV1 {
        local,
        projection: Box::default(),
    };
    let operand = |local| ReferenceOperandV1::Copy(place(local));
    // Keep both existing branch guards. On their successful path, retain each
    // slice assertion before the two real indexed loads and their output sum.
    let mut blocks = binding.effect_ir.blocks.to_vec();
    let ReferenceTerminatorV1::Switch { otherwise, .. } = &mut blocks[1].terminator else {
        panic!("second input guard")
    };
    assert_eq!(*otherwise, 2);
    *otherwise = 4;
    for (block, condition, length, success) in [(4, 6, 5, 5), (5, 8, 7, 2)] {
        blocks.push(ReferenceBlockV1 {
            block,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Assert {
                condition: operand(condition),
                expected: true,
                success,
                bounds_check: Some(ReferenceBoundsCheckV1 {
                    index: operand(1),
                    length: operand(length),
                }),
            },
        });
    }
    let mut assignments = [1, 2]
        .into_iter()
        .enumerate()
        .map(|(statement, reference_argument)| ReferenceAssignmentV1 {
            statement: statement as u32,
            destination: place(9 + statement as u32),
            value: ReferenceValueV1::Use(ReferenceOperandV1::Copy(ReferencePlaceV1 {
                local: reference_argument + 1,
                projection: vec![
                    ReferencePlaceProjectionV1::Dereference,
                    ReferencePlaceProjectionV1::Index(1),
                ]
                .into_boxed_slice(),
            })),
        })
        .collect::<Vec<_>>();
    let mut output = blocks[2].assignments[0].clone();
    output.statement = 2;
    output.value = ReferenceValueV1::Binary {
        operation: ReferenceBinaryOpV1::Add,
        lhs: operand(9),
        rhs: operand(10),
        checked: false,
    };
    assignments.push(output.clone());
    blocks[2].assignments = assignments.into_boxed_slice();
    binding.effect_ir.blocks = blocks.into_boxed_slice();
    binding.effect_ir.local_count = 11;
    let guard = crate::reference_effect_v1::reference_block_path_predicates_v1(&binding.effect_ir)
        .unwrap()[2]
        .clone();
    assert_eq!(guard, binding.observable_output_writes[0].guard);
    binding.observable_output_writes[0].guard = guard;
    binding.observable_output_writes[0].statement = output.statement;
    binding.observable_output_writes[0].value = output.value;
    let input = |reference_argument| ReferenceEffectExpressionV1::InputLoad {
        reference_argument,
        index: Box::new(ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }),
    };
    binding.observable_output_writes[0].rhs = ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::Add,
        lhs: Box::new(input(1)),
        rhs: Box::new(input(2)),
        checked: false,
    };
    binding.effect_ir.observable_output_effects = binding.observable_output_writes.clone();
    binding.effect_ir_sha256 = binding.effect_ir.canonical_sha256_v1();
    let mut loads = Vec::new();
    collect_semantic_loads_v2(write.value.as_ref().unwrap(), &mut loads);
    let loads = loads.into_iter().cloned().collect();
    let ProductionSemanticExpressionV2::Binary { rhs, scalar, .. } = write.value.as_mut().unwrap()
    else {
        panic!("sum")
    };
    *rhs = Box::new(ProductionSemanticExpressionV2::Constant {
        scalar: *scalar,
        bits: 1,
    });
    (
        kernel,
        AuthenticatedReferenceEffectBindingsV1::new(vec![binding]),
        write,
        loads,
        reserved,
    )
}

#[test]
fn reference_roster_fixture_retains_both_exact_bounds_assertions() {
    let (kernel, bindings, write, loads, reserved) = roster_join_fixture();
    let binding = &bindings.as_slice()[0];
    let checks = binding
        .effect_ir
        .resolved_bounds_checks_with_budget_v1(
            &mut crate::reference_effect_v1::ReferenceSymbolicWorkBudgetV2::default(),
        )
        .unwrap();
    assert_eq!(checks.len(), 2);
    for (check, (block, reference_argument)) in checks.iter().zip([(4, 1), (5, 2)]) {
        assert_eq!(check.block, block);
        assert!(check.expected);
        assert_eq!(
            check.index,
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }
        );
        assert_eq!(
            check.length,
            ReferenceEffectExpressionV1::InputLength { reference_argument }
        );
        assert_eq!(
            check.condition,
            ReferenceEffectExpressionV1::Binary {
                operation: ReferenceBinaryOpV1::LessThan,
                lhs: Box::new(check.index.clone()),
                rhs: Box::new(check.length.clone()),
                checked: false,
            }
        );
    }
    assert_eq!(
        binding.effect_ir.observable_output_effects,
        binding.observable_output_writes
    );
    assert_eq!(
        binding.effect_ir_sha256,
        binding.effect_ir.canonical_sha256_v1()
    );
    prepare_projected_reference_effect_request_v2(
        roster_join_input(kernel, write, loads),
        &bindings,
        reserved,
    )
    .unwrap_or_else(|error| panic!("both checked reference reads must discharge: {error:?}"));
}

#[test]
fn reference_roster_each_missing_bounds_assertion_rejects_before_proof() {
    for block in [4, 5] {
        let (kernel, bindings, write, loads, reserved) = roster_join_fixture();
        let mut binding = bindings.as_slice()[0].clone();
        let ReferenceTerminatorV1::Assert { success, .. } =
            binding.effect_ir.blocks[block].terminator
        else {
            panic!("retained slice assertion")
        };
        binding.effect_ir.blocks[block].terminator =
            ReferenceTerminatorV1::Goto { target: success };
        binding.effect_ir_sha256 = binding.effect_ir.canonical_sha256_v1();
        assert!(matches!(
            prepare_projected_reference_effect_request_v2(
                roster_join_input(kernel, write, loads),
                &AuthenticatedReferenceEffectBindingsV1::new(vec![binding]),
                reserved,
            ),
            Err(ProductionReferenceEffectJoinErrorV2::ReferenceBoundsCheck {
                block: 2,
                detail,
            }) if detail == "safe-slice access `point[0]` has no exact retained bounds assertion"
        ));
    }
}

fn roster_join_input(
    kernel: ProductionRankedKernelV1,
    write: RankedGpuWriteV2,
    loads: Vec<RosterLoad>,
) -> ProjectedReferenceInputV2 {
    ProjectedReferenceInputV2::fixture(
        kernel,
        [2; 32],
        vec![write],
        loads,
        read_placement_sources(),
    )
    .unwrap_or_else(|error| panic!("read roster sealing failed: {error:?}"))
}

#[test]
fn reference_roster_keeps_cpu_only_read_and_exact_remapped_source_roster() {
    let (kernel, bindings, write, loads, reserved) = roster_join_fixture();
    let input = roster_join_input(kernel, write, loads);
    let mut request = prepare_projected_reference_effect_request_v2(input, &bindings, reserved)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let operations = request.kernel.blocks()[3].operations();
    let expression = |index| match &operations[index] {
        ProductionRankedOperationV1::SemanticExpression { expression, .. } => expression,
        _ => panic!("typed root"),
    };
    let ProductionSemanticExpressionV2::Binary { rhs: gpu, .. } = expression(2) else {
        panic!("GPU add")
    };
    let ProductionSemanticExpressionV2::Binary { rhs: cpu, .. } = expression(3) else {
        panic!("CPU add")
    };
    assert!(matches!(
        gpu.as_ref(),
        ProductionSemanticExpressionV2::Constant { bits: 1, .. }
    ));
    assert!(
        matches!(cpu.as_ref(), ProductionSemanticExpressionV2::Load(load) if (load.block, load.operation) == (3, 1))
    );
    assert_eq!(
        operations
            .iter()
            .filter(|op| matches!(
                op,
                ProductionRankedOperationV1::Access { .. }
                    | ProductionRankedOperationV1::ValueAccess { .. }
            ))
            .count(),
        3
    );
    let (accesses, generated) = request.take_and_remap_sources_v2().unwrap();
    assert!(generated.is_empty());
    for (original, remapped) in read_placement_sources().iter().zip(&accesses) {
        assert_eq!(
            (
                original.semantic_block(),
                original.semantic_statement(),
                original.semantic_access_ordinal()
            ),
            (
                remapped.semantic_block(),
                remapped.semantic_statement(),
                remapped.semantic_access_ordinal()
            )
        );
    }
    assert_eq!(
        accesses
            .iter()
            .map(|source| (source.ranked_block(), source.ranked_operation()))
            .collect::<Vec<_>>(),
        vec![(3, 0), (3, 1), (3, 4)]
    );
    assert!(request.source_site_remap.is_none());
}

#[test]
fn reference_roster_rejects_other_root_even_with_identical_graph_coordinates() {
    let (kernel, bindings, write, loads, reserved) = roster_join_fixture();
    let input = roster_join_input(kernel, write, loads);
    let mut foreign = bindings.as_slice()[0].clone();
    foreign.kernel.function_sha256 = [3; 32];
    assert!(matches!(
        prepare_projected_reference_effect_request_v2(
            input,
            &AuthenticatedReferenceEffectBindingsV1::new(vec![foreign]),
            reserved
        ),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference read roster belongs to another kernel root"
        ))
    ));
}

#[test]
fn reference_roster_missing_read_never_becomes_an_opaque_reference_symbol() {
    let (kernel, bindings, write, mut loads, reserved) = roster_join_fixture();
    loads.pop();
    let input = roster_join_input(kernel, write, loads);
    assert!(matches!(
        prepare_projected_reference_effect_request_v2(input, &bindings, reserved),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "safe reference load has no exact ranked GPU read with matching input, type, and index"
        ))
    ));
}

#[test]
fn reference_roster_source_metadata_must_agree_with_gpu_leaf() {
    for mutation in 0..4 {
        let (kernel, bindings, mut write, loads, reserved) = roster_join_fixture();
        let ProductionSemanticExpressionV2::Binary { lhs, .. } = write.value.as_mut().unwrap()
        else {
            panic!("add")
        };
        let ProductionSemanticExpressionV2::Load(load) = lhs.as_mut() else {
            panic!("load")
        };
        match mutation {
            0 => load.read_mode = fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedVolatile,
            1 => load.allocation_origin = 2,
            2 => {
                load.indices[0] = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1))
            }
            _ => load.view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(5)),
        }
        let input = roster_join_input(kernel, write, loads);
        assert!(matches!(
            prepare_projected_reference_effect_request_v2(input, &bindings, reserved),
            Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                "one reference read occurrence has conflicting source metadata"
            ))
        ));
    }
}

#[test]
fn reference_roster_input_type_and_index_matching_stays_exact() {
    for mutation in 0..3 {
        let (kernel, bindings, write, loads, reserved) = roster_join_fixture();
        let mut binding = bindings.as_slice()[0].clone();
        let expected = if mutation == 0 {
            let ReferenceEffectExpressionV1::Binary { rhs, .. } =
                &mut binding.observable_output_writes[0].rhs
            else {
                panic!("CPU add")
            };
            let ReferenceEffectExpressionV1::InputLoad {
                reference_argument, ..
            } = rhs.as_mut()
            else {
                panic!("CPU read")
            };
            *reference_argument = 99;
            "safe reference load base is not one exact shared-slice input"
        } else {
            if mutation == 1 {
                for relation in &mut binding.effect_ir.relations {
                    if let ReferenceArgumentRelationV1::SharedSliceInput {
                        argument: 1,
                        element,
                    } = relation
                    {
                        *element = ReferenceScalarTypeV1::U64;
                    }
                }
            } else {
                let ReferenceEffectExpressionV1::Binary { rhs, .. } =
                    &mut binding.observable_output_writes[0].rhs
                else {
                    panic!("CPU add")
                };
                let ReferenceEffectExpressionV1::InputLoad { index, .. } = rhs.as_mut() else {
                    panic!("CPU read")
                };
                *index = Box::new(ReferenceEffectExpressionV1::Constant(
                    ReferenceConstantV1::Scalar {
                        scalar: ReferenceScalarTypeV1::Usize,
                        bits: 1,
                    },
                ));
            }
            "safe reference load has no exact ranked GPU read with matching input, type, and index"
        };
        let input = roster_join_input(kernel, write, loads);
        assert!(
            matches!(prepare_projected_reference_effect_request_v2(input, &AuthenticatedReferenceEffectBindingsV1::new(vec![binding]), reserved),
            Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(actual)) if actual == expected)
        );
    }
}

#[test]
fn reference_roster_distinct_equal_address_reads_are_not_merged() {
    let (kernel, bindings, mut write, mut loads, reserved) = roster_join_fixture();
    let mut blocks = kernel.blocks().to_vec();
    let mut operations = blocks[3].operations().to_vec();
    operations.insert(2, operations[1].clone());
    blocks[3] = ProductionRankedBlockV1::new(operations, blocks[3].terminator().clone());
    let mut second = loads[1].clone();
    second.operation = 2;
    loads.push(second);
    write.operation = 3;
    let sources = (0..4)
        .map(|ordinal| {
            fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1::new(
                11,
                Some(2),
                ordinal,
                3,
                ordinal,
            )
        })
        .collect();
    let kernel =
        ProductionRankedKernelV1::new(kernel.function_name(), kernel.argument_count(), blocks)
            .unwrap();
    let input = ProjectedReferenceInputV2::fixture(kernel, [2; 32], vec![write], loads, sources)
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert!(matches!(
        prepare_projected_reference_effect_request_v2(input, &bindings, reserved),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "safe reference load matches multiple ranked GPU reads"
        ))
    ));
}

#[test]
fn reference_roster_duplicate_occurrence_is_rejected_but_payload_aliases_are_not_reads() {
    let (kernel, _, write, mut loads, _) = roster_join_fixture();
    loads.push(loads[1].clone());
    assert!(matches!(
        ProjectedReferenceInputV2::fixture(
            kernel,
            [2; 32],
            vec![write],
            loads,
            read_placement_sources()
        ),
        Err(
            crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::Incomplete(
                "reference read roster contains a duplicate occurrence"
            )
        )
    ));
    let (kernel, bindings, mut write, loads, reserved) = roster_join_fixture();
    let ProductionSemanticExpressionV2::Binary { lhs, rhs, .. } = write.value.as_mut().unwrap()
    else {
        panic!("add")
    };
    *rhs = lhs.clone();
    let input = roster_join_input(kernel, write, loads);
    prepare_projected_reference_effect_request_v2(input, &bindings, reserved)
        .unwrap_or_else(|e| panic!("{e:?}"));
}

#[test]
fn reference_roster_requires_independent_read_to_dominate_write() {
    let (kernel, bindings, mut write, mut loads, reserved) = roster_join_fixture();
    let mut blocks = kernel.blocks().to_vec();
    let mut operations = blocks[3].operations().to_vec();
    let read = operations.remove(1);
    blocks[3] = ProductionRankedBlockV1::new(operations, blocks[3].terminator().clone());
    blocks[4] = ProductionRankedBlockV1::new(vec![read], blocks[4].terminator().clone());
    loads[1].block = 4;
    loads[1].operation = 0;
    write.operation = 1;
    let sources = [(3, 0), (4, 0), (3, 1)]
        .into_iter()
        .enumerate()
        .map(|(ordinal, (block, operation))| {
            fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1::new(
                11,
                Some(2),
                ordinal as u32,
                block,
                operation,
            )
        })
        .collect();
    let kernel =
        ProductionRankedKernelV1::new(kernel.function_name(), kernel.argument_count(), blocks)
            .unwrap();
    let input = ProjectedReferenceInputV2::fixture(kernel, [2; 32], vec![write], loads, sources)
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert!(matches!(
        prepare_projected_reference_effect_request_v2(input, &bindings, reserved),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "read-root placement requires each original read to dominate the write"
        ))
    ));
}

#[test]
fn reference_roster_cannot_remap_foreign_sources_or_skip_owned_remap() {
    let (kernel, bindings, write, loads, reserved) = roster_join_fixture();
    let input = roster_join_input(kernel, write, loads);
    let mut request = prepare_projected_reference_effect_request_v2(input, &bindings, reserved)
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert!(matches!(
        request.remap_source_sites_v2(&mut read_placement_sources(), &mut []),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference request must remap its owned source roster"
        ))
    ));
    // This returns before opening a runtime or executing a proof.
    assert!(matches!(
        request.prove_and_compile(),
        Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "read-root placement requires its exact source-site roster remap before compilation"
        ))
    ));
}

#[test]
fn reference_roster_f32_cpu_only_volatile_read_keeps_exact_numerical_contract() {
    let (kernel, bindings, mut write, mut loads, reserved) = roster_join_fixture();
    let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
    for load in &mut loads {
        load.scalar = scalar;
        load.read_mode = fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedVolatile;
    }
    write.value = Ok(ProductionSemanticExpressionV2::Binary {
        operation: ProductionSemanticBinaryOpV2::Add,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(ProductionSemanticExpressionV2::Load(loads[0].clone())),
        rhs: Box::new(ProductionSemanticExpressionV2::Constant {
            scalar,
            bits: u64::from(1.0_f32.to_bits()),
        }),
    });
    let mut binding = bindings.as_slice()[0].clone();
    for relation in &mut binding.effect_ir.relations {
        match relation {
            ReferenceArgumentRelationV1::SharedSliceInput { element, .. }
            | ReferenceArgumentRelationV1::DisjointOutputCoordinate { element, .. }
            | ReferenceArgumentRelationV1::InvocationDisjointOutputCoordinate1D {
                element, ..
            } => *element = ReferenceScalarTypeV1::F32,
            _ => {}
        }
    }
    let input = roster_join_input(kernel, write, loads);
    let request = prepare_projected_reference_effect_request_v2(
        input,
        &AuthenticatedReferenceEffectBindingsV1::new(vec![binding]),
        reserved,
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    for index in [2, 3] {
        let ProductionRankedOperationV1::SemanticExpression {
            expression,
            numerical_contract,
            ..
        } = &request.kernel.blocks()[3].operations()[index]
        else {
            panic!("typed expression")
        };
        assert_eq!(
            *numerical_contract,
            ProductionNumericalContractV2::exact_for(scalar)
        );
        let mut retained = Vec::new();
        collect_semantic_loads_v2(expression, &mut retained);
        assert_eq!(retained.len(), if index == 2 { 1 } else { 2 });
        assert!(retained.iter().all(|load| load.scalar == scalar
            && load.read_mode == fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedVolatile));
    }
}
