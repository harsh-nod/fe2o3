use dialect_kernel::AlgorithmOp;
use dialect_mir::{
    MirTypeId,
    pliron::{MirDialectLimits, MirModuleOp},
};
use fe2o3_lower_mir_kernel::{
    DIALECT_REGISTRATION_ORDER, LoweringConfig, LoweringLimits, LoweringResult,
    MirKernelLoweringPass, PASS_NAME, PassRegistrationOutcome, SourceOperationEvidence,
    register_pass,
};
use pliron::{
    context::Context,
    irbuild::IRStatus,
    op::Op,
    operation::{Operation, verify_operation},
    pass::{AnalysisManager, Pass},
};

fn config() -> LoweringConfig {
    LoweringConfig::new(
        LoweringLimits::new(1, 4, 8, 32, 4).expect("bounded limits"),
        2,
    )
    .expect("bounded rank")
}

fn source(context: &mut Context) -> MirModuleOp {
    let source_limits = MirDialectLimits::new(4, 4, 128).expect("bounded MIR limits");
    let module = MirModuleOp::try_new(context, "crate::kernels", source_limits)
        .expect("valid source module");
    let first = module
        .append_function(
            context,
            "crate::kernels::zeta",
            &[MirTypeId(7), MirTypeId(2)],
        )
        .expect("valid first function");
    first.append_block(context).expect("valid second block");
    module
        .append_function(context, "crate::kernels::alpha", &[])
        .expect("valid second function");
    module
}

fn lower() -> (Context, LoweringResult) {
    let mut context = Context::new();
    register_pass(&mut context).expect("explicit registration");
    let source = source(&mut context);
    let mut pass = MirKernelLoweringPass::new(config());
    pass.run_checked(source.get_operation(), &mut context)
        .expect("supported lowering");
    let result = pass.take_result().expect("successful result");
    (context, result)
}

#[test]
fn registration_is_explicit_idempotent_and_ordered() {
    assert_eq!(DIALECT_REGISTRATION_ORDER, ["mir", "kernel"]);

    let mut context = Context::new();
    assert_eq!(
        register_pass(&mut context),
        Ok(PassRegistrationOutcome::Registered)
    );
    assert_eq!(
        register_pass(&mut context),
        Ok(PassRegistrationOutcome::AlreadyRegistered)
    );

    let module = source(&mut context);
    verify_operation(module.get_operation(), &context).expect("MIR dialect is registered");
    let algorithm = AlgorithmOp::new(&mut context, 2).expect("kernel dialect entity");
    verify_operation(algorithm.get_operation(), &context).expect("kernel dialect is registered");
}

#[test]
fn lowering_record_is_deterministic_and_preserves_source_evidence() {
    let (left_context, left) = lower();
    let (right_context, right) = lower();

    assert_eq!(left.record(), right.record());
    assert_eq!(left.record().source().identity(), "crate::kernels");
    assert_eq!(left.record().source().block_count(), 3);
    assert_eq!(left.record().source().operation_count(), 9);
    assert_eq!(left.record().rewrite_count(), 2);
    assert!(!left.grants_authority());

    let functions = left.record().source().functions();
    assert_eq!(functions[0].ordinal(), 0);
    assert_eq!(functions[0].identity(), "crate::kernels::zeta");
    assert_eq!(
        functions[0].argument_type_ids(),
        &[MirTypeId(7), MirTypeId(2)]
    );
    assert_eq!(functions[0].blocks().len(), 2);
    assert_eq!(functions[0].blocks()[1].block_id(), 1);
    assert_eq!(
        functions[0].blocks()[1].operations(),
        &[
            SourceOperationEvidence::BlockMarker { block_id: 1 },
            SourceOperationEvidence::Return,
        ]
    );
    assert_eq!(functions[1].identity(), "crate::kernels::alpha");

    for (index, operation) in left.operations().iter().enumerate() {
        verify_operation(*operation, &left_context).expect("verified kernel output");
        let algorithm = Operation::get_op::<AlgorithmOp>(*operation, &left_context)
            .expect("kernel algorithm root");
        assert_eq!(algorithm.iteration_domain(&left_context).unwrap().rank(), 2);
        assert_eq!(
            left.record().steps()[index].source_function_ordinal(),
            index
        );
        assert_eq!(left.record().steps()[index].iteration_rank(), 2);
    }
    left.validate(&left_context).expect("left postconditions");
    right
        .validate(&right_context)
        .expect("right postconditions");
}

#[test]
fn pliron_pass_adapter_reports_detached_output_without_ir_change() {
    let mut context = Context::new();
    register_pass(&mut context).expect("registration");
    let source = source(&mut context);
    let mut pass = MirKernelLoweringPass::new(config());
    let mut analyses = AnalysisManager::default();

    assert_eq!(Pass::name(&pass), PASS_NAME);
    let pass_result = Pass::run(
        &mut pass,
        source.get_operation(),
        &mut context,
        &mut analyses,
    )
    .expect("Pliron adapter succeeds");

    assert_eq!(pass_result.ir_changed, IRStatus::Unchanged);
    assert!(pass.last_result().is_some());
}
