use super::*;

#[path = "production_source_enum_helper_abi_v55_tests.rs"]
mod helper_abi_tests;

#[path = "production_source_enum_tag_transport_v55_tests.rs"]
mod tag_transport_tests;

#[path = "production_source_reference_enum_resources_v29_tests.rs"]
mod resource_tests;

#[path = "production_source_reference_enum_slot_demands_v29_tests.rs"]
mod slot_demand_tests;

#[path = "production_source_reference_enum_read_guards_v29_tests.rs"]
mod read_guard_tests;

#[path = "production_source_enum_checked_read_v58_tests.rs"]
mod checked_read_tests;

#[path = "production_source_reference_owned_types_source_v29_tests.rs"]
mod owned_type_tests;

const ENUM: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const TAGS: [u128; 3] = [3, 17, 250];

#[derive(Clone, Copy, Debug)]
enum EnumCase {
    Construct(u32),
    ProjectedConstructor,
    ProjectedTagOnly,
    MoveSibling,
    Reinitialize,
    ConditionalVariants,
    CorrelatedLoans,
    RetainedCorrelatedLoans,
    LiveAlternativeDies,
    SavedDiscriminator,
    DuplicateTarget,
    TagDoesNotInitialize,
}

fn enum_declaration() -> SemanticTypeDeclV1 {
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 255),
    );
    let variants = (0..3)
        .map(|index| {
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                24,
                8,
                SemanticFieldsShapeV1::arbitrary(vec![8, 16], vec![0, 1]).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                70 + u64::from(index),
                SemanticAggregateLayoutV1::new(
                    vec![8, 16],
                    vec![SemanticPaddingV1::new(1, 7).unwrap()],
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([154; 32]),
        SemanticLayoutIdentityV1::from_sha256([154; 32]),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            24,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            WORD,
            TAGS.iter()
                .map(|&tag| {
                    SemanticEnumVariantV1::new(
                        tag,
                        SemanticAggregateTypeV1::new(vec![REFERENCE, WORD]).unwrap(),
                    )
                })
                .collect(),
        )
        .unwrap(),
    )
}

fn enum_field(local: u32, variant: u32, field: u32) -> SemanticPlaceV1 {
    projected(
        local,
        &[
            (SemanticProjectionKindV1::Downcast(variant), ENUM),
            (
                SemanticProjectionKindV1::Field(field),
                if field == 0 { REFERENCE } else { WORD },
            ),
        ],
    )
}

fn construct(variant: u32, loan: u32) -> SemanticStatementV1 {
    assign(
        place(5, ENUM),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::EnumVariant(variant),
                vec![
                    SemanticOperandV1::Copy(place(loan, REFERENCE)),
                    SemanticOperandV1::Copy(place(3, WORD)),
                ],
            )
            .unwrap(),
        ),
    )
}

fn go(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::Goto,
        SemanticBlockIdV1::from_index(target),
    ))
}

fn switch(local: u32, value: u128, yes: u32, no: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(local, WORD)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                value,
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
    }
}

fn enum_owner(case: EnumCase) -> ProductionSemanticSsaOwnerV1 {
    try_enum_owner(case).unwrap()
}

fn try_enum_owner(
    case: EnumCase,
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    try_enum_owner_transform(case, |_, _| {})
}

fn try_enum_owner_transform(
    case: EnumCase,
    transform: impl FnOnce(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>),
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    try_owner_with(Case::Shared, |types, functions| {
        assert_eq!(types.len(), ENUM.index() as usize);
        types.push(enum_declaration());
        let old = &functions[2];
        let mut locals = old.locals().to_vec();
        locals.extend([
            local(155, ENUM, SemanticLocalRoleV1::Temporary),
            local(156, WORD, SemanticLocalRoleV1::Temporary),
            local(157, REFERENCE, SemanticLocalRoleV1::Temporary),
            local(158, WORD, SemanticLocalRoleV1::Temporary),
        ]);
        let mut initial = old.blocks()[0]
            .statements()
            .iter()
            .filter(|statement| {
                !matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if assignment.destination().local().index() == 0)
            })
            .cloned()
            .collect::<Vec<_>>();
        let conditional = matches!(
            case,
            EnumCase::ConditionalVariants
                | EnumCase::CorrelatedLoans
                | EnumCase::RetainedCorrelatedLoans
                | EnumCase::LiveAlternativeDies
                | EnumCase::DuplicateTarget
        );
        let blocks = if conditional {
            let different_loans = matches!(
                case,
                EnumCase::CorrelatedLoans
                    | EnumCase::RetainedCorrelatedLoans
                    | EnumCase::LiveAlternativeDies
            );
            if different_loans {
                initial.push(assign(
                    place(4, REFERENCE),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(3, WORD),
                    },
                ));
            }
            let right_variant = if different_loans { 0 } else { 1 };
            let mut joining = vec![assign(
                place(6, WORD),
                SemanticRvalueKindV1::Discriminant(place(5, ENUM)),
            )];
            if different_loans {
                joining.push(assign(
                    place(7, REFERENCE),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(enum_field(5, 0, 0))),
                ));
                if matches!(case, EnumCase::LiveAlternativeDies) {
                    joining.extend([dead(4), dead(3)]);
                }
                joining.push(unit());
            }
            let branch = |loan, variant| {
                let mut statements = vec![construct(variant, loan)];
                if matches!(case, EnumCase::RetainedCorrelatedLoans) {
                    statements.push(assign(
                        enum_field(5, variant, 1),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, WORD))),
                    ));
                }
                statements
            };
            let mut result = vec![
                block(150, initial, switch(3, 0, 1, 2)),
                block(151, branch(2, 0), go(3)),
                block(
                    152,
                    branch(if different_loans { 4 } else { 2 }, right_variant),
                    go(3),
                ),
                block(
                    153,
                    joining,
                    if different_loans {
                        SemanticTerminatorKindV1::Return
                    } else {
                        switch(
                            6,
                            TAGS[0],
                            4,
                            if matches!(case, EnumCase::DuplicateTarget) {
                                4
                            } else {
                                5
                            },
                        )
                    },
                ),
            ];
            if !different_loans {
                result.extend([
                    block(
                        154,
                        vec![
                            assign(
                                place(8, WORD),
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(enum_field(
                                    5, 0, 1,
                                ))),
                            ),
                            unit(),
                        ],
                        SemanticTerminatorKindV1::Return,
                    ),
                    block(
                        155,
                        vec![
                            assign(
                                place(8, WORD),
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(enum_field(
                                    5, 1, 1,
                                ))),
                            ),
                            unit(),
                        ],
                        SemanticTerminatorKindV1::Return,
                    ),
                ]);
                if matches!(case, EnumCase::DuplicateTarget) {
                    result.pop();
                }
            }
            result
        } else {
            let variant = if let EnumCase::Construct(variant) = case {
                variant
            } else {
                0
            };
            if matches!(
                case,
                EnumCase::ProjectedConstructor | EnumCase::ProjectedTagOnly
            ) {
                initial.push(assign(
                    enum_field(5, variant, 0),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, REFERENCE))),
                ));
                if matches!(case, EnumCase::ProjectedConstructor) {
                    initial.push(assign(
                        enum_field(5, variant, 1),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, WORD))),
                    ));
                }
                initial.push(SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::SetDiscriminant {
                        place: place(5, ENUM),
                        variant_index: variant,
                    },
                ));
            } else {
                initial.push(construct(variant, 2));
            }
            if matches!(case, EnumCase::MoveSibling | EnumCase::Reinitialize) {
                initial.push(assign(
                    place(7, REFERENCE),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(enum_field(5, variant, 0))),
                ));
                if matches!(case, EnumCase::Reinitialize) {
                    initial.push(assign(
                        enum_field(5, variant, 0),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, REFERENCE))),
                    ));
                    initial.push(assign(
                        place(7, REFERENCE),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(enum_field(
                            5, variant, 0,
                        ))),
                    ));
                }
            }
            initial.push(assign(
                place(6, WORD),
                SemanticRvalueKindV1::Discriminant(place(5, ENUM)),
            ));
            if matches!(case, EnumCase::SavedDiscriminator) {
                initial.push(construct(1, 2));
            }
            if matches!(case, EnumCase::TagDoesNotInitialize) {
                initial.extend([
                    SemanticStatementV1::new(
                        source(),
                        SemanticStatementKindV1::Deinitialize(enum_field(5, variant, 0)),
                    ),
                    SemanticStatementV1::new(
                        source(),
                        SemanticStatementKindV1::SetDiscriminant {
                            place: place(5, ENUM),
                            variant_index: variant,
                        },
                    ),
                ]);
            }
            let selected = if matches!(case, EnumCase::TagDoesNotInitialize) {
                assign(
                    place(7, REFERENCE),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(enum_field(5, variant, 0))),
                )
            } else {
                assign(
                    place(8, WORD),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(enum_field(5, variant, 1))),
                )
            };
            vec![
                block(150, initial, switch(6, TAGS[variant as usize], 1, 2)),
                block(
                    151,
                    vec![selected, unit()],
                    SemanticTerminatorKindV1::Return,
                ),
                block(152, vec![unit()], SemanticTerminatorKindV1::Return),
            ]
        };
        functions[2] = function(30, false, CAPTURE, locals, blocks);
        transform(types, functions);
    })
}

