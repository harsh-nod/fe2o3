use super::*;

pub(super) fn challenge_generated_holder_roles(
    equations: &OptimizedSourceCurrentnessEquationsV18<'_, '_>,
    transport: &OptimizedSourceAliasTransportV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        // This is the genuine producer output over the checked optimized graph.
        // Mutated copies below are deliberately only equation hypotheses.
        with_canonical_call_scratch_v1(budget, |budget| {
            equations.check(transport.equations(), budget)
        })?;
        budget.reserve_storage(size_of::<Vec<SourceAddressLogicalUseV29>>())?;
        let mut challenged = 0;
        for (fresh_event_index, event) in transport.boundaries.iter().enumerate() {
            budget.charge_work(2)?;
            let SourceAddressBoundaryKindV29::Alias(fresh) = event.kind else {
                continue;
            };
            let alias = &transport.aliases[fresh];
            if !matches!(alias.recipe, SourceAddressAliasRecipeV29::Birth) {
                continue;
            }
            let block = equations.graph.blocks[equations.graph.block(event.block, budget)?].1;
            let Some(terminator) = &block.terminator else {
                continue;
            };
            let edges = match terminator {
                Terminator::Switch { cases, .. } => cases.len() + 1,
                Terminator::IntegerSwitch { cases, .. } => cases.len() + 1,
                Terminator::ConditionalBranch { .. } => 2,
                Terminator::Branch { .. } => 1,
                Terminator::Return { .. } | Terminator::Unreachable => 0,
            };
            budget.charge_work(edges)?;
            let mut backedges = 0;
            source_reference_cell_edge_values_v29(terminator, block.id, |_| {
                backedges += 1;
                Ok(())
            })?;
            if backedges == 0 {
                continue;
            }
            let mut activation = None;
            for (index, boundary) in transport.boundaries.iter().enumerate() {
                budget.charge_work(1)?;
                let SourceAddressBoundaryKindV29::Lifetime(lifetime) = boundary.kind else {
                    continue;
                };
                let row = &equations.lifetimes[lifetime];
                if row.slot != alias.slot {
                    continue;
                }
                if !row.live {
                    // The authentic source also ends each allocation at Return.
                    // This is not another loop activation or an interior kill.
                    let exit = equations.graph.blocks[equations.graph.block(row.block, budget)?].1;
                    assert!(
                        matches!(&exit.terminator, Some(Terminator::Return { .. }))
                            && row.gap == exit.operations.len()
                            && row.block != block.id,
                        "only final return deactivation may accompany the loop's single static activation"
                    );
                    continue;
                }
                assert!(
                    activation.is_none(),
                    "one static activation must serve every iteration of this allocation"
                );
                assert!(row.block == block.id && index < fresh_event_index);
                activation = Some(lifetime);
            }
            if activation.is_none() {
                continue;
            }
            for later in &transport.boundaries[fresh_event_index + 1..] {
                budget.charge_work(2)?;
                let SourceAddressBoundaryKindV29::Alias(stale) = later.kind else {
                    continue;
                };
                let old = &transport.aliases[stale];
                if later.block != block.id
                    || old.slot != alias.slot
                    || !matches!(old.recipe, SourceAddressAliasRecipeV29::Birth)
                {
                    continue;
                }
                assert_eq!(
                    alias.pointer, old.pointer,
                    "separate source births in one activation must share the folded physical backing"
                );
                for (index, usage) in transport.uses.iter().enumerate() {
                    budget.charge_work(2)?;
                    if usage.alias != fresh || usage.operand != 1 || usage.block != block.id {
                        continue;
                    }
                    let Some(operation) = usage.operation else {
                        continue;
                    };
                    if operation < event.gap || operation >= later.gap {
                        continue;
                    }
                    let Some(access) =
                        source_address_value_access_v29(&block.operations[operation])?
                    else {
                        continue;
                    };
                    if !access.writing || access.value != usage.value {
                        continue;
                    }
                    let holder = equations.graph.exact(access.pointer, budget)?.unwrap();
                    assert!(
                        equations.graph.pointer_cells[holder].is_some(),
                        "actual pointer-holder write required"
                    );
                    let mut stale_store = None;
                    for old_use in &transport.uses {
                        budget.charge_work(1)?;
                        if old_use.alias != stale
                            || old_use.operand != 1
                            || old_use.block != block.id
                        {
                            continue;
                        }
                        let Some(position) = old_use.operation else {
                            continue;
                        };
                        if position < later.gap {
                            continue;
                        }
                        let Some(write) =
                            source_address_value_access_v29(&block.operations[position])?
                        else {
                            continue;
                        };
                        if write.writing
                            && write.value == old_use.value
                            && equations.graph.exact(write.pointer, budget)? == Some(holder)
                        {
                            stale_store = Some(position);
                        }
                    }
                    let Some(stale_store) = stale_store else {
                        continue;
                    };
                    let mut loaded_then_dereferenced = false;
                    for (read_index, candidate) in block
                        .operations
                        .iter()
                        .enumerate()
                        .take(later.gap)
                        .skip(operation + 1)
                    {
                        budget.charge_work(1)?;
                        let Some(read) = source_address_value_access_v29(candidate)? else {
                            continue;
                        };
                        if read.writing
                            || equations.graph.exact(read.pointer, budget)? != Some(holder)
                            || !matches!(equations.graph.ty(read.value, budget)?, Type::Pointer(_))
                        {
                            continue;
                        }
                        for candidate in &block.operations[read_index + 1..later.gap] {
                            budget.charge_work(1)?;
                            if source_address_value_access_v29(candidate)?
                                .is_some_and(|access| access.pointer == read.value)
                            {
                                budget.charge_work(transport.uses.len())?;
                                assert!(
                                    !transport.uses.iter().any(|usage| usage.value == read.value),
                                    "holder Load, not an alias override, must supply dereference currentness"
                                );
                                loaded_then_dereferenced = true;
                            }
                        }
                    }
                    assert!(
                        loaded_then_dereferenced && stale_store >= later.gap,
                        "fresh write/load/dereference must precede later birth/write and the actual backedge"
                    );
                    // B is valid at its real late store, crosses the backedge,
                    // then the original live boundary expires it before A.
                    // B is also undefined on first entry: this is a genuine
                    // stale-path witness, not a second-iteration-only oracle.
                    with_canonical_call_scratch_v1(budget, |budget| {
                        let mut uses = emission_vec_v1(transport.uses.len(), budget)?;
                        budget.charge_work(transport.uses.len())?;
                        uses.extend_from_slice(&transport.uses);
                        uses[index].alias = stale;
                        let forged = SourceAddressAliasTransportV29 {
                            uses: &uses,
                            ..transport.equations()
                        };
                        assert_eq!(uses[index].value, transport.aliases[stale].pointer);
                        forged.validate(
                            equations.graph,
                            equations.slots,
                            equations.lifetimes,
                            equations.kills,
                            budget,
                        )?;
                        let error = equations.check(forged, budget).unwrap_err();
                        assert!(
                            matches!(
                                error,
                                ProductionSemanticKirErrorV1::Unsupported {
                                    detail: "physical raw access crosses a storage activation or unresolved alias",
                                    ..
                                }
                            ),
                            "{error:?}"
                        );
                        Ok(())
                    })?;
                    challenged += 1;
                }
            }
        }
        Ok(challenged)
    })
}

