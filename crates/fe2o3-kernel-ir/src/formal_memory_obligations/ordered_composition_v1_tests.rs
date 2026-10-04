//! Reuses the structural owner's one shared inert fixture; no source-custody claim.
use super::*;

fn extract(
    owner: &VerifiedOrderedProgramCompositionV1,
    index: FormalIndexWidth,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<
        (
            FormalMemoryObligationAnalysis,
            OrderedCompositionFormalStorageV1,
        ),
        OrderedCompositionFormalErrorV1,
    >,
    usize,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let report = derive_ordered_composition_memory_obligations_v1(
        owner,
        &owner.canonical().module().kernels[0].id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        },
        index,
        &mut budget,
    );
    assert_eq!(budget.storage(), floor);
    (report, budget.work(), budget.peak_storage())
}
#[test]
fn composition_formal_helpers_and_regions_do_not_erase_real_root_write_conflicts() {
    for module in [
        fixture::module(1, &[], &[], 3),
        fixture::module(0, &[1], &[0, 0], 3),
        fixture::module(1, &[0], &[0], 3),
    ] {
        let owner = compose(&module).unwrap();
        let (report, _, _) = extract(&owner, FormalIndexWidth::Bits64, 17, 1_000_000, 1_000_000);
        let (report, _) = report.unwrap();
        assert!(report.is_complete());
        assert_eq!(report.obligations().accesses().len(), 1);
        assert!(!report.obligations().inter_invocation_conflicts().is_empty());
        assert_eq!(
            report.obligations().accesses()[0].byte_offset(),
            ByteExpression::constant(0)
        );
        let generic = derive_kernel_memory_obligations_from_verified_for_launch(
            owner.canonical().verified_module_ref_v1(),
            &module.kernels[0].id,
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            },
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        assert!(!generic.is_complete());
        assert_eq!(
            generic.obligations().accesses(),
            report.obligations().accesses()
        );
        assert_eq!(
            generic.obligations().bounds_requirements(),
            report.obligations().bounds_requirements()
        );
    }
}
#[test]
fn composition_formal_preserves_unknown_width_and_launch_refusals() {
    let owner = compose(&fixture::module(0, &[1], &[0], 1)).unwrap();
    for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Unknown] {
        let (report, _, _) = extract(&owner, width, 19, 1_000_000, 1_000_000);
        assert!(!report.unwrap().0.is_complete());
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    let report = derive_ordered_composition_memory_obligations_v1(
        &owner,
        &owner.canonical().module().kernels[0].id,
        ExplicitLaunchExtent::Unknown,
        FormalIndexWidth::Bits64,
        &mut budget,
    )
    .unwrap()
    .0;
    assert!(!report.is_complete());
}
#[test]
fn composition_formal_wrong_selected_root_and_resource_cutoffs_refuse() {
    let owner = compose(&fixture::module(0, &[1], &[0, 0], 3)).unwrap();
    for floor in [0, 71] {
        let (report, work, peak) = extract(
            &owner,
            FormalIndexWidth::Bits64,
            floor,
            1_000_000,
            1_000_000,
        );
        assert!(report.is_ok());
        assert!(
            extract(&owner, FormalIndexWidth::Bits64, floor, work, peak)
                .0
                .is_ok()
        );
        assert!(
            extract(&owner, FormalIndexWidth::Bits64, floor, work - 1, peak)
                .0
                .is_err()
        );
        assert!(
            extract(&owner, FormalIndexWidth::Bits64, floor, work, peak - 1)
                .0
                .is_err()
        );
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(23).unwrap();
    assert!(
        derive_ordered_composition_memory_obligations_v1(
            &owner,
            &KernelId::new("other"),
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1]
            },
            FormalIndexWidth::Bits64,
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), 23);
}

#[path = "ordered_composition_bool_switch_v1_tests.rs"]
mod bool_switch;
