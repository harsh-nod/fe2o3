use super::*;
use std::mem::{align_of, size_of};

fn check_referents(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let archive = relation
        .source
        .root_row(0)?
        .rvalue_results
        .as_ref()
        .unwrap();
    let semantic = relation.source.source_semantic(budget)?;
    assert_eq!(archive.index_readers.len(), 2);
    for reader in &archive.index_readers {
        let endpoint =
            relation.ssa_typed_endpoint_v36(0, reader.instance, reader.argument, budget)?;
        let reference = endpoint
            .reference(budget)?
            .expect("checked scalar referent");
        assert_eq!(
            reference.carrier(budget)?,
            ProductionSourceReferenceCarrierV38::StableScalar
        );
        assert_eq!(
            reference.origin_instance(budget)?,
            reader.origin_instance.index()
        );
        assert_eq!(reference.origin_local(budget)?, reader.origin_local);
        assert_eq!(reference.origin_type(budget)?, reader.witness_type);
        assert_eq!(
            reference.origin_generation(budget)?,
            reader.origin_generation
        );
        let function = reference.origin_function(budget)?;
        assert_eq!(
            function,
            relation
                .source
                .instance(0, reader.origin_instance.index(), budget)?
                .0
        );
        assert_eq!(
            semantic.functions()[function.index() as usize].locals()
                [reader.origin_local.index() as usize]
                .ty(),
            reader.witness_type
        );
        let (instance, block, statement) = reference.borrow_site(budget)?;
        assert_eq!(
            (instance, block, statement),
            (
                reader.loan_site.instance.index(),
                reader.loan_site.block,
                reader.loan_site.statement,
            )
        );
        let (function, _) = relation.source.instance(0, instance, budget)?;
        let statement = &semantic.functions()[function.index() as usize].blocks()
            [block.index() as usize]
            .statements()[statement.unwrap()];
        assert!(
            matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared, place,
            } if place.local() == reader.origin_local && place.projections().is_empty()))
        );
        assert!(matches!(
            endpoint.physical_type(budget)?,
            Some(Type::Scalar(_))
        ));
        let physical = endpoint.original_definition(budget)?.unwrap();
        assert!(matches!(
            relation.inventory.definitions()[physical].ty,
            Type::Scalar(_)
        ));
        let result = relation.ssa_typed_endpoint_v36(0, reader.instance, reader.result, budget)?;
        assert!(
            result.reference(budget)?.is_none(),
            "ordinary reader result is not a borrowed reference"
        );
    }
    Ok(())
}

#[test]
fn source_stable_referent_endpoint_joins_genuine_borrow_origin_and_both_index_readers() {
    with_index_relation_v35(check_referents).unwrap();
}