fn event(
    segment: usize,
    gap: usize,
    phase: usize,
    order: usize,
    kind: SourceAddressBoundaryKindV29,
) -> OptimizedAliasBoundaryV18 {
    OptimizedAliasBoundaryV18 {
        actual: SourceAddressBoundaryEventV29 {
            block: BlockId(3),
            gap: 0,
            kind,
        },
        order: [segment, gap, phase, order],
    }
}

fn lifetime(sequence: usize, live: bool) -> SourceAddressLifetimeV29 {
    SourceAddressLifetimeV29 {
        block: BlockId(3),
        gap: 0,
        sequence,
        slot: 0,
        live,
    }
}

// These rows are inert ordering inputs. Genuine optimized-source formation is
// exercised separately by the Stored/FreshRestart production consumer tests.
#[test]
fn optimized_alias_order_follows_merged_segments_and_interleaves_lifetime_restarts() {
    let mut ordered = [
        event(1, 0, 0, 1, SourceAddressBoundaryKindV29::Alias(1)),
        event(0, 7, 1, 5, SourceAddressBoundaryKindV29::Lifetime(1)),
        event(1, 1, 0, 2, SourceAddressBoundaryKindV29::Alias(2)),
        event(0, 6, 0, 0, SourceAddressBoundaryKindV29::Alias(0)),
        event(0, 7, 1, 6, SourceAddressBoundaryKindV29::Lifetime(2)),
        event(1, 1, 1, 0, SourceAddressBoundaryKindV29::Lifetime(0)),
    ];
    let mut lifetimes = [lifetime(0, false), lifetime(1, false), lifetime(2, true)];
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    let actual =
        optimized_alias_order_boundaries_v18(&mut ordered, &mut lifetimes, &[], &mut budget)
            .unwrap();
    assert!(matches!(
        actual.as_slice(),
        [
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Alias(0),
                ..
            },
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Lifetime(0),
                ..
            },
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Lifetime(1),
                ..
            },
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Alias(1),
                ..
            },
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Alias(2),
                ..
            },
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Lifetime(2),
                ..
            },
        ]
    ));
    assert_eq!(
        lifetimes,
        [lifetime(0, false), lifetime(1, true), lifetime(2, false)]
    );
}

