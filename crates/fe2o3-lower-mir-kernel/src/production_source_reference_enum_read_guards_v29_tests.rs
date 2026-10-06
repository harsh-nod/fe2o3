use super::*;

#[derive(Clone, Copy, Debug)]
enum PayloadUse {
    Copy,
    Move,
    SharedBorrow,
    SharedDereference,
    MutableBorrow,
    RawDereference,
}

#[derive(Clone, Copy, Debug)]
enum ReadEdges {
    Distinct,
    DistinctReversed,
    ValueAndOtherwise,
    TwoValuesAndOtherwise,
}

fn payload_use_owner(kind: PayloadUse, edges: ReadEdges) -> ProductionSemanticSsaOwnerV1 {
    let original = enum_owner(EnumCase::ConditionalVariants);
    let source = original.source_semantic();
    owner_with(Case::Shared, |types, functions| {
        types.push(source.types()[ENUM.index() as usize].clone());
        let original = &source.functions()[2];
        let mut locals = original.locals().to_vec();
        let pointer = if matches!(kind, PayloadUse::MutableBorrow | PayloadUse::RawDereference) {
            let raw = matches!(kind, PayloadUse::RawDereference);
            let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
            let mut declaration = SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([179; 32]),
                SemanticLayoutIdentityV1::from_sha256([179; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(
                            if raw { 0 } else { 1 },
                            u64::MAX.into(),
                        ),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        WORD,
                        if raw {
                            SemanticPointerKindV1::Raw
                        } else {
                            SemanticPointerKindV1::Reference
                        },
                        SemanticMutabilityV1::Mutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            );
            if !raw {
                declaration = declaration.with_rustc_abi_properties(
                    SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                        Some(
                            SemanticAbiPointeeInfoV1::new(
                                SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                                8,
                                8,
                            )
                            .unwrap(),
                        ),
                        None,
                    ),
                );
            }
            types.push(declaration);
            pointer
        } else {
            REFERENCE
        };
        if !matches!(kind, PayloadUse::Copy | PayloadUse::Move) {
            locals.push(local(179, pointer, SemanticLocalRoleV1::Temporary));
        }
        let mut blocks = original.blocks().to_vec();
        for (index, variant) in [(4, 0), (5, 1)] {
            let field = enum_field(5, variant, 1);
            let mut statements = match kind {
                PayloadUse::Copy => vec![assign(
                    place(8, WORD),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
                )],
                PayloadUse::Move => vec![assign(
                    place(8, WORD),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field)),
                )],
                PayloadUse::SharedBorrow
                | PayloadUse::SharedDereference
                | PayloadUse::MutableBorrow => vec![assign(
                    place(9, pointer),
                    SemanticRvalueKindV1::Borrow {
                        kind: if matches!(
                            kind,
                            PayloadUse::SharedBorrow | PayloadUse::SharedDereference
                        ) {
                            SemanticBorrowKindV1::Shared
                        } else {
                            SemanticBorrowKindV1::Mutable
                        },
                        place: field,
                    },
                )],
                PayloadUse::RawDereference => vec![
                    assign(
                        place(9, pointer),
                        SemanticRvalueKindV1::AddressOf {
                            mutability: SemanticMutabilityV1::Mutable,
                            place: field,
                        },
                    ),
                    assign(
                        place(8, WORD),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                            9,
                            &[(SemanticProjectionKindV1::Dereference, WORD)],
                        ))),
                    ),
                ],
            };
            if matches!(kind, PayloadUse::SharedDereference) {
                statements.push(assign(
                    place(8, WORD),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                        9,
                        &[(SemanticProjectionKindV1::Dereference, WORD)],
                    ))),
                ));
            }
            statements.push(unit());
            blocks[index] = block(
                150 + index as u8,
                statements,
                SemanticTerminatorKindV1::Return,
            );
        }
        if matches!(edges, ReadEdges::DistinctReversed) {
            blocks[3] = block(
                153,
                original.blocks()[3].statements().to_vec(),
                switch(6, TAGS[1], 5, 4),
            );
        } else if !matches!(edges, ReadEdges::Distinct) {
            let mut values = vec![SemanticSwitchTargetV1::new(
                TAGS[0],
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(4),
                ),
            )];
            if matches!(edges, ReadEdges::TwoValuesAndOtherwise) {
                values.push(SemanticSwitchTargetV1::new(
                    TAGS[1],
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchValue,
                        SemanticBlockIdV1::from_index(4),
                    ),
                ));
            }
            blocks[3] = block(
                153,
                original.blocks()[3].statements().to_vec(),
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(6, WORD)),
                    targets: SemanticSwitchTargetsV1::new(
                        values,
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(4),
                        ),
                    )
                    .unwrap(),
                },
            );
            blocks.pop();
        }
        functions[2] = function(30, false, CAPTURE, locals, blocks);
    })
}

