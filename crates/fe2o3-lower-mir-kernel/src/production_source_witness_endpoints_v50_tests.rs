use super::*;
use std::mem::{align_of, size_of};

fn witness_rows_v50(
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
    let mut counts = [0usize; 2];
    for row in &archive.values {
        let SourceSsaPhysicalV36::Witness(witness) = row.typed.physical else {
            continue;
        };
        let endpoint = relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
        assert_eq!(endpoint.source_local(budget)?, row.typed.local);
        assert_eq!(endpoint.source_type(budget)?, row.typed.ty);
        let function = endpoint.source_function(budget)?;
        assert_eq!(
            function,
            relation.source.instance(0, row.instance, budget)?.0
        );
        assert_eq!(
            semantic.functions()[function.index() as usize].locals()
                [row.typed.local.index() as usize]
                .ty(),
            row.typed.ty
        );
        let expected_type = if witness.disjoint { DISJOINT } else { THREAD };
        assert_eq!(row.typed.ty, expected_type);
        assert_eq!(witness.index_space, SemanticDisjointIndexSpaceV1::Index1d);
        assert_eq!(witness.availability, None);
        assert_eq!(
            endpoint.carrier_shape(budget)?,
            ProductionSourceSsaCarrierShapeV37::Value
        );
        assert_eq!(endpoint.physical_type(budget)?, Some(&Type::INDEX));
        assert!(endpoint.reference(budget)?.is_none());
        let definition = endpoint.original_definition(budget)?.unwrap();
        assert_eq!(
            relation.inventory.definitions()[definition].value,
            Some(witness.value)
        );
        assert_eq!(
            relation.inventory.definitions()[definition].ty,
            &Type::INDEX
        );
        assert_eq!(row.endpoint, SourceRvalueEndpointV30::Unmodeled);
        counts[usize::from(witness.disjoint)] += 1;
    }
    assert!(counts.into_iter().all(|count| count > 0));
    assert_eq!(archive.index_readers.len(), 2);
    for reader in &archive.index_readers {
        let result = archive
            .values
            .iter()
            .find(|row| row.instance == reader.instance && row.original == reader.result)
            .unwrap();
        assert_eq!(result.typed.ty, INDEX);
        assert!(matches!(result.typed.physical,
            SourceSsaPhysicalV36::Value {
                value, ty: SourceSsaCarrierTypeV36::Scalar(ScalarType::Index), loan: None,
            } if value == reader.emitted_index));
        assert!(archive.values.iter().any(|row| {
            row.instance == reader.origin_instance.index()
                && row.typed.local == reader.origin_local
                && row.typed.ty == reader.witness_type
                && matches!(row.typed.physical, SourceSsaPhysicalV36::Witness(witness)
                    if witness.disjoint == reader.disjoint && witness.index_space == reader.index_space)
        }));
        let endpoint =
            relation.ssa_typed_endpoint_v36(0, reader.instance, reader.result, budget)?;
        assert!(endpoint.reference(budget)?.is_none());
    }
    Ok(())
}

#[test]
fn direct_witness_endpoints_preserve_nominal_class_through_moves_and_distinguish_plain_index() {
    for moves in [false, true] {
        let reached = std::cell::Cell::new(false);
        let owner = index_production_owner_from_component_v40(
            index_reader_owner_with_transfers_v40(moves, true),
        );
        let (result, _, _, remaining) =
            index_relation_probe_owner_v40(owner, 1_000_000_000, 256 << 20, |relation, budget| {
                witness_rows_v50(relation, budget)?;
                reached.set(true);
                Ok(())
            });
        result.unwrap();
        assert!(reached.get());
        assert_eq!(remaining, 0);
    }
}

fn copied_rows_v50<T: Copy>(
    rows: &[T],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    let mut copy = emission_vec_v1(rows.len(), budget)?;
    budget.charge_work(rows.len())?;
    copy.extend_from_slice(rows);
    Ok(copy)
}

