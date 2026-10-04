use super::*;

fn paired_tag_owner() -> ProductionSemanticSsaOwnerV1 {
    try_enum_owner_transform(EnumCase::CorrelatedLoans, |types, functions| {
        let original = &types[ENUM.index() as usize];
        types[ENUM.index() as usize] = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            original.layout().clone(),
            SemanticTypeShapeV1::enum_type(
                WORD,
                TAGS.iter()
                    .map(|&tag| {
                        SemanticEnumVariantV1::new(
                            tag,
                            SemanticAggregateTypeV1::new(vec![REFERENCE, REFERENCE]).unwrap(),
                        )
                    })
                    .collect(),
            )
            .unwrap(),
        );
        let original = &functions[2];
        let mut blocks = original.blocks().to_vec();
        for (block, first, second) in [(1, 2, 4), (2, 4, 2)] {
            let old = &blocks[block];
            blocks[block] = SemanticBasicBlockV1::new(
                old.identity(),
                old.source(),
                vec![assign(
                    place(5, ENUM),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::EnumVariant(0),
                            vec![
                                SemanticOperandV1::Copy(place(first, REFERENCE)),
                                SemanticOperandV1::Copy(place(second, REFERENCE)),
                            ],
                        )
                        .unwrap(),
                    ),
                )],
                old.terminator().clone(),
            )
            .unwrap();
        }
        let old = &blocks[3];
        blocks[3] = SemanticBasicBlockV1::new(
            old.identity(),
            old.source(),
            vec![
                assign(
                    place(6, WORD),
                    SemanticRvalueKindV1::Discriminant(place(5, ENUM)),
                ),
                unit(),
            ],
            old.terminator().clone(),
        )
        .unwrap();
        functions[2] = function(30, false, CAPTURE, original.locals().to_vec(), blocks);
    })
    .unwrap()
}

fn joined_parent(plan: &SourceReferencePlanV29<'_, '_>) -> usize {
    plan.nodes
        .iter()
        .enumerate()
        .find_map(|(index, row)| {
            (row.ty == ENUM
                && matches!(row.kind, SourceReferenceNodeKindV29::Enum { count, .. } if count > 1))
            .then_some(index)
        })
        .expect("genuine original joined enum parent")
}

#[test]
fn source_enum_tag_retains_complete_same_variant_alternatives_without_payload_authority() {
    let reached = std::cell::Cell::new(false);
    run_enum(EnumCase::CorrelatedLoans, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let node = joined_parent(plan);
        let SourceReferenceNodeKindV29::Enum { first, count } = plan.nodes[node].kind else {
            unreachable!()
        };
        assert!(count > 1);
        let members = &plan.enum_members[first..first + count];
        assert!(
            members
                .iter()
                .all(|member| plan.enum_alternatives[*member].variant == 0)
        );
        let tag_type = source_enum_tag_type_v55(plan, node, budget)?;
        let binding = rebuild_source_enum_tag_v55(
            &references,
            node,
            &[ValueDef::new(ValueId(700), tag_type)],
            budget,
        )?;
        let SemanticValueBindingV1::SourceEnumTag(tag) = &binding else {
            panic!("source-owned tag")
        };
        assert_eq!(tag.node, node);
        assert_eq!(tag.tag.id, ValueId(700));
        assert!(binding.value().is_err());
        assert!(binding.values().is_err());
        assert!(!semantic_binding_can_restore_from_unique_source_v1(
            &binding
        ));
        merge_source_enum_tag_v55(&references, node, &binding, &binding, budget)?;
        let extracted = source_enum_transport_tag_v55(&references, node, &binding, budget)?;
        assert_eq!(extracted.id, tag.tag.id);
        assert_eq!(extracted.ty, tag.tag.ty);
        assert_eq!(&plan.enum_members[first..first + count], members);
        reached.set(true);
        Ok(())
    })
    .unwrap();
    assert!(reached.get());
}