#[test]
fn actual_payload_uses_require_their_original_edge_not_just_the_destination_block() {
    for kind in [
        PayloadUse::Copy,
        PayloadUse::Move,
        PayloadUse::SharedBorrow,
        PayloadUse::SharedDereference,
        PayloadUse::MutableBorrow,
        PayloadUse::RawDereference,
    ] {
        for edges in [
            ReadEdges::Distinct,
            ReadEdges::DistinctReversed,
            ReadEdges::ValueAndOtherwise,
            ReadEdges::TwoValuesAndOtherwise,
        ] {
            let owner = payload_use_owner(kind, edges);
            let reached = std::cell::Cell::new(false);
            let result = run_enum_with_original_demands(owner, |plan, _| {
                assert_eq!(
                    plan.instances
                        .instances()
                        .iter()
                        .filter(|instance| instance.function().index() == 2)
                        .count(),
                    2
                );
                reached.set(true);
                Ok(())
            });
            let accepted = matches!(edges, ReadEdges::Distinct | ReadEdges::DistinctReversed);
            assert_eq!(result.is_ok(), accepted, "{kind:?}, {edges:?}: {result:?}");
            if !accepted {
                assert!(
                    format!("{:?}", result.as_ref().unwrap_err())
                        .contains("source reference reads an uninitialized partial holder")
                );
            }
            assert_eq!(
                reached.get(),
                accepted,
                "an unguarded original payload must not reach the callback"
            );
        }
    }
}

#[test]
fn existing_enum_source_negatives_are_not_repaired_by_conditional_initialization() {
    for case in [
        EnumCase::DuplicateTarget,
        EnumCase::SavedDiscriminator,
        EnumCase::LiveAlternativeDies,
    ] {
        let reached = std::cell::Cell::new(false);
        let result = run_enum(case, |_, _| {
            reached.set(true);
            Ok(())
        });
        assert!(result.is_err(), "{case:?}");
        assert!(!reached.get(), "{case:?}");
    }
}

fn indexed_read_owner(dynamic: bool, payload: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = enum_owner(EnumCase::Construct(0));
    let source = original.source_semantic();
    owner_with(Case::Shared, |types, functions| {
        types.push(source.types()[ENUM.index() as usize].clone());
        let array = SemanticTypeIdV1::from_index(types.len() as u32);
        let element = if payload { ENUM } else { WORD };
        let stride = if payload { 24 } else { 8 };
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([180; 32]),
            SemanticLayoutIdentityV1::from_sha256([180; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                2 * stride,
                8,
                SemanticFieldsShapeV1::array(stride, 2),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array { element, length: 2 },
        ));
        let original = &source.functions()[2];
        let mut locals = original.locals().to_vec();
        locals.push(local(180, array, SemanticLocalRoleV1::Temporary));
        let mut initial = original.blocks()[0].statements().to_vec();
        initial.push(assign(
            place(9, array),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Array,
                    vec![
                        SemanticOperandV1::Copy(place(if payload { 5 } else { 3 }, element)),
                        SemanticOperandV1::Copy(place(if payload { 5 } else { 3 }, element)),
                    ],
                )
                .unwrap(),
            ),
        ));
        let mut projections = vec![(
            if dynamic {
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(3))
            } else {
                SemanticProjectionKindV1::ConstantIndex {
                    offset: 0,
                    minimum_length: 2,
                    from_end: false,
                }
            },
            element,
        )];
        if payload {
            projections.extend([
                (SemanticProjectionKindV1::Downcast(0), ENUM),
                (SemanticProjectionKindV1::Field(1), WORD),
            ]);
        }
        let indexed = projected(9, &projections);
        if !payload {
            initial.push(assign(
                indexed.clone(),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, WORD))),
            ));
        }
        functions[2] = function(
            30,
            false,
            CAPTURE,
            locals,
            vec![
                block(180, initial, go(1)),
                block(
                    181,
                    vec![
                        assign(
                            place(8, WORD),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(indexed)),
                        ),
                        unit(),
                    ],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
    })
}

