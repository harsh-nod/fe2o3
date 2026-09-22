// Genuine constructed UnitLocal source on the existing gfx942 fixture lane.
use super::*;
use crate::ProductionRedundantStoreAdmissionErrorV1 as StoreError;

fn input(
    expected: bool,
    roots: usize,
    repeated: bool,
    kill: Option<SemanticStatementKindV1>,
) -> Fixture6 {
    let FinalFixture {
        source,
        bound,
        checked,
        bound_storage,
        ..
    } = final_fixture_from(
        erased_effect_fixture_mode(expected, roots, kill, true, true, repeated),
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
fn consuming_unit_local_moves_whole_n_e_history_and_checks_mutation_and_noop() {
    for expected in [false, true] {
        for roots in [1, 2] {
            for repeated in [false, true] {
                let input = input(expected, roots, repeated, None);
                let floor = input.floor + 61;
                let prefix = admit6(input, WORK, STORAGE).0.unwrap();
                let pointers = (
                    prefix
                        .original_source()
                        .executable()
                        .canonical()
                        .canonical_bytes()
                        .as_ptr(),
                    prefix.erased().canonical().canonical_bytes().as_ptr(),
                    prefix.bound().canonical().canonical_bytes().as_ptr(),
                    prefix.output().canonical().canonical_bytes().as_ptr(),
                );
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                let (owner, storage) = prefix
                    .continue_redundant_private_stores_v1(&mut budget)
                    .unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                owner.assert_independent_added_receipt_v1(storage);
                assert_eq!(
                    pointers,
                    (
                        owner
                            .prefix()
                            .original_source()
                            .executable()
                            .canonical()
                            .canonical_bytes()
                            .as_ptr(),
                        owner
                            .prefix()
                            .erased()
                            .canonical()
                            .canonical_bytes()
                            .as_ptr(),
                        owner
                            .prefix()
                            .bound()
                            .canonical()
                            .canonical_bytes()
                            .as_ptr(),
                        owner
                            .prefix()
                            .output()
                            .canonical()
                            .canonical_bytes()
                            .as_ptr(),
                    )
                );
                assert_ne!(
                    owner
                        .prefix()
                        .original_source()
                        .executable()
                        .canonical()
                        .canonical_bytes(),
                    owner.prefix().erased().canonical().canonical_bytes()
                );
                assert_eq!(
                    owner.continuation().rows().len(),
                    if repeated { roots } else { 0 }
                );
                assert_eq!(
                    owner.output().canonical().canonical_bytes()
                        != owner.prefix().output().canonical().canonical_bytes(),
                    repeated
                );
                assert_eq!(owner.kernels().len(), roots);
                assert!(!owner.grants_artifact_or_launch_authority());
                owner.verify_equivalence(&mut budget).unwrap();
                drop(owner);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            }
        }
    }
}

#[test]
fn consuming_unit_local_fresh_j_formal_global_coordinates_move_and_stale_i_reports_refuse() {
    for roots in [1, 2] {
        let input = input(true, roots, true, None);
        let floor = input.floor;
        let prefix = admit6(input, WORK, STORAGE).0.unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (mut owner, storage) = prefix
            .continue_redundant_private_stores_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(owner.continuation().rows().len(), roots);
        let mut moved = 0;
        for (old, fresh) in owner.prefix().kernels().iter().zip(owner.kernels()) {
            assert_eq!(old.kernel(), fresh.kernel());
            assert_eq!(old.entry(), fresh.entry());
            assert_eq!(old.accesses().len(), fresh.accesses().len());
            assert!(!fresh.accesses().is_empty());
            for (old_access, new_access) in old.accesses().iter().zip(fresh.accesses()) {
                assert_eq!(old_access.kind(), new_access.kind());
                assert_eq!(old_access.address_space(), new_access.address_space());
                let new_location = new_access.location();
                let mut matching =
                    owner
                        .continuation()
                        .retained_operations()
                        .iter()
                        .filter(|row| {
                            let function = &owner.output().module().functions
                                [row.output.block.function.0 as usize];
                            function.id == *fresh.entry()
                                && function.body.as_ref().unwrap().blocks
                                    [row.output.block.block as usize]
                                    .id
                                    == new_location.block
                                && row.output.operation as usize == new_location.operation_index
                        });
                let retained = matching.next().unwrap();
                assert!(matching.next().is_none());
                let original_function = &owner.prefix().output().module().functions
                    [retained.input.block.function.0 as usize];
                let original_block = &original_function.body.as_ref().unwrap().blocks
                    [retained.input.block.block as usize];
                assert_eq!(&original_function.id, old.entry());
                assert_eq!(original_block.id, old_access.location().block);
                assert_eq!(
                    retained.input.operation as usize,
                    old_access.location().operation_index
                );
                assert!(
                    matches!(original_block.operations[retained.input.operation as usize].kind,
                    OperationKind::Store { access, .. } if access.address_space == fe2o3_kernel_ir::AddressSpace::Global)
                );
                assert_eq!(old_access.location().block, new_location.block);
                assert_eq!(
                    old_access.location().operation_index,
                    new_location.operation_index + 1
                );
                moved += 1;
            }
        }
        assert_eq!(
            moved, roots,
            "actual source-derived global coordinates must move"
        );
        assert_ne!(owner.prefix().kernels(), owner.kernels());
        owner.exercise_old_i_report_refusal_v1(&mut budget);
        drop(owner);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn consuming_unit_local_lifetime_restart_keeps_exact_source_refusal_and_entry_floor() {
    let input = input(
        true,
        1,
        true,
        Some(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(3),
        )),
    );
    let floor = input.floor + 89;
    let prefix = admit6(input, WORK, STORAGE).0.unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let candidate = fe2o3_kernel_opt::prepare_owned_redundant_store_continuation_v1(
        prefix.output(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(candidate.rows().len(), 1);
    drop(candidate);
    assert!(matches!(
        prefix.continue_redundant_private_stores_v1(&mut budget),
        Err(StoreError::Admission(
            crate::ProductionCheckedOutputAdmissionErrorPolicy3V1::Unsupported {
                phase: "redundant Store source",
                detail: "no lifetime or Move invalidation between identical Stores",
            }
        ))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn consuming_unit_local_exact_one_short_and_underfloor_budgets_preserve_caller_reservations() {
    let run = |work_limit, storage_limit| {
        let input = input(true, 1, true, None);
        let floor = input.floor + 53;
        let prefix = admit6(input, WORK, STORAGE).0.unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = prefix.continue_redundant_private_stores_v1(&mut budget);
        let success = result.is_ok();
        drop(result);
        assert_eq!(budget.storage(), floor);
        (success, budget.work(), budget.peak_storage())
    };
    let (ok, work, peak) = run(WORK, STORAGE);
    assert!(ok);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
    let input = input(true, 1, true, None);
    let prefix = admit6(input, WORK, STORAGE).0.unwrap();
    let minimum = prefix.retained_input_storage_floor_v1().unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(minimum - 1).unwrap();
    assert!(matches!(
        prefix.continue_redundant_private_stores_v1(&mut budget),
        Err(StoreError::Resource(ArgumentResourceV1::Accounting))
    ));
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), minimum - 1);
}
