fn lifetime_marker(dead: bool, n: u32) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        source(),
        if dead {
            SemanticStatementKindV1::StorageDead(local(n))
        } else {
            SemanticStatementKindV1::StorageLive(local(n))
        },
    )
}

fn captured_with_death(moved: bool) -> Fixture {
    let mut fixture = Fixture::new(moved);
    let mut statements = fixture.function.blocks()[3].statements().to_vec();
    statements.extend([
        SemanticStatementV1::new(source(), SemanticStatementKindV1::Nop),
        SemanticStatementV1::new(source(), SemanticStatementKindV1::Nop),
        lifetime_marker(true, 2),
    ]);
    fixture.replace_block(3, statements, goto(4));
    let mut statements = fixture.function.blocks()[6].statements().to_vec();
    statements.push(lifetime_marker(true, 3));
    fixture.replace_block(6, statements, SemanticTerminatorKindV1::Return);
    fixture
}

#[test]
fn ordinary_enum_alias_lifetime_actual_move_kill_and_later_carrier_kill_keep_exact_slot() {
    for moved in [false, true] {
        let baseline = Fixture::new(moved);
        let (before_blocks, _) = baseline.emit().unwrap();
        let fixture = captured_with_death(moved);
        let source_before = fixture.function.clone();
        let (blocks, lowering) = fixture.emit().unwrap();
        assert_eq!(
            blocks, before_blocks,
            "lifetime markers create no values or default payloads"
        );
        assert_eq!(
            source_before, fixture.function,
            "retain all original statements and branches"
        );
        assert!(lowering.locals[2].is_none());
        assert!(lowering.locals[3].is_none());
        let proof = lowering.enum_payload_aliases.as_ref().unwrap()[&3];
        assert_eq!(proof.source(), 2);
        let pointer = lowering.enum_payload_storage[&(2, 1, 0)].components[0].pointer;
        let mut loads = Vec::new();
        let mut stores = Vec::new();
        for block in &blocks {
            for op in &block.operations {
                match op.kind {
                    OperationKind::Load { pointer: p, .. } if p == pointer => {
                        loads.push(block.id.0)
                    }
                    OperationKind::Store { pointer: p, .. } if p == pointer => {
                        stores.push(block.id.0)
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(loads, [6]);
        assert_eq!(stores, [1]);
    }
}

#[test]
fn ordinary_enum_alias_lifetime_move_switch_then_successor_death_preserves_kir() {
    fn shape(with_markers: bool) -> Fixture {
        let mut f = if with_markers {
            captured_with_death(true)
        } else {
            Fixture::new(true)
        };
        let mut selected = f.function.blocks()[6].statements().to_vec();
        selected.retain(|s| !matches!(s.kind(), SemanticStatementKindV1::StorageDead(_)));
        f.replace_block(6, selected, goto(5));
        let mut cleanup = vec![SemanticStatementV1::new(source(), SemanticStatementKindV1::Nop); 4];
        if with_markers {
            cleanup.push(lifetime_marker(true, 3));
        }
        f.replace_block(5, cleanup, SemanticTerminatorKindV1::Return);
        let SemanticTerminatorKindV1::SwitchInt { targets, .. } =
            f.function.blocks()[4].terminator().kind()
        else {
            unreachable!()
        };
        let term = SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Move(place(4, 1)),
            targets: targets.clone(),
        };
        f.replace_block(4, f.function.blocks()[4].statements().to_vec(), term);
        f
    }
    let baseline = shape(false);
    let fixture = shape(true);
    let original = fixture.function.clone();
    let (expected, _) = baseline.emit().unwrap();
    let (actual, lowering) = fixture.emit().unwrap();
    assert_eq!(actual, expected);
    assert_eq!(fixture.function, original);
    assert_eq!(
        lowering.enum_payload_aliases.as_ref().unwrap()[&3].source(),
        2
    );
    assert!(lowering.locals[2].is_none());
    assert!(lowering.locals[3].is_none());
}

#[test]
fn ordinary_enum_alias_lifetime_source_death_must_be_strictly_after_same_block_capture() {
    let base = Fixture::new(true);
    let transport = base.lowering().control_flow_ssa;
    for mutation in 0..4 {
        let mut f = Fixture::new(true);
        match mutation {
            0 => {
                let mut statements = f.function.blocks()[3].statements().to_vec();
                statements.insert(0, lifetime_marker(true, 2));
                f.replace_block(3, statements, goto(4));
            }
            1 => f.replace_block(
                5,
                vec![lifetime_marker(true, 2)],
                SemanticTerminatorKindV1::Return,
            ),
            2 => {
                let mut statements = f.function.blocks()[1].statements().to_vec();
                statements.push(lifetime_marker(true, 2));
                f.replace_block(1, statements, goto(3));
            }
            _ => {
                let mut statements = f.function.blocks()[3].statements().to_vec();
                statements.extend([lifetime_marker(true, 2), lifetime_marker(true, 2)]);
                f.replace_block(3, statements, goto(4));
            }
        }
        // The independent source audit must reject even with the old transport
        // roster. This is a mutation test, not admission of inconsistent SSA.
        let mut budget = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
        assert!(
            ordinary_enum_alias_v1::plan(&f.types, &f.function, &transport, &mut budget)
                .unwrap()
                .is_empty(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordinary_enum_alias_lifetime_new_slot_value_escape_and_cleanup_still_reject() {
    let base = Fixture::new(true);
    let transport = base.lowering().control_flow_ssa;
    for mutation in 0..7 {
        let mut f = captured_with_death(true);
        let mut statements = f.function.blocks()[3].statements().to_vec();
        let mut term = goto(4);
        match mutation {
            0 => statements.extend([lifetime_marker(false, 2), some(2, constant(99))]),
            1 => statements.push(SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::Deinitialize(place(2, 2)),
            )),
            2 => statements.push(SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::SetDiscriminant {
                    place: place(2, 2),
                    variant_index: 0,
                },
            )),
            3 => statements.push(assign(
                5,
                1,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: place(2, 2),
                },
            )),
            4 => term = goto(1),
            5 => {
                term = SemanticTerminatorKindV1::Assert {
                    condition: copy(1, 3),
                    expected: true,
                    message: SemanticAssertMessageV1::ResumedAfterPanic,
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 4),
                    unwind: SemanticUnwindActionV1::Cleanup(edge(
                        SemanticEdgeRoleV1::AssertUnwind,
                        1,
                    )),
                }
            }
            _ => {
                term = SemanticTerminatorKindV1::TailCall(
                    SemanticDirectTailCallV1::new_callable(
                        SemanticCallableIdV1::from_index(0),
                        vec![copy(2, 2)],
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                )
            }
        }
        f.replace_block(3, statements, term);
        let mut budget = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
        assert!(
            ordinary_enum_alias_v1::plan(&f.types, &f.function, &transport, &mut budget)
                .unwrap()
                .is_empty(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordinary_enum_alias_lifetime_exact_use_version_and_prefix_are_required() {
    let fixture = captured_with_death(true);
    let transport = fixture.lowering().control_flow_ssa;
    let mut budget = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
    let proof = ordinary_enum_alias_v1::plan_aliases(
        &fixture.types,
        &fixture.function,
        &transport,
        &mut budget,
    )
    .unwrap()[&3];
    assert!(
        proof
            .allows_use(
                &fixture.function,
                &transport,
                SemanticBlockIdV1::from_index(6),
                Some(0),
                3,
                &mut budget
            )
            .unwrap()
    );
    assert!(
        !proof
            .allows_use(
                &fixture.function,
                &transport,
                SemanticBlockIdV1::from_index(3),
                Some(0),
                3,
                &mut budget
            )
            .unwrap()
    );
    assert!(
        !proof
            .allows_use(
                &fixture.function,
                &transport,
                SemanticBlockIdV1::from_index(6),
                None,
                3,
                &mut budget
            )
            .unwrap()
    );
    for dead in [false, true] {
        let mut f = captured_with_death(true);
        let mut statements = f.function.blocks()[6].statements().to_vec();
        statements.insert(0, lifetime_marker(dead, 3));
        f.replace_block(6, statements, SemanticTerminatorKindV1::Return);
        // Retaining an old entry value must not bypass a new same-block kill.
        assert!(
            !proof
                .allows_use(
                    &f.function,
                    &transport,
                    SemanticBlockIdV1::from_index(6),
                    Some(1),
                    3,
                    &mut budget
                )
                .unwrap()
        );
    }
    let mut changed = transport.clone();
    changed.block_entry_values.remove(&(6, 3));
    assert!(
        !proof
            .allows_use(
                &fixture.function,
                &changed,
                SemanticBlockIdV1::from_index(6),
                Some(0),
                3,
                &mut budget
            )
            .unwrap()
    );
    changed.block_entry_values.insert(
        (6, 3),
        SsaValueV1::BlockArgument {
            block: SsaBlockIdV1::new(6),
            variable: fe2o3_mir_model::SsaVariableIdV1::new(3),
        },
    );
    assert!(
        !proof
            .allows_use(
                &fixture.function,
                &changed,
                SemanticBlockIdV1::from_index(6),
                Some(0),
                3,
                &mut budget
            )
            .unwrap()
    );
}

#[test]
fn ordinary_enum_alias_lifetime_does_not_resurrect_source_or_carrier_at_ssa() {
    for killed in [2, 3] {
        let mut f = captured_with_death(false);
        let mut statements = f.function.blocks()[3].statements().to_vec();
        if killed == 3 {
            statements.push(lifetime_marker(true, 3));
        }
        statements.push(assign(
            4,
            1,
            SemanticRvalueKindV1::Discriminant(place(killed, 2)),
        ));
        f.replace_block(3, statements, goto(4));
        let result = plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &f.function,
            &f.types,
            &[],
            ProductionSemanticSsaLimitsV1::default(),
        );
        assert!(
            matches!(result, Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::Planner {
            function, error: fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse { block, variable, .. }
        }) if function.index()==0 && block.get()==3 && variable.get()==killed),
            "{result:?}"
        );
    }
}

#[test]
fn ordinary_enum_alias_lifetime_killed_predecessor_and_backedge_reject_at_ssa() {
    for backedge in [false, true] {
        let mut f = captured_with_death(false);
        let mut selected = f.function.blocks()[6].statements().to_vec();
        selected.retain(|s| !matches!(s.kind(), SemanticStatementKindV1::StorageDead(_)));
        f.replace_block(
            6,
            selected,
            if backedge {
                goto(5)
            } else {
                SemanticTerminatorKindV1::Return
            },
        );
        f.replace_block(5, vec![lifetime_marker(true, 3)], goto(6));
        let result = plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &f.function,
            &f.types,
            &[],
            ProductionSemanticSsaLimitsV1::default(),
        );
        assert!(
            matches!(result, Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::Planner {
            function, error: fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtEdge { edge, target, variable }
        }) if function.index()==0 && edge.source().get()==5 && target.get()==6 && variable.get()==3),
            "{result:?}"
        );
    }
}

#[test]
fn ordinary_enum_alias_lifetime_variant_guard_and_bypass_remain_required() {
    let mut missing = captured_with_death(true);
    missing.replace_block(4, vec![], goto(6));
    require_missing_payload(&missing);
    let mut bypass = captured_with_death(true);
    bypass.replace_block(5, vec![], goto(6));
    require_missing_payload(&bypass);
}

#[test]
fn ordinary_enum_alias_lifetime_shared_budget_includes_capture_and_use_without_refund() {
    let f = captured_with_death(true);
    let transport = f.lowering().control_flow_ssa;
    let run =
        |budget: &mut SemanticEnumAnalysisBudgetV1| -> Result<bool, ProductionSemanticKirErrorV1> {
            let proof =
                ordinary_enum_alias_v1::plan_aliases(&f.types, &f.function, &transport, budget)
                    .unwrap()[&3];
            proof.allows_use(
                &f.function,
                &transport,
                SemanticBlockIdV1::from_index(6),
                Some(0),
                3,
                budget,
            )
        };
    let mut measured = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
    assert!(run(&mut measured).unwrap());
    let mut exact = SemanticEnumAnalysisBudgetV1::new(measured.work + 17, measured.storage + 29);
    exact.charge_work(17).unwrap();
    exact.charge_storage(29).unwrap();
    assert!(run(&mut exact).unwrap());
    assert_eq!(
        (exact.work, exact.storage),
        (measured.work + 17, measured.storage + 29)
    );
    let mut short = SemanticEnumAnalysisBudgetV1::new(measured.work + 16, measured.storage + 29);
    short.charge_work(17).unwrap();
    short.charge_storage(29).unwrap();
    assert!(
        matches!(run(&mut short), Err(ProductionSemanticKirErrorV1::ResourceLimit {
        resource: ProductionSemanticKirResourceV1::AnalysisWork, actual, limit
    }) if actual==measured.work+17 && limit==measured.work+16)
    );
    let mut storage_short = SemanticEnumAnalysisBudgetV1::new(measured.work, measured.storage - 1);
    assert!(
        matches!(ordinary_enum_alias_v1::plan_aliases(&f.types, &f.function, &transport, &mut storage_short),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisStorage, actual, limit,
        }) if actual==measured.storage && limit==measured.storage-1)
    );
}
