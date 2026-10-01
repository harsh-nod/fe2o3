use super::*;

#[path = "production_source_call_return_v32_tests.rs"]
mod call_return;

#[path = "production_source_scalar_boundary_forwarding_v32_tests.rs"]
mod forwarding;
#[path = "production_source_scalar_boundary_incoming_v32_tests.rs"]
mod incoming;

#[path = "production_source_scalar_boundary_function_scope_v31_tests.rs"]
mod function_scope;
#[path = "production_source_issued_presence_v31_tests.rs"]
mod issued_presence;

#[path = "production_source_scalar_boundary_normalization_v31_tests.rs"]
mod normalization;

fn rebuild_root(
    old: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        locals,
        old.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(old.kernel_entry().unwrap().clone())
}

fn loop_owner(entry_loop: bool, parallel: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = private_entry_phi_owner_v20();
    let source = base.source_semantic();
    let jump = |target| {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(target),
        ))
    };
    let switch = |variable, yes, no| SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(variable, U32)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(yes),
                ),
            )],
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                SemanticBlockIdV1::from_index(no),
            ),
        )
        .unwrap(),
    };
    let invoke = |variable, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(2),
                vec![SemanticOperandV1::Copy(place(variable, U32))],
                Some(SemanticCallDestinationV1::new(
                    place(0, UNIT),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let blocks = if entry_loop {
        vec![
            block(180, vec![], switch(1, 2, 1)),
            block(
                181,
                vec![assign(place(1, U32), SemanticRvalueKindV1::Use(literal(0)))],
                if parallel { switch(1, 0, 0) } else { jump(0) },
            ),
            block(182, vec![], invoke(1, 3)),
            block(183, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![
            block(
                190,
                vec![assign(
                    place(2, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32))),
                )],
                jump(1),
            ),
            block(191, vec![], switch(2, 3, 2)),
            block(
                192,
                vec![assign(
                    place(2, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Subtract,
                        left: SemanticOperandV1::Copy(place(2, U32)),
                        right: literal(1),
                    },
                )],
                jump(1),
            ),
            block(193, vec![], invoke(2, 4)),
            block(194, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    let old = &source.functions()[0];
    let root = rebuild_root(old, old.locals().to_vec(), blocks);
    let mut functions = source.functions().to_vec();
    functions[0] = root;
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}
fn header_loop() -> ProductionSemanticSsaOwnerV1 {
    loop_owner(false, false)
}
fn entry_loop() -> ProductionSemanticSsaOwnerV1 {
    loop_owner(true, false)
}
fn parallel_entry_loop() -> ProductionSemanticSsaOwnerV1 {
    loop_owner(true, true)
}

fn two_phi_roots() -> ProductionSemanticSsaOwnerV1 {
    let base = private_entry_phi_owner_v20();
    let source = base.source_semantic();
    let first = &source.functions()[0];
    let second = &source.functions()[1];
    let mut functions = source.functions().to_vec();
    functions[1] = rebuild_root(
        second,
        first
            .locals()
            .iter()
            .enumerate()
            .map(|(at, row)| local(221 + u8::try_from(at).unwrap(), row.ty(), row.role()))
            .collect(),
        first
            .blocks()
            .iter()
            .enumerate()
            .map(|(at, row)| {
                block(
                    230 + u8::try_from(at).unwrap(),
                    row.statements().to_vec(),
                    row.terminator().kind().clone(),
                )
            })
            .collect(),
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn comparison_loop() -> ProductionSemanticSsaOwnerV1 {
    let base = header_loop();
    let source = base.source_semantic();
    let mut types = source.types().to_vec();
    let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([243; 32]),
        SemanticLayoutIdentityV1::from_sha256([243; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    ));
    let old = &source.functions()[0];
    let mut locals = old.locals().to_vec();
    locals.push(local(210, boolean, SemanticLocalRoleV1::Temporary));
    let mut blocks = old.blocks().to_vec();
    let replacement = block(
        191,
        vec![assign(
            place(3, boolean),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: literal(0),
                right: SemanticOperandV1::Copy(place(2, U32)),
            },
        )],
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(place(3, boolean)),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchValue,
                        SemanticBlockIdV1::from_index(3),
                    ),
                )],
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchOtherwise,
                    SemanticBlockIdV1::from_index(2),
                ),
            )
            .unwrap(),
        },
    );
    assert_eq!(replacement.identity(), blocks[1].identity());
    blocks[1] = replacement;
    let mut functions = source.functions().to_vec();
    functions[0] = rebuild_root(old, locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn with_policy11(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> ProductionOptimizerTestResultV18 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, _) = integer_handoff_prepared_v18(factory, &mut budget);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let (output, (), receipt) = source.with_checked_mixed_fixedpoint_optimization_v18(
                budget,
                |original, optimized, budget| {
                    consume(original, optimized, budget)?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                },
            )?;
            assert_eq!(
                receipt.retained_storage(),
                size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>()
            );
            assert!(!output.grants_authority());
            drop(output);
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
    }));
    assert_eq!(budget.storage(), MODULE_FLOOR);
    match caught {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn with_check<'work>(
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl for<'scope> FnOnce(
        &SourceBoundaryCheckV31<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    let original = leaves.original.leaves;
    let relation = original.relation;
    source_scalar_normalization_scratch_v18(
        relation.source.cleanup,
        budget,
        source_boundary_control_headers_v31()?,
        |budget| {
            let index = OriginalEntryIndexV20::build(relation, budget)?;
            let arguments = SourceRootArgumentsV18::build(relation, original.root, budget)?;
            let inline = Gfx942InlineScalarCorrespondenceV30::build_source_v18(
                relation,
                original.root,
                budget,
            )?;
            let transported = inline.transport_optimized_source_v18(
                relation,
                leaves.optimized,
                original.root,
                budget,
            )?;
            let inventory = leaves.optimized.output_inventory(budget)?;
            let result = value_origin_v1::with_optimized_whole_value_origins_v18(
                relation,
                leaves.optimized,
                inventory,
                leaves.function.coordinate,
                budget,
                |origins, budget| {
                    let check = SourceBoundaryCheckV31 {
                        leaves: original,
                        index: &index,
                        arguments: &arguments,
                        origins,
                        inline: &transported,
                        optimized: Some(leaves),
                        inventory,
                        bindings: leaves.function.edge_arguments.clone(),
                    };
                    consume(&check, budget)
                },
            );
            drop((transported, inline, arguments, index));
            relation.retain_query(result)
        },
    )
}

