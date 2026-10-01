use super::*;

#[test]
fn witness_reborrow_frame_accounts_for_both_distinct_origin_rows() {
    type Fields = (
        Option<SourceIndexWitnessBorrowV29>,
        Option<SemanticValueBindingV1>,
        SourceReferenceSiteV29,
        Option<usize>,
        ValueDef,
        Option<SemanticCapabilityAvailabilityV1>,
        [&'static (); 9],
        [usize; 8],
    );
    assert_eq!(
        source_index_reborrow_headers_v43().unwrap(),
        std::mem::size_of::<Fields>()
            + 2 * std::mem::size_of::<Result<Fields, ProductionSemanticKirErrorV1>>()
    );
}

fn reborrow_checks(
    references: &SourceReferenceEmissionV29<'_, '_>,
    binding: &SemanticSourceReferenceBindingV29,
    expected: (SemanticTypeIdV1, SemanticDisjointIndexSpaceV1, bool),
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let loan = binding.origin.single_loan()?;
    let record = &references.plan.loans[loan];
    let proof = references.index_witnesses[loan].get().unwrap();
    assert_eq!(
        references.index_reader_value_v29(binding, binding.source_type, expected, budget)?,
        binding.values[0].id
    );
    if let Some(parent) = record.parent {
        let parent_proof = references.index_witnesses[parent].get().unwrap();
        assert_eq!(proof.origin, record.origin);
        assert_eq!(parent_proof.origin, references.plan.loans[parent].origin);
        assert_ne!(proof.origin, parent_proof.origin);
        let origin = &references.plan.origins[proof.origin];
        let parent_origin = &references.plan.origins[parent_proof.origin];
        assert_eq!(
            (
                origin.instance,
                origin.local,
                origin.generation,
                origin.ty,
                origin.anchor
            ),
            (
                parent_origin.instance,
                parent_origin.local,
                parent_origin.generation,
                parent_origin.ty,
                parent_origin.anchor
            )
        );
        assert!(origin.projections.is_empty());
        assert!(parent_origin.projections.is_empty());
        assert_eq!(proof.availability, parent_proof.availability);
        let function = references
            .plan
            .instances
            .instance(record.site.instance)
            .unwrap()
            .declaration();
        let SemanticStatementKindV1::Assign(assignment) = function.blocks()
            [record.site.block.index() as usize]
            .statements()[record.site.statement.unwrap()]
        .kind() else {
            panic!("original borrow assignment")
        };
        let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind() else {
            panic!("original borrow")
        };
        assert!(
            source_index_reborrow_proof_v43(references, record.site, place, loan, budget)?
                .is_some()
        );
        let cloned = place.clone();
        assert!(
            source_index_reborrow_proof_v43(references, record.site, &cloned, loan, budget)
                .is_err()
        );
        let claimed = references.claimed[parent].replace(false);
        let denied = source_index_reborrow_proof_v43(references, record.site, place, loan, budget);
        references.claimed[parent].set(claimed);
        assert!(denied.is_err());
        references.index_witnesses[parent].set(Some(SourceIndexWitnessBorrowV29 {
            origin: proof.origin,
            ..parent_proof
        }));
        let substituted =
            source_index_reborrow_proof_v43(references, record.site, place, loan, budget);
        references.index_witnesses[parent].set(Some(parent_proof));
        assert!(
            substituted.is_err(),
            "equal referent coordinates do not authorize a different loan receipt"
        );
        for access in [
            SourceReferenceAccessV29::Read,
            SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Mutable),
        ] {
            assert!(
                references
                    .reborrow_index_referent_v43(
                        binding,
                        binding.source_type,
                        record.site,
                        place,
                        0,
                        access,
                        budget
                    )
                    .is_err()
            );
        }
        assert!(
            source_index_reborrow_proof_v43(references, record.site, place, loan, budget)?
                .is_some()
        );
    }
    READER_CHECKS.set(READER_CHECKS.get() + 1);
    Ok(())
}

