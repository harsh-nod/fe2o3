use super::*;
use std::mem::size_of;

const FLOOR: usize = 43;

fn key(ordinal: usize) -> (usize, ValueId) {
    (ordinal / 4, ValueId((2 * (ordinal % 4) + 1) as u32))
}

// This index is an inert lookup fixture, not source-origin or memory authority.
fn index(ty: &Type, count: usize) -> SourceReferencePointerIndexV29<'_> {
    let mut definitions: Vec<_> = (0..count)
        .rev()
        .map(|ordinal| {
            (
                key(ordinal),
                (ty, SourceReferencePointerDefinitionV29::Parameter(ordinal)),
            )
        })
        .collect();
    definitions.sort_unstable_by_key(|row| row.0);
    SourceReferencePointerIndexV29 {
        definitions,
        functions: BTreeMap::new(),
        name_width: 0,
        calls: Vec::new(),
        seeds: BTreeMap::new(),
    }
}

fn retained_bytes(count: usize) -> usize {
    size_of::<Vec<(usize, ValueId)>>()
        + 2 * size_of::<Result<Vec<(usize, ValueId)>, ProductionSemanticKirErrorV1>>()
        + count * size_of::<(usize, ValueId)>()
}

fn construct(
    count: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize, bool) {
    let ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let index = index(&ty, count);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut completed = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let ordinals = source_reference_pointer_ordinals_v29(&index, budget)?;
        assert_eq!(ordinals.len(), count);
        assert_eq!(ordinals.capacity(), count);
        for (ordinal, actual) in ordinals.iter().enumerate() {
            assert_eq!(*actual, key(ordinal));
        }
        assert_eq!(budget.work(), count + 5);
        assert_eq!(budget.storage(), FLOOR + retained_bytes(count));
        drop(ordinals);
        completed = true;
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn pointer_ordinals_preserve_exact_sorted_coordinates_with_linear_prepaid_storage() {
    for count in [0, 1, 2, 31, 32, 63, 64, 127, 128, 255] {
        // Three allocation steps, N key copies, and two enclosing cleanup steps.
        let expected_work = count + 5;
        let expected_peak = FLOOR + retained_bytes(count);
        let (result, work, peak, completed) = construct(count, expected_work, expected_peak);
        result.unwrap();
        assert!(completed);
        assert_eq!((work, peak), (expected_work, expected_peak));
    }
}

#[test]
fn pointer_ordinal_construction_has_exact_one_short_work_and_storage_boundaries() {
    for count in [0, 1, 32, 64, 128] {
        let work = count + 5;
        let storage = FLOOR + retained_bytes(count);
        for short_work in [true, false] {
            let (result, _, _, completed) = construct(
                count,
                work - usize::from(short_work),
                storage - usize::from(!short_work),
            );
            assert!(!completed);
            let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) = result
            else {
                panic!("the independently derived one-short resource must refuse");
            };
            assert!(matches!(
                (short_work, error),
                (true, ArgumentResourceV1::Work(_)) | (false, ArgumentResourceV1::Storage(_))
            ));
        }
    }
}

fn lookup_work(count: usize) -> usize {
    // The shared lookup allowance is 16 visits per tree-depth level plus two.
    16 * (count.checked_ilog2().unwrap_or(0) as usize + 2)
}