fn assert_binding(result: ProductionOptimizerTestResultV18, expected: &'static str) {
    assert!(
        matches!(result, Err(ProductionSourceOptimizationErrorV18::Source(
        ProductionSourceOwnedViewErrorV18::Binding(actual))) if actual == expected),
        "expected exact source binding {expected:?}, got {result:?}"
    );
}

#[test]
fn source_scalar_boundaries_check_diamond_header_loop_entry_recurrence_and_parallel_edges() {
    for factory in [
        private_entry_phi_owner_v20 as fn() -> _,
        header_loop,
        comparison_loop,
        entry_loop,
        parallel_entry_loop,
    ] {
        let reached = std::cell::Cell::new(false);
        with_policy11(factory, |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    assert!(!leaves.original.leaves.boundaries.rows.is_empty());
                    assert_eq!(
                        leaves.boundaries.definitions.len(),
                        leaves.original.leaves.boundaries.rows.len()
                    );
                    leaves
                        .original
                        .leaves
                        .check_boundary_equations_v31(budget)?;
                    leaves
                        .original
                        .leaves
                        .check_boundary_actual_v31(Some(leaves), budget)?;
                    reached.set(true);
                    Ok(())
                },
            )
        })
        .unwrap();
        assert!(reached.get());
    }
}

