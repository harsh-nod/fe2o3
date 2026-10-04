use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1 as Coordinates;
use fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1;
use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;

fn site(statement: u32) -> Site {
    Site::Statement {
        block: SsaBlockIdV1::new(0),
        statement,
    }
}

fn initializer(components: u64) -> ProductionSourceOutputPrivateArrayInitializerV1 {
    ProductionSourceOutputPrivateArrayInitializerV1::Checked {
        components,
        retained_executable: components,
        retained_nonexecutable: 0,
        omitted_unreachable: 0,
    }
}

// These are genuine semantic-source fixtures, not rustc collector tests. The
// existing AMD binder supplies B; its independent checker supplies N/B custody.
pub(super) fn with_policy3_candidate(
    source: ProductionPreRankedKirOwnerV1,
    profile: Profile,
    next: impl FnOnce(
        &ProductionPreRankedKirOwnerV1,
        &CheckedNeutralKernelIrOwnerPolicy3V1,
        &Coordinates<'_, '_>,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let target =
        dialect_amdgcn::bind_production_target_v1(source.executable().module(), profile).unwrap();
    assert_eq!(target.profile(), profile);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let source_storage = retained(&source);
    budget.reserve_storage(source_storage).unwrap();
    let (bound, bound_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            target.module(),
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(bound_storage.retained_storage())
        .unwrap();
    drop(target);
    assert!(!std::ptr::eq(source.executable(), &bound));
    let checked = optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut budget).unwrap();
    assert_eq!(checked.report().passes().len(), 8);
    assert!(!checked.grants_authority());
    assert_eq!(
        checked.native_input_audit_bytes(),
        bound.canonical().canonical_bytes()
    );
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    let (coordinates, coordinate_storage) =
        dialect_amdgcn::check_production_target_coordinate_preservation_v1(
            source.executable(),
            &bound,
            profile,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(coordinate_storage.retained_storage())
        .unwrap();
    assert!(std::ptr::eq(coordinates.input(), source.executable()));
    assert!(std::ptr::eq(coordinates.output(), &bound));
    let live = budget.storage();
    next(&source, &checked, &coordinates, &mut budget);
    assert_eq!(budget.storage(), live);
    #[allow(
        clippy::drop_non_drop,
        reason = "End borrowed coordinate custody before release"
    )]
    drop(coordinates);
    budget
        .release_storage(coordinate_storage.retained_storage())
        .unwrap();
    let checked_storage = checked.storage().retained_storage();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(bound);
    budget
        .release_storage(bound_storage.retained_storage())
        .unwrap();
    drop(source);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn with_policy3_view(
    source: ProductionPreRankedKirOwnerV1,
    profile: Profile,
    next: impl FnOnce(
        &ProductionSourceOutputOccurrencesV1<'_, '_>,
        &CheckedNeutralKernelIrOwnerPolicy3V1,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    with_policy3_candidate(source, profile, |source, checked, coordinates, budget| {
        let incoming = budget.storage();
        let (view, storage) =
            derive_source_output_occurrences_policy3_v1(source, coordinates, checked, budget)
                .unwrap();
        assert_eq!(budget.storage(), incoming);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(std::ptr::eq(view.source(), source));
        assert!(std::ptr::eq(view.bound(), coordinates.output()));
        assert!(std::ptr::eq(view.output(), checked.owner()));
        assert!(!view.grants_authority());
        assert_eq!(view.storage, storage);
        let live = budget.storage();
        next(&view, checked, budget);
        assert_eq!(budget.storage(), live);
        drop(view);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), incoming);
    });
}

