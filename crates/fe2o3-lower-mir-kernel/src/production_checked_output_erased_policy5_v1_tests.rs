// Child of the existing genuine erased Policy4 fixtures. Semantic-MIR and
// fresh source/ranked/N/E/B/C/S/O coverage, not ordinary-Rust or runtime proof.
use super::*;
type Final5 = crate::ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1;
type Error5 = crate::ProductionCheckedOutputAdmissionErrorPolicy5V1;

struct Fixture5 {
    source: ProductionUnitLocalErasedSourceOwnerV1,
    bound: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy5V1,
    floor: usize,
    bound_storage: usize,
}
fn fixture5(expected: bool, roots: usize, mutation: Option<fn(&mut Module)>) -> Fixture5 {
    let FinalFixture {
        source,
        bound,
        checked,
        bound_storage,
        ..
    } = final_fixture_from(
        erased_effect_fixture_with_load_forwarding(expected, roots),
        mutation,
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
    Fixture5 {
        source,
        bound,
        checked,
        floor: budget.storage(),
        bound_storage,
    }
}
fn admit5(
    input: Fixture5,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<Final5, Error5>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(input.floor).unwrap();
    let result = Final5::try_admit_v1(input.source, input.bound, input.checked, &mut budget);
    assert_eq!(budget.storage(), input.floor);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn erased_policy5_admits_nonidentity_load_forwarding_after_real_global_effect() {
    for expected in [false, true] {
        for roots in [1, 2] {
            let input = fixture5(expected, roots, None);
            assert!(
                input
                    .checked
                    .intermediate_policy4()
                    .forwarding_rows()
                    .is_empty()
            );
            assert_eq!(input.checked.load_forwarding_rows().len(), roots);
            assert_eq!(
                private_load_count(input.checked.intermediate_policy4().owner().module()),
                roots * 2
            );
            assert_eq!(private_load_count(input.checked.owner().module()), roots);
            let neutral = input
                .source
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes()
                .to_vec();
            let erased = input.source.erased().canonical().canonical_bytes().to_vec();
            let output = input.checked.owner().canonical().canonical_bytes().to_vec();
            assert_ne!(neutral, erased);
            let floor = input.floor;
            let retained =
                input.source.retained_storage_floor_v1() + input.checked.retained_storage();
            assert_eq!(floor, FLOOR + retained + input.bound_storage);
            let owner = admit5(input, WORK, STORAGE).0.unwrap();
            assert_eq!(
                owner
                    .original_source()
                    .executable()
                    .canonical()
                    .canonical_bytes(),
                neutral
            );
            assert_eq!(owner.erased().canonical().canonical_bytes(), erased);
            assert_eq!(owner.output().canonical().canonical_bytes(), output);
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
fn erased_policy5_cannot_skip_source_lifetime_root_or_effect_replay() {
    for mutation in 0..5 {
        let mut input = fixture5(true, 2, None);
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
        assert!(
            matches!(
                admit5(input, WORK, STORAGE).0,
                Err(Error5::Prefix(FinalError::Admission(
                    AdmissionError::Source(_)
                )))
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn fresh_verified_bound_mutation_is_not_authorized_by_valid_load_rewrite() {
    for mutation in [
        change_root_private_value as fn(&mut Module),
        change_root_global_value,
    ] {
        let input = fixture5(true, 1, Some(mutation));
        assert!(matches!(
            admit5(input, WORK, STORAGE).0,
            Err(Error5::Prefix(FinalError::Admission(
                AdmissionError::Coordinates(_)
            )))
        ));
    }
}

#[test]
fn checked_prefix_from_another_original_source_cannot_be_spliced() {
    let mut input = fixture5(true, 1, None);
    input.checked = fixture5(false, 1, None).checked;
    input.floor = FLOOR
        + input.source.retained_storage_floor_v1()
        + input.bound_storage
        + input.checked.retained_storage();
    assert!(matches!(
        admit5(input, WORK, STORAGE).0,
        Err(Error5::Prefix(FinalError::Admission(
            AdmissionError::SourceOutput(ProductionSourceOutputErrorV1::InputCustody)
        )))
    ));
}

#[test]
fn source_initialized_read_is_required_even_when_optimizer_reports_no_load_rows() {
    for killed in [
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)),
        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(3)),
    ] {
        let original = erased_effect_fixture_with_lifetime(true, 1, Some(killed));
        let site = [(0, 1, Some(2), 3)];
        let FinalFixture {
            source,
            bound,
            checked,
            bound_storage,
            ..
        } = retained_load_fault_v1_tests::with_exact_sites(&site, || {
            final_fixture_from(original, None)
        });
        drop(checked);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget
            .reserve_storage(FLOOR + source.retained_storage_floor_v1() + bound_storage)
            .unwrap();
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, &mut budget)
                .unwrap();
        assert!(checked.load_forwarding_rows().is_empty());
        budget.reserve_storage(checked.retained_storage()).unwrap();
        let floor = budget.storage();
        let result = retained_load_fault_v1_tests::with_exact_sites(&site, || {
            Final5::try_admit_v1(source, bound, checked, &mut budget)
        });
        assert!(matches!(
            result,
            Err(Error5::Prefix(FinalError::Admission(
                AdmissionError::Unsupported {
                    phase: "private source",
                    detail: "no source lifetime or Move invalidation between Store and Load",
                }
            )))
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn final_erased_policy5_uses_exact_receipts_and_restores_late_failed_budget_floor() {
    let baseline = fixture5(true, 1, None);
    let floor = baseline.floor;
    let (owner, spent, peak) = admit5(baseline, WORK, STORAGE);
    drop(owner.unwrap());
    assert!(peak > floor);
    for (work, storage, success) in [
        (spent, peak, true),
        (spent - 1, peak, false),
        (spent, peak - 1, false),
    ] {
        let input = fixture5(true, 1, None);
        assert_eq!(input.floor, floor);
        assert_eq!(admit5(input, work, storage).0.is_ok(), success);
    }
}