#[test]
fn source_scalar_boundaries_keep_repeated_value_ids_in_distinct_roots_separate() {
    let reached = std::cell::Cell::new(0);
    with_policy11(two_phi_roots, |original, optimized, budget| {
        for root in 0..2 {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                root,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    assert!(!leaves.boundaries.values.is_empty());
                    with_check(leaves, budget, |check, budget| {
                        for row in &leaves.boundaries.values {
                            let foreign = check
                                .inventory
                                .definitions()
                                .iter()
                                .enumerate()
                                .find(|(_, definition)| {
                                    definition.value == Some(row.value)
                                        && matches!(definition.coordinate,
                                            fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, .. }
                                            if block.function != leaves.function.coordinate)
                                })
                                .expect("the second root repeats the actual phi ValueId");
                            assert!(check.boundary_target(row.definition, budget)?);
                            assert!(!check.boundary_target(foreign.0, budget)?);
                        }
                        Ok(())
                    })?;
                    leaves
                        .original
                        .leaves
                        .check_boundary_actual_v31(Some(leaves), budget)?;
                    reached.set(reached.get() + 1);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(reached.get(), 2);
}

#[test]
fn source_scalar_boundaries_reject_a_different_predecessor_value_and_repeated_edge() {
    for duplicate in [false, true] {
        let result = with_policy11(
            private_entry_phi_owner_v20,
            |original, optimized, budget| {
                original.with_optimized_scalar_leaf_namespace_v18(
                    optimized,
                    0,
                    &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                    budget,
                    |leaves, budget| {
                        with_check(leaves, budget, |check, budget| {
                            let row = check
                                .leaves
                                .boundary_find_v31([0, 0, 3, 2], budget)?
                                .unwrap();
                            let owner = original.source.source_ssa(budget)?;
                            let plan = owner
                                .plan_for_function(SemanticFunctionIdV1::from_index(0))
                                .unwrap()
                                .plan();
                            let incoming = |block| {
                                plan.edge_arguments(BoundaryEdgeV31::new(
                                    BoundaryBlockV31::new(block),
                                    0,
                                ))
                                .unwrap()
                                .iter()
                                .find(|argument| argument.variable() == row.variable)
                                .unwrap()
                                .value()
                            };
                            let controls = source_reference_selection_control_index_v30(
                                &original.source.root_row(0)?.coordinates.controls.rows,
                                budget,
                            )
                            .map_err(source_emission_error_v18)?;
                            let control = controls
                                .iter()
                                .find(|row| row.instance.index() == 0 && row.source.index() == 1)
                                .unwrap();
                            let function = original.source.root_row(0)?.function_ordinal;
                            let physical = original
                                .inventory
                                .block_for_id(
                                    fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                                        function as u32,
                                    ),
                                    control.terminal,
                                    budget,
                                )
                                .map_err(source_pointer_inventory_error_v18)?
                                .unwrap();
                            let actual = check.block(physical.coordinate, budget)?;
                            let mut seen =
                                emission_vec_v1(check.inventory.edge_arguments().len(), budget)
                                    .map_err(source_emission_error_v18)?;
                            seen.resize(check.inventory.edge_arguments().len(), false);
                            check.edge(actual, 0, row, incoming(1), &mut seen, budget)?;
                            if !duplicate {
                                seen.fill(false);
                            }
                            let result = check.edge(
                                actual,
                                0,
                                row,
                                incoming(if duplicate { 1 } else { 2 }),
                                &mut seen,
                                budget,
                            );
                            drop((seen, controls));
                            result
                        })
                    },
                )
            },
        );
        assert_binding(
            result,
            if duplicate {
                "source SSA physical argument binding differs or repeats"
            } else {
                "actual optimized scalar expression differs from its original source value"
            },
        );
        with_policy11(
            private_entry_phi_owner_v20,
            |original, optimized, budget| {
                original.with_optimized_scalar_leaf_namespace_v18(
                    optimized,
                    0,
                    &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                    budget,
                    |_, _| Ok(()),
                )
            },
        )
        .unwrap();
    }
}

#[test]
fn source_scalar_boundaries_reject_original_type_name_and_definition_substitution() {
    for fault in 0..4 {
        let result = with_policy11(private_entry_phi_owner_v20, |original, _, budget| {
            source_scalar_normalization_scratch_v18(
                original.source.cleanup,
                budget,
                source_boundary_headers_v31()?,
                |budget| {
                    let mut leaves = SourceScalarLeavesV18::build(
                        original,
                        0,
                        &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                        budget,
                    )?;
                    let row = *leaves.boundary_find_v31([0, 0, 3, 2], budget)?.unwrap();
                    let result = if fault == 0 {
                        ProductionSourceScalarLeavesV18 { leaves: &leaves }
                            .boundary_expression_v31(
                                row.instance,
                                row.block,
                                row.variable,
                                UNIT,
                                row.scalar,
                                budget,
                            )
                            .map(|_| ())
                    } else {
                        let at = leaves
                            .boundaries
                            .rows
                            .iter()
                            .position(|entry| entry.definition == row.definition)
                            .unwrap();
                        match fault {
                            1 => leaves.boundaries.rows[at].value = ValueId(u32::MAX),
                            2 => leaves.boundaries.rows[at].symbol += 1,
                            _ => {
                                leaves.boundaries.rows[at].scalar =
                                    ProductionSemanticScalarTypeV2::Bool
                            }
                        }
                        leaves
                            .boundary_find_v31(
                                if fault == 2 {
                                    [2, row.symbol as usize, 0, 0]
                                } else {
                                    [0, 0, 3, 2]
                                },
                                budget,
                            )
                            .map(|_| ())
                    };
                    drop(leaves);
                    result
                },
            )
        });
        assert_binding(
            result,
            if fault == 0 {
                "source scalar boundary original type differs"
            } else {
                "source scalar boundary lookup changed its exact binding"
            },
        );
    }
}

#[test]
fn source_scalar_boundaries_reject_missing_optimized_parameter_and_control_transport() {
    for fault in 0..4 {
        let result = with_policy11(
            if fault == 3 {
                two_phi_roots
            } else {
                private_entry_phi_owner_v20
            },
            |original, optimized, budget| {
                original.with_optimized_scalar_leaf_namespace_v18(
                    optimized,
                    0,
                    &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                    budget,
                    |leaves, budget| {
                        source_scalar_normalization_scratch_v18(
                            original.source.cleanup,
                            budget,
                            optimized_source_boundary_headers_v31()?,
                            |budget| {
                                let mut boundaries = optimized_source_scalar_boundaries_v31(
                                    leaves.original,
                                    optimized,
                                    budget,
                                )?;
                                match fault {
                                    0 => boundaries.definitions.fill(usize::MAX),
                                    1 => boundaries.entries.fill(None),
                                    2 => boundaries.terminals.fill(None),
                                    _ => {
                                        let value = boundaries.values[0].value;
                                        let inventory = optimized.output_inventory(budget)?;
                                        boundaries.definitions[boundaries.values[0].original] = inventory
                                            .definitions()
                                            .iter()
                                            .position(|definition| {
                                                definition.value == Some(value)
                                                    && matches!(definition.coordinate,
                                                        fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, .. }
                                                        if block.function != leaves.function.coordinate)
                                            })
                                            .expect("the other root repeats this actual phi ValueId");
                                    }
                                }
                                let altered = ProductionOptimizedSourceScalarLeavesV18 {
                                    original: leaves.original,
                                    optimized,
                                    function: leaves.function,
                                    reads: leaves.reads,
                                    wrapping: leaves.wrapping,
                                    boundaries: &boundaries,
                                    presences: leaves.presences,
                                    lengths: leaves.lengths,
                                    slot: leaves.slot,
                                    ledger: leaves.ledger,
                                    floor: leaves.floor,
                                };
                                let result = leaves
                                    .original
                                    .leaves
                                    .check_boundary_actual_v31(Some(&altered), budget);
                                drop(altered);
                                drop(boundaries);
                                result
                            },
                        )
                    },
                )
            },
        );
        assert_binding(
            result,
            if fault == 0 || fault == 3 {
                "source SSA optimized boundary definition is absent"
            } else {
                "source SSA optimized boundary control was erased"
            },
        );
        with_policy11(
            private_entry_phi_owner_v20,
            |original, optimized, budget| {
                original.with_optimized_scalar_leaf_namespace_v18(
                    optimized,
                    0,
                    &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                    budget,
                    |_, _| Ok(()),
                )
            },
        )
        .unwrap();
    }
}

