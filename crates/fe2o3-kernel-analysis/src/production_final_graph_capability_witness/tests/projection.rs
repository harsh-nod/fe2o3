use super::*;
use fe2o3_kernel_ir::{Constant, Operation as KirOperation, Type, ValueDef, ValueId};

#[test]
fn checked_projection_is_required_and_replayed_at_the_w4_handoff() {
    let mut module = kir_module();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(KirOperation::effect_free(
            ValueDef::new(ValueId(0), Type::INDEX),
            OperationKind::Constant(Constant::Index(17)),
        ));
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let mut context = context();
    let (original, _) = clean_function(&mut context);
    let (substitute, _) = clean_function(&mut context);
    let (subject, target) = subject_and_target(&canonical);
    let id = &module.functions[0].id;
    let missing = [ProductionW4LiveFunctionV1::new(id, &original)];
    assert!(matches!(
        execute_production_w4_final_graph_capability_witness_v1(
            &canonical,
            &module,
            subject.clone(),
            target.clone(),
            &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
            &context,
            &missing,
        ),
        Err(ProductionW4ExecutionErrorV1::AnalysisProjectionRejected { .. })
    ));

    let view = crate::compile_canonical_ranked_view_v1(
        &mut context,
        &canonical,
        subject.final_epoch(),
        id,
    )
    .unwrap();
    let live = [ProductionW4LiveFunctionV1::with_ranked_view(
        id, &original, &view,
    )];
    let execution = execute_production_w4_final_graph_capability_witness_v1(
        &canonical,
        &module,
        subject.clone(),
        target.clone(),
        &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
        &context,
        &live,
    )
    .unwrap();
    let ProductionW4FinalGraphExecutionV1::Complete(witness) = execution else {
        panic!("closed constant projection must complete W4");
    };
    witness
        .require_exact_w6_handoff_v1(
            &canonical,
            &module,
            &subject,
            &target,
            &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
            &context,
            &live,
        )
        .unwrap();
    assert!(
        witness
            .require_exact_w6_handoff_v1(
                &canonical,
                &module,
                &subject,
                &target,
                &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
                &context,
                &missing,
            )
            .is_err()
    );
    let substituted = [ProductionW4LiveFunctionV1::with_ranked_view(
        id,
        &substitute,
        &view,
    )];
    assert!(matches!(
        witness.require_exact_w6_handoff_v1(
            &canonical,
            &module,
            &subject,
            &target,
            &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
            &context,
            &substituted,
        ),
        Err(ProductionW4HandoffErrorV1::FunctionSubstituted)
    ));
}