#[test]
fn actual_indexed_enum_payload_keeps_guarded_alias_refusal() {
    for dynamic in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result =
            run_enum_with_original_demands(indexed_read_owner(dynamic, true), |plan, _| {
                assert!(plan.selectors.is_empty());
                reached.set(true);
                Ok(())
            });
        if dynamic {
            assert!(format!("{:?}", result.unwrap_err()).contains(
                "source dynamic enum or union selection needs guarded alias correspondence"
            ));
        } else {
            result.unwrap();
        }
        assert_eq!(reached.get(), !dynamic);
    }
}

#[test]
fn selected_read_wrapper_requires_the_original_selector_place_site_and_instance() {
    for mutation in 0..4 {
        let owner = indexed_read_owner(true, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(71).unwrap();
        let demands =
            source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)
                .unwrap();
        let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
            &owner,
            demands.types(&owner, &mut budget).unwrap(),
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
            &mut budget,
        )
        .unwrap();
        let reached = std::cell::Cell::new(false);
        let result = production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let lens = demands.root_lens(&owner, 0, budget).unwrap();
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                    source_storage_v29::with_source_storage_descriptor_demands_root_v29(
                        &mut layouts,
                        instances,
                        None,
                        lens,
                        budget,
                        |plan, root, budget| {
                            assert_eq!(plan.selectors.len(), 4);
                            let reads = plan
                                .selectors
                                .iter()
                                .filter(|row| {
                                    let occurrences = instances.occurrences(row.instance).unwrap();
                                    let event = &occurrences.events()[row.occurrence];
                                    scoped_memory_site_key_v29(event.site()).0 == 1
                                })
                                .copied()
                                .collect::<Vec<_>>();
                            assert_eq!(reads.len(), 2);
                            let row = reads[0];
                            let source = row.check(instances, budget)?;
                            assert_eq!(source.local().index(), 9);
                            let occurrences = instances.occurrences(row.instance).unwrap();
                            let event = &occurrences.events()[row.occurrence];
                            let (block, statement) = scoped_memory_site_key_v29(event.site());
                            let site = SourceReferenceSiteV29 {
                                instance: row.instance,
                                block: SemanticBlockIdV1::from_index(block),
                                statement: statement.map(|value| value as usize),
                            };
                            let entry = plan
                                .blocks
                                .iter()
                                .find(|entry| {
                                    entry.instance == row.instance && entry.block == site.block
                                })
                                .unwrap();
                            let snapshot = plan.storage_snapshots
                                [plan.states[entry.entry][9].storage.unwrap()];
                            let context = Some((site, source as *const SemanticPlaceV1 as usize));
                            assert!(root.snapshot_selected_readable(
                                snapshot,
                                source.projections(),
                                plan,
                                context,
                                budget
                            )?);
                            let copy = source.clone();
                            let changed = match mutation {
                                0 => context,
                                1 => None,
                                2 => Some((site, &copy as *const SemanticPlaceV1 as usize)),
                                _ => Some((
                                    SourceReferenceSiteV29 {
                                        instance: reads[1].instance,
                                        ..site
                                    },
                                    source as *const SemanticPlaceV1 as usize,
                                )),
                            };
                            let result = root.snapshot_selected_readable(
                                snapshot,
                                source.projections(),
                                plan,
                                changed,
                                budget,
                            );
                            reached.set(true);
                            if mutation == 0 {
                                assert!(result?);
                            } else {
                                assert!(result.is_err());
                            }
                            Ok(())
                        },
                    ),
                )
            },
        )
        .unwrap();
        assert!(reached.get());
        assert_eq!(result.is_err(), mutation != 0);
        let cleanup = layouts.release(&mut budget);
        assert_eq!(
            format!("{:?}", cleanup.err()),
            format!("{:?}", result.err())
        );
        demands.discard(&mut budget).unwrap();
        assert_eq!(budget.storage(), 71);
    }
}

