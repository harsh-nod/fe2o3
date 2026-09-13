use dialect_mir::MirTypeId;
use fe2o3_lower_mir_kernel::{
    DIALECT_REGISTRATION_ORDER, LoweringConfig, LoweringLimits,
    MirKernelLoweringConformanceFunctionV1, MirKernelLoweringConformanceInputV1,
    MirKernelLoweringConformanceV1, SourceOperationEvidence,
};

fn config() -> LoweringConfig {
    LoweringConfig::new(
        LoweringLimits::new(1, 4, 8, 32, 4).expect("bounded limits"),
        2,
    )
    .expect("bounded rank")
}

fn input() -> MirKernelLoweringConformanceInputV1 {
    MirKernelLoweringConformanceInputV1::new(
        "crate::kernels",
        vec![
            MirKernelLoweringConformanceFunctionV1::new(
                "crate::kernels::zeta",
                vec![MirTypeId(7), MirTypeId(2)],
            )
            .with_block_count(2),
            MirKernelLoweringConformanceFunctionV1::new("crate::kernels::alpha", vec![]),
        ],
    )
}

#[test]
fn registration_order_remains_fixed_inside_the_facade() {
    assert_eq!(DIALECT_REGISTRATION_ORDER, ["mir", "kernel"]);
}

#[test]
fn lowering_record_is_deterministic_and_preserves_source_evidence() {
    let left = MirKernelLoweringConformanceV1
        .run(&input(), config())
        .expect("supported lowering");
    let right = MirKernelLoweringConformanceV1
        .run(&input(), config())
        .expect("supported lowering");

    assert_eq!(left, right);
    assert_eq!(left.config(), &config());
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

    for (index, step) in left.record().steps().iter().enumerate() {
        assert_eq!(step.source_function_ordinal(), index);
        assert_eq!(step.iteration_rank(), 2);
    }
}
