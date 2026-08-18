use dialect_kernel::{AlgorithmOp, IterationDomainAttr};
use dialect_mir::{
    MAX_EXECUTABLE_BLOCKS, MirBlockId,
    pliron::{
        MirBlockIdAttr, MirBlockOp, MirDialectLimits, MirFunctionOp, MirIdentityAttr,
        MirLimitsAttr, MirModuleOp, MirReturnOp,
    },
};
use fe2o3_lower_mir_kernel::{
    ConfigError, LimitKind, LoweringConfig, LoweringError, LoweringLimits, MAX_REWRITES,
    MAX_SOURCE_BLOCKS, MAX_SOURCE_FUNCTIONS, MAX_SOURCE_OPERATIONS, MAX_STRUCTURED_RANK,
    MirKernelLoweringPass, PASS_REGISTRATION_MARKER_KEY, PassRegistrationError, PostconditionError,
    SourceEntityKind, register_pass,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{attributes::TypeAttr, types::FunctionType, types::UnitType},
    context::Context,
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    r#type::TypeHandle,
};

fn limits(functions: usize, blocks: usize, operations: usize, rewrites: usize) -> LoweringLimits {
    LoweringLimits::new(1, functions, blocks, operations, rewrites).expect("bounded limits")
}

fn config_with(limits: LoweringLimits) -> LoweringConfig {
    LoweringConfig::new(limits, 1).expect("bounded config")
}