#[test]
fn actual_snapshot_read_wrappers_share_original_custody_and_cleanup() {
    for mode in 0..4 {
        let owner = enum_owner(EnumCase::Construct(0));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(83).unwrap();
        let demands =
            source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)
                .unwrap();
        let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
            &owner,
            demands.types(&owner, &mut budget).unwrap(),
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
            &mut budget,
        )
        .unwrap();
        let reached = std::cell::Cell::new(false);
        let result = production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let lens = demands.root_lens(&owner, 0, budget).unwrap();
                let result = source_storage_v29::with_source_storage_descriptor_demands_root_v29(
                    &mut layouts,
                    instances,
                    None,
                    lens,
                    budget,
                    |plan, root, budget| {
                        let mut checked = 0;
                        let mut first_snapshot = None;
                        for block in &plan.blocks {
                            if block.block.index() != 1
                                || instances
                                    .instance(block.instance)
                                    .unwrap()
                                    .function()
                                    .index()
                                    != 2
                            {
                                continue;
                            }
                            let snapshot = plan.storage_snapshots
                                [plan.states[block.entry][5].storage.unwrap()];
                            let field = enum_field(5, 0, 1);
                            assert!(root.snapshot_readable(
                                snapshot,
                                field.projections(),
                                budget
                            )?);
                            assert!(root.snapshot_selected_readable(
                                snapshot,
                                field.projections(),
                                plan,
                                None,
                                budget
                            )?);
                            let wrong = enum_field(5, 1, 1);
                            assert!(!root.snapshot_readable(
                                snapshot,
                                wrong.projections(),
                                budget
                            )?);
                            assert!(!root.snapshot_selected_readable(
                                snapshot,
                                wrong.projections(),
                                plan,
                                None,
                                budget
                            )?);
                            first_snapshot.get_or_insert(snapshot);
                            checked += 1;
                        }
                        assert_eq!(checked, 2);
                        reached.set(true);
                        if mode == 3 {
                            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
                            foreign.reserve_storage(29).unwrap();
                            let field = enum_field(5, 0, 1);
                            assert!(
                                root.snapshot_readable(
                                    first_snapshot.unwrap(),
                                    field.projections(),
                                    &mut foreign
                                )
                                .is_err()
                            );
                            assert_eq!(foreign.storage(), 29);
                            assert_eq!(foreign.work(), 0);
                            foreign.release_storage(29).unwrap();
                        }
                        match mode {
                            1 => {
                                Err(source_reference_error_v29("read wrapper cleanup sentinel")
                                    .into())
                            }
                            2 => panic!("read wrapper cleanup panic"),
                            _ => Ok(()),
                        }
                    },
                );
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
            },
        )
        .unwrap();
        assert!(reached.get());
        assert_eq!(result.is_err(), mode != 0);
        let expected = match mode {
            0 => None,
            1 => Some(source_reference_error_v29("read wrapper cleanup sentinel")),
            2 => Some(source_reference_error_v29(
                "source reference callback panicked",
            )),
            _ => Some(ProductionSemanticKirErrorV1::from(
                ArgumentResourceV1::Accounting,
            )),
        };
        assert_eq!(format!("{:?}", result.err()), format!("{expected:?}"));
        let cleanup = layouts.release(&mut budget);
        assert_eq!(
            format!("{:?}", cleanup.err()),
            format!("{expected:?}"),
            "owner cleanup must preserve the first original callback or query diagnostic"
        );
        demands.discard(&mut budget).unwrap();
        assert_eq!(
            budget.storage(),
            83,
            "root exits retain no scratch or schema credit after owner release"
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum RawPayloadChange {
    Overwrite,
    Tag,
    Move,
    Deinitialize,
    Reinitialize,
    SiblingDeinitialize,
    Restart,
}

fn changed_raw_payload_owner(change: RawPayloadChange) -> ProductionSemanticSsaOwnerV1 {
    let original = payload_use_owner(PayloadUse::RawDereference, ReadEdges::Distinct);
    let semantic = original.source_semantic();
    owner_with(Case::Shared, |types, functions| {
        *types = semantic.types().to_vec();
        *functions = semantic.functions().to_vec();
        let original = &semantic.functions()[2];
        let mut blocks = original.blocks().to_vec();
        for (index, variant) in [(4, 0), (5, 1)] {
            let mut statements = vec![original.blocks()[index].statements()[0].clone()];
            let field = enum_field(5, variant, 1);
            match change {
                RawPayloadChange::Overwrite => statements.push(assign(
                    field.clone(),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            WORD,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(91, 8).unwrap(),
                            ),
                        ),
                    )),
                )),
                RawPayloadChange::Tag => statements.push(SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::SetDiscriminant {
                        place: place(5, ENUM),
                        variant_index: 2,
                    },
                )),
                RawPayloadChange::Move => statements.push(assign(
                    place(8, WORD),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field.clone())),
                )),
                RawPayloadChange::Deinitialize | RawPayloadChange::Reinitialize => {
                    statements.push(SemanticStatementV1::new(
                        source(),
                        SemanticStatementKindV1::Deinitialize(field.clone()),
                    ));
                    if matches!(change, RawPayloadChange::Reinitialize) {
                        statements.push(assign(
                            field.clone(),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, WORD))),
                        ));
                    }
                }
                RawPayloadChange::SiblingDeinitialize => statements.push(SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::Deinitialize(enum_field(5, variant, 0)),
                )),
                RawPayloadChange::Restart => {
                    statements.push(dead(5));
                    statements.push(SemanticStatementV1::new(
                        source(),
                        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(5)),
                    ));
                    statements.push(construct(variant, 2));
                }
            }
            statements.extend_from_slice(&original.blocks()[index].statements()[1..]);
            blocks[index] = block(
                150 + index as u8,
                statements,
                SemanticTerminatorKindV1::Return,
            );
        }
        functions[2] = function(30, false, CAPTURE, original.locals().to_vec(), blocks);
    })
}

