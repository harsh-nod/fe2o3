// Child of the genuine erased Policy5 fixtures; no reconstructed original N.
use super::*;
type Final6 = crate::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1;
type Error6 = crate::ProductionCheckedOutputAdmissionErrorPolicy6V1;
struct Fixture6 {
    source: ProductionUnitLocalErasedSourceOwnerV1,
    bound: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1,
    floor: usize,
    bound_storage: usize,
}
fn continue6(input: Fixture5) -> Fixture6 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(input.floor).unwrap();
    let retained = input.checked.retained_storage();
    let expected_floor = input.floor - retained;
    let old = input.checked.owner().canonical().canonical_bytes().to_vec();
    let record = input.checked.execution().canonical_bytes().to_vec();
    let checked = fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
        &input.bound,
        input.checked,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), expected_floor);
    assert_eq!(
        checked
            .intermediate_policy5()
            .owner()
            .canonical()
            .canonical_bytes(),
        old
    );
    assert_eq!(
        checked.intermediate_policy5().execution().canonical_bytes(),
        record.as_slice()
    );
    budget.reserve_storage(checked.retained_storage()).unwrap();
    Fixture6 {
        source: input.source,
        bound: input.bound,
        checked,
        floor: budget.storage(),
        bound_storage: input.bound_storage,
    }
}
fn admit6(input: Fixture6, work: usize, storage: usize) -> (Result<Final6, Error6>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage);
    budget.reserve_storage(input.floor).unwrap();
    let result = Final6::try_admit_v1(input.source, input.bound, input.checked, &mut budget);
    assert_eq!(budget.storage(), input.floor);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn erased_policy6_retains_distinct_n_e_and_full_p5_history_before_fresh_i_checks() {
    for expected in [false, true] {
        for roots in [1, 2] {
            let input = continue6(fixture5(expected, roots, None));
            let original = input
                .source
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes()
                .to_vec();
            let erased = input.source.erased().canonical().canonical_bytes().to_vec();
            assert_ne!(original, erased);
            let final_i = input.checked.owner().canonical().canonical_bytes().to_vec();
            let retained =
                input.source.retained_storage_floor_v1() + input.checked.retained_storage();
            let floor = input.floor;
            assert_eq!(floor, FLOOR + retained + input.bound_storage);
            let owner = admit6(input, WORK, STORAGE).0.unwrap();
            assert_eq!(
                owner
                    .original_source()
                    .executable()
                    .canonical()
                    .canonical_bytes(),
                original
            );
            assert_eq!(owner.erased().canonical().canonical_bytes(), erased);
            assert_eq!(owner.output().canonical().canonical_bytes(), final_i);
            assert_eq!(
                owner
                    .checked_output()
                    .intermediate_policy5()
                    .load_forwarding_rows()
                    .len(),
                roots
            );
            assert_eq!(owner.retained_input_storage_floor_v1().unwrap(), retained);
            assert_eq!(owner.kernels().len(), roots);
            assert!(
                owner
                    .kernels()
                    .iter()
                    .all(|kernel| !kernel.accesses().is_empty())
            );
            assert!(!owner.grants_artifact_or_launch_authority());
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            owner.verify_equivalence(&mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn erased_policy6_does_not_skip_original_source_roster_effect_or_assertion_checks() {
    for mutation in 0..5 {
        let mut input = continue6(fixture5(true, 2, None));
        match mutation {
            0 => {
                input
                    .source
                    .original
                    .correspondence
                    .statement_operation_spans[0]
                    .operation_count += 1
            }
            1 => input.source.original.assert_origins.bindings[0].expected ^= true,
            2 => {
                input.source.operations[0] =
                    Some(ProductionUnitLocalOperationDeletionV1::DeletedUnitCall)
            }
            3 => {
                input.source.roots[0].access_sources[0] =
                    ProductionRankedAccessSourceV1::new(1, Some(99), 0, 0, 5)
            }
            4 => input.source.original.launch_roots[0].global_extents[0] += 1,
            _ => unreachable!(),
        }
        let result = admit6(input, WORK, STORAGE).0;
        assert!(
            matches!(
                &result,
                Err(Error6::Prefix(prefix)) if matches!(prefix.as_ref(),
                    Error5::Prefix(FinalError::Admission(AdmissionError::Source(_))))
            ),
            "original-source mutation {mutation}: {result:?}"
        );
    }
    for mutation in [
        change_root_private_value as fn(&mut Module),
        change_root_global_value,
    ] {
        let result = admit6(continue6(fixture5(true, 1, Some(mutation))), WORK, STORAGE).0;
        assert!(
            matches!(&result, Err(Error6::Prefix(prefix)) if matches!(prefix.as_ref(),
            Error5::Prefix(FinalError::Admission(AdmissionError::Coordinates(_))))),
            "fresh verified B substitution: {result:?}"
        );
    }
}

#[test]
fn failed_consuming_policy6_releases_only_the_moved_prefix_after_discard() {
    let input = fixture5(true, 1, None);
    let expected = input.floor - input.checked.retained_storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(input.floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
        &input.bound,
        input.checked,
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(
            fe2o3_kernel_opt::CanonicalPolicy6OptimizationErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(_)
            )
        )
    ));
    assert_eq!(budget.storage(), expected);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn erased_policy6_exact_final_admission_budgets_restore_the_retained_floor() {
    let input = continue6(fixture5(true, 1, None));
    let floor = input.floor;
    let (owner, work, peak) = admit6(input, WORK, STORAGE);
    drop(owner.unwrap());
    assert!(peak > floor);
    for (work, storage, expected) in [
        (work, peak, true),
        (work - 1, peak, false),
        (work, peak - 1, false),
    ] {
        let input = continue6(fixture5(true, 1, None));
        assert_eq!(input.floor, floor);
        assert_eq!(admit6(input, work, storage).0.is_ok(), expected);
    }
}

fn fixture6_with_live_integer_identity(expected: bool, roots: usize) -> Fixture6 {
    let FinalFixture {
        source,
        bound,
        checked,
        bound_storage,
        ..
    } = final_fixture_from(
        erased_effect_fixture_with_integer_identity(expected, roots),
        None,
    );
    drop(checked);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(FLOOR + source.retained_storage_floor_v1() + bound_storage)
        .unwrap();
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, &mut budget)
            .unwrap();
    budget.reserve_storage(checked.retained_storage()).unwrap();
    continue6(Fixture5 {
        source,
        bound,
        checked,
        floor: budget.storage(),
        bound_storage,
    })
}

#[test]
fn erased_policy6_genuine_integer_rewrite_retains_shifted_trap_origins_for_every_root() {
    use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    use fe2o3_kernel_ir::CanonicalKirOperationOriginV1;
    for expected in [false, true] {
        for roots in [1, 2] {
            let input = fixture6_with_live_integer_identity(expected, roots);
            assert_eq!(
                input
                    .checked
                    .intermediate_policy5()
                    .load_forwarding_rows()
                    .len(),
                roots
            );
            let original = input
                .source
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes()
                .to_vec();
            let erased = input.source.erased().canonical().canonical_bytes().to_vec();
            assert_ne!(original, erased);
            assert_ne!(
                input
                    .checked
                    .intermediate_policy5()
                    .owner()
                    .canonical()
                    .canonical_bytes(),
                input.checked.owner().canonical().canonical_bytes()
            );
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(input.floor).unwrap();
            {
                let (old, storage) = CanonicalKirInventoryV1::derive(
                    input.checked.intermediate_policy5().owner(),
                    &mut budget,
                )
                .unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let (new, storage) =
                    CanonicalKirInventoryV1::derive(input.checked.owner(), &mut budget).unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let is_identity = |operation: &fe2o3_kernel_ir::Operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Binary {
                            op: fe2o3_kernel_ir::BinaryOp::BitXor,
                            ..
                        }
                    )
                };
                assert_eq!(
                    old.operations()
                        .iter()
                        .filter(|row| is_identity(row.operation))
                        .count(),
                    roots
                );
                assert_eq!(
                    new.operations()
                        .iter()
                        .filter(|row| is_identity(row.operation))
                        .count(),
                    0
                );
                let trap = fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap.operation(None);
                let traps: Vec<_> = new
                    .operations()
                    .iter()
                    .enumerate()
                    .filter(|(_, row)| row.operation == &trap)
                    .collect();
                assert_eq!(traps.len(), roots);
                for (ordinal, actual) in traps {
                    let row = &input
                        .checked
                        .continuation()
                        .occurrences()
                        .candidate()
                        .operations[ordinal];
                    assert_eq!(row.output, actual.coordinate);
                    let CanonicalKirOperationOriginV1::Retained(origin) = row.origin else {
                        panic!("every actual trap needs its own retained qualified O origin");
                    };
                    let old_ordinal = old
                        .operations()
                        .iter()
                        .position(|row| row.coordinate == origin)
                        .unwrap();
                    assert_eq!(old.operations()[old_ordinal].operation, actual.operation);
                    // Local trap-block coordinates can stay identical while
                    // preceding DCE changes the flattened inventory position.
                    assert!(old_ordinal > ordinal);
                    assert_ne!(old.operations()[ordinal].coordinate, origin);
                }
            }
            budget
                .release_storage(budget.storage() - input.floor)
                .unwrap();
            let floor = input.floor;
            let owner = admit6(input, WORK, STORAGE).0.unwrap();
            assert_eq!(
                owner
                    .original_source()
                    .executable()
                    .canonical()
                    .canonical_bytes(),
                original
            );
            assert_eq!(owner.erased().canonical().canonical_bytes(), erased);
            assert_eq!(owner.kernels().len(), roots);
            assert!(
                owner
                    .kernels()
                    .iter()
                    .all(|kernel| !kernel.accesses().is_empty())
            );
            owner.verify_equivalence(&mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn erased_policy6_rejects_a_checked_prefix_from_another_source_at_exact_custody_stage() {
    let mut input = continue6(fixture5(true, 1, None));
    input.checked = continue6(fixture5(false, 1, None)).checked;
    input.floor = FLOOR
        + input.source.retained_storage_floor_v1()
        + input.bound_storage
        + input.checked.retained_storage();
    let result = admit6(input, WORK, STORAGE).0;
    assert!(
        matches!(&result, Err(Error6::Prefix(prefix)) if matches!(prefix.as_ref(),
        Error5::Prefix(FinalError::Admission(
            AdmissionError::SourceOutput(ProductionSourceOutputErrorV1::InputCustody))))),
        "foreign genuine checked history: {result:?}"
    );
}