fn run_enum(
    case: EnumCase,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    super::super::cells_tests::run_cells(enum_owner(case), consume)
}

// An admitted semantic-source fixture, not a hand-built source-reference plan.
// No assignment, address formation, or incoming value supplies this enum local.
fn tag_only_activation_owner_v29(restarts: usize) -> ProductionSemanticSsaOwnerV1 {
    tag_only_activation_owner_with_loop_v29(restarts, false)
}

fn tag_only_activation_owner_with_loop_v29(
    restarts: usize,
    looped: bool,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |types, functions| {
        assert_eq!(types.len(), ENUM.index() as usize);
        let tag = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 8, 1),
            SemanticScalarValidityRangeV1::new(0, 255),
        );
        let variants = (0..3)
            .map(|index| {
                SemanticEnumVariantLayoutV1::from_rustc(
                    index,
                    1,
                    1,
                    SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    1,
                    170 + u64::from(index),
                    SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
                )
                .unwrap()
            })
            .collect();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([171; 32]),
            SemanticLayoutIdentityV1::from_sha256([171; 32]),
            SemanticTypeLayoutV1::enum_layout_with_backend_repr(
                1,
                1,
                SemanticBackendReprV1::memory(true),
                false,
                SemanticEnumLayoutV1::new(
                    variants,
                    SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::enum_type(
                WORD,
                TAGS.iter()
                    .map(|&tag| {
                        SemanticEnumVariantV1::new(
                            tag,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        )
                    })
                    .collect(),
            )
            .unwrap(),
        ));
        let mut statements = Vec::new();
        for index in 0..restarts {
            statements.extend([
                statement(SemanticStatementKindV1::StorageLive(
                    SemanticLocalIdV1::from_index(2),
                )),
                statement(SemanticStatementKindV1::SetDiscriminant {
                    place: place(2, ENUM),
                    variant_index: (index % 3) as u32,
                }),
                dead(2),
            ]);
        }
        statements.push(unit());
        let blocks = if looped {
            vec![
                block(
                    170,
                    statements,
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Constant(SemanticConstantV1::new(
                            WORD,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(0, 8).unwrap(),
                            ),
                        )),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                SemanticControlFlowEdgeV1::new(
                                    SemanticEdgeRoleV1::SwitchValue,
                                    SemanticBlockIdV1::from_index(0),
                                ),
                            )],
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::SwitchOtherwise,
                                SemanticBlockIdV1::from_index(1),
                            ),
                        )
                        .unwrap(),
                    },
                ),
                block(171, vec![unit()], SemanticTerminatorKindV1::Return),
            ]
        } else {
            vec![block(170, statements, SemanticTerminatorKindV1::Return)]
        };
        functions[2] = function(
            30,
            false,
            CAPTURE,
            vec![
                local(30, UNIT, SemanticLocalRoleV1::Return),
                local(31, CAPTURE, SemanticLocalRoleV1::Argument(0)),
                local(172, ENUM, SemanticLocalRoleV1::Temporary),
            ],
            blocks,
        );
    })
}

fn run_enum_with_original_demands(
    owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(73)?;
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )?;
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let lens = demands.root_lens(&owner, 0, budget).unwrap();
            let credit = layouts.capture_emission_credit(&owner, budget).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source_storage_v29::with_source_storage_descriptor_demands_root_v29(
                    &mut layouts,
                    instances,
                    None,
                    lens,
                    budget,
                    |plan, _, budget| consume(plan, budget).map_err(Into::into),
                )
            }));
            let scratch_cleanup = credit
                .into_root_credit(&layouts, budget)
                .ok_or_else(|| ProductionSemanticKirErrorV1::from(ArgumentResourceV1::Accounting))
                .and_then(|scratch| {
                    if !layouts.permits_root_emission_refund(&owner, scratch, budget) {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    budget.release_storage(scratch).map_err(Into::into)
                });
            let result = result.map(|original| original.and(scratch_cleanup));
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap();
    let cleanup = layouts.release(&mut budget);
    let demand_cleanup = demands.discard(&mut budget);
    assert_eq!(
        budget.storage(),
        73,
        "all activation/schema credit must settle at the caller floor"
    );
    result
        .expect("source scope must consume callback panic after bounded cleanup")
        .and(cleanup)
        .and(demand_cleanup)
}