#[test]
fn raw_enum_payload_replay_observes_current_updates_and_not_the_formation_value() {
    for change in [
        RawPayloadChange::Overwrite,
        RawPayloadChange::Reinitialize,
        RawPayloadChange::SiblingDeinitialize,
    ] {
        let reached = std::cell::Cell::new(false);
        let result =
            run_enum_with_original_demands(changed_raw_payload_owner(change), |plan, budget| {
                let mut accesses = 0;
                for access in plan.raw_accesses.values() {
                    let range = plan.raw_projection_range(access.set, WORD, budget)?;
                    assert_eq!(range.len(), 2);
                    assert!(matches!(
                        plan.projections[range.start].kind(),
                        SemanticProjectionKindV1::Downcast(0 | 1)
                    ));
                    assert_eq!(
                        plan.projections[range.start + 1].kind(),
                        SemanticProjectionKindV1::Field(1)
                    );
                    assert_eq!(plan.raw_sets[access.set].local.index(), 5);
                    accesses += 1;
                }
                assert_eq!(
                    accesses, 4,
                    "two guarded branches in each original helper instance"
                );
                reached.set(true);
                Ok(())
            });
        assert!(result.is_ok(), "{change:?}: {result:?}");
        assert!(reached.get());
    }
}

#[test]
fn raw_enum_address_does_not_keep_old_tags_payloads_or_activations_alive() {
    for change in [
        RawPayloadChange::Tag,
        RawPayloadChange::Move,
        RawPayloadChange::Deinitialize,
        RawPayloadChange::Restart,
    ] {
        let reached = std::cell::Cell::new(false);
        let result = run_enum_with_original_demands(changed_raw_payload_owner(change), |_, _| {
            reached.set(true);
            Ok(())
        });
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ),
            "{change:?}: {result:?}"
        );
        assert!(!reached.get());
        if matches!(change, RawPayloadChange::Restart) {
            assert!(
                format!("{result:?}")
                    .contains("source raw pointer outlived its storage activation")
            );
        }
    }
}

