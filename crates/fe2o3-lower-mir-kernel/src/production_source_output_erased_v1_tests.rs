// Registered below erased owner tests to share the actual semantic fixture.
use super::*;

struct OccurrenceFixture {
    source: ProductionUnitLocalErasedSourceOwnerV1,
    bound: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    floor: usize,
}

fn occurrence_fixture(expected: bool, roots: usize) -> OccurrenceFixture {
    let (original, roots) = erased_effect_fixture(expected, roots);
    let input = erased_input_floor(&original, &roots);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR + input).unwrap();
    let (source, storage) =
        ProductionUnitLocalErasedSourceOwnerV1::try_produce_v1(original, roots, &mut budget)
            .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let binding = dialect_amdgcn::bind_production_target_v1(
        source.erased().module(),
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    let (bound, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        binding.module(), &mut budget,
    ).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    drop(binding);
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut budget)
            .unwrap();
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    let floor = budget.storage();
    OccurrenceFixture {
        source,
        bound,
        checked,
        floor,
    }
}

fn occurrence_context<R>(
    fixture: &OccurrenceFixture,
    next: impl FnOnce(
        &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
        &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
        &CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
        &CanonicalKirInventoryV1<'_>,
        usize,
    ) -> R,
) -> R {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(fixture.floor).unwrap();
    let (coordinates, storage) =
        fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1(
            fixture.source.erased(),
            &fixture.bound,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (input, storage) = CanonicalKirInventoryV1::derive(&fixture.bound, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (output, storage) =
        CanonicalKirInventoryV1::derive(fixture.checked.owner(), &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (erased, storage) =
        CanonicalKirInventoryV1::derive(fixture.source.erased(), &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (transition, storage) = fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
        &input,
        &output,
        fixture.checked.occurrences().candidate(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (control, storage) =
        CheckedCanonicalKirControlIndexV1::derive(&transition, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    next(
        &coordinates,
        &transition,
        &control,
        &erased,
        budget.storage(),
    )
}

#[test]
fn erased_occurrences_map_every_retained_and_deleted_physical_component() {
    for expected in [false, true] {
        for roots in [1, 2] {
            let fixture = occurrence_fixture(expected, roots);
            occurrence_context(
                &fixture,
                |coordinates, transition, control, erased, floor| {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                    budget.reserve_storage(floor).unwrap();
                    with_erased_source_output_occurrences_v1(
                        &fixture.source,
                        coordinates,
                        &fixture.checked,
                        transition,
                        control,
                        &mut budget,
                        |view, budget| {
                            assert!(std::ptr::eq(view.bound(budget)?, &fixture.bound));
                            assert!(std::ptr::eq(view.checked_output(budget)?, &fixture.checked));
                            let map = view.coordinates();
                            let original = map.inventory();
                            let mut deleted_calls = 0;
                            let mut deleted_helpers = 0;
                            let mut shifted_functions = 0;
                            for function in original.functions() {
                                let before = budget.work();
                                let disposition =
                                    map.canonical_function(function.coordinate, budget)?;
                                assert_eq!(budget.work() - before, 15);
                                match disposition {
                                    ErasedSourceOccurrenceV1::Retained(mapped) => {
                                        let actual = &erased.functions()[mapped.0 as usize];
                                        assert_eq!(function.function.id, actual.function.id);
                                        shifted_functions +=
                                            usize::from(mapped != function.coordinate);
                                    }
                                    ErasedSourceOccurrenceV1::DeletedLocalHelper => {
                                        deleted_helpers += 1
                                    }
                                    _ => panic!("a function is not a removed call"),
                                }
                            }
                            for operation in original.operations() {
                                let before = budget.work();
                                let disposition = map.operation(operation.coordinate, budget)?;
                                assert_eq!(budget.work() - before, 30);
                                match disposition {
                                    ErasedSourceOccurrenceV1::Retained(mapped) => {
                                        let actual = erased
                                            .operations()
                                            .iter()
                                            .find(|row| row.coordinate == mapped)
                                            .unwrap();
                                        assert_eq!(actual.operation, operation.operation);
                                    }
                                    ErasedSourceOccurrenceV1::DeletedUnitCall => {
                                        assert!(matches!(
                                            operation.operation.kind,
                                            OperationKind::Call { .. }
                                        ));
                                        assert!(
                                            operation.results.is_empty()
                                                && operation.operands.is_empty()
                                        );
                                        deleted_calls += 1;
                                    }
                                    ErasedSourceOccurrenceV1::DeletedLocalHelper => {}
                                }
                            }
                            for definition in original.definitions() {
                                let before = budget.work();
                                let disposition = map.definition(definition.coordinate, budget)?;
                                let expected_work = match definition.coordinate {
                                    ErasedDefinitionV1::FunctionArgument { .. } => 29,
                                    ErasedDefinitionV1::BlockArgument { .. } => 47,
                                    ErasedDefinitionV1::Result { .. } => 52,
                                };
                                assert_eq!(budget.work() - before, expected_work);
                                match disposition {
                                    ErasedSourceOccurrenceV1::Retained(mapped) => {
                                        let actual = erased
                                            .definitions()
                                            .iter()
                                            .find(|row| row.coordinate == mapped)
                                            .unwrap();
                                        assert_eq!(actual.value, definition.value);
                                        assert_eq!(actual.ty, definition.ty);
                                    }
                                    ErasedSourceOccurrenceV1::DeletedLocalHelper => {}
                                    _ => panic!("deleted Unit call cannot define a value"),
                                }
                            }
                            for operand in original.uses() {
                                let before = budget.work();
                                let disposition = map.operand(operand.coordinate, budget)?;
                                let expected_work = match operand.coordinate {
                                    ErasedOperandV1::OperationOperand { .. } => 52,
                                    ErasedOperandV1::TerminatorOperand { .. } => 47,
                                };
                                assert_eq!(budget.work() - before, expected_work);
                                match disposition {
                                    ErasedSourceOccurrenceV1::Retained(mapped) => {
                                        let actual = erased
                                            .uses()
                                            .iter()
                                            .find(|row| row.coordinate == mapped)
                                            .unwrap();
                                        assert_eq!(actual.value, operand.value);
                                    }
                                    ErasedSourceOccurrenceV1::DeletedLocalHelper => {}
                                    _ => {
                                        panic!("deleted Unit call cannot consume a physical value")
                                    }
                                }
                            }
                            for edge in original.edges() {
                                let before = budget.work();
                                let disposition = map.edge(edge.coordinate, budget)?;
                                assert_eq!(budget.work() - before, 47);
                                if let ErasedSourceOccurrenceV1::Retained(mapped) = disposition {
                                    let actual = erased
                                        .edges()
                                        .iter()
                                        .find(|row| row.coordinate == mapped)
                                        .unwrap();
                                    assert_eq!(actual.arguments, edge.arguments);
                                    assert_eq!(actual.target_id, edge.target_id);
                                    assert_eq!(
                                        actual.coordinate.successor,
                                        edge.coordinate.successor
                                    );
                                }
                            }
                            for argument in original.edge_arguments() {
                                let before = budget.work();
                                let disposition = map.edge_argument(argument.coordinate, budget)?;
                                assert_eq!(budget.work() - before, 69);
                                if let ErasedSourceOccurrenceV1::Retained(mapped) = disposition {
                                    let actual = erased
                                        .edge_arguments()
                                        .iter()
                                        .find(|row| row.coordinate == mapped)
                                        .unwrap();
                                    assert_eq!(actual.value, argument.value);
                                }
                            }
                            for effect in original.effects() {
                                let before = budget.work();
                                let disposition = map.access(effect.coordinate, budget)?;
                                assert_eq!(budget.work() - before, 52);
                                if let ErasedSourceOccurrenceV1::Retained(mapped) = disposition {
                                    assert!(
                                        erased.effects().iter().any(|row| row.coordinate == mapped)
                                    );
                                }
                            }
                            assert_eq!(deleted_calls, roots);
                            assert_eq!(deleted_helpers, 1);
                            if roots == 2 {
                                assert!(shifted_functions > 0);
                            }
                            Ok(())
                        },
                    )
                    .unwrap();
                    assert_eq!(budget.storage(), floor);
                },
            );
        }
    }
}

#[test]
fn erased_occurrences_preserve_root_assertions_and_distinguish_deleted_helper_assertions() {
    for expected in [false, true] {
        let fixture = occurrence_fixture(expected, 2);
        occurrence_context(&fixture, |coordinates, transition, control, _, floor| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            with_erased_source_output_occurrences_v1(
                &fixture.source,
                coordinates,
                &fixture.checked,
                transition,
                control,
                &mut budget,
                |view, budget| {
                    let original = fixture.source.original_source();
                    let origins = original.assert_origins();
                    let mut retained_assertions = 0;
                    let mut deleted_assertions = 0;
                    for alias in &origins.origins.aliases {
                        let site = alias.site;
                        let old = origins.origins.bindings[alias.binding];
                        assert_eq!(old.expected(), expected);
                        match view.assertion(
                            site.correspondence_owner,
                            site.semantic_function,
                            site.semantic_block,
                            budget,
                        )? {
                            ErasedSourceAssertionV1::Retained(current) => {
                                let reversed = SemanticKirAssertConditionBindingV1 {
                                    expected: !old.expected(),
                                    ..old
                                };
                                assert!(
                                    erased_rebase_assertion_v1(
                                        view.coordinates(),
                                        reversed,
                                        budget
                                    )
                                    .is_err()
                                );
                                assert_eq!(current.expected(), expected);
                                assert_eq!(current.semantic_success(), old.semantic_success());
                                assert_eq!(
                                    current.import_binding(),
                                    erased_rebase_assertion_v1(view.coordinates(), old, budget)?
                                        .unwrap()
                                );
                                retained_assertions += 1;
                            }
                            ErasedSourceAssertionV1::DeletedLocalHelper { original: removed } => {
                                assert_eq!(removed, old);
                                assert!(
                                    view.assertions(budget)?
                                        .assert_condition(
                                            site.correspondence_owner,
                                            site.semantic_function,
                                            site.semantic_block,
                                            budget,
                                        )
                                        .is_err()
                                );
                                deleted_assertions += 1;
                            }
                        }
                    }
                    assert_eq!(retained_assertions, 2);
                    assert!(deleted_assertions >= 2);
                    let mut deleted_blocks = 0;
                    for row in &original.correspondence.blocks {
                        match view.block(
                            row.correspondence_owner,
                            row.semantic_function,
                            row.semantic_block,
                            budget,
                        )? {
                            ErasedSourceBlockV1::DeletedLocalHelper { original } => {
                                assert_eq!(
                                    view.coordinates().block(original, budget)?,
                                    ErasedSourceOccurrenceV1::DeletedLocalHelper
                                );
                                deleted_blocks += 1;
                            }
                            ErasedSourceBlockV1::Retained {
                                original,
                                erased,
                                checked,
                            } => {
                                assert_eq!(
                                    view.coordinates().block(original, budget)?.retained()?,
                                    erased
                                );
                                assert_eq!(checked, control.block(erased, budget).unwrap());
                            }
                            _ => panic!("materialized row must not become NotMaterialized"),
                        }
                    }
                    assert!(deleted_blocks > 0);
                    assert!(
                        view.input_pipeline_catalog(budget)?
                            .definitions()
                            .is_empty()
                    );
                    assert!(view.output_pipeline_catalog(budget)?.bindings().is_empty());
                    let invalid = ErasedBlockV1 {
                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(u32::MAX),
                        block: 0,
                    };
                    assert!(view.coordinates().block(invalid, budget).is_err());
                    assert!(
                        view.coordinates()
                            .operation(
                                ErasedOperationV1 {
                                    block: invalid,
                                    operation: 0
                                },
                                budget
                            )
                            .is_err()
                    );
                    assert!(
                        view.coordinates()
                            .function(
                                SemanticFunctionIdV1::from_index(u32::MAX),
                                SemanticFunctionIdV1::from_index(0),
                                budget,
                            )
                            .is_err()
                    );
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn erased_occurrences_refuse_equal_bytes_under_foreign_endpoint_owners() {
    let fixture = occurrence_fixture(true, 1);
    let foreign = occurrence_fixture(true, 1);
    assert_eq!(
        fixture.source.erased().canonical().canonical_bytes(),
        foreign.source.erased().canonical().canonical_bytes()
    );
    occurrence_context(&fixture, |coordinates, transition, control, _, floor| {
        occurrence_context(
            &foreign,
            |other_coordinates, other_transition, other_control, _, other_floor| {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                let floor = floor + other_floor;
                budget.reserve_storage(floor).unwrap();
                for mode in 0..3 {
                    let mut entered = false;
                    let (coordinates, checked, transition, control) = match mode {
                        0 => (other_coordinates, &fixture.checked, transition, control),
                        1 => (coordinates, &foreign.checked, transition, control),
                        _ => (
                            coordinates,
                            &fixture.checked,
                            other_transition,
                            other_control,
                        ),
                    };
                    let result = with_erased_source_output_occurrences_v1(
                        &fixture.source,
                        coordinates,
                        checked,
                        transition,
                        control,
                        &mut budget,
                        |_, _| {
                            entered = true;
                            Ok(())
                        },
                    );
                    assert!(matches!(
                        result,
                        Err(ProductionSourceOutputErrorV1::InputCustody)
                    ));
                    assert!(!entered);
                    assert_eq!(budget.storage(), floor);
                }
                // Equal, independently checked rows are not the actual immutable
                // checked owner's capture. Endpoint equality alone is insufficient.
                let candidate = fixture.checked.occurrences().candidate();
                assert!(!candidate.functions.is_empty());
                let functions = candidate.functions.to_vec();
                let row_storage = std::mem::size_of_val(&functions)
                    + functions.capacity() * std::mem::size_of_val(&functions[0]);
                budget.reserve_storage(row_storage).unwrap();
                let (detached, storage) = fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
                    control.input(),
                    control.output(),
                    fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1 {
                        functions: &functions,
                        ..candidate
                    },
                    &mut budget,
                )
                .unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let detached_floor = budget.storage();
                let mut entered = false;
                let result = with_erased_source_output_occurrences_v1(
                    &fixture.source,
                    coordinates,
                    &fixture.checked,
                    &detached,
                    control,
                    &mut budget,
                    |_, _| {
                        entered = true;
                        Ok(())
                    },
                );
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::InputCustody)
                ));
                assert!(!entered);
                assert_eq!(budget.storage(), detached_floor);
            },
        );
    });
}

#[test]
fn erased_occurrences_have_exact_limits_and_restore_partial_allocation_failures() {
    let fixture = occurrence_fixture(false, 1);
    occurrence_context(&fixture, |coordinates, transition, control, _, floor| {
        let run = |limit, storage| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage);
            budget.reserve_storage(floor).unwrap();
            let result = with_erased_source_output_occurrences_v1(
                &fixture.source,
                coordinates,
                &fixture.checked,
                transition,
                control,
                &mut budget,
                |_, _| Ok(()),
            );
            assert_eq!(budget.storage(), floor);
            let accepted_work = budget.work();
            let peak = budget.peak_storage();
            let failed_storage = budget.failed_storage();
            (
                result,
                accepted_work,
                peak,
                work.failed_work(),
                failed_storage,
            )
        };
        let (result, exact_work, peak, _, _) = run(WORK, STORAGE);
        result.unwrap();
        assert!(exact_work > 21 && peak > floor);
        assert!(run(exact_work, peak).0.is_ok());
        let (result, accepted, _, failed_work, failed_storage) = run(exact_work - 1, STORAGE);
        assert!(result.is_err());
        assert!(accepted < exact_work);
        assert_eq!(failed_work, Some(exact_work));
        assert_eq!(failed_storage, None);
        let (result, _, _, failed_work, failed_storage) = run(WORK, peak - 1);
        assert!(result.is_err());
        assert_eq!(failed_work, None);
        assert!(failed_storage.is_some());
        let (result, accepted, entry_peak, failed_work, _) = run(19, STORAGE);
        assert!(matches!(
            result,
            Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Work(_)
            ))
        ));
        assert_eq!(accepted, 0);
        assert_eq!(failed_work, Some(20));
        assert_eq!(entry_peak, floor);
        for extra in [0, 1, 24, 128, (peak - floor) / 2] {
            let (result, _, observed_peak, _, failure) = run(WORK, floor + extra);
            assert!(result.is_err());
            assert!(failure.is_some());
            assert!(observed_peak <= floor + extra);
        }
    });
}

#[test]
fn erased_occurrences_callback_error_panic_loss_and_surplus_do_not_escape_cleanup() {
    let fixture = occurrence_fixture(true, 1);
    occurrence_context(&fixture, |coordinates, transition, control, _, floor| {
        for mode in 0..5 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let mut entered = false;
            let result = with_erased_source_output_occurrences_v1(
                &fixture.source,
                coordinates,
                &fixture.checked,
                transition,
                control,
                &mut budget,
                |_, budget| {
                    entered = true;
                    match mode {
                        0 => Ok(()),
                        1 => Err(ProductionSourceOutputErrorV1::Invalid("callback sentinel")),
                        2 => panic!("erased occurrence callback unwind"),
                        3 => {
                            budget.release_storage(1).unwrap();
                            Ok(())
                        }
                        _ => {
                            budget.reserve_storage(1).unwrap();
                            Ok(())
                        }
                    }
                },
            );
            assert!(entered);
            match mode {
                0 => result.unwrap(),
                1 => assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Invalid("callback sentinel"))
                )),
                2 => assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Panicked)
                )),
                _ => assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Resource(
                        AssertOriginResourceV1::Accounting
                    ))
                )),
            }
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn erased_occurrences_same_slot_foreign_work_refuses_queries_and_no_getter_cleanup() {
    let fixture = occurrence_fixture(false, 1);
    occurrence_context(&fixture, |coordinates, transition, control, _, floor| {
        for query in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let mut callback_floor = 0;
            let result = with_erased_source_output_occurrences_v1(
                &fixture.source,
                coordinates,
                &fixture.checked,
                transition,
                control,
                &mut budget,
                |view, budget| {
                    callback_floor = budget.storage();
                    foreign.reserve_storage(callback_floor).unwrap();
                    std::mem::swap(budget, &mut foreign);
                    if query {
                        assert!(view.assertions(budget).is_err());
                        std::mem::swap(budget, &mut foreign);
                        Ok(())
                    } else {
                        Ok(())
                    }
                },
            );
            if query {
                result.unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(foreign.work(), 5);
            } else {
                assert!(result.is_err());
                assert_eq!(budget.storage(), callback_floor);
                assert_eq!(budget.work(), 0);
                assert_eq!(budget.peak_storage(), callback_floor);
                assert_eq!(budget.failed_storage(), None);
                std::mem::swap(&mut budget, &mut foreign);
                budget.release_storage(budget.storage() - floor).unwrap();
            }
            assert_eq!(foreign.storage(), callback_floor);
            assert_eq!(foreign.peak_storage(), callback_floor);
            assert_eq!(foreign.failed_storage(), None);
            foreign.release_storage(callback_floor).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    });
}
