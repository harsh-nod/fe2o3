use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticSsaLimitsV1};
include!("frame_tests.rs");
mod physical_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/reusable_lds/physical_fixture.rs"
    ));
}

#[test]
fn reusable_lds_bindings_require_the_same_value_and_complete_capability_type() {
    let (_, _, ty, _, _) = physical_fixture::fixture();
    let binding = SemanticValueBindingV1::Value {
        id: ValueId(1),
        ty: ty.clone(),
    };
    assert!(same_capability_binding(
        Some(&binding),
        Some(&binding.clone())
    ));
    assert!(!same_capability_binding(None, None));
    assert!(!same_capability_binding(Some(&binding), None));
    assert!(!same_capability_binding(
        Some(&SemanticValueBindingV1::Unit),
        Some(&SemanticValueBindingV1::Unit)
    ));
    let other = SemanticValueBindingV1::Value {
        id: ValueId(2),
        ty: ty.clone(),
    };
    assert!(!same_capability_binding(Some(&binding), Some(&other)));
    let Type::ExecutionCapability(original) = ty else {
        unreachable!()
    };
    for mutation in 0..4 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.epoch = Some([99; 32]),
            1 => changed.workgroup_brand = Some([99; 32]),
            2 => changed.provenance.kernel_binding = [99; 32],
            3 => changed.source_type = fe2o3_kernel_ir::ExecutionTypeIdentityV1::new([99; 32]),
            _ => unreachable!(),
        }
        let other = SemanticValueBindingV1::Value {
            id: ValueId(1),
            ty: Type::ExecutionCapability(changed),
        };
        assert!(!same_capability_binding(Some(&binding), Some(&other)));
    }
}

mod fixture {
    use fe2o3_mir_model::semantic_mir_v1::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-pliron/src/production/semantic_ssa/defined_reusable_lds_results/source.rs"
    ));
}

fn owner() -> ProductionSemanticSsaOwnerV1 {
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            fixture::source(true),
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn reusable_lds_lowerer_requires_exact_allocation_parameter_return_sequence() {
    let owner = owner();
    let plan = owner
        .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let [row] = plan.defined_reusable_lds_results() else {
        panic!("one receipt")
    };
    let input = plan
        .plan()
        .resolved_events(fe2o3_mir_model::SsaBlockIdV1::new(
            row.parameter_block().index(),
        ))
        .unwrap();
    let output = plan
        .plan()
        .resolved_events(fe2o3_mir_model::SsaBlockIdV1::new(
            row.return_block().index(),
        ))
        .unwrap();
    let (allocation, parameter) = input_events(*row, input).unwrap();
    let (returned, destination) = output_events(*row, parameter, output).unwrap();
    assert_ne!(allocation, parameter);
    assert_ne!(parameter, returned);
    assert_ne!(returned, destination);
    assert!(matches!(
        output_events(*row, allocation, output),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

#[test]
fn reusable_lds_lowerer_rejects_missing_move_kill_and_reordered_definitions() {
    let owner = owner();
    let plan = owner
        .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let row = plan.defined_reusable_lds_results()[0];
    let input = plan
        .plan()
        .resolved_events(fe2o3_mir_model::SsaBlockIdV1::new(
            row.parameter_block().index(),
        ))
        .unwrap();
    let output = plan
        .plan()
        .resolved_events(fe2o3_mir_model::SsaBlockIdV1::new(
            row.return_block().index(),
        ))
        .unwrap();
    let (_, parameter) = input_events(row, input).unwrap();
    for omitted in 0..input.len() {
        let mut changed = input.to_vec();
        changed.remove(omitted);
        assert!(matches!(
            input_events(row, &changed),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    }
    for omitted in 0..output.len() {
        let mut changed = output.to_vec();
        changed.remove(omitted);
        assert!(matches!(
            output_events(row, parameter, &changed),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    }
    let mut changed = output.to_vec();
    changed.swap(2, 5);
    assert!(matches!(
        output_events(row, parameter, &changed),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}
