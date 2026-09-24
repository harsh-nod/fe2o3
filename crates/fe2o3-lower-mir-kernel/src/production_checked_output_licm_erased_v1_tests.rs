use super::*;

#[test]
fn source_licm_unit_local_rejects_reordered_final_reports() {
    let (prefix, inherited) = prefix(Profile::Gfx942, true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (mut owner, added) = prefix.continue_licm_v1(&mut budget).unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    owner.exercise_licm_report_order_v1(&mut budget);
}

fn roots(source: &ProductionPreRankedKirOwnerV1) -> Vec<ProductionRankedSemanticProjectionRootV1> {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    source
        .source_launch()
        .roots()
        .iter()
        .map(|root| {
            let function = &source.semantic_ssa().source_semantic().functions()
                [root.selected_root().index() as usize];
            let name =
                std::str::from_utf8(function.kernel_entry().unwrap().export_symbol().as_bytes())
                    .unwrap();
            let layout = root.layout();
            let kernel = ProductionRankedKernelV1::new(
                name,
                0,
                vec![ProductionRankedBlockV1::new(
                    vec![ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: layout.grid_identity(),
                        global_extents: layout.global_extents(),
                        workgroup_extents: layout.workgroup_extents(),
                        subgroup_size: layout.subgroup_size(),
                        full_physical_workgroups: layout.full_physical_workgroups(),
                    }],
                    ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            let lowering = compile_ranked_kernel_for_lowering_v1(
                ProductionConstructionV1::ranked_kernel("source_licm_unit", kernel).unwrap(),
                ProductionSessionLimitsV1::default(),
            )
            .unwrap();
            assert!(lowering.all_mandatory_reports_are_clean());
            ProductionRankedSemanticProjectionRootV1::new(
                root.selected_root(),
                root.source_rank(),
                lowering,
                "constructed private source loop; exact empty external footprint".to_owned(),
                vec![],
                vec![],
            )
        })
        .collect()
}

pub(super) fn prefix(profile: Profile, mutation: bool) -> (ErasedPreheaders, usize) {
    let original = source(true, mutation);
    prefix_from_source(original, profile, mutation)
}

pub(super) fn prefix_from_source(
    original: ProductionPreRankedKirOwnerV1,
    profile: Profile,
    mutation: bool,
) -> (ErasedPreheaders, usize) {
    let roots = roots(&original);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let bytes = ProductionUnitLocalErasedSourceOwnerV1::input_storage_floor_v1(
        &original,
        &roots,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(FLOOR + bytes).unwrap();
    let (source, added) =
        ProductionUnitLocalErasedSourceOwnerV1::try_produce_v1(original, roots, &mut budget)
            .unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    assert_eq!(source.deleted_call_count(), 3);
    assert_eq!(source.deleted_function_count(), 2);
    let binding =
        dialect_amdgcn::bind_production_target_v1(source.erased().module(), profile).unwrap();
    let (bound, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            binding.module(),
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let fifth =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(&bound, &mut budget)
            .unwrap();
    budget.reserve_storage(fifth.retained_storage()).unwrap();
    let sixth = fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
        &bound,
        fifth,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(sixth.retained_storage()).unwrap();
    let prefix = crate::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1::try_admit_v1(
        source,
        bound,
        sixth,
        &mut budget,
    )
    .unwrap();
    let (prefix, added) = prefix
        .continue_redundant_private_stores_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    let (prefix, added) = prefix
        .continue_commutative_bitwise_cse_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    let (prefix, added) = prefix
        .continue_private_cell_promotion_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    let (prefix, added) = prefix.continue_loop_preheaders_v1(&mut budget).unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    if mutation {
        assert_eq!(prefix.origins().len(), 2);
        assert_eq!(prefix.incoming_origins().len(), 4);
        assert!(!prefix.parameter_origins().is_empty());
    }
    (prefix, budget.storage())
}

#[test]
fn source_licm_unit_local_keeps_n_e_and_actual_motion_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let (prefix, inherited) = prefix(profile, mutation);
            let original = prefix
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes()
                .as_ptr();
            let erased = prefix
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .erased()
                .canonical()
                .canonical_bytes()
                .as_ptr();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(inherited).unwrap();
            let (owner, added) = prefix.continue_licm_v1(&mut budget).unwrap();
            assert_eq!(budget.storage(), inherited);
            budget.reserve_storage(added.retained_storage()).unwrap();
            assert_eq!(
                owner
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .original_source()
                    .executable()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                original
            );
            assert_eq!(
                owner
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .erased()
                    .canonical()
                    .canonical_bytes()
                    .as_ptr(),
                erased
            );
            assert_eq!(owner.kernels().len(), 2);
            if mutation {
                actual_mutation(
                    owner.prefix().output(),
                    owner.output(),
                    owner.operation_origins(),
                );
                owner.exercise_licm_source_join_v1(&mut budget);
            } else {
                assert_eq!(
                    owner.output().canonical().canonical_bytes(),
                    owner.prefix().output().canonical().canonical_bytes()
                );
                assert!(!std::ptr::eq(owner.output(), owner.prefix().output()));
                assert!(
                    owner
                        .operation_origins()
                        .iter()
                        .all(|row| row.hoist.is_none())
                );
            }
            owner.verify_equivalence(&mut budget).unwrap();
            assert_eq!(budget.storage(), inherited + added.retained_storage());
            assert!(!owner.grants_artifact_or_launch_authority());
        }
    }
}