#[test]
fn source_scalar_boundaries_unwind_disposes_the_complete_original_and_output_scopes() {
    let entered = std::cell::Cell::new(false);
    let dropped = std::cell::Cell::new(0);
    struct Tracked<'a>(&'a std::cell::Cell<usize>);
    impl Drop for Tracked<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_policy11(entry_loop, |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |_, _| {
                    let _owned = Tracked(&dropped);
                    entered.set(true);
                    panic!("selected source scalar boundary unwind")
                },
            )
        })
    }));
    assert!(entered.get());
    assert_eq!(dropped.get(), 1);
    let result = match caught {
        Ok(result) => result,
        Err(_) => panic!("bounded optimizer adoption must retain its typed panic refusal"),
    };
    assert_binding(result, "actual source optimizer adoption rejected");
}

#[test]
fn source_scalar_boundaries_reject_a_branch_selector_from_the_phi_result() {
    let result = with_policy11(
        private_entry_phi_owner_v20,
        |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    with_check(leaves, budget, |check, budget| {
                        let owner = original.source.source_semantic(budget)?;
                        let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } =
                            owner.functions()[0].blocks()[0].terminator().kind()
                        else {
                            unreachable!()
                        };
                        let row = check
                            .leaves
                            .boundary_find_v31([0, 0, 3, 2], budget)?
                            .unwrap();
                        let definition = check.definition(row, budget)?;
                        let wrong = check.inventory.definitions()[definition].value.unwrap();
                        check.selector(
                            0,
                            SemanticBlockIdV1::from_index(0),
                            discriminant,
                            wrong,
                            budget,
                        )
                    })
                },
            )
        },
    );
    assert_binding(
        result,
        "actual optimized scalar expression differs from its original source value",
    );
}

