use super::*;

fn call_return_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = header_loop();
    let source = base.source_semantic();
    let old = &source.functions()[0];
    let mut blocks = old.blocks().to_vec();
    blocks[0] = block(
        180,
        vec![],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(4),
                vec![SemanticOperandV1::Copy(place(1, U32))],
                Some(SemanticCallDestinationV1::new(
                    place(2, U32),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    let mut functions = source.functions().to_vec();
    functions[0] = rebuild_root(old, old.locals().to_vec(), blocks.clone());
    functions[1] = rebuild_root(
        &source.functions()[1],
        old.locals()
            .iter()
            .enumerate()
            .map(|(at, row)| local(201 + at as u8, row.ty(), row.role()))
            .collect(),
        blocks
            .into_iter()
            .enumerate()
            .map(|(at, row)| {
                block(
                    211 + at as u8,
                    row.statements().to_vec(),
                    row.terminator().kind().clone(),
                )
            })
            .collect(),
    );
    let direct = || {
        SemanticAbiValueV1::new(
            U32,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )
    };
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([221; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct())],
        direct(),
    )
    .unwrap();
    functions.push(function(
        220,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        vec![
            local(222, U32, SemanticLocalRoleV1::Return),
            local(223, U32, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(
            224,
            vec![assign(
                place(0, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32))),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    ));
    let callables = (0..functions.len())
        .map(|at| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(at as u32)))
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
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

fn source_call(
    check: &SourceBoundaryCheckV31<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(SourceScalarBoundaryV31, EntryValueV20, usize)> {
    let row = *check
        .leaves
        .boundaries
        .rows
        .iter()
        .find(|row| row.instance == 0 && row.variable.get() == 2 && row.block.get() == 1)
        .unwrap();
    let relation = check.leaves.relation;
    let (function, _) = relation
        .source
        .instance(check.leaves.root, row.instance, budget)?;
    let owner = relation.source.source_ssa(budget)?;
    let rows = owner.occurrences_v1().unwrap().function(function).unwrap();
    let (edge, call) = rows
        .edge_definitions()
        .iter()
        .enumerate()
        .find(|(_, row)| row.edge().source().get() == 0 && row.variable().get() == 2)
        .unwrap();
    Ok((row, call.value().unwrap(), edge))
}

fn exercise(
    check: &SourceBoundaryCheckV31<'_>,
    mut row: SourceScalarBoundaryV31,
    value: EntryValueV20,
    actual: ValueId,
    fault: Option<u8>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    match fault {
        Some(0) => row.ty = UNIT,
        Some(1) => row.variable = BoundaryVariableV31::new(1),
        Some(2) => row.instance = 1,
        Some(3) => {
            // A loop parameter has distinct initial and recurrence inputs. It
            // cannot stand for the exact source call-result incoming value.
            let definition = check.definition(&row, budget)?;
            let conflicting = check.inventory.definitions()[definition].value.unwrap();
            assert!(
                check
                    .origins
                    .resolve(conflicting, budget)
                    .map_err(source_pointer_inventory_error_v18)?
                    .is_none()
            );
            return check
                .call_return_value_v32(&row, value, conflicting, budget)
                .map(|_| ());
        }
        Some(4) => {
            return check
                .expression(
                    row.instance,
                    row.variable,
                    value,
                    row.ty,
                    row.scalar,
                    budget,
                )
                .map(|_| ());
        }
        _ => {}
    }
    if fault.is_none() {
        assert!(check.call_return_value_v32(&row, value, actual, budget)?);
    }
    check.value(&row, value, actual, budget)
}

fn transport_case(optimized_endpoint: bool, fault: Option<u8>) -> ProductionOptimizerTestResultV18 {
    with_policy11(call_return_owner, |original, optimized, budget| {
        for root in 0..2 {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                root,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    with_check(leaves, budget, |check, budget| {
                        let (row, value, _) = source_call(check, budget)?;
                        let input = original
                            .ssa_scalar_definition_v30(root, 0, value, budget)?
                            .unwrap();
                        let definition = &original.inventory.definitions()[input];
                        if optimized_endpoint {
                            let descendants =
                                optimized.definition_descendants(definition.coordinate, budget)?;
                            assert!(!descendants.is_empty());
                            for descendant in descendants {
                                let actual = optimized_source_definition_row_v18(
                                    check.inventory,
                                    descendant.output,
                                    budget,
                                )?
                                .value
                                .unwrap();
                                exercise(check, row, value, actual, fault, budget)?;
                            }
                            Ok(())
                        } else {
                            let function = &original.inventory.functions()
                                [original.source.root(root, budget)?.1];
                            value_origin_v1::with_whole_value_origins_v18(
                                original,
                                function.coordinate,
                                budget,
                                |origins, budget| {
                                    let input_check = SourceBoundaryCheckV31 {
                                        leaves: check.leaves,
                                        index: check.index,
                                        arguments: check.arguments,
                                        origins,
                                        inline: check.inline,
                                        optimized: None,
                                        inventory: original.inventory,
                                        bindings: function.edge_arguments.clone(),
                                    };
                                    exercise(
                                        &input_check,
                                        row,
                                        value,
                                        definition.value.unwrap(),
                                        fault,
                                        budget,
                                    )
                                },
                            )
                        }
                    })
                },
            )?;
        }
        Ok(())
    })
}

#[test]
fn source_call_return_checks_original_and_optimized_transport_across_roots() {
    transport_case(false, None).unwrap();
    transport_case(true, None).unwrap();
}

#[test]
fn source_call_return_rejects_changed_destination_type_and_foreign_instance() {
    for optimized in [false, true] {
        for fault in 0..3 {
            let result = transport_case(optimized, Some(fault));
            assert!(
                matches!(
                    result,
                    Err(ProductionSourceOptimizationErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Binding(_)
                    ))
                ),
                "{result:?}"
            );
        }
    }
}

#[test]
fn source_call_return_rejects_conflicting_forwarding() {
    for optimized in [false, true] {
        assert_binding(
            transport_case(optimized, Some(3)),
            "source SSA call-return incoming value differs",
        );
    }
}

#[test]
fn source_call_return_transport_does_not_admit_callee_computation() {
    assert_binding(
        transport_case(false, Some(4)),
        "source SSA call-return computation is not interpreted",
    );
}

#[test]
fn source_call_return_archive_rejects_changed_edge_local_and_definition() {
    for fault in 0..3 {
        let result = with_policy11(call_return_owner, |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    with_check(leaves, budget, |check, budget| {
                        let (row, value, edge) = source_call(check, budget)?;
                        let EntryValueV20::Definition(value) = value else {
                            panic!("not a captured call definition")
                        };
                        source_entry_call_return_v32(
                            original,
                            SemanticFunctionIdV1::from_index(0),
                            if fault == 0 { edge + 1 } else { edge },
                            if fault == 1 {
                                row.variable.get() + 1
                            } else {
                                row.variable.get()
                            },
                            if fault == 2 { u32::MAX } else { value.get() },
                            budget,
                        )
                        .map(|_| ())
                    })
                },
            )
        });
        assert!(
            matches!(
                result,
                Err(ProductionSourceOptimizationErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(_)
                ))
            ),
            "{result:?}"
        );
    }
}

