use super::*;
use fe2o3_amdgcn_model::{
    ProductionTargetCoordinateErrorV1 as CoordinateError,
    check_production_target_coordinate_preservation_v1 as check,
};

fn admit(module: &Module, budget: &mut Budget<'_>) -> (Owner, usize) {
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, budget).unwrap();
    let retained = storage.retained_storage();
    budget.reserve_storage(retained).unwrap();
    (owner, retained)
}

#[test]
fn actual_binder_delta_is_exact_for_both_targets_and_non_dense_executables() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for changed in [false, true] {
            let module = neutral(changed);
            let actual = bind_production_target_v1(&module, profile).unwrap();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(PREFIX).unwrap();
            let (input, a) = admit(&module, &mut budget);
            let (bound, b) = admit(actual.module(), &mut budget);
            let floor = budget.storage();
            let (coordinates, storage) = check(&input, &bound, profile, &mut budget).unwrap();
            assert!(std::ptr::eq(coordinates.input(), &input));
            assert!(std::ptr::eq(coordinates.output(), &bound));
            assert!(!coordinates.grants_authority());
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let checked = optimize_checked_canonical_kernel_ir_v1(&bound, &mut budget).unwrap();
            assert_eq!(checked.owner().module().kernels, bound.module().kernels);
            assert_eq!(
                checked.owner().module().required_capabilities,
                bound.module().required_capabilities
            );
            assert_eq!(
                checked.report().passes().iter().any(|pass| pass.changed()),
                changed
            );
            drop(checked);
            #[allow(
                clippy::drop_non_drop,
                reason = "End the borrowed witness before releasing its ledger reservation"
            )]
            drop(coordinates);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
            drop(bound);
            budget.release_storage(b).unwrap();
            drop(input);
            budget.release_storage(a).unwrap();
            assert_eq!(budget.storage(), PREFIX);
        }
    }
}

#[test]
fn exact_binder_checker_refuses_wrong_profile_removed_added_and_body_substitutions() {
    let module = neutral(true);
    let actual = bind_production_target_v1(&module, Profile::Gfx942).unwrap();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (input, a) = admit(&module, &mut budget);
    let (bound, b) = admit(actual.module(), &mut budget);
    let floor = budget.storage();
    assert!(matches!(
        check(&input, &bound, Profile::Gfx950, &mut budget),
        Err(CoordinateError::Metadata(_))
    ));
    assert_eq!(budget.storage(), floor);
    let mut candidates = Vec::new();
    let mut missing = actual.module().clone();
    missing.kernels[0]
        .required_capabilities
        .remove(&TargetCapability::WaveWidth(WaveWidth::Wave64));
    candidates.push((missing, false));
    let mut extra = actual.module().clone();
    extra
        .required_capabilities
        .insert(TargetCapability::Extension {
            namespace: "test".into(),
            name: "unissued-capability".into(),
        });
    candidates.push((extra, false));
    let mut wrong_body = actual.module().clone();
    wrong_body.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::Constant(Constant::U32(8));
    candidates.push((wrong_body, true));
    for (candidate, body_mismatch) in candidates {
        let (other, retained) = admit(&candidate, &mut budget);
        let before = budget.storage();
        let error = check(&input, &other, Profile::Gfx942, &mut budget).unwrap_err();
        if body_mismatch {
            assert!(matches!(error, CoordinateError::Coordinates(_)));
        } else {
            assert!(matches!(error, CoordinateError::Metadata(_)));
        }
        assert_eq!(budget.storage(), before);
        drop(other);
        budget.release_storage(retained).unwrap();
    }
    drop(bound);
    budget.release_storage(b).unwrap();
    drop(input);
    budget.release_storage(a).unwrap();
    assert_eq!(budget.storage(), 0);
}