fn indexed_raw_payload_owner(dynamic: bool, payload: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = indexed_read_owner(dynamic, payload);
    let source = original.source_semantic();
    let raw_owner = payload_use_owner(PayloadUse::RawDereference, ReadEdges::Distinct);
    let raw_type = raw_owner.source_semantic().types().last().unwrap().clone();
    owner_with(Case::Shared, |types, functions| {
        *types = source.types().to_vec();
        *functions = source.functions().to_vec();
        let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([181; 32]),
                SemanticLayoutIdentityV1::from_sha256([181; 32]),
                raw_type.layout().clone(),
                raw_type.shape().clone(),
            )
            .with_rustc_abi_properties(raw_type.abi_properties()),
        );
        let original = &source.functions()[2];
        let mut locals = original.locals().to_vec();
        locals.push(local(181, pointer, SemanticLocalRoleV1::Temporary));
        let mut blocks = original.blocks().to_vec();
        let SemanticStatementKindV1::Assign(read) = blocks[1].statements()[0].kind() else {
            panic!("original indexed read");
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(target)) = read.value().kind() else {
            panic!("original indexed source place");
        };
        blocks[1] = block(
            181,
            vec![
                assign(
                    place(10, pointer),
                    SemanticRvalueKindV1::AddressOf {
                        mutability: SemanticMutabilityV1::Mutable,
                        place: target.clone(),
                    },
                ),
                assign(
                    place(8, WORD),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                        10,
                        &[(SemanticProjectionKindV1::Dereference, WORD)],
                    ))),
                ),
                unit(),
            ],
            SemanticTerminatorKindV1::Return,
        );
        functions[2] = function(30, false, CAPTURE, locals, blocks);
    })
}

#[test]
fn raw_current_projection_handles_nested_static_arrays_and_keeps_selector_obligation() {
    for payload in [false, true] {
        let result = run_enum_with_original_demands(
            indexed_raw_payload_owner(false, payload),
            |plan, budget| {
                assert_eq!(plan.raw_accesses.len(), 2);
                for access in plan.raw_accesses.values() {
                    let range = plan.raw_projection_range(access.set, WORD, budget)?;
                    assert_eq!(range.len(), if payload { 3 } else { 1 });
                    assert!(matches!(
                        plan.projections[range.start].kind(),
                        SemanticProjectionKindV1::ConstantIndex { .. }
                    ));
                }
                Ok(())
            },
        );
        assert!(result.is_ok(), "{payload}: {result:?}");
    }
    let reached = std::cell::Cell::new(false);
    let result = run_enum_with_original_demands(indexed_raw_payload_owner(true, false), |_, _| {
        reached.set(true);
        Ok(())
    });
    assert!(
        format!("{result:?}")
            .contains("source raw selected address requires correlated selector transport")
    );
    assert!(!reached.get());
}
