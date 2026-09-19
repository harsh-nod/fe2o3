// Child of erased-owner tests; these are genuine semantic source/N/E/B/C/O
// fixtures, not collected Rust, authenticated runtime or artifact qualification.
use super::*;

#[path = "production_checked_output_erased_policy5_v1_tests.rs"]
mod policy5_tests;

type FinalErased = crate::ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1;
type FinalError = crate::ProductionCheckedOutputAdmissionErrorPolicy4V1;
type AdmissionError = crate::ProductionCheckedOutputAdmissionErrorPolicy3V1;

struct FinalFixture {
    source: ProductionUnitLocalErasedSourceOwnerV1,
    bound: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1,
    bound_storage: usize,
    floor: usize,
}

fn final_fixture(expected: bool, roots: usize, mutation: Option<fn(&mut Module)>) -> FinalFixture {
    final_fixture_from(erased_effect_fixture(expected, roots), mutation)
}

fn final_fixture_from(
    (original, roots): (
        ProductionPreRankedKirOwnerV1,
        Vec<ProductionRankedSemanticProjectionRootV1>,
    ),
    mutation: Option<fn(&mut Module)>,
) -> FinalFixture {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(FLOOR + erased_input_floor(&original, &roots))
        .unwrap();
    let (source, storage) = ErasedOwner::try_produce_v1(original, roots, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let binding = dialect_amdgcn::bind_production_target_v1(
        source.erased().module(),
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    let mut candidate = binding.module().clone();
    if let Some(mutation) = mutation {
        mutation(&mut candidate);
    }
    let (bound, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        &candidate, &mut budget).unwrap();
    let bound_storage = storage.retained_storage();
    budget.reserve_storage(bound_storage).unwrap();
    drop(candidate);
    drop(binding);
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, &mut budget)
            .unwrap();
    budget.reserve_storage(checked.retained_storage()).unwrap();
    FinalFixture {
        source,
        bound,
        checked,
        bound_storage,
        floor: budget.storage(),
    }
}

fn final_admit(
    input: FinalFixture,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<FinalErased, FinalError>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(input.floor).unwrap();
    let result = FinalErased::try_admit_v1(input.source, input.bound, input.checked, &mut budget);
    assert_eq!(budget.storage(), input.floor);
    (result, budget.work(), budget.peak_storage())
}

fn private_load_count(module: &Module) -> usize {
    module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(&operation.kind,
            OperationKind::Load { access, .. } if access.address_space == AddressSpace::Private)
        })
        .count()
}