#[test]
fn source_enum_tag_retains_only_its_logical_discriminant_endpoint() {
    let reached = std::cell::Cell::new(false);
    run_enum(EnumCase::CorrelatedLoans, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let node = joined_parent(plan);
        let tag_type = source_enum_tag_type_v55(plan, node, budget)?;
        let binding = rebuild_source_enum_tag_v55(
            &references,
            node,
            &[ValueDef::new(ValueId(710), tag_type)],
            budget,
        )?;
        let mut carriers = source_reference_owned_vec_v29(plan, 0, budget)?;
        let physical = retain_source_carrier_tree_v37(
            plan.instances,
            &references,
            ENUM,
            &binding,
            &mut carriers,
            budget,
        )?;
        assert_eq!(
            physical,
            SourceSsaPhysicalV36::Enum {
                discriminant: 0,
                start: 1,
                length: 0,
                known_variant: None,
                presence: None,
            }
        );
        assert_eq!(carriers.len(), 1);
        assert_eq!(carriers[0].ty, WORD);
        assert!(matches!(
            carriers[0].physical,
            SourceSsaPhysicalV36::Value {
                value: ValueId(710),
                ty: SourceSsaCarrierTypeV36::Scalar(_),
                loan: None,
            }
        ));
        assert!(
            retain_source_enum_tag_carriers_v55(
                plan.instances,
                &references,
                REFERENCE,
                &binding,
                &mut carriers,
                budget,
            )
            .is_err()
        );
        assert_eq!(
            carriers.len(),
            1,
            "mismatched source type cannot append a carrier"
        );
        reached.set(true);
        Ok(())
    })
    .unwrap();
    assert!(reached.get());
}

#[test]
fn source_enum_tag_temporary_refund_preserves_callback_credit_on_all_exits() {
    for exit in 0..3 {
        let reached = std::cell::Cell::new(false);
        run_enum(EnumCase::CorrelatedLoans, |plan, budget| {
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let node = joined_parent(plan);
            let tag_type = source_enum_tag_type_v55(plan, node, budget)?;
            let binding = rebuild_source_enum_tag_v55(
                &references,
                node,
                &[ValueDef::new(ValueId(711), tag_type)],
                budget,
            )?;
            let floor = budget.storage();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_source_enum_transport_tag_v55(
                    &references,
                    node,
                    &binding,
                    budget,
                    |values, budget| {
                        assert_eq!(values.len(), 1);
                        assert_eq!(values[0].id, ValueId(711));
                        budget.reserve_storage(53)?;
                        match exit {
                            0 => Ok(()),
                            1 => Err(source_reference_error_v29("tag callback test refusal")),
                            _ => panic!("tag callback test unwind"),
                        }
                    },
                )
            }));
            match (exit, result) {
                (0, Ok(Ok(()))) | (1, Ok(Err(_))) | (2, Err(_)) => {}
                _ => panic!("tag callback exit differs"),
            }
            assert_eq!(
                budget.storage(),
                floor + 53,
                "temporary tag credit must not refund callback-owned storage"
            );
            budget.release_storage(53)?;
            references.check(budget)?;
            reached.set(true);
            Ok(())
        })
        .unwrap();
        assert!(reached.get());
    }
}

#[test]
fn source_enum_tag_rejects_foreign_owner_identity_node_type_and_archive_substitution() {
    let reached = std::cell::Cell::new(false);
    run_enum(EnumCase::CorrelatedLoans, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let node = joined_parent(plan);
        let tag_type = source_enum_tag_type_v55(plan, node, budget)?;
        let binding = rebuild_source_enum_tag_v55(
            &references,
            node,
            &[ValueDef::new(ValueId(700), tag_type)],
            budget,
        )?;
        let SemanticValueBindingV1::SourceEnumTag(tag) = &binding else {
            unreachable!()
        };
        for mutation in 0..5 {
            let mut changed = tag.clone();
            match mutation {
                0 => changed.owner ^= 1,
                1 => changed.source[0] ^= 1,
                2 => changed.node = usize::MAX,
                3 => changed.tag.ty = Type::Unit,
                _ => changed.tag.id = ValueId(701),
            }
            let changed = SemanticValueBindingV1::SourceEnumTag(changed);
            assert!(
                merge_source_enum_tag_v55(&references, node, &binding, &changed, budget).is_err(),
                "mutation {mutation}"
            );
        }
        assert!(rebuild_source_enum_tag_v55(&references, node, &[], budget).is_err());
        assert!(
            rebuild_source_enum_tag_v55(
                &references,
                node,
                &[tag.tag.clone(), tag.tag.clone()],
                budget
            )
            .is_err()
        );
        reached.set(true);
        Ok(())
    })
    .unwrap();
    assert!(reached.get());
}