#[test]
fn direct_witness_replay_rejects_nominal_instance_definition_space_and_availability_substitutions()
{
    let reached = std::cell::Cell::new(0);
    with_index_relation_v35(|relation, budget| {
        witness_rows_v50(relation, budget)?;
        let original = relation.source.root_row(0)?.rvalue_results.as_ref().unwrap();
        assert!(original.enum_spills.is_empty());
        let at = original.values.iter().position(|row| {
            matches!(row.typed.physical, SourceSsaPhysicalV36::Witness(witness) if !witness.disjoint)
        }).unwrap();
        for fault in 0..10 {
            source_scalar_normalization_scratch_v18(relation.source.cleanup, budget, 0, |budget| {
                let mut changed = OwnedSourceRvaluesV30 {
                    source: original.source,
                    ledger: original.ledger,
                    storage: original.storage,
                    rows: copied_rows_v50(&original.rows, budget).map_err(source_emission_error_v18)?,
                    values: copied_rows_v50(&original.values, budget).map_err(source_emission_error_v18)?,
                    carriers: copied_rows_v50(&original.carriers, budget).map_err(source_emission_error_v18)?,
                    index_readers: copied_rows_v50(&original.index_readers, budget).map_err(source_emission_error_v18)?,
                    enum_spills: Vec::new(),
                };
                let row = &mut changed.values[at];
                match fault {
                    0 => {}
                    1 => row.instance = usize::MAX,
                    2 => row.typed.local = SemanticLocalIdV1::from_index(u32::MAX),
                    3 => row.typed.ty = DISJOINT,
                    4 => row.original = original.index_readers[0].result,
                    _ => {
                        let SourceSsaPhysicalV36::Witness(witness) = &mut row.typed.physical else {
                            unreachable!()
                        };
                        match fault {
                            5 => witness.value = ValueId(u32::MAX),
                            6 => witness.index_space = SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 7 },
                            7 => witness.disjoint = true,
                            8 => witness.availability = Some(SemanticCapabilityAvailabilityV1::EnumPayload {
                                local: row.typed.local, variant: 1,
                            }),
                            9 => row.typed.physical = SourceSsaPhysicalV36::Value {
                                value: witness.value,
                                ty: SourceSsaCarrierTypeV36::Scalar(ScalarType::Index),
                                loan: None,
                            },
                            _ => unreachable!(),
                        }
                    }
                }
                assert_eq!(original.matches_replay_v30(&changed, budget)
                    .map_err(source_emission_error_v18)?, fault == 0);
                reached.set(reached.get() + 1);
                drop(changed);
                Ok(())
            })?;
        }
        Ok(())
    }).unwrap();
    assert_eq!(reached.get(), 10);
}

#[test]
fn direct_witness_endpoint_requires_exact_canonical_index_definition() {
    let reached = std::cell::Cell::new(false);
    let result = with_index_relation_v35(|relation, budget| {
        witness_rows_v50(relation, budget)?;
        let archive = relation
            .source
            .root_row(0)?
            .rvalue_results
            .as_ref()
            .unwrap();
        let row = archive
            .values
            .iter()
            .find(|row| matches!(row.typed.physical, SourceSsaPhysicalV36::Witness(_)))
            .unwrap();
        let endpoint = relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
        let SourceSsaPhysicalV36::Witness(mut witness) = *endpoint.physical else {
            unreachable!()
        };
        assert!(SourceSsaCarrierTypeV36::Scalar(ScalarType::Index).matches(&Type::INDEX));
        for scalar in [
            ScalarType::U32,
            ScalarType::U64,
            ScalarType::I64,
            ScalarType::Bool,
        ] {
            assert!(
                !SourceSsaCarrierTypeV36::Scalar(ScalarType::Index).matches(&Type::Scalar(scalar))
            );
        }
        witness.value = ValueId(u32::MAX);
        let denied = relation.retain_query(source_carrier_definition_v37(
            relation,
            endpoint.coordinate,
            &SourceSsaPhysicalV36::Witness(witness),
            &archive.carriers,
            budget,
        ));
        assert!(matches!(
            denied,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "original typed SSA canonical value is absent"
            ))
        ));
        let before = (budget.work(), budget.storage());
        assert!(matches!(
            endpoint.original_definition(budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "original typed SSA canonical value is absent"
            ))
        ));
        assert_eq!((budget.work(), budget.storage()), before);
        reached.set(true);
        Ok(())
    });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "original typed SSA canonical value is absent"
        ))
    ));
}