fn archive_work_cut(remaining: Option<usize>) -> bool {
    let completed = std::cell::Cell::new(None);
    let result = with_policy11(call_return_owner, |original, optimized, budget| {
        original.with_optimized_scalar_leaf_namespace_v18(
            optimized,
            0,
            &SourceScalarNamespaceV18::PrivateSourceWritesV22,
            budget,
            |leaves, budget| {
                with_check(leaves, budget, |check, budget| {
                    let (row, value, edge) = source_call(check, budget)?;
                    let EntryValueV20::Definition(value) = value else {
                        panic!("not a call definition")
                    };
                    let owner = original.source.source_ssa(budget)?;
                    assert_eq!(
                        owner
                            .occurrences_v1()
                            .unwrap()
                            .function(SemanticFunctionIdV1::from_index(0))
                            .unwrap()
                            .successors()
                            .len(),
                        5
                    );
                    if let Some(remaining) = remaining {
                        budget.charge_work(
                            OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - remaining,
                        )?;
                    }
                    let before = (budget.work(), budget.storage());
                    match source_entry_call_return_v32(
                        original,
                        SemanticFunctionIdV1::from_index(0),
                        edge,
                        row.variable.get(),
                        value.get(),
                        budget,
                    ) {
                        Ok(_) => {
                            // Two retained-owner queries, 14 identity checks,
                            // three lower-bound probes, three successor checks.
                            assert_eq!(budget.work() - before.0, 2 + 14 + 3 + 3);
                            assert_eq!(budget.storage(), before.1);
                            completed.set(Some(true));
                            Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "call-return exact work stop",
                            ))
                        }
                        Err(error) => {
                            assert!(
                                matches!(
                                    error,
                                    ProductionSourceOwnedViewErrorV18::Resource(
                                        ArgumentResourceV1::Work(_)
                                    )
                                ),
                                "{error:?}"
                            );
                            let after = (budget.work(), budget.storage());
                            let repeated = source_entry_call_return_v32(
                                original,
                                SemanticFunctionIdV1::from_index(0),
                                edge,
                                row.variable.get(),
                                value.get(),
                                budget,
                            )
                            .unwrap_err();
                            assert_eq!(repeated.to_string(), error.to_string());
                            assert_eq!((budget.work(), budget.storage()), after);
                            completed.set(Some(false));
                            Err(error)
                        }
                    }
                })
            },
        )
    });
    assert!(result.is_err());
    completed
        .get()
        .expect("call-return resource checkpoint reached")
}