#[test]
fn original_witness_reborrows_preserve_loan_recipes_and_refuse_owned_dereference() {
    let _guard = ObserverGuard;
    SOURCE_INDEX_READER_OBSERVER_V29.set(Some(reborrow_checks));
    for depth in [1, 2, 4] {
        for copies in [false, true] {
            READER_CHECKS.set(0);
            cell_emission_tests::with_cell_source_lowered(
                index_reader_owner_with_reborrows_v43(true, copies, false, depth),
                |plan, references, emitted, _| {
                    assert_eq!(plan.loans.len(), 2 * (depth + 1));
                    assert!(
                        references
                            .index_witnesses
                            .iter()
                            .all(|proof| proof.get().is_some())
                    );
                    assert_eq!(READER_CHECKS.get(), 2);
                    assert_eq!(emitted.len(), 1);
                    Ok(())
                },
            )
            .unwrap_or_else(|error| panic!("depth={depth} copies={copies}: {error:?}"));
        }
    }
}

fn probe(
    depth: usize,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, usize) {
    let owner = index_production_owner_from_component_v40(index_reader_owner_with_reborrows_v43(
        true, true, false, depth,
    ));
    index_relation_probe_owner_v40(owner, work, storage, |relation, budget| {
        source_scalar_normalization_scratch_v18(relation.source.cleanup, budget, 0, |budget| {
            let rows = relation
                .source
                .root_row(0)?
                .rvalue_results
                .as_ref()
                .unwrap();
            assert_eq!(rows.index_readers.len(), 2);
            for block in [1, 3] {
                let reader = relation
                    .index_reader_computation_v35(
                        0,
                        0,
                        SemanticBlockIdV1::from_index(block),
                        budget,
                    )?
                    .unwrap();
                assert_eq!(
                    reader.expression(budget)?,
                    ProductionSemanticExpressionV2::GlobalInvocation1d {
                        scalar: ProductionSemanticScalarTypeV2::Integer {
                            signed: false,
                            bits: 64
                        },
                    }
                );
            }
            Ok(())
        })
    })
}

#[test]
fn original_witness_nested_reborrows_reach_mandatory_production_correspondence() {
    for depth in [1, 2, 4] {
        probe(depth, 1_000_000_000, 256 << 20).0.unwrap();
    }
}

#[test]
fn original_witness_reborrow_complete_transaction_has_exact_resource_boundaries() {
    let (result, work, storage, retained) = probe(2, 1_000_000_000, 256 << 20);
    result.unwrap();
    assert_eq!(retained, 0);
    let exact = probe(2, work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, storage, 0));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let refused = probe(2, work_limit, storage_limit).0;
        let mut error: &(dyn std::error::Error + 'static) = refused.as_ref().unwrap_err();
        let resource = loop {
            if let Some(resource) = error.downcast_ref::<ArgumentResourceV1>() {
                break resource;
            }
            error = error
                .source()
                .unwrap_or_else(|| panic!("missing resource cause: {refused:?}"));
        };
        match resource {
            ArgumentResourceV1::Work(limit) if is_work => {
                assert_eq!(limit.limit(), work_limit);
                assert!(limit.actual() > limit.limit());
            }
            ArgumentResourceV1::Storage(limit) if !is_work => {
                assert_eq!(limit.limit(), storage_limit);
                assert!(limit.actual() > limit.limit());
            }
            other => panic!("wrong resource: {other:?}"),
        }
    }
}

fn corrupt_reborrow(
    this: &mut SemanticFunctionLoweringV1<'_, '_>,
    _: SemanticBlockIdV1,
    _: Option<u32>,
    loan: usize,
) {
    let references = this.execution.as_ref().unwrap().references.unwrap();
    let Some(parent) = references.plan.loans[loan].parent else {
        return;
    };
    if BORROW_FAULT.get() == 0 {
        let SemanticValueBindingV1::SourceReference(reference) = this.locals[7].as_mut().unwrap()
        else {
            panic!("copied parent reference")
        };
        reference.values[0].id = ValueId(u32::MAX);
    } else {
        references.claimed[parent].set(false);
    }
}

#[test]
fn original_witness_reborrow_rejects_same_owner_substituted_carrier_and_stale_parent() {
    let _guard = ObserverGuard;
    SOURCE_INDEX_BORROW_OBSERVER_V29.set(Some(corrupt_reborrow));
    for fault in [0, 1] {
        BORROW_FAULT.set(fault);
        let refused = cell_emission_tests::with_cell_source_lowered(
            index_reader_owner_with_reborrows_v43(true, true, false, 1),
            |_, _, _, _| panic!("substituted reborrow must not complete"),
        );
        assert!(
            matches!(
                refused,
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ),
            "fault={fault}: {refused:?}"
        );
    }
}
