use fe2o3_lower_mir_kernel::{
    ConfigError, LimitKind, LoweringConfig, LoweringError, LoweringLimits, MAX_REWRITES,
    MAX_SOURCE_BLOCKS, MAX_SOURCE_FUNCTIONS, MAX_SOURCE_OPERATIONS, MAX_STRUCTURED_RANK,
    MirKernelLoweringConformanceFunctionV1, MirKernelLoweringConformanceInputV1,
    MirKernelLoweringConformanceV1,
};

fn limits(functions: usize, blocks: usize, operations: usize, rewrites: usize) -> LoweringLimits {
    LoweringLimits::new(1, functions, blocks, operations, rewrites).expect("bounded limits")
}

fn config_with(limits: LoweringLimits) -> LoweringConfig {
    LoweringConfig::new(limits, 1).expect("bounded config")
}

fn functions(count: usize) -> MirKernelLoweringConformanceInputV1 {
    MirKernelLoweringConformanceInputV1::new(
        "module",
        (0..count)
            .map(|index| {
                MirKernelLoweringConformanceFunctionV1::new(format!("function_{index}"), vec![])
            })
            .collect(),
    )
}

#[test]
fn rejects_every_unbounded_configuration_dimension() {
    let cases = [
        (LimitKind::Modules, 0, 1),
        (LimitKind::Modules, 2, 1),
        (LimitKind::Functions, 0, MAX_SOURCE_FUNCTIONS),
        (
            LimitKind::Functions,
            MAX_SOURCE_FUNCTIONS + 1,
            MAX_SOURCE_FUNCTIONS,
        ),
        (LimitKind::Blocks, 0, MAX_SOURCE_BLOCKS),
        (LimitKind::Blocks, MAX_SOURCE_BLOCKS + 1, MAX_SOURCE_BLOCKS),
        (LimitKind::Operations, 0, MAX_SOURCE_OPERATIONS),
        (
            LimitKind::Operations,
            MAX_SOURCE_OPERATIONS + 1,
            MAX_SOURCE_OPERATIONS,
        ),
        (LimitKind::Rewrites, 0, MAX_REWRITES),
        (LimitKind::Rewrites, MAX_REWRITES + 1, MAX_REWRITES),
    ];

    for (kind, value, hard_limit) in cases {
        let result = match kind {
            LimitKind::Modules => LoweringLimits::new(value, 1, 1, 1, 1),
            LimitKind::Functions => LoweringLimits::new(1, value, 1, 1, 1),
            LimitKind::Blocks => LoweringLimits::new(1, 1, value, 1, 1),
            LimitKind::Operations => LoweringLimits::new(1, 1, 1, value, 1),
            LimitKind::Rewrites => LoweringLimits::new(1, 1, 1, 1, value),
        };
        assert_eq!(
            result,
            Err(ConfigError::LimitOutOfBounds {
                kind,
                value,
                hard_limit,
            })
        );
    }

    let bounded = limits(1, 1, 4, 1);
    assert_eq!(
        LoweringConfig::new(bounded, 0),
        Err(ConfigError::RankOutOfBounds(0))
    );
    assert_eq!(
        LoweringConfig::new(bounded, MAX_STRUCTURED_RANK + 1),
        Err(ConfigError::RankOutOfBounds(MAX_STRUCTURED_RANK + 1))
    );
}

#[test]
fn source_counts_and_rewrite_work_are_bounded_terminally() {
    let runner = MirKernelLoweringConformanceV1;
    assert_eq!(
        runner.run(&functions(2), config_with(limits(1, 4, 16, 1))),
        Err(LoweringError::SourceLimitExceeded {
            kind: LimitKind::Functions,
            observed: 2,
            limit: 1,
        })
    );

    let two_blocks = MirKernelLoweringConformanceInputV1::new(
        "module",
        vec![MirKernelLoweringConformanceFunctionV1::new("function", vec![]).with_block_count(2)],
    );
    assert_eq!(
        runner.run(&two_blocks, config_with(limits(1, 1, 16, 1))),
        Err(LoweringError::SourceLimitExceeded {
            kind: LimitKind::Blocks,
            observed: 2,
            limit: 1,
        })
    );

    assert_eq!(
        runner.run(&functions(1), config_with(limits(1, 1, 2, 1))),
        Err(LoweringError::SourceLimitExceeded {
            kind: LimitKind::Operations,
            observed: 3,
            limit: 2,
        })
    );
    assert_eq!(
        runner.run(&functions(2), config_with(limits(2, 2, 16, 1))),
        Err(LoweringError::RewriteLimitExceeded {
            required: 2,
            limit: 1,
        })
    );
}

#[test]
fn invalid_and_empty_recipes_fail_closed_without_state_reuse() {
    let runner = MirKernelLoweringConformanceV1;
    let config = config_with(limits(2, 2, 16, 2));
    let valid = functions(1);
    let first = runner.run(&valid, config.clone()).expect("valid recipe");

    assert_eq!(
        runner.run(&functions(0), config.clone()),
        Err(LoweringError::EmptyModule)
    );

    let zero_blocks = MirKernelLoweringConformanceInputV1::new(
        "module",
        vec![MirKernelLoweringConformanceFunctionV1::new("function", vec![]).with_block_count(0)],
    );
    assert_eq!(
        runner.run(&zero_blocks, config.clone()),
        Err(LoweringError::SourceVerificationFailed)
    );

    let duplicates = MirKernelLoweringConformanceInputV1::new(
        "module",
        vec![
            MirKernelLoweringConformanceFunctionV1::new("duplicate", vec![]),
            MirKernelLoweringConformanceFunctionV1::new("duplicate", vec![]),
        ],
    );
    assert_eq!(
        runner.run(&duplicates, config.clone()),
        Err(LoweringError::SourceVerificationFailed)
    );

    let after_failures = runner.run(&valid, config).expect("fresh private session");
    assert_eq!(first, after_failures);
}