fn boundary_cut(cut: Option<(bool, usize)>) -> (usize, usize, bool) {
    let observed = std::cell::Cell::new(None);
    let result = with_policy11(entry_loop, |original, optimized, budget| {
        let floor = budget.storage();
        let result =
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
                let remaining = match cut {
                    Some((false, amount)) => amount,
                    _ => MODULE_LIMIT / 2,
                };
                budget.reserve_storage(MODULE_LIMIT - budget.storage() - remaining)?;
                if let Some((true, amount)) = cut {
                    budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - amount)?;
                }
                let before = (budget.work(), budget.storage());
                let entered = std::cell::Cell::new(false);
                let result = original.with_optimized_scalar_leaf_namespace_v18(
                    optimized,
                    0,
                    &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                    budget,
                    |_, _| {
                        entered.set(true);
                        Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected scalar boundary stop",
                        ))
                    },
                );
                observed.set(Some((
                    budget.work() - before.0,
                    budget.peak_storage() - before.1,
                    entered.get(),
                )));
                match (&result, entered.get(), cut) {
                    (
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected scalar boundary stop",
                        )),
                        true,
                        _,
                    ) => {}
                    (
                        Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                            refusal,
                        ))),
                        false,
                        Some((true, _)),
                    ) => assert!(refusal.actual() > refusal.limit()),
                    (
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Storage(refusal),
                        )),
                        false,
                        Some((false, _)),
                    ) => assert!(refusal.actual() > refusal.limit()),
                    _ => panic!("unexpected exact scalar boundary result: {result:?}"),
                }
                result
            });
        assert_eq!(budget.storage(), floor);
        result
    });
    assert!(result.is_err());
    observed
        .get()
        .expect("the complete scalar scope was attempted")
}