#[test]
fn source_enum_parent_transport_preserves_whole_original_member_subset() {
    let reached = std::cell::Cell::new(false);
    run_enum(EnumCase::CorrelatedLoans, |plan, budget| {
        let joined = joined_parent(plan);
        let SourceReferenceNodeKindV29::Enum { first, count } = plan.nodes[joined].kind else {
            unreachable!()
        };
        assert!(count > 1);
        let members = &plan.enum_members[first..first + count];
        let singleton = plan
            .nodes
            .iter()
            .enumerate()
            .find_map(|(node, row)| {
                let SourceReferenceNodeKindV29::Enum { first, count: 1 } = row.kind else {
                    return None;
                };
                (row.ty == ENUM && members.contains(&plan.enum_members[first])).then_some(node)
            })
            .expect("genuine original constructor member");
        assert!(source_enum_parent_subset_v55(
            plan, singleton, joined, budget
        )?);
        assert!(!source_enum_parent_subset_v55(
            plan, joined, singleton, budget
        )?);
        assert!(source_enum_parent_subset_v55(plan, joined, joined, budget)?);
        reached.set(true);
        Ok(())
    })
    .unwrap();
    assert!(reached.get());
}

#[test]
fn source_enum_tag_refuses_cross_member_reference_tuples() {
    let reached = std::cell::Cell::new(false);
    run_enum_with_original_demands(paired_tag_owner(), |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let joined = joined_parent(plan);
        let SourceReferenceNodeKindV29::Enum { first, count } = plan.nodes[joined].kind else {
            unreachable!()
        };
        assert!(count >= 2);
        let members = &plan.enum_members[first..first + count];
        let node = plan
            .nodes
            .iter()
            .enumerate()
            .find_map(|(node, row)| {
                let SourceReferenceNodeKindV29::Enum { first, count: 1 } = row.kind else {
                    return None;
                };
                (row.ty == ENUM && members.contains(&plan.enum_members[first])).then_some(node)
            })
            .expect("original paired constructor");
        let values = source_reference_node_types_v29(plan, node, budget)?
            .into_iter()
            .enumerate()
            .map(|(index, ty)| ValueDef::new(ValueId(800 + index as u32), ty))
            .collect::<Vec<_>>();
        let mut values = values.iter();
        let mut binding = source_reference_rebuild_node_v29(
            &references,
            node,
            false,
            &mut [].iter(),
            &mut values,
            &mut 0,
            budget,
        )?;
        assert!(values.next().is_none());
        let SemanticValueBindingV1::Enum { variant, .. } = &mut binding else {
            unreachable!()
        };
        *variant = Some(0);
        merge_source_enum_tag_v55(&references, joined, &binding, &binding, budget)?;
        let mut forged = binding.clone();
        let SemanticValueBindingV1::Enum { payloads, .. } = &mut forged else {
            unreachable!()
        };
        let fields = payloads.get_mut(&0).unwrap();
        assert_eq!(fields.len(), 2);
        let (
            SemanticValueBindingV1::SourceReference(left),
            SemanticValueBindingV1::SourceReference(right),
        ) = (&fields[0], &fields[1])
        else {
            unreachable!()
        };
        assert_ne!(left.origin, right.origin);
        fields[1] = fields[0].clone();
        assert!(merge_source_enum_tag_v55(&references, joined, &forged, &forged, budget).is_err());
        reached.set(true);
        Ok(())
    })
    .unwrap();
    assert!(reached.get());
}