#[test]
fn optimized_alias_order_rejects_duplicate_keys_changed_points_and_missing_kills() {
    for fault in 0..4 {
        let mut ordered = [
            event(0, 1, 2, 0, SourceAddressBoundaryKindV29::Kill(0)),
            event(0, 1, 2, 1, SourceAddressBoundaryKindV29::Kill(1)),
        ];
        let kills = [
            SourceAddressKillV29 {
                block: BlockId(3),
                gap: 0,
                slot: 0,
            },
            SourceAddressKillV29 {
                block: BlockId(3),
                gap: 0,
                slot: 1,
            },
        ];
        let expected = match fault {
            0 => {
                ordered[1].order = ordered[0].order;
                "logical alias source boundary order is not unique"
            }
            1 => {
                ordered[1].actual.kind = SourceAddressBoundaryKindV29::Kill(0);
                "logical alias source transition coverage changed"
            }
            2 => {
                ordered[0].actual.block = BlockId(1);
                "logical alias kill changed output position"
            }
            3 => {
                ordered[1].actual.kind = SourceAddressBoundaryKindV29::Alias(0);
                "logical alias source transition coverage changed"
            }
            _ => unreachable!(),
        };
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
        let result =
            optimized_alias_order_boundaries_v18(&mut ordered, &mut [], &kills, &mut budget);
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(detail)) if detail == expected),
            "fault {fault}"
        );
    }
}

#[test]
fn optimized_alias_order_canonicalizes_source_kill_reversal_and_repeated_clears() {
    let mut ordered = [
        event(0, 1, 2, 1, SourceAddressBoundaryKindV29::Kill(1)),
        event(0, 2, 0, 0, SourceAddressBoundaryKindV29::Alias(0)),
        event(0, 3, 1, 0, SourceAddressBoundaryKindV29::Lifetime(0)),
        event(1, 0, 2, 0, SourceAddressBoundaryKindV29::Kill(0)),
        event(1, 1, 2, 1, SourceAddressBoundaryKindV29::Kill(1)),
        event(1, 2, 0, 1, SourceAddressBoundaryKindV29::Alias(1)),
    ];
    let kills = [
        SourceAddressKillV29 {
            block: BlockId(3),
            gap: 0,
            slot: 0,
        },
        SourceAddressKillV29 {
            block: BlockId(3),
            gap: 0,
            slot: 1,
        },
    ];
    let mut lifetimes = [lifetime(0, true)];
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    let actual =
        optimized_alias_order_boundaries_v18(&mut ordered, &mut lifetimes, &kills, &mut budget)
            .unwrap();
    assert!(matches!(
        actual.as_slice(),
        [
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Alias(0),
                ..
            },
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Lifetime(0),
                ..
            },
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Alias(1),
                ..
            },
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Kill(0),
                ..
            },
            SourceAddressBoundaryEventV29 {
                kind: SourceAddressBoundaryKindV29::Kill(1),
                ..
            },
        ]
    ));
    assert_eq!(
        budget.storage(),
        actual.capacity() * size_of::<SourceAddressBoundaryEventV29>()
    );
}