#[test]
fn source_scalar_boundaries_exact_and_one_short_work_and_peak_storage_are_cumulative() {
    let (work, storage, entered) = boundary_cut(None);
    assert!(entered && work > 0 && storage > 0);
    for (kind, exact) in [(true, work), (false, storage)] {
        assert!(boundary_cut(Some((kind, exact))).2);
        assert!(!boundary_cut(Some((kind, exact - 1))).2);
    }
}

#[test]
fn source_scalar_boundaries_fixed_headers_match_independent_field_envelopes() {
    type Row = (
        usize,
        BoundaryBlockV31,
        BoundaryVariableV31,
        SemanticTypeIdV1,
        ProductionSemanticScalarTypeV2,
        usize,
        ValueId,
        u32,
    );
    type Lookup = ([usize; 4], usize);
    type Owner = (
        Vec<SourceScalarBoundaryV31>,
        Vec<SourceScalarBoundaryLookupV31>,
    );
    type Frame<'a> = (
        Owner,
        Row,
        Lookup,
        Vec<Row>,
        Vec<Lookup>,
        Result<Vec<Row>, ProductionSemanticKirErrorV1>,
        Result<Vec<Lookup>, ProductionSemanticKirErrorV1>,
        SourceOwnedResultV18<Owner>,
        SourceOwnedResultV18<Option<&'a Row>>,
        SourceOwnedResultV18<ProductionSemanticExpressionV2>,
        SourceOwnedResultV18<Option<usize>>,
        [usize; 8],
        [usize; 4],
        &'a (),
        &'a (),
        &'a (),
        Type,
    );
    assert_eq!(size_of::<Row>(), size_of::<SourceScalarBoundaryV31>());
    assert_eq!(
        size_of::<Lookup>(),
        size_of::<SourceScalarBoundaryLookupV31>()
    );
    assert_eq!(size_of::<Owner>(), size_of::<SourceScalarBoundariesV31>());
    assert_eq!(
        source_boundary_headers_v31().unwrap(),
        size_of::<Frame<'_>>() + std::mem::align_of::<Frame<'_>>()
    );
    type OutputRow = (ValueId, usize, usize);
    type OutputOwner = (
        Vec<OptimizedSourceScalarBoundaryV31>,
        Vec<usize>,
        Vec<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>>,
        Vec<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>>,
    );
    type OutputFrame<'a> = (
        OutputOwner,
        OutputRow,
        Vec<OutputRow>,
        Vec<usize>,
        [Vec<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>>; 2],
        Result<Vec<OutputRow>, ProductionSemanticKirErrorV1>,
        Result<Vec<usize>, ProductionSemanticKirErrorV1>,
        [Result<
            Vec<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>>,
            ProductionSemanticKirErrorV1,
        >; 2],
        SourceOwnedResultV18<OutputOwner>,
        SourceOwnedResultV18<Option<&'a Row>>,
        ProductionOptimizedSourceCfgRootV18<'a, 'a>,
        SourceOwnedResultV18<ProductionOptimizedSourceCfgRootV18<'a, 'a>>,
        (
            &'a ProductionSourceOwnedViewV18<'a>,
            &'a mut ArgumentBudgetV1<'a>,
        ),
        Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>,
        &'a Row,
        &'a [fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1],
        [usize; 8],
        [SourceOwnedResultV18<()>; 2],
    );
    assert_eq!(
        size_of::<OutputOwner>(),
        size_of::<OptimizedSourceScalarBoundariesV31>()
    );
    assert_eq!(
        size_of::<OutputRow>(),
        size_of::<OptimizedSourceScalarBoundaryV31>()
    );
    type CfgMeterFields<'a> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    assert_eq!(
        size_of::<OptimizedScalarCfgMeterV31<'_, '_, '_, '_>>(),
        size_of::<CfgMeterFields<'_>>()
    );
    assert_eq!(
        std::mem::align_of::<OptimizedScalarCfgMeterV31<'_, '_, '_, '_>>(),
        std::mem::align_of::<CfgMeterFields<'_>>()
    );
    assert_eq!(
        optimized_source_boundary_headers_v31().unwrap(),
        size_of::<OutputFrame<'_>>() + std::mem::align_of::<OutputFrame<'_>>()
    );
}