#[test]
fn policy3_actual_target_output_preserves_source_blocks_and_initializer_components() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for repetitions in [1, 2] {
            let values = [11, 29, 31, 37, 41, 43, 47, u32::MAX];
            with_policy3_view(
                array_owner(ArrayCase::Initializer {
                    values,
                    repetitions,
                    float: false,
                }),
                profile,
                |view, _, budget| {
                    assert_eq!(view.private_arrays.len(), repetitions * 8 + 1);
                    let block = view
                        .block(
                            ARRAY_ROOT,
                            ARRAY_ROOT,
                            SemanticBlockIdV1::from_index(0),
                            budget,
                        )
                        .unwrap();
                    assert!(
                        matches!(block, ProductionSourceOutputBlockV1::Materialized {
                        original, placement: Some(_), executable: true,
                    } if original.function.0 == 0 && original.block == 0)
                    );
                    for statement in 1..=repetitions {
                        let result = view
                            .private_array_initializer(
                                ARRAY_ROOT,
                                ARRAY_ROOT,
                                site(statement as u32),
                                budget,
                            )
                            .unwrap();
                        assert_eq!(result, initializer(8));
                        assert!(!result.grants_authority());
                        let mut components = 0;
                        for row in view
                            .private_arrays
                            .iter()
                            .filter(|row| row.key[3] == statement as u32)
                        {
                            assert_eq!(row.key[6] as usize, components);
                            let SourceOutputArrayPlacementV1::Retained(anchors) = row.placement
                            else {
                                panic!("actual policy-3 initializer Store must remain retained");
                            };
                            let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                                operation,
                                result: 0,
                            } = anchors.uses[4].definition
                            else {
                                panic!(
                                    "literal initializer retains its actual constant definition"
                                );
                            };
                            let value =
                                source_output_operation_v1(view.output(), operation, budget)
                                    .unwrap();
                            assert!(
                                matches!(value.kind, OperationKind::Constant(Constant::U32(bits))
                                if bits == values[components])
                            );
                            let store =
                                source_output_operation_v1(view.output(), anchors.memory, budget)
                                    .unwrap();
                            assert!(
                                matches!(store.kind, OperationKind::Store { value: actual, .. }
                                if actual == value.results[0].id)
                            );
                            components += 1;
                        }
                        assert_eq!(components, 8);
                    }
                    assert!(matches!(
                        view.private_array_write(
                            ARRAY_ROOT,
                            ARRAY_ROOT,
                            site(repetitions as u32 + 1),
                            Role::Destination,
                            budget,
                        )
                        .unwrap(),
                        ProductionSourceOutputPrivateArrayAccessV1::Retained {
                            index: 0,
                            executable: true,
                            ..
                        }
                    ));
                },
            );
        }
    }
}

#[test]
fn policy3_foreign_equal_byte_source_and_wrong_bound_history_keep_exact_refusals() {
    let case = ArrayCase::Initializer {
        values: [11; 8],
        repetitions: 1,
        float: false,
    };
    with_policy3_candidate(
        array_owner(case),
        Profile::Gfx942,
        |source, checked, coordinates, budget| {
            let incoming = budget.storage();
            let foreign = array_owner(case);
            let foreign_storage = retained(&foreign);
            budget.reserve_storage(foreign_storage).unwrap();
            assert_eq!(
                source.executable().canonical().canonical_bytes(),
                foreign.executable().canonical().canonical_bytes()
            );
            assert!(!std::ptr::eq(source.executable(), foreign.executable()));
            let floor = budget.storage();
            assert!(matches!(
                derive_source_output_occurrences_policy3_v1(&foreign, coordinates, checked, budget,),
                Err(ProductionSourceOutputErrorV1::InputCustody)
            ));
            assert_eq!(budget.storage(), floor);
            let wrong_history =
                optimize_checked_canonical_kernel_ir_policy3_v1(source.executable(), budget)
                    .unwrap();
            let wrong_storage = wrong_history.storage().retained_storage();
            budget.reserve_storage(wrong_storage).unwrap();
            assert_ne!(
                coordinates.output().canonical().canonical_bytes(),
                wrong_history.native_input_audit_bytes()
            );
            let floor = budget.storage();
            assert!(matches!(
                derive_source_output_occurrences_policy3_v1(
                    source,
                    coordinates,
                    &wrong_history,
                    budget,
                ),
                Err(ProductionSourceOutputErrorV1::InputCustody)
            ));
            assert_eq!(budget.storage(), floor);
            drop(wrong_history);
            budget.release_storage(wrong_storage).unwrap();
            drop(foreign);
            budget.release_storage(foreign_storage).unwrap();
            assert_eq!(budget.storage(), incoming);
            let (view, storage) =
                derive_source_output_occurrences_policy3_v1(source, coordinates, checked, budget)
                    .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert!(std::ptr::eq(view.output(), checked.owner()));
            drop(view);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), incoming);
        },
    );
}