#[test]
fn pointer_ordinal_lookup_is_exact_across_instances_gaps_and_boundary_keys() {
    for count in [0, 1, 2, 31, 32, 63, 64, 127, 128, 255] {
        let ty = Type::Scalar(ScalarType::U32);
        let index = index(&ty, count);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let ordinals = source_reference_pointer_ordinals_v29(&index, &mut budget).unwrap();
        for ordinal in 0..count {
            let before = (budget.work(), budget.storage());
            assert_eq!(
                source_reference_pointer_ordinal_v29(&ordinals, key(ordinal), &mut budget).unwrap(),
                ordinal
            );
            assert_eq!(
                (budget.work(), budget.storage()),
                (before.0 + lookup_work(count), before.1)
            );
        }
        for missing in [
            (0, ValueId(0)),
            (0, ValueId(2)),
            key(count),
            (usize::MAX, ValueId(u32::MAX)),
        ] {
            let before = (budget.work(), budget.storage());
            let error =
                source_reference_pointer_ordinal_v29(&ordinals, missing, &mut budget).unwrap_err();
            assert!(matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "execution call parameters differ from their source instance",
                }
            ));
            assert_eq!(
                (budget.work(), budget.storage()),
                (before.0 + lookup_work(count), before.1)
            );
        }
    }
    let ordinals = [
        (0, ValueId(u32::MAX)),
        (usize::MAX, ValueId(0)),
        (usize::MAX, ValueId(u32::MAX)),
    ];
    for (ordinal, &key) in ordinals.iter().enumerate() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(lookup_work(ordinals.len()));
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert_eq!(
            source_reference_pointer_ordinal_v29(&ordinals, key, &mut budget).unwrap(),
            ordinal
        );
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn pointer_ordinal_lookup_one_short_refuses_before_existing_or_missing_key_query() {
    for count in [0, 1, 32, 64, 128] {
        let ordinals: Vec<_> = (0..count).map(key).collect();
        for query in [key(0), (usize::MAX, ValueId(u32::MAX))] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(lookup_work(count) - 1);
            let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR);
            budget.reserve_storage(FLOOR).unwrap();
            assert!(matches!(
                source_reference_pointer_ordinal_v29(&ordinals, query, &mut budget),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

fn definition_storage(count: usize) -> usize {
    size_of::<Vec<SourceReferencePointerRowV29<'_>>>()
        + 2 * size_of::<Result<Vec<SourceReferencePointerRowV29<'_>>, ProductionSemanticKirErrorV1>>(
        )
        + count * size_of::<SourceReferencePointerRowV29<'_>>()
}

fn definition_sort_work(count: usize) -> usize {
    let coordinates = count * 2;
    let search = usize::BITS as usize - coordinates.leading_zeros() as usize + 1;
    // Two-coordinate comparisons, the shared four-visit sort allowance, then
    // a two-coordinate adjacent-duplicate scan and one exact census check.
    1 + coordinates * search * 4 + coordinates
}

fn construct_definitions(
    count: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize, bool) {
    let ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let block = BasicBlock::new(BlockId(7));
    let operation = Operation::new(
        vec![],
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(3),
            to: ty.clone(),
        },
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut completed = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        // These descriptors are inert lookup fixtures, not memory authority.
        let mut definitions = source_reference_emission_vec_v29(count, budget)?;
        let backing = definitions.as_ptr();
        assert_eq!(definitions.capacity(), count);
        for ordinal in (0..count).rev() {
            let definition = match ordinal % 3 {
                0 => SourceReferencePointerDefinitionV29::Parameter(ordinal),
                1 => SourceReferencePointerDefinitionV29::BlockParameter(&block, ordinal),
                _ => SourceReferencePointerDefinitionV29::Operation(&operation, ordinal),
            };
            let before = (budget.work(), budget.storage());
            source_reference_insert_pointer_definition_v29(
                &mut definitions,
                key(ordinal),
                (&ty, definition),
                budget,
            )?;
            assert_eq!((budget.work(), budget.storage()), (before.0 + 1, before.1));
            assert_eq!(definitions.as_ptr(), backing);
            assert_eq!(definitions.capacity(), count);
        }
        source_reference_finish_pointer_definitions_v29(&mut definitions, count, budget)?;
        assert_eq!(definitions.as_ptr(), backing);
        assert_eq!(definitions.capacity(), count);
        for (ordinal, &(actual_key, (actual_type, actual))) in definitions.iter().enumerate() {
            assert_eq!(actual_key, key(ordinal));
            assert!(std::ptr::eq(actual_type, &ty));
            match (ordinal % 3, actual) {
                (0, SourceReferencePointerDefinitionV29::Parameter(actual)) => {
                    assert_eq!(actual, ordinal)
                }
                (1, SourceReferencePointerDefinitionV29::BlockParameter(actual, result)) => {
                    assert!(std::ptr::eq(actual, &block));
                    assert_eq!(result, ordinal);
                }
                (2, SourceReferencePointerDefinitionV29::Operation(actual, result)) => {
                    assert!(std::ptr::eq(actual, &operation));
                    assert_eq!(result, ordinal);
                }
                _ => panic!("the exact borrowed definition kind and ordinal must survive"),
            }
        }
        assert_eq!(budget.work(), 5 + count + definition_sort_work(count));
        assert_eq!(budget.storage(), FLOOR + definition_storage(count));
        drop(definitions);
        completed = true;
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn fixed_pointer_definitions_preserve_borrowed_identity_with_linear_prepaid_storage() {
    for count in [0, 1, 2, 31, 32, 63, 64, 127, 128, 255] {
        let expected_work = 5 + count + definition_sort_work(count);
        let expected_storage = FLOOR + definition_storage(count);
        let (result, work, storage, completed) =
            construct_definitions(count, expected_work, expected_storage);
        result.unwrap();
        assert!(completed);
        assert_eq!((work, storage), (expected_work, expected_storage));
    }
}

#[test]
fn fixed_pointer_definition_prepayment_has_exact_one_short_boundaries_before_publication() {
    for count in [0, 1, 2, 31, 32, 63, 64, 127, 128] {
        let work = 5 + count + definition_sort_work(count);
        let storage = FLOOR + definition_storage(count);
        for short_work in [true, false] {
            let (result, _, _, completed) = construct_definitions(
                count,
                work - usize::from(short_work),
                storage - usize::from(!short_work),
            );
            assert!(!completed);
            assert!(matches!(
                (short_work, result),
                (
                    true,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ) | (
                    false,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                )
            ));
        }
    }
}

#[test]
fn fixed_pointer_definitions_refuse_growth_and_inexact_census_without_publication() {
    let ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    for count in [0, 1, 32, 64, 128] {
        let mut definitions = index(&ty, count).definitions;
        assert_eq!(definitions.len(), definitions.capacity());
        let backing = definitions.as_ptr();
        for allowance in [0, 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(allowance);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = source_reference_insert_pointer_definition_v29(
                &mut definitions,
                key(count),
                (&ty, SourceReferencePointerDefinitionV29::Parameter(count)),
                &mut budget,
            );
            assert!(matches!(
                (allowance, result),
                (
                    0,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ) | (
                    1,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                )
            ));
            assert_eq!(definitions.len(), count);
            assert_eq!(definitions.as_ptr(), backing);
            assert_eq!(budget.storage(), 0);
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert!(matches!(
            source_reference_finish_pointer_definitions_v29(
                &mut definitions,
                count + 1,
                &mut budget
            ),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(budget.work(), 1);
        assert_eq!(definitions.as_ptr(), backing);
    }
}

#[test]
fn pointer_definition_sort_prepays_before_mutation_and_rejects_duplicate_coordinates() {
    let ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let keys = [
        (0, ValueId(0)),
        (1, ValueId(0)),
        (0, ValueId(1)),
        (usize::MAX, ValueId(0)),
        (0, ValueId(u32::MAX)),
        (usize::MAX, ValueId(u32::MAX)),
    ];
    let entry = |ordinal| (&ty, SourceReferencePointerDefinitionV29::Parameter(ordinal));
    for short in [true, false] {
        let mut definitions: Vec<_> = keys
            .into_iter()
            .enumerate()
            .map(|(i, key)| (key, entry(i)))
            .collect();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(
            definition_sort_work(keys.len()) - usize::from(short),
        );
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = source_reference_finish_pointer_definitions_v29(
            &mut definitions,
            keys.len(),
            &mut budget,
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
            assert_eq!(
                definitions.iter().map(|row| row.0).collect::<Vec<_>>(),
                keys
            );
        } else {
            result.unwrap();
            for (ordinal, key) in keys.into_iter().enumerate() {
                let row = definitions.binary_search_by_key(&key, |row| row.0).unwrap();
                assert!(matches!(definitions[row].1.1,
                    SourceReferencePointerDefinitionV29::Parameter(actual) if actual == ordinal));
            }
        }
        assert_eq!(budget.storage(), 0);
    }
    for duplicate in keys {
        let mut definitions: Vec<_> = keys
            .into_iter()
            .enumerate()
            .map(|(i, key)| (key, entry(i)))
            .collect();
        definitions.push((duplicate, entry(keys.len())));
        let count = definitions.len();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(definition_sort_work(count));
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let error =
            source_reference_finish_pointer_definitions_v29(&mut definitions, count, &mut budget)
                .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "execution call parameters differ from their source instance",
            }
        ));
        assert_eq!(budget.work(), definition_sort_work(count));
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn pointer_definition_lookup_keeps_exact_borrowed_entries_and_prepaid_search_boundaries() {
    let ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    for count in [0, 1, 2, 31, 32, 63, 64, 127, 128, 255] {
        let definitions = index(&ty, count).definitions;
        for query in (0..count).map(key).chain([
            (0, ValueId(0)),
            (0, ValueId(2)),
            key(count),
            (usize::MAX, ValueId(u32::MAX)),
        ]) {
            for short in [true, false] {
                let mut work =
                    CanonicalKernelIrWorkBudgetV1::new(lookup_work(count) - usize::from(short));
                let mut budget = ArgumentBudgetV1::new(&mut work, 0);
                let result = source_reference_pointer_definition_lookup_v29(
                    &definitions,
                    query,
                    &mut budget,
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
                    let actual = result.unwrap();
                    let expected = definitions
                        .iter()
                        .find(|row| row.0 == query)
                        .map(|row| &row.1);
                    match (actual, expected) {
                        (Some(actual), Some(expected)) => {
                            assert!(std::ptr::eq(actual, expected));
                            assert!(std::ptr::eq(actual.0, &ty));
                        }
                        (None, None) => {}
                        _ => panic!("lookup must use both exact coordinates"),
                    }
                    assert_eq!(budget.work(), lookup_work(count));
                }
                assert_eq!(budget.storage(), 0);
            }
        }
    }
}
