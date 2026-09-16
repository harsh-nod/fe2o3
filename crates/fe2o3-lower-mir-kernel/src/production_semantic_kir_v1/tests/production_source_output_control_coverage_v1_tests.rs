#[cfg(test)]
mod conditional_memory_control_component_tests_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirBlockCoordinateV1 as Block,
        CanonicalKirDefinitionCoordinateV1 as Definition,
        CanonicalKirEdgeArgumentCoordinateV1 as Argument, CanonicalKirEdgeCoordinateV1 as Edge,
        CanonicalKirFunctionCoordinateV1 as Function,
    };

    fn literal_fixture() -> (
        FunctionBody,
        SourceOutputProjectionLiteralV1,
        SourceOutputControlUseRowV1,
    ) {
        use fe2o3_kernel_ir::{
            CanonicalKirOperationCoordinateV1 as Op, CanonicalKirUseCoordinateV1 as Use,
        };
        let at = |operation| Op {
            block: Block {
                function: Function(0),
                block: 0,
            },
            operation,
        };
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U64)),
                OperationKind::Constant(Constant::U64(17)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U64)),
                OperationKind::Constant(Constant::U64(17)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::INDEX),
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(0),
                    to: Type::INDEX,
                },
            ),
        ];
        let literal = SourceOutputProjectionLiteralV1 {
            block: SemanticBlockIdV1::from_index(0),
            statement: 0,
            original: Definition::Result {
                operation: at(0),
                result: 0,
            },
            value: ValueId(0),
            ssa: SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(0)),
            bits: 17,
            first_use: 0,
            end_use: 1,
        };
        let identity = SourceOutputControlUseIdentityV1 {
            coordinate: Use::OperationOperand {
                operation: at(3),
                operand: 0,
            },
            value: ValueId(2),
            definition: Definition::Result {
                operation: at(2),
                result: 0,
            },
        };
        (
            FunctionBody {
                parameters: vec![],
                blocks: vec![block],
            },
            literal,
            SourceOutputControlUseRowV1 {
                input: identity,
                output: Some(identity),
            },
        )
    }

    fn literal_types() -> Vec<SemanticTypeDeclV1> {
        use fe2o3_mir_model::semantic_mir_v1::{
            SemanticLayoutIdentityV1, SemanticScalarValidityRangeV1, SemanticTypeIdentityV1,
            SemanticTypeLayoutV1,
        };
        let primitive = SemanticBackendPrimitiveV1::integer(false, 64, 8);
        vec![SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    primitive,
                    SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        )]
    }

    #[test]
    fn literal_typed_interpretation_and_exact_component_work_preserve_history() {
        use fe2o3_mir_model::semantic_mir_v1::SemanticConstantV1;
        const HISTORY: usize = 7;
        // Not construction or frame admission: interpretation8 + direct
        // ancestry(1+4) + use row5, independently derived from these bodies.
        const QUERY: usize = 18;
        let types = literal_types();
        let ty = SemanticTypeIdV1::from_index(0);
        let constant = SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(17, 8).unwrap()),
        );
        let (body, literal, row) = literal_fixture();
        for exact in [true, false] {
            let mut work = Work::new(HISTORY + QUERY - usize::from(!exact));
            let mut budget = AssertOriginBudgetV1::new(&mut work, 17);
            budget.reserve_storage(17).unwrap();
            budget.charge_work(HISTORY).unwrap();
            let result =
                source_output_control_literal_constant_v1(&types, ty, &constant, &mut budget)
                    .and_then(|(_, _, scalar, bits)| {
                        assert_eq!(bits, 17);
                        source_output_control_literal_ancestry_v1(
                            &body,
                            Function(0),
                            literal.value,
                            literal,
                            scalar,
                            &mut budget,
                        )?;
                        source_output_control_literal_row_v1(
                            row,
                            row.input.coordinate,
                            row.input.value,
                            &mut budget,
                        )
                    });
            assert_eq!(result.is_ok(), exact);
            assert_eq!(budget.work(), if exact { 25 } else { 20 });
            assert_eq!(budget.storage(), 17);
            assert_eq!(budget.peak_storage(), 17);
            budget.release_storage(17).unwrap();
            assert_eq!(work.failed_work(), (!exact).then_some(25));
        }
        for (value, size, supplied_type) in
            [(17, 4, 0), (17, 8, 1), (u128::from(u64::MAX) + 1, 16, 0)]
        {
            let mut work = Work::new(8);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
            let constant = SemanticConstantV1::new(
                SemanticTypeIdV1::from_index(supplied_type),
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, size).unwrap()),
            );
            assert!(matches!(
                source_output_control_literal_constant_v1(&types, ty, &constant, &mut budget),
                Err(ProductionSourceOutputErrorV1::Invalid(_))
            ));
            assert_eq!(budget.work(), 8);
        }
    }

    #[test]
    fn literal_transport_rejects_equal_bits_wrong_definition_type_and_result_slot() {
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 64,
        };
        let (body, literal, _) = literal_fixture();
        let mut work = Work::new(10_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
        source_output_control_literal_ancestry_v1(
            &body,
            Function(0),
            ValueId(2),
            literal,
            scalar,
            &mut budget,
        )
        .unwrap();
        for fault in 0..7 {
            let mut changed = literal;
            let mut changed_body = body.clone();
            let mut start = ValueId(2);
            match fault {
                0 => changed.bits = 19,
                1 => changed.value = ValueId(1),
                2 => {
                    if let Definition::Result { operation, .. } = &mut changed.original {
                        operation.operation = 1;
                    }
                }
                3 => {
                    if let Definition::Result { result, .. } = &mut changed.original {
                        *result = 1;
                    }
                }
                4 => {
                    changed_body.blocks[0].operations[0].results[0].ty =
                        Type::Scalar(ScalarType::U32)
                }
                5 => {
                    if let OperationKind::Cast { to, .. } =
                        &mut changed_body.blocks[0].operations[2].kind
                    {
                        *to = Type::Scalar(ScalarType::U32);
                    }
                }
                _ => start = ValueId(1),
            }
            assert!(
                source_output_control_literal_ancestry_v1(
                    &changed_body,
                    Function(0),
                    start,
                    changed,
                    scalar,
                    &mut budget
                )
                .is_err(),
                "fault {fault}"
            );
        }
    }

    #[test]
    fn literal_eliminated_compare_and_wrong_operand_definition_are_not_fabricated() {
        use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as Use;
        let (_, _, row) = literal_fixture();
        for fault in 0..6 {
            let mut changed = row;
            match fault {
                0 => changed.output = None,
                1 => changed.input.value = ValueId(99),
                2 => {
                    if let Use::OperationOperand { operand, .. } = &mut changed.input.coordinate {
                        *operand = 1;
                    }
                }
                3 => {
                    if let Definition::Result { result, .. } = &mut changed.input.definition {
                        *result = 1;
                    }
                }
                4 => {
                    if let Use::OperationOperand { operand, .. } =
                        &mut changed.output.as_mut().unwrap().coordinate
                    {
                        *operand = 1;
                    }
                }
                _ => {
                    if let Definition::Result { result, .. } =
                        &mut changed.output.as_mut().unwrap().definition
                    {
                        *result = 1;
                    }
                }
            }
            let mut work = Work::new(5);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
            assert!(
                source_output_control_literal_row_v1(
                    changed,
                    row.input.coordinate,
                    row.input.value,
                    &mut budget
                )
                .is_err(),
                "fault {fault}"
            );
            assert_eq!(budget.work(), 5);
        }
    }

    #[test]
    fn literal_guard_payload_exact_storage_late_denial_and_unwind_restore_floor() {
        const FLOOR: usize = 17;
        const HEADER: usize = std::mem::size_of::<Vec<SourceOutputProjectionLiteralUseV1>>();
        const PAYLOAD: usize = 4 * std::mem::size_of::<SourceOutputProjectionLiteralUseV1>();
        let (_, _, row) = literal_fixture();
        let used = SourceOutputProjectionLiteralUseV1 {
            guard: SemanticBlockIdV1::from_index(0),
            input: row.input,
            output: row.output.unwrap(),
        };
        for (work_short, storage_short, callback_error, panic) in [
            (false, false, false, false),
            (true, false, false, false),
            (false, true, false, false),
            (false, false, true, false),
            (false, false, false, true),
        ] {
            // First push: relocation0+reserve1+push1. The following work1 is
            // the independent late denial point, after retaining actual rows.
            let mut work = Work::new(7 + 3 - usize::from(work_short));
            let mut budget = AssertOriginBudgetV1::new(
                &mut work,
                FLOOR + HEADER + PAYLOAD - usize::from(storage_short),
            );
            budget.reserve_storage(FLOOR).unwrap();
            budget.charge_work(7).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source_output_global_scratch_scope_v1(&mut budget, |budget| {
                    budget
                        .reserve_storage(HEADER)
                        .map_err(ProductionSourceOutputErrorV1::Resource)?;
                    let mut uses = Vec::new();
                    assert_origin_push_v1(&mut uses, used, budget)
                        .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
                    assert_eq!(uses.capacity(), 4);
                    budget
                        .charge_work(1)
                        .map_err(ProductionSourceOutputErrorV1::Resource)?;
                    if panic {
                        panic!("literal guard callback");
                    }
                    if callback_error {
                        return Err(ProductionSourceOutputErrorV1::Invalid(
                            "literal guard callback",
                        ));
                    }
                    Ok(())
                })
            }));
            assert_eq!(result.is_err(), panic);
            if let Ok(result) = result {
                assert_eq!(
                    result.is_ok(),
                    !work_short && !storage_short && !callback_error
                );
            }
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(
                budget.work(),
                if storage_short {
                    8
                } else if work_short {
                    9
                } else {
                    10
                }
            );
            budget.release_storage(FLOOR).unwrap();
            assert_eq!(work.failed_work(), work_short.then_some(10));
        }
    }

    fn rows() -> SourceOutputCheckedControlRowsV1 {
        let source = Block {
            function: Function(0),
            block: 0,
        };
        let target = Block {
            function: Function(0),
            block: 1,
        };
        let mut rows = SourceOutputCheckedControlRowsV1 {
            compare_uses: Vec::new(),
            edges: Vec::new(),
            uses: Vec::new(),
            arguments: Vec::new(),
        };
        for successor in 0..2 {
            let edge = Edge { source, successor };
            let identity = SourceOutputControlEdgeIdentityV1 {
                coordinate: edge,
                target,
                selection: SourceOutputControlSelectionV1::Boolean {
                    value: ValueId(7),
                    expected: successor == 0,
                },
                argument_count: 1,
            };
            rows.edges.push(SourceOutputControlEdgeRowV1 {
                input: identity.clone(),
                output: Some(identity),
                checked: fe2o3_kernel_analysis::CanonicalKirEdgeControlV1 {
                    placement: fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Retained(edge),
                    executable: true,
                },
            });
            let identity = SourceOutputControlArgumentIdentityV1 {
                coordinate: Argument { edge, argument: 0 },
                value: ValueId(successor),
                incoming: Definition::FunctionArgument {
                    function: Function(0),
                    argument: successor,
                },
                target: Definition::BlockArgument {
                    block: target,
                    argument: 0,
                },
                target_value: ValueId(9),
            };
            rows.arguments.push(SourceOutputControlArgumentRowV1 {
                input: identity,
                output: Some(identity),
            });
        }
        rows
    }

    #[test]
    fn duplicate_targets_do_not_collapse_polarity_or_incoming_argument_occurrences() {
        let rows = rows();
        let mut work = Work::new(100);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 17);
        budget.reserve_storage(17).unwrap();
        for successor in 0..2 {
            let found = source_output_control_edge_v1(
                &rows,
                rows.edges[successor].input.coordinate,
                &mut budget,
            )
            .unwrap();
            assert_eq!(found.input.target, rows.edges[1 - successor].input.target);
            assert_ne!(
                found.input.coordinate,
                rows.edges[1 - successor].input.coordinate
            );
            assert_ne!(
                found.input.selection,
                rows.edges[1 - successor].input.selection
            );
            source_output_control_argument_rows_v1(&rows, found, &mut budget).unwrap();
        }
        assert_ne!(
            rows.arguments[0].input.incoming,
            rows.arguments[1].input.incoming
        );
        assert_eq!(budget.storage(), 17);
        budget.release_storage(17).unwrap();
    }

    #[test]
    fn wrong_edge_argument_occurrence_and_omission_are_rejected() {
        for fault in 0..3 {
            let mut rows = rows();
            match fault {
                0 => {
                    rows.arguments[0]
                        .output
                        .as_mut()
                        .unwrap()
                        .coordinate
                        .edge
                        .successor = 1
                }
                1 => {
                    rows.arguments[0]
                        .output
                        .as_mut()
                        .unwrap()
                        .coordinate
                        .argument = 1
                }
                _ => rows.arguments[0].output = None,
            }
            let mut work = Work::new(100);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
            assert!(matches!(
                source_output_control_argument_rows_v1(&rows, &rows.edges[0], &mut budget),
                Err(ProductionSourceOutputErrorV1::Invalid(_))
            ));
        }
    }

    #[test]
    fn edge_lookup_component_exact_boundary_keeps_prior_work_and_storage_floor() {
        // Component only, not descriptor construction or the whole frame:
        // edge0: two (find1 + compare3) = 8;
        // argument0: outer1 + two(find1+compare4) + final3 = 14.
        const QUERY: usize = 22;
        const HISTORY: usize = 7;
        let rows = rows();
        for exact in [true, false] {
            let mut work = Work::new(HISTORY + QUERY - usize::from(!exact));
            let mut budget = AssertOriginBudgetV1::new(&mut work, 17);
            budget.reserve_storage(17).unwrap();
            budget.charge_work(HISTORY).unwrap();
            let result =
                source_output_control_edge_v1(&rows, rows.edges[0].input.coordinate, &mut budget)
                    .and_then(|edge| {
                        source_output_control_argument_rows_v1(&rows, edge, &mut budget)
                    });
            assert_eq!(result.is_ok(), exact);
            assert_eq!(budget.storage(), 17);
            assert_eq!(budget.peak_storage(), 17);
            assert_eq!(budget.work(), if exact { 29 } else { 26 });
            budget.release_storage(17).unwrap();
            assert_eq!(work.failed_work(), if exact { None } else { Some(29) });
        }
    }

    #[test]
    fn one_numeric_row_allocation_has_independent_minimum_capacity_and_work_boundaries() {
        // This component excludes borrowed fixture construction, inventories,
        // and the whole control query. The shared reserve helper requests four
        // rows for an empty vector and prepays relocation0+reserve1, then push1.
        const HISTORY: usize = 7;
        const FLOOR: usize = 17;
        const HEADER: usize = std::mem::size_of::<Vec<SourceOutputControlEdgeRowV1>>();
        const PAYLOAD: usize = 4 * std::mem::size_of::<SourceOutputControlEdgeRowV1>();
        let fixture = rows();
        for (work_short, storage_short) in [(false, false), (true, false), (false, true)] {
            let mut work = Work::new(HISTORY + 2 - usize::from(work_short));
            let mut budget = AssertOriginBudgetV1::new(
                &mut work,
                FLOOR + HEADER + PAYLOAD - usize::from(storage_short),
            );
            budget.reserve_storage(FLOOR).unwrap();
            budget.charge_work(HISTORY).unwrap();
            let mut published = false;
            let result = source_output_global_scratch_scope_v1(&mut budget, |budget| {
                budget
                    .reserve_storage(HEADER)
                    .map_err(ProductionSourceOutputErrorV1::Resource)?;
                let mut rows = Vec::new();
                assert_origin_push_v1(&mut rows, fixture.edges[0].clone(), budget)
                    .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
                // This positive also qualifies the requested exact capacity on
                // this allocator; excess is reconciled by the production helper.
                assert_eq!(rows.capacity(), 4);
                assert_eq!(rows.len(), 1);
                published = true;
                Ok(())
            });
            assert_eq!(published, !work_short && !storage_short);
            assert_eq!(result.is_ok(), published);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.work(), HISTORY + if published { 2 } else { 1 });
            assert_eq!(
                budget.peak_storage(),
                FLOOR + HEADER + if storage_short { 0 } else { PAYLOAD }
            );
            assert_eq!(
                budget.failed_storage(),
                storage_short.then_some(FLOOR + HEADER + PAYLOAD)
            );
            budget.release_storage(FLOOR).unwrap();
            assert_eq!(work.failed_work(), work_short.then_some(HISTORY + 2));
        }
    }

    #[test]
    fn numeric_row_scope_drops_payload_before_floor_restore_on_error_and_panic() {
        let fixture = rows();
        let mut work = Work::new(10_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 4096);
        budget.reserve_storage(17).unwrap();
        for panic in [false, true] {
            let before = budget.work();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source_output_global_scratch_scope_v1(&mut budget, |budget| {
                    budget
                        .reserve_storage(std::mem::size_of::<Vec<SourceOutputControlEdgeRowV1>>())
                        .map_err(ProductionSourceOutputErrorV1::Resource)?;
                    let mut copied = Vec::new();
                    assert_origin_push_v1(&mut copied, fixture.edges[0].clone(), budget)
                        .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
                    if panic {
                        panic!("component callback failure");
                    }
                    Err::<(), _>(ProductionSourceOutputErrorV1::Invalid(
                        "component callback failure",
                    ))
                })
            }));
            assert_eq!(result.is_err(), panic);
            if let Ok(result) = result {
                assert!(result.is_err());
            }
            assert_eq!(budget.storage(), 17);
            assert!(budget.work() > before);
            budget.charge_work(1).unwrap();
        }
        budget.release_storage(17).unwrap();
    }
}