#[test]
fn optimized_alias_kill_coverage_refuses_foreign_missing_duplicate_and_cross_gap_keys() {
    for fault in 0..5 {
        let mut ordered = [event(0, 1, 2, 0, SourceAddressBoundaryKindV29::Kill(0))];
        let mut kills = vec![SourceAddressKillV29 {
            block: BlockId(3),
            gap: 0,
            slot: 0,
        }];
        let expected = match fault {
            0 => {
                ordered[0].actual.kind = SourceAddressBoundaryKindV29::Kill(1);
                "logical alias source kill index"
            }
            1 => {
                kills.push(kills[0]);
                "logical alias physical kills are not canonical"
            }
            2 => {
                kills[0].gap = 1;
                "logical alias kill changed output position"
            }
            3 => {
                kills.push(SourceAddressKillV29 {
                    block: BlockId(4),
                    gap: 0,
                    slot: 0,
                });
                "logical alias source transition coverage changed"
            }
            4 => {
                ordered[0].actual.kind = SourceAddressBoundaryKindV29::Alias(0);
                "logical alias source transition coverage changed"
            }
            _ => unreachable!(),
        };
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
        let result =
            optimized_alias_order_boundaries_v18(&mut ordered, &mut [], &kills, &mut budget);
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(detail)) if detail == expected),
            "fault {fault}"
        );
    }
}

#[test]
fn optimized_alias_collapsed_kill_scratch_has_exact_and_one_short_resource_boundaries() {
    let run = |work_limit, storage_limit| {
        let mut ordered = [
            event(0, 0, 2, 0, SourceAddressBoundaryKindV29::Kill(0)),
            event(0, 1, 0, 0, SourceAddressBoundaryKindV29::Alias(0)),
            event(1, 0, 2, 0, SourceAddressBoundaryKindV29::Kill(0)),
        ];
        let kills = [SourceAddressKillV29 {
            block: BlockId(3),
            gap: 0,
            slot: 0,
        }];
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(41).unwrap();
        let result =
            optimized_alias_order_boundaries_v18(&mut ordered, &mut [], &kills, &mut budget).map(
                |events| {
                    assert_eq!(events.len(), 2);
                    assert_eq!(
                        budget.storage(),
                        41 + events.capacity() * size_of::<SourceAddressBoundaryEventV29>()
                    );
                    drop(events);
                },
            );
        // Inert helper scope owns its scratch on error and returned rows on
        // success. Production uses the existing checked-source lexical lease.
        budget.release_storage(budget.storage() - 41).unwrap();
        assert_eq!(budget.storage(), 41);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, work, storage) = run(100_000, 100_000);
    result.unwrap();
    let (result, seen_work, seen_storage) = run(work, storage);
    result.unwrap();
    assert_eq!((seen_work, seen_storage), (work, storage));
    assert!(matches!(
        run(work - 1, storage).0,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Work(_)
        ))
    ));
    assert!(matches!(
        run(work, storage - 1).0,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
    // Independent first-allocation oracle, not derived from a successful run.
    let header = size_of::<Vec<u8>>() + size_of::<Option<(BlockId, usize)>>();
    let (result, work, _) = run(100_000, 41 + header - 1);
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
    assert_eq!(work, 0);
}

#[test]
fn optimized_alias_collapsed_kill_ordering_work_is_subquadratic() {
    for count in [16_usize, 64, 256, 4096] {
        let mut ordered = (0..count)
            .rev()
            .map(|index| event(index, 0, 2, 0, SourceAddressBoundaryKindV29::Kill(0)))
            .collect::<Vec<_>>();
        let kills = [SourceAddressKillV29 {
            block: BlockId(3),
            gap: 0,
            slot: 0,
        }];
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000_000);
        let events =
            optimized_alias_order_boundaries_v18(&mut ordered, &mut [], &kills, &mut budget)
                .unwrap();
        assert_eq!(events.len(), 1);
        let log = count.ilog2() as usize + 1;
        assert!(
            budget.work() <= 64 * count * log + 64,
            "constructor and complete indexed coverage work: N={count}, work={}",
            budget.work()
        );
    }
}