fn module_with_functions(context: &mut Context, count: usize) -> MirModuleOp {
    let mir_limits = MirDialectLimits::new(count.max(1), 4, 128).expect("bounded MIR limits");
    let module = MirModuleOp::try_new(context, "module", mir_limits).expect("valid module");
    for index in 0..count {
        module
            .append_function(context, format!("function_{index}"), &[])
            .expect("valid function");
    }
    module
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
fn rejects_missing_colliding_and_corrupt_registration() {
    let mut context = Context::new();
    let source = module_with_functions(&mut context, 1);
    let mut pass = MirKernelLoweringPass::new(config_with(limits(1, 1, 4, 1)));
    assert_eq!(
        pass.run_checked(source.get_operation(), &mut context),
        Err(LoweringError::PassNotRegistered)
    );

    let mut collision = Context::new();
    let key: Identifier = PASS_REGISTRATION_MARKER_KEY
        .try_into()
        .expect("valid marker key");
    let hostile = collision.aux_data.insert(Box::new(17_u32));
    collision.aux_data_map.insert(key.clone(), hostile);
    assert_eq!(
        register_pass(&mut collision),
        Err(PassRegistrationError::MarkerCollision)
    );
    collision.aux_data.remove(hostile);
    assert_eq!(
        register_pass(&mut collision),
        Err(PassRegistrationError::CorruptMarker)
    );

    let mut kernel_collision = Context::new();
    let kernel_key: Identifier = "fe2o3_dialect_kernel_registration_v1"
        .try_into()
        .expect("valid kernel marker key");
    let hostile = kernel_collision.aux_data.insert(Box::new(23_u32));
    kernel_collision.aux_data_map.insert(kernel_key, hostile);
    assert_eq!(
        register_pass(&mut kernel_collision),
        Err(PassRegistrationError::KernelDialect(
            dialect_kernel::RegistrationError::MarkerCollision
        ))
    );
}

#[test]
fn source_counts_and_rewrite_work_are_bounded_terminally() {
    let mut context = Context::new();
    register_pass(&mut context).expect("registration");

    let two_functions = module_with_functions(&mut context, 2);
    let mut function_limited = MirKernelLoweringPass::new(config_with(limits(1, 4, 16, 1)));
    assert_eq!(
        function_limited.run_checked(two_functions.get_operation(), &mut context),
        Err(LoweringError::SourceLimitExceeded {
            kind: LimitKind::Functions,
            observed: 2,
            limit: 1,
        })
    );
    assert!(function_limited.last_result().is_none());

    let one_function = module_with_functions(&mut context, 1);
    let function = Operation::get_op::<MirFunctionOp>(
        one_function
            .body(&context)
            .deref(&context)
            .get_head()
            .expect("function"),
        &context,
    )
    .expect("MIR function");
    function.append_block(&mut context).expect("second block");
    let mut block_limited = MirKernelLoweringPass::new(config_with(limits(1, 1, 16, 1)));
    assert_eq!(
        block_limited.run_checked(one_function.get_operation(), &mut context),
        Err(LoweringError::SourceLimitExceeded {
            kind: LimitKind::Blocks,
            observed: 2,
            limit: 1,
        })
    );

    let compact = module_with_functions(&mut context, 1);
    let mut operation_limited = MirKernelLoweringPass::new(config_with(limits(1, 1, 2, 1)));
    assert_eq!(
        operation_limited.run_checked(compact.get_operation(), &mut context),
        Err(LoweringError::SourceLimitExceeded {
            kind: LimitKind::Operations,
            observed: 3,
            limit: 2,
        })
    );

    let mut rewrite_limited = MirKernelLoweringPass::new(config_with(limits(2, 2, 16, 1)));
    assert_eq!(
        rewrite_limited.run_checked(two_functions.get_operation(), &mut context),
        Err(LoweringError::RewriteLimitExceeded {
            required: 2,
            limit: 1,
        })
    );
    assert!(rewrite_limited.last_result().is_none());
}

#[test]
fn unsupported_and_malformed_sources_never_produce_a_result() {
    let mut context = Context::new();
    register_pass(&mut context).expect("registration");
    let bounded = config_with(limits(4, 8, 32, 4));
    let mut pass = MirKernelLoweringPass::new(bounded);

    let foreign = AlgorithmOp::new(&mut context, 1).expect("valid foreign source");
    assert_eq!(
        pass.run_checked(foreign.get_operation(), &mut context),
        Err(LoweringError::UnsupportedSourceOperation)
    );

    let empty = module_with_functions(&mut context, 0);
    assert_eq!(
        pass.run_checked(empty.get_operation(), &mut context),
        Err(LoweringError::EmptyModule)
    );

    let foreign_child = module_with_functions(&mut context, 1);
    AlgorithmOp::new(&mut context, 1)
        .expect("foreign child")
        .get_operation()
        .insert_at_back(foreign_child.body(&context), &context);
    assert_eq!(
        pass.run_checked(foreign_child.get_operation(), &mut context),
        Err(LoweringError::UnsupportedModuleChild { ordinal: 1 })
    );

    let unsupported = module_with_functions(&mut context, 1);
    let function = Operation::get_op::<MirFunctionOp>(
        unsupported
            .body(&context)
            .deref(&context)
            .get_head()
            .unwrap(),
        &context,
    )
    .unwrap();
    AlgorithmOp::new(&mut context, 1)
        .expect("foreign body operation")
        .get_operation()
        .insert_at_back(function.entry_block(&context), &context);
    assert_eq!(
        pass.run_checked(unsupported.get_operation(), &mut context),
        Err(LoweringError::UnsupportedMirOperation {
            function: 0,
            block: 0,
            operation: 2,
        })
    );

    let malformed = module_with_functions(&mut context, 1);
    malformed
        .get_operation()
        .deref_mut(&context)
        .attributes
        .0
        .clear();
    assert_eq!(
        pass.run_checked(malformed.get_operation(), &mut context),
        Err(LoweringError::MalformedSourceEntity(
            SourceEntityKind::Module
        ))
    );

    let mir_limits = MirDialectLimits::new(1, 1, 64).expect("MIR limits");
    let raw = Operation::new(
        &mut context,
        MirModuleOp::get_concrete_op_info(),
        vec![],
        vec![],
        vec![],
        1,
    );
    let module_with_body_argument = MirModuleOp::from_operation(raw);
    module_with_body_argument
        .set_attr_module_identity(&context, MirIdentityAttr::new("body_argument"));
    module_with_body_argument.set_attr_module_limits(&context, MirLimitsAttr::new(mir_limits));
    let unit: TypeHandle = UnitType::get(&context).into();
    let body = BasicBlock::new(
        &mut context,
        Some("module".try_into().expect("valid label")),
        vec![unit],
    );
    body.insert_at_front(raw.deref(&context).get_region(0), &context);
    assert_eq!(
        pass.run_checked(raw, &mut context),
        Err(LoweringError::MalformedSourceEntity(
            SourceEntityKind::Module
        ))
    );
    assert!(pass.last_result().is_none());
}

#[test]
fn verified_foreign_argument_types_are_unsupported_not_erased() {
    let mut context = Context::new();
    register_pass(&mut context).expect("registration");
    let mir_limits = MirDialectLimits::new(1, 1, 64).expect("MIR limits");
    let module = MirModuleOp::try_new(&mut context, "module", mir_limits).expect("module");

    let unit: TypeHandle = UnitType::get(&context).into();
    let signature = FunctionType::get(&context, vec![unit], vec![]);
    let operation = Operation::new(
        &mut context,
        MirFunctionOp::get_concrete_op_info(),
        vec![],
        vec![],
        vec![],
        1,
    );
    let function = MirFunctionOp::from_operation(operation);
    function.set_attr_function_identity(&context, MirIdentityAttr::new("foreign_type"));
    function.set_attr_function_limits(&context, MirLimitsAttr::new(mir_limits));
    function.set_attr_function_signature(&context, TypeAttr::new(signature.into()));
    let entry = BasicBlock::new(
        &mut context,
        Some("bb0".try_into().expect("valid label")),
        vec![unit],
    );
    entry.insert_at_front(operation.deref(&context).get_region(0), &context);
    MirBlockOp::new(&mut context, MirBlockId(0))
        .get_operation()
        .insert_at_back(entry, &context);
    MirReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(entry, &context);
    operation.insert_at_back(module.body(&context), &context);

    let mut pass = MirKernelLoweringPass::new(config_with(limits(1, 1, 4, 1)));
    assert_eq!(
        pass.run_checked(module.get_operation(), &mut context),
        Err(LoweringError::UnsupportedArgumentType {
            function: 0,
            argument: 0,
        })
    );
    assert!(pass.last_result().is_none());
}

#[test]
fn verifier_failure_is_terminal_and_clears_prior_success() {
    let mut context = Context::new();
    register_pass(&mut context).expect("registration");
    let config = config_with(limits(1, 1, 4, 1));
    let source = module_with_functions(&mut context, 1);
    let mut pass = MirKernelLoweringPass::new(config);
    pass.run_checked(source.get_operation(), &mut context)
        .expect("initial success");
    assert!(pass.last_result().is_some());

    let function = Operation::get_op::<MirFunctionOp>(
        source.body(&context).deref(&context).get_head().unwrap(),
        &context,
    )
    .unwrap();
    let marker = MirBlockOp::from_operation(
        function
            .entry_block(&context)
            .deref(&context)
            .get_head()
            .unwrap(),
    );
    marker.set_attr_block_id(
        &context,
        MirBlockIdAttr::new(MirBlockId(MAX_EXECUTABLE_BLOCKS as u32)),
    );

    assert_eq!(
        pass.run_checked(source.get_operation(), &mut context),
        Err(LoweringError::SourceVerificationFailed)
    );
    assert!(pass.last_result().is_none());
}

#[test]
fn postconditions_detect_stale_source_and_mutated_kernel_output() {
    let mut context = Context::new();
    register_pass(&mut context).expect("registration");
    let source = module_with_functions(&mut context, 1);
    let mut pass = MirKernelLoweringPass::new(config_with(limits(1, 1, 4, 1)));
    pass.run_checked(source.get_operation(), &mut context)
        .expect("lowering");
    let result = pass.take_result().expect("result");

    source.set_attr_module_identity(&context, MirIdentityAttr::new("changed"));
    assert_eq!(
        result.validate(&context),
        Err(PostconditionError::SourceEvidenceMismatch)
    );
    source.set_attr_module_identity(&context, MirIdentityAttr::new("module"));

    let target = result.operations()[0];
    let algorithm = AlgorithmOp::from_operation(target);
    algorithm.set_iteration_domain(
        &context,
        IterationDomainAttr::new(2).expect("valid different rank"),
    );
    assert_eq!(
        result.validate(&context),
        Err(PostconditionError::InvalidKernelOperation { index: 0 })
    );

    source
        .get_operation()
        .deref_mut(&context)
        .attributes
        .0
        .clear();
    assert_eq!(
        result.validate(&context),
        Err(PostconditionError::SourceNoLongerValid)
    );
}
