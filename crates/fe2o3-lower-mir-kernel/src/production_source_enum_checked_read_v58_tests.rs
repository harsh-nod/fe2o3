use super::*;

fn original_read<'a>(
    plan: &'a SourceReferencePlanV29<'_, '_>,
    row: &SourceReferenceAccessRecordV29,
) -> &'a SemanticPlaceV1 {
    let function = plan
        .instances
        .instance(row.key.site.instance)
        .unwrap()
        .declaration();
    let statement = &function.blocks()[row.key.site.block.index() as usize].statements()
        [row.key.site.statement.unwrap()];
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        panic!("original assignment")
    };
    let SemanticRvalueKindV1::Use(
        SemanticOperandV1::Copy(source) | SemanticOperandV1::Move(source),
    ) = assignment.value().kind()
    else {
        panic!("original payload read")
    };
    assert_eq!(row.key.source, source as *const SemanticPlaceV1 as usize);
    source
}

#[test]
fn checked_enum_read_requires_exact_original_occurrence_generation_and_path() {
    for case in [EnumCase::Construct(0), EnumCase::ConditionalVariants] {
        let mut checked = 0;
        run_enum(case, |plan, budget| {
            assert!(!plan.has_storage_demands);
            assert!(plan.storage_root.is_some());
            for row in &plan.accesses {
                let Some(claim) = row.checked_enum_read else {
                    continue;
                };
                let source = original_read(plan, row);
                let SemanticProjectionKindV1::Downcast(variant) = source.projections()[0].kind()
                else {
                    panic!("root downcast")
                };
                assert!(source_enum_checked_read_at_v58(
                    plan,
                    row.key.site,
                    source,
                    ENUM,
                    variant,
                    budget
                )?);
                assert!(!source_enum_checked_read_at_v58(
                    plan,
                    row.key.site,
                    &source.clone(),
                    ENUM,
                    variant,
                    budget
                )?);
                assert!(!source_enum_checked_read_at_v58(
                    plan,
                    row.key.site,
                    source,
                    WORD,
                    variant,
                    budget
                )?);
                assert!(!source_enum_checked_read_at_v58(
                    plan,
                    row.key.site,
                    source,
                    ENUM,
                    (variant + 1) % 3,
                    budget
                )?);
                let other_site = SourceReferenceSiteV29 {
                    statement: Some(usize::MAX),
                    ..row.key.site
                };
                assert!(!source_enum_checked_read_at_v58(
                    plan, other_site, source, ENUM, variant, budget
                )?);
                for mutation in 0..4 {
                    let mut changed = SourceReferenceAccessRecordV29 {
                        key: row.key,
                        source_local: row.source_local,
                        ty: row.ty,
                        instance: row.instance,
                        local: row.local,
                        generation: row.generation,
                        projections: row.projections.clone(),
                        loan: row.loan,
                        traversed: row.traversed.clone(),
                        shared_path: row.shared_path,
                        checked_enum_read: row.checked_enum_read,
                    };
                    match mutation {
                        0 => changed.generation = changed.generation.wrapping_add(1),
                        1 => changed.loan = Some(0),
                        2 => changed.projections = 0..0,
                        _ => changed.local = SemanticLocalIdV1::from_index(0),
                    }
                    assert!(!claim.allows(
                        plan,
                        &changed,
                        row.key.site,
                        source,
                        ENUM,
                        variant,
                        budget
                    )?);
                }
                checked += 1;
            }
            Ok(())
        })
        .unwrap();
        assert!(
            checked >= 2,
            "both genuine helper instances must retain checked payload reads"
        );
    }
}

#[test]
fn checked_enum_read_refuses_an_equal_credit_foreign_ledger() {
    let reached = std::cell::Cell::new(false);
    let result = run_enum(EnumCase::Construct(0), |plan, _| {
        let row = plan
            .accesses
            .iter()
            .find(|row| row.checked_enum_read.is_some())
            .unwrap();
        let source = original_read(plan, row);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
        foreign.reserve_storage(37)?;
        assert!(
            source_enum_checked_read_at_v58(plan, row.key.site, source, ENUM, 0, &mut foreign)
                .is_err()
        );
        assert_eq!(foreign.work(), 0);
        assert_eq!(foreign.storage(), 37);
        reached.set(true);
        Ok(())
    });
    assert!(result.is_err());
    assert!(reached.get());
}

#[test]
fn checked_enum_read_current_holder_comparison_rejects_tag_payload_and_type_substitution() {
    let value = SemanticValueBindingV1::Enum {
        discriminant: ValueId(41),
        discriminant_ty: Type::Scalar(ScalarType::U8),
        semantic_type: ENUM,
        variant: None,
        payloads: BTreeMap::from([(
            0,
            vec![SemanticValueBindingV1::Value {
                id: ValueId(42),
                ty: Type::Scalar(ScalarType::U32),
            }],
        )]),
    };
    for mutation in 0..6 {
        let mut changed = value.clone();
        let SemanticValueBindingV1::Enum {
            discriminant,
            semantic_type,
            variant,
            payloads,
            ..
        } = &mut changed
        else {
            unreachable!()
        };
        match mutation {
            1 => *discriminant = ValueId(43),
            2 => *semantic_type = WORD,
            3 => *variant = Some(0),
            4 => {
                payloads.get_mut(&0).unwrap()[0] = SemanticValueBindingV1::Value {
                    id: ValueId(43),
                    ty: Type::Scalar(ScalarType::U32),
                }
            }
            5 => {
                payloads.get_mut(&0).unwrap()[0] = SemanticValueBindingV1::Value {
                    id: ValueId(42),
                    ty: Type::Scalar(ScalarType::U64),
                }
            }
            _ => {}
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        assert_eq!(
            source_enum_current_binding_same_v58(&value, &changed, &mut 0, &mut budget).unwrap(),
            mutation == 0
        );
    }
}
