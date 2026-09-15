use super::*;
use fe2o3_kernel_ir::{Axis, IndexKind, IntrinsicKind};

fn hierarchy_source(kind: IndexKind, axis: Axis) -> Module {
    let mut source = module();
    source.kernels[0].domain = LaunchDomain::D3 {
        x: LaunchExtent::Static(130),
        y: LaunchExtent::Static(5),
        z: LaunchExtent::Static(3),
    };
    source.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(0), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex { kind, axis },
                Type::INDEX,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), Type::INDEX),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::Index(17)),
        ),
    ];
    source
}

#[test]
fn final_owner_checks_canonical_hierarchy_without_changing_executable_ir() {
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        for kind in [
            IndexKind::Local,
            IndexKind::Workgroup,
            IndexKind::WorkgroupSize,
        ] {
            let source = hierarchy_source(kind, axis);
            let (canonical, contract) = input_for(source.clone());
            let bytes = canonical.canonical_bytes().to_vec();
            let owner = ProductionFinalGraphOwnerV1::try_new(canonical, 8, contract).unwrap();
            assert!(owner.ranked_views[0].is_some());
            let mut verified = owner.verify().unwrap();
            verified.revalidate_live().unwrap();
            let (canonical, output, report) = verified.into_parts().unwrap();
            assert_eq!(canonical.canonical_bytes(), bytes);
            assert_eq!(output, source);
            assert_eq!(
                report.functions()[0]
                    .checks()
                    .preservation()
                    .certificates()
                    .len(),
                9
            );
        }
    }
}

#[test]
fn projected_hierarchy_rejects_geometry_and_target_decision_substitution() {
    let source = hierarchy_source(IndexKind::Local, Axis::X);
    let (_, old_contract) = input_for(source.clone());
    let mut changed = source;
    changed.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
    let canonical = VerifiedCanonicalKernelIrV13::from_module(changed.clone()).unwrap();
    assert!(matches!(
        ProductionFinalGraphOwnerV1::try_new(canonical, 8, old_contract),
        Err(ProductionFinalGraphVerificationErrorV1::TargetSubjectMismatch)
    ));

    // This contract names the changed graph but still carries 64-lane resource
    // decisions. Projection must not make those decisions describe 32 lanes.
    let (canonical, mismatched) = input_for(changed);
    let owner = ProductionFinalGraphOwnerV1::try_new(canonical, 8, mismatched).unwrap();
    assert!(matches!(
        owner.verify(),
        Err(ProductionFinalGraphVerificationErrorV1::ResourceClosureMismatch)
    ));
}

fn projected_owner(bits: u64) -> ProductionFinalGraphOwnerV1 {
    let mut source = module();
    source.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(0), Type::INDEX),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::Index(bits)),
        ));
    let (canonical, contract) = input_for(source);
    ProductionFinalGraphOwnerV1::try_new(canonical, 8, contract).unwrap()
}

#[test]
fn final_owner_uses_checked_projection_and_retains_exact_canonical_bytes() {
    let owner = projected_owner(17);
    assert!(owner.ranked_views[0].is_some());
    let bytes = owner.canonical.canonical_bytes().to_vec();
    let mut verified = owner.verify().unwrap();
    verified.revalidate_live().unwrap();
    let (canonical, _, report) = verified.into_parts().unwrap();
    assert_eq!(canonical.canonical_bytes(), bytes);
    assert_eq!(report.functions().len(), 1);
}

#[test]
fn final_owner_rejects_omitted_truncated_and_substituted_analysis_views() {
    let mut omitted = projected_owner(17);
    omitted.ranked_views[0] = None;
    assert!(matches!(
        omitted.verify(),
        Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch)
    ));

    let mut truncated = projected_owner(17);
    truncated.ranked_views.clear();
    assert!(matches!(
        truncated.verify(),
        Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch)
    ));

    let mut substituted = projected_owner(17);
    let mut other = projected_owner(19);
    substituted.ranked_views[0] = other.ranked_views[0].take();
    assert!(matches!(
        substituted.verify(),
        Err(ProductionFinalGraphVerificationErrorV1::AnalysisProjection(
            _
        ))
    ));
}

#[test]
fn final_owner_rejects_stale_analysis_epoch_and_touched_verified_analysis() {
    let mut stale = projected_owner(17);
    stale.final_epoch += 1;
    assert!(matches!(
        stale.verify(),
        Err(ProductionFinalGraphVerificationErrorV1::AnalysisProjection(
            _
        ))
    ));

    let mut verified = projected_owner(17).verify().unwrap();
    let pointer = verified.owner.ranked_views[0]
        .as_ref()
        .unwrap()
        .pliron()
        .get_operation();
    // Even a mutable borrow without a semantic change invalidates the receipt.
    drop(pointer.deref_mut(&mut verified.owner.session.context));
    assert!(matches!(
        verified.into_parts(),
        Err(ProductionFinalGraphVerificationErrorV1::MutationAfterVerification { .. })
    ));
}