#[test]
fn final_erased_policy4_composes_shared_roots_and_nonidentity_private_forwarding() {
    for expected in [false, true] {
        for roots in [1, 2] {
            let input = final_fixture(expected, roots, None);
            assert_eq!(input.checked.forwarding_rows().len(), roots);
            assert_eq!(
                private_load_count(input.checked.intermediate_policy3().owner().module()),
                roots
            );
            assert_eq!(private_load_count(input.checked.owner().module()), 0);
            let erased = input.source.erased().canonical().canonical_bytes().to_vec();
            let neutral = input
                .source
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes()
                .to_vec();
            let bound = input.bound.canonical().canonical_bytes().to_vec();
            let output = input.checked.owner().canonical().canonical_bytes().to_vec();
            assert_ne!(neutral, erased);
            assert_ne!(
                input
                    .checked
                    .intermediate_policy3()
                    .owner()
                    .canonical()
                    .canonical_bytes(),
                output
            );
            let expected_floor =
                input.source.retained_storage_floor_v1() + input.checked.retained_storage();
            let original_bound_storage = input.bound_storage;
            assert_eq!(input.floor, FLOOR + expected_floor + input.bound_storage);
            let owner = final_admit(input, WORK, STORAGE).0.unwrap();
            assert_eq!(
                owner
                    .original_source()
                    .executable()
                    .canonical()
                    .canonical_bytes(),
                neutral
            );
            assert_eq!(owner.erased().canonical().canonical_bytes(), erased);
            assert_eq!(owner.bound().canonical().canonical_bytes(), bound);
            assert_eq!(owner.output().canonical().canonical_bytes(), output);
            assert_eq!(
                owner.retained_input_storage_floor_v1().unwrap(),
                expected_floor
            );
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
            // Preserve the receipt from this exact B's original admission.
            let floor = FLOOR + expected_floor + original_bound_storage;
            budget.reserve_storage(floor).unwrap();
            owner.verify_equivalence(&mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn final_erased_policy4_refuses_original_spans_origins_maps_and_ranked_effect_changes() {
    for mutation in 0..5 {
        let mut input = final_fixture(true, 2, None);
        match mutation {
            0 => {
                input
                    .source
                    .original
                    .correspondence
                    .statement_operation_spans[0]
                    .operation_count += 1
            }
            1 => {
                input.source.original.assert_origins.bindings[0].expected =
                    !input.source.original.assert_origins.bindings[0].expected
            }
            2 => {
                input.source.operations[0] =
                    Some(ProductionUnitLocalOperationDeletionV1::DeletedUnitCall)
            }
            3 => {
                input.source.roots[0].access_sources[0] =
                    ProductionRankedAccessSourceV1::new(2, Some(99), 0, 0, 5)
            }
            4 => input.source.original.launch_roots[0].global_extents[0] += 1,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                final_admit(input, WORK, STORAGE).0,
                Err(FinalError::Admission(AdmissionError::Source(_)))
            ),
            "mutation {mutation}"
        );
    }
}

fn change_root_private_value(module: &mut Module) {
    let operation = module
        .functions
        .iter_mut()
        .filter_map(|function| function.body.as_mut())
        .flat_map(|body| &mut body.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::Constant(Constant::U32(7))))
        .unwrap();
    operation.kind = OperationKind::Constant(Constant::U32(6));
}

fn change_root_global_value(module: &mut Module) {
    let operations: Vec<_> = module
        .functions
        .iter_mut()
        .filter_map(|function| function.body.as_mut())
        .flat_map(|body| &mut body.blocks)
        .flat_map(|block| &mut block.operations)
        .collect();
    let operation = operations
        .into_iter()
        .rev()
        .find(|operation| matches!(operation.kind, OperationKind::Constant(Constant::U32(7))))
        .unwrap();
    operation.kind = OperationKind::Constant(Constant::U32(9));
}

#[test]
fn final_erased_policy4_refuses_fresh_verified_b_value_mutations() {
    for mutation in [
        change_root_private_value as fn(&mut Module),
        change_root_global_value,
    ] {
        let input = final_fixture(true, 1, Some(mutation));
        assert!(matches!(
            final_admit(input, WORK, STORAGE).0,
            Err(FinalError::Admission(AdmissionError::Coordinates(_)))
        ));
    }
}

#[test]
fn final_erased_policy4_refuses_checked_history_from_another_source() {
    let mut original = final_fixture(true, 1, None);
    let replacement = final_fixture(false, 1, None);
    original.checked = replacement.checked;
    original.floor = FLOOR
        + original.source.retained_storage_floor_v1()
        + original.bound_storage
        + original.checked.retained_storage();
    assert!(matches!(
        final_admit(original, WORK, STORAGE).0,
        Err(FinalError::Admission(AdmissionError::SourceOutput(
            ProductionSourceOutputErrorV1::InputCustody
        )))
    ));
}

#[test]
fn final_erased_policy4_retains_source_lifetime_kill_refusals_after_erasure() {
    for killed in [
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)),
        SemanticStatementKindV1::Deinitialize(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], ARRAY_SCALAR).unwrap(),
        ),
    ] {
        let input = final_fixture_from(
            erased_effect_fixture_with_lifetime(true, 1, Some(killed)),
            None,
        );
        assert!(matches!(
            final_admit(input, WORK, STORAGE).0,
            Err(FinalError::Admission(AdmissionError::Unsupported {
                phase: "private source",
                detail: "no source lifetime or Move invalidation between Store and Load",
            }))
        ));
    }
}

#[test]
fn final_erased_policy4_restores_full_input_floor_at_work_and_storage_edges() {
    let (result, work, peak) = final_admit(final_fixture(false, 2, None), WORK, STORAGE);
    drop(result.unwrap());
    assert!(work > 1 && peak > 1);
    final_admit(final_fixture(false, 2, None), work, peak)
        .0
        .unwrap();
    assert!(
        final_admit(final_fixture(false, 2, None), work - 1, STORAGE)
            .0
            .is_err()
    );
    assert!(
        final_admit(final_fixture(false, 2, None), WORK, peak - 1)
            .0
            .is_err()
    );
    let input = final_fixture(false, 1, None);
    let required = input.source.retained_storage_floor_v1() + input.checked.retained_storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(required - 1).unwrap();
    assert!(matches!(
        FinalErased::try_admit_v1(input.source, input.bound, input.checked, &mut budget),
        Err(FinalError::Admission(AdmissionError::Resource(
            AssertOriginResourceV1::Accounting
        )))
    ));
    assert_eq!(budget.storage(), required - 1);
}