#[test]
fn source_call_return_archive_has_independent_exact_work_and_sticky_one_short() {
    assert!(archive_work_cut(None));
    assert!(archive_work_cut(Some(22)));
    assert!(!archive_work_cut(Some(21)));
}

#[test]
fn source_call_return_headers_cover_exact_and_one_short_reservations() {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    type Frame<'a> = (
        [&'a (); 32],
        [usize; 24],
        [u32; 6],
        [bool; 3],
        OriginalEntryDefinitionRowV20,
        EntryValueV20,
        SourceScalarBoundaryV31,
        SemanticFunctionIdV1,
        SemanticTypeIdV1,
        Type,
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        [Definition; 2],
        [Option<Definition>; 2],
        [Result<Option<Definition>, fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1>; 2],
        SourceOwnedResultV18<&'a SemanticDirectCallV1>,
        SourceOwnedResultV18<(SemanticFunctionIdV1, Option<(usize, SemanticBlockIdV1)>)>,
        SourceOwnedResultV18<(SemanticFunctionIdV1, usize)>,
        SourceOwnedResultV18<Option<usize>>,
        SourceOwnedResultV18<OriginalEntryDefinitionRowV20>,
        SourceOwnedResultV18<bool>,
        SourceOwnedResultV18<()>,
        Result<Type, ProductionSemanticKirErrorV1>,
        Result<
            Option<&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>>,
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
        >,
        SourceOwnedResultV18<&'a [fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1]>,
        std::slice::Iter<'a, fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>,
    );
    let expected = size_of::<Frame<'_>>() + std::mem::align_of::<Frame<'_>>();
    assert_eq!(source_call_return_headers_v32().unwrap(), expected);
    assert!(original_private_expression_headers_v22().unwrap() >= expected);
    for short in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, 31 + expected - short);
        budget.reserve_storage(31).unwrap();
        let result = budget.reserve_storage(source_call_return_headers_v32().unwrap());
        if short == 0 {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        } else {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(_))));
        }
        assert_eq!(budget.storage(), 31);
    }
}