#[test]
fn original_enum_paid_payload_query_error_and_panic_settle_callback_credit() {
    for panic_callback in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result = run_enum_with_original_demands(
            enum_owner(EnumCase::RetainedCorrelatedLoans),
            |plan, budget| {
                let loan = plan
                    .cells
                    .strategies
                    .iter()
                    .position(|strategy| {
                        matches!(strategy, SourceReferenceCellStrategyV29::Object(_))
                    })
                    .expect("the real retained reference payload requires selected object backing");
                let (_, cell) = plan
                    .backing_cell(loan, budget)?
                    .expect("the selected loan retains its original backing cell");
                let SourceBackingKindV29::Object(schema) = cell.kind else {
                    panic!("object loan lost its selected schema")
                };
                let origin = &plan.origins[plan.loans[loan].origin];
                assert!(origin.projections.is_empty());
                assert_eq!(
                    (cell.instance, cell.local, cell.generation, cell.ty),
                    (origin.instance, origin.local, origin.generation, origin.ty)
                );
                let before = budget.storage();
                let types = source_reference_payload_types_v29(plan, loan, budget)?;
                assert_eq!(types.len(), 1);
                let Type::Pointer(pointer) = &types[0] else {
                    panic!("retained object payload is not a pointer")
                };
                assert_eq!(*pointer.pointee, Type::StorageObject(schema));
                assert_eq!(pointer.address_space, AddressSpace::Private);
                assert_eq!(pointer.access, AccessMode::ReadOnly);
                assert!(
                    budget.storage() > before,
                    "exercise actual paid callback scratch"
                );
                drop(types);
                reached.set(true);
                if panic_callback {
                    panic!("paid enum callback panic probe");
                }
                Err(source_reference_error_v29("paid enum callback error probe"))
            },
        );
        assert!(reached.get());
        assert!(result.is_err());
        if !panic_callback {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "paid enum callback error probe",
                    ..
                })
            ));
        }
    }
}

#[test]
fn original_tag_only_enum_activations_cover_each_lifetime_and_helper_instance() {
    let reached = std::cell::Cell::new(false);
    run_enum_with_original_demands(tag_only_activation_owner_v29(3), |plan, budget| {
        let (requests, _) = plan
            .storage_demands
            .unwrap()
            .requests(plan.instances, budget)?;
        let mut instances = std::collections::BTreeSet::new();
        let mut observed = 0;
        for (index, row) in plan.storage_activations.iter().enumerate() {
            assert_eq!(
                plan.storage_activation_sites.get(&(
                    row.instance.index(),
                    row.local.index(),
                    row.generation
                )),
                Some(&index)
            );
            let declaration = plan.instances.instance(row.instance).unwrap().declaration();
            if declaration.locals()[row.local.index() as usize].ty() != ENUM {
                continue;
            }
            assert!(source_backing_original_request_v29(
                requests,
                row.instance,
                row.local,
                budget
            )?);
            assert_eq!(row.local.index(), 2);
            instances.insert(row.instance.index());
            observed += 1;
            match row.origin {
                SourceReferenceActivationOriginV29::Entry => assert_eq!(row.generation, 0),
                SourceReferenceActivationOriginV29::StorageLive(site) => {
                    assert_eq!(site.instance, row.instance);
                    assert_eq!(site.block.index(), 0);
                    assert!(matches!(site.statement, Some(0 | 3 | 6)));
                    assert_eq!(row.generation as usize, 1 + site.statement.unwrap());
                }
            }
            let cell = plan
                .cells
                .rows
                .iter()
                .find(|cell| {
                    (cell.instance, cell.local, cell.generation)
                        == (row.instance, row.local, row.generation)
                })
                .expect("B must consume tag-only activation");
            assert_eq!(cell.ty, ENUM);
            assert!(matches!(cell.kind, SourceBackingKindV29::Object(_)));
            assert!(
                !plan
                    .representation_demands
                    .iter()
                    .any(|write| write.instance == row.instance && write.local == row.local)
            );
            assert!(
                !plan
                    .origins
                    .iter()
                    .any(|origin| origin.instance == row.instance && origin.local == row.local)
            );
        }
        assert_eq!(
            instances.len(),
            2,
            "the same helper is instantiated at both original call sites"
        );
        assert_eq!(
            observed, 8,
            "entry plus three live sites for each actual helper instance"
        );
        assert_eq!(
            plan.storage_activations.len(),
            plan.storage_activation_sites.len()
        );
        with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
            for row in &plan.storage_activations {
                let (_, cell) = layouts
                    .backing_cell(row.instance, row.local, row.generation, budget)?
                    .expect("signature-owned index must consume the same activation census");
                assert_eq!(
                    (cell.instance, cell.local, cell.generation),
                    (row.instance, row.local, row.generation)
                );
                assert!(
                    layouts
                        .backing_cell(row.instance, row.local, u32::MAX, budget)?
                        .is_none()
                );
            }
            Ok(())
        })?;
        reached.set(true);
        Ok(())
    })
    .unwrap();
    assert!(
        reached.get(),
        "actual source plan and backing selection must both complete"
    );
}

#[test]
fn original_activation_scope_settles_nonzero_floor_on_success_error_and_panic() {
    for mode in 0..3 {
        let reached = std::cell::Cell::new(false);
        let result = run_enum_with_original_demands(tag_only_activation_owner_v29(1), |plan, _| {
            assert!(!plan.storage_activations.is_empty());
            reached.set(true);
            match mode {
                0 => Ok(()),
                1 => Err(source_reference_error_v29("activation callback failure")),
                _ => panic!("activation callback panic"),
            }
        });
        assert!(
            reached.get(),
            "source analysis must reach the requested cleanup case"
        );
        assert_eq!(result.is_err(), mode != 0);
    }
}

#[test]
fn original_loop_activations_retain_static_sites_without_unbounded_fixed_point_rows() {
    let reached = std::cell::Cell::new(false);
    run_enum_with_original_demands(
        tag_only_activation_owner_with_loop_v29(1, true),
        |plan, _| {
            let rows = plan
                .storage_activations
                .iter()
                .filter(|row| {
                    plan.instances
                        .instance(row.instance)
                        .unwrap()
                        .declaration()
                        .locals()[row.local.index() as usize]
                        .ty()
                        == ENUM
                })
                .collect::<Vec<_>>();
            assert_eq!(
                rows.len(),
                4,
                "each of two helper instances retains entry and one loop live site"
            );
            assert_eq!(
                rows.iter()
                    .filter(|row| matches!(
                        row.origin,
                        SourceReferenceActivationOriginV29::StorageLive(_)
                    ))
                    .count(),
                2
            );
            for row in rows {
                let declaration = plan.instances.instance(row.instance).unwrap().declaration();
                let SemanticTerminatorKindV1::SwitchInt { targets, .. } =
                    declaration.blocks()[0].terminator().kind()
                else {
                    panic!("fixture lost its original backedge")
                };
                assert_eq!(targets.values()[0].edge().target().index(), 0);
            }
            assert_eq!(
                plan.storage_activations.len(),
                plan.storage_activation_sites.len()
            );
            reached.set(true);
            Ok(())
        },
    )
    .unwrap();
    assert!(reached.get());
}