#[test]
fn policy3_unit_local_later_root_refusal_has_exact_history_prefix_and_cleanup() {
    use super::local_helper_source_v1_tests::{UnitCase, unit_owner};
    let source = unit_owner(UnitCase::Initializer, &[0, 2]);
    assert_eq!(
        source.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::UnitLocal
    );
    assert_eq!(source.empty_effect_helpers().iter().count(), 0);
    with_policy3_candidate(
        source,
        Profile::Gfx942,
        |source, checked, coordinates, live_budget| {
            let floor = live_budget.storage();
            // Entry5 + checked length sum2 + complete B/history byte comparison.
            // No source replay or indexed inventory work is charged by this prefix.
            let exact = 7
                + coordinates.output().canonical().canonical_bytes().len()
                + checked.native_input_audit_bytes().len();
            for limit in [exact - 1, exact] {
                // Isolated component ledger over the genuine prepared owners, not a
                // replacement phase ledger in the production constructor.
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor + 3);
                budget.reserve_storage(floor + 3).unwrap();
                budget.release_storage(3).unwrap();
                let result = derive_source_output_occurrences_policy3_v1(
                    source,
                    coordinates,
                    checked,
                    &mut budget,
                );
                if limit == exact {
                    assert!(matches!(
                        result,
                        Err(ProductionSourceOutputErrorV1::SourceReplay(
                            ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                                consumer: "source/output replay"
                            }
                        ))
                    ));
                    assert_eq!(budget.work(), exact);
                } else {
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Work(error)))
                    if error.actual() == exact && error.limit() == limit)
                    );
                    assert_eq!(budget.work(), 7);
                }
                assert_eq!(
                    (budget.storage(), budget.peak_storage()),
                    (floor, floor + 3)
                );
                assert_eq!(
                    work.failed_work(),
                    if limit < exact { Some(exact) } else { None }
                );
            }
            assert!(matches!(
                derive_source_output_occurrences_policy3_v1(
                    source,
                    coordinates,
                    checked,
                    live_budget
                ),
                Err(ProductionSourceOutputErrorV1::SourceReplay(
                    ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                        consumer: "source/output replay"
                    }
                ))
            ));
            assert_eq!(live_budget.storage(), floor);
        },
    );
}

#[test]
fn policy3_constructor_entry_work_and_missing_live_storage_fail_before_allocation() {
    with_policy3_candidate(
        array_owner(ArrayCase::Write { sparse: false }),
        Profile::Gfx942,
        |source, checked, coordinates, live_budget| {
            let minimum = retained(source) + checked.storage().retained_storage();
            for (limit, reserved) in [(4, minimum), (5, minimum - 1), (5, minimum)] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, reserved);
                budget.reserve_storage(reserved).unwrap();
                let result = derive_source_output_occurrences_policy3_v1(
                    source,
                    coordinates,
                    checked,
                    &mut budget,
                );
                if reserved < minimum {
                    assert!(matches!(
                        result,
                        Err(ProductionSourceOutputErrorV1::Resource(
                            AssertOriginResourceV1::Accounting
                        ))
                    ));
                    assert_eq!(budget.work(), 5);
                } else {
                    let attempted = if limit == 4 { 5 } else { 7 };
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Work(error)))
                        if error.actual() == attempted && error.limit() == limit)
                    );
                    assert_eq!(budget.work(), if limit == 4 { 0 } else { 5 });
                }
                assert_eq!(
                    (budget.storage(), budget.peak_storage()),
                    (reserved, reserved)
                );
            }
            let floor = live_budget.storage();
            let (view, storage) = derive_source_output_occurrences_policy3_v1(
                source,
                coordinates,
                checked,
                live_budget,
            )
            .unwrap();
            live_budget
                .reserve_storage(storage.retained_storage())
                .unwrap();
            drop(view);
            live_budget
                .release_storage(storage.retained_storage())
                .unwrap();
            assert_eq!(live_budget.storage(), floor);
        },
    );
}

#[test]
fn policy3_indexed_write_query_preserves_literal_work_header_and_floor_boundaries() {
    with_policy3_view(
        array_owner(ArrayCase::Write { sparse: false }),
        Profile::Gfx950,
        |view, checked, live_budget| {
            // Actual enum-bearing view header, one source block/array row and
            // two empty catalog encodings (8+2+2+4+32+4+4 bytes each).
            let expected_storage =
                std::mem::size_of::<ProductionSourceOutputOccurrencesV1<'_, '_>>()
                    + std::mem::size_of::<SourceOutputBlockRowV1>()
                    + std::mem::size_of::<SourceOutputArrayRowV1>()
                    + 2 * 56;
            assert_eq!(view.storage.retained_storage(), expected_storage);
            // Unchanged source280 + floor4 + row29 + ancestry15 + index20
            // + three result lookups18 + physical45. Policy routing adds zero.
            const EXACT: usize = 280 + 4 + 29 + 15 + 20 + 18 + 45;
            let floor = live_budget.storage();
            for limit in [EXACT - 1, EXACT] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor + 3);
                budget.reserve_storage(floor + 3).unwrap();
                budget.release_storage(3).unwrap();
                let result = view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(1),
                    Role::Destination,
                    &mut budget,
                );
                if limit == EXACT {
                    assert!(matches!(
                        result,
                        Ok(ProductionSourceOutputPrivateArrayAccessV1::Retained {
                            index: 0,
                            executable: true,
                            ..
                        })
                    ));
                } else {
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::PrivateArray(
                        SemanticKirPrivateArrayQueryErrorV1::Resource(AssertOriginResourceV1::Work(error))
                    )) if error.actual() == EXACT && error.limit() == limit)
                    );
                }
                assert_eq!(budget.work(), limit);
                assert_eq!(
                    (budget.storage(), budget.peak_storage()),
                    (floor, floor + 3)
                );
            }
            let minimum = retained(view.source())
                + checked.storage().retained_storage()
                + view.storage.retained_storage();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
            let mut budget = AssertOriginBudgetV1::new(&mut work, minimum - 1);
            budget.reserve_storage(minimum - 1).unwrap();
            assert!(matches!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(1),
                    Role::Destination,
                    &mut budget
                ),
                Err(ProductionSourceOutputErrorV1::Resource(
                    AssertOriginResourceV1::Accounting
                ))
            ));
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (4, minimum - 1, minimum - 1)
            );
            assert!(matches!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(1),
                    Role::Destination,
                    live_budget
                )
                .unwrap(),
                ProductionSourceOutputPrivateArrayAccessV1::Retained { index: 0, .. }
            ));
            assert_eq!(live_budget.storage(), floor);
        },
    );
}

