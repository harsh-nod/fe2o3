use super::*;

fn terminal_seen(
    check: &SourceBoundaryCheckV31<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(Vec<bool>, usize, usize)> {
    let mut seen = vec![false; check.bindings.len()];
    let mut terminals = 0;
    let mut forwarded = 0;
    for (at, binding) in check.inventory.edge_arguments()[check.bindings.clone()]
        .iter()
        .enumerate()
    {
        if let Some(row) = check.boundary_name_v32(binding.target_definition, budget)? {
            if check.definition(row, budget)? == binding.target_definition {
                seen[at] = true;
                terminals += 1;
            } else {
                forwarded += 1;
            }
        }
    }
    Ok((seen, terminals, forwarded))
}

#[test]
fn source_boundary_incoming_census_checks_original_and_output_forwarding_equations() {
    with_policy11(
        private_entry_phi_owner_v20,
        |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    with_check(leaves, budget, |check, budget| {
                        let function = &original.inventory.functions()
                            [original.source.root_row(0)?.function_ordinal];
                        // The census uses only exact endpoint definitions and the
                        // checked names, not the scalar normalizer or its origins.
                        let source_check = SourceBoundaryCheckV31 {
                            leaves: check.leaves,
                            index: check.index,
                            arguments: check.arguments,
                            origins: check.origins,
                            inline: check.inline,
                            optimized: None,
                            inventory: original.inventory,
                            bindings: function.edge_arguments.clone(),
                        };
                        for census in [&source_check, check] {
                            let (mut seen, terminals, forwarded) = terminal_seen(census, budget)?;
                            assert!(terminals > 0 && forwarded > 0);
                            census.incoming_census_v32(&mut seen, budget)?;
                            let mut counted = 0;
                            for (at, binding) in census.inventory.edge_arguments()
                                [census.bindings.clone()]
                            .iter()
                            .enumerate()
                            {
                                if census
                                    .boundary_name_v32(binding.target_definition, budget)?
                                    .is_some()
                                {
                                    assert!(seen[at]);
                                    counted += 1;
                                }
                            }
                            assert_eq!(counted, terminals + forwarded);
                        }
                        Ok(())
                    })
                },
            )
        },
    )
    .unwrap();
}

#[test]
fn source_boundary_incoming_census_keeps_terminal_obligations_and_rejects_changed_forwarding() {
    for fault in 0..5 {
        let result = with_policy11(
            private_entry_phi_owner_v20,
            |original, optimized, budget| {
                original.with_optimized_scalar_leaf_namespace_v18(optimized, 0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22, budget, |leaves, budget| {
                    with_check(leaves, budget, |check, budget| {
                        let (mut seen, terminals, forwarded) = terminal_seen(check, budget)?;
                        assert!(terminals > 0 && forwarded > 0);
                        if fault == 0 {
                            let at = seen.iter().position(|seen| *seen).unwrap();
                            seen[at] = false;
                            return check.incoming_census_v32(&mut seen, budget);
                        }
                        for (at, binding) in check.inventory.edge_arguments()[check.bindings.clone()].iter().enumerate() {
                            let Some(target) = check.boundary_name_v32(binding.target_definition, budget)? else { continue; };
                            if check.definition(target, budget)? == binding.target_definition { continue; }
                            if fault == 1 {
                                seen[at] = true;
                                return check.incoming_census_v32(&mut seen, budget);
                            }
                            let mut changed = fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1 {
                                coordinate: binding.coordinate, value: binding.value,
                                incoming_definition: binding.incoming_definition,
                                target_definition: binding.target_definition,
                            };
                            match fault {
                                2 => changed.value = ValueId(u32::MAX),
                                3 => {
                                    let (definition, row) = check.inventory.definitions().iter().enumerate().find(|(_, row)|
                                        matches!(row.coordinate, fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::FunctionArgument { function, .. } if function == leaves.function.coordinate)).unwrap();
                                    changed.incoming_definition = definition;
                                    changed.value = row.value.unwrap();
                                }
                                _ => changed.coordinate.argument += 1,
                            }
                            return check.forwarded_edge_v32(&changed, target, budget);
                        }
                        panic!("fixture has no actual forwarded incoming edge");
                    })
                })
            },
        );
        assert_binding(
            result,
            match fault {
                0 => "source SSA boundary has an unaccounted incoming edge",
                1 => "source SSA forwarded incoming edge repeats",
                _ => "source SSA forwarded edge changes its checked boundary",
            },
        );
    }
}