#[test]
fn source_stable_referent_endpoint_exact_and_one_short_complete_resources() {
    let run = |work, storage| index_relation_probe_v35(work, storage, check_referents);
    let measured = run(1_000_000_000, 256 << 20);
    measured.0.unwrap();
    let exact = run(measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    for short_work in [true, false] {
        let result = run(
            measured.1 - usize::from(short_work),
            measured.2 - usize::from(!short_work),
        );
        let refusal = result.0.expect_err("one-short account must be refused");
        let error =
            crate::production_semantic_kir_v1::aggregate_source_owned_resource_v30(&refusal)
                .unwrap_or_else(|| panic!("not a resource refusal: {refusal:?}"));
        assert!(
            matches!(error, ArgumentResourceV1::Work(_) if short_work)
                || matches!(error, ArgumentResourceV1::Storage(_) if !short_work),
            "short_work={short_work}, error={error:?}"
        );
    }
}

#[test]
fn source_stable_referent_getters_have_constant_work_and_no_retained_allocation() {
    with_index_relation_v35(|relation, budget| {
        let reader = &relation
            .source
            .root_row(0)?
            .rvalue_results
            .as_ref()
            .unwrap()
            .index_readers[0];
        let endpoint =
            relation.ssa_typed_endpoint_v36(0, reader.instance, reader.argument, budget)?;
        let reference = endpoint.reference(budget)?.unwrap();
        let storage = budget.storage();
        let mut cost = None;
        for _ in 0..64 {
            let work = budget.work();
            reference.carrier(budget)?;
            reference.origin_instance(budget)?;
            reference.origin_function(budget)?;
            reference.origin_local(budget)?;
            reference.origin_type(budget)?;
            reference.origin_generation(budget)?;
            reference.borrow_site(budget)?;
            let actual = budget.work() - work;
            assert_eq!(*cost.get_or_insert(actual), actual);
            assert_eq!(budget.storage(), storage);
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_stable_referent_endpoint_replay_rejects_class_function_type_and_generation_changes() {
    with_index_relation_v35(|relation, budget| {
        let original = relation
            .source
            .root_row(0)?
            .rvalue_results
            .as_ref()
            .unwrap();
        for fault in 0..5 {
            let mut changed = OwnedSourceRvaluesV30 {
                source: original.source,
                ledger: original.ledger,
                storage: original.storage,
                rows: original.rows.clone(),
                values: original.values.clone(),
                index_readers: original.index_readers.clone(),
                carriers: original.carriers.clone(),
            };
            let loan = changed
                .values
                .iter_mut()
                .find_map(|row| match &mut row.typed.physical {
                    SourceSsaPhysicalV36::Value {
                        loan: Some(loan), ..
                    } if loan.carrier == ProductionSourceReferenceCarrierV38::StableScalar => {
                        Some(loan)
                    }
                    _ => None,
                })
                .expect("genuine checked stable referent");
            match fault {
                0 => {}
                1 => loan.carrier = ProductionSourceReferenceCarrierV38::MemoryPointer,
                2 => loan.origin_function = SemanticFunctionIdV1::from_index(u32::MAX),
                3 => loan.origin_type = SemanticTypeIdV1::from_index(u32::MAX),
                4 => loan.origin_generation ^= 1,
                _ => unreachable!(),
            }
            assert_eq!(
                original
                    .matches_replay_v30(&changed, budget)
                    .map_err(source_emission_error_v18)?,
                fault == 0
            );
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_stable_referent_getter_rejects_funded_foreign_ledger_and_retains_failure() {
    let at_refusal = std::cell::Cell::new(None);
    let (result, _, _, remaining) =
        index_relation_probe_v35(1_000_000_000, 256 << 20, |relation, budget| {
            let reader = &relation
                .source
                .root_row(0)?
                .rvalue_results
                .as_ref()
                .unwrap()
                .index_readers[0];
            let endpoint =
                relation.ssa_typed_endpoint_v36(0, reader.instance, reader.argument, budget)?;
            let reference = endpoint.reference(budget)?.unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
            let mut foreign = ArgumentBudgetV1::new(&mut work, 256 << 20);
            foreign.reserve_storage(budget.storage())?;
            let before = (foreign.work(), foreign.storage());
            assert!(matches!(
                reference.origin_type(&mut foreign),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            assert_eq!((foreign.work(), foreign.storage()), before);
            assert!(matches!(
                reference.origin_type(budget),
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
            at_refusal.set(Some(budget.storage()));
            Ok(())
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert_eq!(Some(remaining), at_refusal.get());
}

#[test]
fn source_stable_referent_endpoint_borrowed_header_has_independent_field_layout() {
    type Fields = (
        &'static ProductionSourceCorrespondenceV18<'static>,
        &'static SourceSsaLoanV36,
    );
    assert_eq!(
        size_of::<Fields>(),
        size_of::<ProductionSourceReferenceEndpointV38<'_, '_>>()
    );
    assert_eq!(
        align_of::<Fields>(),
        align_of::<ProductionSourceReferenceEndpointV38<'_, '_>>()
    );
}