#[test]
fn direct_witness_queries_preserve_exact_work_foreign_ledger_and_restored_floor_refusals() {
    const LIMIT: usize = 1_000_000_000;
    for fault in 0..3 {
        let reached = std::cell::Cell::new(false);
        let held = std::cell::Cell::new(0);
        let (result, _, _, remaining) =
            index_relation_probe_v35(LIMIT, 256 << 20, |relation, budget| {
                witness_rows_v50(relation, budget)?;
                let archive = relation
                    .source
                    .root_row(0)?
                    .rvalue_results
                    .as_ref()
                    .unwrap();
                let row = archive
                    .values
                    .iter()
                    .find(|row| matches!(row.typed.physical, SourceSsaPhysicalV36::Witness(_)))
                    .unwrap();
                let endpoint =
                    relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
                for _ in 0..32 {
                    let before = (budget.work(), budget.storage());
                    endpoint.carrier_shape(budget)?;
                    endpoint.original_definition(budget)?;
                    endpoint.physical_type(budget)?;
                    endpoint.reference(budget)?;
                    assert_eq!(budget.work() - before.0, 2 + 2 + 2 + 3);
                    assert_eq!(budget.storage(), before.1);
                }
                let first = match fault {
                    0 => {
                        budget.charge_work(LIMIT - budget.work() - 1)?;
                        endpoint.carrier_shape(budget).unwrap_err()
                    }
                    1 => {
                        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                        let mut foreign = ArgumentBudgetV1::new(&mut work, 256 << 20);
                        foreign.reserve_storage(budget.storage())?;
                        let before = (foreign.work(), foreign.storage());
                        let denied = endpoint.carrier_shape(&mut foreign).unwrap_err();
                        assert_eq!((foreign.work(), foreign.storage()), before);
                        denied
                    }
                    2 => {
                        let floor = budget.storage();
                        budget.release_storage(floor)?;
                        let denied = endpoint.carrier_shape(budget).unwrap_err();
                        budget.reserve_storage(floor)?;
                        denied
                    }
                    _ => unreachable!(),
                };
                let check = |error| match (fault, error) {
                    (
                        0,
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                            bound,
                        )),
                    ) => {
                        assert_eq!((bound.actual(), bound.limit()), (LIMIT + 1, LIMIT));
                    }
                    (
                        1 | 2,
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting),
                    ) => {}
                    (_, other) => panic!("unexpected witness query refusal: {other:?}"),
                };
                check(first);
                let before = (budget.work(), budget.storage());
                check(endpoint.carrier_shape(budget).unwrap_err());
                assert_eq!((budget.work(), budget.storage()), before);
                held.set(budget.storage());
                reached.set(true);
                Ok(())
            });
        assert!(reached.get());
        match (fault, result) {
            (
                0,
                Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(bound))),
            ) => {
                assert_eq!((bound.actual(), bound.limit()), (LIMIT + 1, LIMIT));
                assert_eq!(remaining, 0);
            }
            (
                1 | 2,
                Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)),
            ) => {
                assert_eq!(remaining, held.get());
            }
            (_, other) => panic!("unexpected outer witness refusal: {other:?}"),
        }
    }
}

#[test]
fn direct_witness_endpoints_have_exact_complete_work_and_peak_storage_boundaries() {
    let run = |work, storage| index_relation_probe_v35(work, storage, witness_rows_v50);
    let measured = run(1_000_000_000, 256 << 20);
    measured.0.unwrap();
    let exact = run(measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, measured.2, 0));
    for short_work in [true, false] {
        let work = measured.1 - usize::from(short_work);
        let storage = measured.2 - usize::from(!short_work);
        let (result, _, _, remaining) = run(work, storage);
        let denied = result.unwrap_err();
        match crate::production_semantic_kir_v1::aggregate_source_owned_resource_v30(&denied) {
            Some(ArgumentResourceV1::Work(bound)) if short_work => {
                assert_eq!((bound.actual(), bound.limit()), (measured.1, work));
            }
            Some(ArgumentResourceV1::Storage(bound)) if !short_work => {
                assert_eq!((bound.actual(), bound.limit()), (measured.2, storage));
            }
            other => panic!("unexpected one-short witness refusal: {other:?}"),
        }
        assert_eq!(remaining, 0);
    }
}

#[test]
fn direct_witness_retained_header_preserves_the_full_emitter_binding_metadata() {
    type Fields = (
        ValueId,
        SemanticDisjointIndexSpaceV1,
        bool,
        Option<SemanticCapabilityAvailabilityV1>,
    );
    assert_eq!(size_of::<Fields>(), size_of::<SourceSsaWitnessV50>());
    assert_eq!(align_of::<Fields>(), align_of::<SourceSsaWitnessV50>());
}
