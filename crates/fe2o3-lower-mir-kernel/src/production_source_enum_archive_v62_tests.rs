use super::*;

fn with_reference_enum(
    consume: impl FnOnce(
        SemanticValueBindingV1,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) {
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
            .expect("actual correlated source reference");
        let origin = SourceReferenceBindingOriginV29::EnumView(node);
        let types =
            source_reference_binding_origin_types_v29(plan, origin, REFERENCE, &mut 0, budget)?;
        let reference = SemanticSourceReferenceBindingV29 {
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
        source_reference_validate_binding_v29(plan, &reference, budget)?;
        consume(
            SemanticValueBindingV1::Enum {
                discriminant: ValueId(41),
                discriminant_ty: Type::Scalar(ScalarType::U8),
                semantic_type: ENUM,
                variant: Some(0),
                payloads: BTreeMap::from([(
                    0,
                    vec![SemanticValueBindingV1::SourceReference(reference)],
                )]),
            },
            budget,
        )
    })
    .unwrap();
}

fn archive_inputs(
    value: &SemanticValueBindingV1,
    projected: bool,
) -> (SemanticSsaBindingsV1, SsaValueV1, SemanticPlaceV1) {
    let definition = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(0));
    let projections = if projected {
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), ENUM).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), REFERENCE).unwrap(),
        ]
    } else {
        vec![]
    };
    (
        SemanticSsaBindingsV1::from([(definition, value.clone())]),
        definition,
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(0),
            projections,
            if projected { REFERENCE } else { ENUM },
        )
        .unwrap(),
    )
}

#[test]
fn source_enum_archive_compares_every_holder_and_reference_identity_field() {
    let foreign_ssa = enum_owner(EnumCase::Construct(0)).identity();
    with_reference_enum(|value, budget| {
        for projected in [false, true] {
            let (archive, definition, place) = archive_inputs(&value, projected);
            for mutation in 0..18 {
                let mut changed = value.clone();
                let SemanticValueBindingV1::Enum {
                    discriminant,
                    discriminant_ty,
                    semantic_type,
                    variant,
                    payloads,
                } = &mut changed
                else {
                    unreachable!()
                };
                match mutation {
                    1 => *discriminant = ValueId(42),
                    2 => *discriminant_ty = Type::Scalar(ScalarType::U32),
                    3 => *semantic_type = WORD,
                    4 => *variant = None,
                    5 => {
                        let fields = payloads.remove(&0).unwrap();
                        payloads.insert(1, fields);
                    }
                    6 => payloads
                        .get_mut(&0)
                        .unwrap()
                        .push(SemanticValueBindingV1::Unit),
                    7 => payloads.get_mut(&0).unwrap()[0] = SemanticValueBindingV1::MovedExecution,
                    8..=16 => {
                        let SemanticValueBindingV1::SourceReference(reference) =
                            &mut payloads.get_mut(&0).unwrap()[0]
                        else {
                            unreachable!()
                        };
                        match mutation {
                            8 => reference.owner ^= 1,
                            9 => reference.source[0] ^= 1,
                            10 => {
                                reference.root =
                                    ProductionCallInstanceIdV1(reference.root.index() + 1)
                            }
                            11 => reference.source_type = WORD,
                            12 => reference.origin = SourceReferenceBindingOriginV29::SingleLoan(0),
                            13 => reference.values[0].id = ValueId(500),
                            14 => {
                                assert_ne!(reference.values[0].ty, Type::Scalar(ScalarType::U8));
                                reference.values[0].ty = Type::Scalar(ScalarType::U8);
                            }
                            15 => reference.values.clear(),
                            16 => {
                                assert_ne!(reference.ssa, foreign_ssa);
                                reference.ssa = foreign_ssa;
                            }
                            _ => unreachable!(),
                        }
                    }
                    17 => {
                        payloads.insert(1, vec![]);
                    }
                    _ => {}
                }
                let result = check_execution_archive_v29(
                    &[Some(changed)],
                    &archive,
                    &place,
                    definition,
                    budget,
                );
                if mutation == 0 {
                    result.unwrap();
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                                if detail == "execution availability differs from its source SSA instance"
                        ),
                        "projected={projected}, mutation={mutation}"
                    );
                }
            }
        }
        Ok(())
    });
}

#[test]
fn source_enum_archive_keeps_missing_moved_and_nested_holder_refusals() {
    with_reference_enum(|value, budget| {
        for projected in [false, true] {
            let (archive, definition, place) = archive_inputs(&value, projected);
            for held in [
                None,
                Some(SemanticValueBindingV1::MovedExecution),
                Some(SemanticValueBindingV1::Unmaterialized),
            ] {
                assert!(
                    check_execution_archive_v29(&[held], &archive, &place, definition, budget)
                        .is_err()
                );
            }
            assert!(
                check_execution_archive_v29(
                    &[Some(value.clone())],
                    &SemanticSsaBindingsV1::default(),
                    &place,
                    definition,
                    budget
                )
                .is_err()
            );
            let aggregate = SemanticValueBindingV1::Aggregate(vec![value.clone()]);
            let mut path =
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ENUM).unwrap()];
            path.extend_from_slice(place.projections());
            let nested = SemanticPlaceV1::new(place.local(), path, place.ty()).unwrap();
            let archive = SemanticSsaBindingsV1::from([(definition, aggregate.clone())]);
            check_execution_archive_v29(&[Some(aggregate)], &archive, &nested, definition, budget)?;
            assert!(
                check_execution_archive_v29(
                    &[Some(SemanticValueBindingV1::Aggregate(vec![
                        SemanticValueBindingV1::MovedExecution
                    ]))],
                    &archive,
                    &nested,
                    definition,
                    budget
                )
                .is_err()
            );
        }
        Ok(())
    });
}

#[test]
fn source_enum_archive_has_exact_one_short_work_and_preserves_caller_storage() {
    with_reference_enum(|value, _| {
        for projected in [false, true] {
            let (archive, definition, place) = archive_inputs(&value, projected);
            let locals = [Some(value.clone())];
            let mut measured = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut measured, 37);
            budget.reserve_storage(37)?;
            check_execution_archive_v29(&locals, &archive, &place, definition, &mut budget)?;
            let exact = budget.work();
            assert!(exact > 0);
            assert_eq!(budget.storage(), 37);
            for limit in [exact, exact - 1] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = ArgumentBudgetV1::new(&mut work, 37);
                budget.reserve_storage(37)?;
                let result =
                    check_execution_archive_v29(&locals, &archive, &place, definition, &mut budget);
                if limit == exact {
                    result.unwrap();
                    assert_eq!(budget.work(), exact);
                    assert_eq!(budget.failed_work(), None);
                } else {
                    let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error),
                    )) = result
                    else {
                        panic!("expected exact archive work denial");
                    };
                    assert_eq!(error.limit(), limit);
                    assert_eq!(error.actual(), exact);
                    assert_eq!(budget.failed_work(), Some(error.actual()));
                    assert!(budget.work() <= limit);
                }
                assert_eq!(budget.storage(), 37);
            }
        }
        Ok(())
    });
}