#[test]
fn policy3_does_not_reclassify_a_retained_array_read_as_a_write() {
    with_policy3_view(
        array_owner(ArrayCase::RetainedValueRead),
        Profile::Gfx942,
        |view, _, budget| {
            assert_eq!(view.private_arrays.len(), 10);
            assert_eq!(
                view.private_array_initializer(ARRAY_ROOT, ARRAY_ROOT, site(1), budget)
                    .unwrap(),
                initializer(8)
            );
            assert!(matches!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(2),
                    Role::Destination,
                    budget
                )
                .unwrap(),
                ProductionSourceOutputPrivateArrayAccessV1::Retained { index: 0, .. }
            ));
            assert!(matches!(
                view.private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(3),
                    Role::RvalueOperand(0),
                    budget
                ),
                Err(ProductionSourceOutputErrorV1::PrivateArray(
                    SemanticKirPrivateArrayQueryErrorV1::Incomplete(
                        "checked output currently requires an ordinary private-array write"
                    )
                ))
            ));
        },
    );
}

#[test]
fn policy3_inventory_storage_denials_restore_partially_reserved_constructor_floor() {
    use fe2o3_kernel_analysis::{
        CanonicalKirBlockRefV1, CanonicalKirFunctionRefV1, CanonicalKirInventoryErrorV1,
        CanonicalKirInventoryV1,
    };
    with_policy3_candidate(
        array_owner(ArrayCase::Write { sparse: false }),
        Profile::Gfx942,
        |source, checked, coordinates, live_budget| {
            let floor = live_budget.storage();
            // Inventory build performs its census, then reserves its header,
            // functions Vec and blocks Vec in that order. This fixture has one
            // actual function/block; all byte thresholds follow those types.
            assert_eq!(coordinates.output().module().functions.len(), 1);
            assert_eq!(
                coordinates.output().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .len(),
                1
            );
            let header = std::mem::size_of::<CanonicalKirInventoryV1<'_>>();
            let functions = std::mem::size_of::<CanonicalKirFunctionRefV1<'_>>();
            let blocks = std::mem::size_of::<CanonicalKirBlockRefV1<'_>>();
            for (available, peak_extra, attempted_extra) in [
                (header - 1, 0, header),
                (header, header, header + functions),
                (
                    header + functions,
                    header + functions,
                    header + functions + blocks,
                ),
            ] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor + available);
                budget.reserve_storage(floor).unwrap();
                let result = derive_source_output_occurrences_policy3_v1(
                    source,
                    coordinates,
                    checked,
                    &mut budget,
                );
                assert!(
                    matches!(result, Err(ProductionSourceOutputErrorV1::Inventory(
                    CanonicalKirInventoryErrorV1::Resource(AssertOriginResourceV1::Storage(error))
                )) if error.actual() == floor + attempted_extra && error.limit() == floor + available)
                );
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.peak_storage(), floor + peak_extra);
                assert_eq!(budget.failed_storage(), Some(floor + attempted_extra));
                assert_eq!(work.failed_work(), None);
            }
            let (view, storage) = derive_source_output_occurrences_policy3_v1(
                source,
                coordinates,
                checked,
                live_budget,
            )
            .unwrap();
            live_budget
                .reserve_storage(storage.retained_storage())
                .unwrap();
            assert!(std::ptr::eq(view.output(), checked.owner()));
            drop(view);
            live_budget
                .release_storage(storage.retained_storage())
                .unwrap();
            assert_eq!(live_budget.storage(), floor);
        },
    );
}