#[test]
fn optimized_alias_transport_live_headers_have_an_independent_shape_mirror() {
    struct TransportShape {
        aliases: Vec<SourceAddressLogicalAliasV29>,
        uses: Vec<SourceAddressLogicalUseV29>,
        boundaries: Vec<SourceAddressBoundaryEventV29>,
    }
    struct MeterShape<'a, 'g, 'b, 'w> {
        source: &'a ProductionSourceOwnedViewV18<'g>,
        budget: &'b mut ArgumentBudgetV1<'w>,
    }
    struct BoundaryShape {
        actual: SourceAddressBoundaryEventV29,
        order: [usize; 4],
    }
    #[allow(dead_code)]
    struct EquationShape<'a, 'g> {
        function: &'a Function,
        graph: &'a SourceAddressMemoryV29<'g>,
        slots: &'a [ScopedSourceSlotV29],
        accesses: &'a [SourceAddressAccessV29],
        kills: &'a [SourceAddressKillV29],
        initial: &'a [bool],
        lifetimes: &'a [SourceAddressLifetimeV29],
        births: &'a [SourceAddressBirthV29],
        failures: &'a [SourceIndexFailureV29],
        geometry: SourceAddressGeometryV29,
    }
    #[allow(dead_code)]
    fn keep_shapes_live(
        row: TransportShape,
        meter: MeterShape<'_, '_, '_, '_>,
        boundary: BoundaryShape,
    ) {
        let _ = (
            row.aliases,
            row.uses,
            row.boundaries,
            meter.source,
            meter.budget,
            boundary.actual,
            boundary.order,
        );
    }
    let expected = size_of::<TransportShape>()
        + size_of::<EquationShape<'_, '_>>()
        + 7 * size_of::<Vec<usize>>()
        + size_of::<MeterShape<'_, '_, '_, '_>>()
        + size_of::<(
            Vec<Option<OptimizedAliasSegmentV18>>,
            Vec<OptimizedAliasUseJoinV18>,
        )>()
        + size_of::<
            SourceOwnedResultV18<(
                Vec<Option<OptimizedAliasSegmentV18>>,
                Vec<OptimizedAliasUseJoinV18>,
            )>,
        >()
        + size_of::<SourceOwnedResultV18<TransportShape>>()
        + size_of::<SourceOwnedResultV18<Option<(AliasDefinitionV18, ValueId)>>>()
        + size_of::<SourceOwnedResultV18<(AliasBlockV18, usize, Option<(BlockId, usize)>)>>()
        + size_of::<BoundaryShape>()
        + size_of::<Option<[usize; 6]>>()
        + size_of::<[usize; 3]>()
        + size_of::<Result<usize, usize>>();
    assert_eq!(optimized_alias_transport_headers_v18().unwrap(), expected);
    for short in [0, 1] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, 41 + expected - short);
        budget.reserve_storage(41).unwrap();
        let result = budget.reserve_storage(optimized_alias_transport_headers_v18().unwrap());
        if short == 0 {
            result.unwrap();
            assert_eq!(budget.storage(), 41 + expected);
        } else {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(_))));
            assert_eq!(budget.storage(), 41);
        }
    }
}