#[test]
fn source_scalar_boundaries_derivation_and_control_frames_match_field_envelopes() {
    type Shared<'a> = fe2o3_kernel_analysis::SourceSsaBoundariesV31<'a>;
    type Receipt = fe2o3_kernel_analysis::SourceSsaBoundaryStorageV31;
    type SharedError = fe2o3_kernel_analysis::SourceSsaBoundaryErrorV31;
    type Derive<'a, 'w> = (
        Vec<Vec<BoundaryBlockV31>>,
        Vec<BoundaryBlockV31>,
        Shared<'a>,
        Receipt,
        Result<(Shared<'a>, Receipt), SharedError>,
        SourceOwnedResultV18<(Shared<'a>, Receipt)>,
        SourceOwnedResultV18<Vec<Vec<BoundaryBlockV31>>>,
        SourceOwnedResultV18<Vec<BoundaryBlockV31>>,
        Result<Vec<Vec<BoundaryBlockV31>>, ProductionSemanticKirErrorV1>,
        Result<Vec<BoundaryBlockV31>, ProductionSemanticKirErrorV1>,
        Result<EntryValueV20, SharedError>,
        SourceOwnedResultV18<EntryValueV20>,
        &'a (),
        &'a (),
        &'a (),
        &'a mut (),
        &'a mut ArgumentBudgetV1<'w>,
        [usize; 8],
        SourceOwnedResultV18<()>,
    );
    assert_eq!(
        source_boundary_derive_headers_v31().unwrap(),
        size_of::<Derive<'_, '_>>() + std::mem::align_of::<Derive<'_, '_>>()
    );
    type Check<'a> = (
        &'a (),
        &'a (),
        &'a (),
        &'a (),
        &'a (),
        Option<&'a ()>,
        &'a (),
        std::ops::Range<usize>,
    );
    type Index<'a> = (
        &'a (),
        Vec<OriginalEntryDefinitionRowV20>,
        Vec<OriginalHelperCallV33>,
        usize,
    );
    type Arguments<'a> = (&'a (), usize, Vec<Option<SourceRootParameterV18>>);
    type Normalizer<'a> = (&'a (), &'a ());
    assert_eq!(
        size_of::<Check<'_>>(),
        size_of::<SourceBoundaryCheckV31<'_>>()
    );
    assert_eq!(
        size_of::<Index<'_>>(),
        size_of::<OriginalEntryIndexV20<'_, '_>>()
    );
    assert_eq!(
        size_of::<Arguments<'_>>(),
        size_of::<SourceRootArgumentsV18<'_, '_>>()
    );
    assert_eq!(
        size_of::<Normalizer<'_>>(),
        size_of::<OptimizedSourceScalarNormalizationV18<'_>>()
    );
    type Control<'a, 'w> = (
        Check<'a>,
        Index<'a>,
        Arguments<'a>,
        Gfx942InlineScalarCorrespondenceV30<'a>,
        Option<Gfx942InlineScalarCorrespondenceV30<'a>>,
        SourceOwnedResultV18<Option<Gfx942InlineScalarCorrespondenceV30<'a>>>,
        Normalizer<'a>,
        Vec<SourceReferenceSelectionControlV30>,
        Vec<bool>,
        std::ops::Range<usize>,
        (std::ops::Range<usize>, Vec<bool>),
        SourceOwnedResultV18<(std::ops::Range<usize>, Vec<bool>)>,
        SourceOwnedResultV18<usize>,
        [SourceOwnedResultV18<Option<&'a ()>>; 3],
        [Option<&'a ()>; 2],
        [&'a (); 2],
        [&'a (); 2],
        &'a (),
        &'a (),
        SourceOwnedResultV18<Index<'a>>,
        SourceOwnedResultV18<Arguments<'a>>,
        SourceOwnedResultV18<Gfx942InlineScalarCorrespondenceV30<'a>>,
        SourceOwnedResultV18<Vec<SourceReferenceSelectionControlV30>>,
        SourceOwnedResultV18<Vec<bool>>,
        Result<Vec<SourceReferenceSelectionControlV30>, ProductionSemanticKirErrorV1>,
        Result<Vec<bool>, ProductionSemanticKirErrorV1>,
        OriginalPrivateInputV22<'a>,
        OriginalEntryDefinitionRowV20,
        ProductionSemanticExpressionV2,
        SourceOwnedResultV18<ProductionSemanticExpressionV2>,
        SourceScalarBoundaryV31,
        InvocationArgumentRowV1,
        InvocationComponentRowV1,
        ScopedEmittedPointsV29<'a, 'a, 'w>,
        &'a (),
        &'a (),
        &'a (),
        &'a (),
        &'a mut ArgumentBudgetV1<'w>,
        [usize; 16],
        [SourceOwnedResultV18<()>; 4],
        Result<Option<(BlockId, u32)>, ScopedTileFailureKindV29>,
    );
    assert_eq!(
        source_boundary_control_headers_v31().unwrap(),
        size_of::<Control<'_, '_>>()
            + std::mem::align_of::<Control<'_, '_>>()
            + original_private_expression_headers_v22().unwrap()
    );
}

#[test]
fn source_scalar_boundaries_reject_funded_foreign_ledger_without_query_work_or_refund() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, _) = integer_handoff_prepared_v18(entry_loop, &mut budget);
    let reached = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        source
            .with_checked_mixed_fixedpoint_optimization_v18(
                budget,
                |original, optimized, budget| {
                    original.with_optimized_scalar_leaf_namespace_v18(
                        optimized,
                        0,
                        &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                        budget,
                        |leaves, budget| {
                            let value = leaves.boundaries.values[0].value;
                            let mut foreign_work =
                                CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                            let mut foreign =
                                ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                            foreign.reserve_storage(budget.storage())?;
                            let foreign_before =
                                (foreign.work(), foreign.storage(), foreign.peak_storage());
                            let before = (budget.work(), budget.storage(), budget.peak_storage());
                            let refused =
                                leaves.boundary_value_v31(value, &mut foreign).map(|_| ());
                            assert!(matches!(
                                refused,
                                Err(ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Accounting
                                ))
                            ));
                            assert_eq!(
                                (foreign.work(), foreign.storage(), foreign.peak_storage()),
                                foreign_before
                            );
                            assert_eq!(
                                (budget.work(), budget.storage(), budget.peak_storage()),
                                before
                            );
                            assert!(original.source.cleanup.is_denied());
                            assert!(matches!(
                                leaves.boundary_value_v31(value, budget),
                                Err(ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Accounting
                                ))
                            ));
                            assert_eq!(
                                (budget.work(), budget.storage(), budget.peak_storage()),
                                before
                            );
                            reached.set(true);
                            refused
                        },
                    )?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                },
            )
            .map(|(owner, (), _)| drop(owner))
    });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOptimizationErrorV18::Source(
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        ))
    ));
    assert!(budget.storage() > MODULE_FLOOR);
}