#[test]
fn original_demand_backing_keeps_correlated_enum_loans_as_distinct_objects() {
    let reached = std::cell::Cell::new(false);
    run_enum_with_original_demands(enum_owner(EnumCase::RetainedCorrelatedLoans), |plan, budget| {
        assert!(plan.storage_demands.is_some());
        let (requests, _) = plan.storage_demands.unwrap().requests(plan.instances, budget)?;
        let layouts = plan.storage_root.as_ref().unwrap().source_layouts(plan.instances, budget)?;
        let mut represented_joins = 0;
        for block in plan.blocks.iter().filter(|block| block.block.index() == 3) {
            let original = plan.instances.instance(block.instance).unwrap().declaration();
            assert_eq!(original.locals()[5].ty(), ENUM);
            let instance = plan.instances.instance(block.instance).unwrap();
            assert!(!instance.ssa().plan().promoted_variables().iter().any(|local| local.get() == 5));
            assert!(requests.iter().any(|row| row.instance == block.instance && row.local.index() == 5
                && row.ty == ENUM && row.path.is_empty()
                && row.kind == source_storage_demands_v29::DemandKindV29::WholeBackingCandidate));
            for branch in [1, 2] {
                let SemanticStatementKindV1::Assign(write) = original.blocks()[branch].statements()[1].kind()
                    else { panic!("retained fixture lost its genuine projected write") };
                assert_eq!(write.destination(), &enum_field(5, 0, 1));
                assert!(matches!(write.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source))
                    if source.local().index() == 3 && source.projections().is_empty() && source.ty() == WORD));
            }
            let state = plan.states[block.entry][5];
            let node = state.node.expect("actual original enum block-entry value");
            let value = &plan.nodes[node];
            assert_eq!(value.ty, ENUM);
            let SourceReferenceNodeKindV29::Enum { first, count } = value.kind
                else { panic!("original conditional branches must retain an enum join") };
            assert_eq!(count, 2);
            let cell = plan.cells.rows.iter().find(|cell|
                (cell.instance, cell.local.index(), cell.generation) == (block.instance, 5, state.generation))
                .expect("both original writes constrain the same existing backing object");
            assert_eq!(cell.ty, ENUM);
            let SourceBackingKindV29::Object(schema) = cell.kind
                else { panic!("the original retained enum must select an Object schema") };
            layouts.check_selected_schema(plan.instances.owner(), ENUM, schema, budget)?;
            let mut identities = std::collections::BTreeSet::new();
            let mut physical = None;
            for offset in 0..count {
                let member = plan.enum_member(first, count, offset, budget)?;
                let alternative = plan.enum_alternative(member, budget)?;
                assert_eq!((alternative.ty, alternative.variant, alternative.count), (ENUM, 0, 2));
                assert!(alternative.opaque.is_none());
                let child = plan.children[alternative.first];
                let SourceReferenceNodeKindV29::Loan(loan) = plan.nodes[child].kind
                    else { panic!("actual retained reference alternative lost its original loan") };
                let origin = &plan.origins[plan.loans[loan].origin];
                let (_, cell) = plan.backing_cell(loan, budget)?
                    .expect("a loan stored in a represented enum must be addressable");
                assert_eq!((cell.instance, cell.local, cell.generation),
                    (origin.instance, origin.local, origin.generation));
                identities.insert((cell.instance.index(), cell.local.index(), cell.generation));
                let types = source_reference_payload_types_v29(plan, loan, budget)?;
                assert_eq!(types.len(), 1);
                assert!(matches!(types[0], Type::Pointer(_)));
                if let Some(previous) = &physical { assert_eq!(previous, &types); }
                else { physical = Some(types); }
            }
            assert_eq!(identities.len(), 2, "compatible pointer types cannot merge original allocations");
            represented_joins += 1;
        }
        assert_eq!(represented_joins, 2, "both actual helper instances retain their original enum backing");
        reached.set(true);
        Ok(())
    }).unwrap();
    assert!(reached.get());
}

#[test]
fn admitted_three_variant_constructors_use_logical_tags_and_preserve_payload_state() {
    for variant in 0..3 {
        run_enum(EnumCase::Construct(variant), |plan, _| {
            assert!(
                plan.enum_alternatives
                    .iter()
                    .any(|row| row.ty == ENUM && row.variant == variant && row.count == 2)
            );
            assert!(plan.enum_observations.iter().any(|row| row.ty == ENUM));
            assert!(!plan.storage_snapshots.is_empty());
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn admitted_projected_enum_constructor_initializes_only_its_actual_fields_and_tag() {
    run_enum(EnumCase::ProjectedConstructor, |plan, _| {
        assert!(plan.enum_alternatives.iter().any(|row| row.ty == ENUM
            && row.variant == 0
            && row.opaque.is_none()
            && row.count == 2));
        assert!(plan.enum_observations.iter().any(|row| row.ty == ENUM));
        assert!(!plan.storage_snapshots.is_empty());
        Ok(())
    })
    .unwrap();
    let reached = std::cell::Cell::new(false);
    assert!(
        run_enum(EnumCase::ProjectedTagOnly, |_, _| {
            reached.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(
        !reached.get(),
        "retagging must not initialize the untouched word payload"
    );
}

#[test]
fn correlated_reference_bindings_retain_the_complete_parent_view_without_a_representative_loan() {
    let completed = std::cell::Cell::new(false);
    run_enum(EnumCase::CorrelatedLoans, |plan, budget| {
        let node = plan
            .nodes
            .iter()
            .enumerate()
            .find_map(|(node, row)| {
                let SourceReferenceNodeKindV29::EnumView(index) = row.kind else {
                    return None;
                };
                (row.ty == REFERENCE && plan.enum_views[index].child_count > 1).then_some(node)
            })
            .expect("actual CFG join must retain the correlated reference view");
        let origin = SourceReferenceBindingOriginV29::EnumView(node);
        assert!(origin.single_loan().is_err());
        let types =
            source_reference_binding_origin_types_v29(plan, origin, REFERENCE, &mut 0, budget)?;
        let binding = SemanticSourceReferenceBindingV29 {
            owner: plan as *const SourceReferencePlanV29<'_, '_> as usize,
            source: plan.source,
            ssa: plan.ssa,
            root: plan.root,
            origin,
            source_type: REFERENCE,
            values: types
                .into_iter()
                .enumerate()
                .map(|(index, ty)| ValueDef::new(ValueId(100 + index as u32), ty))
                .collect(),
        };
        source_reference_validate_binding_v29(plan, &binding, budget)?;
        assert_eq!(binding.origin, origin);
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let held = SemanticValueBindingV1::SourceReference(binding.clone());
        let (leaves, ids) = source_reference_call_shape_v29(&references, node, &held, budget)?;
        assert!(leaves.is_empty());
        assert_eq!(
            ids,
            binding
                .values
                .iter()
                .map(|value| Some(value.id))
                .collect::<Vec<_>>()
        );
        let SourceReferenceNodeKindV29::EnumView(view) = plan.nodes[node].kind else {
            unreachable!()
        };
        let child = plan.children[plan.enum_views[view].children];
        let SourceReferenceNodeKindV29::Loan(loan) = plan.nodes[child].kind else {
            unreachable!()
        };
        let mut representative = binding.clone();
        representative.origin = SourceReferenceBindingOriginV29::SingleLoan(loan);
        let representative = SemanticValueBindingV1::SourceReference(representative);
        assert!(
            source_reference_merge_node_v29(
                &references,
                node,
                &held,
                &representative,
                &mut [].iter_mut(),
                &mut 0,
                budget
            )
            .is_err()
        );
        let mut wrong = binding.clone();
        wrong.source_type = WORD;
        assert!(source_reference_validate_binding_v29(plan, &wrong, budget).is_err());
        completed.set(true);
        Ok(())
    })
    .unwrap();
    assert!(completed.get());
}

#[test]
fn reference_validation_correlated_candidates_are_query_scratch() {
    run_enum(EnumCase::CorrelatedLoans, |plan, budget| {
        let node = plan
            .nodes
            .iter()
            .enumerate()
            .find_map(|(node, row)| {
                let SourceReferenceNodeKindV29::EnumView(index) = row.kind else {
                    return None;
                };
                (row.ty == REFERENCE && plan.enum_views[index].child_count > 1).then_some(node)
            })
            .expect("genuine correlated reference view");
        let origin = SourceReferenceBindingOriginV29::EnumView(node);
        let types =
            source_reference_binding_origin_types_v29(plan, origin, REFERENCE, &mut 0, budget)?;
        let binding = SemanticSourceReferenceBindingV29 {
            owner: plan as *const SourceReferencePlanV29<'_, '_> as usize,
            source: plan.source,
            ssa: plan.ssa,
            root: plan.root,
            origin,
            source_type: REFERENCE,
            values: types
                .into_iter()
                .enumerate()
                .map(|(index, ty)| ValueDef::new(ValueId(500 + index as u32), ty))
                .collect(),
        };
        let floor = budget.storage();
        for _ in 0..16 {
            source_reference_validate_binding_v29(plan, &binding, budget)?;
            assert_eq!(budget.storage(), floor);
            assert_eq!(binding.origin, origin);
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn reference_validation_object_schema_stays_owned_by_the_original_arena() {
    run_enum_with_original_demands(
        enum_owner(EnumCase::RetainedCorrelatedLoans),
        |plan, budget| {
            let loan = plan
                .cells
                .strategies
                .iter()
                .position(|strategy| matches!(strategy, SourceReferenceCellStrategyV29::Object(_)))
                .expect("genuine object-backed reference");
            let types = source_reference_payload_types_v29(plan, loan, budget)?;
            assert_eq!(types.len(), 1);
            assert!(matches!(types[0], Type::Pointer(_)));
            let binding = SemanticSourceReferenceBindingV29 {
                owner: plan as *const SourceReferencePlanV29<'_, '_> as usize,
                source: plan.source,
                ssa: plan.ssa,
                root: plan.root,
                origin: SourceReferenceBindingOriginV29::SingleLoan(loan),
                source_type: plan.loans[loan].source_type,
                values: types
                    .into_iter()
                    .map(|ty| ValueDef::new(ValueId(501), ty))
                    .collect(),
            };
            budget.charge_work(source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29)?;
            let root = plan.storage_root.as_ref().expect("original storage arena");
            let growth = root.capture_retained_growth().expect("live root custody");
            let floor = budget.storage();
            for _ in 0..16 {
                source_reference_validate_binding_v29(plan, &binding, budget)?;
                assert_eq!(budget.storage(), floor);
                assert!(root.retains_custody(plan.instances, &plan.failure, budget));
                assert!(growth.permits_refund(floor, floor, budget.storage(), 0));
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn admitted_enum_partial_moves_preserve_siblings_and_exact_reinitialization() {
    for case in [EnumCase::MoveSibling, EnumCase::Reinitialize] {
        run_enum(case, |plan, _| {
            assert!(plan.inactive_removals.iter().any(|removal| {
                plan.projections[removal.projections.clone()]
                    .iter()
                    .any(|projection| {
                        matches!(projection.kind(), SemanticProjectionKindV1::Downcast(0))
                    })
            }));
            assert!(
                plan.nodes
                    .iter()
                    .any(|node| node.kind == SourceReferenceNodeKindV29::Absent)
            );
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn original_switch_edges_refine_joined_payload_guards_without_initializing_bytes() {
    run_enum(EnumCase::ConditionalVariants, |plan, _| {
        assert!(
            plan.nodes
                .iter()
                .any(|node| matches!(node.kind, SourceReferenceNodeKindV29::Enum { count: 2, .. }))
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn same_variant_join_keeps_distinct_complete_loan_alternatives() {
    run_enum(EnumCase::CorrelatedLoans, |plan, _| {
        let mut found = false;
        for view in &plan.enum_views {
            if view.child_count != 2 {
                continue;
            }
            let children = &plan.children[view.children..view.children + view.child_count];
            let (SourceReferenceNodeKindV29::Loan(left), SourceReferenceNodeKindV29::Loan(right)) =
                (plan.nodes[children[0]].kind, plan.nodes[children[1]].kind)
            else {
                continue;
            };
            assert_ne!(left, right);
            assert_ne!(plan.loans[left].origin, plan.loans[right].origin);
            assert!(matches!(
                plan.nodes[view.source].kind,
                SourceReferenceNodeKindV29::Enum { count: 2, .. }
            ));
            found = true;
        }
        assert!(
            found,
            "the source join lost its parent-correlated loan alternatives"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn saved_tags_duplicate_target_edges_and_retagging_never_grant_payload_authority() {
    for case in [
        EnumCase::SavedDiscriminator,
        EnumCase::DuplicateTarget,
        EnumCase::TagDoesNotInitialize,
        EnumCase::LiveAlternativeDies,
    ] {
        let owner = match try_enum_owner(case) {
            Ok(owner) => owner,
            Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ..
            }) if matches!(case, EnumCase::TagDoesNotInitialize) => continue,
            Err(error) => panic!("unexpected fixture admission failure for {case:?}: {error:?}"),
        };
        let reached = std::cell::Cell::new(false);
        assert!(
            super::super::cells_tests::run_cells(owner, |_, _| {
                reached.set(true);
                Ok(())
            })
            .is_err(),
            "{case:?}"
        );
        assert!(
            !reached.get(),
            "invalid enum source state reached completion: {case:?}"
        );
    }
}

fn rebuilt_original_enum_binding_v29(
    references: &SourceReferenceEmissionV29<'_, '_>,
    variant: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(usize, SemanticValueBindingV1), ProductionSemanticKirErrorV1> {
    let plan = references.plan;
    let node = plan
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, row)| {
            let SourceReferenceNodeKindV29::Enum { first, count: 1 } = row.kind else {
                return None;
            };
            let alternative = plan.enum_alternatives[plan.enum_members[first]];
            (row.ty == ENUM
                && alternative.variant == variant
                && alternative.opaque.is_none()
                && alternative.count == 2)
                .then_some(node)
        })
        .expect("actual source constructor enum");
    let types = source_reference_node_types_v29(plan, node, budget)?;
    let values = types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| ValueDef::new(ValueId(900 + index as u32), ty))
        .collect::<Vec<_>>();
    let mut values = values.iter();
    let mut leaves = [].iter();
    let binding = source_reference_rebuild_node_v29(
        references,
        node,
        false,
        &mut leaves,
        &mut values,
        &mut 0,
        budget,
    )?;
    assert!(values.next().is_none());
    assert!(leaves.next().is_none());
    assert!(matches!(
        &binding,
        SemanticValueBindingV1::Enum { variant: None, .. }
    ));
    Ok((node, binding))
}

#[test]
fn enum_transport_preserves_exact_archived_variant_metadata_without_creating_proof() {
    for original in 0..3 {
        let reached = std::cell::Cell::new(false);
        run_enum(EnumCase::Construct(original), |plan, budget| {
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let (node, rebuilt) = rebuilt_original_enum_binding_v29(&references, original, budget)?;
            for (held_variant, archived_variant, accepted) in [
                (None, None, true),
                (Some(original), Some(original), true),
                (Some(original), None, false),
                (None, Some(original), false),
                (Some(original), Some((original + 1) % 3), false),
                (Some((original + 1) % 3), Some((original + 1) % 3), false),
                (Some(u32::MAX), Some(u32::MAX), false),
            ] {
                let mut held = rebuilt.clone();
                let mut archived = rebuilt.clone();
                let SemanticValueBindingV1::Enum { variant, .. } = &mut held else {
                    unreachable!()
                };
                *variant = held_variant;
                let SemanticValueBindingV1::Enum { variant, .. } = &mut archived else {
                    unreachable!()
                };
                *variant = archived_variant;
                let mut untouched = [None];
                let mut leaves = untouched.iter_mut();
                let mut nodes = 0;
                let result = source_reference_merge_node_v29(
                    &references,
                    node,
                    &held,
                    &archived,
                    &mut leaves,
                    &mut nodes,
                    budget,
                );
                assert_eq!(
                    result.is_ok(),
                    accepted,
                    "variant {original}: {held_variant:?}/{archived_variant:?}"
                );
                assert!(
                    leaves.next().is_some(),
                    "metadata checks must not consume unrelated leaf slots"
                );
                assert_eq!(untouched, [None]);
                if !accepted {
                    assert_eq!(nodes, 1, "reject before traversing child bindings");
                }
                assert!(matches!(
                    &rebuilt,
                    SemanticValueBindingV1::Enum { variant: None, .. }
                ));
            }
            reached.set(true);
            Ok(())
        })
        .unwrap();
        assert!(reached.get());
    }
}

#[test]
fn enum_variant_metadata_validation_has_an_independent_exact_work_boundary() {
    for original in 0..3 {
        for short in [false, true] {
            let reached = std::cell::Cell::new(false);
            let result = run_enum(EnumCase::Construct(original), |plan, budget| {
                let references = SourceReferenceEmissionV29::new(plan, budget)?;
                let (node, held) =
                    rebuilt_original_enum_binding_v29(&references, original, budget)?;
                let mut archived = held.clone();
                let SemanticValueBindingV1::Enum { variant, .. } = &mut archived else {
                    unreachable!()
                };
                *variant = Some(original);
                assert!(plan.nodes[node].atomic_custody.is_none());
                // Emission owner5 + plan owner5 + node1.
                // The atomic-type None probe still charges owner5 plus(owner5+12).
                // Then enum owner5 + shape(owner5+6), two fields(owner5+2 each),
                // and metadata(owner5+2). The final2 rejects the unequal variants.
                let exact = 5 + 5 + 1 + (5 + (5 + 12)) + 5 + (5 + 6) + 2 * (5 + 2) + (5 + 2);
                assert_eq!(exact, 70);
                budget.charge_work(usize::MAX - budget.work() - exact + usize::from(short))?;
                let before = (budget.work(), budget.storage());
                let mut untouched = [None];
                let mut leaves = untouched.iter_mut();
                let mut nodes = 0;
                let checked = source_reference_merge_node_v29(
                    &references,
                    node,
                    &held,
                    &archived,
                    &mut leaves,
                    &mut nodes,
                    budget,
                );
                assert!(checked.is_err());
                assert_eq!(budget.work() - before.0, exact - if short { 2 } else { 0 });
                assert_eq!(budget.storage(), before.1);
                assert_eq!(nodes, 1);
                assert!(leaves.next().is_some());
                assert_eq!(untouched, [None]);
                if short {
                    let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(bound),
                    )) = &checked
                    else {
                        panic!("final metadata charge must exhaust the original work meter");
                    };
                    // With69 available,68 are accepted. The final2 overflows MAX;
                    // the original meter reports its documented MAX sentinel.
                    assert_eq!(budget.work(), usize::MAX - 1);
                    assert_eq!(bound.actual(), usize::MAX);
                    assert_eq!(bound.limit(), usize::MAX);
                    assert!(matches!(plan.failure.first_error(),
                        Some(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(first))) if first == *bound));
                    let stopped = (budget.work(), budget.storage());
                    assert!(
                        source_reference_merge_node_v29(
                            &references,
                            node,
                            &held,
                            &archived,
                            &mut [].iter_mut(),
                            &mut 0,
                            budget
                        )
                        .is_err()
                    );
                    assert_eq!((budget.work(), budget.storage()), stopped);
                } else {
                    assert!(matches!(
                        checked,
                        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                    ));
                }
                reached.set(true);
                Err(source_reference_error_v29(
                    "completed enum variant metadata boundary",
                ))
            });
            assert!(
                reached.get(),
                "metadata resource assertions must run before scope cleanup"
            );
            if short {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "completed enum variant metadata boundary",
                        ..
                    })
                ));
            }
        }
    }
}

fn assert_original_boundary_census_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(usize, usize), ProductionSemanticKirErrorV1> {
    let mut expected = std::collections::BTreeSet::new();
    let mut calls = 0;
    let mut returns = 0;
    for (ordinal, instance) in plan.instances.instances().iter().enumerate() {
        let id = plan.instances.id_at(ordinal).unwrap();
        if plan.instances.instance_reachable(id) != Some(true) {
            continue;
        }
        for (block, declaration) in instance.declaration().blocks().iter().enumerate() {
            let block = SemanticBlockIdV1::from_index(block as u32);
            if plan.instances.block_reachable(id, block) != Some(true) {
                continue;
            }
            let site = SourceReferenceSiteV29 {
                instance: id,
                block,
                statement: None,
            };
            let roles = match declaration.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => {
                    calls += call.arguments().len();
                    (0..call.arguments().len())
                        .map(|index| SourceReferenceBoundaryRoleV29::Argument(index as u32))
                        .collect::<Vec<_>>()
                }
                SemanticTerminatorKindV1::Return => {
                    returns += 1;
                    vec![SourceReferenceBoundaryRoleV29::Return]
                }
                _ => vec![],
            };
            for role in roles {
                let (index, row) = plan
                    .boundary_at(site, role, budget)?
                    .expect("actual original boundary");
                assert_eq!(plan.boundary_value(index, budget)?, row);
                assert_eq!((row.site, row.role), (site, role));
                assert_ne!(
                    plan.nodes[row.node].kind,
                    SourceReferenceNodeKindV29::Absent
                );
                assert!(expected.insert((id.index(), block.index(), role)));
            }
        }
    }
    assert_eq!(plan.boundary_values.len(), expected.len());
    assert_eq!(plan.boundary_sites.len(), expected.len());
    Ok((calls, returns))
}

#[test]
fn original_boundary_census_preserves_each_actual_call_and_return_instance() {
    for case in [
        EnumCase::Construct(0),
        EnumCase::Construct(1),
        EnumCase::Construct(2),
        EnumCase::CorrelatedLoans,
        EnumCase::MoveSibling,
    ] {
        let reached = std::cell::Cell::new(false);
        run_enum(case, |plan, budget| {
            let (calls, returns) = assert_original_boundary_census_v29(plan, budget)?;
            assert_eq!(
                calls, 4,
                "two root calls and their two nested closure arguments"
            );
            assert!(returns >= 5);
            let captures = plan
                .boundary_values
                .iter()
                .filter(|row| {
                    matches!(row.role, SourceReferenceBoundaryRoleV29::Argument(_))
                        && plan.nodes[row.node].ty == CAPTURE
                })
                .collect::<Vec<_>>();
            assert_eq!(captures.len(), 2);
            assert_ne!(captures[0].site.instance, captures[1].site.instance);
            reached.set(true);
            Ok(())
        })
        .unwrap();
        assert!(reached.get());
    }
}

fn boundary_after_move_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    boundary_after_move_owner_with_use_v29(true)
}

fn boundary_after_move_owner_with_use_v29(later_use: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |_, functions| {
        let first = functions[1].blocks()[0].clone();
        let locals = functions[1].locals().to_vec();
        functions[1] = function(
            20,
            false,
            WORD,
            locals,
            vec![
                first,
                block(
                    21,
                    vec![
                        assign(
                            place(4, REFERENCE),
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Shared,
                                place: place(1, WORD),
                            },
                        ),
                        assign(
                            place(3, CAPTURE),
                            SemanticRvalueKindV1::Aggregate(
                                SemanticAggregateRvalueV1::new(
                                    SemanticAggregateKindV1::Tuple,
                                    vec![SemanticOperandV1::Move(place(4, REFERENCE))],
                                )
                                .unwrap(),
                            ),
                        ),
                        dead(4),
                    ],
                    go(2),
                ),
                block(
                    22,
                    if later_use {
                        vec![
                            statement(SemanticStatementKindV1::StorageLive(
                                SemanticLocalIdV1::from_index(4),
                            )),
                            assign(
                                place(4, REFERENCE),
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                                    3,
                                    &[(SemanticProjectionKindV1::Field(0), REFERENCE)],
                                ))),
                            ),
                            dead(4),
                            unit(),
                        ]
                    } else {
                        vec![unit()]
                    },
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
    })
}

#[test]
fn original_argument_rows_survive_move_and_later_holder_reassignment() {
    let reached = std::cell::Cell::new(false);
    run_enum_with_original_demands(boundary_after_move_owner_v29(), |plan, budget| {
        assert_original_boundary_census_v29(plan, budget)?;
        let mut checked = 0;
        for row in &plan.boundary_values {
            if !matches!(row.role, SourceReferenceBoundaryRoleV29::Argument(0))
                || plan.nodes[row.node].ty != CAPTURE
            {
                continue;
            }
            let entered = |block| {
                let entry = plan
                    .blocks
                    .iter()
                    .find(|entry| {
                        entry.instance == row.site.instance && entry.block.index() == block
                    })
                    .unwrap();
                &plan.states[entry.entry]
            };
            assert!(
                entered(1)[3].node.is_none(),
                "actual Move consumes the caller holder, not the boundary snapshot"
            );
            let later = entered(2)[3].node.unwrap();
            assert_ne!(later, row.node);
            let SourceReferenceNodeKindV29::Aggregate {
                first: original,
                count: 1,
            } = plan.nodes[row.node].kind
            else {
                panic!("original capture snapshot")
            };
            let SourceReferenceNodeKindV29::Aggregate {
                first: replacement,
                count: 1,
            } = plan.nodes[later].kind
            else {
                panic!("later capture assignment")
            };
            let SourceReferenceNodeKindV29::Loan(original) =
                plan.nodes[plan.children[original]].kind
            else {
                panic!("original evaluated loan")
            };
            let SourceReferenceNodeKindV29::Loan(replacement) =
                plan.nodes[plan.children[replacement]].kind
            else {
                panic!("later evaluated loan")
            };
            assert_ne!(plan.loans[original].site, plan.loans[replacement].site);
            let (_, queried) = plan.boundary_at(row.site, row.role, budget)?.unwrap();
            assert_eq!(queried, *row);
            checked += 1;
        }
        assert_eq!(checked, 2);
        reached.set(true);
        Ok(())
    })
    .unwrap();
    assert!(reached.get());
}

#[test]
fn original_argument_rows_survive_expiry_of_an_unused_replacement_holder() {
    run_enum_with_original_demands(
        boundary_after_move_owner_with_use_v29(false),
        |plan, budget| {
            assert_original_boundary_census_v29(plan, budget)?;
            let mut checked = 0;
            for row in &plan.boundary_values {
                if !matches!(row.role, SourceReferenceBoundaryRoleV29::Argument(0))
                    || plan.nodes[row.node].ty != CAPTURE
                {
                    continue;
                }
                let later = plan
                    .blocks
                    .iter()
                    .find(|entry| entry.instance == row.site.instance && entry.block.index() == 2)
                    .unwrap();
                assert!(plan.states[later.entry][3].node.is_none());
                assert!(matches!(
                    plan.nodes[row.node].kind,
                    SourceReferenceNodeKindV29::Aggregate { count: 1, .. }
                ));
                assert_eq!(
                    plan.boundary_at(row.site, row.role, budget)?.unwrap().1,
                    *row
                );
                checked += 1;
            }
            assert_eq!(checked, 2);
            Ok(())
        },
    )
    .unwrap();
}

fn intrinsic_boundary_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let original = owner(Case::Shared);
    let semantic = original.source_semantic();
    let value = || {
        SemanticAbiValueV1::new(
            WORD,
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
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(value()),
            SemanticAbiArgumentV1::source(value()),
        ],
        value(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ])
    .unwrap();
    let intrinsic = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([201; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([201; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([201; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([201; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([201; 32]),
            source(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::SaturatingInteger(
            SemanticSaturatingIntegerOpV1::Add,
        ),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([201; 32]),
    };
    let root = function(
        10,
        true,
        WORD,
        vec![
            local(10, UNIT, SemanticLocalRoleV1::Return),
            local(11, WORD, SemanticLocalRoleV1::Argument(0)),
            local(12, WORD, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                10,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        vec![
                            SemanticOperandV1::Copy(place(1, WORD)),
                            SemanticOperandV1::Constant(SemanticConstantV1::new(
                                WORD,
                                SemanticConstantValueV1::Scalar(
                                    SemanticScalarValueV1::new(1, 8).unwrap(),
                                ),
                            )),
                        ],
                        Some(SemanticCallDestinationV1::new(
                            place(2, WORD),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(11, vec![unit()], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(semantic.functions()[0].kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types()[..2].to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![SemanticCallableDeclV1::defined(ROOT), intrinsic],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v30(SemanticMirLimitsV1::default())
    .unwrap();
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    owner
}

#[test]
fn intrinsic_arguments_use_the_same_original_boundary_census() {
    let reached = std::cell::Cell::new(false);
    run_enum_with_original_demands(intrinsic_boundary_owner_v29(), |plan, budget| {
        assert_original_boundary_census_v29(plan, budget)?;
        let mut intrinsic_arguments = 0;
        for row in &plan.boundary_values {
            if !matches!(row.role, SourceReferenceBoundaryRoleV29::Argument(_)) {
                continue;
            }
            let call = plan
                .instances
                .calls(row.site.instance)
                .unwrap()
                .iter()
                .find(|call| call.occurrence().block == row.site.block)
                .unwrap();
            if matches!(
                call.callable(),
                SemanticCallableDeclV1::CompilerIntrinsic { .. }
            ) {
                intrinsic_arguments += 1;
                assert_eq!(
                    plan.boundary_at(row.site, row.role, budget)?.unwrap().1,
                    *row
                );
            }
        }
        assert_eq!(
            intrinsic_arguments, 2,
            "both evaluated intrinsic arguments retain their source sites"
        );
        reached.set(true);
        Ok(())
    })
    .unwrap();
    assert!(reached.get());
}

fn empty_demand_boundary_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let original = owner(Case::Shared);
    let source = original.source_semantic();
    let root = function(
        10,
        true,
        WORD,
        vec![
            local(10, UNIT, SemanticLocalRoleV1::Return),
            local(11, WORD, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(10, vec![unit()], SemanticTerminatorKindV1::Return)],
    )
    .with_kernel_entry(source.functions()[0].kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types()[..2].to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    owner
}

#[test]
fn genuine_empty_storage_demands_still_retain_original_value_boundaries() {
    let owner = empty_demand_boundary_owner_v29();
    let reached = std::cell::Cell::new(false);
    run_enum_with_original_demands(owner, |plan, budget| {
        assert!(!plan.has_storage_demands);
        assert!(
            plan.storage_demands
                .unwrap()
                .requests(plan.instances, budget)?
                .0
                .is_empty()
        );
        assert!(plan.storage_activations.is_empty());
        assert!(plan.cells.rows.is_empty());
        assert_eq!(assert_original_boundary_census_v29(plan, budget)?, (0, 1));
        assert_eq!(plan.nodes[plan.boundary_values[0].node].ty, UNIT);
        reached.set(true);
        Ok(())
    })
    .unwrap();
    assert!(reached.get());
}

#[test]
fn original_loop_boundaries_are_bounded_by_original_sites_and_cleanup_on_all_exits() {
    for mode in 0..3 {
        let reached = std::cell::Cell::new(false);
        let result = run_enum_with_original_demands(
            tag_only_activation_owner_with_loop_v29(1, true),
            |plan, budget| {
                assert_original_boundary_census_v29(plan, budget)?;
                assert!(plan.instances.instances().iter().any(|instance| {
                    instance.declaration().blocks().iter().any(|block| {
                        matches!(
                            block.terminator().kind(),
                            SemanticTerminatorKindV1::SwitchInt { .. }
                        )
                    })
                }));
                reached.set(true);
                match mode {
                    0 => Ok(()),
                    1 => Err(source_reference_error_v29("boundary callback refusal")),
                    _ => panic!("boundary callback panic"),
                }
            },
        );
        assert!(reached.get());
        assert_eq!(result.is_err(), mode != 0);
    }
}

fn boundary_correlated_enum_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |types, functions| {
        types.push(enum_declaration());
        let mut locals = functions[2].locals().to_vec();
        locals.push(local(187, ENUM, SemanticLocalRoleV1::Temporary));
        let mut initial = functions[2].blocks()[0].statements().to_vec();
        initial.push(assign(
            place(4, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(3, WORD),
            },
        ));
        functions[2] = function(
            30,
            false,
            CAPTURE,
            locals,
            vec![
                block(180, initial, switch(3, 0, 1, 2)),
                block(181, vec![construct(0, 2)], go(3)),
                block(182, vec![construct(0, 4)], go(3)),
                block(
                    183,
                    vec![],
                    call(3, SemanticOperandV1::Copy(place(5, ENUM)), 4),
                ),
                block(184, vec![unit()], SemanticTerminatorKindV1::Return),
            ],
        );
        let mode = SemanticAbiPassModeV1::Indirect {
            attributes: SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesNone),
                    true,
                    false,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                24,
                Some(8),
            )
            .unwrap(),
            metadata_attributes: None,
            on_stack: false,
        };
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([190; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                ENUM, mode,
            ))],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
        .unwrap();
        functions.push(
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([190; 32]),
                SemanticFunctionRoleV1::InternalHelper,
                SemanticItemDefinitionIdentityV1::from_sha256([190; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([190; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([190; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([190; 32]),
                source(),
                abi,
                vec![
                    local(190, UNIT, SemanticLocalRoleV1::Return),
                    local(191, ENUM, SemanticLocalRoleV1::Argument(0)),
                ],
                SemanticBlockIdV1::from_index(0),
                vec![block(190, vec![unit()], SemanticTerminatorKindV1::Return)],
            )
            .unwrap(),
        );
    })
}

#[test]
fn actual_caller_boundaries_retain_whole_correlated_enum_alternatives() {
    let reached = std::cell::Cell::new(false);
    run_enum_with_original_demands(boundary_correlated_enum_owner_v29(), |plan, budget| {
        assert_original_boundary_census_v29(plan, budget)?;
        let mut checked = 0;
        for row in &plan.boundary_values {
            if row.role != SourceReferenceBoundaryRoleV29::Argument(0)
                || plan.nodes[row.node].ty != ENUM
            {
                continue;
            }
            let SourceReferenceNodeKindV29::Enum { first, count } = plan.nodes[row.node].kind
            else {
                panic!("actual source call must retain the joined enum")
            };
            assert_eq!(count, 2);
            let mut origins = std::collections::BTreeSet::new();
            for offset in 0..count {
                let member = plan.enum_member(first, count, offset, budget)?;
                let alternative = plan.enum_alternative(member, budget)?;
                assert_eq!(
                    (alternative.variant, alternative.count, alternative.opaque),
                    (0, 2, None)
                );
                let SourceReferenceNodeKindV29::Loan(loan) =
                    plan.nodes[plan.children[alternative.first]].kind
                else {
                    panic!("whole alternative lost its actual loan child")
                };
                let origin = &plan.origins[plan.loans[loan].origin];
                origins.insert((
                    origin.instance.index(),
                    origin.local.index(),
                    origin.generation,
                ));
            }
            assert_eq!(
                origins.len(),
                2,
                "no representative loan may replace a caller value"
            );
            assert_eq!(
                plan.boundary_at(row.site, row.role, budget)?.unwrap().1,
                *row
            );
            checked += 1;
        }
        assert_eq!(
            checked, 2,
            "both original callers retain their own evaluation"
        );
        reached.set(true);
        Ok(())
    })
    .unwrap();
    assert!(reached.get());
}
